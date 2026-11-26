use crate::sha256::Sha256;
use crate::toolchain::{java_home_for, k3_home_dir};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const RUNTIME_KT: &str = include_str!("../../../runtime/src/main/kotlin/k3/runtime/Runtime.kt");
pub const TEST_KT: &str = include_str!("../../../runtime/src/main/kotlin/k3/test/Test.kt");

pub fn runtime_sources_hash(kotlin_version: &str) -> String {
    let mut h = Sha256::new();
    h.update(kotlin_version.as_bytes());
    h.update(b"\0");
    h.update(RUNTIME_KT.as_bytes());
    h.update(b"\0");
    h.update(TEST_KT.as_bytes());
    h.finalize_hex()
}

/// Ensures `k3-runtime.jar` (written in Kotlin) is compiled with `kotlinc` and cached.
pub fn ensure_runtime_jar(
    kotlinc_exe: &Path,
    java_exe: Option<&PathBuf>,
    kotlin_version: &str,
) -> Result<PathBuf, String> {
    let hash = runtime_sources_hash(kotlin_version);
    let cache_dir = k3_home_dir().join("cache").join("runtime").join(&hash);
    let jar_path = cache_dir.join("k3-runtime.jar");
    if jar_path.is_file() {
        return Ok(jar_path);
    } else if jar_path.is_dir() {
        let _ = fs::remove_dir_all(&jar_path);
    }

    let src_dir = cache_dir.join("src");
    fs::create_dir_all(&src_dir).map_err(|e| e.to_string())?;
    let rt_file = src_dir.join("Runtime.kt");
    let test_file = src_dir.join("Test.kt");
    fs::write(&rt_file, RUNTIME_KT).map_err(|e| e.to_string())?;
    fs::write(&test_file, TEST_KT).map_err(|e| e.to_string())?;

    let tmp_jar = cache_dir.join("k3-runtime.tmp.jar");
    let mut cmd = Command::new(kotlinc_exe);
    if let Some(j) = java_exe {
        if let Some(jh) = java_home_for(j) {
            cmd.env("JAVA_HOME", jh);
        }
    }
    let out = cmd
        .arg(&rt_file)
        .arg(&test_file)
        .arg("-d")
        .arg(&tmp_jar)
        .output()
        .map_err(|e| format!("Failed launching kotlinc to compile k3-runtime: {}", e))?;

    if !out.status.success() {
        let _ = fs::remove_file(&tmp_jar);
        return Err(format!(
            "Failed compiling Kotlin runtime:\n{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ));
    }

    fs::rename(&tmp_jar, &jar_path).map_err(|e| e.to_string())?;
    Ok(jar_path)
}
