use bus::{BusActionType, PUBLISH_ACTION, SUBSCRIBE_ACTION};
use clap::{Arg, Command};
use tokio::{
	io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
	net::UnixStream,
};

#[tokio::main]
async fn main() {
	let app = Command::new("busctl")
		.version("0.1.0")
		.about("A message bus daemon")
		.arg(
			Arg::new("socket")
				.long("socket")
				.num_args(1)
				.default_value("/run/busd/control.sock")
				.help("The path to the control socket"),
		)
		.arg(
			Arg::new("topic")
				.long("topic")
				.num_args(1)
				.required(true)
				.help("The topic to talk to"),
		)
		.arg(
			Arg::new("action")
				.num_args(1)
				.required(true)
				.help("The action to perform"),
		)
		.get_matches();

	let socket_path: &String = app.get_one("socket").unwrap();
	let topic: &String = app.get_one("topic").unwrap();
	let action: &String = app.get_one("action").unwrap();
	let action = if action == SUBSCRIBE_ACTION {
		BusActionType::Subscribe
	} else if action == PUBLISH_ACTION {
		BusActionType::Publish
	} else {
		panic!("Unknown action: {}", action);
	};

	let header = format!("ACTION={} topic={}\n", action, topic);

	let mut stream = UnixStream::connect(socket_path).await.unwrap();
	stream.write_all(header.as_bytes()).await.unwrap();

	match action {
		BusActionType::Subscribe => {
			let mut reader = BufReader::new(stream);
			let mut line = String::new();
			while reader.read_line(&mut line).await.unwrap() > 0 {
				line.pop(); // Remove the newline
				println!("{}", line);
				line.clear();
			}
		}
		BusActionType::Publish => {
			let mut writer = BufWriter::new(stream);
			let mut reader = BufReader::new(io::stdin());

			let mut line = String::new();
			while reader.read_line(&mut line).await.unwrap() > 0 {
				writer.write_all(line.as_bytes()).await.unwrap();
				writer.flush().await.unwrap();
				line.clear();
			}
		}
	}
}
