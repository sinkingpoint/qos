#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct DrmModeGetConnector {
	pub encoders_ptr: u64,
	pub modes_ptr: u64,
	pub props_ptr: u64,
	pub prop_values_ptr: u64,

	pub count_modes: u32,
	pub count_props: u32,
	pub count_encoders: u32,

	pub encoder_id: u32,
	pub connector_id: u32,

	pub connector_type: u32,
	pub connector_type_id: u32,

	pub connection: u32,
	pub mm_width: u32,
	pub mm_height: u32,
	pub subpixel: u32,
	pub pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct DrmModeRes {
	pub framebuffer_id_ptr: u64,
	pub crtc_id_ptr: u64,
	pub connector_id_ptr: u64,
	pub encoder_id_ptr: u64,

	pub count_framebuffers: u32,
	pub count_crtcs: u32,
	pub count_connectors: u32,
	pub count_encoders: u32,

	pub min_width: u32,
	pub max_width: u32,
	pub min_height: u32,
	pub max_height: u32,
}

const DRM_DISPLAY_MODE_LEN: usize = 32;
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct DrmModeInfo {
	pub clock: u32,
	pub hdisplay: u16,
	pub hsync_start: u16,
	pub hsync_end: u16,
	pub htotal: u16,
	pub hskew: u16,
	pub vdisplay: u16,
	pub vsync_start: u16,
	pub vsync_end: u16,
	pub vtotal: u16,
	pub vscan: u16,
	pub vrefresh: u32,
	pub flags: u32,
	pub ty: u32,
	pub name: [u8; DRM_DISPLAY_MODE_LEN],
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct DrmModeGetEncoder {
	pub encoder_id: u32,
	pub encoder_type: u32,
	pub crtc_id: u32,
	pub possible_crtcs: u32,
	pub possible_clones: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct DrmModeCreateDumb {
	pub height: u32,
	pub width: u32,
	pub bpp: u32,
	pub flags: u32,
	pub handle: u32,
	pub pitch: u32,
	pub size: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct DrmModeFbCmd {
	pub fb_id: u32,
	pub width: u32,
	pub height: u32,
	pub pitch: u32,
	pub bpp: u32,
	pub depth: u32,
	pub handle: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct DrmModeMapDumb {
	pub handle: u32,
	pub _pad: u32,
	pub offset: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct DrmModeSetCrtc {
	pub set_connectors_ptr: u64,
	pub count_connectors: u32,
	pub crtc_id: u32,
	pub fb_id: u32,
	pub x: u32,
	pub y: u32,
	pub gamma_size: u32,
	pub mode_valid: u32,
	pub mode: DrmModeInfo,
}
