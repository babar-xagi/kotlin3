use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct ToolchainInfo {
    pub java_exe: Option<PathBuf>,
    pub javac_exe: Option<PathBuf>,
    pub jar_exe: Option<PathBuf>,
    pub kotlinc_exe: Option<PathBuf>,
    pub kotlin_exe: Option<PathBuf>,
    pub java_version: Option<String>,
    pub kotlin_version: Option<String>,
}

pub fn k3_home_dir() -> PathBuf {
    if let Ok(home) = std::env::var("K3_HOME") {
        if !home.trim().is_empty() {
            return PathBuf::from(home);
        }
    }
    if let Some(home) = home_dir() {
        let candidate = home.join(".k3");
        if candidate.exists() || std::fs::create_dir_all(&candidate).is_ok() {
            let probe = candidate.join(".write_probe");
            if std::fs::write(&probe, b"").is_ok() {
                let _ = std::fs::remove_file(&probe);
                return candidate;
            }
        }
    }
    std::env::temp_dir().join("k3_home")
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn exe_name(base: &str) -> String {
    if cfg!(windows) {
        format!("{}.exe", base)
    } else {
        base.to_string()
    }
}

fn script_name(base: &str) -> String {
    if cfg!(windows) {
        format!("{}.bat", base)
    } else {
        base.to_string()
    }
}

fn find_binary(name: &str, extra_candidates: &[PathBuf]) -> Option<PathBuf> {
    for p in extra_candidates {
        if p.exists() {
            return Some(p.clone());
        }
    }

    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(name);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    let check_cmd = if cfg!(windows) { "where.exe" } else { "which" };
    if let Ok(out) = Command::new(check_cmd).arg(name).output() {
        if out.status.success() {
            let line = String::from_utf8_lossy(&out.stdout)
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if !line.is_empty() {
                return Some(PathBuf::from(line));
            }
        }
    }

    None
}

pub fn find_jdk_tool(tool: &str) -> Option<PathBuf> {
    let bin = exe_name(tool);
    let mut candidates = Vec::new();

    if let Ok(jh) = std::env::var("JAVA_HOME") {
        candidates.push(PathBuf::from(jh).join("bin").join(&bin));
    }
    candidates.push(k3_home_dir().join("toolchains").join("jdk").join("bin").join(&bin));

    if let Some(h) = home_dir() {
        candidates.push(h.join(".local").join("opt").join("jdk").join("bin").join(&bin));
        candidates.push(h.join(".local").join("bin").join(&bin));
        candidates.push(h.join(".sdkman").join("candidates").join("java").join("current").join("bin").join(&bin));
    }

    if !cfg!(windows) {
        candidates.push(PathBuf::from("/usr/lib/jvm/default-java/bin").join(&bin));
        candidates.push(PathBuf::from("/usr/bin").join(&bin));
    }

    find_binary(&bin, &candidates)
}

pub fn find_kotlin_tool(tool: &str) -> Option<PathBuf> {
    let bin = script_name(tool);
    let mut candidates = Vec::new();

    if let Ok(kh) = std::env::var("KOTLIN_HOME") {
        candidates.push(PathBuf::from(kh).join("bin").join(&bin));
    }
    candidates.push(k3_home_dir().join("toolchains").join("kotlinc").join("bin").join(&bin));

    if let Some(h) = home_dir() {
        candidates.push(h.join(".local").join("opt").join("kotlinc").join("bin").join(&bin));
        candidates.push(h.join(".local").join("bin").join(&bin));
        candidates.push(h.join(".sdkman").join("candidates").join("kotlin").join("current").join("bin").join(&bin));
    }

    find_binary(&bin, &candidates)
}

pub fn detect_toolchain_fast() -> ToolchainInfo {
    let java_exe = find_jdk_tool("java");
    let javac_exe = find_jdk_tool("javac");
    let jar_exe = find_jdk_tool("jar");
    let kotlinc_exe = find_kotlin_tool("kotlinc");
    let kotlin_exe = find_kotlin_tool("kotlin");

    let java_version = java_exe.as_ref().and_then(read_java_release_file);
    let kotlin_version = kotlinc_exe.as_ref().and_then(read_kotlin_build_file);

    ToolchainInfo {
        java_exe,
        javac_exe,
        jar_exe,
        kotlinc_exe,
        kotlin_exe,
        java_version,
        kotlin_version,
    }
}

pub fn detect_toolchain() -> ToolchainInfo {
    let mut info = detect_toolchain_fast();

    if info.java_version.is_none() {
        info.java_version = info.java_exe.as_ref().and_then(|j| {
            Command::new(j).arg("-version").output().ok().and_then(|out| {
                let text = format!(
                    "{}\n{}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
                text.lines()
                    .map(str::trim)
                    .find(|l| !l.is_empty())
                    .map(|l| l.to_string())
            })
        });
    }

    if info.kotlin_version.is_none() {
        info.kotlin_version = info.kotlinc_exe.as_ref().and_then(|k| {
            let mut cmd = Command::new(k);
            if let Some(java) = &info.java_exe {
                if let Some(java_home) = java_home_for(java) {
                    cmd.env("JAVA_HOME", java_home);
                }
            }
            cmd.arg("-version").output().ok().and_then(|out| {
                let text = format!(
                    "{}\n{}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
                let line = text.lines().map(str::trim).find(|l| !l.is_empty())?;
                let ver = line
                    .split_whitespace()
                    .find(|tok| tok.chars().next().is_some_and(|c| c.is_ascii_digit()) && tok.contains('.'))
                    .unwrap_or(line);
                Some(ver.to_string())
            })
        });
    }

    info
}

fn read_java_release_file(java_exe: &PathBuf) -> Option<String> {
    let jh = java_home_for(java_exe)?;
    let content = std::fs::read_to_string(jh.join("release")).ok()?;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("JAVA_RUNTIME_VERSION=") {
            let v = rest.trim().trim_matches('"').trim();
            if !v.is_empty() {
                return Some(format!("Temurin {}", v));
            }
        }
    }
    None
}

fn read_kotlin_build_file(kotlinc_exe: &PathBuf) -> Option<String> {
    let canonical = std::fs::canonicalize(kotlinc_exe).unwrap_or_else(|_| kotlinc_exe.clone());
    let kh = canonical.parent()?.parent()?;
    let ver = std::fs::read_to_string(kh.join("build.txt")).ok()?;
    let trimmed = ver.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn java_home_for(java_exe: &PathBuf) -> Option<PathBuf> {
    let canonical = std::fs::canonicalize(java_exe).unwrap_or_else(|_| java_exe.clone());
    canonical.parent().and_then(|p| p.parent()).map(|p| p.to_path_buf())
}
