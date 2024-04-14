use tokio::{
	io::{self, AsyncBufReadExt, BufReader},
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
pub trait Action {
	/// The type of error that this action can produce.
	type Error: Sync + Send;

	/// Runs the action with the given reader.
	fn run(self, reader: BufReader<UnixStream>) -> Result<(), Self::Error>;
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
	pub fn open(path: &str, factory: F) -> io::Result<Self> {
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
	let mut reader = BufReader::new(stream);

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

	factory.build(action.unwrap_or(""), &args)?.run(reader)
}
