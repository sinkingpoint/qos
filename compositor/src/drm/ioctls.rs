use nix::{ioctl_none, ioctl_readwrite};

use super::cstructs::{
	DrmModeCreateDumb, DrmModeCrtc, DrmModeFbCmd, DrmModeGetConnector, DrmModeGetEncoder, DrmModeMapDumb, DrmModeRes,
	DrmModeSetCrtc,
};

const DRM_IOCTL_BASE: u8 = b'd';
ioctl_none!(drm_set_master, DRM_IOCTL_BASE, 0x1e);
ioctl_none!(drm_drop_master, DRM_IOCTL_BASE, 0x1f);
ioctl_readwrite!(drm_mode_get_resources, DRM_IOCTL_BASE, 0xA0, DrmModeRes);
ioctl_readwrite!(drm_mode_get_connector, DRM_IOCTL_BASE, 0xA7, DrmModeGetConnector);
ioctl_readwrite!(drm_mode_get_encoder, DRM_IOCTL_BASE, 0xA6, DrmModeGetEncoder);
ioctl_readwrite!(drm_mode_create_dumb, DRM_IOCTL_BASE, 0xB2, DrmModeCreateDumb);
ioctl_readwrite!(drm_mode_map_dumb, DRM_IOCTL_BASE, 0xB3, DrmModeMapDumb);
ioctl_readwrite!(drm_mode_add_fb, DRM_IOCTL_BASE, 0xAE, DrmModeFbCmd);
ioctl_readwrite!(drm_mode_set_crtc, DRM_IOCTL_BASE, 0xA2, DrmModeSetCrtc);
ioctl_readwrite!(drm_mode_page_flip, DRM_IOCTL_BASE, 0xB0, DrmModeCrtc);
