mod config;
mod service;

use std::{collections::HashMap, io::stderr, path::PathBuf, process::ExitCode, sync::Arc};

use anyhow::Context;
use common::obs::assemble_logger;
use config::load_config;
use service::{Service, ServiceManager};
use slog::error;

#[tokio::main]
async fn main() -> ExitCode {
	let logger = assemble_logger(stderr());

	let config_directories = ["./configs/services", "/etc/qinit/services"].map(PathBuf::from);

	let (config, errors) = load_config(config_directories);
	if errors.is_error() {
		error!(logger, "Error loading configuration"; "errors" => format!("{:?}", errors));
	}

	let errors = config.validate();
	if errors.is_error() {
		error!(logger, "Error validating configuration"; "errors" => format!("{:?}", errors));
		if errors.is_fatal() {
			return ExitCode::FAILURE;
		}
	}

	let manager = Arc::new(ServiceManager::new(logger.clone()));

	start_sphere(&logger, manager.clone(), &config, "base").await.unwrap();

	manager.reaper().await;
	ExitCode::SUCCESS
}

/// Starts a service and its dependencies, returning an error if the service can't be started due to dependency issues.
async fn start_service(
	logger: &slog::Logger,
	manager: Arc<ServiceManager>,
	config: &config::Config,
	service_name: &str,
	service_args: HashMap<String, String>,
) -> anyhow::Result<()> {
	let (to_start, wants) = match config.resolve_to_service_set(service_name, service_args) {
		Ok(services) => services,
		Err(errors) => {
			error!(logger, "Error resolving services"; "errors" => format!("{:?}", errors));
			return Err(anyhow::anyhow!("Error resolving services"));
		}
	};

	for (config, args) in wants {
		manager
			.satisfies(&Service::new(config, args))
			.await
			.with_context(|| format!("Service {} wants {}", service_name, config.name))?;
	}

	for (config, args) in to_start {
		let service = Service::new(config, args);
		manager.start(service).await?;
	}

	Ok(())
}

/// Starts a sphere and its dependencies, returning an error if the sphere can't be started due to dependency issues.
async fn start_sphere(
	logger: &slog::Logger,
	manager: Arc<ServiceManager>,
	config: &config::Config,
	sphere_name: &str,
) -> anyhow::Result<()> {
	let (to_start, wants) = match config.resolve_sphere_to_service_set(sphere_name) {
		Ok(sphere) => sphere,
		Err(errors) => {
			error!(logger, "Error resolving sphere"; "errors" => format!("{:?}", errors));
			return Err(anyhow::anyhow!("Error resolving sphere"));
		}
	};

	for (config, args) in wants {
		manager
			.satisfies(&Service::new(config, args))
			.await
			.with_context(|| format!("Sphere {} wants {}", sphere_name, config.name))?;
	}

	for (config, args) in to_start {
		println!("Starting service: {:?} with arguments {:?}", config.name, args);
		let service = Service::new(config, args);
		manager.start(service).await?;
	}

	Ok(())
}
