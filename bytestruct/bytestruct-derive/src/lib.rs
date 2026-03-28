extern crate proc_macro2;

use std::str::FromStr;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput};

#[proc_macro_derive(ByteStruct, attributes(big_endian, little_endian))]
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

		let generics = input
			.generics
			.params
			.iter()
			.map(|param| {
				quote! {#param}
			})
			.collect::<Vec<_>>();

		let generic_names = input
			.generics
			.params
			.iter()
			.map(|param| {
				let param = match param {
					syn::GenericParam::Type(ty) => &ty.ident,
					_ => panic!("Only type parameters are supported"),
				};
				quote! {#param}
			})
			.collect::<Vec<_>>();

		let gen = if little_endian || big_endian {
			quote! {
				impl<#(#generics)*> ::bytestruct::ReadFrom for #name<#(#generic_names)*> {
					fn read_from<R: ::std::io::Read>(source: &mut R) -> ::std::io::Result<Self> where Self: Sized {
						#(#set_endian_fields)*
						Ok(Self {
							#(#names),*
						})
					}
				}

				impl<#(#generics)*> ::bytestruct::WriteTo for #name<#(#generic_names)*> {
					fn write_to<W: ::std::io::Write>(&self, writer: &mut W) -> ::std::io::Result<()> {
						#(#write_fields)*
						Ok(())
					}
				}
			}
		} else {
			quote! {
				impl<#(#generics)*> ::bytestruct::ReadFromWithEndian for #name<#(#generic_names)*> {
					fn read_from_with_endian<R: ::std::io::Read>(source: &mut R, endian: ::bytestruct::Endian) -> ::std::io::Result<Self> where Self: Sized {
						#(#set_endian_fields)*
						Ok(Self {
							#(#names),*
						})
					}
				}

				impl<#(#generics)*> ::bytestruct::WriteToWithEndian for #name<#(#generic_names)*> {
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

		let ty = get_attribute_value("repr", &input.attrs).expect("missing repr attribute");

		for (i, variant) in data.variants.iter().enumerate() {
			let ident = &variant.ident;
			let discriminant = if let Some((_, v)) = &variant.discriminant {
				quote! {#v}
			} else {
				TokenStream::from_str(&i.to_string()).unwrap()
			};

			read_matches.push(quote! {
				#discriminant => #name::#ident,
			});

			write_matches.push(quote! {
				#name::#ident => #discriminant,
			});
		}

		quote! {
			impl ::bytestruct::ReadFromWithEndian for #name {
				fn read_from_with_endian<T: ::std::io::Read>(source: &mut T, endian: ::bytestruct::Endian) -> ::std::io::Result<Self> where Self: Sized {
					let discriminant = <#ty as ::bytestruct::ReadFromWithEndian>::read_from_with_endian(source, endian)?;
					let variant = match discriminant {
						#(#read_matches)*
						_ => return Err(::std::io::Error::new(::std::io::ErrorKind::InvalidData, format!("invalid discriminant for {}: {}", ::std::any::type_name::<#name>(), discriminant)))
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

	let generics = input
		.generics
		.params
		.iter()
		.map(|param| {
			quote! {#param}
		})
		.collect::<Vec<_>>();

	let generic_names = input
		.generics
		.params
		.iter()
		.map(|param| {
			let param = match param {
				syn::GenericParam::Type(ty) => &ty.ident,
				_ => panic!("Only type parameters are supported"),
			};
			quote! {#param}
		})
		.collect::<Vec<_>>();

	if let Data::Struct(data) = &input.data {
		let size = data.fields.iter().map(|field| {
			let ty = &field.ty;
			let name = field.ident.as_ref().unwrap();

			quote! {
				<#ty as ::bytestruct::Size>::size(&self.#name)
			}
		});

		let gen = quote! {
			impl<#(#generics)*> ::bytestruct::Size for #name<#(#generic_names)*> {
				fn size(&self) -> usize {
					0 #(+ #size)*
				}
			}
		};

		gen.into()
	} else if let Data::Enum(_) = &input.data {
		let repr = get_attribute_value("repr", &input.attrs).expect("missing repr attribute");
		let gen = quote! {
			impl<#(#generics)*> ::bytestruct::Size for #name<#(#generic_names)*> {
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

fn get_attribute_value(name: &str, attrs: &[syn::Attribute]) -> Option<syn::Expr> {
	let ty = attrs.iter().find(|attr| attr.path().is_ident(name))?;

	Some(ty.parse_args().unwrap())
}

#[proc_macro_derive(
	TLVValues,
	attributes(discriminant, end_type, type_type, length_type, no_length_type)
)]
pub fn derive_tlv_values(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let input = parse_macro_input!(input as DeriveInput);

	if let Data::Struct(data) = &input.data {
		let end_type = get_attribute_value("end_type", &input.attrs).expect("missing end type");
		let end_matches = quote! {
			if matches!(ty, #end_type) {
				return Ok(true)
			}
		};

		let no_length_matches = if let Some(no_length_type) = get_attribute_value("no_length_type", &input.attrs) {
			quote! {
				if matches!(ty, #no_length_type) {
					return Ok(false)
				}
			}
		} else {
			quote! {}
		};

		let set_matches: Vec<_> = data
			.fields
			.iter()
			.map(|f| {
				let name = f.ident.as_ref().expect("field name");
				let ty = extract_type_from_option(&f.ty).expect("option type");

				let matcher = get_attribute_value("discriminant", &f.attrs).expect("missing discriminant");

				quote! {
					#matcher => self.#name = Some(<#ty>::read_from_with_endian(&mut source, endian).map_err(|e| io::Error::new(e.kind(), format!("failed to read value for {}: {}", stringify!(#name), e)))?),
				}
			})
			.collect();

		let type_type = get_attribute_value("type_type", &input.attrs).expect("missing type type");
		let length_type = get_attribute_value("length_type", &input.attrs).expect("missing length type");

		let write_fields: Vec<_> = data
			.fields
			.iter()
			.map(|f| {
				let name = f.ident.as_ref().expect("field name");
				let matcher = get_attribute_value("discriminant", &f.attrs).expect("missing discriminant");
				quote! {
					if let Some(f) = &self.#name {
						let mut value = Vec::new();
						f.write_to_with_endian(&mut value, endian)?;
						#matcher.write_to_with_endian(&mut options_bytes, endian)?;
						(value.len() as #length_type).write_to_with_endian(&mut options_bytes, endian)?;
						options_bytes.write_all(&value)?;
					}
				}
			})
			.collect();

		let name = input.ident;
		quote! {
			impl ::bytestruct::TLVValues for #name {
				fn read_value<T: ::std::io::Read>(&mut self, source: &mut T, endian: ::bytestruct::Endian) -> ::std::io::Result<bool> {
					let ty = <#type_type>::read_from_with_endian(source, endian).map_err(|e| io::Error::new(e.kind(), format!("failed to read type: {}", e)))?;
					#end_matches
					#no_length_matches

					let len = <#length_type>::read_from_with_endian(source, endian).map_err(|e| io::Error::new(e.kind(), format!("failed to read length for {:?}: {}", ty, e)))?;
					let mut value_bytes = vec![0_u8; len as usize];
					source.read_exact(&mut value_bytes)?;

					let mut source = ::std::io::Cursor::new(&mut value_bytes);
					match ty {
						#(#set_matches)*
						_ => {},
					}

					Ok(false)
				}
			}

			impl ::bytestruct::ReadFromWithEndian for #name {
				fn read_from_with_endian<T: ::std::io::Read>(source: &mut T, endian: ::bytestruct::Endian) -> ::std::io::Result<Self> {
					use ::bytestruct::TLVValues;
					let mut new = Self::default();
					while !new.read_value(source, endian)? {}
					Ok(new)
				}
			}

			impl ::bytestruct::WriteToWithEndian for #name {
				fn write_to_with_endian<W: ::std::io::Write>(&self, writer: &mut W, endian: ::bytestruct::Endian) -> ::std::io::Result<()> {
					use ::std::io::Write;
					let mut options_bytes = Vec::new();
					#(#write_fields)*
					#end_type.write_to_with_endian(&mut options_bytes, endian)?;
					writer.write_all(&options_bytes)?;
					Ok(())
				}
			}
		}
		.into()
	} else {
		todo!("{:?}", input);
	}
}

fn extract_type_from_option(ty: &syn::Type) -> Option<&syn::Type> {
	use syn::{GenericArgument, Path, PathArguments, PathSegment};

	fn extract_type_path(ty: &syn::Type) -> Option<&Path> {
		match *ty {
			syn::Type::Path(ref typepath) if typepath.qself.is_none() => Some(&typepath.path),
			_ => None,
		}
	}

	// TODO store (with lazy static) the vec of string
	// TODO maybe optimization, reverse the order of segments
	fn extract_option_segment(path: &Path) -> Option<&PathSegment> {
		let idents_of_path = path.segments.iter().fold(String::new(), |mut acc, v| {
			acc.push_str(&v.ident.to_string());
			acc.push('|');
			acc
		});
		vec!["Option|", "std|option|Option|", "core|option|Option|"]
			.into_iter()
			.find(|s| idents_of_path == *s)
			.and_then(|_| path.segments.last())
	}

	extract_type_path(ty)
		.and_then(|path| extract_option_segment(path))
		.and_then(|path_seg| {
			let type_params = &path_seg.arguments;
			// It should have only on angle-bracketed param ("<String>"):
			match *type_params {
				PathArguments::AngleBracketed(ref params) => params.args.first(),
				_ => None,
			}
		})
		.and_then(|generic_arg| match *generic_arg {
			GenericArgument::Type(ref ty) => Some(ty),
			_ => None,
		})
}
