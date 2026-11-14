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
}
