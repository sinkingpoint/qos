use std::path::Path;

use tokio::{
	io::{self, AsyncWrite, AsyncWriteExt, BufReader},
	net::UnixStream,
};

/// The action to subscribe to a topic.
pub const SUBSCRIBE_ACTION: &str = "subscribe";

/// The action to publish to a topic.
pub const PUBLISH_ACTION: &str = "publish";

pub const DEFAULT_BUSD_SOCKET: &str = "/run/busd/control.sock";

pub struct BusClient {
	socket: UnixStream,
}

impl BusClient {
	pub async fn new() -> io::Result<BusClient> {
		let stream = UnixStream::connect(DEFAULT_BUSD_SOCKET).await?;

		Ok(BusClient { socket: stream })
	}

	pub async fn new_from_path<P: AsRef<Path>>(socket_path: P) -> io::Result<BusClient> {
		let stream = UnixStream::connect(socket_path).await?;

		Ok(BusClient { socket: stream })
	}

	fn assemble_header(action: &str, topic: &str) -> String {
		format!("ACTION={} topic={}\n", action, topic)
	}

	pub async fn subscribe(mut self, topic: &str) -> io::Result<BufReader<UnixStream>> {
		self.socket
			.write_all(BusClient::assemble_header(SUBSCRIBE_ACTION, topic).as_bytes())
			.await?;

		Ok(BufReader::new(self.socket))
	}

	pub async fn publish(mut self, topic: &str) -> io::Result<impl AsyncWrite> {
		self.socket
			.write_all(BusClient::assemble_header(PUBLISH_ACTION, topic).as_bytes())
			.await?;

		Ok(self.socket)
	}
}
