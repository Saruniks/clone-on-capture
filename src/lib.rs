extern crate proc_macro;

mod clone_on_capture;

use syn::{parse_macro_input, AttributeArgs, ItemFn, NestedMeta};

use crate::proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn clone_on_capture(args: TokenStream, item: TokenStream) -> TokenStream {
    let args: Vec<NestedMeta> = parse_macro_input!(args as AttributeArgs);
    clone_on_capture::clone_on_capture_impl(args, parse_macro_input!(item as ItemFn))
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}
