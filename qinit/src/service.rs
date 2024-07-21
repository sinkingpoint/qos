use std::{
	collections::HashMap,
	env::set_current_dir,
	ffi::{CStr, CString},
	fmt::Display,
	fs::create_dir_all,
	future::Future,
	pin::Pin,
	task::{Context, Poll},
};

use auth::{Group, User};
use slog::{error, info, warn};
use tokio::sync::{oneshot, Mutex, Notify};

use anyhow::{anyhow, Context as _, Result};
use nix::{
	errno::Errno,
	sys::{
		signal::Signal,
		wait::{waitpid, WaitPidFlag, WaitStatus},
	},
	unistd::{chown, execve, fork, setgid, setuid, ForkResult, Gid, Pid, Uid},
};

use crate::config::{Permissions, ServiceConfig, StartMode};

#[derive(Debug)]
#[allow(dead_code)] // Some of the variants aren't used yet, but will be once we have a ctl binary.
pub enum ServiceState {
	// The service failed to start during the exec process.
	Error(anyhow::Error),
	Stopped,
	// The service has been started, but has not yet hit its started conditions.
	Started(Pid),

	// The service has been started, and has finished its startup process.
	Running(Pid),
	Signaled(Pid, Signal),
	Terminated(i32),
}

#[derive(Debug)]
pub struct Service {
	name: String,
	args: HashMap<String, String>,
	command: String,
	state: ServiceState,

	permissions: Permissions,
	runtime_directory: Option<String>,
	start_mode: StartMode,
}

impl Service {
	pub fn new(config: &ServiceConfig, args: HashMap<String, String>) -> Self {
		Self {
			name: config.name.clone(),
			args,
			command: config.service.command.clone(),
			state: ServiceState::Stopped,
			permissions: config.permissions.clone(),
			runtime_directory: config.runtime_directory.clone(),
			start_mode: config.start_mode,
		}
	}

	pub fn matches(&self, other: &Service) -> bool {
		if self.name != other.name {
			return false;
		}

		for (key, value) in &other.args {
			if self.args.get(key) != Some(value) {
				return false;
			}
		}

		true
	}

	/// Splits the command into arguments that can be passed to `execve`.
	fn split_args(&self) -> Result<Option<Vec<CString>>> {
		let mut parts = self.command.split_whitespace().peekable();
		if parts.peek().is_none() {
			return Ok(None);
		}

		let args = parts
			.map(|s| Ok(CString::new(self.template(s))?))
			.collect::<Result<Vec<CString>>>()?;

		Ok(Some(args))
	}

	/// Replaces the template variables in the command with the arguments.
	fn template(&self, command: &str) -> String {
		let mut command = command.to_string();
		for (key, value) in &self.args {
			command = command.replace(&format!("${{{}}}", key), value);
		}

		command
	}

	/// Sets the user and group for the service.
	fn set_user_group(&self) -> Result<()> {
		let user = match User::from_username(&self.permissions.user)? {
			Some(user) => user,
			None if self.permissions.create => User::create(
				&self.permissions.user,
				None,
				None,
				"",
				&format!("/run/{}", self.permissions.user),
				None,
			)?,
			None => return Err(anyhow!(format!("User not found: {}", self.permissions.user))),
		};

		let group = match Group::from_groupname(&self.permissions.group)? {
			Some(group) => group,
			None if self.permissions.create => Group::create(&self.permissions.group, None)?,
			None => return Err(anyhow!(format!("Group not found: {}", self.permissions.group))),
		};

		let uid = Uid::from_raw(user.uid);
		let gid = Gid::from_raw(group.gid);

		// Change the ownership of the runtime directory.
		if let Some(runtime_dir) = &self.runtime_directory {
			chown(runtime_dir.as_str(), Some(uid), Some(gid))?;
		}

		setgid(gid)?;
		setuid(uid)?;

		Ok(())
	}

	fn set_runtime_directory(&self) -> Result<()> {
		if let Some(ref directory) = self.runtime_directory {
			// Create the directory if it doesn't exist.
			create_dir_all(directory).with_context(|| format!("failed to create runtime directory: {}", directory))?;

			// Change the working directory.
			set_current_dir(directory).with_context(|| format!("failed to set runtime directory: {}", directory))?;
		}

		Ok(())
	}

