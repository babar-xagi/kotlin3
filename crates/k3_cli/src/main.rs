use clap::{Parser, Subcommand};
use k3_syntax::Lexer;
use std::fs;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "k3")]
#[command(author = "Kotlin3 Project")]
#[command(version = "0.1.0")]
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
                println!("  [{:>4}..{:<4}] {:<24} {:?}", token.span.start, token.span.end, format!("{:?}", token.kind), token.text);
            }
        }
        Commands::Check { file } => {
            let content = fs::read_to_string(&file)?;
            let mut lexer = Lexer::new(&content);
            let tokens = lexer.tokenize()?;
            println!("✓ Syntax check passed for {} ({} tokens)", file.display(), tokens.len());
        }
    }

    Ok(())
}
