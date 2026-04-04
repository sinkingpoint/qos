#[macro_export]
macro_rules! wayland_interface {
    ($subsystem:ty, $enum_name:ident { $($opcode:pat => $variant:ident ($ty:ty)),* $(,)? }) => {
        pub enum $enum_name {
            $($variant($ty),)*
        }

        impl $crate::wayland::types::CommandRegistry for $enum_name {
            fn parse(command: $crate::wayland::types::WaylandPacket, fds: &mut ::std::collections::VecDeque<::std::os::fd::OwnedFd>) -> Option<Self> {
                match command.opcode {
                    $($opcode => Some(Self::$variant(
                        <$ty as $crate::wayland::types::FromPacket>::from_packet(command, fds)?
                    )),)*
                    _ => None,
                }
            }
        }

        impl $crate::wayland::types::Command<$subsystem> for $enum_name {
            fn handle(
                self,
                connection: &::std::sync::Arc<::std::os::unix::net::UnixStream>,
                subsystem: &mut $subsystem,
            ) -> $crate::wayland::types::WaylandResult<::std::option::Option<$crate::wayland::types::ClientEffect>> {
                match self {
                    $(Self::$variant(cmd) => cmd.handle(connection, subsystem),)*
                }
            }
        }
    };
}

macro_rules! subsystem_type {
	($($variant:ident($ty:ty)),* $(,)?) => {
		pub enum SubsystemType {
			$($variant($ty),)*
		}

		impl SubsystemType {
			pub fn name(&self) -> &'static str {
				match self {
					$(Self::$variant(_) => <$ty>::NAME,)*
				}
			}

			pub fn version(&self) -> u32 {
				match self {
					$(Self::$variant(_) => <$ty>::VERSION,)*
				}
			}

			fn handle_command(
				&mut self,
				connection: &Arc<UnixStream>,
				command: WaylandPacket,
        fds: &mut VecDeque<OwnedFd>,
			) -> WaylandResult<Option<ClientEffect>> {
				match self {
					$(SubsystemType::$variant(inner) => {
						if let Some(cmd) = inner.parse_command(command, fds) {
							cmd.handle(connection, inner)
						} else {
							Ok(None)
						}
					})*
				}
			}
		}
	};
}
