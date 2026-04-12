use std::{
	collections::VecDeque,
	io::{self, IoSlice, IoSliceMut},
	os::{
		fd::{FromRawFd, OwnedFd, RawFd},
		unix::{io::AsRawFd, net::UnixStream},
	},
	sync::Arc,
};

use nix::{
	cmsg_space,
	sys::socket::{ControlMessage, ControlMessageOwned, MsgFlags, recvmsg, sendmsg},
};

use crate::types::WaylandPacket;

/// A Wayland socket connection that handles SCM_RIGHTS fd passing on both
/// reads and writes, shared between compositor and client code.
pub struct WaylandConnection {
	pub stream: Arc<UnixStream>,
	pub fds: VecDeque<OwnedFd>,
}

impl WaylandConnection {
	pub fn new(stream: UnixStream) -> Self {
		Self {
			stream: Arc::new(stream),
			fds: VecDeque::new(),
		}
	}

	/// Receive one Wayland packet from the socket. Any file descriptors
	/// received alongside the packet are pushed onto `self.fds`.
	pub fn recv_packet(&mut self) -> io::Result<WaylandPacket> {
		let mut header = [0u8; 8];
		self.read_exact(&mut header)?;

		let object_id = u32::from_le_bytes(header[0..4].try_into().unwrap());
		let opcode = u16::from_le_bytes(header[4..6].try_into().unwrap());
		let msg_size = u16::from_le_bytes(header[6..8].try_into().unwrap()) as usize;
		let payload_len = msg_size.saturating_sub(8);
		let mut payload = vec![0u8; payload_len];
		if !payload.is_empty() {
			self.read_exact(&mut payload)?;
		}
		Ok(WaylandPacket::new(object_id, opcode, payload))
	}

	/// Drain all accumulated file descriptors as a Vec, useful when
	/// dispatching packets to a handler that expects per-packet FDs.
	pub fn drain_fds(&mut self) -> Vec<OwnedFd> {
		self.fds.drain(..).collect()
	}

	/// Send a Wayland packet with an attached file descriptor via SCM_RIGHTS.
	pub fn send_with_fd(&self, object_id: u32, opcode: u16, payload: &[u8], fd: RawFd) -> io::Result<()> {
		let msg_size = (8 + payload.len()) as u16;
		let mut buf = Vec::with_capacity(msg_size as usize);
		buf.extend_from_slice(&object_id.to_le_bytes());
		buf.extend_from_slice(&opcode.to_le_bytes());
		buf.extend_from_slice(&msg_size.to_le_bytes());
		buf.extend_from_slice(payload);
		let iov = [IoSlice::new(&buf)];
		let fds = [fd];
		let cmsg = [ControlMessage::ScmRights(&fds)];
		sendmsg::<()>(self.stream.as_raw_fd(), &iov, &cmsg, MsgFlags::empty(), None).map_err(io::Error::other)?;
		Ok(())
	}

	/// Returns true if the socket has at least one byte ready to read without blocking.
	pub fn has_data(&self) -> bool {
		let mut buf = [0u8; 1];
		let mut iov = [IoSliceMut::new(&mut buf)];
		matches!(
			recvmsg::<()>(
				self.stream.as_raw_fd(),
				&mut iov,
				None,
				MsgFlags::MSG_PEEK | MsgFlags::MSG_DONTWAIT,
			),
			Ok(msg) if msg.bytes > 0
		)
	}

	fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
		let mut filled = 0;
		while filled < buf.len() {
			let mut iov = [IoSliceMut::new(&mut buf[filled..])];
			let mut cmsg_buf = cmsg_space!(RawFd);
			let msg = recvmsg::<()>(
				self.stream.as_raw_fd(),
				&mut iov,
				Some(&mut cmsg_buf),
				MsgFlags::empty(),
			)
			.map_err(|e| io::Error::from_raw_os_error(e as i32))?;
			if msg.bytes == 0 {
				return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "connection closed"));
			}
			for cmsg in msg.cmsgs() {
				if let ControlMessageOwned::ScmRights(rfds) = cmsg {
					for fd in rfds {
						self.fds.push_back(unsafe { OwnedFd::from_raw_fd(fd) });
					}
				}
			}
			filled += msg.bytes;
		}
		Ok(())
	}
}
