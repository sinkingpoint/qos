use std::{
	collections::HashMap,
	ffi::{CStr, CString},
	future::Future,
	pin::Pin,
	task::{Context, Poll},
};

use auth::{Group, User};
use slog::error;
use tokio::sync::{oneshot, Mutex, Notify};

use anyhow::{anyhow, Context as _, Result};
use nix::{
	errno::Errno,
	sys::{
		signal::Signal,
		wait::{waitpid, WaitPidFlag, WaitStatus},
	},
	unistd::{execve, fork, setgid, setuid, ForkResult, Gid, Pid, Uid},
};

use crate::config::Permissions;
use crate::config::ServiceConfig;

#[derive(Debug)]
pub enum ServiceState {
	Stopped,
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
}

impl Service {
	pub fn new(config: &ServiceConfig, args: HashMap<String, String>) -> Self {
		Self {
			name: config.name.clone(),
			args,
			command: config.service.command.clone(),
			state: ServiceState::Stopped,
			permissions: config.permissions.clone(),
		}
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

		setgid(Gid::from_raw(group.gid))?;
		setuid(Uid::from_raw(user.uid))?;

		Ok(())
	}

	/// Starts the service, forking and executing the command.
	pub fn start(&mut self) -> Result<()> {
		let args = self.split_args()?.unwrap();
		match unsafe { fork()? } {
			ForkResult::Parent { child } => {
				self.state = ServiceState::Running(child);
			}
			ForkResult::Child => {
				// Setup all the pre-execution stuff. `unwrap` is fine here because we absolutely shouldn't return
				// in the child process.

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

/// Manages the services that the system has started.
pub struct ServiceManager {
	/// The services that have been started.
	services: Mutex<Vec<Service>>,

	/// A notify that is triggered when a new service is started.
	new_service_notify: Notify,

	logger: slog::Logger,
}

impl ServiceManager {
	pub fn new(logger: slog::Logger) -> Self {
		Self {
			services: Mutex::new(Vec::new()),
			new_service_notify: Notify::new(),
			logger,
		}
	}

	/// Checks if there is a service running that satisfies the given service.
	pub async fn satisfies(&self, wants: &Service) -> anyhow::Result<()> {
		let services = self.services.lock().await;
		for s in services.iter() {
			if s.name != wants.name {
				continue;
			}

			for (key, value) in &wants.args {
				if s.args.get(key) != Some(value) {
					return Err(anyhow!("Missing {} = {} in service {}", key, value, s.name));
				}
			}

			if !matches!(s.state, ServiceState::Running(_)) {
				return Err(anyhow!("Service {} is not running", s.name));
			}
		}

		Ok(())
	}

	/// Starts a new service.
	pub async fn start(&self, mut service: Service) -> Result<()> {
		let mut services = self.services.lock().await;
		service.start()?;

		services.push(service);

		self.new_service_notify.notify_one();

		Ok(())
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
		if let Some(service) = services.iter_mut().find(|s| match s.state {
			ServiceState::Running(p) => p == pid,
			_ => false,
		}) {
			match status {
				WaitStatus::Exited(_, status) => {
					service.state = ServiceState::Terminated(status);
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
