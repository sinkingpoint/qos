use std::{
	io::{self, stderr},
	path::{Path, PathBuf},
	sync::Arc,
};

use anyhow::Result;
use clap::{Arg, Command};
use common::obs::assemble_logger;
use control::listen::{Action, ActionFactory, ControlSocket};
use loggerd::{control::START_WRITE_STREAM_ACTION, LogMessage, OpenLogFile};
use slog::{error, info};
use tokio::{
	fs,
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
		.arg(
			Arg::new("data-dir")
				.default_value("/var/log/loggerd")
				.long("data-dir")
				.short('d')
				.num_args(1)
				.help("The directory to store log files in"),
		)
		.get_matches();

	let logger = assemble_logger(stderr());
	let listen_path: &String = matches.get_one("listen-path").unwrap();
	let data_dir: &PathBuf = matches.get_one("data-dir").unwrap();
	info!(logger, "Listening on {}", listen_path);

	let api = Arc::new(Api::new(data_dir, logger.clone()));

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
			_ if action == START_WRITE_STREAM_ACTION => Ok(ControlAction::StartWriteStream(self.api.clone())),
			_ => Err(ControlError::UnknownAction),
		}
	}
}

enum ControlAction {
	StartWriteStream(Arc<Api>),
}

impl Action for ControlAction {
	type Error = ControlError;

	fn run(self, reader: BufReader<UnixStream>) -> Result<(), Self::Error> {
		match self {
			ControlAction::StartWriteStream(api) => {
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
		let log_stream = self.api.write_log_stream().await;

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
	logger: slog::Logger,
	/// The pipe that the API receives logs over to write them to disk.
	log_stream_read: Mutex<mpsc::Receiver<LogMessage>>,

	/// The pipe that producers can write logs to.
	log_stream_write: mpsc::Sender<LogMessage>,

	data_dir: PathBuf,

	log_files: Mutex<Vec<OpenLogFile>>,
}

impl Api {
	fn new(data_dir: &Path, logger: slog::Logger) -> Self {
		let (sender, receiver) = mpsc::channel(1024);
		Self {
			logger,
			log_stream_read: Mutex::new(receiver),
			log_stream_write: sender,
			data_dir: data_dir.to_path_buf(),
			log_files: Mutex::new(vec![]),
		}
	}

	/// Load all the log files in the data directory.
	async fn load_log_files(&self) -> io::Result<()> {
		let mut log_files = fs::read_dir(&self.data_dir).await?;
		while let Ok(Some(entry)) = log_files.next_entry().await {
			let file_type = entry.file_type().await?;
			if file_type.is_file() {
				match OpenLogFile::open(&entry.path()).await {
					Ok(file) => self.log_files.lock().await.push(file),
					Err(e) => {
						error!(self.logger, "Failed to open log file: {}", e);
					}
				}
			}
		}

		self.log_files.lock().await.sort_by_key(|f| f.header.time_min);

		Ok(())
	}

	async fn run(&self) -> Result<()> {
		self.load_log_files().await?;
		let mut log_files = self.log_files.lock().await;
		let last_log_file = match log_files.last_mut() {
			Some(file) => file,
			None => {
				info!(self.logger, "No log files found");
				return Ok(());
			}
		};

		let mut log_stream = self.log_stream_read.lock().await;
		loop {
			let message = log_stream.recv().await.unwrap();
			last_log_file.write_log(message).await?;
		}
	}

	async fn write_log_stream(&self) -> mpsc::Sender<LogMessage> {
		self.log_stream_write.clone()
	}
}
