mod codegen;
mod expand;

use crate::config::Config;
use crate::parse::Fun;
use crate::parse::MarshalingRule;
use proc_macro::TokenStream;
use syn::ItemFn;
use syn::ItemStruct;

pub struct Bindgen<'cfg> {
    config: &'cfg Config,
}

impl<'cfg> crate::Bindgen<'cfg> for Bindgen<'cfg> {
    fn fun(&self, item: &mut ItemFn, args: &Fun) -> TokenStream {
        match MarshalingRule::parse(item.sig.inputs.iter_mut()) {
            Ok(input_rules) => {
                if crate::config::env_riko_enabled() {
                    codegen::fun(args, &input_rules);
                }
                expand::fun(&item.sig, args).into()
            }
            Err(err) => err.to_compile_error().into(),
        }
    }

    fn new(config: &'cfg Config) -> Self {
        Self { config }
    }

    fn config(&self) -> &'cfg Config {
        self.config
    }

    fn heaped(&self, item: &ItemStruct) -> TokenStream {
        expand::heaped(&item.ident).into()
    }
}
