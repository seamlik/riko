//! Sample IRs for testing.

use super::*;

pub(crate) fn empty_module() -> Crate {
    Crate {
        name: "riko_sample".into(),
        modules: vec![Module {
            functions: vec![],
            path: vec!["example".into()],
            cfg: Default::default(),
        }],
    }
}

pub(crate) fn simple_function() -> Crate {
    Crate {
        name: "riko_sample".into(),
        modules: vec![Module {
            functions: vec![Function {
                name: "function".into(),
                pubname: "function".into(),
                inputs: vec![
                    Input {
                        rule: MarshalingRule::I32,
                        borrow: true,
                        unwrapped_type: syn::parse_quote! { i32 },
                    },
                    Input {
                        rule: MarshalingRule::I64,
                        borrow: false,
                        unwrapped_type: syn::parse_quote! { i64 },
                    },
                ],
                output: Output {
                    future: false,
                    rule: MarshalingRule::String,
                    unwrapped_type: syn::parse_quote! { String },
                },
                cfg: Default::default(),
            }],
            path: vec!["example".into()],
            cfg: Default::default(),
        }],
    }
}

pub(crate) fn returning_object() -> Crate {
    Crate {
        name: "riko_sample".into(),
        modules: vec![Module {
            functions: vec![Function {
                name: "function".into(),
                pubname: "function".into(),
                inputs: vec![],
                output: Output {
                    future: false,
                    rule: MarshalingRule::Object,
                    unwrapped_type: syn::parse_quote! { crate::Love },
                },
                cfg: vec![],
            }],
            path: vec!["example".into()],
            cfg: vec![],
        }],
    }
}

pub(crate) fn function_with_nothing() -> Crate {
    Crate {
        name: "riko_sample".into(),
        modules: vec![Module {
            functions: vec![Function {
                name: "function".into(),
                pubname: "function".into(),
                inputs: vec![],
                output: Output {
                    future: false,
                    rule: MarshalingRule::Unit,
                    unwrapped_type: syn::Path {
                        leading_colon: None,
                        segments: Default::default(),
                    },
                },
                cfg: vec![],
            }],
            path: vec!["example".into()],
            cfg: vec![],
        }],
    }
}

/// `riko_sample::example::function() -> Future<Output = Result<String, Error>>`
pub(crate) fn function_async() -> Crate {
    Crate {
        name: "riko_sample".into(),
        modules: vec![Module {
            functions: vec![Function {
                name: "function".into(),
                pubname: "function".into(),
                inputs: vec![],
                output: Output {
                    future: true,
                    rule: MarshalingRule::String,
                    unwrapped_type: syn::parse_quote! { String },
                },
                cfg: vec![],
            }],
            path: vec!["example".into()],
            cfg: vec![],
        }],
    }
}
