extern crate proc_macro;
use std::iter;

use quote::quote;
use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput, Data, Lit};

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
            for (idx, attr) in field.attrs.iter().enumerate() {
                if attr.path().is_ident("default") {
                    if let Lit::Int(default) = attr.parse_args().unwrap() {
                        defaults.push(default.base10_parse().unwrap());
                    } else {
                        panic!("Default attribute must be an integer");
                    }
                }

                idxs.push(syn::Index::from(idx));
            }
        }

        if defaults.len() > 0 && defaults.len() != num_args {
            panic!("All fields must have a default value");
        }
    } else {
        panic!("Only structs are supported");
    }

    let output_string = iter::repeat("{}").take(3 + num_args).collect::<String>();

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
                write!(f, #output_string, ESC, CSI, #(self.#idxs),*, #escape)
            }
        }
    };

    gen.into()
}
