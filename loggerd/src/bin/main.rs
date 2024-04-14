use std::{io::stderr, sync::Arc};

use anyhow::Result;
use clap::{Arg, Command};
use common::obs::assemble_logger;
use control::listen::{Action, ActionFactory, ControlSocket};
use loggerd::{control::START_STREAM_ACTION, LogMessage, OpenLogFile};
use slog::{error, info};
use tokio::{
	io::{AsyncBufReadExt, BufReader},
	net::UnixStream,
	sync::{mpsc, Mutex},
};

#[tokio::main]
async fn main() {
	let matches = Command::new("loggerd")
		.version("0.1.0")
		.author("Colin Douch")
		.about("A simple logging daemon")
		.arg(
			Arg::new("listen-path")
				.default_value("/run/loggerd/loggerd.sock")
				.long("listen-path")
				.short('l')
				.num_args(1)
				.help("The path to the unix socket to listen on"),
		)
		.get_matches();

	let logger = assemble_logger(stderr());
	let listen_path: &String = matches.get_one("listen-path").unwrap();
	info!(logger, "Listening on {}", listen_path);

	let api = Arc::new(Api::new());

	let control = ControlSocket::open(listen_path, Controller::new(api.clone())).unwrap();

	tokio::select! {
		_ = tokio::signal::ctrl_c() => {
			info!(logger, "Shutting down");
		}
		_ = control.listen() => {
			error!(logger, "Control socket failed");
		},
		err = api.run() => {
			if let Err(e) = err {
				error!(logger, "Failed to run api: {}", e);
			}
		}
	}
}

enum ControlError {
	UnknownAction,
}

#[derive(Clone)]
struct Controller {
	api: Arc<Api>,
}

impl Controller {
	fn new(api: Arc<Api>) -> Self {
		Self { api }
	}
}

impl ActionFactory for Controller {
	type Action = ControlAction;

	fn build(&self, action: &str, _args: &[(&str, &str)]) -> Result<Self::Action, <Self::Action as Action>::Error> {
		match action {
			_ if action == START_STREAM_ACTION => Ok(ControlAction::StartStream(self.api.clone())),
			_ => Err(ControlError::UnknownAction),
		}
	}
}

enum ControlAction {
	StartStream(Arc<Api>),
}

impl Action for ControlAction {
	type Error = ControlError;

	fn run(self, reader: BufReader<UnixStream>) -> Result<(), Self::Error> {
		match self {
			ControlAction::StartStream(api) => {
				let handler = WriteStreamHandler::new(reader, api);
				tokio::spawn(handler.run());
				Ok(())
			}
		}
	}
}

struct WriteStreamHandler {
	stream: BufReader<UnixStream>,
	api: Arc<Api>,
}

impl WriteStreamHandler {
	fn new(stream: BufReader<UnixStream>, api: Arc<Api>) -> Self {
		Self { stream, api }
	}

	async fn logstream(mut self) -> Result<()> {
		let log_stream = self.api.log_stream().await;

		loop {
			let mut buffer = vec![];
			let len = self.stream.read_until(b'\n', &mut buffer).await?;
			if len == 0 {
				break;
			}

			let message = LogMessage {
				timestamp: chrono::Utc::now(),
				fields: vec![],
				message: String::from_utf8_lossy(&buffer[0..len - 1]).to_string(),
			};

			log_stream.send(message).await?;
		}
		Ok(())
	}

	async fn run(self) -> Result<()> {
		self.logstream().await
	}
}

struct Api {
	log_stream_read: Mutex<mpsc::Receiver<LogMessage>>,
	log_stream_write: mpsc::Sender<LogMessage>,
}

impl Api {
	fn new() -> Self {
		let (sender, receiver) = mpsc::channel(1024);
		Self {
			log_stream_read: Mutex::new(receiver),
			log_stream_write: sender,
		}
	}

	async fn run(&self) -> Result<()> {
		let mut file = OpenLogFile::new("test.log").await?;
		let mut log_stream = self.log_stream_read.lock().await;
		loop {
			let message = log_stream.recv().await.unwrap();
			file.write_log(message).await?;
		}
	}

	async fn log_stream(&self) -> mpsc::Sender<LogMessage> {
		self.log_stream_write.clone()
	}
}