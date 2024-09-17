#![feature(extract_if)]
mod config;
mod service;

use std::{
	collections::HashMap,
	io::{self, stderr},
	path::PathBuf,
	process::ExitCode,
	sync::Arc,
	time::Duration,
};

use anyhow::{anyhow, Result};
use clap::{Arg, Command};
use common::obs::assemble_logger;
use config::{load_config, Dependency};
use control::listen::{Action, ActionFactory, ControlSocket};
use nix::unistd::Pid;
use service::{Service, ServiceManager};
use slog::{error, info};
use tokio::{fs::create_dir_all, net::unix::UCred, time::sleep};

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

	start_sphere(&logger, manager.clone(), &config, "user").await.unwrap();

	sleep(Duration::from_secs(5)).await;

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
	info!(logger, "queuing sphere"; "name" => sphere_name);
	let sphere = match config.get_sphere(sphere_name) {
		Some(s) => s,
		None => return Err(anyhow!("sphere {} doesn't exist", sphere_name)),
	};

	let mut to_start = Vec::new();
	let mut stack = vec![sphere];
	while let Some(sphere) = stack.pop() {
		for dep_name in sphere.needs.iter() {
			let sphere = match config.get_sphere(dep_name) {
				Some(s) => s,
				None => return Err(anyhow!("sphere {} doesn't exist", sphere_name)),
			};

			stack.push(sphere);
		}

		to_start.push(sphere);
	}

	let mut started: HashMap<String, Vec<Dependency>> = HashMap::new();
	while !to_start.is_empty() {
		let mut new_started = HashMap::new();
		for startable in to_start
			.iter()
			.filter(|d| d.needs.iter().all(|s| started.contains_key(s)))
		{
			let deps = startable
				.needs
				.iter()
				.flat_map(|n| started.get(n).unwrap())
				.cloned()
				.collect::<Vec<Dependency>>();

			let mut new_deps = Vec::new();
			for dep in startable.services.iter() {
				start_service(
					logger,
					manager.clone(),
					config,
					&dep.name,
					dep.arguments.clone(),
					Some(&deps),
				)
				.await?;

				new_deps.push(dep.clone());
			}

			new_deps.extend(deps);
			new_started.insert(startable.name.clone(), new_deps);
		}

		to_start.retain(|s| !new_started.contains_key(&s.name));
		started.extend(new_started);
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
	extra_deps: Option<&Vec<Dependency>>,
) -> anyhow::Result<()> {
	let service_config = match config.get_service_config(service_name) {
		Some(conf) => conf,
		None => return Err(anyhow!("service {} doesn't exist", service_name)),
	};

	let mut to_start = Vec::new();
	let mut stack = vec![(service_config, service_args)];
	let default_extra_deps = Vec::new();
	let extra_deps = extra_deps.unwrap_or(&default_extra_deps);

	while let Some((service_config, args)) = stack.pop() {
		let dep_service = Service::new(service_config, args);
		if to_start.iter().any(|(s, _): &(Service, _)| s.matches(&dep_service)) {
			continue;
		}

		let mut dependencies: Vec<Service> = Vec::new();
		for dep in service_config.needs.iter().chain(extra_deps) {
			if dependencies.iter().any(|s| s.matches_dep(dep)) || dep_service.matches_dep(dep) {
				continue;
			}

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
