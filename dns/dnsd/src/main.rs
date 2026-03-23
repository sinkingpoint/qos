use std::io::Cursor;

use bytestruct::ReadFromWithEndian;
use clap::Command;
use dns::message::DNSMessage;
use tokio::net::UdpSocket;

#[tokio::main]
async fn main() {
	let app = Command::new("dnsd")
		.version(env!("CARGO_PKG_VERSION"))
		.about("A DNS server written in Rust")
		.arg(
			clap::Arg::new("listen")
				.short('l')
				.long("listen")
				.value_name("ADDRESS:PORT")
				.help("The address and port to listen on (default: 0.0.0.0:53)"),
		)
		.get_matches();

	let address = app
		.get_one::<String>("listen")
		.map(|s| s.as_str())
		.unwrap_or("0.0.0.0:53");

	let server = DNSServer::new(address).await.expect("Failed to start DNS server");

	if let Err(e) = server.run().await {
		eprintln!("Error running DNS server: {}", e);
	}
}

struct DNSServer {
	listener: UdpSocket,
}

impl DNSServer {
	async fn new(listen_addr: &str) -> std::io::Result<Self> {
		let listener = UdpSocket::bind(listen_addr).await?;
		Ok(Self { listener })
	}

	async fn run(&self) -> std::io::Result<()> {
		loop {
			let mut header_bytes = [0u8; 512];
			let (size, src) = self.listener.recv_from(&mut header_bytes).await?;
			for byte in &header_bytes[..size] {
				print!("{:#010b} ", byte);
			}
			println!();
			let header =
				DNSMessage::read_from_with_endian(&mut Cursor::new(&header_bytes[..size]), bytestruct::Endian::Big)?;
			println!("Parsed header: {:?}", header);
		}
	}
}
