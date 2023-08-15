extern crate proc_macro;

mod clone_on_capture;

use syn::{parse::Parser, parse_macro_input, punctuated::Punctuated, ItemFn, Meta, Token};

use crate::proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn clone_on_capture(args: TokenStream, item: TokenStream) -> TokenStream {
    let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
    let args = parser.parse(args).expect("Failed to parse args");
    clone_on_capture::clone_on_capture_impl(args, parse_macro_input!(item as ItemFn))
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}
