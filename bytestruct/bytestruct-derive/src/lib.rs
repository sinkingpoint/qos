extern crate proc_macro;

use quote::quote;
use proc_macro::TokenStream;
use syn::{parse_macro_input, Data, DeriveInput};

#[proc_macro_derive(ByteStruct, attributes(default, escape))]
pub fn derive_byte_struct(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;

    if let Data::Struct(data) = &input.data {
        let fields = data.fields.iter().map(|field| {
            let name = field.ident.as_ref().unwrap();
            let ty = &field.ty;

            quote! {
                #name: <#ty as ::bytestruct::ReadFrom>::read_from(source)?
            }
        });

        let gen = quote! {
            impl ::bytestruct::ReadFrom for #name {
                fn read_from<T: ::std::io::Read>(source: &mut T) -> ::std::io::Result<Self> where Self: Sized {
                    Ok(Self {
                        #(#fields),*
                    })
                }
            }
        };

        gen.into()
    } else {
        panic!("Only structs are supported")
    }
}
