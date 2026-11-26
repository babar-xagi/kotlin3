# Kotlin3 CLI & Toolchain Reference

`k3` (`0.7.0`) provides a unified single-binary interface inspired by `uv` and `bun` for managing Kotlin toolchains, dependencies, compilation, testing, and packaging without Gradle or Maven.

---

## Command Summary

| Command | Usage | Description |
| :--- | :--- | :--- |
| `init` | `k3 init [DIR] [--name NAME]` | Scaffold `k3.toml`, `src/Main.kt`, and `tests/MainTest.kt`. |
| `add` | `k3 add <group:artifact[:version]> [--dev]` | Resolve version from Maven Central (latest if omitted), add to `k3.toml`, sync JARs, and write `k3.lock`. |
| `remove` | `k3 remove <group:artifact>` | Remove dependency from `k3.toml` and update `k3.lock`. |
| `sync` | `k3 sync [--refresh]` | Resolve & download all `k3.toml` dependencies into `~/.k3/cache/maven` and update `k3.lock`. |
| `run` | `k3 run [TARGET] [--fast \| --kotlinc] [--no-cache] [-- args...]` | Compile (cached by SHA-256) and run a `.kt` file, current `k3.toml` project, or named `[scripts]` entry. |
| `build` | `k3 build [FILE] [-o PATH] [--fast \| --kotlinc] [--fat]` | Build a runnable thin JAR or a self-contained `--fat` uber-JAR (bundling `kotlin-stdlib`, `k3-runtime`, and Maven JARs). |
| `test` | `k3 test [FILTER]` | Discover `tests/**/*.kt`, compile with `src/` and `k3-runtime`, and execute each suite via `k3.test`. |
| `check` | `k3 check [TARGET] [--strict]` | Run fast Rust syntax + multi-file K2 FIR semantic checks (add `--strict` to also run `kotlinc`). |
| `toolchain` | `k3 toolchain [status \| install]` | Show detected JDK/Kotlin binaries & versions, or download/install Temurin JDK + latest Kotlin into `~/.k3/toolchains`. |
| `cache` | `k3 cache [dir \| status \| clean]` | Show cache path, inspect cached builds/runtime/Maven artifact sizes, or clean `~/.k3/cache`. |
| `info` | `k3 info` | Show active JDK, Kotlin, project name/version, and dependency counts. |
| `ir` / `parse` / `lex` | `k3 <ir\|parse\|lex> <FILE>` | Compiler debugging commands printing lowered IR, parsed AST, or lexed tokens. |

---

## Manifest (`k3.toml`) & Lockfile (`k3.lock`)

Example `k3.toml`:

```toml
[package]
name = "my-app"
version = "0.7.0"
kotlin = "2.4.21"
entry = "src/Main.kt"

[dependencies]
"org.json:json" = "20260814"

[scripts]
dev = "src/Main.kt"
```

`k3.lock` records every resolved direct and transitive package with its exact version, scope, SHA-256 hash, and download URL so repeated `k3 sync` / `k3 run` invocations are reproducible.

---

## Cache & Toolchain Directory Layout

By default, `k3` stores shared state under `~/.k3` (override via `K3_HOME`):

```text
~/.k3/
├── toolchains/
│   ├── jdk/          # Installed OpenJDK (when installed via `k3 toolchain install`)
│   └── kotlinc/      # Installed Kotlin compiler
└── cache/
    ├── builds/       # Content-addressed compiled JARs (<sha256>.jar)
    ├── runtime/      # Version/hash-keyed k3-runtime.jar builds
    └── maven/        # Downloaded Maven POMs and JAR artifacts (plus ~/.m2/repository fallback)
```

For fast startup (~0.05 ms instead of spawning JVM tool `--version` checks), `k3` reads `<JDK_HOME>/release` and `<KOTLIN_HOME>/build.txt` first when inspecting toolchains.
