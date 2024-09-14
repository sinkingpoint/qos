use std::io;

use super::address::MacAddress;

pub(crate) fn new_mac_address(buffer: &[u8]) -> io::Result<MacAddress> {
	Ok(MacAddress::new(buffer.try_into().map_err(|e| {
		io::Error::new(io::ErrorKind::InvalidData, format!("expected 6 bytes, got {:?}", e))
	})?))
}
