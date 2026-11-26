mod runner;

use clap::{Parser, Subcommand};
use k3_fir::analyze_files;
use k3_ir::compile_to_lowered_ir;
use k3_pkg::{
    build_cache_dir, collect_kt_files, detect_toolchain, ensure_runtime_jar, init_project,
    k3_home_dir, maven, resolve_latest_version, runtime_cache_dir, sync_project, K3Lockfile,
    K3Manifest, MavenCoord,
};
use k3_syntax::Lexer;
use runner::{build_target, compile_with_kotlinc, parse_kotlin_source, run_target, run_tests, CompilerMode};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

#[derive(Parser, Debug)]
#[command(name = "k3")]
#[command(author = "Kotlin3 Project")]
#[command(version = "0.7.0")]
#[command(
    about = "Fast, zero-Gradle, zero-Maven Kotlin CLI, package manager, and runtime in Rust & Kotlin",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Initialize a new Kotlin3 project (k3.toml, src/Main.kt, tests/MainTest.kt) without Gradle/Maven
    Init {
        /// Target directory (defaults to current directory)
        dir: Option<PathBuf>,
        /// Project name
        #[arg(long)]
        name: Option<String>,
    },
    /// Add a Maven Central dependency (e.g. group:artifact or group:artifact:version)
    Add {
        /// Dependency coordinate (group:artifact[:version])
        coordinate: String,
        /// Add as development/test dependency
        #[arg(long)]
        dev: bool,
    },
    /// Remove a dependency from k3.toml and refresh k3.lock
    Remove {
        /// Dependency coordinate (group:artifact)
        coordinate: String,
    },
    /// Resolve and sync dependencies from k3.toml and k3.lock into global cache
    Sync {
        /// Force re-resolving versions from Maven repositories
        #[arg(long)]
        refresh: bool,
    },
    /// Compile and execute a Kotlin file, project (k3.toml), or named script
    Run {
        /// Source file (.kt), or script name from k3.toml [scripts]
        target: Option<String>,
        /// Force fast Rust JVM bytecode compiler
        #[arg(long)]
        fast: bool,
        /// Force official kotlinc compiler
        #[arg(long)]
        kotlinc: bool,
        /// Bypass content-addressed build cache
        #[arg(long)]
        no_cache: bool,
        /// Arguments passed to Kotlin main(args)
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Build a runnable JAR (or self-contained --fat uber-JAR) without Gradle or Maven
    Build {
        /// Source file path (.kt, .kts); defaults to current k3.toml project
        file: Option<PathBuf>,
        /// Output JAR file path
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Force fast Rust JVM bytecode compiler
        #[arg(long)]
        fast: bool,
        /// Force official kotlinc compiler
        #[arg(long)]
        kotlinc: bool,
        /// Package Kotlin stdlib, k3-runtime, and Maven dependencies into a standalone fat JAR
        #[arg(long)]
        fat: bool,
    },
    /// Discover and run Kotlin tests (tests/**/*.kt) using k3.test
    Test {
        /// Optional test file, directory, or name filter
        filter: Option<String>,
    },
    /// Fast syntax and K2 FIR semantic type-checking (use --strict for kotlinc check)
    Check {
        /// Source file or directory; defaults to current project src/
        target: Option<PathBuf>,
        /// Also verify using installed kotlinc
        #[arg(long)]
        strict: bool,
    },
    /// Lower Kotlin AST into Universal Backend IR
    Ir {
        /// Source file path (.kt, .kts)
        file: PathBuf,
    },
    /// Parse Kotlin source file into AST
    Parse {
        /// Source file path (.kt, .kts)
        file: PathBuf,
    },
    /// Tokenize and inspect Kotlin source file
    Lex {
        /// Source file path (.kt, .kts)
        file: PathBuf,
    },
    /// Inspect or install JDK & latest Kotlin compiler toolchains
    Toolchain {
        /// Action: status (default) or install
        #[arg(default_value = "status")]
        action: String,
    },
    /// Inspect or clean the global k3 dependency & build cache
    Cache {
        /// Action: dir | status | clean
        #[arg(default_value = "status")]
        action: String,
    },
    /// Show project manifest, dependencies, and active toolchain summary
    Info,
}

