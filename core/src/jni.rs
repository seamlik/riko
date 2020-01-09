//! Java support using JNI.

use crate::ir::Crate;
use crate::ir::Function;
use crate::ir::Module;
use crate::parse::MarshalingRule;
use crate::ErrorSource;
use crate::TargetCodeWriter;
use itertools::Itertools;
use quote::ToTokens;
use std::path::Path;

const CLASS_FOR_MODULE: &str = "__riko_Module";
pub const PACKAGE_FOR_BRIDGE: &str = "__riko_bridge";

/// Writes JNI bindings.
pub struct JniWriter;

impl TargetCodeWriter for JniWriter {
    fn write_all(&self, root: &Crate, output_directory: &Path) -> Result<(), crate::Error> {
        // Modules
        for module in root.modules.iter() {
            let mut file_path = output_directory.to_owned();
            file_path.push(&root.name);
            file_path.extend(module.path.iter());
            file_path.push(format!("{}.java", CLASS_FOR_MODULE));

            self.write_target_file(&file_path, &self.write_module(module, root))
                .map_err(|err| crate::Error {
                    file: file_path,
                    source: ErrorSource::Write(err),
                })?;
        }

        // Bridges
        let mut file_path_bridge = output_directory.to_owned();
        file_path_bridge.push(&root.name);
        file_path_bridge.push(PACKAGE_FOR_BRIDGE);
        file_path_bridge.push(format!("{}.java", CLASS_FOR_MODULE));
        self.write_target_file(&file_path_bridge, &self.write_bridges(root))
            .map_err(|err| crate::Error {
                file: file_path_bridge,
                source: ErrorSource::Write(err),
            })?;
        Ok(())
    }

    fn write_function(&self, function: &Function, _: &Module, root: &Crate) -> String {
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
        let params = (0..(function.inputs.len()))
            .map(|idx| format!("final {} arg_{}", return_type_java, idx))
            .join(", ");

        format!(
            r#"
                public static {return_type_java} {name_public}( {params} ) {{
                    {return_prefix} {crate_name}.{package_bridge}.{module}.{name_bridge}( {args} );
                    {return_block}
                }}
            "#,
            args = args,
            crate_name = root.name,
            module = CLASS_FOR_MODULE,
            name_bridge = &function.name_bridge,
            name_public = &function.name_public,
            package_bridge = PACKAGE_FOR_BRIDGE,
            params = params,
            return_block = return_block,
            return_prefix = return_prefix,
            return_type_java = return_type_java,
        )
    }

    fn write_bridges(&self, root: &Crate) -> String {
        let body = root
            .bridges
            .iter()
            .map(|b| {
                let return_type = if b.output { "byte[]" } else { "void" };
                let params = (0..b.input)
                    .map(|idx| format!("byte[] arg_{}", idx))
                    .join(", ");
                // Put `\n` at the beginning so the output can be sightly prettier.
                format!(
                    "\npublic static native {return_type} {name}( {params} );",
                    name = &b.name,
                    params = params,
                    return_type = return_type,
                )
            })
            .join("");

        format!(
            r#"
                package {crate_name}.{package};

                public final class {class} {{
                    private {class}() {{}}
                    {body}
                }}
            "#,
            body = body,
            class = CLASS_FOR_MODULE,
            crate_name = &root.name,
            package = PACKAGE_FOR_BRIDGE,
        )
    }

    fn write_module(&self, module: &Module, root: &Crate) -> String {
        let mut body = Vec::<String>::new();
        for function in module.functions.iter() {
            body.push(self.write_function(function, module, root));
        }

        let result_package = std::iter::once(&root.name)
            .chain(module.path.iter())
            .join(".");

        format!(
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
        )
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
    use crate::ir::*;

    #[test]
    fn module_nothing() {
        let expected = r#"
            package riko_sample.example;

            public final class __riko_Module {
                private __riko_Module() {}
            }
        "#;
        let ir = Crate {
            bridges: vec![],
            name: "riko_sample".into(),
            modules: vec![Module {
                functions: vec![],
                path: vec!["example".into()],
            }],
        };
        let actual = JniWriter.write_module(&ir.modules[0], &ir);
        assert_eq!(
            crate::normalize_source_code(expected),
            crate::normalize_source_code(&actual),
        );
    }

    #[test]
    fn function_nothing() {
        let expected = r#"
            public static void function( ) {
                riko_sample.__riko_bridge.__riko_Module.xxx( );
            }
        "#;
        let ir = Crate {
            bridges: vec![],
            name: "riko_sample".into(),
            modules: vec![Module {
                functions: vec![Function {
                    name_public: "function".into(),
                    name_bridge: "xxx".into(),
                    inputs: vec![],
                    output: None,
                }],
                path: vec!["example".into()],
            }],
        };
        let actual = JniWriter.write_function(&ir.modules[0].functions[0], &ir.modules[0], &ir);

        assert_eq!(
            crate::normalize_source_code(expected),
            crate::normalize_source_code(&actual),
        );
    }

    #[test]
    fn function_simple() {
        let expected = r#"
            public static java.lang.String function(
                final java.lang.String arg_0,
                final java.lang.String arg_1
            ) {
                final byte[] returned = riko_sample.__riko_bridge.__riko_Module.xxx(
                    riko.Marshaler.toBytes(arg_0),
                    riko.Marshaler.toBytes(arg_1)
                );
                final riko.Returned<java.lang.String> result = riko.Marshaler.fromBytes(returned);
                return result.unwrap();
            }
        "#;
        let ir = Crate {
            bridges: vec![],
            name: "riko_sample".into(),
            modules: vec![Module {
                functions: vec![Function {
                    name_public: "function".into(),
                    name_bridge: "xxx".into(),
                    inputs: vec![MarshalingRule::String, MarshalingRule::String],
                    output: Some(MarshalingRule::String),
                }],
                path: vec!["example".into()],
            }],
        };
        let actual = JniWriter.write_function(&ir.modules[0].functions[0], &ir.modules[0], &ir);

        assert_eq!(
            crate::normalize_source_code(expected),
            crate::normalize_source_code(&actual),
        );
    }

    #[test]
    fn bridge() {
        let expected = r#"
            package riko_sample.__riko_bridge;

            public final class __riko_Module {
                private __riko_Module() {}

                public static native void xxx( );
                public static native byte[] yyy( byte[] arg_0, byte[] arg_1 );
            }
        "#;

        let ir = Crate {
            name: "riko_sample".into(),
            modules: vec![],
            bridges: vec![
                Bridge {
                    name: "xxx".into(),
                    input: 0,
                    output: false,
                },
                Bridge {
                    name: "yyy".into(),
                    input: 2,
                    output: true,
                },
            ],
        };
        let actual = JniWriter.write_bridges(&ir);

        assert_eq!(
            crate::normalize_source_code(expected),
            crate::normalize_source_code(&actual),
        );
    }
}
