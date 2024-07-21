use bus::{BusClient, DEFAULT_BUSD_SOCKET};
use clap::{Arg, Command};
use tokio::io::{self, AsyncBufReadExt, BufReader};

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
			while let Ok(msg) = reader.read_message().await {
				let msg = match String::from_utf8(msg) {
					Ok(msg) => msg,
					Err(_) => {
						println!("<Invalid UTF8 Msg");
						continue;
					}
				};

				println!("{}", msg.trim());
			}
		}
		"publish" => {
			let mut writer = client.publish(topic).await.unwrap();
			let mut reader = BufReader::new(io::stdin());

			let mut line = String::new();
			while reader.read_line(&mut line).await.unwrap() > 0 {
				if let Err(e) = writer.publish_message(line.as_bytes()).await {
					println!("Failed to publish message: {}", e);
				}
				line.clear();
			}
		}
		_ => {
			eprintln!("Unknown action: {}", action);
		}
	}
}
