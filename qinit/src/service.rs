use std::{
	collections::HashMap,
	ffi::{CStr, CString},
};

use anyhow::Result;
use nix::unistd::{execve, fork, ForkResult, Pid};

use crate::config::ServiceConfig;

pub enum ServiceState {
	Stopped,
	Running(Pid),
	Terminated(u32),
}

pub struct Service {
	name: String,
	args: HashMap<String, String>,
	command: String,
	state: ServiceState,
}

impl Service {
	pub fn new(config: &ServiceConfig, args: HashMap<String, String>) -> Self {
		Self {
			name: config.name.clone(),
			args,
			command: config.service.command.clone(),
			state: ServiceState::Stopped,
		}
	}

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

	fn template(&self, command: &str) -> String {
		let mut command = command.to_string();
		for (key, value) in &self.args {
			command = command.replace(&format!("${{{}}}", key), value);
		}

		command
	}

	pub fn start(&mut self) -> Result<()> {
		let args = self.split_args()?.unwrap();
		println!("Starting service: {} {:?}", args[0].to_str()?, args);
		match unsafe { fork()? } {
			ForkResult::Parent { child } => {
				self.state = ServiceState::Running(child);
			}
			ForkResult::Child => {
				execve::<_, &CStr>(&args[0], &args, &[]).unwrap();
			}
		};

		Ok(())
	}
}
