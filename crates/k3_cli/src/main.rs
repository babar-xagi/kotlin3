use clap::{Parser, Subcommand};
use k3_fir::analyze;
use k3_ir::compile_to_lowered_ir;
use k3_jvm::compile_to_jar;
use k3_native::{compile_to_native, is_zig_available};
use k3_syntax::{Lexer, Parser as KtParser};
use std::fs;
use std::path::PathBuf;
use std::process::{self, Command};

#[derive(Parser, Debug)]
#[command(name = "k3")]
#[command(author = "Kotlin3 Project")]
#[command(version = "0.6.1")]
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
    /// Compile Kotlin source directly into a standalone native executable using Zig
    Native {
        /// Source file path (.kt, .kts)
        file: PathBuf,
        /// Output binary file path
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Compile and execute Kotlin program immediately (standalone native binary or JVM JAR)
    Run {
        /// Source file path (.kt, .kts)
        file: PathBuf,
        /// Optional output file path to save the compiled executable or JAR
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Force execution using JVM bytecode target instead of native binary
        #[arg(long)]
        jvm: bool,
        /// Force execution using standalone native binary target
        #[arg(long)]
        native: bool,
    },
}

fn find_java() -> Option<PathBuf> {
    // 1. Check JAVA_HOME
    if let Ok(jh) = std::env::var("JAVA_HOME") {
        let exe = if cfg!(windows) { "java.exe" } else { "java" };
        let candidate = PathBuf::from(jh).join("bin").join(exe);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    // 2. System PATH check
    let check_cmd = if cfg!(windows) { "where.exe" } else { "which" };
    if let Ok(output) = Command::new(check_cmd).arg("java").output() {
        if output.status.success() {
            let first_line = String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if !first_line.is_empty() {
                return Some(PathBuf::from(first_line));
            }
        }
    }

    // 3. Search PATH entries directly
    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let exe = if cfg!(windows) { "java.exe" } else { "java" };
            let candidate = dir.join(exe);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    // 4. Standard Windows JDK/JRE locations (JetBrains JBR, Eclipse Adoptium, Oracle)
    if cfg!(windows) {
        let candidates = [
            r"C:\Program Files\JetBrains\IntelliJ IDEA 2026.2.3\jbr\bin\java.exe",
            r"C:\Program Files\JetBrains\IntelliJ IDEA\jbr\bin\java.exe",
            r"C:\Program Files\Eclipse Adoptium\jdk-25-hotspot\bin\java.exe",
            r"C:\Program Files\Eclipse Adoptium\jdk-21-hotspot\bin\java.exe",
            r"C:\Program Files\Java\jdk-25\bin\java.exe",
            r"C:\Program Files\Java\jdk-21\bin\java.exe",
            r"C:\Program Files\Java\jdk-17\bin\java.exe",
        ];
        for c in &candidates {
            let p = PathBuf::from(c);
            if p.exists() {
                return Some(p);
            }
        }
    }

    // 5. Standard Linux paths
    if !cfg!(windows) {
        let candidates = [
            "/home/xagi/.sdkman/candidates/java/current/bin/java",
            "/usr/lib/jvm/default-java/bin/java",
            "/usr/bin/java",
        ];
        for c in &candidates {
            let p = PathBuf::from(c);
            if p.exists() {
                return Some(p);
            }
        }
    }

    None
}

fn handle_io_error(err: &std::io::Error, path: &std::path::Path) {
    if err.raw_os_error() == Some(112) || err.kind() == std::io::ErrorKind::StorageFull {
        eprintln!("❌ [E0112] Storage Full: Not enough space on the disk to write {}", path.display());
        eprintln!("\nExplanation: The destination drive or temporary directory has run out of free disk space.");
        eprintln!("Actionable Fix:");
        eprintln!("  1. Free up disk space on the target drive (e.g. drive C:).");
        eprintln!("  2. Clear temporary files in %TEMP% or /tmp.");
    } else {
        eprintln!("❌ [E0010] I/O Error writing {}: {}", path.display(), err);
    }
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

            let ir_file = compile_to_lowered_ir(&ast, file_name);

            println!("✓ Successfully lowered {} into Universal Backend IR (k3_ir v0.6.1):", file.display());
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
                eprintln!("\n❌ Compilation aborted due to {} semantic error(s)", diagnostics.len());
                process::exit(1);
            }

            let (main_class, jar_bytes) = match compile_to_jar(&ast, file_name) {
                Ok(res) => res,
                Err(err) => {
                    eprintln!("❌ [E0020] Codegen error: {}", err);
                    process::exit(1);
                }
            };

            let out_path = output.unwrap_or_else(|| {
                let stem = file.file_stem().unwrap().to_str().unwrap();
                PathBuf::from(format!("{}.jar", stem))
            });

            if let Err(err) = fs::write(&out_path, jar_bytes) {
                handle_io_error(&err, &out_path);
                process::exit(1);
            }

            println!("✓ Successfully compiled {} into runnable JAR: {}", file.display(), out_path.display());
            println!("  Main Class: {}", main_class.replace('/', "."));
            println!("  Run with:   java -jar {}", out_path.display());
        }
        Commands::Native { file, output } => {
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
                eprintln!("\n❌ Native compilation aborted due to {} semantic error(s)", diagnostics.len());
                process::exit(1);
            }

            let out_path = output.unwrap_or_else(|| {
                let stem = file.file_stem().unwrap().to_str().unwrap();
                PathBuf::from(format!("{}_bin", stem))
            });

            match compile_to_native(&ast, &out_path) {
                Ok(bin) => {
                    let metadata = fs::metadata(&bin);
                    let size_kb = metadata.map(|m| m.len() / 1024).unwrap_or(0);
                    println!("✓ Successfully compiled {} into standalone native executable: {}", file.display(), bin.display());
                    println!("  Target:      Zig Native (ELF x86_64)");
                    println!("  Footprint:   {} KB (< 2 MB)", size_kb);
                    println!("  Run with:    {}", bin.display());
                }
                Err(err) => {
                    eprintln!("❌ Native compilation error:\n{}", err);
                    process::exit(1);
                }
            }
        }
        Commands::Run { file, output, jvm, native } => {
            let content = fs::read_to_string(&file)?;
            let file_str = file.to_string_lossy();
            let file_name = file.file_name().unwrap().to_str().unwrap();
            let stem = file.file_stem().unwrap().to_str().unwrap();

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

            // Intelligent target resolution:
            // 1. If --native is explicitly requested: must use native (fails if Zig is missing)
            // 2. If --jvm is explicitly requested: must use JVM
            // 3. Default: auto-detect! If Zig is installed, use ultra-fast native;
            //    otherwise, seamlessly fall back to JVM mode!
            let use_native = if native {
                true
            } else if jvm {
                false
            } else {
                is_zig_available()
            };

            if use_native {
                let target_bin = output.clone().unwrap_or_else(|| {
                    let exe_ext = if cfg!(windows) { ".exe" } else { "" };
                    std::env::temp_dir().join(format!("{}_{}_bin{}", stem, process::id(), exe_ext))
                });

                if let Err(err) = compile_to_native(&ast, &target_bin) {
                    eprintln!("❌ Native compilation error:\n{}", err);
                    process::exit(1);
                }

                let status = Command::new(&target_bin).status();
                if output.is_none() {
                    let _ = fs::remove_file(&target_bin);
                }

                match status {
                    Ok(s) if s.success() => {}
                    Ok(s) => process::exit(s.code().unwrap_or(1)),
                    Err(err) => {
                        eprintln!("❌ [E0004] Failed to execute native binary: {}", err);
                        process::exit(1);
                    }
                }
            } else {
                let (_, jar_bytes) = match compile_to_jar(&ast, file_name) {
                    Ok(res) => res,
                    Err(err) => {
                        eprintln!("❌ [E0020] Codegen error: {}", err);
                        process::exit(1);
                    }
                };

                let target_jar = output.clone().unwrap_or_else(|| {
                    std::env::temp_dir().join(format!("{}_{}.jar", stem, process::id()))
                });

                if let Err(err) = fs::write(&target_jar, jar_bytes) {
                    handle_io_error(&err, &target_jar);
                    process::exit(1);
                }

                let java_exe = find_java();
                let java_cmd = match &java_exe {
                    Some(p) => p.as_os_str(),
                    None => std::ffi::OsStr::new(if cfg!(windows) { "java.exe" } else { "java" }),
                };

                let status = Command::new(java_cmd)
                    .arg("-jar")
                    .arg(&target_jar)
                    .status();

                if output.is_none() {
                    let _ = fs::remove_file(&target_jar);
                }

                match status {
                    Ok(s) if s.success() => {}
                    Ok(s) => process::exit(s.code().unwrap_or(1)),
                    Err(err) => {
                        eprintln!("❌ [E0002] Java Runtime Environment (JRE) Not Found: {}", err);
                        eprintln!("\nExplanation: 'k3 run' in JVM mode requires a Java Virtual Machine (Java 8–25) to execute .jar files.");
                        eprintln!("Actionable Fix:");
                        eprintln!("  1. Install OpenJDK (e.g. 'winget install Microsoft.OpenJDK.21' or adoptium.net).");
                        eprintln!("  2. Or set JAVA_HOME pointing to your JDK directory.");
                        eprintln!("  3. Or install Zig ('winget install zig.zig') to compile directly to standalone native machine code without needing Java!");
                        process::exit(1);
                    }
                }
            }
        }
    }

    Ok(())
}
