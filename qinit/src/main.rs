use std::ffi::{CStr, CString};

use nix::unistd::execve;

fn main() {
	let command = CString::new("/sbin/getty").unwrap();
	let args = [command.as_c_str(), &CString::new("/dev/tty1").unwrap()];
	execve::<_, &CStr>(&command, &args, &[]).unwrap();
}
