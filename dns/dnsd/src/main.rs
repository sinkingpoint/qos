use std::{net::SocketAddr, sync::Arc};

use bytestruct::WriteToWithEndian;
use clap::Command;
use common::qinit::mark_running;
use dns::{
	message::{DNSMessage, DNSQuestion, DNSResponseCode, QType},
	resolver::DNSStubResolver,
};
use tokio::{net::UdpSocket, sync::Mutex};

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
	mark_running().unwrap();

	if let Err(e) = server.run().await {
		eprintln!("Error running DNS server: {}", e);
	}
}

#[derive(Debug, Default)]
struct Config {
	upstream_resolvers: Vec<String>,
}

type DNSExpiryTime = chrono::DateTime<chrono::Utc>;
type DNSCache = std::collections::HashMap<DNSCacheKey, DNSCacheEntry>;
struct DNSCacheEntry {
	expiry: DNSExpiryTime,
	response: DNSMessage,
}

#[derive(Debug, Hash, PartialEq, Eq)]
struct DNSCacheKey {
	name: String,
	record_type: QType,
}

struct DNSServer {
	config: Config,
	listener: UdpSocket,
	resolver: DNSStubResolver,

	cache: Arc<Mutex<DNSCache>>,
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
			cache: Arc::new(Mutex::new(DNSCache::new())),
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

	async fn check_cache(&self, question: &DNSQuestion) -> Option<DNSMessage> {
		let cache_key = DNSCacheKey {
			name: question.name.to_domain_name(),
			record_type: question.qtype.clone(),
		};

		let mut cache = self.cache.lock().await;
		if let Some(entry) = cache.get(&cache_key) {
			if entry.expiry > chrono::Utc::now() {
				Some(entry.response.clone())
			} else {
				cache.remove(&cache_key);
				None
			}
		} else {
			None
		}
	}

	// Caches the response for a given question with the appropriate TTL
	async fn cache_result(&self, question: &DNSQuestion, response: &DNSMessage) {
		let cache_key = DNSCacheKey {
			name: question.name.to_domain_name(),
			record_type: question.qtype.clone(),
		};

		let expiry = chrono::Utc::now()
			+ chrono::Duration::seconds(response.answers.iter().map(|a| a.ttl).min().unwrap_or(0) as i64);
		if expiry <= chrono::Utc::now() {
			// If the TTL is expired or zero, we shouldn't cache the response at all
			// This will happen in the case of an NXDOMAIN because we don't parse the
			// TTL from the SOA record yet.
			return;
		}
		let mut cache = self.cache.lock().await;
		cache.insert(
			cache_key,
			DNSCacheEntry {
				expiry,
				response: response.clone(),
			},
		);
	}

	async fn handle_request(&self, src: SocketAddr, request: DNSMessage) {
		let mut answers = Vec::new();
		let mut response_code = DNSResponseCode::NoError;
		for question in &request.questions {
			if let Some(cached_response) = self.check_cache(question).await {
				answers.extend(cached_response.answers);
				if cached_response.header.flags.response_code != DNSResponseCode::NoError {
					response_code = cached_response.header.flags.response_code;
				}
				continue;
			}

			if let Some(resp) = self
				.resolver
				.resolve(question.clone(), &self.config.upstream_resolvers)
				.await
			{
				// Cache the response with the appropriate TTL
				self.cache_result(question, &resp).await;

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