fn handle_io_error(err: &std::io::Error, path: &Path) {
    if err.raw_os_error() == Some(112) || err.kind() == std::io::ErrorKind::StorageFull {
        eprintln!("❌ [E0112] Storage Full: Not enough space on the disk to write {}", path.display());
    } else {
        eprintln!("❌ [E0010] I/O Error accessing {}: {}", path.display(), err);
    }
}

fn mode_from_flags(fast: bool, kotlinc: bool) -> CompilerMode {
    if fast {
        CompilerMode::Fast
    } else if kotlinc {
        CompilerMode::Kotlinc
    } else {
        CompilerMode::Auto
    }
}

fn main() {
    let cli = Cli::parse();
    if let Err(err) = dispatch(cli.command) {
        eprintln!("❌ {}", err);
        process::exit(1);
    }
}

fn dispatch(cmd: Commands) -> Result<(), String> {
    match cmd {
        Commands::Init { dir, name } => {
            let target_dir = dir.unwrap_or_else(|| PathBuf::from("."));
            if !target_dir.exists() {
                fs::create_dir_all(&target_dir).map_err(|e| format!("[E0010] {}", e))?;
            }
            let tc = detect_toolchain();
            let kotlin_ver = tc
                .kotlin_version
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or("2.4.21");

            let manifest = init_project(&target_dir, name.as_deref(), kotlin_ver)?;
            println!("✓ Initialized Kotlin3 project '{}' in {}", manifest.package.name, target_dir.display());
            println!("  Manifest: k3.toml (Kotlin {})", manifest.package.kotlin);
            println!("  Entry:    {}", manifest.package.entry);
            println!("  Next:     k3 run   |   k3 test   |   k3 add <group:artifact>");
        }
        Commands::Add { coordinate, dev } => {
            let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
            let manifest_path = cwd.join("k3.toml");
            if !manifest_path.exists() {
                return Err("No k3.toml found in current directory. Run 'k3 init' first.".to_string());
            }
            let mut manifest = K3Manifest::load(&manifest_path)?;
            let coord = MavenCoord::parse(&coordinate)?;
            let version = match coord.version.clone() {
                Some(v) => v,
                None => {
                    println!("Resolving latest version for {}...", coord.key());
                    resolve_latest_version(&coord.group, &coord.artifact, &manifest.repositories)?
                }
            };
            manifest.add_dependency(&coord.key(), &version, dev);
            manifest.save(&manifest_path)?;
            let (_, lock, _) = sync_project(&cwd, true)?;
            println!(
                "✓ Added {}:{} ({} package(s) locked in k3.lock)",
                coord.key(),
                version,
                lock.packages.len()
            );
        }
        Commands::Remove { coordinate } => {
            let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
            let manifest_path = cwd.join("k3.toml");
            let mut manifest = K3Manifest::load(&manifest_path)?;
            let coord = MavenCoord::parse(&coordinate)?;
            if !manifest.remove_dependency(&coord.key()) {
                return Err(format!("Dependency '{}' is not present in k3.toml", coord.key()));
            }
            manifest.save(&manifest_path)?;
            let (_, lock, _) = sync_project(&cwd, true)?;
            println!(
                "✓ Removed {} ({} package(s) remaining in k3.lock)",
                coord.key(),
                lock.packages.len()
            );
        }
        Commands::Sync { refresh } => {
            let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
            let (_, lock, _) = sync_project(&cwd, refresh)?;
            println!("✓ Synced {} package(s) into {}", lock.packages.len(), maven::maven_cache_dir().display());
        }
        Commands::Run { target, fast, kotlinc, no_cache, args } => {
            let path_opt = target.as_deref().and_then(|t| {
                let p = Path::new(t);
                if p.exists() || t.ends_with(".kt") || t.ends_with(".kts") {
                    Some(p)
                } else {
                    None
                }
            });
            run_target(
                path_opt,
                target.as_deref(),
                mode_from_flags(fast, kotlinc),
                no_cache,
                &args,
            )?;
        }
        Commands::Build { file, output, fast, kotlinc, fat } => {
            let (out_path, main_cls) = build_target(
                file.as_deref(),
                output.as_deref(),
                mode_from_flags(fast, kotlinc),
                fat,
            )?;
            println!("✓ Built runnable JAR: {}", out_path.display());
            println!("  Main Class: {}", main_cls);
            println!("  Run with:   java -jar {}", out_path.display());
        }
        Commands::Test { filter } => {
            run_tests(filter.as_deref())?;
        }
        Commands::Check { target, strict } => {
            let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
            let project_root = match &target {
                Some(p) if p.is_dir() && p.join("k3.toml").exists() => Some(p.clone()),
                Some(p) => p.parent().and_then(|d| {
                    if d.join("k3.toml").exists() {
                        Some(d.to_path_buf())
                    } else if d.parent().is_some_and(|gp| gp.join("k3.toml").exists()) {
                        d.parent().map(Path::to_path_buf)
                    } else {
                        None
                    }
                }),
                None if cwd.join("k3.toml").exists() => Some(cwd.clone()),
                None => None,
            };

            let files = match target {
                Some(p) if p.is_file() => vec![p],
                Some(p) if p.is_dir() => collect_kt_files(&p),
                Some(p) => return Err(format!("[E0010] Path not found: {}", p.display())),
                None if cwd.join("src").exists() => collect_kt_files(&cwd.join("src")),
                None => return Err("Specify a .kt file or run inside a project with src/".to_string()),
            };
            if files.is_empty() {
                return Err("No .kt files found to check".to_string());
            }

            // Include sibling project src/ files when checking a test or single project file so cross-file calls resolve cleanly
            let mut analysis_files = files.clone();
            if let Some(ref root) = project_root {
                for src_f in collect_kt_files(&root.join("src")) {
                    if !analysis_files.contains(&src_f) {
                        analysis_files.push(src_f);
                    }
                }
            }

            let mut parsed = Vec::with_capacity(analysis_files.len());
            for f in &analysis_files {
                let content = fs::read_to_string(f).map_err(|e| format!("[E0010] {}", e))?;
                let ast = parse_kotlin_source(&content, f)?;
                parsed.push((f.clone(), content, ast));
            }
            let ast_refs: Vec<&k3_syntax::ast::KtFile> = parsed.iter().map(|(_, _, a)| a).collect();
            let (_, diags) = analyze_files(&ast_refs);

            let target_count = files.len();
            let mut total_errors = 0usize;
            let mut per_file_has_diags = vec![false; target_count];
            for (idx, d) in &diags {
                if *idx >= target_count {
                    continue;
                }
                per_file_has_diags[*idx] = true;
                let (f, content, _) = &parsed[*idx];
                eprintln!("{}", d.format_with_source(content, &f.to_string_lossy()));
                if d.severity == k3_fir::DiagnosticSeverity::Error {
                    total_errors += 1;
                }
            }
            for (idx, f) in files.iter().enumerate() {
                if !per_file_has_diags[idx] {
                    println!(
                        "✓ Type check passed for {} ({} top-level declaration(s) verified)",
                        f.to_string_lossy(),
                        parsed[idx].2.declarations.len()
                    );
                }
            }
            if total_errors > 0 {
                return Err(format!("Check failed with {} error(s)", total_errors));
            }
            if strict {
                let tc = detect_toolchain();
                let java_exe = tc.java_exe.ok_or_else(runner::jre_not_found_msg)?;
                let kotlinc_exe = tc.kotlinc_exe.ok_or_else(runner::kotlinc_not_found_msg)?;
                let (kotlin_ver, mut cp) = if let Some(ref root) = project_root {
                    let (m, _, deps) = sync_project(root, false)?;
                    (m.package.kotlin, deps)
                } else {
                    (tc.kotlin_version.unwrap_or_else(|| "2.4.21".to_string()), Vec::new())
                };
                cp.push(ensure_runtime_jar(&kotlinc_exe, Some(&java_exe), &kotlin_ver)?);
                let tmp = std::env::temp_dir().join(format!("k3_strict_{}.jar", process::id()));
                compile_with_kotlinc(&kotlinc_exe, &java_exe, &analysis_files, &cp, &tmp)?;
                let _ = fs::remove_file(&tmp);
                println!("✓ Strict kotlinc verification passed ({} file(s))", files.len());
            }
        }
        Commands::Ir { file } => {
            let content = fs::read_to_string(&file).map_err(|e| {
                handle_io_error(&e, &file);
                e.to_string()
            })?;
            let ast = parse_kotlin_source(&content, &file)?;
            let file_name = file.file_name().unwrap().to_str().unwrap();
            let ir_file = compile_to_lowered_ir(&ast, file_name);
            println!("✓ Lowered {} into Universal Backend IR:", file.display());
            println!("  Package:      {}", if ir_file.package_name.is_empty() { "<root>" } else { &ir_file.package_name });
            println!("  Declarations: {}", ir_file.declarations.len());
            println!("{:#?}", ir_file);
        }
        Commands::Parse { file } => {
            let content = fs::read_to_string(&file).map_err(|e| e.to_string())?;
            let ast = parse_kotlin_source(&content, &file)?;
            println!("Successfully parsed {}:", file.display());
            println!("{:#?}", ast);
        }
        Commands::Lex { file } => {
            let content = fs::read_to_string(&file).map_err(|e| e.to_string())?;
            let mut lexer = Lexer::new(&content);
            let tokens = lexer.tokenize().map_err(|e| e.to_string())?;
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
        Commands::Toolchain { action } => match action.as_str() {
            "status" | "list" => {
                let tc = detect_toolchain();
                println!("Kotlin3 Toolchain Status (k3 0.7.0)");
                println!("  k3 home: {}", k3_home_dir().display());
                println!(
                    "  java:    {} ({})",
                    tc.java_exe.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "not found".into()),
                    tc.java_version.unwrap_or_else(|| "unknown".into())
                );
                println!(
                    "  javac:   {}",
                    tc.javac_exe.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "not found".into())
                );
                println!(
                    "  kotlinc: {} ({})",
                    tc.kotlinc_exe.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "not found".into()),
                    tc.kotlin_version.unwrap_or_else(|| "unknown".into())
                );
            }
            "install" => install_toolchains()?,
            other => return Err(format!("Unknown toolchain action '{}'. Use 'status' or 'install'.", other)),
        },
        Commands::Cache { action } => match action.as_str() {
            "dir" => println!("{}", k3_home_dir().join("cache").display()),
            "status" => {
                let maven_sz = dir_size(&maven::maven_cache_dir());
                let build_sz = dir_size(&build_cache_dir());
                let rt_sz = dir_size(&runtime_cache_dir());
                println!("Kotlin3 Cache ({})", k3_home_dir().join("cache").display());
                println!("  Maven JARs:   {} KB", maven_sz / 1024);
                println!("  Build cache:  {} KB", build_sz / 1024);
                println!("  Runtime JARs: {} KB", rt_sz / 1024);
            }
            "clean" => {
                let cache_root = k3_home_dir().join("cache");
                if cache_root.exists() {
                    fs::remove_dir_all(&cache_root).map_err(|e| e.to_string())?;
                }
                println!("✓ Cleared {}", cache_root.display());
            }
            other => return Err(format!("Unknown cache action '{}'. Use 'dir', 'status', or 'clean'.", other)),
        },
        Commands::Info => {
            let tc = detect_toolchain();
            println!("Kotlin3 (k3 0.7.0)");
            println!("  JDK:     {}", tc.java_version.unwrap_or_else(|| "not installed".into()));
            println!("  Kotlin:  {}", tc.kotlin_version.unwrap_or_else(|| "not installed".into()));
            let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
            let mpath = cwd.join("k3.toml");
            if mpath.exists() {
                let manifest = K3Manifest::load(&mpath)?;
                println!("  Project: {} v{} (entry: {})", manifest.package.name, manifest.package.version, manifest.package.entry);
                println!("  Deps:    {} direct", manifest.dependencies.len() + manifest.dev_dependencies.len());
                if let Ok(lock) = K3Lockfile::load(&cwd.join("k3.lock")) {
                    println!("  Locked:  {} package(s)", lock.packages.len());
                }
            } else {
                println!("  Project: none (no k3.toml in current directory)");
            }
        }
    }
    Ok(())
}

