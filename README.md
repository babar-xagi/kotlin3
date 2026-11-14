# Kotlin3 (k3)

Next-generation, ultra-fast Kotlin compiler and unified toolchain written in **Rust**, **Zig**, and **Kotlin**.

## 🚀 Key Objectives

- **Ultra-Fast Compilation**: Up to 10x–50x faster than traditional JVM-based kotlinc by eliminating JVM startup, daemon warmup, and heavy heap management.
- **Zero-Dependency Unified Toolchain**: A single, self-contained binary `k3` that handles building, running, testing, and packaging (similar to `cargo` and `zig build`), free of mandatory Gradle/Maven requirements.
- **100% Compatibility with Kotlin 2.x**: Conforms to the K2 (FIR) compiler specification, emitting standard JVM `.class`/`.jar` bytecode, `@kotlin.Metadata`, and multiplatform `.klib` packages.
- **Native & JVM Multiplatform**: First-class JVM output alongside high-performance native machine code via Zig's runtime and LLVM integration.

---

## 🏗️ Architecture

```
kotlin3/
├── crates/                       # Rust Compiler Engine
│   ├── k3_syntax/                # SIMD/Zero-copy Lexer, Tokens, CST, AST
│   ├── k3_fir/                   # K2 FIR (Frontend IR) & Cone Kotlin Type System
│   ├── k3_ir/                    # Backend Intermediate Representation & Lowerings
│   ├── k3_jvm/                   # JVM Bytecode & Classfile Emitter
│   └── k3_cli/                   # Rust CLI (`k3` lexer & syntax checker)
├── runtime/                      # Zig Native Runtime & Unified Toolchain Driver
│   └── src/
│       ├── main.zig              # `k3-driver` toolchain CLI
│       └── runtime/              # Native GC, Allocators, Coroutines, C-interop
├── samples/                      # Kotlin sample programs
└── stdlib/                       # Kotlin standard library
```

---

## 🛠️ Getting Started

### Prerequisites (WSL / Linux)
- **Rust**: `1.100+` (`cargo`, `rustc`)
- **Zig**: `0.17+`

### Build the Rust Compiler Crates
```bash
cargo build
cargo test
```

### Try the Rust Lexer & Checker
```bash
cargo run -p k3_cli -- lex samples/hello.kt
cargo run -p k3_cli -- check samples/hello.kt
```

### Build the Native Zig Driver
```bash
zig build
./zig-out/bin/k3-driver version
```
