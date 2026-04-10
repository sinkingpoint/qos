use std::{os::unix::net::UnixStream, sync::Arc};

use wayland::registry::BindRequest;
use wayland::seat::CapabilitiesEvent;
use wayland::shm::FormatEvent;
use wayland::types::WaylandPayload;

use crate::wayland::{
	DisplayGeometry,
	compositor::Compositor,
	output::{DoneEvent, Output},
	seat::{Seat, SeatCapabilities},
	shm::SharedMemory,
	types::{ClientEffect, Command, SubSystem, SubsystemType, WaylandError, WaylandResult},
	xdg_wm_base::XdgWmBase,
};

pub struct Registry {
	display_geometry: DisplayGeometry,
}

impl Registry {
	pub fn new(display_geometry: DisplayGeometry) -> Self {
		Self { display_geometry }
	}
}

impl SubSystem for Registry {
	type Request = RegistryRequest;
	const NAME: &'static str = "wl_registry";
}

wayland_interface!(Registry, RegistryRequest {
  BindRequest::OPCODE => Bind(BindRequest),
});

impl Command<Registry> for BindRequest {
	fn handle(self, connection: &Arc<UnixStream>, registry: &mut Registry) -> WaylandResult<Option<ClientEffect>> {
		match self.interface.as_ref() {
			"wl_compositor" => Ok(Some(ClientEffect::Register(
				self.new_id,
				SubsystemType::Compositor(Compositor),
			))),
			"wl_shm" => {
				FormatEvent { format: 0 }
					.write_as_packet(self.new_id, connection)
					.map_err(WaylandError::IOError)?;
				FormatEvent { format: 1 }
					.write_as_packet(self.new_id, connection)
					.map_err(WaylandError::IOError)?;
				Ok(Some(ClientEffect::Register(
					self.new_id,
					SubsystemType::SharedMemory(SharedMemory),
				)))
			}
			"xdg_wm_base" => Ok(Some(ClientEffect::Register(
				self.new_id,
				SubsystemType::XdgWmBase(XdgWmBase),
			))),
			"wl_seat" => {
				CapabilitiesEvent {
					capabilities: (SeatCapabilities::KEYBOARD | SeatCapabilities::POINTER).bits(),
				}
				.write_as_packet(self.new_id, connection)?;
				Ok(Some(ClientEffect::Register(self.new_id, SubsystemType::Seat(Seat))))
			}
			"wl_output" => {
				registry
					.display_geometry
					.geometry_event()
					.write_as_packet(self.new_id, connection)?;
				registry
					.display_geometry
					.mode_event()
					.write_as_packet(self.new_id, connection)?;
				DoneEvent.write_as_packet(self.new_id, connection)?;
				Ok(Some(ClientEffect::Register(self.new_id, SubsystemType::Output(Output))))
			}
			"zwlr_layer_shell_v1" => Ok(Some(ClientEffect::Register(
				self.new_id,
				SubsystemType::ZwlrLayerShellV1(crate::wayland::zwlr_layer_shell_v1::ZwlrLayerShellV1),
			))),
			_ => Ok(None), // unrecognised interface, ignore for now
		}
	}
}