fn dir_size(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| {
            let p = e.path();
            if p.is_dir() {
                dir_size(&p)
            } else {
                e.metadata().map(|m| m.len()).unwrap_or(0)
            }
        })
        .sum()
}

fn install_toolchains() -> Result<(), String> {
    if cfg!(windows) {
        return Err("Automatic toolchain install is currently implemented for Linux/macOS shells; install JDK 25 & Kotlin 2.4+ and add to PATH.".to_string());
    }
    let tool_dir = k3_home_dir().join("toolchains");
    let jdk_dir = tool_dir.join("jdk");
    let kc_dir = tool_dir.join("kotlinc");
    fs::create_dir_all(&tool_dir).map_err(|e| e.to_string())?;

    println!("--> Installing OpenJDK 25 & latest Kotlin into {}...", tool_dir.display());
    let script = format!(
        r#"set -euo pipefail
mkdir -p "{tool_dir}"
KOTLIN_URL=$(curl -fsSL https://api.github.com/repos/JetBrains/kotlin/releases/latest | python3 -c 'import sys,json; d=json.load(sys.stdin); print(next(a["browser_download_url"] for a in d["assets"] if a["name"].startswith("kotlin-compiler-") and a["name"].endswith(".zip")))')
JDK_URL=$(curl -fsSL "https://api.adoptium.net/v3/assets/latest/25/hotspot?architecture=x64&image_type=jdk&os=linux" | python3 -c 'import sys,json; d=json.load(sys.stdin); print(d[0]["binary"]["package"]["link"])')
curl -fL -o /tmp/k3_jdk.tar.gz "$JDK_URL"
rm -rf "{jdk_dir}" && mkdir -p "{jdk_dir}"
tar -xzf /tmp/k3_jdk.tar.gz -C "{jdk_dir}" --strip-components=1
curl -fL -o /tmp/k3_kotlin.zip "$KOTLIN_URL"
rm -rf /tmp/k3_kc_ext && unzip -q /tmp/k3_kotlin.zip -d /tmp/k3_kc_ext
rm -rf "{kc_dir}" && mv /tmp/k3_kc_ext/kotlinc "{kc_dir}"
rm -rf /tmp/k3_kc_ext /tmp/k3_jdk.tar.gz /tmp/k3_kotlin.zip
"#,
        tool_dir = tool_dir.display(),
        jdk_dir = jdk_dir.display(),
        kc_dir = kc_dir.display()
    );
    let status = Command::new("bash")
        .arg("-c")
        .arg(&script)
        .status()
        .map_err(|e| format!("Toolchain install failed: {}", e))?;
    if !status.success() {
        return Err("Toolchain installer script returned non-zero exit code".to_string());
    }
    println!("✓ Installed JDK and Kotlin into {}", tool_dir.display());
    Ok(())
}
