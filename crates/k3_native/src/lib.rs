pub mod codegen;
pub mod compiler;

pub use codegen::NativeCodegen;
pub use compiler::{compile_to_native, is_zig_available, NativeCompiler, NativeError, RUNTIME_ZIG};

#[cfg(test)]
mod tests {
    use super::*;
    use k3_syntax::{Lexer, Parser};

    #[test]
    fn test_native_codegen_hello_world() {
        let code = r#"
            package test.hello
            fun main() {
                println("Hello, Kotlin3 Native!")
            }
        "#;
        let mut lexer = Lexer::new(code);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens, code.len() as u32);
        let ast = parser.parse_file().unwrap();

        let mut codegen = NativeCodegen::new();
        let zig_code = codegen.generate_file(&ast).unwrap();

        assert!(zig_code.contains("@import(\"runtime.zig\")"));
        assert!(zig_code.contains("pub fn main() !void"));
        assert!(zig_code.contains("k3.println(\"Hello, Kotlin3 Native!\")"));
    }

    #[test]
    fn test_native_codegen_data_class() {
        let code = r#"
            package test.oop
            data class Developer(val name: String, val role: String) {
                fun printDetails() {
                    println("Dev: " + name)
                }
            }
            fun main() {
                val dev = Developer("Alice", "Engineer")
                dev.printDetails()
            }
        "#;
        let mut lexer = Lexer::new(code);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens, code.len() as u32);
        let ast = parser.parse_file().unwrap();

        let mut codegen = NativeCodegen::new();
        let zig_code = codegen.generate_file(&ast).unwrap();

        assert!(zig_code.contains("pub const Developer = struct"));
        assert!(zig_code.contains("pub fn printDetails(self: Developer) void"));
        assert!(zig_code.contains("Developer{"));
    }

    #[test]
    fn test_e2e_native_compilation() {
        let code = r#"
            package test.e2e
            fun main() {
                println("Kotlin3 Native is alive!")
            }
        "#;
        let mut lexer = Lexer::new(code);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens, code.len() as u32);
        let ast = parser.parse_file().unwrap();

        let bin_path = std::env::temp_dir().join("k3_e2e_test_bin");
        let result = compile_to_native(&ast, &bin_path);
        assert!(result.is_ok(), "Native compilation failed: {:?}", result.err());

        // Run the binary and verify stdout
        let output = std::process::Command::new(&bin_path).output().unwrap();
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.trim(), "Kotlin3 Native is alive!");

        let _ = std::fs::remove_file(&bin_path);
    }
}
