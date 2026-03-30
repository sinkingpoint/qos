use std::os::fd::{AsFd, AsRawFd};

use crate::drm::ioctls::{drm_mode_get_connector, drm_mode_get_resources};

mod cstructs;
mod ioctls;
use bitflags::bitflags;
use bytestruct::int_enum;
use bytestruct_derive::ByteStruct;

pub fn set_master(fd: impl AsFd) -> nix::Result<()> {
	unsafe {
		ioctls::drm_set_master(fd.as_fd().as_raw_fd())?;
	}
	Ok(())
}

#[derive(Debug, Clone)]
pub struct DrmModeResources {
	pub connectors: Vec<u32>,
	pub crtcs: Vec<u32>,
	pub encoders: Vec<u32>,
	pub framebuffers: Vec<u32>,

	pub min_width: u32,
	pub max_width: u32,
	pub min_height: u32,
	pub max_height: u32,
}

impl DrmModeResources {
	fn from_cstruct(res: &cstructs::DrmModeRes) -> Self {
		Self {
			connectors: vec![0; res.count_connectors as usize],
			crtcs: vec![0; res.count_crtcs as usize],
			encoders: vec![0; res.count_encoders as usize],
			framebuffers: vec![0; res.count_framebuffers as usize],
			min_width: res.min_width,
			max_width: res.max_width,
			min_height: res.min_height,
			max_height: res.max_height,
		}
	}
}

// Gets the DRM resources for the given file descriptor, along with the ids of the connectors, crtcs, encoders, and framebuffers.
// SAFETY: The caller must ensure that `fd` is a valid file descriptor for a DRM device,
// and that the caller has the necessary permissions to perform the ioctl operations.
pub fn get_drm_resources(fd: impl AsFd) -> nix::Result<DrmModeResources> {
	let mut res = cstructs::DrmModeRes::default();
	unsafe { drm_mode_get_resources(fd.as_fd().as_raw_fd(), &mut res) }?;

	let mut resources = DrmModeResources::from_cstruct(&res);

	res.connector_id_ptr = resources.connectors.as_mut_ptr() as u64;
	res.crtc_id_ptr = resources.crtcs.as_mut_ptr() as u64;
	res.encoder_id_ptr = resources.encoders.as_mut_ptr() as u64;
	res.framebuffer_id_ptr = resources.framebuffers.as_mut_ptr() as u64;

	unsafe { drm_mode_get_resources(fd.as_fd().as_raw_fd(), &mut res) }?;

	Ok(resources)
}

bitflags! {
  #[derive(Debug, Clone)]
	pub struct DrmModeInfoFlags: u32 {
	const DRM_MODE_FLAG_PHSYNC = 1;
	const DRM_MODE_FLAG_NHSYNC = 1<<1;
	const DRM_MODE_FLAG_PVSYNC = 1<<2;
	const DRM_MODE_FLAG_NVSYNC = 1<<3;
	const DRM_MODE_FLAG_INTERLACE = 1<<4;
	const DRM_MODE_FLAG_DBLSCAN = 1<<5;
	const DRM_MODE_FLAG_CSYNC = 1<<6;
	const DRM_MODE_FLAG_PCSYNC = 1<<7;
	const DRM_MODE_FLAG_NCSYNC = 1<<8;
	const DRM_MODE_FLAG_HSKEW = 1<<9;
	const DRM_MODE_FLAG_BCAST = 1<<10;
	const DRM_MODE_FLAG_PIXMUX = 1<<11;
	const DRM_MODE_FLAG_DBLCLK = 1<<12;
	const DRM_MODE_FLAG_CLKDIV2 = 1<<13;
  }
}

bitflags! {
  #[derive(Debug, Clone)]
	pub struct DrmModeInfoType: u32 {
	const DRM_MODE_TYPE_BUILTIN= 1;
	const DRM_MODE_TYPE_CLOCK_C = 1<<1;
	const DRM_MODE_TYPE_CRTC_C = 1<<2;
	const DRM_MODE_TYPE_PREFERRED = 1<<3;
  const DRM_MODE_TYPE_DEFAULT = 1<<4;
	const DRM_MODE_TYPE_USERDEF = 1<<5;
	const DRM_MODE_TYPE_DRIVER = 1<<6;
  }
}

#[derive(Debug, Clone)]
pub struct DrmModeInfo {
	// Pixel clock in kHz
	pub clock: u32,

	// horizontal display size
	pub hdisplay: u16,

	// horizontal sync start
	pub hsync_start: u16,

	// horizontal sync end
	pub hsync_end: u16,

	// horizontal total size
	pub htotal: u16,

	// horizontal skew
	pub hskew: u16,

	// vertical display size
	pub vdisplay: u16,

