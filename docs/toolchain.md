# 🛠️ Kotlin3 Toolchain Reference

The Kotlin3 toolchain (`k3`) is a unified, self-contained development tool designed to replace the need for heavy external build orchestrators for common Kotlin workflows.

> 📖 For comprehensive tutorials, code walkthroughs, and step-by-step examples, see the [User Guide](user_guide.md).

---

## ⚡ Command Line Interface Overview

```bash
k3 <command> [options] [arguments]
```

### Primary Commands in v0.6.0

| Command | Description | Example |
| :--- | :--- | :--- |
| `k3 native` | Compile Kotlin source directly into a standalone native binary using Zig | `k3 native src/main.kt -o app` |
| `k3 run` | Compile and execute Kotlin program immediately (native by default, or JVM) | `k3 run src/main.kt` |
| `k3 build` | Compile Kotlin source into a standalone runnable JVM JAR | `k3 build src/main.kt -o app.jar` |
| `k3 ir` | Lower AST into Universal Backend IR with CPS lowerings and default stubs | `k3 ir src/main.kt` |
| `k3 check` | Fast syntax and Cone type-checking without bytecode emission | `k3 check src/main.kt` |
| `k3 parse` | Parse Kotlin source into Abstract Syntax Tree (AST) | `k3 parse src/main.kt` |
| `k3 lex` | Inspect tokens produced by the SIMD zero-copy lexer | `k3 lex src/main.kt` |

---

## 🎯 Compilation Targets

Kotlin3 supports dual compilation backends:

```bash
# 1. Standalone Native Executable (via Zig Native Backend)
k3 native main.kt -o app
./app

# 2. Standard JVM Target (Emits runnable .jar)
k3 build main.kt -o app.jar
java -jar app.jar

# 3. Instant Execution (Native by default)
k3 run main.kt

# 4. Instant Execution (via JVM)
k3 run main.kt --jvm
```

---

## ⚙️ Building Kotlin3 from Source

### Prerequisites
- **Rust Toolchain**: `1.85+` / `1.100+` (`cargo`, `rustc`)
- **Zig Toolchain**: `0.17+`

### Build Commands

```bash
# Build Rust compiler crates in release mode
cargo build --release

# Run unit tests across all workspace crates
cargo test --workspace

# Run Zig runtime tests
zig test runtime/src/runtime.zig
```
