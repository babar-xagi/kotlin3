use crate::codegen::NativeCodegen;
use k3_syntax::ast::KtFile;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

pub const RUNTIME_ZIG: &str = include_str!("../../../runtime/src/runtime.zig");

#[derive(Error, Debug)]
pub enum NativeError {
    #[error("Codegen error: {0}")]
    Codegen(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("[E0001] Zig Native Compiler Not Found\n\nExplanation: 'k3 native' compiles Kotlin directly into standalone native machine-code executables using Zig's zero-dependency linker.\n\nTo run or compile without Zig:\n  -> Use 'k3 build <file.kt>' to generate standard runnable JARs (zero dependencies!).\n  -> Use 'k3 run <file.kt>' which automatically uses JVM mode when Zig is not present.\n\nTo install Zig for native compilation:\n  -> Windows: winget install zig.zig  (or download from https://ziglang.org/download/)\n  -> Linux:   sudo apt install zig    (or download from https://ziglang.org/download/)")]
    ZigNotFound,
    #[error("Zig compilation failed (exit code {status:?}):\n{stderr}")]
    CompilationFailed {
        status: Option<i32>,
        stderr: String,
    },
}

pub struct NativeCompiler {
    zig_path: PathBuf,
}

impl NativeCompiler {
    pub fn new() -> Result<Self, NativeError> {
        let zig_path = Self::find_zig()?;
        Ok(Self { zig_path })
    }

    pub fn find_zig() -> Result<PathBuf, NativeError> {
        // 1. Explicit environment variable
        if let Ok(p) = std::env::var("ZIG_EXE").or_else(|_| std::env::var("ZIG_PATH")) {
            let pb = PathBuf::from(p);
            if pb.exists() {
                return Ok(pb);
            }
        }

        // 2. Standard location in WSL / Linux
        let opt_zig = PathBuf::from("/opt/zig/zig");
        if opt_zig.exists() {
            return Ok(opt_zig);
        }

        // 3. Standard Windows locations
        if cfg!(windows) {
            let win_paths = [
                r"C:\zig\zig.exe",
                r"C:\Program Files\zig\zig.exe",
            ];
            for p in &win_paths {
                let pb = PathBuf::from(p);
                if pb.exists() {
                    return Ok(pb);
                }
            }
        }

        // 4. System PATH check (Windows where.exe or Unix which)
        let check_cmd = if cfg!(windows) { "where.exe" } else { "which" };
        if let Ok(output) = Command::new(check_cmd).arg("zig").output() {
            if output.status.success() {
                let first_line = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !first_line.is_empty() {
                    return Ok(PathBuf::from(first_line));
                }
            }
        }

        // 5. Check PATH environment variable directly
        if let Some(path_var) = std::env::var_os("PATH") {
            for dir in std::env::split_paths(&path_var) {
                let exe_name = if cfg!(windows) { "zig.exe" } else { "zig" };
                let candidate = dir.join(exe_name);
                if candidate.exists() {
                    return Ok(candidate);
                }
            }
        }

        Err(NativeError::ZigNotFound)
    }

    pub fn compile(
        &self,
        ast: &KtFile,
        output_binary: &Path,
    ) -> Result<PathBuf, NativeError> {
        // 1. Generate Zig source
        let mut codegen = NativeCodegen::new();
        let zig_code = codegen
            .generate_file(ast)
            .map_err(NativeError::Codegen)?;

        // 2. Prepare temporary build directory
        let temp_dir = std::env::temp_dir().join(format!("k3_native_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;

        let main_zig_path = temp_dir.join("main.zig");
        let runtime_zig_path = temp_dir.join("runtime.zig");

        fs::write(&main_zig_path, zig_code)?;
        fs::write(&runtime_zig_path, RUNTIME_ZIG)?;

        // 3. Ensure parent directory for output binary exists
        if let Some(parent) = output_binary.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }

        // 4. Invoke Zig build-exe
        let output_arg = format!("-femit-bin={}", output_binary.display());
        let mut cmd = Command::new(&self.zig_path);
        cmd.current_dir(&temp_dir)
            .arg("build-exe")
            .arg("main.zig")
            .arg("-O")
            .arg("ReleaseFast")
            .arg(&output_arg);

        let result = cmd.output()?;

        if !result.status.success() {
            let stderr = String::from_utf8_lossy(&result.stderr).to_string();
            return Err(NativeError::CompilationFailed {
                status: result.status.code(),
                stderr,
            });
        }

        // 5. Cleanup temporary source files
        let _ = fs::remove_dir_all(&temp_dir);

        Ok(output_binary.to_path_buf())
    }
}

/// Convenience function to compile Kotlin AST into a standalone native binary
pub fn compile_to_native(ast: &KtFile, output_binary: &Path) -> Result<PathBuf, NativeError> {
    let compiler = NativeCompiler::new()?;
    compiler.compile(ast, output_binary)
}

/// Check if Zig compiler is available on the current system
pub fn is_zig_available() -> bool {
    NativeCompiler::find_zig().is_ok()
}
