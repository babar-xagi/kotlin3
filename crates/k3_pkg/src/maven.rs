use crate::lockfile::{K3Lockfile, LockedPackage};
use crate::sha256::sha256_file;
use crate::toolchain::{home_dir, k3_home_dir};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MavenCoord {
    pub group: String,
    pub artifact: String,
    pub version: Option<String>,
}

impl MavenCoord {
    pub fn parse(spec: &str) -> Result<Self, String> {
        let parts: Vec<&str> = spec.trim().split(':').collect();
        match parts.as_slice() {
            [g, a] if !g.is_empty() && !a.is_empty() => Ok(Self {
                group: g.to_string(),
                artifact: a.to_string(),
                version: None,
            }),
            [g, a, v] if !g.is_empty() && !a.is_empty() && !v.is_empty() => Ok(Self {
                group: g.to_string(),
                artifact: a.to_string(),
                version: Some(v.to_string()),
            }),
            _ => Err(format!(
                "Invalid Maven coordinate '{}'; expected 'group:artifact' or 'group:artifact:version'",
                spec
            )),
        }
    }

    pub fn key(&self) -> String {
        format!("{}:{}", self.group, self.artifact)
    }
}

#[derive(Debug, Clone)]
pub struct PomDependency {
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub scope: String,
    pub optional: bool,
    pub exclusions: BTreeSet<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ParsedPom {
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub packaging: String,
    pub parent: Option<(String, String, String)>,
    pub properties: HashMap<String, String>,
    pub managed_versions: HashMap<String, String>,
    pub dependencies: Vec<PomDependency>,
}

pub fn maven_cache_dir() -> PathBuf {
    k3_home_dir().join("cache").join("maven")
}

pub fn artifact_rel_path(group: &str, artifact: &str, version: &str, ext: &str) -> PathBuf {
    let mut p = PathBuf::new();
    for seg in group.split('.') {
        p.push(seg);
    }
    p.push(artifact);
    p.push(version);
    p.push(format!("{}-{}.{}", artifact, version, ext));
    p
}

pub fn artifact_url(repo_base: &str, group: &str, artifact: &str, version: &str, ext: &str) -> String {
    let base = repo_base.trim_end_matches('/');
    let gp = group.replace('.', "/");
    format!("{}/{}/{}/{}/{}-{}.{}", base, gp, artifact, version, artifact, version, ext)
}

pub fn metadata_url(repo_base: &str, group: &str, artifact: &str) -> String {
    let base = repo_base.trim_end_matches('/');
    let gp = group.replace('.', "/");
    format!("{}/{}/{}/maven-metadata.xml", base, gp, artifact)
}

pub fn cached_jar_path_in(cache_dir: &Path, group: &str, artifact: &str, version: &str) -> PathBuf {
    cache_dir.join(artifact_rel_path(group, artifact, version, "jar"))
}

pub fn cached_jar_path(group: &str, artifact: &str, version: &str) -> PathBuf {
    cached_jar_path_in(&maven_cache_dir(), group, artifact, version)
}

pub fn m2_jar_path(group: &str, artifact: &str, version: &str) -> Option<PathBuf> {
    home_dir().map(|h| {
        h.join(".m2")
            .join("repository")
            .join(artifact_rel_path(group, artifact, version, "jar"))
    })
}

fn fetch_text_from_url(url: &str) -> Result<String, String> {
    if let Some(local_path) = url.strip_prefix("file://") {
        return fs::read_to_string(local_path)
            .map_err(|e| format!("Failed reading {}: {}", local_path, e));
    }
    let out = Command::new("curl")
        .args(["-fsSL", "--retry", "2", "--connect-timeout", "10", url])
        .output()
        .map_err(|e| format!("Failed executing curl for {}: {}", url, e))?;
    if !out.status.success() {
        return Err(format!(
            "HTTP fetch failed for {}: {}",
            url,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn download_file(url: &str, dest: &Path) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed creating cache directory {}: {}", parent.display(), e))?;
    }
    if let Some(local_path) = url.strip_prefix("file://") {
        fs::copy(local_path, dest)
            .map_err(|e| format!("Failed copying {} to {}: {}", local_path, dest.display(), e))?;
        return Ok(());
    }
    let tmp = dest.with_extension("part");
    let out = Command::new("curl")
        .args(["-fsSL", "--retry", "2", "--connect-timeout", "15", "-o"])
        .arg(&tmp)
        .arg(url)
        .output()
        .map_err(|e| format!("Failed executing curl for {}: {}", url, e))?;
    if !out.status.success() {
        let _ = fs::remove_file(&tmp);
        return Err(format!(
            "Download failed for {}: {}",
            url,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    fs::rename(&tmp, dest)
        .map_err(|e| format!("Failed moving {} to {}: {}", tmp.display(), dest.display(), e))?;
    Ok(())
}

/// Simple XML element extractor handling nested blocks cleanly for Maven POM / metadata XML.
#[derive(Debug, Clone)]
struct XmlNode {
    tag: String,
    text: String,
    children: Vec<XmlNode>,
}

impl XmlNode {
    fn child(&self, name: &str) -> Option<&XmlNode> {
        self.children.iter().find(|c| c.tag == name)
    }

    fn child_text(&self, name: &str) -> Option<String> {
        self.child(name).map(|c| c.text.trim().to_string()).filter(|s| !s.is_empty())
    }
}

fn strip_xml_comments_and_pi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while !rest.is_empty() {
        if let Some(start) = rest.find("<!--") {
            out.push_str(&rest[..start]);
            if let Some(end) = rest[start + 4..].find("-->") {
                rest = &rest[start + 4 + end + 3..];
            } else {
                break;
            }
        } else {
            out.push_str(rest);
            break;
        }
    }
    out
}

fn parse_xml(input: &str) -> Option<XmlNode> {
    let cleaned = strip_xml_comments_and_pi(input);
    let mut stack: Vec<XmlNode> = Vec::new();
    let mut root: Option<XmlNode> = None;
    let mut pos = 0;
    let bytes = cleaned.as_bytes();

    while pos < bytes.len() {
        if let Some(lt_rel) = cleaned[pos..].find('<') {
            let text_chunk = &cleaned[pos..pos + lt_rel];
            if let Some(top) = stack.last_mut() {
                top.text.push_str(text_chunk);
            }
            let tag_start = pos + lt_rel + 1;
            let Some(gt_rel) = cleaned[tag_start..].find('>') else {
                break;
            };
            let raw_tag = cleaned[tag_start..tag_start + gt_rel].trim();
            pos = tag_start + gt_rel + 1;

            if raw_tag.starts_with('?') || raw_tag.starts_with('!') {
                continue;
            }
            if let Some(closing) = raw_tag.strip_prefix('/') {
                let tag_name = closing.split_whitespace().next().unwrap_or("").to_string();
                while let Some(node) = stack.pop() {
                    let matched = node.tag == tag_name;
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(node);
                    } else {
                        root = Some(node);
                    }
                    if matched {
                        break;
                    }
                }
            } else {
                let self_closing = raw_tag.ends_with('/');
                let trimmed = raw_tag.trim_end_matches('/').trim();
                let tag_name = trimmed
                    .split(|c: char| c.is_whitespace())
                    .next()
                    .unwrap_or("")
                    .rsplit(':')
                    .next()
                    .unwrap_or("")
                    .to_string();
                let node = XmlNode {
                    tag: tag_name,
                    text: String::new(),
                    children: Vec::new(),
                };
                if self_closing {
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(node);
                    } else {
                        root = Some(node);
                    }
                } else {
                    stack.push(node);
                }
            }
        } else {
            if let Some(top) = stack.last_mut() {
                top.text.push_str(&cleaned[pos..]);
            }
            break;
        }
    }

    while let Some(node) = stack.pop() {
        if let Some(parent) = stack.last_mut() {
            parent.children.push(node);
        } else {
            root = Some(node);
        }
    }
    root
}

pub fn parse_latest_version_from_metadata(xml: &str) -> Option<String> {
    let root = parse_xml(xml)?;
    let versioning = root.child("versioning")?;
    if let Some(rel) = versioning.child_text("release") {
        return Some(rel);
    }
    if let Some(lat) = versioning.child_text("latest") {
        return Some(lat);
    }
    versioning
        .child("versions")?
        .children
        .iter()
        .filter(|c| c.tag == "version")
        .filter_map(|c| {
            let t = c.text.trim();
            if t.is_empty() { None } else { Some(t.to_string()) }
        })
        .last()
}

pub fn parse_pom(xml: &str) -> Result<ParsedPom, String> {
    let root = parse_xml(xml).ok_or_else(|| "Invalid POM XML".to_string())?;
    let mut pom = ParsedPom::default();

    if let Some(parent_node) = root.child("parent") {
        let pg = parent_node.child_text("groupId").unwrap_or_default();
        let pa = parent_node.child_text("artifactId").unwrap_or_default();
        let pv = parent_node.child_text("version").unwrap_or_default();
        if !pg.is_empty() && !pa.is_empty() && !pv.is_empty() {
            pom.parent = Some((pg.clone(), pa.clone(), pv.clone()));
            pom.properties.insert("project.parent.groupId".to_string(), pg);
            pom.properties.insert("project.parent.version".to_string(), pv);
        }
    }

    pom.group = root
        .child_text("groupId")
        .or_else(|| pom.parent.as_ref().map(|p| p.0.clone()))
        .unwrap_or_default();
    pom.artifact = root.child_text("artifactId").unwrap_or_default();
    pom.version = root
        .child_text("version")
        .or_else(|| pom.parent.as_ref().map(|p| p.2.clone()))
        .unwrap_or_default();
    pom.packaging = root
        .child_text("packaging")
        .unwrap_or_else(|| "jar".to_string());

    pom.properties.insert("project.groupId".to_string(), pom.group.clone());
    pom.properties.insert("project.artifactId".to_string(), pom.artifact.clone());
    pom.properties.insert("project.version".to_string(), pom.version.clone());

    if let Some(props) = root.child("properties") {
        for c in &props.children {
            pom.properties.insert(c.tag.clone(), c.text.trim().to_string());
        }
    }

    if let Some(dm) = root.child("dependencyManagement").and_then(|d| d.child("dependencies")) {
        for dep in dm.children.iter().filter(|c| c.tag == "dependency") {
            let g = resolve_props(&dep.child_text("groupId").unwrap_or_default(), &pom.properties);
            let a = resolve_props(&dep.child_text("artifactId").unwrap_or_default(), &pom.properties);
            let v = resolve_props(&dep.child_text("version").unwrap_or_default(), &pom.properties);
            if !g.is_empty() && !a.is_empty() && !v.is_empty() {
                pom.managed_versions.insert(format!("{}:{}", g, a), v);
            }
        }
    }

    if let Some(deps) = root.child("dependencies") {
        for dep in deps.children.iter().filter(|c| c.tag == "dependency") {
            let g = resolve_props(&dep.child_text("groupId").unwrap_or_default(), &pom.properties);
            let a = resolve_props(&dep.child_text("artifactId").unwrap_or_default(), &pom.properties);
            let mut v = resolve_props(&dep.child_text("version").unwrap_or_default(), &pom.properties);
            if v.is_empty() {
                if let Some(mv) = pom.managed_versions.get(&format!("{}:{}", g, a)) {
                    v = mv.clone();
                }
            }
            let scope = dep
                .child_text("scope")
                .unwrap_or_else(|| "compile".to_string());
            let optional = dep.child_text("optional").map(|s| s == "true").unwrap_or(false);
            let mut exclusions = BTreeSet::new();
            if let Some(excl_node) = dep.child("exclusions") {
                for ex in excl_node.children.iter().filter(|c| c.tag == "exclusion") {
                    if let (Some(eg), Some(ea)) = (ex.child_text("groupId"), ex.child_text("artifactId")) {
                        exclusions.insert(format!("{}:{}", eg, ea));
                    }
                }
            }
            if !g.is_empty() && !a.is_empty() {
                pom.dependencies.push(PomDependency {
                    group: g,
                    artifact: a,
                    version: v,
                    scope,
                    optional,
                    exclusions,
                });
            }
        }
    }

    Ok(pom)
}

fn resolve_props(input: &str, props: &HashMap<String, String>) -> String {
    let mut cur = input.trim().to_string();
    for _ in 0..4 {
        let mut next = cur.clone();
        while let Some(start) = next.find("${") {
            let Some(end_rel) = next[start + 2..].find('}') else {
                break;
            };
            let key = &next[start + 2..start + 2 + end_rel];
            if let Some(val) = props.get(key) {
                next = format!("{}{}{}", &next[..start], val, &next[start + 2 + end_rel + 1..]);
            } else {
                break;
            }
        }
        if next == cur {
            break;
        }
        cur = next;
    }
    cur
}

pub fn resolve_latest_version(
    group: &str,
    artifact: &str,
    repositories: &[String],
) -> Result<String, String> {
    for repo in repositories {
        let url = metadata_url(repo, group, artifact);
        if let Ok(xml) = fetch_text_from_url(&url) {
            if let Some(v) = parse_latest_version_from_metadata(&xml) {
                return Ok(v);
            }
        }
    }
    Err(format!(
        "Could not resolve latest version for {}:{} from repositories",
        group, artifact
    ))
}

fn fetch_pom_with_parent(
    group: &str,
    artifact: &str,
    version: &str,
    repositories: &[String],
    cache_dir: &Path,
    visited_parents: &mut BTreeSet<String>,
) -> Result<ParsedPom, String> {
    let cache_pom = cache_dir.join(artifact_rel_path(group, artifact, version, "pom"));
    let xml = if cache_pom.exists() {
        fs::read_to_string(&cache_pom)
            .map_err(|e| format!("Failed reading cached POM {}: {}", cache_pom.display(), e))?
    } else {
        let mut fetched = None;
        for repo in repositories {
            let url = artifact_url(repo, group, artifact, version, "pom");
            if let Ok(text) = fetch_text_from_url(&url) {
                if let Some(parent) = cache_pom.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let _ = fs::write(&cache_pom, &text);
                fetched = Some(text);
                break;
            }
        }
        fetched.ok_or_else(|| {
            format!("POM not found for {}:{}:{}", group, artifact, version)
        })?
    };

    let mut pom = parse_pom(&xml)?;
    if let Some((pg, pa, pv)) = pom.parent.clone() {
        let key = format!("{}:{}:{}", pg, pa, pv);
        if visited_parents.insert(key) {
            if let Ok(parent_pom) = fetch_pom_with_parent(&pg, &pa, &pv, repositories, cache_dir, visited_parents) {
                for (k, v) in parent_pom.properties {
                    pom.properties.entry(k).or_insert(v);
                }
                for (k, v) in parent_pom.managed_versions {
                    pom.managed_versions.entry(k).or_insert(v);
                }
                for dep in &mut pom.dependencies {
                    dep.group = resolve_props(&dep.group, &pom.properties);
                    dep.artifact = resolve_props(&dep.artifact, &pom.properties);
                    dep.version = resolve_props(&dep.version, &pom.properties);
                    if dep.version.is_empty() {
                        if let Some(mv) = pom.managed_versions.get(&format!("{}:{}", dep.group, dep.artifact)) {
                            dep.version = mv.clone();
                        }
                    }
                }
            }
        }
    }
    Ok(pom)
}

pub fn ensure_jar_cached(
    group: &str,
    artifact: &str,
    version: &str,
    repositories: &[String],
) -> Result<(PathBuf, String, String), String> {
    ensure_jar_cached_in(&maven_cache_dir(), group, artifact, version, repositories)
}

pub fn ensure_jar_cached_in(
    cache_dir: &Path,
    group: &str,
    artifact: &str,
    version: &str,
    repositories: &[String],
) -> Result<(PathBuf, String, String), String> {
    let dest = cached_jar_path_in(cache_dir, group, artifact, version);
    let default_url = artifact_url(
        repositories.first().map(String::as_str).unwrap_or("https://repo1.maven.org/maven2"),
        group,
        artifact,
        version,
        "jar",
    );

    if dest.exists() {
        let sha = sha256_file(&dest).map_err(|e| format!("Failed hashing {}: {}", dest.display(), e))?;
        return Ok((dest, sha, default_url));
    }

    if let Some(m2) = m2_jar_path(group, artifact, version) {
        if m2.exists() {
            if let Some(parent) = dest.parent() {
                let _ = fs::create_dir_all(parent);
            }
            fs::copy(&m2, &dest)
                .map_err(|e| format!("Failed copying from ~/.m2 {}: {}", m2.display(), e))?;
            let sha = sha256_file(&dest).map_err(|e| format!("Failed hashing {}: {}", dest.display(), e))?;
            return Ok((dest, sha, default_url));
        }
    }

    let mut last_err = String::new();
    for repo in repositories {
        let url = artifact_url(repo, group, artifact, version, "jar");
        match download_file(&url, &dest) {
            Ok(()) => {
                let sha = sha256_file(&dest)
                    .map_err(|e| format!("Failed hashing {}: {}", dest.display(), e))?;
                return Ok((dest, sha, url));
            }
            Err(e) => last_err = e,
        }
    }

    Err(format!(
        "Failed to download {}:{}:{}: {}",
        group, artifact, version, last_err
    ))
}

pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |s: &str| -> Vec<(bool, u64, String)> {
        s.split(|c: char| c == '.' || c == '-')
            .map(|part| match part.parse::<u64>() {
                Ok(n) => (true, n, String::new()),
                Err(_) => (false, 0, part.to_string()),
            })
            .collect()
    };
    let pa = parse(a);
    let pb = parse(b);
    let len = pa.len().max(pb.len());
    for i in 0..len {
        let ea = pa.get(i).cloned().unwrap_or((true, 0, String::new()));
        let eb = pb.get(i).cloned().unwrap_or((true, 0, String::new()));
        let ord = match (ea.0, eb.0) {
            (true, true) => ea.1.cmp(&eb.1),
            _ => ea.2.cmp(&eb.2),
        };
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
    }
    std::cmp::Ordering::Equal
}

pub fn resolve_dependencies_to_lockfile(
    kotlin_version: &str,
    deps: &BTreeMap<String, String>,
    dev_deps: &BTreeMap<String, String>,
    repositories: &[String],
) -> Result<K3Lockfile, String> {
    resolve_dependencies_to_lockfile_in(
        &maven_cache_dir(),
        kotlin_version,
        deps,
        dev_deps,
        repositories,
    )
}

pub fn resolve_dependencies_to_lockfile_in(
    cache_dir: &Path,
    kotlin_version: &str,
    deps: &BTreeMap<String, String>,
    dev_deps: &BTreeMap<String, String>,
    repositories: &[String],
) -> Result<K3Lockfile, String> {
    let mut selected: HashMap<String, (String, String, Vec<String>, BTreeSet<String>)> = HashMap::new();
    let mut queue: VecDeque<(String, String, String, String, BTreeSet<String>)> = VecDeque::new();

    for (coord_str, ver) in deps {
        let coord = MavenCoord::parse(coord_str)?;
        let resolved_ver = if ver.trim().is_empty() || ver == "latest" {
            resolve_latest_version(&coord.group, &coord.artifact, repositories)?
        } else {
            ver.clone()
        };
        queue.push_back((coord.group, coord.artifact, resolved_ver, "compile".to_string(), BTreeSet::new()));
    }

    for (coord_str, ver) in dev_deps {
        let coord = MavenCoord::parse(coord_str)?;
        let resolved_ver = if ver.trim().is_empty() || ver == "latest" {
            resolve_latest_version(&coord.group, &coord.artifact, repositories)?
        } else {
            ver.clone()
        };
        queue.push_back((coord.group, coord.artifact, resolved_ver, "test".to_string(), BTreeSet::new()));
    }

    let mut visited_coords: BTreeSet<String> = BTreeSet::new();

    while let Some((group, artifact, version, scope, exclusions)) = queue.pop_front() {
        let key = format!("{}:{}", group, artifact);
        if exclusions.contains(&key) || exclusions.contains(&format!("{}:*", group)) || exclusions.contains("*:*") {
            continue;
        }
        let full = format!("{}:{}", key, version);
        if !visited_coords.insert(full) {
            continue;
        }

        if let Some((existing_ver, _, _, _)) = selected.get(&key) {
            if compare_versions(&version, existing_ver) != std::cmp::Ordering::Greater {
                continue;
            }
        }

        let mut parent_guard = BTreeSet::new();
        let pom = fetch_pom_with_parent(&group, &artifact, &version, repositories, cache_dir, &mut parent_guard)
            .unwrap_or_default();

        let mut direct_deps = Vec::new();
        for dep in pom.dependencies {
            if dep.optional {
                continue;
            }
            if !matches!(dep.scope.as_str(), "" | "compile" | "runtime") {
                continue;
            }
            if dep.version.is_empty() || dep.version.contains("${") {
                continue;
            }
            let dep_key = format!("{}:{}", dep.group, dep.artifact);
            if exclusions.contains(&dep_key) {
                continue;
            }
            direct_deps.push(format!("{}:{}", dep_key, dep.version));
            let mut merged_exclusions = exclusions.clone();
            merged_exclusions.extend(dep.exclusions);
            queue.push_back((
                dep.group,
                dep.artifact,
                dep.version,
                scope.clone(),
                merged_exclusions,
            ));
        }

        selected.insert(key, (version, scope, direct_deps, exclusions));
    }

    let mut packages = Vec::new();
    for (key, (version, scope, dep_list, _)) in selected {
        let (group, artifact) = key.split_once(':').unwrap();
        let (_jar_path, sha256, url) = ensure_jar_cached_in(cache_dir, group, artifact, &version, repositories)?;
        packages.push(LockedPackage {
            group: group.to_string(),
            artifact: artifact.to_string(),
            version,
            sha256,
            url,
            scope,
            dependencies: dep_list,
        });
    }

    Ok(K3Lockfile {
        version: 1,
        kotlin_version: kotlin_version.to_string(),
        packages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pom_and_properties() {
        let xml = r#"
        <project>
          <groupId>com.example</groupId>
          <artifactId>demo</artifactId>
          <version>1.2.3</version>
          <properties>
            <dep.ver>2.0.0</dep.ver>
          </properties>
          <dependencies>
            <dependency>
              <groupId>org.jetbrains</groupId>
              <artifactId>annotations</artifactId>
              <version>${dep.ver}</version>
            </dependency>
            <dependency>
              <groupId>junit</groupId>
              <artifactId>junit</artifactId>
              <version>4.13.2</version>
              <scope>test</scope>
            </dependency>
          </dependencies>
        </project>
        "#;
        let pom = parse_pom(xml).unwrap();
        assert_eq!(pom.group, "com.example");
        assert_eq!(pom.dependencies.len(), 2);
        assert_eq!(pom.dependencies[0].version, "2.0.0");
        assert_eq!(pom.dependencies[1].scope, "test");
    }

    #[test]
    fn test_parse_maven_metadata() {
        let xml = r#"
        <metadata>
          <groupId>org.jetbrains.kotlin</groupId>
          <artifactId>kotlin-stdlib</artifactId>
          <versioning>
            <latest>2.4.21</latest>
            <release>2.4.21</release>
            <versions><version>2.4.20</version><version>2.4.21</version></versions>
          </versioning>
        </metadata>
        "#;
        assert_eq!(
            parse_latest_version_from_metadata(xml).as_deref(),
            Some("2.4.21")
        );
    }
}
