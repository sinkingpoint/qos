use bus::{PUBLISH_ACTION, SUBSCRIBE_ACTION};
use clap::{Arg, Command};
use common::obs::assemble_logger;
use control::listen::{Action, ActionFactory, ControlSocket};
use slog::{info, o};
use std::{collections::HashMap, io::stderr, path::PathBuf, str::FromStr, sync::Arc};
use thiserror::Error;
use tokio::{
	io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
	sync::{mpsc, Mutex},
};

#[tokio::main]
async fn main() {
	let app = Command::new("busd")
		.version("0.1.0")
		.about("A message bus daemon")
		.arg(
			Arg::new("socket")
				.long("socket")
				.num_args(1)
				.default_value("/run/busd/control.sock")
				.help("The path to the control socket"),
		)
		.get_matches();
	let logger = assemble_logger(stderr());
	let api = Arc::new(Mutex::new(BusAPI::new(logger.clone())));
	let factory: BusControlActionFactory = BusControlActionFactory { api };
	let socket_path: &String = app.get_one("socket").unwrap();

	let socket = ControlSocket::open(&PathBuf::from_str(socket_path).unwrap(), factory).unwrap();

	socket.listen().await;
}

struct Topic {
	logger: slog::Logger,
	name: String,
	subscribers: Vec<Subscription>,
}

impl Topic {
	async fn publish(&mut self, message: Vec<u8>) {
		let mut i = 0;
		self.subscribers.retain(|r| {
			if r.connection.try_send(message.clone()).is_ok() {
				i += 1;
				true
			} else {
				info!(self.logger, "Removing reader"; "topic" => self.name.as_str());
				false
			}
		});

		info!(self.logger, "Published message"; "topic" => self.name.as_str(), "subscribers" => i.to_string());
	}

	fn subscribe(&mut self) -> mpsc::Receiver<Vec<u8>> {
		let (tx, rx) = mpsc::channel(100);
		self.subscribers.push(Subscription { connection: tx });
		rx
	}
}

struct Subscription {
	connection: mpsc::Sender<Vec<u8>>,
}

struct BusAPI {
	logger: slog::Logger,
	topics: HashMap<String, Topic>,
}

impl BusAPI {
	fn new(logger: slog::Logger) -> Self {
		Self {
			logger,
			topics: HashMap::new(),
		}
	}

	fn create_topic(&mut self, name: String) {
		if self.topics.contains_key(&name) {
			return;
		}

		self.topics.insert(
			name.clone(),
			Topic {
				logger: self.logger.new(o!("topic" => name.clone())),
				name,
				subscribers: Vec::new(),
			},
		);
	}

	fn subscribe(&mut self, topic_name: &str) -> Option<mpsc::Receiver<Vec<u8>>> {
		let topic = self.topics.get_mut(topic_name)?;
		Some(topic.subscribe())
	}
}

#[derive(Clone)]
struct BusControlActionFactory {
	api: Arc<Mutex<BusAPI>>,
}

impl ActionFactory for BusControlActionFactory {
	type Action = BusAction;
	fn build(&self, action: &str, args: &[(&str, &str)]) -> Result<Self::Action, <Self::Action as Action>::Error> {
		if action == SUBSCRIBE_ACTION {
			BusAction::try_new_subscribe_action(self.api.clone(), args)
		} else if action == PUBLISH_ACTION {
			BusAction::try_new_publish_action(self.api.clone(), args)
		} else {
			Err(BusError::UnknownAction(action.to_string()))
		}
	}
}

enum BusAction {
	Subscribe { api: Arc<Mutex<BusAPI>>, topic: String },
	Publish { api: Arc<Mutex<BusAPI>>, topic: String },
}

impl BusAction {
	fn try_new_subscribe_action(api: Arc<Mutex<BusAPI>>, args: &[(&str, &str)]) -> Result<Self, BusError> {
		let topic = args
			.iter()
			.find(|(k, _)| k == &"topic")
			.ok_or(BusError::MissingArgument("topic"))?
			.1;
		Ok(Self::Subscribe {
			api,
			topic: topic.to_string(),
		})
	}

	fn try_new_publish_action(api: Arc<Mutex<BusAPI>>, args: &[(&str, &str)]) -> Result<Self, BusError> {
		let topic = args
			.iter()
			.find(|(k, _)| k == &"topic")
			.ok_or(BusError::MissingArgument("topic"))?
			.1;
		Ok(Self::Publish {
			api,
			topic: topic.to_string(),
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
		reader: R,
		writer: W,
	) -> Result<(), Self::Error> {
		match self {
			BusAction::Subscribe { api, topic } => {
				api.lock().await.create_topic(topic.clone());
				let rx = api.lock().await.subscribe(&topic).ok_or(BusError::TopicNotFound)?;
				let mut writer = BufWriter::new(writer);
				let mut rx = rx;
				while let Some(message) = rx.recv().await {
					writer.write_all(&message).await.unwrap();
					writer.flush().await.unwrap();
				}

				Ok(())
			}
			BusAction::Publish { api, topic } => {
				api.lock().await.create_topic(topic.clone());
				let mut reader = BufReader::new(reader);
				let mut buffer = Vec::new();
				reader.read_to_end(&mut buffer).await?;

				let mut api = api.lock().await;
				let topic = api.topics.get_mut(&topic).ok_or(BusError::TopicNotFound)?;

				topic.publish(buffer).await;

				Ok(())
			}
		}
	}
}

#[derive(Debug, Error)]
enum BusError {
	#[error("Missing argument: {0}")]
	MissingArgument(&'static str),

	#[error("Topic not found")]
	TopicNotFound,

	#[error("Unknown action: {0}")]
	UnknownAction(String),

	#[error("IO error: {0}")]
	IOError(#[from] std::io::Error),
}
