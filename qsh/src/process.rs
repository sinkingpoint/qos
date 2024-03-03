use std::{
	ffi::CString,
	fs::File,
	io::{self, Read, Write},
	os::fd::FromRawFd,
};

use nix::{
	errno::Errno,
	sys::wait::{waitid, Id, WaitPidFlag, WaitStatus},
	unistd::{close, dup2, execvp, fork, pipe, setpgid, ForkResult, Pid},
};

use thiserror::Error;

/// The standard input file descriptor.
const STDIN_FD: i32 = 0;

/// The standard output file descriptor.
const STDOUT_FD: i32 = 1;

/// The standard error file descriptor.
const STDERR_FD: i32 = 2;

/// The exit code of a process.
#[derive(Debug, Clone, Copy)]
pub enum ExitCode {
	/// The process exited successfully with the given exit code.
	/// Exitting sucessfully doesn't necessarily mean the program didn't fail,
	/// it just means that the program exited with its own exit code.
	Success(i32),

	/// The process was terminated with the given error code.
	Err(Errno),
}

impl From<io::Error> for ExitCode {
	fn from(e: io::Error) -> Self {
		ExitCode::Err(Errno::from_i32(e.raw_os_error().unwrap_or(1)))
	}
}

/// The state of a process.
#[derive(Debug)]
pub enum ProcessState {
	/// The process has not been started.
	Unstarted,

	/// The process is currently running.
	Running(Pid),

	/// The process has terminated.
	Terminated(ExitCode),
}

/// The standard input, output, and error file descriptors.
#[derive(Debug, Clone, Copy)]
pub struct IOTriple {
	pub stdin: i32,
	pub stdout: i32,
	pub stderr: i32,
}

impl IOTriple {
	/// Gets a new `File` handle to the standard input.
	/// Note that this leaks the file handle, so any calling code
	/// should be careful to ensure that the fd is cleaned up.
	pub fn stdin(&self) -> impl Read + Write {
		let file = Box::new(unsafe { File::from_raw_fd(self.stdin) });
		Box::leak(file)
	}

	/// Gets a new `File` handle to the standard output.
	/// Note that this leaks the file handle, so any calling code
	/// should be careful to ensure that the fd is cleaned up.
	pub fn stdout(&self) -> impl Read + Write {
		let file = Box::new(unsafe { File::from_raw_fd(self.stdout) });
		Box::leak(file)
	}

	/// Gets a new `File` handle to the standard error.
	/// Note that this leaks the file handle, so any calling code
	/// should be careful to ensure that the fd is cleaned up.
	pub fn stderr(&self) -> impl Read + Write {
		let file = Box::new(unsafe { File::from_raw_fd(self.stderr) });
		Box::leak(file)
	}

	/// Create a new pipe, and return the read and write ends of the pipe.
	/// The first element of the tuple is the read end of the pipe, which should be used to read from the pipe,
	/// and the second element is the write end of the pipe which should be used to write to the pipe.
	pub fn pipe(&self) -> Result<(IOTriple, IOTriple), nix::Error> {
		let (read, write) = pipe()?;

		let read = IOTriple {
			stdin: read,
			stdout: self.stdout,
			stderr: self.stderr,
		};

		let write = IOTriple {
			stdin: self.stdin,
			stdout: write,
			stderr: self.stderr,
		};

		Ok((read, write))
	}
}

impl Default for IOTriple {
	fn default() -> Self {
		IOTriple {
			stdin: STDIN_FD,
			stdout: STDOUT_FD,
			stderr: STDERR_FD,
		}
	}
}

// A process that can be started and waited on.
#[derive(Debug)]
pub struct Process {
	pub argv: Vec<String>,
	pub state: ProcessState,
}

impl Process {
	pub fn new(argv: Vec<String>) -> Self {
		Process {
			argv,
			state: ProcessState::Unstarted,
		}
	}

	/// `exec` the process, replacing the current process with the new process.
	/// Because this function is always called in a child process, any persistent state set here will be lost.
	fn exec(&self, triple: IOTriple) {
		if triple.stdin != STDIN_FD {
			dup2(triple.stdin, STDIN_FD).unwrap();
			close(triple.stdin).unwrap();
		}

		if triple.stdout != STDOUT_FD {
			dup2(triple.stdout, STDOUT_FD).unwrap();
			close(triple.stdout).unwrap();
		}

		if triple.stderr != STDERR_FD {
			dup2(triple.stderr, STDERR_FD).unwrap();
			close(triple.stderr).unwrap();
		}

		let filename = CString::new(self.argv[0].as_str()).unwrap();
		let args: Vec<CString> = self
			.argv
			.iter()
			.map(|arg| CString::new(arg.as_str()).unwrap())
			.collect();

		if let Err(e) = execvp(&filename, &args) {
			if e == Errno::ENOENT {
				std::process::exit(127);
			}

			std::process::exit(e as i32);
		}

		// We can never reach this point (because we've `exec`ed), but the compiler doesn't know that.
		panic!("BUG: exec failed");
	}

