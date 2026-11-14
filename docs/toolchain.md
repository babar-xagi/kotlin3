# 🛠️ Kotlin3 Toolchain Guide

The Kotlin3 toolchain (`k3`) is a unified, self-contained development tool designed to replace the need for heavy external build orchestrators for common Kotlin workflows.

---

## ⚡ Command Line Interface Overview

```bash
k3 <command> [options] [arguments]
```

### Primary Commands

| Command | Description | Example |
| :--- | :--- | :--- |
| `k3 build` | Compile a single file or an entire project | `k3 build src/main.kt -o app.jar` |
| `k3 run` | Compile and immediately execute the entrypoint | `k3 run src/main.kt` |
| `k3 test` | Run tests with integrated test runner | `k3 test` |
| `k3 check` | Fast syntax and type checking without bytecode emission | `k3 check src/main.kt` |
| `k3 lex` | Inspect tokens produced by the fast lexer | `k3 lex src/main.kt` |
| `k3 version` | Display toolchain version and component details | `k3 version` |

---

## 🎯 Compilation Targets

Kotlin3 supports multiple output targets via the `--target` flag:

```bash
# 1. Standard JVM Target (Emits runnable .jar / .class)
k3 build main.kt --target jvm -o app.jar
java -jar app.jar

# 2. Native Executable (via Zig Native Backend)
k3 build main.kt --target native -o app
./app

# 3. WebAssembly Target (WASI or Browser Wasm GC)
k3 build main.kt --target wasm -o app.wasm

# 4. Multiplatform KLib
k3 build main.kt --target klib -o library.klib
```

---

## ⚙️ Building Kotlin3 from Source

### Prerequisites
- **Rust Toolchain**: `1.100+` (`cargo`, `rustc`)
- **Zig Toolchain**: `0.17+`

### Build Commands

```bash
# Build Rust compiler crates in release mode
cargo build --release

# Run unit tests across all crates
cargo test

# Build Zig native toolchain driver
zig build -Doptimize=ReleaseFast
```
