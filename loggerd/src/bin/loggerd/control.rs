use std::sync::Arc;

use anyhow::Result;
use control::listen::{Action, ActionFactory};
use loggerd::{
	control::{ReadStreamOpts, ReadStreamOptsParseError, START_READ_STREAM_ACTION, START_WRITE_STREAM_ACTION},
	LogMessage,
};
use thiserror::Error;
use tokio::{
	io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt},
	net::unix::UCred,
};

use crate::api::Api;

/// Errors that can occur when running a control action.
#[derive(Debug, Clone, Error)]
pub enum ControlError {
	#[error("unknown action")]
	UnknownAction,

	#[error("failed to read log stream: {0}")]
	InvalidReadOpts(#[from] ReadStreamOptsParseError),
}

/// A controller for handling control actions.
#[derive(Clone)]
pub struct Controller {
	api: Arc<Api>,
}

impl Controller {
	pub fn new(api: Arc<Api>) -> Self {
		Self { api }
	}
}

impl ActionFactory for Controller {
	type Action = ControlAction;

	fn build(&self, action: &str, args: &[(&str, &str)]) -> Result<Self::Action, <Self::Action as Action>::Error> {
		match action {
			_ if action == START_WRITE_STREAM_ACTION => Ok(ControlAction::StartWriteStream(self.api.clone())),
			_ if action == START_READ_STREAM_ACTION => {
				let opts = ReadStreamOpts::from_kvs(args)?;
				Ok(ControlAction::StartReadStream(self.api.clone(), opts))
			}
			_ => Err(ControlError::UnknownAction),
		}
	}
}

/// A control action that can be run by the controller.
pub enum ControlAction {
	StartWriteStream(Arc<Api>),
	StartReadStream(Arc<Api>, ReadStreamOpts),
}

impl Action for ControlAction {
	type Error = ControlError;

	async fn run<R: AsyncBufRead + Unpin + Send + 'static, W: AsyncWrite + Unpin + Send + 'static>(
		self,
		_peer: UCred,
		reader: R,
		writer: W,
	) -> Result<(), Self::Error> {
		match self {
			ControlAction::StartWriteStream(api) => {
				let handler = WriteStreamHandler::new(reader, api);
				tokio::spawn(handler.run());
			}
			ControlAction::StartReadStream(api, opts) => {
				let handler = ReadStreamHandler::new(writer, api, opts);
				tokio::spawn(handler.run());
			}
		};
		Ok(())
	}
}

/// A handler for streaming logs into a log file.
struct WriteStreamHandler<R: AsyncBufRead> {
	stream: R,
	api: Arc<Api>,
}

impl<R: AsyncBufRead + Unpin + Send> WriteStreamHandler<R> {
	fn new(stream: R, api: Arc<Api>) -> Self {
		Self { stream, api }
	}

	async fn run(mut self) -> Result<()> {
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
}

struct ReadStreamHandler<W: AsyncWrite> {
	stream: W,
	api: Arc<Api>,
	opts: ReadStreamOpts,
}

impl<W: AsyncWrite + Unpin + Send + 'static> ReadStreamHandler<W> {
	fn new(stream: W, api: Arc<Api>, opts: ReadStreamOpts) -> Self {
		Self { stream, api, opts }
	}

	async fn run(mut self) -> Result<()> {
		let iter = match self.api.read_logs(self.opts.clone()).await {
			Ok(iter) => iter,
			Err(e) => {
				eprintln!("Failed to read logs: {}", e);
				return Err(e);
			}
		};

		for log in iter {
			let log = match log {
				Ok(log) => log,
				Err(e) => {
					eprintln!("Failed to read log: {}", e);
					break;
				}
			};
			self.stream.write_all(self.opts.format_log(&log).as_bytes()).await?;
		}

		Ok(())
	}
}
