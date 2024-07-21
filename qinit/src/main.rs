#![feature(extract_if)]
mod config;
mod service;

use std::{
	collections::HashMap,
	io::{self, stderr},
	path::PathBuf,
	process::ExitCode,
	sync::Arc,
};

use anyhow::{anyhow, Result};
use clap::{Arg, Command};
use common::obs::assemble_logger;
use config::load_config;
use control::listen::{Action, ActionFactory, ControlSocket};
use nix::unistd::Pid;
use service::{Service, ServiceManager};
use slog::{error, info};
use tokio::{fs::create_dir_all, net::unix::UCred};

#[tokio::main]
async fn main() -> ExitCode {
	let matches = Command::new("qinit")
		.arg(Arg::new("socket").num_args(1).default_value("/run/qinit/control.sock"))
		.get_matches();

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

	let socket_path: &String = matches.get_one("socket").unwrap();
	if let Err(e) = open_control_socket(socket_path, manager.clone()).await {
		error!(logger, "failed to open control socket"; "error" => e);
		return ExitCode::FAILURE;
	}

	start_sphere(&logger, manager.clone(), &config, "base").await.unwrap();

	manager.reaper().await;
	ExitCode::SUCCESS
}

async fn open_control_socket(socket_path: &str, manager: Arc<ServiceManager>) -> io::Result<()> {
	let socket_path = PathBuf::from(socket_path);

	if let Some(parent) = socket_path.parent() {
		if !parent.exists() {
			create_dir_all(parent).await?;
		}
	}

	let socket = ControlSocket::open(&socket_path, ControlFactory::new(manager))?;

	tokio::spawn(async move { socket.listen().await });
	Ok(())
}

enum ControlActionType {
	Ready,
}

struct ControlAction {
	ty: ControlActionType,
	manager: Arc<ServiceManager>,
}

impl ControlAction {
	fn new(ty: ControlActionType, manager: Arc<ServiceManager>) -> Self {
		Self { ty, manager }
	}
}

impl Action for ControlAction {
	type Error = anyhow::Error;

	async fn run<
		R: tokio::io::AsyncBufRead + Unpin + Send + 'static,
		W: tokio::io::AsyncWrite + Unpin + Send + 'static,
	>(
		self,
		peer: UCred,
		_reader: R,
		_writer: W,
	) -> Result<(), Self::Error> {
		match self.ty {
			ControlActionType::Ready => {
				let pid = peer.pid().expect("failed to get pid");
				self.manager.mark_service_running(Pid::from_raw(pid)).await;
				Ok(())
			}
		}
	}
}

#[derive(Clone)]
struct ControlFactory {
	manager: Arc<ServiceManager>,
}

impl ActionFactory for ControlFactory {
	type Action = ControlAction;

	fn build(&self, action: &str, _args: &[(&str, &str)]) -> Result<Self::Action, <Self::Action as Action>::Error> {
		match action {
			"running" => Ok(ControlAction::new(ControlActionType::Ready, self.manager.clone())),
			_ => Err(anyhow!("unsupported action: {}", action)),
		}
	}
}

impl ControlFactory {
	fn new(manager: Arc<ServiceManager>) -> Self {
		ControlFactory { manager }
	}
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
