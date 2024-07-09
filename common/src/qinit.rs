use std::{
	io::{self, Write},
	os::unix::net::UnixStream,
};

/// Signals to qinit that the service has finished its initialization routines.
pub fn mark_running() -> io::Result<()> {
	let mut sock = UnixStream::connect("/run/qinit/control.sock")?;
	sock.write_all(b"ACTION=running\n")?;

	Ok(())
}
