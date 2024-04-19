use std::sync::Arc;

use anyhow::Result;
use control::listen::{Action, ActionFactory};
use loggerd::{control::START_WRITE_STREAM_ACTION, LogMessage};
use tokio::{
	io::{AsyncBufReadExt, BufReader},
	net::UnixStream,
};

use crate::api::Api;

/// Errors that can occur when running a control action.
pub enum ControlError {
	UnknownAction,
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

	fn build(&self, action: &str, _args: &[(&str, &str)]) -> Result<Self::Action, <Self::Action as Action>::Error> {
		match action {
			_ if action == START_WRITE_STREAM_ACTION => Ok(ControlAction::StartWriteStream(self.api.clone())),
			_ => Err(ControlError::UnknownAction),
		}
	}
}

/// A control action that can be run by the controller.
pub enum ControlAction {
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

/// A handler for streaming logs into a log file.
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
