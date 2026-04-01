#[macro_export]
macro_rules! wayland_interface {
    ($subsystem:ty, $enum_name:ident { $($opcode:literal => $variant:ident ($ty:ty)),* $(,)? }) => {
        pub enum $enum_name {
            $($variant($ty),)*
        }

        impl $crate::wayland::types::CommandRegistry for $enum_name {
            fn parse(command: $crate::wayland::types::WaylandPacket) -> Option<Self> {
                match command.opcode {
                    $($opcode => Some(Self::$variant(
                        <$ty as ::bytestruct::ReadFromWithEndian>::read_from_with_endian(
                            &mut ::std::io::Cursor::new(command.payload),
                            ::bytestruct::Endian::Little,
                        ).ok()?
                    )),)*
                    _ => None,
                }
            }
        }

        impl $crate::wayland::types::Command<$subsystem> for $enum_name {
            fn handle(
                &self,
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
