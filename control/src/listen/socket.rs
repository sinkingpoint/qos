use std::{fmt::Debug, fs, future::Future, path::Path};

use tokio::{
	io::{self, AsyncBufRead, AsyncBufReadExt, AsyncWrite, BufReader},
	net::{UnixListener, UnixStream},
};

/// The key that is used to indicate the action to be run in a control socket message.
const ACTION_KEY: &str = "ACTION";

/// A factory for creating actions to be run in response to control socket messages.
pub trait ActionFactory: Clone {
	/// The type of action that this factory produces.
	type Action: Action;

	/// Builds an action from the given action name and arguments.
	fn build(&self, action: &str, args: &[(&str, &str)]) -> Result<Self::Action, <Self::Action as Action>::Error>;
}

/// An action that can be run in response to a control socket message.
pub trait Action: Send {
	/// The type of error that this action can produce.
	type Error: Sync + Send + Debug;

	/// Runs the action with the given reader.
	fn run<R: AsyncBufRead + Unpin + Send + 'static, W: AsyncWrite + Unpin + Send + 'static>(
		self,
		reader: R,
		writer: W,
	) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

/// A control socket that listens for messages and runs actions in response.
pub struct ControlSocket<F: ActionFactory> {
	/// The socket that is being listened on.
	socket: UnixListener,

	/// The factory that is used to create actions to run.
	factory: F,
}

impl<F: ActionFactory + Send + 'static> ControlSocket<F> {
	/// Opens a new control socket at the given path with the given action factory.
	pub fn open(path: &Path, factory: F) -> io::Result<Self> {
		if path.exists() {
			// TODO: Check if the socket is actually a socket and not a file before removing it.
			fs::remove_file(path)?;
		}

		Ok(Self {
			socket: UnixListener::bind(path)?,
			factory,
		})
	}

	/// Listens for incoming connections and runs actions in response.
	/// This function will block the current thread, looping indefinitely.
	pub async fn listen(&self) {
		loop {
			let (stream, _) = self.socket.accept().await.unwrap();
			tokio::spawn(handler(self.factory.clone(), stream));
		}
	}
}

/// Handles a single incoming connection.
async fn handler<F: ActionFactory>(factory: F, stream: UnixStream) -> Result<(), <F::Action as Action>::Error> {
	let (read, write) = stream.into_split();
	let mut reader = BufReader::new(read);

	// Read the first line, which will be a whitespace seperated list of k=v pairs that
	// are arguments to the control socket, indicating what the connection wants to do.
	// e.g. "ACTION=start-stream FILE=/var/log/messages"
	let mut arg_string = String::new();
	reader.read_line(&mut arg_string).await.unwrap();
	let mut action = None;

	let mut args = Vec::new();
	for arg in arg_string.split_whitespace() {
		let (k, v) = arg.split_once('=').unwrap();
		args.push((k, v));

		if k == ACTION_KEY {
			action = Some(v);
		}
	}

	factory.build(action.unwrap_or(""), &args)?.run(reader, write).await
}
