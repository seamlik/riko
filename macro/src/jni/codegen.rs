use crate::codegen::TargetCodeWriter;
use crate::parse::Fun;
use crate::parse::MarshalingRule;
use itertools::Itertools;
use quote::ToTokens;
use std::path::PathBuf;

/// Generates JNI code to the target directory.
///
/// All parameters must be fully expanded.
pub fn fun(args: &Fun, input_rules: &[MarshalingRule]) {
    let generated = function(&args.module, &args.name, input_rules, &args.marshal);
    let writer = JavaWriter {
        module_name: &args.module,
    };
    writer.insert(&generated, &args.name).expect(&format!(
        "JNI source for `{}`",
        args.module
            .iter()
            .chain(&[args.name.to_string()])
            .join("::")
    ));
}

const CLASS_FOR_MODULE: &str = "__riko_Module";

struct JavaWriter<'a> {
    module_name: &'a [String],
}

impl TargetCodeWriter for JavaWriter<'_> {
    fn cursor() -> &'static str {
        "/* __riko_cursor */"
    }

    fn encloser(module: &[String], name: &str) -> (String, String) {
        let opening = format!(
            "/* __riko_opening {} */",
            module
                .iter()
                .chain(std::iter::once(&name.into()))
                .join("::")
        );
        let closing = format!(
            "/* __riko_closing {} */",
            module
                .iter()
                .chain(std::iter::once(&name.into()))
                .join("::")
        );
        (opening, closing)
    }

    fn target_name() -> &'static str {
        "jni"
    }

    fn module_name(&self) -> &[String] {
        self.module_name
    }

    fn module_template(&self) -> String {
        module(self.module_name)
    }

    fn target_file_path(&self) -> PathBuf {
        let mut result = self.target_root();
        result.extend(self.module_name.iter());
        result.push(format!("{}.java", CLASS_FOR_MODULE));
        result
    }
}

fn module(name: &[String]) -> String {
    let result_package = if name.is_empty() {
        String::default()
    } else {
        format!("package {};", name.join("."))
    };
    format!(
        r#"
            {package}

            public final class {class} {{

                private {class}() {{}}

                {cursor}
            }}
        "#,
        class = CLASS_FOR_MODULE,
        package = &result_package,
        cursor = JavaWriter::cursor(),
    )
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

/// Generates code for a function.
fn function(
    module: &[String],
    name: &str,
    inputs: &[MarshalingRule],
    output: &Option<MarshalingRule>,
) -> String {
    let (opening, closing) = JavaWriter::encloser(module, name);

    let return_type_native = if output.is_none() { "void" } else { "byte[]" };
    let return_type_result = match output {
        None => "void".into(),
        Some(MarshalingRule::Iterator(_)) => return_type(&MarshalingRule::I32),
        Some(inner) => return_type(inner),
    };
    let return_type_java = match output {
        None => "void".into(),
        Some(inner) => return_type(inner),
    };
    let return_prefix = if output.is_none() {
        ""
    } else {
        "final byte[] returned ="
    };
    let return_block = if output.is_none() {
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

    let args = (0..(inputs.len()))
        .map(|idx| format!("riko.Marshaler.toBytes(arg_{})", idx))
        .join(", ");
    let params_native = (0..(inputs.len()))
        .map(|idx| format!("byte[] arg_{}", idx))
        .join(", ");
    let params_java = (0..(inputs.len()))
        .map(|idx| format!("final {} arg_{}", return_type_java, idx))
        .join(", ");

    format!(
        r#"
            {opening}
            private static native {return_type_native} __riko_{name}( {params_native} );
            public static {return_type_java} {name}( {params_java} ) {{
                {return_prefix} __riko_{name}( {args} );
                {return_block}
            }}
            {closing}
        "#,
        args = args,
        closing = closing,
        name = name,
        opening = opening,
        params_java = params_java,
        params_native = params_native,
        return_block = return_block,
        return_prefix = return_prefix,
        return_type_java = return_type_java,
        return_type_native = return_type_native
    )
}

mod tests {
    use super::*;

    #[test]
    fn module_simple() {
        let expected = r#"
            package org.example;

            public final class __riko_Module {
                private __riko_Module() {}

                /* __riko_cursor */
            }
        "#;
        assert_eq!(
            crate::codegen::normalize_source_code(expected),
            crate::codegen::normalize_source_code(&module(&["org".into(), "example".into()]))
        );
    }

    #[test]
    fn module_none() {
        let expected = r#"
            public final class __riko_Module {
                private __riko_Module() {}

                /* __riko_cursor */
            }
        "#;
        assert_eq!(
            crate::codegen::normalize_source_code(expected),
            crate::codegen::normalize_source_code(&module(&[]))
        );
    }

    #[test]
    fn function_nothing() {
        let expected = r#"
            /* __riko_opening function */
            private static native void __riko_function( );
            public static void function( ) {
                __riko_function( );
            }
            /* __riko_closing function */
        "#;
        assert_eq!(
            crate::codegen::normalize_source_code(expected),
            crate::codegen::normalize_source_code(&function(
                &[],
                "function",
                Default::default(),
                &None
            ))
        )
    }

    #[test]
    fn function_simple() {
        let inputs = [MarshalingRule::String, MarshalingRule::String];
        let output = Some(MarshalingRule::String);
        let expected = r#"
            /* __riko_opening org::example::function */
            private static native byte[] __riko_function(
                byte[] arg_0,
                byte[] arg_1
            );
            public static java.lang.String function(
                final java.lang.String arg_0,
                final java.lang.String arg_1
            ) {
                final byte[] returned = __riko_function(
                    riko.Marshaler.toBytes(arg_0),
                    riko.Marshaler.toBytes(arg_1)
                );
                final riko.Returned<java.lang.String> result = riko.Marshaler.fromBytes(returned);
                return result.unwrap();
            }
            /* __riko_closing org::example::function */
        "#;
        assert_eq!(
            crate::codegen::normalize_source_code(expected),
            crate::codegen::normalize_source_code(&function(
                &["org".into(), "example".into()],
                "function",
                &inputs,
                &output
            ))
        )
    }
}