	/// Starts the service, forking and executing the command.
	pub fn start(&mut self) -> Result<()> {
		let args = self.split_args()?.unwrap();
		match unsafe { fork()? } {
			ForkResult::Parent { child } => {
				self.state = ServiceState::Started(child);
			}
			ForkResult::Child => {
				// Setup all the pre-execution stuff. `unwrap` is fine here because we absolutely shouldn't return
				// in the child process.

				self.set_runtime_directory()
					.with_context(|| {
						format!(
							"failed to start service name: {}, args: {:?}: failed to set runtime directory",
							self.name, self.args
						)
					})
					.unwrap();

				// Set the user and group. This should be last as it may drop permissions and we wont be root anymore.
				self.set_user_group()
					.with_context(|| {
						format!(
							"failed to start service name: {}, args: {:?}: failed to set user and group",
							self.name, self.args
						)
					})
					.unwrap();

				execve::<_, &CStr>(&args[0], &args, &[])
					.with_context(|| format!("failed to start service name: {}, args: {:?}", self.name, self.args))
					.unwrap();
			}
		};

		Ok(())
	}
}

impl Display for Service {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_fmt(format_args!("{} (", self.name))?;

		for (k, v) in self.args.iter() {
			f.write_fmt(format_args!(" {}=\"{}\"", k, v))?;
		}

		f.write_str(")")
	}
}

/// Manages the services that the system has started.
pub struct ServiceManager {
	/// The services that have been started.
	services: Mutex<Vec<Service>>,

	/// The services that are waiting on other services to start.
	pending_services: Mutex<Vec<ServiceWaiter>>,

	/// A notify that is triggered when a new service is started.
	new_service_notify: Notify,

	logger: slog::Logger,
}

impl ServiceManager {
	pub fn new(logger: slog::Logger) -> Self {
		Self {
			services: Mutex::new(Vec::new()),
			pending_services: Mutex::new(Vec::new()),
			new_service_notify: Notify::new(),
			logger,
		}
	}

	/// Checks if there is a service running that satisfies the given service.
	pub async fn is_running(&self, wants: &Service) -> bool {
		let services = self.services.lock().await;
		for s in services.iter() {
			if !s.matches(wants) {
				continue;
			}

			return matches!(s.state, ServiceState::Running(_));
		}

		false
	}

	/// Adds the given service to the queue of services to start, starting it if
	/// all its dependencies are running, or putting it in a pending state if not.
	pub async fn queue(&self, service: Service, dependencies: Vec<Service>) {
		{
			let services = self.services.lock().await;
			if services.iter().any(|s| s.matches(&service)) {
				return;
			}
		}

		let mut unmet_dependencies = Vec::new();
		for dep in dependencies.into_iter() {
			if !self.is_running(&dep).await {
				unmet_dependencies.push(dep);
			}
		}

		if unmet_dependencies.is_empty() {
			self.start(service).await;
		} else {
			let mut pending_services = self.pending_services.lock().await;
			let watcher = ServiceWaiter::new(service, unmet_dependencies);
			pending_services.push(watcher);
		}
	}

	/// Starts the given service, forking a child and handling start modes.
	async fn start(&self, mut service: Service) {
		info!(self.logger, "starting service"; "service" => service.to_string());
		let start_future = async move {
			if let Err(e) = service.start() {
				service.state = ServiceState::Error(e);
				return;
			}

			let pid = match service.state {
				ServiceState::Started(pid) => pid,
				_ => return,
			};

			let start_mode = service.start_mode;

			{
				let mut services = self.services.lock().await;
				services.push(service);
			}

			if start_mode == StartMode::Run {
				self.mark_service_running(pid).await;
			}

			// Notify the reaper that it should start listening for chiildren again.
			self.new_service_notify.notify_one();
		};

		// We have to pin the future here because this could recurse:
		// -> service gets marked as running, which triggers a waiting service, which triggers a `start` call.
		Box::pin(start_future).await;
	}

	/// Marks the given service as running, notifying any pending services.
	pub async fn mark_service_running(&self, pid: Pid) {
		let mut services = self.services.lock().await;
		let service = services.iter_mut().find(|s| match s.state {
			ServiceState::Running(p) | ServiceState::Started(p) => p == pid,
			_ => false,
		});

		if let Some(service) = service {
			if matches!(service.state, ServiceState::Running(_)) {
				// The service is already running, so this is a no-op, and a bug probably.
				warn!(
					self.logger,
					"Service {}({}) is being marked as running, but is already running", service.name, pid
				);
				return;
			}

			service.state = ServiceState::Running(pid);
			self.trigger_start_sweep(service).await;
		} else {
			warn!(
				self.logger,
				"PID {} is being marked as running, but is not managed by this version of qinit", pid
			);

			println!("{:?}", services);
		}
	}

