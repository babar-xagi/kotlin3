# Contributing to Kotlin3 (`k3`)

Thank you for helping improve Kotlin3 — a fast, zero-Gradle Kotlin CLI, package manager, compiler, and runtime built strictly in **Rust** and **Kotlin**.

---

## Repository Layout

```text
kotlin3/
├── Cargo.toml                  # Rust workspace manifest
├── crates/
│   ├── k3_syntax/              # Lexer (logos) & Kotlin AST parser
│   ├── k3_fir/                 # Multi-file K2-style semantic analyzer & DFA
│   ├── k3_ir/                  # Universal IR & lowering passes
│   ├── k3_jvm/                 # Direct JVM .class & JAR bytecode emitter
│   ├── k3_pkg/                 # k3.toml, k3.lock, Maven resolver, cache, toolchains
│   └── k3_cli/                 # Unified `k3` CLI binary
├── runtime/                    # Kotlin runtime (k3.runtime) & test runner (k3.test)
├── examples/                   # Standalone & k3.toml project examples (01..06)
├── benches/                    # JVM compute benchmarks
├── scripts/                    # Benchmark runners & distribution packaging
└── docs/                       # Documentation
```

---

## Development Setup

### Prerequisites
- **Rust Toolchain**: `rustc` and `cargo` (Edition 2021)
- **JDK**: Java 21+ or 25 LTS (`java`, `javac`, `jar`)
- **Kotlin Compiler**: Kotlin 2.4+ (`kotlinc`, `kotlin`) — or install both into `~/.k3/toolchains` via `k3 toolchain install`

### Build & Test Commands

```bash
# Run all Rust workspace unit tests (k3_syntax, k3_fir, k3_ir, k3_jvm, k3_pkg)
cargo test --workspace --offline

# Build optimized release binary
cargo build --release --offline

# Run Kotlin runtime test suite (runtime/tests/RuntimeTest.kt)
(cd runtime && ../target/release/k3 test)

# Check and test project workflow example
(cd examples/06_k3_project_and_runtime && ../../target/release/k3 check --strict && ../../target/release/k3 test)

# Verify examples 01..05 against kotlinc with strict bytecode verification
python3 scripts/benchmark_compare.py

# Run compute benchmarks
python3 scripts/benchmark_runner.py
```

---

## Coding Guidelines

1. **Pure Rust + Kotlin**: Do not introduce Gradle, Maven wrapper scripts, or C/Zig runtime dependencies into the core repository. `crates/k3_pkg` intentionally has zero external Cargo dependencies so builds work cleanly offline.
2. **Strict Bytecode Verification**: Any change to `crates/k3_jvm` must pass `java -Xverify:all -jar ...` on all examples and `benches/compute_benchmark.kt` with identical output/checksums to `kotlinc`.
3. **Truthful Docs & Benchmarks**: Keep documentation and benchmark numbers strictly tied to real, reproduced runs.
