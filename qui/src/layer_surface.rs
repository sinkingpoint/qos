use std::{
	cell::RefCell,
	io::{self, Cursor},
	rc::Rc,
};

use bytestruct::{Endian, ReadFromWithEndian};
use wayland::{
	buffer::ReleaseEvent,
	compositor::CreateSurfaceRequest,
	keyboard::KeyboardEvent,
	pointer::PointerEvent,
	surface::{AttachRequest, CommitRequest, DamageRequest, FrameCallbackEvent, FrameRequest},
	types::{WaylandEncodedString, WaylandPayload},
	zwlr_layer_shell_v1::{
		AckConfigureRequest, Anchor, ConfigureEvent, GetLayerSurfaceRequest, Layer, SetAnchorRequest, SetSizeRequest,
	},
};

use crate::{
	AppEvent,
	buffer::DoubleBuffer,
	canvas::Canvas,
	context::{ContextEvent, WaylandContext},
};

pub struct LayerSurface {
	context: Rc<RefCell<WaylandContext>>,
	surface_id: u32,
	buffers: DoubleBuffer,
	frame_callback_id: u32,
	layer_surface_id: u32,
	last_interaction_serial: Option<u32>,
	last_pointer_position: Option<(i32, i32)>,
	awaiting_frame: bool,
	awaiting_release: bool,
}

impl LayerSurface {
	pub fn new(width: i32, height: i32, layer: Layer, anchor: Anchor) -> io::Result<Self> {
		let ctx = WaylandContext::connect()?;
		Self::new_with_context(ctx, width, height, layer, anchor)
	}

	pub fn new_with_context(
		ctx: Rc<RefCell<WaylandContext>>,
		mut width: i32,
		mut height: i32,
		layer: Layer,
		anchor: Anchor,
	) -> io::Result<Self> {
		let mut context = ctx.borrow_mut();
		let surface_id = context.next_object_id.next();
		CreateSurfaceRequest { new_id: surface_id }
			.write_as_packet(context.globals.compositor.unwrap(), &context.conn.stream)?;

		let layer_surface_id = context.next_object_id.next();
		GetLayerSurfaceRequest {
			new_id: layer_surface_id,
			wl_surface_id: surface_id,
			output_id: 0, // TODO: support multiple outputs
			layer,
			namespace: WaylandEncodedString("qui".into()),
		}
		.write_as_packet(context.globals.zwlr_layer_shell_v1.unwrap(), &context.conn.stream)?;

		SetSizeRequest { width, height }.write_as_packet(layer_surface_id, &context.conn.stream)?;

		SetAnchorRequest { anchor }.write_as_packet(layer_surface_id, &context.conn.stream)?;

		// Initial commit with no buffer: required by xdg_shell to signal that
		// setup is complete and prompt the compositor to send xdg_surface.configure.
		CommitRequest.write_as_packet(surface_id, &context.conn.stream)?;

		// Wait for xdg_surface.configure, then ack
		loop {
			let packet = context.conn.recv_packet()?;
			context.conn.drain_fds(); // discard any fds (e.g. keyboard keymap) arriving before poll loop
			if packet.object_id == layer_surface_id && packet.opcode == ConfigureEvent::OPCODE {
				let event = ConfigureEvent::read_from_with_endian(&mut Cursor::new(&packet.payload), Endian::Little)?;
				AckConfigureRequest { serial: event.serial }.write_as_packet(layer_surface_id, &context.conn.stream)?;
				if event.width > 0 {
					width = event.width as i32;
				}
				if event.height > 0 {
					height = event.height as i32;
				}
				break;
			}
		}

		let mut buffers = DoubleBuffer::new(&mut context, width, height)?;
		AttachRequest {
			buffer_id: buffers.id(),
			x: 0,
			y: 0,
		}
		.write_as_packet(surface_id, &context.conn.stream)?;
		CommitRequest.write_as_packet(surface_id, &context.conn.stream)?;
		buffers.swap();
		let frame_callback_id = context.next_object_id.next();
		drop(context); // drop mutable borrow before returning
		Ok(Self {
			surface_id,
			buffers,
			frame_callback_id,
			last_interaction_serial: None,
			last_pointer_position: None,
			context: ctx,
			layer_surface_id,
			// We already committed an initial frame during setup, so wait for both
			// callback + release before advertising RenderReady.
			awaiting_frame: true,
			awaiting_release: true,
		})
	}

