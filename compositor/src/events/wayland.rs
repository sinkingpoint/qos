use std::{
	collections::HashMap,
	fs::set_permissions,
	io,
	os::unix::{
		fs::{PermissionsExt, chown},
		net::{UnixListener, UnixStream},
	},
	sync::Arc,
	thread,
};

use nix::{sys::epoll, unistd::Uid};

use crate::events::{CompositorEvent, event_threads::EventThreadHandle, scm_bufreader::ScmBufReader};

const LISTENER_ID: u64 = 0;
const KILL_ID: u64 = u64::MAX;

#[derive(Debug)]
pub struct WaylandEvent {
	pub client_id: u32,
	pub packet: crate::wayland::WaylandPacket,
	pub client: Arc<UnixStream>,
}

pub struct WaylandSocket {
	pub socket: UnixListener,

	client_id_counter: u32,
}

impl WaylandSocket {
	pub fn new(display_name: String) -> io::Result<Self> {
		let socket_path = format!("/run/user/{}/{}", Uid::current(), display_name);
		println!("Creating Wayland socket at {}", socket_path);
		let parent_dir = std::path::Path::new(&socket_path).parent().unwrap();
		std::fs::create_dir_all(parent_dir)?; // TODO: Move this to a more appropriate place, and handle permissions properly.
		let socket = UnixListener::bind(&socket_path)?;
		chown(&socket_path, None, Some(101))?; // chown the socket to the video group so that non-root users in the video group can access it.
		set_permissions(&socket_path, std::fs::Permissions::from_mode(0o660))?;
		Ok(Self {
			socket,
			client_id_counter: 1,
		})
	}

	pub fn start(mut self, output_channel: std::sync::mpsc::Sender<CompositorEvent>) -> io::Result<EventThreadHandle> {
		let killfd = nix::sys::eventfd::eventfd(0, nix::sys::eventfd::EfdFlags::empty()).map_err(io::Error::from)?;
		let killfd_dup = killfd.try_clone()?;
		thread::spawn(move || {
			if let Err(e) = self.run(output_channel, killfd_dup) {
				eprintln!("Wayland socket error: {}", e);
			}
		});
		Ok(EventThreadHandle { killfd })
	}

	fn run(
		&mut self,
		output_channel: std::sync::mpsc::Sender<CompositorEvent>,
		killfd: std::os::unix::io::OwnedFd,
	) -> io::Result<()> {
		let epoll = epoll::Epoll::new(epoll::EpollCreateFlags::empty())?;
		epoll.add(
			&self.socket,
			epoll::EpollEvent::new(epoll::EpollFlags::EPOLLIN, LISTENER_ID),
		)?;
		epoll.add(&killfd, epoll::EpollEvent::new(epoll::EpollFlags::EPOLLIN, KILL_ID))?;

		let mut clients = HashMap::new();

		loop {
			let mut events = vec![epoll::EpollEvent::empty(); 16];
			let num_events = epoll.wait(&mut events, -1)?;
			for event in events.into_iter().take(num_events) {
				if event.data() == KILL_ID {
					return Ok(());
				} else if event.data() == LISTENER_ID {
					match self.socket.accept() {
						Ok((stream, _)) => {
							let client_id = self.client_id_counter;
							self.client_id_counter += 1;
							println!("New client connected: {}", client_id);
							epoll.add(
								&stream,
								epoll::EpollEvent::new(epoll::EpollFlags::EPOLLIN, client_id as u64),
							)?;
							let write = match stream.try_clone() {
								Ok(s) => s,
								Err(e) => {
									eprintln!("Failed to clone client stream for client {}: {}", client_id, e);
									continue;
								}
							};

							if let Err(e) = stream.set_nonblocking(true) {
								eprintln!(
									"Failed to set client stream to non-blocking for client {}: {}",
									client_id, e
								);
								continue;
							}

							clients.insert(client_id, (ScmBufReader::new(stream), Arc::new(write)));
						}
						Err(e) => eprintln!("Failed to accept client: {}", e),
					}
				} else {
					let client_id = event.data() as u32;
					let (read_client, write_client) = clients.get_mut(&client_id).unwrap();

					loop {
						let packet = match read_client.read_packet() {
							Ok(packet) => packet,
							Err(e) if e.kind() == io::ErrorKind::WouldBlock => break, // no more data right now
							Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
								if let Some((reader, _)) = clients.remove(&client_id)
									&& let Err(e) = epoll.delete(reader.socket())
								{
									eprintln!("Failed to remove client {} from epoll: {}", client_id, e);
								}
								break;
							}
							Err(e) => {
								eprintln!("Failed to read Wayland packet from client {}: {}", client_id, e);
								break;
							}
						};
						output_channel
							.send(CompositorEvent::Wayland(WaylandEvent {
								client_id,
								packet,
								client: Arc::clone(write_client),
							}))
							.unwrap_or_else(|e| {
								eprintln!("Failed to send Wayland event from client {}: {}", client_id, e)
							});
					}
				}
			}
		}
	}
}
