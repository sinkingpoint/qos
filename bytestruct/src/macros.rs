#[macro_export]
/// int_enum provides a macro to derive an enum from a set of numbers that can be read/write.
macro_rules! int_enum {
    (
        $(#[$outer:meta])*
        $v:vis enum $EnumName:ident : $Type:ty{
            $(
                $(#[$inner:ident $($args:tt)*])*
                $Variant:ident = $Value:expr,
            )+
        }
    ) => {
        $(#[$outer])*
        $v enum $EnumName {
            $(
                $(#[$inner $($args)*])*
                $Variant,
            )+
        }

        impl ::std::convert::TryFrom<$Type> for $EnumName {
            type Error = String;

            // Required method
            fn try_from(value: $Type) -> Result<Self, String> {
                match value {
                    $(
                        $Value => Ok($EnumName::$Variant),
                    )+
                    i => Err(format!("{:?} is not a valid {}", value, stringify!($EnumName)))
                }
            }
        }

        impl From<&$EnumName> for $Type {
            fn from(e: &$EnumName) -> $Type {
                match e {
                    $(
                        $EnumName::$Variant => $Value,
                    )+
                }
            }
        }

        impl ::bytestruct::Size for $EnumName {
            fn size(&self) -> usize {
                let val: $Type = self.into();
                val.size()
            }
        }

        impl ::bytestruct::ReadFromWithEndian for $EnumName {
            fn read_from_with_endian<T: ::std::io::Read>(source: &mut T, endian: ::bytestruct::Endian) -> ::std::io::Result<Self> {
                let val = <$Type>::read_from_with_endian(source, endian)?;

                match val {
                    $(
                        $Value => Ok($EnumName::$Variant),
                    )+
                    _ => {
                        Err(::std::io::Error::new(::std::io::ErrorKind::InvalidData, format!("invalid value for {}: {}", stringify!($EnumName), val)))
                    }
                }
            }
        }

        impl ::bytestruct::WriteToWithEndian for $EnumName {
            fn write_to_with_endian<W: ::std::io::Write>(&self, writer: &mut W, endian: ::bytestruct::Endian) -> ::std::io::Result<()> {
                let val: $Type = match self {
                    $(
                        $EnumName::$Variant => $Value,
                    )+
                };

                val.write_to_with_endian(writer, endian)
            }
        }
    }
}
