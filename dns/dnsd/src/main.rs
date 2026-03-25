use std::net::SocketAddr;

use bytestruct::WriteToWithEndian;
use clap::Command;
use dns::{
	message::{DNSMessage, DNSResponseCode},
	resolver::DNSStubResolver,
};
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
				.help("The address and port to listen on (default: 127.0.0.1:53)"),
		)
		.get_matches();

	let address = app
		.get_one::<String>("listen")
		.map(|s| s.as_str())
		.unwrap_or("127.0.0.1:53");

	let server = DNSServer::new(address).await.expect("Failed to start DNS server");

	if let Err(e) = server.run().await {
		eprintln!("Error running DNS server: {}", e);
	}
}

#[derive(Debug, Default)]
struct Config {
	upstream_resolvers: Vec<String>,
}

struct DNSServer {
	config: Config,
	listener: UdpSocket,
	resolver: DNSStubResolver,
}

impl DNSServer {
	async fn new(listen_addr: &str) -> std::io::Result<Self> {
		let listener = UdpSocket::bind(listen_addr).await?;
		let resolver = DNSStubResolver::new().await?;
		let config = Config {
			upstream_resolvers: vec!["1.1.1.1:53".to_string()],
		};

		Ok(Self {
			listener,
			config,
			resolver,
		})
	}

	async fn run(&self) -> std::io::Result<()> {
		loop {
			let mut header_bytes = [0u8; 512];
			let (size, src) = self.listener.recv_from(&mut header_bytes).await?;
			let header = DNSMessage::from_bytes(&header_bytes[..size], bytestruct::Endian::Big)?;
			self.handle_request(src, header).await;
		}
	}

	async fn handle_request(&self, src: SocketAddr, request: DNSMessage) {
		let mut answers = Vec::new();
		let mut response_code = DNSResponseCode::NoError;
		for question in &request.questions {
			if let Some(resp) = self
				.resolver
				.resolve(question.clone(), &self.config.upstream_resolvers)
				.await
			{
				answers.extend(resp.answers);
				if resp.header.flags.response_code != DNSResponseCode::NoError {
					response_code = resp.header.flags.response_code;
				}
			} else {
				response_code = DNSResponseCode::ServerFailure;
				break;
			}
		}

		let response = DNSMessage::new_response(&request, response_code, answers);
		let mut response_bytes = Vec::new();
		if let Err(e) = response.write_to_with_endian(&mut response_bytes, bytestruct::Endian::Big) {
			eprintln!("Failed to serialize DNS response: {}", e);
			return;
		}

		if let Err(e) = self.listener.send_to(&response_bytes, src).await {
			eprintln!("Failed to send DNS response to {}: {}", src, e);
		}
	}
}
