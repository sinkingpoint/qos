#[macro_export]
macro_rules! wayland_interface {
    ($subsystem:ty, $enum_name:ident { $($opcode:literal => $variant:ident ($ty:ty)),* $(,)? }) => {
        pub enum $enum_name {
            $($variant($ty),)*
        }

        impl $crate::wayland::types::CommandRegistry for $enum_name {
            fn parse(opcode: u16, args: &[u8]) -> Option<Self> {
                match opcode {
                    $($opcode => Some(Self::$variant(
                        <$ty as ::bytestruct::ReadFromWithEndian>::read_from_with_endian(
                            &mut ::std::io::Cursor::new(args),
                            ::bytestruct::Endian::Little,
                        ).ok()?
                    )),)*
                    _ => None,
                }
            }
        }

        impl $crate::wayland::types::Command<$subsystem> for $enum_name {
            fn handle(&self, client: &mut $crate::wayland::types::Client, subsystem: &mut $subsystem) -> $crate::wayland::types::WaylandResult<()> {
                match self {
                    $(Self::$variant(cmd) => cmd.handle(client, subsystem),)*
                }
            }
        }
    };
}
