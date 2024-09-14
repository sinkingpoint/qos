use std::{
	pin::Pin,
	task::{ready, Context, Poll},
};

use tokio::io::{self, unix::AsyncFd, AsyncRead, AsyncWrite, ReadBuf};

use crate::{NetlinkSockType, NetlinkSocket};

/// An async wrapper around a Netlink socket.
pub struct AsyncNetlinkSocket<T: NetlinkSockType>(AsyncFd<NetlinkSocket<T>>);

impl<T: NetlinkSockType> AsyncNetlinkSocket<T> {
	pub fn new(groups: T::SockGroups) -> std::io::Result<Self> {
		let socket = NetlinkSocket::new(groups)?;
		let async_fd = AsyncFd::new(socket)?;

		Ok(Self(async_fd))
	}
}

impl<T: NetlinkSockType> AsyncRead for AsyncNetlinkSocket<T> {
	fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
		loop {
			let mut guard = ready!(self.0.poll_read_ready(cx))?;

			let unfilled = buf.initialize_unfilled();
			match guard.try_io(|inner| inner.get_ref().uread(unfilled)) {
				Ok(Ok(len)) => {
					buf.advance(len);
					return Poll::Ready(Ok(()));
				}
				Ok(Err(err)) => return Poll::Ready(Err(err)),
				Err(_would_block) => continue,
			}
		}
	}
}

impl<T: NetlinkSockType> AsyncWrite for AsyncNetlinkSocket<T> {
	fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
		loop {
			let mut guard = ready!(self.0.poll_write_ready(cx))?;

			match guard.try_io(|inner| inner.get_ref().uwrite(buf)) {
				Ok(result) => return Poll::Ready(result),
				Err(_would_block) => continue,
			}
		}
	}

	fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
		Poll::Ready(Ok(()))
	}

	fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
		Poll::Ready(Ok(()))
	}
}
