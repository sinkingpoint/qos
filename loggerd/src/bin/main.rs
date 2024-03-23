use std::{borrow::BorrowMut, collections::HashMap, io::stderr, path::PathBuf, sync::Arc};

use anyhow::Result;
use clap::{Arg, Command};
use common::obs::assemble_logger;
use loggerd::{ConnectionHeader, LogMessage};
use slog::{error, info};
use tokio::{
	fs::remove_file,
	io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, BufReader},
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

	let api = Arc::new(API::new());
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
	api: Arc<API>,
}

impl Listener {
	fn new(listen_path: &str, api: Arc<API>) -> Self {
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
			let mut handler = Handler::new(stream, self.api.clone());
			tokio::spawn(async move {
				if let Err(e) = handler.run().await {
					eprintln!("Error: {}", e);
				}
			});
		}

		Ok(())
	}
}

struct Handler {
	stream: BufReader<UnixStream>,
	api: Arc<API>,
}

impl Handler {
	fn new(stream: UnixStream, api: Arc<API>) -> Self {
		Self {
			stream: BufReader::new(stream),
			api,
		}
	}

	async fn logstream(mut self, fields: HashMap<String, String>) -> Result<()> {
		let log_stream = self.api.log_stream().await;

		loop {
			println!("Reading");
			let mut buffer = vec![];
			let len = self.stream.read_until(b'\n', &mut buffer).await?;
			if len == 0 {
				println!("EOF");
				break;
			}

			println!("Read {} bytes", len);
			let message = LogMessage {
				timestamp: chrono::Utc::now(),
				fields: fields.clone(),
				message: String::from_utf8_lossy(&buffer[0..len - 1]).to_string(),
			};

			log_stream.send(message).await?;
		}
		Ok(())
	}

	async fn run(mut self) -> Result<()> {
		let header_length = self.stream.read_u16().await?;
		let mut header = vec![0; header_length as usize];
		self.stream.read_exact(&mut header).await?;

		let header: ConnectionHeader = serde_json::from_slice(&header)?;

		match header {
			ConnectionHeader::LogStream { fields } => self.logstream(fields).await?,
		}

		Ok(())
	}
}

struct API {
	log_stream_read: Mutex<mpsc::Receiver<LogMessage>>,
	log_stream_write: mpsc::Sender<LogMessage>,
}

impl API {
	fn new() -> Self {
		let (sender, receiver) = mpsc::channel(1024);
		Self {
			log_stream_read: Mutex::new(receiver),
			log_stream_write: sender,
		}
	}

	async fn run(&self) -> Result<()> {
		let mut log_stream = self.log_stream_read.lock().await;
		loop {
			let message = log_stream.recv().await.unwrap();
			println!("{:?}", message);
		}
	}

	async fn log_stream(&self) -> mpsc::Sender<LogMessage> {
		self.log_stream_write.clone()
	}
}
