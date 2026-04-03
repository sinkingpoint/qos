use std::{
	io,
	os::{
		fd::{FromRawFd, OwnedFd, RawFd},
		unix::{io::AsRawFd, net::UnixStream},
	},
};

use nix::{cmsg_space, sys::socket::recvmsg};
use std::io::IoSliceMut;

use crate::wayland::WaylandPacket;

pub struct ScmBufReader {
	socket: UnixStream,
}

impl ScmBufReader {
	pub fn new(socket: UnixStream) -> Self {
		Self { socket }
	}

	pub fn socket(&self) -> &UnixStream {
		&self.socket
	}

	pub fn read_packet(&mut self) -> io::Result<(WaylandPacket, Vec<OwnedFd>)> {
		let mut fds = Vec::new();

		let mut header = [0u8; 8];
		self.read_exact_collecting_fds(&mut header, &mut fds)?;

		let data_length = u16::from_le_bytes([header[6], header[7]]) as usize;
		let mut payload = vec![0u8; data_length - 8];
		if !payload.is_empty() {
			self.read_exact_collecting_fds(&mut payload, &mut fds)?;
		}

		let object_id = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
		let opcode = u16::from_le_bytes([header[4], header[5]]);

		Ok((WaylandPacket::new(object_id, opcode, payload), fds))
	}

	fn read_exact_collecting_fds(&mut self, buf: &mut [u8], fds: &mut Vec<OwnedFd>) -> io::Result<()> {
		let mut filled = 0;
		while filled < buf.len() {
			let remaining = &mut buf[filled..];
			let mut iov = [IoSliceMut::new(remaining)];
			let mut cmsg_buf = cmsg_space!(RawFd);
			let msg = recvmsg::<()>(
				self.socket.as_raw_fd(),
				&mut iov,
				Some(&mut cmsg_buf),
				nix::sys::socket::MsgFlags::empty(),
			)
			.map_err(|e| io::Error::from_raw_os_error(e as i32))?;

			if msg.bytes == 0 {
				return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "connection closed"));
			}

			for cmsg in msg.cmsgs() {
				if let nix::sys::socket::ControlMessageOwned::ScmRights(received_fds) = cmsg {
					for fd in received_fds {
						fds.push(unsafe { OwnedFd::from_raw_fd(fd) });
					}
				}
			}

			filled += msg.bytes;
		}
		Ok(())
	}
}
