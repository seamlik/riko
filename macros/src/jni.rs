mod expand;

use crate::config::Config;
use crate::parse::Fun;
use proc_macro::TokenStream;
use syn::ItemFn;
use syn::ItemStruct;

pub struct Bindgen<'cfg> {
    config: &'cfg Config,
}

impl<'cfg> crate::Bindgen<'cfg> for Bindgen<'cfg> {
    fn fun(&self, item: &ItemFn, args: &Fun) -> TokenStream {
        expand::fun(&item.sig, args).into()
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
