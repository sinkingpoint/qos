use std::{num::NonZeroUsize, os::fd::AsFd};

use thiserror::Error;

use crate::{
	bmp::BMPImage,
	drm::{
		DrmConnection, DrmModeInfoType, DumbBuffer, add_framebuffer, drop_master, get_drm_connector, get_encoder,
		map_dumb_buffer, page_flip, set_crtc, set_cursor_bitmap, set_master,
	},
	events::{
		CompositorEvent,
		drm::DrmEventType,
		input::{Event, KeyCode, KeyState},
	},
	wayland::WaylandCompositor,
};

mod bmp;
mod cursor;
mod drm;
mod events;
mod wayland;

fn main() {
	let card_path = match find_drm_card() {
		Some(path) => path,
		None => {
			eprintln!("No DRM card found");
			return;
		}
	};

	let card = match std::fs::OpenOptions::new().read(true).write(true).open(&card_path) {
		Ok(file) => file,
		Err(err) => {
			eprintln!("Failed to open {}: {}", card_path, err);
			return;
		}
	};

	if let Err(err) = set_master(&card) {
		eprintln!("Failed to set DRM master: {}", err);
		return;
	}

	let resources = match drm::get_drm_resources(&card) {
		Ok(res) => res,
		Err(err) => {
			eprintln!("Failed to get DRM resources: {}", err);
			return;
		}
	};

	let (connector, mode) = match resources.connectors.iter().find_map(|connector_id| {
		let connector = get_drm_connector(&card, *connector_id).ok()?;
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

	let mut video_buffer = match VideoBuffer::create(&card, mode.hdisplay as u32, mode.vdisplay as u32, 32, 24) {
		Ok(buf) => buf,
		Err(err) => {
			eprintln!("Failed to create video buffer: {:?}", err);
			return;
		}
	};

	let mut video_buffer2 = match VideoBuffer::create(&card, mode.hdisplay as u32, mode.vdisplay as u32, 32, 24) {
		Ok(buf) => buf,
		Err(err) => {
			eprintln!("Failed to create second video buffer: {:?}", err);
			return;
		}
	};

	let encoder = match get_encoder(&card, connector.encoder_id) {
		Ok(enc) => enc,
		Err(err) => {
			eprintln!("Failed to get encoder: {}", err);
			return;
		}
	};

	let (input_event_sender, input_event_receiver) = std::sync::mpsc::channel();
	let input_thread_handle = events::input_watcher_event_thread(input_event_sender.clone());
	let drm_thread_handle = events::drm_event_thread(
		card.try_clone().expect("Failed to clone card file"),
		input_event_sender.clone(),
	);
	let wayland_thread_handle = events::wayland_event_thread("wayland-0".to_string(), input_event_sender);

	let mut wayland = WaylandCompositor::new();

	set_crtc(
		&card,
		encoder.crtc_id,
		video_buffer.framebuffer_id,
		&[connector.connector_id],
		&mode,
	)
	.unwrap();

	let mut active_buffer = &mut video_buffer;
	let mut inactive_buffer = &mut video_buffer2;
	active_buffer.clear(0x000000FF); // Clear to blue
	active_buffer.draw_rect(100, 100, 200, 150, 0x0000FFFF); // Draw a blue rectangle
	active_buffer.draw_rect(200, 100, 200, 150, 0x00FF00FF); // Draw a green rectangle

	if let Err(err) = active_buffer.flip_to(&card, encoder.crtc_id) {
		eprintln!("Failed to flip to initial buffer: {}", err);
		return;
	}

	let (cursor_buffer, cursor_data) = create_cursor(&card);
	set_cursor_bitmap(
		&card,
		encoder.crtc_id,
		cursor_data.width,
		cursor_data.height,
		cursor_buffer.handle,
	)
	.expect("Failed to set cursor bitmap");

	let mut cursor = cursor::Cursor::new(mode.hdisplay as i32, mode.vdisplay as i32);
	cursor.update_kernel(&card, encoder.crtc_id);

	// Event loop to keep the program running and handle page flip events
	'outer: loop {
		let event = match input_event_receiver.recv() {
			Ok(event) => event,
			Err(err) => {
				eprintln!("Failed to receive event: {}", err);
				break;
			}
		};

		match event {
			CompositorEvent::Drm(drm_event) if drm_event.event_type == DrmEventType::FlipComplete => {
				// FlipComplete means inactive_buffer just became the displayed buffer.
				// Swap so that inactive_buffer is the safe-to-write one (just came off screen).
				std::mem::swap(&mut active_buffer, &mut inactive_buffer);

				inactive_buffer.clear(0x000000FF); // Clear to blue
				inactive_buffer.draw_rect(100, 100, 200, 150, 0x0000FFFF); // Draw a blue rectangle
				inactive_buffer.draw_rect(200, 100, 200, 150, 0x00FF00FF); // Draw a green rectangle

				if let Err(err) = inactive_buffer.flip_to(&card, encoder.crtc_id) {
					eprintln!("Failed to flip buffer: {}", err);
				}
			}
			CompositorEvent::Input(event)
				if matches!(
					event,
					Event::Absolute(_, _) | Event::Relative(_, _) | Event::Key(KeyCode::BtnTouch, _)
				) =>
			{
				cursor.handle_input_event(&event);
			}
			CompositorEvent::Input(Event::Synchronise(_, _)) => {
				cursor.update_kernel(&card, encoder.crtc_id);
			}
			CompositorEvent::Input(Event::Key(KeyCode::KeyEsc, KeyState::Pressed)) => {
				println!("Key press event received, exiting...");
				break 'outer;
			}
			CompositorEvent::Wayland(event) => {
				wayland.handle_event(event);
			}
			_ => {
				// Handle other events as needed
			}
		}
	}

	input_thread_handle.kill().unwrap();
	drm_thread_handle.kill().unwrap();
	wayland_thread_handle.kill().unwrap();
	drop_master(&card).unwrap();
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

// Loads a BMP image from the specified file path, creates a dumb buffer for the cursor, and uploads the image data to the buffer.
// Returns the created dumb buffer and the loaded BMP image data.
fn create_cursor(card: &std::fs::File) -> (DumbBuffer, BMPImage) {
	let cursor_data = BMPImage::from_file("/home/colin/cursor.bmp").unwrap();
	let cursor_buffer =
		DumbBuffer::create(card, cursor_data.width, cursor_data.height, 32).expect("Failed to create cursor buffer");

	let offset = map_dumb_buffer(card, &cursor_buffer).expect("Failed to map cursor buffer");

	let cursor_pixels = unsafe {
		nix::sys::mman::mmap(
			None,
			NonZeroUsize::new(cursor_buffer.size).expect("Cursor buffer returned size of 0"),
			nix::sys::mman::ProtFlags::PROT_READ | nix::sys::mman::ProtFlags::PROT_WRITE,
			nix::sys::mman::MapFlags::MAP_SHARED,
			Some(card.as_fd()),
			offset as i64,
		)
		.expect("Failed to mmap cursor buffer") as *mut u32
	};

	unsafe {
		std::slice::from_raw_parts_mut(cursor_pixels, (cursor_data.width * cursor_data.height) as usize)
			.copy_from_slice(&cursor_data.pixels);
	}

	unsafe {
		nix::sys::mman::munmap(cursor_pixels as *mut _, cursor_buffer.size).expect("Failed to unmap cursor buffer")
	};

	(cursor_buffer, cursor_data)
}

#[derive(Error, Debug)]
enum VideoBufferError {
	#[error("Failed to create video buffer: {0}")]
	NixError(nix::Error),
}

struct VideoBuffer {
	pixels: *mut u32,
	width: u32,
	height: u32,
	pitch: u32, // row stride in pixels (pitch_bytes / 4)

	framebuffer_id: u32,
	buffer_size: usize,
}

impl VideoBuffer {
	pub fn create(fd: impl AsFd, width: u32, height: u32, bpp: u32, depth: u32) -> Result<Self, VideoBufferError> {
		let fdfd = fd.as_fd();
		let dumb_buffer = match DumbBuffer::create(fdfd, width, height, bpp) {
			Ok(buf) => buf,
			Err(err) => {
				eprintln!("Failed to create dumb buffer: {}", err);
				return Err(VideoBufferError::NixError(err));
			}
		};

		let fb_id = add_framebuffer(fdfd, width, height, bpp, depth, dumb_buffer.pitch, dumb_buffer.handle)
			.map_err(VideoBufferError::NixError)?;
		let buffer_offset = map_dumb_buffer(fdfd, &dumb_buffer).map_err(VideoBufferError::NixError)?;

		let pixels = unsafe {
			nix::sys::mman::mmap(
				None,
				NonZeroUsize::new(dumb_buffer.size as usize).expect("dumb buffer returned size of 0"),
				nix::sys::mman::ProtFlags::PROT_READ | nix::sys::mman::ProtFlags::PROT_WRITE,
				nix::sys::mman::MapFlags::MAP_SHARED,
				Some(fdfd),
				buffer_offset as i64,
			)
			.map_err(VideoBufferError::NixError)?
		};

		Ok(Self::new(
			pixels as *mut u32,
			width,
			height,
			dumb_buffer.pitch / 4,
			fb_id,
			dumb_buffer.size as usize,
		))
	}

	pub fn new(pixels: *mut u32, width: u32, height: u32, pitch: u32, framebuffer_id: u32, buffer_size: usize) -> Self {
		Self {
			pixels,
			width,
			height,
			pitch,
			framebuffer_id,
			buffer_size,
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

	pub fn flip_to(&self, fd: impl AsFd, crtc_id: u32) -> nix::Result<()> {
		page_flip(fd, crtc_id, self.framebuffer_id, true)?;
		Ok(())
	}
}

impl Drop for VideoBuffer {
	fn drop(&mut self) {
		unsafe {
			nix::sys::mman::munmap(self.pixels as *mut _, self.buffer_size).ok();
		}
	}
}