	// vertical sync start
	pub vsync_start: u16,

	// vertical sync end
	pub vsync_end: u16,

	// vertical total size
	pub vtotal: u16,

	// vertical scan
	pub vscan: u16,

	// approximate vertical refresh rate in Hz
	pub vrefresh: u32,

	// bitmask of misc. flags, see DRM_MODE_FLAG_* defines
	pub flags: DrmModeInfoFlags,

	// bitmask of type flags, see DRM_MODE_TYPE_* defines
	pub ty: DrmModeInfoType,

	// string describing the mode resolution
	pub name: String,
}

impl DrmModeInfo {
	fn from_cstruct(info: cstructs::DrmModeInfo) -> Self {
		Self {
			clock: info.clock,
			hdisplay: info.hdisplay,
			hsync_start: info.hsync_start,
			hsync_end: info.hsync_end,
			htotal: info.htotal,
			hskew: info.hskew,
			vdisplay: info.vdisplay,
			vsync_start: info.vsync_start,
			vsync_end: info.vsync_end,
			vtotal: info.vtotal,
			vscan: info.vscan,
			vrefresh: info.vrefresh,
			flags: DrmModeInfoFlags::from_bits_truncate(info.flags),
			ty: DrmModeInfoType::from_bits_truncate(info.ty),
			name: String::from_utf8_lossy(&info.name).trim_end_matches('\0').to_string(),
		}
	}
}

#[derive(Debug, Clone, PartialEq)]
pub enum DrmConnection {
	Connected = 1,
	Disconnected = 2,
	UnknownConnection = 3,
}

#[derive(Debug, Clone)]
pub struct DrmModeConnector {
	pub encoder_id: u32,
	pub connector_id: u32,

	pub connector_type: u32,
	pub connector_type_id: u32,

	pub connection: DrmConnection,
	pub mm_width: u32,
	pub mm_height: u32,
	pub subpixel: u32,

	pub modes: Vec<DrmModeInfo>,
}

pub fn get_drm_connector(fd: impl AsFd, connector_id: u32) -> nix::Result<DrmModeConnector> {
	let mut res = cstructs::DrmModeGetConnector {
		connector_id,
		..Default::default()
	};

	unsafe { drm_mode_get_connector(fd.as_fd().as_raw_fd(), &mut res) }?;

	let mut modes = vec![cstructs::DrmModeInfo::default(); res.count_modes as usize];
	res.modes_ptr = modes.as_mut_ptr() as u64;

	// Set these to 0 because we don't read them, to skip the kernel trying to write them.
	res.count_props = 0;
	res.count_encoders = 0;

	unsafe { drm_mode_get_connector(fd.as_fd().as_raw_fd(), &mut res) }?;

	let modes: Vec<_> = modes.into_iter().map(DrmModeInfo::from_cstruct).collect();

	Ok(DrmModeConnector {
		encoder_id: res.encoder_id,
		connector_id: res.connector_id,
		connector_type: res.connector_type,
		connector_type_id: res.connector_type_id,
		connection: match res.connection {
			1 => DrmConnection::Connected,
			2 => DrmConnection::Disconnected,
			_ => DrmConnection::UnknownConnection,
		},
		mm_width: res.mm_width,
		mm_height: res.mm_height,
		subpixel: res.subpixel,
		modes,
	})
}

pub struct EncoderInfo {
	pub encoder_id: u32,
	pub encoder_type: u32,
	pub crtc_id: u32,
	pub possible_crtcs: u32,
	pub possible_clones: u32,
}

impl EncoderInfo {
	fn from_cstruct(info: cstructs::DrmModeGetEncoder) -> Self {
		Self {
			encoder_id: info.encoder_id,
			encoder_type: info.encoder_type,
			crtc_id: info.crtc_id,
			possible_crtcs: info.possible_crtcs,
			possible_clones: info.possible_clones,
		}
	}
}

pub fn get_encoder(fd: impl AsFd, encoder_id: u32) -> nix::Result<EncoderInfo> {
	let mut res = cstructs::DrmModeGetEncoder {
		encoder_id,
		..Default::default()
	};

	unsafe { ioctls::drm_mode_get_encoder(fd.as_fd().as_raw_fd(), &mut res) }?;

	Ok(EncoderInfo::from_cstruct(res))
}

pub struct DumbBuffer {
	pub handle: u32,
	pub pitch: u32,
	pub size: usize,
}

impl DumbBuffer {
	pub fn create(fd: impl AsFd, width: u32, height: u32, bpp: u32) -> nix::Result<Self> {
		let mut res = cstructs::DrmModeCreateDumb {
			width,
			height,
			bpp,
			..Default::default()
		};

		unsafe { ioctls::drm_mode_create_dumb(fd.as_fd().as_raw_fd(), &mut res) }?;

		Ok(Self {
			handle: res.handle,
			pitch: res.pitch,
			size: res.size as usize,
		})
	}
}