	pub fn poll(&mut self) -> io::Result<AppEvent> {
		let buffer_ids = self.buffers.all_ids();
		loop {
			let (object_id, event) = self.context.borrow_mut().poll(&[
				self.surface_id,
				self.layer_surface_id,
				self.frame_callback_id,
				buffer_ids[0],
				buffer_ids[1],
			])?;
			if let Some(app_event) = self.interpret_event(object_id, event)? {
				return Ok(app_event);
			}
		}
	}

	pub fn try_poll(&mut self) -> io::Result<Option<AppEvent>> {
		let buffer_ids = self.buffers.all_ids();
		loop {
			let maybe = self.context.borrow_mut().try_poll(&[
				self.surface_id,
				self.layer_surface_id,
				self.frame_callback_id,
				buffer_ids[0],
				buffer_ids[1],
			])?;
			match maybe {
				None => return Ok(None),
				Some((object_id, event)) => {
					if let Some(app_event) = self.interpret_event(object_id, event)? {
						return Ok(Some(app_event));
					}
				}
			}
		}
	}

	fn interpret_event(&mut self, object_id: u32, event: ContextEvent) -> io::Result<Option<AppEvent>> {
		if let ContextEvent::Pointer(pointer_event) = event {
			match pointer_event {
				PointerEvent::Move(event) => {
					self.last_pointer_position = Some((event.x / 256, event.y / 256));
					return Ok(Some(AppEvent::PointerMotion {
						x: event.x / 256,
						y: event.y / 256,
					}));
				}
				PointerEvent::Button(event) => {
					self.last_interaction_serial = Some(event.serial);
					return Ok(Some(AppEvent::PointerButton {
						button: event.button,
						pressed: event.state != 0,
						x: self.last_pointer_position.map(|(x, _)| x).unwrap_or(0),
						y: self.last_pointer_position.map(|(_, y)| y).unwrap_or(0),
					}));
				}
				_ => {}
			}
		} else if let ContextEvent::Keyboard(keyboard_event) = event {
			if let KeyboardEvent::Key(event) = keyboard_event {
				self.last_interaction_serial = Some(event.serial);
				return Ok(Some(AppEvent::Keyboard {
					keycode: event.key,
					pressed: event.state != 0,
				}));
			}
		} else if matches!(event, ContextEvent::Unknown { opcode: ConfigureEvent::OPCODE, .. } if object_id == self.layer_surface_id)
			&& let ContextEvent::Unknown { payload, .. } = event
		{
			let configure = ConfigureEvent::read_from_with_endian(&mut Cursor::new(&payload), Endian::Little)?;
			AckConfigureRequest {
				serial: configure.serial,
			}
			.write_as_packet(self.layer_surface_id, &self.context.borrow().conn.stream)?;
		} else if matches!(event, ContextEvent::Unknown { opcode: FrameCallbackEvent::OPCODE, .. } if object_id == self.frame_callback_id)
		{
			if self.awaiting_frame {
				self.awaiting_frame = false;
			}
			if !self.awaiting_frame && !self.awaiting_release {
				return Ok(Some(AppEvent::RenderReady));
			}
		} else if matches!(event, ContextEvent::Unknown { opcode: ReleaseEvent::OPCODE, .. } if self.buffers.all_ids().contains(&object_id))
		{
			self.buffers.release(object_id);
			if self.awaiting_release {
				self.awaiting_release = false;
			}
			if !self.awaiting_frame && !self.awaiting_release {
				return Ok(Some(AppEvent::RenderReady));
			}
		}
		Ok(None)
	}

	pub fn commit_frame(&mut self) -> io::Result<()> {
		FrameRequest {
			callback_id: self.frame_callback_id,
		}
		.write_as_packet(self.surface_id, &self.context.borrow().conn.stream)?;
		AttachRequest {
			buffer_id: self.buffers.id(),
			x: 0,
			y: 0,
		}
		.write_as_packet(self.surface_id, &self.context.borrow().conn.stream)?;
		for damage in self.buffers.current_buffer().drain_damage() {
			DamageRequest {
				x: damage.0,
				y: damage.1,
				width: damage.2,
				height: damage.3,
			}
			.write_as_packet(self.surface_id, &self.context.borrow().conn.stream)?;
		}
		CommitRequest.write_as_packet(self.surface_id, &self.context.borrow().conn.stream)?;
		self.buffers.swap();
		self.awaiting_frame = true;
		self.awaiting_release = true;

		Ok(())
	}

	pub fn canvas(&mut self) -> Option<Canvas<'_>> {
		self.buffers.canvas()
	}
}