	pub fn handle_wait_status(&mut self, status: WaitStatus) {
		match status {
			WaitStatus::Exited(_, code) => {
				self.state = ProcessState::Terminated(ExitCode::Success(code));
			}
			WaitStatus::Signaled(_, signal, _) => {
				self.state = ProcessState::Terminated(ExitCode::Err(Errno::from_i32(signal as i32)));
			}
			WaitStatus::Stopped(_, signal) => {
				self.state = ProcessState::Terminated(ExitCode::Err(Errno::from_i32(signal as i32)));
			}
			_ => {}
		}
	}

	/// Start the process in a new child process.
	pub fn start(&mut self, pgid: Option<Pid>, triple: IOTriple) -> nix::Result<()> {
		unsafe {
			match fork() {
				Ok(ForkResult::Parent { child }) => {
					if let Some(pgid) = pgid {
						setpgid(child, pgid)?;
					} else {
						setpgid(child, child)?;
					}
					self.state = ProcessState::Running(child);
				}
				Ok(ForkResult::Child) => {
					self.exec(triple);
				}
				Err(e) => {
					self.state = ProcessState::Terminated(ExitCode::Err(e));
				}
			}
		}

		Ok(())
	}
}

#[derive(Debug, Error)]
pub enum WaitError {
	#[error("Process is not running")]
	NotRunning,

	#[error("Nix error: {0}")]
	Nix(#[from] nix::Error),
}

/// The state of a pipeline of processes.
#[derive(Debug)]
pub enum PipelineState {
	Unstarted,
	// The process group ID of the pipeline.
	Running(Pid),
	Terminated,
}

/// A pipeline of processes.
pub struct ProcessPipeline {
	pub processes: Vec<Process>,
	pub status: PipelineState,
}

impl ProcessPipeline {
	pub fn new(processes: Vec<Process>) -> Self {
		ProcessPipeline {
			processes,
			status: PipelineState::Unstarted,
		}
	}

	// Execute the pipeline, starting each process in the pipeline.
	pub fn execute(&mut self, triple: IOTriple) -> Result<(), WaitError> {
		let (last, rest) = self.processes.split_last_mut().expect("BUG: empty commands");
		let mut triple = triple;
		let mut pgid = None;
		for command in rest.iter_mut() {
			let (read, write) = triple.pipe()?;
			command.start(pgid, write)?;

			// The process group ID of the pipeline will be the pgid of the first process in the pipeline.
			if pgid.is_none() {
				match command.state {
					ProcessState::Running(pid) => pgid = Some(pid),
					_ => return Err(WaitError::NotRunning),
				}
			}

			// Close any pipe file descriptors, because they've been moved into the child process.
			if write.stdin != STDIN_FD {
				close(write.stdin)?;
			}

			if write.stdout != STDOUT_FD {
				close(write.stdout)?;
			}

			if write.stderr != STDERR_FD {
				close(write.stderr)?;
			}

			triple = read;
		}

		last.start(pgid, triple)?;
		if pgid.is_none() {
			match last.state {
				ProcessState::Running(pid) => pgid = Some(pid),
				_ => return Err(WaitError::NotRunning),
			}
		}

		self.status = PipelineState::Running(pgid.unwrap());
		Ok(())
	}

	/// Returns true if all processes in the pipeline have finished.
	pub fn has_terminated(&self) -> bool {
		self.processes
			.iter()
			.all(|p| matches!(p.state, ProcessState::Terminated(_)))
	}

	fn get_process_by_id(&mut self, pid: Pid) -> Option<&mut Process> {
		self.processes.iter_mut().find(|p| match p.state {
			ProcessState::Running(pgid) => pgid == pid,
			_ => false,
		})
	}

	pub fn wait(&mut self) -> Result<(), WaitError> {
		let pgid = match self.status {
			PipelineState::Running(pgid) => pgid,
			_ => return Err(WaitError::NotRunning),
		};

		while !self.has_terminated() {
			let status = waitid(Id::PGid(pgid), WaitPidFlag::__WALL | WaitPidFlag::WEXITED)?;
			if let Some(pid) = status.pid() {
				match self.get_process_by_id(pid) {
					Some(process) => process.handle_wait_status(status),
					None => {
						// This should never happen, because we only wait on processes in the pipeline.
						panic!("BUG: process not found");
					}
				}
			}
		}

		self.status = PipelineState::Terminated;

		Ok(())
	}
}
