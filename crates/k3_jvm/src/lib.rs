pub mod classfile;
pub mod bytecode;
pub mod codegen;
pub mod jar;

pub use classfile::ClassFile;
pub use bytecode::CodeBuilder;
pub use codegen::Codegen;
pub use jar::JarBuilder;

use k3_syntax::ast::KtFile;

pub fn compile_to_classes(file: &KtFile, source_name: &str) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut codegen = Codegen::new();
    codegen.compile_file(file, source_name)
}

pub fn compile_to_jar(file: &KtFile, source_name: &str) -> Result<(String, Vec<u8>), String> {
    let classes = compile_to_classes(file, source_name)?;
    let mut jar = JarBuilder::new();

    let mut main_class = None;
    for (class_name, bytes) in classes {
        if class_name.ends_with("Kt") {
            main_class = Some(class_name.clone());
        }
        let file_path = format!("{}.class", class_name);
        jar.add_file(&file_path, bytes);
    }

    let main_class_name = main_class.unwrap_or_else(|| "MainKt".to_string());
    jar.set_main_class(&main_class_name);

    Ok((main_class_name, jar.build()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classfile_magic() {
        let mut cf = ClassFile::new("Test", "java/lang/Object");
        let bytes = cf.to_bytes();
        assert_eq!(&bytes[0..4], &[0xCA, 0xFE, 0xBA, 0xBE]);
    }

    #[test]
    fn test_jar_structure() {
        let mut jar = JarBuilder::new();
        jar.add_file("Test.txt", b"Hello, Kotlin3!".to_vec());
        jar.set_main_class("TestKt");
        let bytes = jar.build();
        assert!(bytes.len() > 30);
        // ZIP local header starts with PK\x03\x04
        assert_eq!(&bytes[0..4], &[0x50, 0x4b, 0x03, 0x04]);
    }

    #[test]
    fn test_constant_pool_large_integers_and_doubles() {
        let mut cf = ClassFile::new("MathUtil", "java/lang/Object");
        let i_idx = cf.cp.add_integer(2_000_000);
        let l_idx = cf.cp.add_long(10_000_000_000);
        let d_idx = cf.cp.add_double(1_000_000.0);

        assert!(i_idx >= 1);
        assert!(l_idx >= 1);
        assert!(d_idx >= 1);

        let bytes = cf.to_bytes();
        assert_eq!(&bytes[0..4], &[0xCA, 0xFE, 0xBA, 0xBE]);
    }

    #[test]
    fn test_compile_data_class_and_long_function() {
        let source = r#"
            data class Pair32(val first: Int, val second: Int)

            fun calculate(count: Int): Pair32 {
                var acc = 0L
                var i = 0
                while (i < count) {
                    acc += i.toLong()
                    i++
                }
                return Pair32(count, acc.toInt())
            }
        "#;
        let mut lexer = k3_syntax::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().expect("tokenize failed");
        let mut parser = k3_syntax::parser::Parser::new(tokens, source.len() as u32);
        let file = parser.parse_file().expect("parse failed");

        let classes = compile_to_classes(&file, "Test.kt").expect("compile failed");
        assert_eq!(classes.len(), 2); // Pair32 and TestKt
        for (name, bytes) in &classes {
            assert_eq!(&bytes[0..4], &[0xCA, 0xFE, 0xBA, 0xBE], "class {} magic mismatch", name);
        }
    }
}
