#![feature(extract_if)]
mod config;
mod service;

use std::{collections::HashMap, io::stderr, path::PathBuf, process::ExitCode, sync::Arc};

use anyhow::anyhow;
use common::obs::assemble_logger;
use config::load_config;
use service::{Service, ServiceManager};
use slog::{error, info};

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

async fn start_sphere(
	logger: &slog::Logger,
	manager: Arc<ServiceManager>,
	config: &config::Config,
	sphere_name: &str,
) -> anyhow::Result<()> {
	let sphere = match config.get_sphere(sphere_name) {
		Some(s) => s,
		None => return Err(anyhow!("sphere {} doesn't exist", sphere_name)),
	};

	info!(logger, "starting sphere"; "sphere_name" => sphere_name);
	for dep in sphere.services.iter() {
		start_service(logger, manager.clone(), config, &dep.name, dep.arguments.clone()).await?;
	}

	Ok(())
}

/// Starts a service and its dependencies, returning an error if the service can't be started due to dependency issues.
async fn start_service(
	_logger: &slog::Logger,
	manager: Arc<ServiceManager>,
	config: &config::Config,
	service_name: &str,
	service_args: HashMap<String, String>,
) -> anyhow::Result<()> {
	let service_config = match config.get_service_config(service_name) {
		Some(conf) => conf,
		None => return Err(anyhow!("service {} doesn't exist", service_name)),
	};

	let mut to_start = Vec::new();
	let mut stack = vec![(service_config, service_args)];

	while let Some((service_config, args)) = stack.pop() {
		let dep_service = Service::new(service_config, args);
		if to_start.iter().any(|(s, _): &(Service, _)| s.matches(&dep_service)) {
			continue;
		}

		let mut dependencies = Vec::new();
		for dep in service_config.needs.iter() {
			let config = match config.get_service_config(&dep.name) {
				Some(conf) => conf,
				None => {
					return Err(anyhow!(
						"BUG: service {} needs service {}, but it doesn't exist",
						service_name,
						dep.name
					))
				}
			};

			stack.push((config, dep.arguments.clone()));

			dependencies.push(Service::new(config, dep.arguments.clone()));
		}

		to_start.push((dep_service, dependencies));
	}

	for (service, deps) in to_start {
		manager.queue(service, deps).await;
	}

	Ok(())
}
