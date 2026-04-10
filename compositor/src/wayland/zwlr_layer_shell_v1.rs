use std::{os::unix::net::UnixStream, sync::Arc};

use wayland::{
	types::{WaylandEncodedString, WaylandPayload},
	zwlr_layer_shell_v1::{
		AckConfigureRequest, Anchor, ConfigureEvent, DestroyRequest, GetLayerSurfaceRequest, GetPopupRequest,
		KeyboardInteractivity, Layer, SetAnchorRequest, SetExclusiveEdgeRequest, SetExclusiveZoneRequest,
		SetKeyboardInteractivityRequest, SetLayerRequest, SetMarginRequest, SetSizeRequest,
	},
};

use crate::wayland::{
	DisplayGeometry,
	types::{ClientEffect, Command, SubSystem, SubsystemType, WaylandResult},
};

pub struct ZwlrLayerShellV1;

impl SubSystem for ZwlrLayerShellV1 {
	type Request = ZwlrLayerShellV1Request;
	const NAME: &'static str = "zwlr_layer_shell_v1";
	const VERSION: u32 = 1;
}

wayland_interface!(ZwlrLayerShellV1, ZwlrLayerShellV1Request {
  DestroyRequest::OPCODE => Destroy(DestroyRequest),
  GetLayerSurfaceRequest::OPCODE => GetLayerSurface(GetLayerSurfaceRequest),
});

impl Command<ZwlrLayerShellV1> for GetLayerSurfaceRequest {
	fn handle(
		self,
		_connection: &Arc<UnixStream>,
		_zwlr_layer_shell_v1: &mut ZwlrLayerShellV1,
	) -> WaylandResult<Option<ClientEffect>> {
		Ok(Some(ClientEffect::Register(
			self.new_id,
			SubsystemType::ZwlrLayerSurfaceV1(ZwlrLayerSurfaceV1::new(
				self.new_id,
				self.wl_surface_id,
				self.output_id,
				self.layer,
				self.namespace,
			)),
		)))
	}
}

impl Command<ZwlrLayerShellV1> for DestroyRequest {
	fn handle(
		self,
		_connection: &Arc<UnixStream>,
		_zwlr_layer_shell_v1: &mut ZwlrLayerShellV1,
	) -> WaylandResult<Option<ClientEffect>> {
		Ok(Some(ClientEffect::DestroySelf))
	}
}

#[derive(Debug)]
pub enum ConfigureState {
	None,
	SentConfigure(u32),
	Configured,
}

#[derive(Debug)]
pub struct ZwlrLayerSurfaceV1 {
	id: u32,
	pub surface_id: u32,
	next_serial: u32,
	pub mapped: bool,
	output_id: u32,
	pub layer: Layer,
	namespace: WaylandEncodedString,

	requested_size: Option<(i32, i32)>,
	anchor: Anchor,
	exclusive_zone: Option<i32>,
	margin: Option<(i32, i32, i32, i32)>,
	keyboard_interactivity: Option<KeyboardInteractivity>,
	configure_state: ConfigureState,
	popup_id: Option<u32>,
	exclusive_edge: Option<Anchor>,
}

impl ZwlrLayerSurfaceV1 {
	pub fn new(id: u32, surface_id: u32, output_id: u32, layer: Layer, namespace: WaylandEncodedString) -> Self {
		Self {
			id,
			surface_id,
			output_id,
			layer,
			namespace,
			next_serial: 1,
			configure_state: ConfigureState::None,
			requested_size: None,
			anchor: Anchor::empty(),
			exclusive_zone: None,
			margin: None,
			keyboard_interactivity: None,
			popup_id: None,
			exclusive_edge: None,
			mapped: false,
		}
	}

	pub fn compute_position(&self, display_geometry: &DisplayGeometry) -> (i32, i32) {
		let (width, height) = self.compute_size(display_geometry);
		let margin = self.margin.unwrap_or((0, 0, 0, 0));

		let x = if self.anchor.contains(Anchor::Left) {
			margin.3
		} else if self.anchor.contains(Anchor::Right) {
			display_geometry.width - width as i32 - margin.1
		} else {
			(display_geometry.width - width as i32) / 2
		};

		let y = if self.anchor.contains(Anchor::Top) {
			margin.0
		} else if self.anchor.contains(Anchor::Bottom) {
			display_geometry.height - height as i32 - margin.2
		} else {
			(display_geometry.height - height as i32) / 2
		};

		(x, y)
	}

	pub fn compute_size(&self, display_geometry: &DisplayGeometry) -> (u32, u32) {
		let fill_w = self.anchor.contains(Anchor::Left) && self.anchor.contains(Anchor::Right);
		let fill_h = self.anchor.contains(Anchor::Top) && self.anchor.contains(Anchor::Bottom);
		let (mut width, mut height) = self.requested_size.unwrap_or((0, 0));
		if width == 0 && fill_w {
			width = display_geometry.width;
		}
		if height == 0 && fill_h {
			height = display_geometry.height;
		}

		width = width.clamp(0, display_geometry.width);
		height = height.clamp(0, display_geometry.height);
		(width as u32, height as u32)
	}

	pub fn on_surface_committed(
		&mut self,
		connection: &Arc<UnixStream>,
		has_buffer: bool,
		display_geometry: &DisplayGeometry,
	) -> WaylandResult<()> {
		match self.configure_state {
			ConfigureState::None if !has_buffer => {
				let (width, height) = self.compute_size(display_geometry);
				let configure_serial = self.next_serial;
				self.next_serial += 1;
				ConfigureEvent {
					serial: configure_serial,
					width,
					height,
				}
				.write_as_packet(self.id, connection)?;
				self.configure_state = ConfigureState::SentConfigure(configure_serial);
			}
			ConfigureState::Configured => {
				self.mapped = has_buffer;
			}
			_ => {}
		}
		Ok(())
	}
}

