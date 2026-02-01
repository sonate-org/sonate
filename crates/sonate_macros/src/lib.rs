extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

#[proc_macro_derive(MergeProperties, attributes(merge_by_method_call))]
pub fn derive_merge_properties(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let fields = if let Data::Struct(data) = &input.data {
        if let Fields::Named(fields) = &data.fields {
            &fields.named
        } else {
            unimplemented!()
        }
    } else {
        unimplemented!()
    };

    let merge_fields = fields.iter().map(|field| {
        let field_name = &field.ident;

        let merge_by_method_call = field
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("merge_by_method_call"));

        if merge_by_method_call {
            return quote! {
                self.#field_name.merge(&other.#field_name);
            };
        }

        let is_option = if let syn::Type::Path(ty) = &field.ty {
            ty.path.segments.len() == 1 && ty.path.segments[0].ident == "Option"
        } else {
            false
        };

        if is_option {
            quote! {
                if let Some(value) = &other.#field_name {
                    self.#field_name = Some(value.clone());
                }
            }
        } else {
            quote! {
                self.#field_name = other.#field_name.clone();
            }
        }
    });

    let expanded = quote! {
        impl #name {
            pub fn merge(&mut self, other: &Self) {
                #(#merge_fields)*
            }
        }
    };

    TokenStream::from(expanded)
}
