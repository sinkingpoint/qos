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
		let mut set_endian_fields = Vec::new();
		let mut write_fields = Vec::new();
		let mut prev_fields = Vec::new();
		for field in data.fields.iter() {
			let name = field.ident.as_ref().unwrap();
			let ty = &field.ty;

			let type_name = quote! { #ty }.to_string();

			let read_field = if type_name.starts_with("Padding <") || type_name.starts_with("bytestruct::Padding <") {
				let out = quote! {
					let #name = ::bytestruct::Padding::read(0 #(+ #prev_fields)*, source)?;
				};

				prev_fields.clear();
				out
			} else if little_endian {
				quote! {
					let #name = <#ty as ::bytestruct::ReadFromWithEndian>::read_from_with_endian(source, ::bytestruct::Endian::Little)?;
				}
			} else if big_endian {
				quote! {
					let #name = <#ty as ::bytestruct::ReadFromWithEndian>::read_from_with_endian(source, ::bytestruct::Endian::Big)?;
				}
			} else {
				quote! {
					let #name = <#ty as ::bytestruct::ReadFromWithEndian>::read_from_with_endian(source, endian)?;
				}
			};

			let write_field = if little_endian {
				quote! {
					<#ty as ::bytestruct::WriteToWithEndian>::write_to_with_endian(&self.#name, writer, ::bytestruct::Endian::Little)?;
				}
			} else if big_endian {
				quote! {
					<#ty as ::bytestruct::WriteToWithEndian>::write_to_with_endian(&self.#name, writer, ::bytestruct::Endian::Big)?;
				}
			} else {
				quote! {
					<#ty as ::bytestruct::WriteToWithEndian>::write_to_with_endian(&self.#name, writer, endian)?;
				}
			};

			prev_fields.push(quote! {<#ty as ::bytestruct::Size>::size(&#name)});

			set_endian_fields.push(read_field);
			write_fields.push(write_field);
		}

		let names = data.fields.iter().map(|field| {
			let name = field.ident.as_ref().unwrap();
			quote! {#name}
		});

		let gen = if little_endian || big_endian {
			quote! {
				impl ::bytestruct::ReadFrom for #name {
					fn read_from<T: ::std::io::Read>(source: &mut T) -> ::std::io::Result<Self> where Self: Sized {
						#(#set_endian_fields)*
						Ok(Self {
							#(#names),*
						})
					}
				}

				impl ::bytestruct::WriteTo for #name {
					fn write_to<W: ::std::io::Write>(&self, writer: &mut W) -> ::std::io::Result<()> {
						#(#write_fields)*
						Ok(())
					}
				}
			}
		} else {
			quote! {
				impl ::bytestruct::ReadFromWithEndian for #name {
					fn read_from_with_endian<T: ::std::io::Read>(source: &mut T, endian: ::bytestruct::Endian) -> ::std::io::Result<Self> where Self: Sized {
						#(#set_endian_fields)*
						Ok(Self {
							#(#names),*
						})
					}
				}

				impl ::bytestruct::WriteToWithEndian for #name {
					fn write_to_with_endian<W: ::std::io::Write>(&self, writer: &mut W, endian: ::bytestruct::Endian) -> ::std::io::Result<()> {
						#(#write_fields)*
						Ok(())
					}
				}
			}
		};

		gen.into()
	} else if let Data::Enum(data) = &input.data {
		let mut read_matches = Vec::new();
		let mut write_matches = Vec::new();

		let ty = get_repr(&input.attrs);

		for (i, variant) in data.variants.iter().enumerate() {
			let discriminant = if let Some((_, v)) = &variant.discriminant {
				quote! {#v}
			} else {
				TokenStream::from_str(&i.to_string()).unwrap()
			};

			read_matches.push(quote! {
				#discriminant => #name::#variant,
			});

			write_matches.push(quote! {
				#name::#variant => #discriminant,
			});
		}

		quote! {
			impl ::bytestruct::ReadFromWithEndian for #name {
				fn read_from_with_endian<T: ::std::io::Read>(source: &mut T, endian: ::bytestruct::Endian) -> ::std::io::Result<Self> where Self: Sized {
					let discriminant = <#ty as ::bytestruct::ReadFromWithEndian>::read_from_with_endian(source, endian)?;
					let variant = match discriminant {
						#(#read_matches)*
						_ => panic!("Invalid discriminant")
					};

					Ok(variant)
				}
			}

			impl ::bytestruct::WriteToWithEndian for #name {
				fn write_to_with_endian<W: ::std::io::Write>(&self, target: &mut W, endian: ::bytestruct::Endian) -> ::std::io::Result<()> {
					let discriminant = match self {
						#(#write_matches)*
					};

					<#ty as ::bytestruct::WriteToWithEndian>::write_to_with_endian(&discriminant, target, endian)
				}
			}
		}
		.into()
	} else {
		panic!("Only structs are supported")
	}
}

#[proc_macro_derive(Size)]
pub fn derive_size(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let input = parse_macro_input!(input as DeriveInput);

	let name = input.ident;

	if let Data::Struct(data) = &input.data {
		let size = data.fields.iter().map(|field| {
			let ty = &field.ty;
			let name = field.ident.as_ref().unwrap();

			quote! {
				<#ty as ::bytestruct::Size>::size(&self.#name)
			}
		});

		let gen = quote! {
			impl ::bytestruct::Size for #name {
				fn size(&self) -> usize {
					0 #(+ #size)*
				}
			}
		};

		gen.into()
	} else if let Data::Enum(_) = &input.data {
		let repr = get_repr(&input.attrs);
		let gen = quote! {
			impl ::bytestruct::Size for #name {
				fn size(&self) -> usize {
					<#repr as ::bytestruct::Size>::size(&0)
				}
			}
		};

		gen.into()
	} else {
		panic!("Only structs are supported")
	}
}

fn get_repr(attrs: &[syn::Attribute]) -> proc_macro2::Ident {
	let ty = match attrs.iter().find(|attr| attr.path().is_ident("repr")) {
		Some(repr) => repr,
		None => panic!("enums require a #[repr] field"),
	};

	if let Ok(Expr::Path(path)) = ty.parse_args() {
		if let Some(ident) = path.path.get_ident() {
			ident.clone()
		} else {
			panic!("Only simple reprs are supported");
		}
	} else {
		panic!("Only u8 is supported as repr for enums")
	}
}
