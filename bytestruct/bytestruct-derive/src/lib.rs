extern crate proc_macro2;

use std::str::FromStr;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Expr};

#[proc_macro_derive(ByteStruct, attributes(big_endian, little_endian, ty))]
pub fn derive_byte_struct(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let input = parse_macro_input!(input as DeriveInput);

	let little_endian = input.attrs.iter().any(|attr| attr.path().is_ident("little_endian"));
	let big_endian = input.attrs.iter().any(|attr| attr.path().is_ident("big_endian"));

	if little_endian && big_endian {
		panic!("Only one of little_endian or big_endian can be specified");
	}

	let name = input.ident;

	if let Data::Struct(data) = &input.data {
		let set_endian_fields = data.fields.iter().map(|field| {
			let name = field.ident.as_ref().unwrap();
			let ty = &field.ty;

			if little_endian {
				quote! {
					#name: <#ty as ::bytestruct::ReadFromWithEndian>::read_from_with_endian(source, ::bytestruct::Endian::Little)?
				}
			} else if big_endian {
				quote! {
					#name: <#ty as ::bytestruct::ReadFromWithEndian>::read_from_with_endian(source, ::bytestruct::Endian::Big)?
				}
			} else {
				quote! {
					#name: <#ty as ::bytestruct::ReadFrom>::read_from(source)?
				}
			}
		});

		let gen = quote! {
			impl ::bytestruct::ReadFrom for #name {
				fn read_from<T: ::std::io::Read>(source: &mut T) -> ::std::io::Result<Self> where Self: Sized {
					Ok(Self {
						#(#set_endian_fields),*
					})
				}
			}
		};

		gen.into()
	} else if let Data::Enum(data) = &input.data {
		let ty = match input.attrs.iter().find(|attr| attr.path().is_ident("ty")) {
			Some(repr) => repr,
			None => panic!("enums require a #[ty] field"),
		};

		let ty: Expr = ty.parse_args().unwrap();

		let mut matches = Vec::new();

		for (i, variant) in data.variants.iter().enumerate() {
			let discriminant = if let Some((_, v)) = &variant.discriminant {
				quote! {#v}
			} else {
				TokenStream::from_str(&i.to_string()).unwrap()
			};

			matches.push(quote! {
				#discriminant => #name::#variant,
			})
		}

		quote! {
			impl ::bytestruct::ReadFrom for #name {
				fn read_from<T: ::std::io::Read>(source: &mut T) -> ::std::io::Result<Self> where Self: Sized {
					let discriminant = <#ty as ::bytestruct::ReadFrom>::read_from(source)?;
					let variant = match discriminant {
						#(#matches)*
						_ => panic!("Invalid discriminant")
					};

					Ok(variant)
				}
			}

			impl ::bytestruct::ReadFromWithEndian for #name {
				fn read_from_with_endian<T: ::std::io::Read>(source: &mut T, endian: ::bytestruct::Endian) -> ::std::io::Result<Self> where Self: Sized {
					let discriminant = <#ty as ::bytestruct::ReadFromWithEndian>::read_from_with_endian(source, endian)?;
					let variant = match discriminant {
						#(#matches)*
						_ => panic!("Invalid discriminant")
					};

					Ok(variant)
				}
			}
		}
		.into()
	} else {
		panic!("Only structs are supported")
	}
}