impl SubSystem for ZwlrLayerSurfaceV1 {
	type Request = ZwlrLayerSurfaceV1Request;
	const NAME: &'static str = "zwlr_layer_surface_v1";
	const VERSION: u32 = 1;
}

wayland_interface!(ZwlrLayerSurfaceV1, ZwlrLayerSurfaceV1Request {
  SetSizeRequest::OPCODE => SetSize(SetSizeRequest),
  SetAnchorRequest::OPCODE => SetAnchor(SetAnchorRequest),
  SetExclusiveZoneRequest::OPCODE => SetExclusiveZone(SetExclusiveZoneRequest),
  SetMarginRequest::OPCODE => SetMargin(SetMarginRequest),
  SetKeyboardInteractivityRequest::OPCODE => SetKeyboardInteractivity(SetKeyboardInteractivityRequest),
  AckConfigureRequest::OPCODE => AckConfigure(AckConfigureRequest),
  GetPopupRequest::OPCODE => GetPopup(GetPopupRequest),
  SetLayerRequest::OPCODE => SetLayer(SetLayerRequest),
  SetExclusiveEdgeRequest::OPCODE => SetExclusiveEdge(SetExclusiveEdgeRequest),
  DestroyRequest::OPCODE => Destroy(DestroyRequest),
});

impl Command<ZwlrLayerSurfaceV1> for SetSizeRequest {
	fn handle(
		self,
		_connection: &Arc<UnixStream>,
		zwlr_layer_surface_v1: &mut ZwlrLayerSurfaceV1,
	) -> WaylandResult<Option<ClientEffect>> {
		zwlr_layer_surface_v1.requested_size = Some((self.width, self.height));
		Ok(None)
	}
}

impl Command<ZwlrLayerSurfaceV1> for SetAnchorRequest {
	fn handle(
		self,
		_connection: &Arc<UnixStream>,
		zwlr_layer_surface_v1: &mut ZwlrLayerSurfaceV1,
	) -> WaylandResult<Option<ClientEffect>> {
		zwlr_layer_surface_v1.anchor = self.anchor;
		Ok(None)
	}
}

impl Command<ZwlrLayerSurfaceV1> for SetExclusiveZoneRequest {
	fn handle(
		self,
		_connection: &Arc<UnixStream>,
		zwlr_layer_surface_v1: &mut ZwlrLayerSurfaceV1,
	) -> WaylandResult<Option<ClientEffect>> {
		zwlr_layer_surface_v1.exclusive_zone = Some(self.zone);
		Ok(None)
	}
}

impl Command<ZwlrLayerSurfaceV1> for SetMarginRequest {
	fn handle(
		self,
		_connection: &Arc<UnixStream>,
		zwlr_layer_surface_v1: &mut ZwlrLayerSurfaceV1,
	) -> WaylandResult<Option<ClientEffect>> {
		zwlr_layer_surface_v1.margin = Some((self.top, self.right, self.bottom, self.left));
		Ok(None)
	}
}

impl Command<ZwlrLayerSurfaceV1> for SetKeyboardInteractivityRequest {
	fn handle(
		self,
		_connection: &Arc<UnixStream>,
		zwlr_layer_surface_v1: &mut ZwlrLayerSurfaceV1,
	) -> WaylandResult<Option<ClientEffect>> {
		zwlr_layer_surface_v1.keyboard_interactivity = Some(self.interactivity);
		Ok(None)
	}
}

impl Command<ZwlrLayerSurfaceV1> for AckConfigureRequest {
	fn handle(
		self,
		_connection: &Arc<UnixStream>,
		zwlr_layer_surface_v1: &mut ZwlrLayerSurfaceV1,
	) -> WaylandResult<Option<ClientEffect>> {
		if let ConfigureState::SentConfigure(serial) = zwlr_layer_surface_v1.configure_state
			&& serial == self.serial
		{
			zwlr_layer_surface_v1.configure_state = ConfigureState::Configured;
		}
		Ok(None)
	}
}

impl Command<ZwlrLayerSurfaceV1> for DestroyRequest {
	fn handle(
		self,
		_connection: &Arc<UnixStream>,
		_zwlr_layer_surface_v1: &mut ZwlrLayerSurfaceV1,
	) -> WaylandResult<Option<ClientEffect>> {
		Ok(Some(ClientEffect::DestroySelf))
	}
}

impl Command<ZwlrLayerSurfaceV1> for GetPopupRequest {
	fn handle(
		self,
		_connection: &Arc<UnixStream>,
		zwlr_layer_surface_v1: &mut ZwlrLayerSurfaceV1,
	) -> WaylandResult<Option<ClientEffect>> {
		zwlr_layer_surface_v1.popup_id = Some(self.popup_id);
		Ok(None)
	}
}

impl Command<ZwlrLayerSurfaceV1> for SetLayerRequest {
	fn handle(
		self,
		_connection: &Arc<UnixStream>,
		zwlr_layer_surface_v1: &mut ZwlrLayerSurfaceV1,
	) -> WaylandResult<Option<ClientEffect>> {
		zwlr_layer_surface_v1.layer = self.layer;
		Ok(None)
	}
}

impl Command<ZwlrLayerSurfaceV1> for SetExclusiveEdgeRequest {
	fn handle(
		self,
		_connection: &Arc<UnixStream>,
		zwlr_layer_surface_v1: &mut ZwlrLayerSurfaceV1,
	) -> WaylandResult<Option<ClientEffect>> {
		zwlr_layer_surface_v1.exclusive_edge = Some(self.edge);
		Ok(None)
	}
}