	/// Sweep the pending services, starting any that were only waiting on the given service to start.
	async fn trigger_start_sweep(&self, started: &Service) {
		let mut pending = self.pending_services.lock().await;
		for to_start in pending.extract_if(|w| {
			w.notify_service_started(started);
			w.done()
		}) {
			self.start(to_start.service).await;
		}
	}

	/// Sets the status of a process.
	async fn set_process_status(&self, status: WaitStatus) {
		// If there is no PID, we can't do anything.
		let pid = match status.pid() {
			Some(pid) => pid,
			None => return,
		};

		// Find the service that the process belongs to and update its status.
		let mut services = self.services.lock().await;
		let service = services.iter_mut().find(|s| match s.state {
			ServiceState::Running(p) | ServiceState::Started(p) => p == pid,
			_ => false,
		});

		if let Some(service) = service {
			match status {
				WaitStatus::Exited(_, status) => {
					service.state = ServiceState::Terminated(status);
					if status == 0 && service.start_mode == StartMode::Done {
						// Done services are considered "started" when they exit.
						self.trigger_start_sweep(service).await;
					}
				}
				WaitStatus::Signaled(_, signal, _) | WaitStatus::Stopped(_, signal) => {
					service.state = ServiceState::Signaled(pid, signal);
				}
				WaitStatus::Continued(_) => {
					service.state = ServiceState::Running(pid);
				}
				_ => {
					error!(self.logger, "Unknown status: {:?}", status);
				}
			}
		} else {
			error!(self.logger, "Received status for zombie process: {:?}", status; "pid"=>pid.to_string());
		}
	}

	/// Infinitely waits for services to exit, marking their status.
	pub async fn reaper(&self) {
		self.new_service_notify.notified().await;
		loop {
			let pid = WaitFuture::new(Pid::from_raw(-1), WaitPidFlag::WNOHANG | WaitPidFlag::__WALL).await;
			match pid {
				Ok(status) => self.set_process_status(status).await,
				Err(Errno::ECHILD) => self.new_service_notify.notified().await,
				Err(err) => {
					error!(self.logger, "Error waiting for service"; "error"=>format!("{:?}", err));
				}
			}
		}
	}
}

/// A future that waits for a process to exit.
enum WaitFuture {
	/// The future has been created, but not yet `await`ed.
	Created(Pid, WaitPidFlag),

	/// The future is running, waiting for a process to exit.
	Running(oneshot::Receiver<nix::Result<WaitStatus>>),

	/// A process exited.
	Terminated(nix::Result<WaitStatus>),
}

impl WaitFuture {
	fn new(pid: Pid, flags: WaitPidFlag) -> Self {
		Self::Created(pid, flags)
	}
}

impl Future for WaitFuture {
	type Output = nix::Result<WaitStatus>;

	fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> std::task::Poll<Self::Output> {
		match *self {
			Self::Created(ref pid, ref flags) => {
				let (tx, rx) = oneshot::channel();
				let waker = cx.waker().clone();

				// Spawn a new thread to block on the waitpid call, and wake once it's sent data through the oneshot channel.
				let pid = *pid;
				let flags = *flags;
				std::thread::spawn(move || {
					tx.send(waitpid(pid, Some(flags))).unwrap();
					waker.wake();
				});

				*self = Self::Running(rx);
				Poll::Pending
			}
			Self::Running(ref mut rx) => match rx.try_recv() {
				Ok(output) => {
					*self = Self::Terminated(output);
					Poll::Ready(output)
				}
				Err(_) => Poll::Pending,
			},
			Self::Terminated(output) => Poll::Ready(output),
		}
	}
}

/// A service that is waiting on some set of dependencies.
struct ServiceWaiter {
	/// The service to start
	service: Service,

	/// The remaining dependencies for the service, if any.
	waiting_dependencies: Vec<Service>,
}

impl ServiceWaiter {
	fn new(service: Service, dependencies: Vec<Service>) -> Self {
		Self {
			service,
			waiting_dependencies: dependencies,
		}
	}

	/// Remove the given service from the set of dependencies.
	fn notify_service_started(&mut self, started: &Service) {
		self.waiting_dependencies.retain(|s| !started.matches(s))
	}

	fn done(&self) -> bool {
		self.waiting_dependencies.is_empty()
	}
}
