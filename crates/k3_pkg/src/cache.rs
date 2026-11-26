use crate::sha256::Sha256;
use crate::toolchain::k3_home_dir;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn build_cache_dir() -> PathBuf {
    k3_home_dir().join("cache").join("builds")
}

pub fn runtime_cache_dir() -> PathBuf {
    k3_home_dir().join("cache").join("runtime")
}

pub fn compute_sources_hash(
    sources: &[PathBuf],
    extra_tags: &[&str],
) -> Result<String, String> {
    let mut hasher = Sha256::new();
    for tag in extra_tags {
        hasher.update(tag.as_bytes());
        hasher.update(b"\n");
    }

    let mut sorted = sources.to_vec();
    sorted.sort();
    for path in sorted {
        let content = fs::read(&path)
            .map_err(|e| format!("Failed reading {} for cache hash: {}", path.display(), e))?;
        hasher.update(path.to_string_lossy().as_bytes());
        hasher.update(b"\0");
        hasher.update(&content);
        hasher.update(b"\0");
    }
    Ok(hasher.finalize_hex())
}

pub fn cached_jar_lookup(hash: &str) -> Option<PathBuf> {
    let p = build_cache_dir().join(format!("{}.jar", hash));
    if p.exists() {
        Some(p)
    } else {
        None
    }
}

pub fn store_cached_jar(hash: &str, jar_path: &Path) -> Result<PathBuf, String> {
    let dir = build_cache_dir();
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed creating build cache dir {}: {}", dir.display(), e))?;
    let dest = dir.join(format!("{}.jar", hash));
    fs::copy(jar_path, &dest)
        .map_err(|e| format!("Failed writing build cache {}: {}", dest.display(), e))?;
    Ok(dest)
}

/// Assembles a self-contained runnable uber/fat JAR from `app_jar` and `classpath_jars`,
/// stripping signature files and merging `META-INF/services/*` entries.
pub fn build_fat_jar(
    jar_exe: &Path,
    app_jar: &Path,
    classpath_jars: &[PathBuf],
    main_class: Option<&str>,
    out_jar: &Path,
) -> Result<(), String> {
    let staging = std::env::temp_dir().join(format!("k3_fat_{}", std::process::id()));
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging)
        .map_err(|e| format!("Failed creating staging dir {}: {}", staging.display(), e))?;

    let mut services: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut all_jars: Vec<PathBuf> = classpath_jars.to_vec();
    all_jars.push(app_jar.to_path_buf());

    for jar in &all_jars {
        if !jar.exists() {
            continue;
        }
        let unpack = staging.join("unpack");
        let _ = fs::remove_dir_all(&unpack);
        fs::create_dir_all(&unpack).map_err(|e| e.to_string())?;

        let status = Command::new(jar_exe)
            .current_dir(&unpack)
            .arg("xf")
            .arg(jar)
            .status()
            .map_err(|e| format!("Failed running jar xf on {}: {}", jar.display(), e))?;
        if !status.success() {
            let _ = fs::remove_dir_all(&staging);
            return Err(format!("jar xf failed on {}", jar.display()));
        }

        let merged_root = staging.join("merged");
        merge_dir(&unpack, &merged_root, &mut services)?;
    }

    let merged_root = staging.join("merged");
    fs::create_dir_all(merged_root.join("META-INF")).map_err(|e| e.to_string())?;

    if !services.is_empty() {
        let svc_dir = merged_root.join("META-INF").join("services");
        fs::create_dir_all(&svc_dir).map_err(|e| e.to_string())?;
        for (name, lines) in services {
            let mut deduped = lines;
            deduped.sort();
            deduped.dedup();
            fs::write(svc_dir.join(name), deduped.join("\n") + "\n").map_err(|e| e.to_string())?;
        }
    }

    let manifest_path = staging.join("MANIFEST.MF");
    let mut manifest = String::from("Manifest-Version: 1.0\nCreated-By: Kotlin3 (k3)\n");
    if let Some(mc) = main_class {
        manifest.push_str(&format!("Main-Class: {}\n", mc.replace('/', ".")));
    }
    fs::write(&manifest_path, manifest).map_err(|e| e.to_string())?;

    if let Some(parent) = out_jar.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }

    let abs_out = if out_jar.is_absolute() {
        out_jar.to_path_buf()
    } else {
        std::env::current_dir().map_err(|e| e.to_string())?.join(out_jar)
    };

    let status = Command::new(jar_exe)
        .current_dir(&merged_root)
        .arg("cfm")
        .arg(&abs_out)
        .arg(&manifest_path)
        .arg(".")
        .status()
        .map_err(|e| format!("Failed running jar cfm: {}", e))?;

    let _ = fs::remove_dir_all(&staging);

    if !status.success() {
        return Err("jar cfm failed while assembling fat JAR".to_string());
    }
    Ok(())
}

fn merge_dir(
    src: &Path,
    dst: &Path,
    services: &mut BTreeMap<String, Vec<String>>,
) -> Result<(), String> {
    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let dst_path = dst.join(&name);

        if path.is_dir() {
            fs::create_dir_all(&dst_path).map_err(|e| e.to_string())?;
            merge_dir(&path, &dst_path, services)?;
        } else {
            let rel_str = dst_path.to_string_lossy();
            if rel_str.contains("META-INF/services/") || rel_str.contains("META-INF\\services\\") {
                if let Ok(content) = fs::read_to_string(&path) {
                    let entry_lines = services.entry(name_str.to_string()).or_default();
                    for l in content.lines() {
                        let t = l.trim();
                        if !t.is_empty() && !t.starts_with('#') {
                            entry_lines.push(t.to_string());
                        }
                    }
                }
                continue;
            }
            if (rel_str.contains("META-INF/") || rel_str.contains("META-INF\\"))
                && (name_str.ends_with(".SF")
                    || name_str.ends_with(".DSA")
                    || name_str.ends_with(".RSA")
                    || name_str == "MANIFEST.MF")
            {
                continue;
            }
            if let Some(p) = dst_path.parent() {
                fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
            fs::copy(&path, &dst_path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
