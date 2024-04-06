use std::{io::stderr, path::PathBuf, sync::Arc};

use anyhow::Result;
use clap::{Arg, Command};
use common::obs::assemble_logger;
use loggerd::{LogMessage, OpenLogFile};
use slog::{error, info};
use tokio::{
	fs::remove_file,
	io::{AsyncBufReadExt, BufReader},
	net::{UnixListener, UnixStream},
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
	let listener = Listener::new(listen_path, api.clone());

	tokio::select! {
		_ = tokio::signal::ctrl_c() => {
			info!(logger, "Shutting down");
		}
		err = listener.run() => {
			if let Err(e) = err {
				error!(logger, "Failed to run listener: {}", e);
			}
		}
		err = api.run() => {
			if let Err(e) = err {
				error!(logger, "Failed to run api: {}", e);
			}
		}
	}
}

struct Listener {
	listen_path: PathBuf,
	api: Arc<Api>,
}

impl Listener {
	fn new(listen_path: &str, api: Arc<Api>) -> Self {
		Self {
			listen_path: PathBuf::from(listen_path),
			api,
		}
	}

	async fn run(&self) -> Result<()> {
		if self.listen_path.exists() {
			remove_file(&self.listen_path).await?;
		}

		let socket = UnixListener::bind(&self.listen_path)?;

		loop {
			let (stream, _) = socket.accept().await?;
			let handler = Handler::new(stream, self.api.clone());
			tokio::spawn(async move {
				if let Err(e) = handler.run().await {
					eprintln!("Error: {}", e);
				}
			});
		}
	}
}

struct Handler {
	stream: BufReader<UnixStream>,
	api: Arc<Api>,
}

impl Handler {
	fn new(stream: UnixStream, api: Arc<Api>) -> Self {
		Self {
			stream: BufReader::new(stream),
			api,
		}
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
