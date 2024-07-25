#[macro_export]
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

        impl ::bytestruct::ReadFromWithEndian for $EnumName {
            fn read_from_with_endian<T: Read>(source: &mut T, endian: ::bytestruct::Endian) -> ::std::io::Result<Self> {
                let val = <$Type>::read_from_with_endian(source, endian)?;

                match val {
                    $(
                        $Value => Ok($EnumName::$Variant),
                    )+
                    _ => {
                        Err(::std::io::Error::new(::std::io::ErrorKind::InvalidData, format!("invalid value: {}", val)))
                    }
                }
            }
        }

        impl ::bytestruct::WriteToWithEndian for $EnumName {
            fn write_to_with_endian<W: ::std::io::Write>(&self, writer: &mut W, endian: ::bytestruct::Endian) -> ::std::io::Result<()> {
                let val = match self {
                    $(
                        $EnumName::$Variant => $Value,
                    )+
                };

                val.write_to_with_endian(writer, endian)
            }
        }
    }
}
