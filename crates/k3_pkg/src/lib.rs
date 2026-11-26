pub mod cache;
pub mod lockfile;
pub mod manifest;
pub mod maven;
pub mod runtime_support;
pub mod sha256;
pub mod toolchain;

pub use cache::{build_cache_dir, build_fat_jar, cached_jar_lookup, compute_sources_hash, runtime_cache_dir, store_cached_jar};
pub use lockfile::{K3Lockfile, LockedPackage};
pub use manifest::K3Manifest;
pub use maven::{cached_jar_path, ensure_jar_cached, resolve_dependencies_to_lockfile, resolve_latest_version, MavenCoord};
pub use runtime_support::{ensure_runtime_jar, runtime_sources_hash};
pub use sha256::{sha256_bytes, sha256_file};
pub use toolchain::{detect_toolchain, detect_toolchain_fast, find_jdk_tool, find_kotlin_tool, java_home_for, k3_home_dir, ToolchainInfo};

use std::fs;
use std::path::{Path, PathBuf};

/// Scaffolds a zero-Gradle, zero-Maven Kotlin project with `k3.toml`, `src/Main.kt`, and `tests/MainTest.kt`.
pub fn init_project(dir: &Path, project_name: Option<&str>, kotlin_version: &str) -> Result<K3Manifest, String> {
    let manifest_path = dir.join("k3.toml");
    if manifest_path.exists() {
        return Err(format!("{} already exists", manifest_path.display()));
    }

    let inferred_name = project_name
        .map(str::to_string)
        .or_else(|| dir.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "kotlin3-app".to_string());

    let src_dir = dir.join("src");
    let tests_dir = dir.join("tests");
    fs::create_dir_all(&src_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&tests_dir).map_err(|e| e.to_string())?;

    let manifest = K3Manifest::new(&inferred_name, kotlin_version);
    manifest.save(&manifest_path)?;

    let main_kt = src_dir.join("Main.kt");
    if !main_kt.exists() {
        fs::write(
            &main_kt,
            format!(
                r#"package app

fun greet(target: String): String = "Hello from {name}, $target!"

fun main() {{
    println(greet("Kotlin3"))
}}
"#,
                name = inferred_name
            ),
        )
        .map_err(|e| e.to_string())?;
    }

    let test_kt = tests_dir.join("MainTest.kt");
    if !test_kt.exists() {
        fs::write(
            &test_kt,
            format!(
                r#"package app

import k3.test.expectEquals
import k3.test.runTests
import k3.test.test

fun main() {{
    runTests("MainTest") {{
        test("greet formats message") {{
            expectEquals("Hello from {name}, Kotlin3!", greet("Kotlin3"))
        }}
    }}
}}
"#,
                name = inferred_name
            ),
        )
        .map_err(|e| e.to_string())?;
    }

    let gitignore = dir.join(".gitignore");
    if !gitignore.exists() {
        fs::write(&gitignore, ".k3/\n*.jar\n*.class\n").map_err(|e| e.to_string())?;
    }

    Ok(manifest)
}

/// Synchronizes dependencies from `k3.toml` into `k3.lock` and downloads missing JARs into the global cache.
pub fn sync_project(project_dir: &Path, refresh: bool) -> Result<(K3Manifest, K3Lockfile, Vec<PathBuf>), String> {
    sync_project_with_cache(project_dir, refresh, &maven::maven_cache_dir())
}

pub fn sync_project_with_cache(
    project_dir: &Path,
    refresh: bool,
    cache_dir: &Path,
) -> Result<(K3Manifest, K3Lockfile, Vec<PathBuf>), String> {
    let manifest_path = project_dir.join("k3.toml");
    let manifest = K3Manifest::load(&manifest_path)?;
    let lock_path = project_dir.join("k3.lock");

    let lock = if lock_path.exists() && !refresh {
        let existing = K3Lockfile::load(&lock_path)?;
        if lock_matches_manifest(&existing, &manifest) {
            for pkg in &existing.packages {
                let (jar_path, sha, _) = maven::ensure_jar_cached_in(
                    cache_dir,
                    &pkg.group,
                    &pkg.artifact,
                    &pkg.version,
                    &manifest.repositories,
                )?;
                if !pkg.sha256.is_empty() && pkg.sha256 != sha {
                    return Err(format!(
                        "Checksum mismatch for {}: lockfile has {}, cache has {} ({})",
                        pkg.full_coordinate(),
                        pkg.sha256,
                        sha,
                        jar_path.display()
                    ));
                }
            }
            existing
        } else {
            let updated = maven::resolve_dependencies_to_lockfile_in(
                cache_dir,
                &manifest.package.kotlin,
                &manifest.dependencies,
                &manifest.dev_dependencies,
                &manifest.repositories,
            )?;
            updated.save(&lock_path)?;
            updated
        }
    } else {
        let fresh = maven::resolve_dependencies_to_lockfile_in(
            cache_dir,
            &manifest.package.kotlin,
            &manifest.dependencies,
            &manifest.dev_dependencies,
            &manifest.repositories,
        )?;
        fresh.save(&lock_path)?;
        fresh
    };

    let jars = lock
        .packages
        .iter()
        .map(|p| maven::cached_jar_path_in(cache_dir, &p.group, &p.artifact, &p.version))
        .collect();
    Ok((manifest, lock, jars))
}

