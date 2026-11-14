# 🤝 Contributing to Kotlin3

Thank you for your interest in contributing to **Kotlin3**! 

---

## 🛠️ Development Setup

Ensure you have the following installed in your development environment (WSL, Linux, or macOS):
- **Rust**: `1.100+` (`rustup update`)
- **Zig**: `0.17+`
- **Git**

### Clone and Verify Setup
```bash
git clone git@github.com:babar-xagi/kotlin3.git
cd kotlin3

# Verify Rust crates build and pass tests
cargo test

# Verify Zig build
zig build
```

---

## 📂 Code Layout Guidelines

- **Rust Compiler Crates (`crates/`)**:
  - Keep modules focused and decoupled.
  - Avoid panics in compiler passes; all errors must be captured as structured diagnostics with source `Span`s.
  - Write unit tests alongside your code in `#[cfg(test)]` modules.
- **Zig Toolchain & Runtime (`runtime/`)**:
  - Follow Zig style conventions (`const std = @import("std");`).
  - Keep memory allocations explicit. Always accept or pass an `allocator` parameter.

---

## 🧪 Testing Guidelines

Before submitting PRs or commits:
```bash
# 1. Format code
cargo fmt --check

# 2. Check for linter warnings
cargo clippy --workspace -- -D warnings

# 3. Run all tests
cargo test
```
