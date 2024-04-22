use std::path::Path;

use tokio::{
	io::{self, AsyncWriteExt},
	net::{UnixSocket, UnixStream},
};

use crate::KV;

pub const START_WRITE_STREAM_ACTION: &str = "start-write-stream";

/// Starts a write stream with the given fields, returning the socket that can then be used
/// to stream logs to a loggerd instance.
pub async fn start_write_stream(socket_path: &Path, fields: Vec<KV>) -> io::Result<UnixStream> {
	let mut conn = UnixSocket::new_stream()?.connect(socket_path).await?;
	let fields_str = fields
		.iter()
		.map(|kv| format!("{}={}", kv.key, kv.value))
		.collect::<Vec<_>>()
		.join(" ");
	let header_string = format!("ACTION={} {}\n", START_WRITE_STREAM_ACTION, fields_str);
	conn.write_all(header_string.as_bytes()).await?;

	Ok(conn)
}
