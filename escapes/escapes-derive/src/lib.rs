extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Lit};

/// Generates a struct that implements the `EscapeSequence` trait and a `Display` implementation for it.
#[proc_macro_derive(EscapeSequence, attributes(default, escape))]
pub fn derive_escape_sequence(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as DeriveInput);

	let name = input.ident;
	let num_args;
	let escape;
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

	let joined = if num_args > 1 {
		quote! { [#(self.#idxs),*].map(|i| format!("{}", i)).join(";") }
	} else {
		quote! { format!("{}", self.0) }
	};

	let gen = quote! {
		impl EscapeSequence for #name {
			fn parse(params: &[u8]) -> Result<Self, AnsiParserError> {
				let defaults: &[u8] = &[#(#defaults),*];
				if params.len() != #num_args && defaults.len() == 0 {
					return Err(AnsiParserError::NumParams(#num_args, 0));
				} else if params.len() == 0 {
					return Ok(Self(#(#defaults),*));
				} else if params.len() != #num_args {
					return Err(AnsiParserError::NumParams(#num_args, params.len()));
				} else {
					return Ok(Self(#(params[#idxs]),*));
				}
			}
		}

		impl std::fmt::Display for #name {
			fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
				let joined = #joined;
				write!(f, "{}{}{}{}", ESC, CSI, joined, #escape)
			}
		}
	};

	gen.into()
}
