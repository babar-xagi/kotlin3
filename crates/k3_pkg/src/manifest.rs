use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageMeta {
    pub name: String,
    pub version: String,
    pub kotlin: String,
    pub entry: String,
    pub main_class: Option<String>,
}

impl Default for PackageMeta {
    fn default() -> Self {
        Self {
            name: "app".to_string(),
            version: "0.1.0".to_string(),
            kotlin: "2.4.21".to_string(),
            entry: "src/Main.kt".to_string(),
            main_class: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct K3Manifest {
    pub package: PackageMeta,
    pub dependencies: BTreeMap<String, String>,
    pub dev_dependencies: BTreeMap<String, String>,
    pub scripts: BTreeMap<String, String>,
    pub repositories: Vec<String>,
}

impl Default for K3Manifest {
    fn default() -> Self {
        Self {
            package: PackageMeta::default(),
            dependencies: BTreeMap::new(),
            dev_dependencies: BTreeMap::new(),
            scripts: BTreeMap::new(),
            repositories: vec!["https://repo1.maven.org/maven2".to_string()],
        }
    }
}

impl K3Manifest {
    pub fn new(name: &str, kotlin_version: &str) -> Self {
        let mut m = Self::default();
        m.package.name = name.to_string();
        m.package.kotlin = kotlin_version.to_string();
        m
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
        Self::parse(&content)
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        fs::write(path, self.to_toml_string())
            .map_err(|e| format!("Failed to write {}: {}", path.display(), e))
    }

    pub fn parse(input: &str) -> Result<Self, String> {
        let mut manifest = Self::default();
        let mut custom_repos: Vec<String> = Vec::new();
        let mut section = String::new();

        for (line_no, raw_line) in input.lines().enumerate() {
            let line = strip_comment(raw_line).trim();
            if line.is_empty() {
                continue;
            }

            if line.starts_with('[') && line.ends_with(']') {
                section = line[1..line.len() - 1].trim().to_string();
                continue;
            }

            let Some((raw_key, raw_val)) = line.split_once('=') else {
                return Err(format!("k3.toml:{}: expected key = value", line_no + 1));
            };

            let key = unquote(raw_key.trim());
            let val = raw_val.trim();

            match section.as_str() {
                "package" => {
                    let s = unquote(val);
                    match key.as_str() {
                        "name" => manifest.package.name = s,
                        "version" => manifest.package.version = s,
                        "kotlin" => manifest.package.kotlin = s,
                        "entry" => manifest.package.entry = s,
                        "main_class" | "main-class" => {
                            manifest.package.main_class = if s.is_empty() { None } else { Some(s) }
                        }
                        _ => {}
                    }
                }
                "dependencies" => {
                    manifest.dependencies.insert(key, unquote(val));
                }
                "dev-dependencies" | "dev_dependencies" => {
                    manifest.dev_dependencies.insert(key, unquote(val));
                }
                "scripts" => {
                    manifest.scripts.insert(key, unquote(val));
                }
                "repositories" => {
                    if key == "urls" && val.starts_with('[') && val.ends_with(']') {
                        for item in parse_string_array(val) {
                            custom_repos.push(item);
                        }
                    } else {
                        custom_repos.push(unquote(val));
                    }
                }
                "" => {}
                _ => {}
            }
        }

        if !custom_repos.is_empty() {
            manifest.repositories = custom_repos;
        }
        Ok(manifest)
    }

    pub fn to_toml_string(&self) -> String {
        let mut out = String::new();
        out.push_str("[package]\n");
        out.push_str(&format!("name = \"{}\"\n", escape_toml(&self.package.name)));
        out.push_str(&format!("version = \"{}\"\n", escape_toml(&self.package.version)));
        out.push_str(&format!("kotlin = \"{}\"\n", escape_toml(&self.package.kotlin)));
        out.push_str(&format!("entry = \"{}\"\n", escape_toml(&self.package.entry)));
        if let Some(mc) = &self.package.main_class {
            out.push_str(&format!("main_class = \"{}\"\n", escape_toml(mc)));
        }

        out.push_str("\n[dependencies]\n");
        for (coord, ver) in &self.dependencies {
            out.push_str(&format!("\"{}\" = \"{}\"\n", escape_toml(coord), escape_toml(ver)));
        }

        if !self.dev_dependencies.is_empty() {
            out.push_str("\n[dev-dependencies]\n");
            for (coord, ver) in &self.dev_dependencies {
                out.push_str(&format!("\"{}\" = \"{}\"\n", escape_toml(coord), escape_toml(ver)));
            }
        }

        if !self.scripts.is_empty() {
            out.push_str("\n[scripts]\n");
            for (name, cmd) in &self.scripts {
                out.push_str(&format!("{} = \"{}\"\n", name, escape_toml(cmd)));
            }
        }

        if self.repositories.len() != 1
            || self.repositories[0] != "https://repo1.maven.org/maven2"
        {
            out.push_str("\n[repositories]\n");
            let items: Vec<String> = self
                .repositories
                .iter()
                .map(|r| format!("\"{}\"", escape_toml(r)))
                .collect();
            out.push_str(&format!("urls = [{}]\n", items.join(", ")));
        }

        out
    }

    pub fn add_dependency(&mut self, coord: &str, version: &str, dev: bool) {
        if dev {
            self.dev_dependencies
                .insert(coord.to_string(), version.to_string());
        } else {
            self.dependencies
                .insert(coord.to_string(), version.to_string());
        }
    }

    pub fn remove_dependency(&mut self, coord: &str) -> bool {
        let removed_main = self.dependencies.remove(coord).is_some();
        let removed_dev = self.dev_dependencies.remove(coord).is_some();
        removed_main || removed_dev
    }
}

pub fn strip_comment(line: &str) -> &str {
    let mut in_quotes = false;
    let mut escaped = false;
    for (idx, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_quotes => escaped = true,
            '"' => in_quotes = !in_quotes,
            '#' if !in_quotes => return &line[..idx],
            _ => {}
        }
    }
    line
}

pub fn unquote(s: &str) -> String {
    let t = s.trim();
    if t.len() >= 2 && ((t.starts_with('"') && t.ends_with('"')) || (t.starts_with('\'') && t.ends_with('\''))) {
        t[1..t.len() - 1].replace("\\\"", "\"").replace("\\\\", "\\")
    } else {
        t.to_string()
    }
}

fn escape_toml(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

pub fn parse_string_array(s: &str) -> Vec<String> {
    let inner = s.trim();
    let inner = inner.strip_prefix('[').unwrap_or(inner);
    let inner = inner.strip_suffix(']').unwrap_or(inner);
    inner
        .split(',')
        .map(|part| unquote(part))
        .filter(|part| !part.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_roundtrip() {
        let mut m = K3Manifest::new("hello-k3", "2.4.21");
        m.add_dependency("org.jetbrains.kotlinx:kotlinx-coroutines-core", "1.10.2", false);
        m.scripts.insert("greet".to_string(), "src/Main.kt".to_string());

        let toml = m.to_toml_string();
        let parsed = K3Manifest::parse(&toml).expect("manifest parse should succeed");
        assert_eq!(parsed.package.name, "hello-k3");
        assert_eq!(parsed.package.kotlin, "2.4.21");
        assert_eq!(
            parsed.dependencies.get("org.jetbrains.kotlinx:kotlinx-coroutines-core").map(String::as_str),
            Some("1.10.2")
        );
        assert_eq!(parsed.scripts.get("greet").map(String::as_str), Some("src/Main.kt"));
    }
}
