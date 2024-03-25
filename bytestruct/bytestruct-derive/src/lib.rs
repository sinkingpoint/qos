extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput};

#[proc_macro_derive(ByteStruct, attributes(big_endian, little_endian))]
pub fn derive_byte_struct(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as DeriveInput);

	let little_endian = input.attrs.iter().any(|attr| attr.path().is_ident("little_endian"));
	let big_endian = input.attrs.iter().any(|attr| attr.path().is_ident("big_endian"));

	if little_endian && big_endian {
		panic!("Only one of little_endian or big_endian can be specified");
	}

	if !little_endian && !big_endian {
		panic!("One of little_endian or big_endian must be specified");
	}

	let name = input.ident;

	if let Data::Struct(data) = &input.data {
		let endian_fields = data.fields.iter().map(|field| {
			let name = field.ident.as_ref().unwrap();
			let ty = &field.ty;

			quote! {
				#name: <#ty as ::bytestruct::ReadFromWithEndian>::read_from_with_endian(source, endian)?
			}
		});

		let set_endian_fields = data.fields.iter().map(|field| {
			let name = field.ident.as_ref().unwrap();
			let ty = &field.ty;

			if little_endian {
				quote! {
					#name: <#ty as ::bytestruct::ReadFromWithEndian>::read_from_with_endian(source, ::bytestruct::Endian::Little)?
				}
			} else {
				quote! {
					#name: <#ty as ::bytestruct::ReadFromWithEndian>::read_from_with_endian(source, ::bytestruct::Endian::Big)?
				}
			}
		});

		let gen = quote! {
			impl ::bytestruct::ReadFromWithEndian for #name {
				fn read_from_with_endian<T: ::std::io::Read>(source: &mut T, endian: ::bytestruct::Endian) -> ::std::io::Result<Self> where Self: Sized {
					Ok(Self {
						#(#endian_fields),*
					})
				}
			}

			impl ::bytestruct::ReadFrom for #name {
				fn read_from<T: ::std::io::Read>(source: &mut T) -> ::std::io::Result<Self> where Self: Sized {
					Ok(Self {
						#(#set_endian_fields),*
					})
				}
			}
		};

		gen.into()
	} else {
		panic!("Only structs are supported")
	}
}
