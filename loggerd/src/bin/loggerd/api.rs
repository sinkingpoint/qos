use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use futures::future::join_all;
use loggerd::{control, LogMessage, OpenLogFile};
use slog::error;
use tokio::{
	fs, io,
	sync::{mpsc, Mutex},
};

pub struct Api {
	logger: slog::Logger,
	/// The pipe that the API receives logs over to write them to disk.
	log_stream_read: Mutex<mpsc::Receiver<LogMessage>>,

	/// The pipe that producers can write logs to.
	log_stream_write: mpsc::Sender<LogMessage>,

	data_dir: PathBuf,
}

impl Api {
	pub fn new(data_dir: &Path, logger: slog::Logger) -> Self {
		let (sender, receiver) = mpsc::channel(1024);
		Self {
			logger,
			log_stream_read: Mutex::new(receiver),
			log_stream_write: sender,
			data_dir: data_dir.to_path_buf(),
		}
	}

	/// Load all the log files in the data directory.
	async fn load_log_files(&self) -> io::Result<Vec<OpenLogFile>> {
		let mut open_log_files = Vec::new();
		let mut log_file_files = fs::read_dir(&self.data_dir).await?;
		while let Ok(Some(entry)) = log_file_files.next_entry().await {
			let file_type = entry.file_type().await?;
			if file_type.is_file() {
				match OpenLogFile::open(&entry.path()).await {
					Ok(file) => open_log_files.push(file),
					Err(e) => {
						error!(self.logger, "Failed to open log file: {}", e);
					}
				}
			}
		}

		open_log_files.sort_by_key(|f| f.header.time_min);

		Ok(open_log_files)
	}

	pub async fn run(&self) -> Result<()> {
		if !self.data_dir.exists() {
			fs::create_dir_all(&self.data_dir)
				.await
				.with_context(|| format!("failed to create data dir: {}", self.data_dir.display()))?;
		}

		let mut log_files = self
			.load_log_files()
			.await
			.with_context(|| "failed to load existing log files")?;
		let last_log_file = match log_files.last_mut() {
			Some(file) => file,
			None => {
				let log_file_path = self.data_dir.join(new_random_log_file_name());
				let new_log_file = OpenLogFile::new(&log_file_path)
					.await
					.with_context(|| "failed to open new log file")?;
				log_files.push(new_log_file);
				log_files.last_mut().unwrap()
			}
		};

		let mut log_stream = self.log_stream_read.lock().await;
		loop {
			let message = log_stream.recv().await.unwrap();
			last_log_file.write_log(message).await?;
		}
	}

	pub async fn write_log_stream(&self) -> mpsc::Sender<LogMessage> {
		self.log_stream_write.clone()
	}

	/// Read logs from the log files, returning an iterator over the logs that .
	pub async fn read_logs(
		&self,
		opts: control::ReadStreamOpts,
	) -> Result<impl Iterator<Item = io::Result<LogMessage>>> {
		let log_files = self.load_log_files().await?;
		let future = join_all(log_files.into_iter().map(|f| f.read_log_stream(opts.clone()))).await;
		Ok(future.into_iter().flatten())
	}
}

fn new_random_log_file_name() -> PathBuf {
	PathBuf::from(format!("log-{}.log", rand::random::<u64>()))
}
