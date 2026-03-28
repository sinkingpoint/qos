use std::{num::NonZeroUsize, os::fd::BorrowedFd};

use nix::{
	fcntl::{OFlag, open},
	sys::stat::Mode,
};

use crate::drm::{
	DrmConnection, DrmModeInfoType, DumbBuffer, add_framebuffer, drm_set_master, get_drm_connector, get_encoder,
	map_dumb_buffer, set_crtc,
};

mod drm;

fn main() {
	let card_path = match find_drm_card() {
		Some(path) => path,
		None => {
			eprintln!("No DRM card found");
			return;
		}
	};

	println!("Using DRM card: {}", card_path);
	let card0_fd = match open(card_path.as_str(), OFlag::O_RDWR, Mode::empty()) {
		Ok(fd) => fd,
		Err(err) => {
			eprintln!("Failed to open {}: {}", card_path, err);
			return;
		}
	};

	if let Err(err) = unsafe { drm_set_master(card0_fd) } {
		eprintln!("Failed to set DRM master: {}", err);
		return;
	}

	let resources = match drm::get_drm_resources(card0_fd) {
		Ok(res) => res,
		Err(err) => {
			eprintln!("Failed to get DRM resources: {}", err);
			return;
		}
	};

	println!("DRM Resources: {:#?}", resources);

	let (connector, mode) = match resources.connectors.iter().find_map(|connector_id| {
		let connector = get_drm_connector(card0_fd, *connector_id).ok()?;
		if connector.connection != DrmConnection::Connected {
			return None;
		}

		let mode = connector
			.modes
			.iter()
			.find(|mode| mode.ty.contains(DrmModeInfoType::DRM_MODE_TYPE_PREFERRED))?
			.clone();

		Some((connector, mode))
	}) {
		Some((connector, mode)) => (connector, mode),
		None => {
			eprintln!("No connected display found");
			return;
		}
	};

	let dumb_buffer = match DumbBuffer::create(card0_fd, mode.hdisplay as u32, mode.vdisplay as u32, 32) {
		Ok(buf) => buf,
		Err(err) => {
			eprintln!("Failed to create dumb buffer: {}", err);
			return;
		}
	};

	let fb_id = add_framebuffer(
		card0_fd,
		mode.hdisplay as u32,
		mode.vdisplay as u32,
		32,
		24,
		dumb_buffer.pitch,
		dumb_buffer.handle,
	)
	.unwrap();
	let buffer_offset = map_dumb_buffer(card0_fd, &dumb_buffer).unwrap();

	let pixels = unsafe {
		nix::sys::mman::mmap(
			None,
			NonZeroUsize::new(dumb_buffer.size as usize).unwrap(),
			nix::sys::mman::ProtFlags::PROT_READ | nix::sys::mman::ProtFlags::PROT_WRITE,
			nix::sys::mman::MapFlags::MAP_SHARED,
			Some(BorrowedFd::borrow_raw(card0_fd)),
			buffer_offset as i64,
		)
		.unwrap()
	};

	let mut video_buffer = VideoBuffer::new(
		pixels as *mut u32,
		mode.hdisplay as u32,
		mode.vdisplay as u32,
		dumb_buffer.pitch / 4,
	);
	video_buffer.clear(0x000000FF); // Clear to red
	video_buffer.draw_rect(100, 100, 200, 150, 0x0000FFFF); // Draw a blue rectangle
	video_buffer.draw_rect(200, 100, 200, 150, 0x00FF00FF); // Draw a green rectangle

	let encoder = match get_encoder(card0_fd, connector.encoder_id) {
		Ok(enc) => enc,
		Err(err) => {
			eprintln!("Failed to get encoder: {}", err);
			return;
		}
	};

	set_crtc(card0_fd, encoder.crtc_id, fb_id, &[connector.connector_id], &mode).unwrap();

	// Wait for Enter, then clean up
	let mut input = String::new();
	std::io::stdin().read_line(&mut input).ok();

	unsafe { drm::drm_drop_master(card0_fd) }.ok();
}

fn find_drm_card() -> Option<String> {
	for i in 0..16 {
		let path = format!("/dev/dri/card{}", i);
		if std::fs::metadata(&path).is_ok() {
			return Some(path);
		}
	}
	None
}

struct VideoBuffer {
	pixels: *mut u32,
	width: u32,
	height: u32,
	pitch: u32, // row stride in pixels (pitch_bytes / 4)
}

impl VideoBuffer {
	pub fn new(pixels: *mut u32, width: u32, height: u32, pitch: u32) -> Self {
		Self {
			pixels,
			width,
			height,
			pitch,
		}
	}

	pub fn clear(&mut self, color: u32) {
		for y in 0..self.height {
			unsafe {
				std::slice::from_raw_parts_mut(self.pixels.add((y * self.pitch) as usize), self.width as usize)
					.fill(color);
			}
		}
	}

	pub fn draw_rect(&mut self, x: u32, y: u32, width: u32, height: u32, color: u32) {
		for j in 0..height {
			for i in 0..width {
				let idx = ((y + j) * self.pitch + (x + i)) as usize;
				unsafe {
					*self.pixels.add(idx) = color;
				}
			}
		}
	}
}
