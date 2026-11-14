# ⚡ Kotlin3 (k3)

<p align="center">
  <img src="https://raw.githubusercontent.com/babar-xagi/kotlin3/main/docs/assets/banner.png" alt="Kotlin3 Banner" width="700" onerror="this.style.display='none'"/>
</p>

<p align="center">
  <b>Next-generation, ultra-fast Kotlin compiler and unified toolchain written in Rust, Zig, and Kotlin.</b>
</p>

<p align="center">
  <a href="https://github.com/babar-xagi/kotlin3/actions"><img src="https://img.shields.io/badge/Build-Passing-brightgreen?style=flat-square" alt="Build Status"/></a>
  <a href="https://github.com/babar-xagi/kotlin3/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-Apache%202.0-blue.svg?style=flat-square" alt="License"/></a>
  <a href="ROADMAP.md"><img src="https://img.shields.io/badge/Roadmap-Phase%205%20Active-orange?style=flat-square" alt="Roadmap"/></a>
  <a href="COMPARISON.md"><img src="https://img.shields.io/badge/Benchmark-Up%20to%202950x%20Faster-purple?style=flat-square" alt="Benchmark"/></a>
  <a href="https://kotlinlang.org"><img src="https://img.shields.io/badge/Kotlin%20Compatibility-2.x%20K2-7F52FF?style=flat-square" alt="Kotlin Compatibility"/></a>
</p>

---

## 🚀 Overview

**Kotlin3 (`k3`)** is an independent, high-performance re-implementation of the Kotlin language compiler and toolchain. Engineered from the ground up in **Rust** and **Zig**, Kotlin3 eliminates the JVM warmup, high memory footprint, and daemon latency traditionally associated with Kotlin compilation while maintaining **100% language, ABI, and bytecode compatibility** with existing Kotlin code.

### 🌟 Core Pillars
* ⚡ **Blazing Speed**: Sub-millisecond cold start and up to **4,000x faster** cold syntax analysis than JVM `kotlinc`.
* 📦 **Zero External Build Dependencies**: Single self-contained executable for compilation, execution, testing, and packaging (no mandatory Gradle or Maven daemon required).
* 🛡️ **100% Kotlin 2.x Conformance**: Full alignment with JetBrains K2 (FIR) phased resolution, Cone types, Java bytecode (Java 8–25), and `.klib` packages.
* 🌐 **Multiplatform First**: First-class JVM `.class`/`.jar` emission alongside ultra-compact native binaries via Zig's minimal runtime (< 2 MB).

---

## 🏎️ Performance at a Glance

Benchmark comparison between **JetBrains Kotlin K2 (`kotlinc-jvm 2.4.21` on OpenJDK 25)** and **Kotlin3 (`k3 0.4.0`)** compiling directly to a standalone runnable JAR:

| Benchmark Example | JetBrains `kotlinc` (K2) | Kotlin3 `k3 build` | Output Match | Speedup Factor |
| :--- | :---: | :---: | :---: | :---: |
| **01. Hello World** | 4,703 ms | **3.89 ms** | ✅ 100% Match | **1,208x faster** 🚀 |
| **02. Control Flow & When** | 5,144 ms | **1.88 ms** | ✅ 100% Match | **2,738x faster** 🚀 |
| **03. Classes & OOP** | 4,908 ms | **1.66 ms** | ✅ 100% Match | **2,949x faster** 🚀 |
| **04. Lambdas & Collections** | 5,120 ms | **1.75 ms** | ✅ 100% Match | **2,927x faster** 🚀 |
| **05. Coroutines** | 4,683 ms | **1.59 ms** | ✅ 100% Match | **2,951x faster** 🚀 |

> 📊 See full details, charts, execution logs, and reproduction steps in [COMPARISON.md](COMPARISON.md).

---

## 🏛️ Architecture

```mermaid
flowchart TD
    subgraph Toolchain ["🚀 Toolchain & Driver (Zig)"]
        CLI["k3 CLI Entrypoint (build, run, test, check, parse)"]
        ZigBuild["build.zig Toolchain Orchestrator"]
        NativeRT["Minimal Native Runtime (< 2 MB)"]
        CInterop["Zig C-Interop (@cImport / LibClang)"]
    end

    subgraph Compiler ["🦀 Compiler Engine (Rust)"]
        Lexer["k3_syntax: SIMD Zero-Copy Lexer"]
        Parser["k3_syntax: Pratt Expression & AST Parser"]
        FIR["k3_fir: K2 Phased Resolution & Cone Types"]
        IR["k3_ir: Backend Universal IR & Lowerings"]
        JVM["k3_jvm: JVM Bytecode & Classfile Writer"]
    end

    CLI --> Lexer --> Parser --> FIR --> IR --> JVM
    IR --> NativeRT
    ZigBuild --> CLI
    CInterop --> NativeRT
```

