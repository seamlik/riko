//! Java support using JNI.

use crate::config::Config;
use crate::ir::Crate;
use crate::ir::Function;
use crate::ir::Module;
use crate::parse::MarshalingRule;
use crate::TargetCodeWriter;
use anyhow::Context;
use itertools::Itertools;
use quote::ToTokens;
use std::path::PathBuf;

const CLASS_FOR_MODULE: &str = "__riko_Module";

/// Writes JNI bindings.
pub struct JniWriter<'cfg> {
    config: &'cfg Config,
}

impl<'cfg> TargetCodeWriter<'cfg> for JniWriter<'cfg> {
    fn config(&self) -> &'cfg Config {
        self.config
    }

    fn target_name() -> &'static str {
        "jni"
    }

    fn write_all(&self, root: &Crate) -> anyhow::Result<()> {
        for module in root.modules.iter() {
            let mut file_path = std::iter::once(&root.name)
                .chain(module.path.iter())
                .collect::<PathBuf>();
            file_path.push(format!("{}.java", CLASS_FOR_MODULE));

            self.write_target_file(&file_path, &self.write_module(module, root)?)
                .with_context(|| {
                    format!("Failed to write to target file: {}", file_path.display())
                })?;
        }
        Ok(())
    }

    fn write_function(&self, function: &Function, _: &Module, _: &Crate) -> syn::Result<String> {
        let return_type_native = if function.output.is_none() {
            "void"
        } else {
            "byte[]"
        };
        let return_type_result = match &function.output {
            None => "void".into(),
            Some(MarshalingRule::Iterator(_)) => return_type(&MarshalingRule::I32),
            Some(inner) => return_type(inner),
        };
        let return_type_java = match &function.output {
            None => "void".into(),
            Some(inner) => return_type(inner),
        };
        let return_prefix = if function.output.is_none() {
            ""
        } else {
            "final byte[] returned ="
        };
        let return_block = if function.output.is_none() {
            "".into()
        } else {
            format!(
                r#"
                    final riko.Returned<{}> result = riko.Marshaler.fromBytes(returned);
                    return result.unwrap();
                "#,
                return_type_result
            )
        };

        let args = (0..(function.inputs.len()))
            .map(|idx| format!("riko.Marshaler.toBytes(arg_{})", idx))
            .join(", ");
        let params_native = (0..(function.inputs.len()))
            .map(|idx| format!("byte[] arg_{}", idx))
            .join(", ");
        let params_java = (0..(function.inputs.len()))
            .map(|idx| format!("final {} arg_{}", return_type_java, idx))
            .join(", ");

        Ok(format!(
            r#"
                private static native {return_type_native} {name_internal}( {params_native} );
                public static {return_type_java} {name_public}( {params_java} ) {{
                    {return_prefix} {name_internal}( {args} );
                    {return_block}
                }}
            "#,
            args = args,
            name_internal = &function.name_internal,
            name_public = &function.name_public,
            params_java = params_java,
            params_native = params_native,
            return_block = return_block,
            return_prefix = return_prefix,
            return_type_java = return_type_java,
            return_type_native = return_type_native
        ))
    }

    fn write_module(&self, module: &Module, root: &Crate) -> syn::Result<String> {
        let mut body = Vec::<String>::new();
        for function in module.functions.iter() {
            body.push(self.write_function(function, module, root)?);
        }

        let result_package = std::iter::once(&root.name)
            .chain(module.path.iter())
            .join(".");

        Ok(format!(
            r#"
                package {package};

                public final class {class} {{

                    private {class}() {{}}

                    {body}
                }}
            "#,
            body = body.as_slice().join("\n"),
            class = CLASS_FOR_MODULE,
            package = &result_package,
        ))
    }

    fn new(config: &'cfg Config) -> Self {
        Self { config }
    }
}

fn return_type(rule: &MarshalingRule) -> String {
    match rule {
        MarshalingRule::Bool => "java.lang.Boolean".into(),
        MarshalingRule::Bytes => "byte[]".into(),
        MarshalingRule::I8 => "java.lang.Byte".into(),
        MarshalingRule::I32 => "java.lang.Integer".into(),
        MarshalingRule::I64 => "java.lang.Long".into(),
        MarshalingRule::Iterator(inner) => format!("java.util.Iterator<{}>", inner),
        MarshalingRule::Serde(inner) => inner.to_token_stream().to_string().replace("::", "."),
        MarshalingRule::String => "java.lang.String".into(),
    }
}

mod tests {
    use super::*;

    #[test]
    fn module_nothing() {
        let expected = r#"
            package riko_sample.example;

            public final class __riko_Module {
                private __riko_Module() {}
            }
        "#;
        let ir = Crate {
            name: "riko_sample".into(),
            modules: vec![Module {
                functions: vec![],
                path: vec!["example".into()],
            }],
        };
        let config = Config::default();
        let writer = JniWriter::new(&config);
        assert_eq!(
            crate::normalize_source_code(expected),
            crate::normalize_source_code(&writer.write_module(&ir.modules[0], &ir).unwrap()),
        );
    }

    #[test]
    fn function_nothing() {
        let expected = r#"
            private static native void xxx( );
            public static void function( ) {
                xxx( );
            }
        "#;
        let ir = Crate {
            name: "riko_sample".into(),
            modules: vec![Module {
                functions: vec![Function {
                    name_public: "function".into(),
                    name_internal: "xxx".into(),
                    inputs: vec![],
                    output: None,
                }],
                path: vec!["example".into()],
            }],
        };
        let config = Config::default();
        let writer = JniWriter::new(&config);
        assert_eq!(
            crate::normalize_source_code(expected),
            crate::normalize_source_code(
                &writer
                    .write_function(&ir.modules[0].functions[0], &ir.modules[0], &ir)
                    .unwrap()
            ),
        );
    }

    #[test]
    fn function_simple() {
        let expected = r#"
            private static native byte[] xxx(
                byte[] arg_0,
                byte[] arg_1
            );
            public static java.lang.String function(
                final java.lang.String arg_0,
                final java.lang.String arg_1
            ) {
                final byte[] returned = xxx(
                    riko.Marshaler.toBytes(arg_0),
                    riko.Marshaler.toBytes(arg_1)
                );
                final riko.Returned<java.lang.String> result = riko.Marshaler.fromBytes(returned);
                return result.unwrap();
            }
        "#;
        let ir = Crate {
            name: "riko_sample".into(),
            modules: vec![Module {
                functions: vec![Function {
                    name_public: "function".into(),
                    name_internal: "xxx".into(),
                    inputs: vec![MarshalingRule::String, MarshalingRule::String],
                    output: Some(MarshalingRule::String),
                }],
                path: vec!["example".into()],
            }],
        };
        let config = Config::default();
        let writer = JniWriter::new(&config);
        assert_eq!(
            crate::normalize_source_code(expected),
            crate::normalize_source_code(
                &writer
                    .write_function(&ir.modules[0].functions[0], &ir.modules[0], &ir)
                    .unwrap()
            ),
        );
    }
}
