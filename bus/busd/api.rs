use std::{collections::HashMap, io::ErrorKind, sync::Arc};

use bus::{PUBLISH_ACTION, SUBSCRIBE_ACTION};
use control::listen::Action;
use slog::{info, o};
use std::fmt;
use tokio::{
	io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
	net::unix::UCred,
	sync::{mpsc, Mutex},
};

use thiserror::Error;

/// The type of action to perform.
pub enum BusActionType {
	Subscribe,
	Publish,
}

impl fmt::Display for BusActionType {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			BusActionType::Subscribe => write!(f, "{}", SUBSCRIBE_ACTION),
			BusActionType::Publish => write!(f, "{}", PUBLISH_ACTION),
		}
	}
}

impl TryFrom<&str> for BusActionType {
	type Error = BusError;
	fn try_from(value: &str) -> Result<Self, Self::Error> {
		match value {
			SUBSCRIBE_ACTION => Ok(Self::Subscribe),
			PUBLISH_ACTION => Ok(Self::Publish),
			_ => Err(BusError::UnknownAction(value.to_string())),
		}
	}
}

/// An action to perform on the bus.
pub struct BusAction {
	pub api: Arc<Mutex<BusAPI>>,
	pub topic: String,
	pub action: BusActionType,
}

impl BusAction {
	pub fn try_new(api: Arc<Mutex<BusAPI>>, action: BusActionType, args: &[(&str, &str)]) -> Result<Self, BusError> {
		let topic = args
			.iter()
			.find(|(k, _)| k == &"topic")
			.ok_or(BusError::MissingArgument("topic"))?
			.1;
		Ok(Self {
			api,
			topic: topic.to_string(),
			action,
		})
	}
}

impl Action for BusAction {
	type Error = BusError;
	async fn run<
		R: tokio::io::AsyncBufRead + Unpin + Send + 'static,
		W: tokio::io::AsyncWrite + Unpin + Send + 'static,
	>(
		self,
		_peer: UCred,
		reader: R,
		writer: W,
	) -> Result<(), Self::Error> {
		match self.action {
			BusActionType::Subscribe => {
				self.api.lock().await.create_topic(&self.topic);
				let rx = self
					.api
					.lock()
					.await
					.subscribe(&self.topic)
					.ok_or(BusError::TopicNotFound)?;

				let mut writer = BufWriter::new(writer);
				let mut rx = rx;
				while let Some(message) = rx.recv().await {
					let len = message.len() as u16;
					writer.write_u16(len).await?;
					writer.write_all(&message).await?;
					if writer.flush().await.is_err() {
						return Ok(());
					}
				}

				Ok(())
			}
			BusActionType::Publish => {
				self.api.lock().await.create_topic(&self.topic);
				let mut reader = BufReader::new(reader);
				loop {
					let len = match reader.read_u16().await {
						Ok(len) => len as usize,
						Err(e) if e.kind() == ErrorKind::UnexpectedEof => break,
						Err(e) => return Err(e.into()),
					};

					let mut buffer = vec![0; len];
					match reader.read_exact(&mut buffer).await {
						Ok(_) => {}
						Err(e) if e.kind() == ErrorKind::UnexpectedEof => break,
						Err(e) => return Err(e.into()),
					};

					let mut api = self.api.lock().await;
					let topic = api.topics.get_mut(&self.topic).ok_or(BusError::TopicNotFound)?;

					topic.publish(&buffer).await;
				}

				Ok(())
			}
		}
	}
}

/// A topic to publish and subscribe to.
struct Topic {
	logger: slog::Logger,

	/// The name of the topic.
	name: String,

	/// The subscribers to the topic.
	subscribers: Vec<Subscription>,
}

impl Topic {
	/// Publish a message to every subscriber.
	async fn publish(&mut self, message: &[u8]) {
		let mut num_sucessfully_sent = 0;
		self.subscribers.retain(|r| {
			if r.connection.try_send(message.to_owned()).is_ok() {
				num_sucessfully_sent += 1;
				true
			} else {
				info!(self.logger, "Removing reader"; "topic" => self.name.as_str());
				false
			}
		});

		info!(self.logger, "Published message"; "topic" => self.name.as_str(), "subscribers" => num_sucessfully_sent.to_string());
	}

	/// Subscribe to the topic.
	fn subscribe(&mut self) -> mpsc::Receiver<Vec<u8>> {
		let (tx, rx) = mpsc::channel(100);
		self.subscribers.push(Subscription { connection: tx });
		rx
	}
}

/// A subscription to a topic that we can send published messages to.
struct Subscription {
	connection: mpsc::Sender<Vec<u8>>,
}

/// The API for the message bus.
pub struct BusAPI {
	logger: slog::Logger,
	topics: HashMap<String, Topic>,
}

impl BusAPI {
	pub fn new(logger: slog::Logger) -> Self {
		Self {
			logger,
			topics: HashMap::new(),
		}
	}

	/// Create a new topic, if it doesn't already exist.
	fn create_topic(&mut self, name: &str) {
		if self.topics.contains_key(name) {
			return;
		}

		self.topics.insert(
			name.to_owned(),
			Topic {
				logger: self.logger.new(o!("topic" => name.to_owned())),
				name: name.to_owned(),
				subscribers: Vec::new(),
			},
		);
	}

	fn subscribe(&mut self, topic_name: &str) -> Option<mpsc::Receiver<Vec<u8>>> {
		let topic = self.topics.get_mut(topic_name)?;
		Some(topic.subscribe())
	}
}

#[derive(Debug, Error)]
pub enum BusError {
	#[error("Missing argument: {0}")]
	MissingArgument(&'static str),

	#[error("Topic not found")]
	TopicNotFound,

	#[error("Unknown action: {0}")]
	UnknownAction(String),

	#[error("IO error: {0}")]
	IOError(#[from] std::io::Error),
}
