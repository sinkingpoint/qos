extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Lit};

/// Generates a struct that implements the `EscapeSequence` trait and a `Display` implementation for it.
#[proc_macro_derive(EscapeSequence, attributes(default, escape, modifier))]
pub fn derive_escape_sequence(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as DeriveInput);

	let name = input.ident;
	let num_args;
	let escape;
	let mut modifier = None;
	let mut idxs = vec![];
	let mut defaults: Vec<u8> = vec![];

	if let Some(attr) = input.attrs.iter().find(|attr| attr.path().is_ident("escape")) {
		if let Lit::Char(c) = attr.parse_args().unwrap() {
			escape = c.value();
		} else {
			panic!("Escape attribute must be a single character");
		}
	} else {
		panic!("Missing escape attribute");
	}

	if let Some(attr) = input.attrs.iter().find(|attr| attr.path().is_ident("modifier")) {
		if let Lit::Str(c) = attr.parse_args().unwrap() {
			modifier = Some(c.value());
		} else {
			panic!("Modifier attribute must be a str");
		}
	}

	let modifier_token = match modifier {
		Some(c) => quote! { #c },
		None => quote! { "" },
	};

	if let Data::Struct(data) = &input.data {
		num_args = data.fields.len();
		for field in data.fields.iter() {
			for attr in field.attrs.iter() {
				if attr.path().is_ident("default") {
					if let Lit::Int(default) = attr.parse_args().unwrap() {
						defaults.push(default.base10_parse().unwrap());
					} else {
						panic!("Default attribute must be an integer");
					}
				}

				idxs.push(syn::Index::from(idxs.len()));
			}
		}

		if !defaults.is_empty() && defaults.len() != num_args {
			panic!("All fields must have a default value");
		}
	} else {
		panic!("Only structs are supported");
	}

	let joined = match num_args {
		0 => quote! { String::new() },
		1 => quote! { format!("{}", self.0) },
		_ => quote! { [#(self.#idxs),*].map(|i| format!("{}", i)).join(";") },
	};

	let parse_impl = if num_args == 0 {
		quote! {
			return Ok(Self);
		}
	} else if defaults.is_empty() {
		quote! {
			if params.len() != #num_args {
				return Err(AnsiParserError::NumParams(#num_args, params.len()));
			} else {
				return Ok(Self(#(params[#idxs]),*));
			}
		}
	} else {
		quote! {
			if params.len() != #num_args && params.len() != 0 {
				return Err(AnsiParserError::NumParams(#num_args, params.len()));
			} else if params.len() == 0 {
				return Ok(Self(#(#defaults),*));
			} else {
				return Ok(Self(#(params[#idxs]),*));
			}
		}
	};

	let gen = quote! {
		impl EscapeSequence for #name {
			fn parse(params: &[u8]) -> Result<Self, AnsiParserError> {
				#parse_impl
			}
		}

		impl std::fmt::Display for #name {
			fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
				let joined = #joined;
				write!(f, "{}{}{}{}{}", ESC, CSI, #modifier_token, joined, #escape)
			}
		}
	};

	gen.into()
}
