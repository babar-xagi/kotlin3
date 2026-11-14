use clap::{Parser, Subcommand};
use k3_fir::analyze;
use k3_ir::compile_to_lowered_ir;
use k3_jvm::compile_to_jar;
use k3_syntax::{Lexer, Parser as KtParser};
use std::fs;
use std::path::PathBuf;
use std::process;

#[derive(Parser, Debug)]
#[command(name = "k3")]
#[command(author = "Kotlin3 Project")]
#[command(version = "0.5.0")]
#[command(about = "Ultra-fast, next-generation Kotlin compiler and toolchain written in Rust & Zig", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Tokenize and inspect Kotlin source file
    Lex {
        /// Source file path (.kt, .kts)
        file: PathBuf,
    },
    /// Parse Kotlin source file into AST
    Parse {
        /// Source file path (.kt, .kts)
        file: PathBuf,
    },
    /// Parse and type check Kotlin source
    Check {
        /// Source file path (.kt, .kts)
        file: PathBuf,
    },
    /// Lower Kotlin AST into Universal Backend IR with coroutines, inlining, and default stubs
    Ir {
        /// Source file path (.kt, .kts)
        file: PathBuf,
    },
    /// Compile Kotlin source into executable JVM JAR
    Build {
        /// Source file path (.kt, .kts)
        file: PathBuf,
        /// Output JAR file path
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Lex { file } => {
            let content = fs::read_to_string(&file)?;
            let mut lexer = Lexer::new(&content);
            let tokens = lexer.tokenize()?;
            println!("Lexed {} tokens successfully from {}:", tokens.len(), file.display());
            for token in tokens {
                println!(
                    "  [{:>4}..{:<4}] {:<24} {:?}",
                    token.span.start,
                    token.span.end,
                    format!("{:?}", token.kind),
                    token.text
                );
            }
        }
        Commands::Parse { file } => {
            let content = fs::read_to_string(&file)?;
            let mut lexer = Lexer::new(&content);
            let tokens = lexer.tokenize()?;
            let mut parser = KtParser::new(tokens, content.len() as u32);
            let ast = parser.parse_file()?;
            println!("Successfully parsed {}:", file.display());
            println!("{:#?}", ast);
        }
        Commands::Check { file } => {
            let content = fs::read_to_string(&file)?;
            let file_str = file.to_string_lossy();
            let mut lexer = Lexer::new(&content);
            let tokens = match lexer.tokenize() {
                Ok(t) => t,
                Err(err) => {
                    eprintln!("{}:{}: error: {}", file_str, err.span, err);
                    process::exit(1);
                }
            };

            let mut parser = KtParser::new(tokens, content.len() as u32);
            let ast = match parser.parse_file() {
                Ok(a) => a,
                Err(err) => {
                    eprintln!("{}:{}: syntax error: {}", file_str, err.span, err.message);
                    process::exit(1);
                }
            };

            let (_, diagnostics) = analyze(&ast);

            let mut has_errors = false;
            for diag in &diagnostics {
                eprintln!("{}", diag.format_with_source(&content, &file_str));
                if diag.severity == k3_fir::DiagnosticSeverity::Error {
                    has_errors = true;
                }
            }

            if has_errors {
                eprintln!("\n❌ Check failed with {} error(s) for {}", diagnostics.len(), file_str);
                process::exit(1);
            } else {
                println!(
                    "✓ Type check passed for {} ({} top-level declaration(s) verified)",
                    file_str,
                    ast.declarations.len()
                );
            }
        }
        Commands::Ir { file } => {
            let content = fs::read_to_string(&file)?;
            let file_str = file.to_string_lossy();
            let file_name = file.file_name().unwrap().to_str().unwrap();

            // 1. Lexing
            let mut lexer = Lexer::new(&content);
            let tokens = match lexer.tokenize() {
                Ok(t) => t,
                Err(err) => {
                    eprintln!("{}:{}: error: {}", file_str, err.span, err);
                    process::exit(1);
                }
            };

            // 2. Parsing
            let mut parser = KtParser::new(tokens, content.len() as u32);
            let ast = match parser.parse_file() {
                Ok(a) => a,
                Err(err) => {
                    eprintln!("{}:{}: syntax error: {}", file_str, err.span, err.message);
                    process::exit(1);
                }
            };

            // 3. Lowering to Backend Universal IR with composable passes
            let ir_file = compile_to_lowered_ir(&ast, file_name);

            println!("✓ Successfully lowered {} into Universal Backend IR (k3_ir v0.5.0):", file.display());
            println!("  Package:      {}", if ir_file.package_name.is_empty() { "<root>" } else { &ir_file.package_name });
            println!("  Declarations: {}", ir_file.declarations.len());

            for (idx, decl) in ir_file.declarations.iter().enumerate() {
                match decl {
                    k3_ir::IrDeclaration::Function(f) => {
                        println!(
                            "    [{}] fun {}({}) : {:?} (origin: {:?}, suspend: {}, inline: {})",
                            idx,
                            f.name,
                            f.value_params.iter().map(|p| format!("{}: {:?}", p.name, p.param_type)).collect::<Vec<_>>().join(", "),
                            f.return_type,
                            f.origin,
                            f.is_suspend,
                            f.is_inline
                        );
                    }
                    k3_ir::IrDeclaration::Class(c) => {
                        println!(
                            "    [{}] {:?} {} (decls: {}, supers: {:?})",
                            idx,
                            c.kind,
                            c.name,
                            c.declarations.len(),
                            c.super_types
                        );
                    }
                    k3_ir::IrDeclaration::Property(p) => {
                        println!("    [{}] prop {}: {:?}", idx, p.name, p.prop_type);
                    }
                    k3_ir::IrDeclaration::Field(f) => {
                        println!("    [{}] field {}: {:?}", idx, f.name, f.field_type);
                    }
                }
            }

            println!("\n--- Full Lowered IR Dump ---");
            println!("{:#?}", ir_file);
        }
        Commands::Build { file, output } => {
            let content = fs::read_to_string(&file)?;
            let file_str = file.to_string_lossy();
            let file_name = file.file_name().unwrap().to_str().unwrap();

            // 1. Lexing
            let mut lexer = Lexer::new(&content);
            let tokens = match lexer.tokenize() {
                Ok(t) => t,
                Err(err) => {
                    eprintln!("{}:{}: error: {}", file_str, err.span, err);
                    process::exit(1);
                }
            };

            // 2. Parsing
            let mut parser = KtParser::new(tokens, content.len() as u32);
            let ast = match parser.parse_file() {
                Ok(a) => a,
                Err(err) => {
                    eprintln!("{}:{}: syntax error: {}", file_str, err.span, err.message);
                    process::exit(1);
                }
            };

            // 3. FIR Semantic Analysis & Type Checking
            let (_, diagnostics) = analyze(&ast);
            let mut has_errors = false;
            for diag in &diagnostics {
                eprintln!("{}", diag.format_with_source(&content, &file_str));
                if diag.severity == k3_fir::DiagnosticSeverity::Error {
                    has_errors = true;
                }
            }
            if has_errors {
                eprintln!("\n❌ Compilation aborted due to {} semantic error(s)", diagnostics.len());
                process::exit(1);
            }

            // 4. JVM Classfile & JAR Generation
            let (main_class, jar_bytes) = match compile_to_jar(&ast, file_name) {
                Ok(res) => res,
                Err(err) => {
                    eprintln!("codegen error: {}", err);
                    process::exit(1);
                }
            };

            let out_path = output.unwrap_or_else(|| {
                let stem = file.file_stem().unwrap().to_str().unwrap();
                PathBuf::from(format!("{}.jar", stem))
            });

            fs::write(&out_path, jar_bytes)?;
            println!("✓ Successfully compiled {} into runnable JAR: {}", file.display(), out_path.display());
            println!("  Main Class: {}", main_class.replace('/', "."));
            println!("  Run with:   java -jar {}", out_path.display());
        }
    }

    Ok(())
}