fn lock_matches_manifest(lock: &K3Lockfile, manifest: &K3Manifest) -> bool {
    for (coord, ver) in manifest.dependencies.iter().chain(manifest.dev_dependencies.iter()) {
        let Ok(parsed) = MavenCoord::parse(coord) else {
            return false;
        };
        let found = lock.packages.iter().find(|p| p.group == parsed.group && p.artifact == parsed.artifact);
        match found {
            None => return false,
            Some(pkg) => {
                if !ver.is_empty() && ver != "latest" && pkg.version != *ver {
                    return false;
                }
            }
        }
    }
    true
}

pub fn lockfile_jars(lock: &K3Lockfile, include_dev: bool) -> Vec<PathBuf> {
    lock.packages
        .iter()
        .filter(|p| include_dev || p.scope != "test")
        .map(|p| cached_jar_path(&p.group, &p.artifact, &p.version))
        .collect()
}

pub fn collect_kt_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_kt_recursive(dir, &mut files);
    files.sort();
    files
}

fn collect_kt_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name();
            let s = name.to_string_lossy();
            if s.starts_with('.') || s == "target" || s == "build" || s == "dist" {
                continue;
            }
            collect_kt_recursive(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("kt") {
            out.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_init_and_offline_maven_sync() {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("k3_pkg_test_{}", nanos));
        let proj = root.join("proj");
        let repo = root.join("repo");
        fs::create_dir_all(&proj).unwrap();

        // Create a local Maven repo with parent POM + transitive dependency + JARs
        let parent_dir = repo.join("com/acme/parent/1.0.0");
        let app_lib_dir = repo.join("com/acme/lib-a/1.2.0");
        let trans_dir = repo.join("com/acme/lib-b/2.0.0");
        fs::create_dir_all(&parent_dir).unwrap();
        fs::create_dir_all(&app_lib_dir).unwrap();
        fs::create_dir_all(&trans_dir).unwrap();

        fs::write(
            parent_dir.join("parent-1.0.0.pom"),
            r#"<project>
                <groupId>com.acme</groupId><artifactId>parent</artifactId><version>1.0.0</version>
                <properties><b.ver>2.0.0</b.ver></properties>
            </project>"#,
        ).unwrap();
        fs::write(
            app_lib_dir.join("lib-a-1.2.0.pom"),
            r#"<project>
                <parent><groupId>com.acme</groupId><artifactId>parent</artifactId><version>1.0.0</version></parent>
                <artifactId>lib-a</artifactId><version>1.2.0</version>
                <dependencies>
                    <dependency><groupId>com.acme</groupId><artifactId>lib-b</artifactId><version>${b.ver}</version></dependency>
                </dependencies>
            </project>"#,
        ).unwrap();
        fs::write(
            trans_dir.join("lib-b-2.0.0.pom"),
            r#"<project><groupId>com.acme</groupId><artifactId>lib-b</artifactId><version>2.0.0</version></project>"#,
        ).unwrap();
        fs::write(app_lib_dir.join("lib-a-1.2.0.jar"), b"JAR_A").unwrap();
        fs::write(trans_dir.join("lib-b-2.0.0.jar"), b"JAR_B").unwrap();

        let mut manifest = init_project(&proj, Some("demo-app"), "2.4.21").unwrap();
        assert!(proj.join("k3.toml").exists());
        assert!(proj.join("src/Main.kt").exists());
        assert!(proj.join("tests/MainTest.kt").exists());

        manifest.repositories = vec![format!("file://{}", repo.display())];
        manifest.add_dependency("com.acme:lib-a", "1.2.0", false);
        manifest.save(&proj.join("k3.toml")).unwrap();

        let cache = root.join("cache");
        let (_, lock, jars) = sync_project_with_cache(&proj, true, &cache).unwrap();
        assert_eq!(lock.packages.len(), 2);
        assert!(lock.packages.iter().any(|p| p.artifact == "lib-a" && p.version == "1.2.0"));
        assert!(lock.packages.iter().any(|p| p.artifact == "lib-b" && p.version == "2.0.0"));
        assert_eq!(jars.len(), 2);
        assert!(proj.join("k3.lock").exists());

        let _ = fs::remove_dir_all(&root);
    }
}
