use std::{io::ErrorKind, path::Path};

use tokio::{
	io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader},
	net::UnixStream,
};

/// The action to subscribe to a topic.
pub const SUBSCRIBE_ACTION: &str = "subscribe";

/// The action to publish to a topic.
pub const PUBLISH_ACTION: &str = "publish";

pub const DEFAULT_BUSD_SOCKET: &str = "/run/busd/control.sock";

const MAX_MESSAGE_LENGTH: usize = u16::MAX as usize;

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

	pub async fn subscribe(mut self, topic: &str) -> io::Result<SubscribeHook<impl AsyncRead>> {
		self.socket
			.write_all(BusClient::assemble_header(SUBSCRIBE_ACTION, topic).as_bytes())
			.await?;

		Ok(SubscribeHook(BufReader::new(self.socket)))
	}

	pub async fn publish(mut self, topic: &str) -> io::Result<PublishHook<impl AsyncWrite>> {
		self.socket
			.write_all(BusClient::assemble_header(PUBLISH_ACTION, topic).as_bytes())
			.await?;

		Ok(PublishHook(self.socket))
	}
}

pub struct PublishHook<T: AsyncWrite + Unpin>(T);

impl<T: AsyncWrite + Unpin> PublishHook<T> {
	pub async fn publish_message(&mut self, data: &[u8]) -> io::Result<()> {
		if data.len() > MAX_MESSAGE_LENGTH {
			return Err(io::Error::new(
				ErrorKind::InvalidData,
				"data length is greater than maximum length",
			));
		}

		self.0.write_u16(data.len() as u16).await?;
		self.0.write_all(data).await?;
		self.0.flush().await?;

		Ok(())
	}
}

pub struct SubscribeHook<T: AsyncRead + Unpin>(T);

impl<T: AsyncRead + Unpin> SubscribeHook<T> {
	pub async fn read_message(&mut self) -> io::Result<Vec<u8>> {
		let len = self.0.read_u16().await? as usize;
		let mut buf = vec![0; len];
		self.0.read_exact(&mut buf).await?;

		Ok(buf)
	}
}
