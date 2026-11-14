use clap::{Parser, Subcommand};
use k3_fir::analyze;
use k3_syntax::{Lexer, Parser as KtParser};
use std::fs;
use std::path::PathBuf;
use std::process;

#[derive(Parser, Debug)]
#[command(name = "k3")]
#[command(author = "Kotlin3 Project")]
#[command(version = "0.3.0")]
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
    }

    Ok(())
}
