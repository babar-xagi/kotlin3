# Kotlin3 (`k3`) v0.7.0

A fast, **zero-Gradle, zero-Maven** Kotlin CLI, package manager, compiler frontend, and runtime written in **Rust and Kotlin** (inspired by `uv` and `bun`).

`k3` eliminates Gradle/Maven daemon overhead and multi-file build boilerplate with a single native CLI, declarative `k3.toml` + `k3.lock` dependency management, content-addressed JAR caching, a fast Rust direct-to-JVM bytecode path for standalone Kotlin subsets, seamless integration with official `kotlinc` (`2.4.21`), and a batteries-included Kotlin runtime (`k3.runtime` & `k3.test`).

---

## Why Kotlin3 (`k3`)?

- **Zero Gradle / Zero Maven**: Replace `build.gradle.kts`, `settings.gradle.kts`, `gradle-wrapper.properties`, and XML `pom.xml` build scripts with a clean 10-line `k3.toml` and deterministic SHA-256 `k3.lock`.
- **Single-Binary Workflow (`uv` / `bun` style)**:
  - `k3 init`, `k3 add <group:artifact[:version]>`, `k3 remove`, `k3 sync`
  - `k3 run` (cached builds, `--fast` or `--kotlinc`, named `[scripts]`)
  - `k3 build` (thin JARs or self-contained `--fat` uber-JARs with merged `META-INF/services`)
  - `k3 test` (discovers `tests/**/*.kt` and runs `k3.test`)
  - `k3 check` (fast Rust K2-style FIR semantic checker, or `--strict` for `kotlinc`)
  - `k3 toolchain` / `k3 cache` / `k3 info`
- **Dual Compilation Engine**:
  - **Fast Rust Direct JVM Compiler (`--fast` / auto fast-path)**: `k3_syntax -> k3_fir -> k3_jvm` compiles supported single-file JVM Kotlin subsets in **~1.7–4.0 ms** with `java -Xverify:all`-verified Java 11 classfiles.
  - **Full Kotlin 2.4.21 Engine (`--kotlinc` / automatic for projects, imports, & Maven deps)**: Automatically delegates multi-file projects, full Kotlin stdlib/generics/coroutines, `k3-runtime`, and Maven Central dependencies to `kotlinc` with SHA-256 content-addressed caching under `~/.k3/cache`.
- **Batteries-Included Kotlin Runtime (`runtime/`)**:
  - `k3.runtime`: `K3Env`, `K3Time`, `K3Fs`, `K3Process`, `K3Http`, `K3Async` (Java virtual threads), `K3Json`
  - `k3.test`: `runTests`, `test`, `expectEquals`, `expectTrue`, `expectThrows`

---

## Quick Start

### 1. Prerequisites & Toolchains

Install Rust (`cargo`), OpenJDK (Temurin 25 LTS tested), and Kotlin `2.4.21`, or inspect/install managed toolchains directly via `k3`:

```bash
# Build k3 CLI
cargo build --release

# Install k3 on PATH (optional)
mkdir -p ~/.local/bin
cp target/release/k3 ~/.local/bin/k3

# Check or install JDK & Kotlin compiler into ~/.k3/toolchains
k3 toolchain status
k3 toolchain install
```

### 2. Run a Single Kotlin File Instantly

```bash
k3 run examples/01_hello_world/main.kt
k3 build --fast examples/02_control_flow/main.kt -o /tmp/control_flow.jar
java -Xverify:all -jar /tmp/control_flow.jar
```

### 3. Create & Run a Zero-Gradle Project

```bash
k3 init my-app
cd my-app
k3 check
k3 run
k3 test

# Add a Maven Central dependency (locks SHA-256 into k3.lock)
k3 add org.json:json

# Build a standalone runnable uber-JAR
k3 build --fat -o dist/my-app.jar
java -jar dist/my-app.jar
```

Example [`examples/06_k3_project_and_runtime/k3.toml`](examples/06_k3_project_and_runtime/k3.toml):

