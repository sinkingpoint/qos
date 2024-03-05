use std::{
	fs::File,
	io::{self, BufRead, BufReader, Read, Write},
	os::fd::FromRawFd,
};

use nix::unistd::pipe;

/// The standard input file descriptor.
pub const STDIN_FD: i32 = 0;

/// The standard output file descriptor.
pub const STDOUT_FD: i32 = 1;

/// The standard error file descriptor.
pub const STDERR_FD: i32 = 2;

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

	/// Prompts the user for input on the stdout fd, reads a line from the stdin fd, and returns the input.
	pub fn prompt(&self, prompt: &str) -> io::Result<String> {
		write!(self.stdout(), "{} ", prompt)?;
		let mut input = String::new();
		BufReader::new(self.stdin()).read_line(&mut input)?;
		Ok(input)
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
