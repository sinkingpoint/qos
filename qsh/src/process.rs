use std::{
    ffi::CString,
    fs::File,
    io::{self, Read, Write},
    os::fd::FromRawFd,
};

use nix::{
    errno::Errno,
    sys::wait::{waitpid, WaitPidFlag, WaitStatus},
    unistd::{close, dup2, execvp, fork, Pid},
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
        let args: Vec<CString> = self.argv.iter().map(|arg| CString::new(arg.as_str()).unwrap()).collect();

        if let Err(e) = execvp(&filename, &args) {
            if e == Errno::ENOENT {
                std::process::exit(127);
            } else {
                std::process::exit(e as i32);
            }
        }
    }

    /// Start the process in a new child process.
    pub fn start(&mut self, triple: IOTriple) -> nix::Result<()> {
        unsafe {
            match fork() {
                Ok(nix::unistd::ForkResult::Parent { child }) => {
                    self.state = ProcessState::Running(child);
                }
                Ok(nix::unistd::ForkResult::Child) => {
                    self.exec(triple);
                }
                Err(e) => {
                    self.state = ProcessState::Terminated(ExitCode::Err(e));
                }
            }
        }

        Ok(())
    }

    /// Block until the process exits, or is otherwise stopped.
    pub fn wait(&mut self) -> Result<(), WaitError> {
        // If the process is not running, return an error.
        let current_pid = match self.state {
            ProcessState::Running(pid) => pid,
            _ => return Err(WaitError::NotRunning),
        };

        match waitpid(current_pid, Some(WaitPidFlag::__WALL | WaitPidFlag::WUNTRACED)) {
            Ok(WaitStatus::Exited(_, exit)) => {
                self.state = ProcessState::Terminated(ExitCode::Success(exit));
            }
            Ok(WaitStatus::Signaled(_, signal, _)) => {
                self.state = ProcessState::Terminated(ExitCode::Err(Errno::from_i32(signal as i32)));
            }
            Ok(WaitStatus::Continued(_)) => {
                self.state = ProcessState::Running(current_pid);
            }
            Err(e) => {
                return Err(WaitError::Nix(e));
            }
            _ => return Err(WaitError::UnsupportedSignal),
        };

        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum WaitError {
    #[error("Process is not running")]
    NotRunning,

    #[error("Unsupported Signal")]
    UnsupportedSignal,

    #[error("Nix error: {0}")]
    Nix(#[from] nix::Error),
}