```toml
[package]
name = "k3-demo"
version = "0.7.0"
kotlin = "2.4.21"
entry = "src/Main.kt"

[dependencies]

[scripts]
demo = "src/Main.kt"
```

---

## Workspace Architecture

`kotlin3` is organized strictly in **Rust** and **Kotlin**:

| Component | Language | Purpose |
| :--- | :--- | :--- |
| [`crates/k3_syntax`](crates/k3_syntax) | Rust | `logos` lexer (with string interpolation & triple quotes) and Pratt/recursive-descent Kotlin AST parser |
| [`crates/k3_fir`](crates/k3_fir) | Rust | Multi-file K2-style Frontend IR resolver, subtyping, smart-cast DFA, suspend/return/abstract checkers |
| [`crates/k3_ir`](crates/k3_ir) | Rust | Universal Backend IR and lowering passes (`inlining`, `default_args`, `coroutines`) |
| [`crates/k3_jvm`](crates/k3_jvm) | Rust | Direct JVM `.class` bytecode & runnable JAR emitter (Java 11 classfile v55, synthetic lambda classes, exception tables) |
| [`crates/k3_pkg`](crates/k3_pkg) | Rust | `k3.toml` manifest, `k3.lock` lockfile, Maven Central POM/metadata resolver, SHA-256 verifier, fat-JAR builder, toolchain/cache manager |
| [`crates/k3_cli`](crates/k3_cli) | Rust | Unified `k3` binary (`clap`) routing fast-path, `kotlinc`, caching, tests, and package management |
| [`runtime/`](runtime) | Kotlin | `k3.runtime` and `k3.test` standard utilities compiled and cached into `k3-runtime.jar` |

---

## Verification & Benchmarks

Verified on Linux x86_64 with OpenJDK `25.0.5.1+2-LTS` (Temurin) and Kotlin `2.4.21`:

- **Rust workspace unit tests** (`cargo test --workspace --offline`): **27 passed, 0 failed** (`k3_syntax`: 6, `k3_fir`: 7, `k3_ir`: 3, `k3_jvm`: 5, `k3_pkg`: 6).
- **Kotlin runtime tests** (`cd runtime && k3 test`): **5 passed, 0 failed** (`62.63`–`84.70 ms`).
- **Project example workflow** (`examples/06_k3_project_and_runtime`): `k3 check --strict`, `k3 run`, `k3 test` (1/1 passed), and `k3 build --fat` (5.5 MB runnable fat JAR verified with `java -Xverify:all -jar`).
- **Examples `01`–`05` parity (`benchmark_compare.py`)**: All 5 examples pass `java -Xverify:all` with identical stdout to `kotlinc 2.4.21`, compiling in **1.75–3.94 ms** via `k3 --fast` vs **4707–5193 ms** via cold `kotlinc -include-runtime`.
- **Compute benchmark (`benches/compute_benchmark.kt`)**: Median compile **2.63 ms** (`k3 --fast`, `7.4 KB` pure-JDK JAR) vs **6460.5 ms** (`kotlinc 2.4.21`, `5615.0 KB` runtime-bundled JAR); median runtime **306.02 ms** (`k3 --fast`) vs **343.51 ms** (`kotlinc 2.4.21`) with identical checksums across ALU, memory, sieve, matrix, and fractal tests.

See [benchmark.md](benchmark.md), [COMPARISON.md](COMPARISON.md), and the [docs/](docs/user_guide.md) directory for details.

---

## Documentation

- [User Guide](docs/user_guide.md)
- [Architecture Overview](docs/architecture.md)
- [CLI & Toolchain Reference](docs/toolchain.md)
- [Compatibility Matrix](docs/compatibility.md)
- [Error Codes Reference](docs/errors.md)
- [Benchmarks (`benchmark.md`)](benchmark.md) & [Compiler Comparison (`COMPARISON.md`)](COMPARISON.md)
- [Contributing](docs/contributing.md), [Changelog](CHANGELOG.md), and [Roadmap](ROADMAP.md)