// Maps the given dumb buffer and returns the offset that can be used with mmap to access the buffer's pixels.
pub fn map_dumb_buffer(fd: impl AsFd, buffer: &DumbBuffer) -> nix::Result<u64> {
	let mut res = cstructs::DrmModeMapDumb {
		handle: buffer.handle,
		..Default::default()
	};

	unsafe { ioctls::drm_mode_map_dumb(fd.as_fd().as_raw_fd(), &mut res) }?;

	Ok(res.offset)
}

// Adds a framebuffer for the given dumb buffer and returns the framebuffer ID.
pub fn add_framebuffer(
	fd: impl AsFd,
	width: u32,
	height: u32,
	bpp: u32,
	depth: u32,
	pitch: u32,
	handle: u32,
) -> nix::Result<u32> {
	let mut res = cstructs::DrmModeFbCmd {
		width,
		height,
		bpp,
		depth,
		pitch,
		handle,
		fb_id: 0,
	};

	unsafe { ioctls::drm_mode_add_fb(fd.as_fd().as_raw_fd(), &mut res) }?;

	Ok(res.fb_id)
}

// Sets the CRTC to display the given framebuffer on the specified connectors with the given mode.
pub fn set_crtc(fd: impl AsFd, crtc_id: u32, fb_id: u32, connectors: &[u32], mode: &DrmModeInfo) -> nix::Result<()> {
	let mut res = cstructs::DrmModeSetCrtc {
		crtc_id,
		fb_id,
		count_connectors: connectors.len() as u32,
		set_connectors_ptr: connectors.as_ptr() as u64,
		x: 0,
		y: 0,
		gamma_size: 0,
		mode_valid: 1,
		mode: cstructs::DrmModeInfo {
			clock: mode.clock,
			hdisplay: mode.hdisplay,
			hsync_start: mode.hsync_start,
			hsync_end: mode.hsync_end,
			htotal: mode.htotal,
			hskew: mode.hskew,
			vdisplay: mode.vdisplay,
			vsync_start: mode.vsync_start,
			vsync_end: mode.vsync_end,
			vtotal: mode.vtotal,
			vscan: mode.vscan,
			vrefresh: mode.vrefresh,
			flags: mode.flags.bits(),
			ty: mode.ty.bits(),
			name: [0; 32], // We don't need the name when setting the CRTC, so we can leave it empty.
		},
	};

	unsafe { ioctls::drm_mode_set_crtc(fd.as_fd().as_raw_fd(), &mut res) }?;

	Ok(())
}

pub fn drop_master(fd: impl AsFd) -> nix::Result<()> {
	unsafe {
		ioctls::drm_drop_master(fd.as_fd().as_raw_fd())?;
	}
	Ok(())
}

// Flips the given CRTC to display the given framebuffer, optionally requesting an event when the flip is complete.
pub fn page_flip(fd: impl AsFd, crtc_id: u32, fd_id: u32, request_event: bool) -> nix::Result<()> {
	let mut res = cstructs::DrmModeCrtc {
		crtc_id,
		buffer_id: fd_id,
		flags: if request_event { 1 } else { 0 },
		..Default::default()
	};

	unsafe { ioctls::drm_mode_page_flip(fd.as_fd().as_raw_fd(), &mut res) }?;

	Ok(())
}

// Sets the cursor image for the given CRTC using the provided GEM buffer handle, width, and height.
pub fn set_cursor_bitmap(fd: impl AsFd, crtc_id: u32, width: u32, height: u32, handle: u32) -> nix::Result<()> {
	let mut res = cstructs::DrmModeCursor2 {
		flags: cstructs::DrmCursorFlags::DRM_MODE_CURSOR_BO,
		crtc_id,
		width,
		height,
		handle,
		..Default::default()
	};

	unsafe { ioctls::drm_mode_cursor2(fd.as_fd().as_raw_fd(), &mut res) }?;

	Ok(())
}

// Moves the cursor for the given CRTC to the specified (x, y) coordinates without changing the cursor image.
pub fn move_cursor(fd: impl AsFd, crtc_id: u32, x: i32, y: i32) -> nix::Result<()> {
	let mut res = cstructs::DrmModeCursor2 {
		flags: cstructs::DrmCursorFlags::DRM_MODE_CURSOR_MOVE,
		crtc_id,
		x,
		y,
		..Default::default()
	};

	unsafe { ioctls::drm_mode_cursor2(fd.as_fd().as_raw_fd(), &mut res) }?;

	Ok(())
}
