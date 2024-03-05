use std::{io::Write, sync::Mutex};

use slog::{o, Drain};

/// Assemble a logger that writes to the given writer.
pub fn assemble_logger<W: Write + Send + 'static>(w: W) -> slog::Logger {
	slog::Logger::root(Mutex::new(slog_json::Json::default(w)).fuse(), o!())
}
