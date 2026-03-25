use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use std::time::Duration;

use bytestruct::WriteToWithEndian;
use tokio::sync::{oneshot, Mutex};
use tokio::{net::UdpSocket, time::timeout};

use crate::message::{DNSMessage, DNSQuestion};

pub struct DNSStubResolver {
	listener: Arc<UdpSocket>,
	active_queries: Arc<Mutex<HashMap<u16, oneshot::Sender<DNSMessage>>>>,
	join_handle: tokio::task::JoinHandle<io::Result<()>>,
}

impl DNSStubResolver {
	pub async fn new() -> std::io::Result<Self> {
		let listener = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
		let active_queries = Arc::new(Mutex::new(HashMap::new()));
		Ok(Self {
			listener: listener.clone(),
			active_queries: active_queries.clone(),
			join_handle: tokio::spawn(start(listener.clone(), active_queries.clone())),
		})
	}

	pub async fn resolve<S: AsRef<str>>(&self, question: DNSQuestion, resolvers_to_try: &[S]) -> Option<DNSMessage> {
		let msg = DNSMessage::new_query(question);

		let mut msg_bytes = Vec::new();
		if let Err(e) = msg.write_to_with_endian(&mut msg_bytes, bytestruct::Endian::Big) {
			eprintln!("Failed to serialize DNS message: {}", e);
			return None;
		}

		for resolver in resolvers_to_try {
			let (response_tx, response_rx) = oneshot::channel();
			let txid = msg.header.transaction_id;
			{
				let mut active_queries = self.active_queries.lock().await;
				active_queries.insert(txid, response_tx);
			}

			if let Err(e) = self.listener.send_to(&msg_bytes, resolver.as_ref()).await {
				eprintln!("Failed to send DNS query to {}: {}", resolver.as_ref(), e);
				let mut active_queries = self.active_queries.lock().await;
				active_queries.remove(&txid);
				continue;
			}

			match timeout(Duration::from_secs(5), response_rx).await {
				Ok(Ok(response)) => return Some(response),
				Ok(Err(_)) => {
					eprintln!("Response channel closed for resolver {}", resolver.as_ref());
					let mut active_queries = self.active_queries.lock().await;
					active_queries.remove(&txid);
				}
				Err(_) => {
					eprintln!("Timeout waiting for response from resolver {}", resolver.as_ref());
					let mut active_queries = self.active_queries.lock().await;
					active_queries.remove(&txid);
				}
			}
		}

		None
	}
}

impl Drop for DNSStubResolver {
	fn drop(&mut self) {
		self.join_handle.abort();
	}
}

async fn start(
	listener: Arc<UdpSocket>,
	active_queries: Arc<Mutex<HashMap<u16, oneshot::Sender<DNSMessage>>>>,
) -> std::io::Result<()> {
	let mut buf = [0u8; 1024];
	loop {
		let (len, _) = listener.recv_from(&mut buf).await?;
		let msg = match DNSMessage::from_bytes(&buf[..len], bytestruct::Endian::Big) {
			Ok(m) => m,
			Err(e) => {
				eprintln!("Failed to parse DNS message: {}", e);
				continue;
			}
		};

		let mut active_queries = active_queries.lock().await;
		if let Some(response_tx) = active_queries.remove(&msg.header.transaction_id) {
			let _ = response_tx.send(msg);
		} else {
			eprintln!(
				"Received unsolicited DNS message with transaction ID {}",
				msg.header.transaction_id
			);
		}
	}
}
