use bus::{BusAPI, BusAction, BusActionType};
use clap::{Arg, Command};
use common::obs::assemble_logger;
use control::listen::{Action, ActionFactory, ControlSocket};
use std::{io::stderr, path::PathBuf, str::FromStr, sync::Arc};
use tokio::sync::Mutex;

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

#[derive(Clone)]
struct BusControlActionFactory {
	api: Arc<Mutex<BusAPI>>,
}

impl ActionFactory for BusControlActionFactory {
	type Action = BusAction;
	fn build(&self, action: &str, args: &[(&str, &str)]) -> Result<Self::Action, <Self::Action as Action>::Error> {
		let action = BusActionType::try_from(action)?;
		BusAction::try_new(self.api.clone(), action, args)
	}
}