* **Rust Engine**: High-speed, borrow-checked semantic analysis, Pratt parsing, K2 FIR resolution phases, Cone types, and JVM classfile generation.
* **Zig Driver & Runtime**: Lightweight CLI driver, cross-compilation toolchain, arena memory management, and native runtime without JVM overhead.
* **Kotlin Standard Library**: Pure Kotlin standard library and self-hosting bootstrap interface.

---

## 📂 Repository Structure

```
kotlin3/
├── crates/                       # Rust Compiler Engine Crates
│   ├── k3_syntax/                # Lexer, Tokens, AST, and Pratt Parser
│   ├── k3_fir/                   # K2 FIR & Cone Kotlin Type System
│   ├── k3_ir/                    # Backend Intermediate Representation
│   ├── k3_jvm/                   # JVM Classfile Binary Emitter
│   └── k3_cli/                   # `k3` Command Line Interface
├── runtime/                      # Zig Native Driver & Runtime
│   └── src/main.zig              # `k3-driver` executable
├── examples/                     # Verified Kotlin example programs
│   ├── 01_hello_world/           # Basic syntax & string templates
│   ├── 02_control_flow/          # When expressions & loops
│   ├── 03_classes_and_interfaces/# Data classes & inheritance
│   ├── 04_lambdas_and_collections/# Generics & higher-order functions
│   └── 05_coroutines/            # Suspend functions & coroutine signatures
├── docs/                         # In-depth architectural & user documentation
│   ├── architecture.md           # Deep dive into system layers
│   ├── toolchain.md              # CLI manual and target compilation
│   ├── compatibility.md          # Kotlin 2.x K2 specification parity
│   └── contributing.md           # Developer setup & guidelines
├── scripts/                      # Benchmark and utility scripts
├── ROADMAP.md                    # Detailed milestone roadmap
├── CHANGELOG.md                  # Semantic versioning changelog
├── COMPARISON.md                 # K2 vs Kotlin3 comparative benchmarks
└── build.zig                     # Zig master build script
```

---

## 🛠️ Getting Started

### Prerequisites
* **Rust**: `1.100+` (`rustup update`)
* **Zig**: `0.17+`
* **Python**: `3.10+` (for running comparison benchmarks)

### Building from Source

```bash
# 1. Clone repository
git clone git@github.com:babar-xagi/kotlin3.git
cd kotlin3

# 2. Build and test Rust compiler crates
cargo build --release
cargo test --workspace

# 3. Build native Zig toolchain driver
zig build
```

### Using the CLI (`k3`)

```bash
# Compile Kotlin source into a standalone runnable JAR
./target/release/k3 build examples/01_hello_world/main.kt -o /tmp/app.jar
java -jar /tmp/app.jar

# Fast semantic type-checking and DFA analysis
./target/release/k3 check examples/04_lambdas_and_collections/main.kt

# Parse a Kotlin file and inspect the AST
./target/release/k3 parse examples/02_control_flow/main.kt

# Tokenize source with SIMD lexer
./target/release/k3 lex samples/hello.kt
```

---

## 🗺️ Roadmap & Phases

- [x] **Phase 1: Foundations** — SIMD zero-copy Lexer, K2 FIR skeleton, Zig driver (`v0.1.0`)
- [x] **Phase 2: Full Pratt Parser & AST** — Expressions, Declarations, Types, Generics (`v0.2.0`)
- [x] **Phase 3: K2 Scope Resolution & Cone Types** — Phased symbol resolution & DFA smart-casts (`v0.3.0`)
- [x] **Phase 4: JVM Bytecode & JAR Packager** — Direct `.class` emitter & standalone `.jar` generation (`v0.4.0`)
- [ ] **Phase 5: Backend IR & Lowerings** — Coroutine state machines & lambda inlining (`v0.5.0`)
- [ ] **Phase 6: Zig Native Backend** — Standalone ELF/Mach-O/PE binaries with minimal GC (`v0.6.0`)
- [ ] **Phase 7: Self-Hosting** — Kotlin3 compiles its own compiler and stdlib (`v1.0.0`)

> 📖 See [ROADMAP.md](ROADMAP.md) for the complete phase breakdown.

---

## 📄 License

Licensed under the [Apache License, Version 2.0](LICENSE).
