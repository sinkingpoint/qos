use bus::{BusClient, DEFAULT_BUSD_SOCKET};
use clap::{Arg, Command};
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::main]
async fn main() {
	let app = Command::new("busctl")
		.version("0.1.0")
		.about("A message bus daemon")
		.arg(
			Arg::new("socket")
				.long("socket")
				.num_args(1)
				.default_value(DEFAULT_BUSD_SOCKET)
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

	let client = BusClient::new_from_path(socket_path).await.unwrap();

	match action.as_str() {
		"subscribe" => {
			let mut reader = client.subscribe(topic).await.unwrap();
			let mut line = String::new();
			while reader.read_line(&mut line).await.unwrap() > 0 {
				line.pop(); // Remove the newline
				println!("{}", line);
				line.clear();
			}
		}
		"publish" => {
			let mut writer = client.publish(topic).await.unwrap();
			let mut reader = BufReader::new(io::stdin());

			let mut line = String::new();
			while reader.read_line(&mut line).await.unwrap() > 0 {
				writer.write_all(line.as_bytes()).await.unwrap();
				writer.flush().await.unwrap();
				line.clear();
			}
		}
		_ => {
			eprintln!("Unknown action: {}", action);
		}
	}
}
