pub mod fir2ir;
pub mod ir;
pub mod lowerings;
pub mod visitor;

pub use fir2ir::Fir2Ir;
pub use ir::*;
pub use lowerings::LoweringPipeline;
pub use visitor::{IrTransformer, IrVisitor};

/// Converts a parsed Kotlin AST file into universal Backend IR (`IrFile`),
/// and runs all standard lowerings (Coroutines, Default Arguments, Inlining, Value Classes).
pub fn compile_to_lowered_ir(file: &k3_syntax::ast::KtFile, file_name: &str) -> IrFile {
    let mut converter = Fir2Ir::new();
    let mut ir_file = converter.convert_file(file, file_name);

    let pipeline = LoweringPipeline::new();
    pipeline.run(&mut ir_file);

    ir_file
}

#[cfg(test)]
mod tests {
    use super::*;
    use k3_syntax::{Lexer, Parser};

    #[test]
    fn test_coroutine_cps_lowering() {
        let code = r#"
            package test.coroutine
            suspend fun asyncTask(): String {
                return "Done"
            }
        "#;
        let mut lexer = Lexer::new(code);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens, code.len() as u32);
        let ast = parser.parse_file().unwrap();

        let ir = compile_to_lowered_ir(&ast, "coroutine.kt");

        // Should contain original function (now with continuation param and Any? return)
        let func = ir.declarations.iter().find_map(|d| match d {
            IrDeclaration::Function(f) if f.name == "asyncTask" => Some(f),
            _ => None,
        }).expect("asyncTask function should exist");

        assert!(func.is_suspend);
        assert_eq!(func.value_params.len(), 1);
        assert_eq!(func.value_params[0].name, "$completion");
        assert!(func.return_type.is_nullable()); // Any?

        // Should also contain synthetic Continuation class
        let has_cont_class = ir.declarations.iter().any(|d| match d {
            IrDeclaration::Class(c) => c.name.contains("asyncTask$Continuation"),
            _ => false,
        });
        assert!(has_cont_class, "Synthetic Continuation class should be generated");
    }

    #[test]
    fn test_default_argument_stub_lowering() {
        let code = r#"
            package test.defaults
            fun greet(name: String, prefix: String = "Hello"): String {
                return "$prefix $name"
            }
        "#;
        let mut lexer = Lexer::new(code);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens, code.len() as u32);
        let ast = parser.parse_file().unwrap();

        let ir = compile_to_lowered_ir(&ast, "defaults.kt");

        // Should contain both original greet and synthetic greet$default
        let has_original = ir.declarations.iter().any(|d| d.name() == "greet");
        let has_default_stub = ir.declarations.iter().any(|d| d.name() == "greet$default");

        assert!(has_original, "Original greet function must be retained");
        assert!(has_default_stub, "Synthetic greet$default stub must be generated");

        let stub = ir.declarations.iter().find_map(|d| match d {
            IrDeclaration::Function(f) if f.name == "greet$default" => Some(f),
            _ => None,
        }).unwrap();

        // params: name, prefix, $mask0, $handler -> 4 params
        assert_eq!(stub.value_params.len(), 4);
        assert_eq!(stub.value_params[2].name, "$mask0");
    }

    #[test]
    fn test_function_inlining_lowering() {
        let code = r#"
            package test.inlining
            inline fun doubleIt(x: Int): Int = x * 2
            fun main(): Int = doubleIt(21)
        "#;
        let mut lexer = Lexer::new(code);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens, code.len() as u32);
        let ast = parser.parse_file().unwrap();

        let ir = compile_to_lowered_ir(&ast, "inline.kt");
        let main_fn = ir.declarations.iter().find_map(|d| match d {
            IrDeclaration::Function(f) if f.name == "main" => Some(f),
            _ => None,
        }).unwrap();

        let stmt = &main_fn.body.as_ref().unwrap().statements[0];
        let debug_str = format!("{:?}", stmt);
        assert!(debug_str.contains("times"), "doubleIt call should inline x * 2 into main, got: {}", debug_str);
    }
}
