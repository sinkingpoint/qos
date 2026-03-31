use std::{
	io::Read,
	os::fd::{AsFd, AsRawFd, OwnedFd},
};

use nix::{
	sys::epoll::{Epoll, EpollEvent},
	unistd::write,
};
use thiserror::Error;

use crate::events::CompositorEvent;

pub trait EventSource {
	type Reader: AsFd + Read;
	type EventType: Into<CompositorEvent>;
	fn get_fds(&self) -> &[Self::Reader];
	fn read_event(&mut self, index: usize) -> std::io::Result<Self::EventType>;
}

pub struct EventThread<S: EventSource> {
	epoll: Epoll,
	event_source: S,
}

pub struct EventThreadHandle {
	pub(crate) killfd: OwnedFd,
}

impl EventThreadHandle {
	pub fn kill(&self) -> nix::Result<()> {
		write(self.killfd.as_fd().as_raw_fd(), &1_u64.to_le_bytes())?;
		Ok(())
	}
}

impl<S: EventSource> EventThread<S> {
	pub fn new(event_source: S) -> nix::Result<(Self, EventThreadHandle)> {
		let epoll = nix::sys::epoll::Epoll::new(nix::sys::epoll::EpollCreateFlags::empty())?;
		for (index, fd) in event_source.get_fds().iter().enumerate() {
			epoll.add(fd, EpollEvent::new(nix::sys::epoll::EpollFlags::EPOLLIN, index as u64))?;
		}

		let killfd = nix::sys::eventfd::eventfd(0, nix::sys::eventfd::EfdFlags::empty())?;
		epoll.add(&killfd, EpollEvent::new(nix::sys::epoll::EpollFlags::EPOLLIN, u64::MAX))?;

		let handle = EventThreadHandle { killfd };

		Ok((Self { epoll, event_source }, handle))
	}

	pub fn start_watching(
		&mut self,
		output_channel: std::sync::mpsc::Sender<CompositorEvent>,
	) -> Result<(), EventThreadError> {
		'outer: loop {
			let mut events = vec![EpollEvent::empty(); 16];
			let num_events = self.epoll.wait(&mut events, -1).map_err(EventThreadError::NixError)?;

			for event in events.into_iter().take(num_events) {
				if event.data() == u64::MAX {
					break 'outer;
				}

				let event = self.event_source.read_event(event.data() as usize);
				let event = event.map_err(EventThreadError::IOError)?;
				if let Err(e) = output_channel.send(event.into()) {
					eprintln!("Failed to send event: {}", e);
					break 'outer;
				}
			}
		}
		Ok(())
	}
}

#[derive(Debug, Error)]
pub enum EventThreadError {
	#[error("Nix error: {0}")]
	NixError(nix::Error),

	#[error("IO error: {0}")]
	IOError(std::io::Error),
}
