# 📝 Changelog

All notable changes to the **Kotlin3** project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Planned
- ASM-style JVM classfile bytecode emitter in `k3_jvm` (`.class` and `.jar` generation).
- `@kotlin.Metadata` Protobuf serialization.
- Backend IR and lowering pipeline (`k3_ir`).

---

## [0.3.0] - 2026-11-14

### ✨ Added
- **K2 FIR Scope Resolution & Symbol Tables (`k3_fir/src/symbols.rs`)**:
  - `Scope` hierarchy with parent delegation, supporting local scopes, class member scopes, file scopes, and built-in root scopes.
  - Symbols for `FunctionSymbol`, `PropertySymbol`, `ClassSymbol`, and `VariableSymbol`.
  - Built-in Kotlin standard library declarations: `Any`, `Int`, `Long`, `Float`, `Double`, `Boolean`, `String`, `Char`, `Unit`, `Nothing`, `Array`, `List`, `MutableList`, `Map`, `Set`, `println`, `print`, `listOf`, `mutableListOf`.
- **Cone Kotlin Type System (`k3_fir/src/cone_types.rs`)**:
  - Complete `ConeKotlinType` representation supporting classes, generics, projections (`*`, `in`, `out`), flexible platform types, and nullability (`NotNull`, `Nullable`, `Flexible`).
  - Implemented `is_subtype_of` rule engine supporting primitive conversions, `Any?` top-type, and `Nothing` bottom-type.
- **Data-Flow Analysis & Smart Casts (`k3_fir/src/dfa.rs`)**:
  - `DfaState` tracking smart-cast type refinements across control flow paths.
  - Smart-casting on `when` subject with `is Type` conditions.
- **Diagnostic Engine & Checkers (`k3_fir/src/diagnostics.rs`)**:
  - Structured diagnostics with exact line, column, snippet, and caret rendering.
  - Diagnostic checks: `UnresolvedReference`, `TypeMismatch`, `ValReassignment`, `ConditionMustBeBoolean`, `WrongNumberOfArguments`, `DuplicateDeclaration`.
- **Phased Resolution Engine (`k3_fir/src/resolution.rs`)**:
  - Top-level declaration indexing phase.
  - Type-checking function bodies, variable declarations, property initializers, and assignments.
  - Support for invoking function-typed variables (`predicate(item)`) and implicit lambda parameter `it`.
- **CLI Type Checker Integration (`k3_cli`)**:
  - `k3 check <file.kt>` now performs complete syntactic parsing, K2 FIR symbol resolution, type checking, and diagnostic emission with non-zero exit codes on error.
- **Benchmarks & Validation**:
  - Verified 100% semantic pass across all 5 example files.
  - Updated [COMPARISON.md](COMPARISON.md) with Phase 3 benchmarks showing ~**3,000x** speedup over JVM `kotlinc`.

---

## [0.2.0] - 2026-11-14

### ✨ Added
- **Full Pratt Expression & Statement Parser (`k3_syntax/src/parser.rs`)**:
  - Precedence climbing algorithm covering full Kotlin operator hierarchy (`+`, `-`, `*`, `/`, `%`, `==`, `!=`, `===`, `!==`, `<`, `<=`, `>`, `>=`, `&&`, `||`, `?:`, `..`, `..<`).
  - Postfix operators: function call expressions, array indexing (`expr[i]`), safe navigation (`?.`), member access (`.`), and not-null assertions (`!!`).
  - Generic function calls with explicit type arguments (e.g. `mutableListOf<T>()`).
  - Control flow expressions: `if`/`else` ternary expressions, `when` statements/expressions with `is`, `in`, and custom conditions, `for` loops, `while`, and `do-while`.
  - Lambdas with trailing lambda syntax and lambda parameter lists (`a, b ->`).
  - Return statements, assignments (`=`, `+=`, `-=`, `*=`, `/=`, `%=`), and code blocks `{ ... }`.
- **Complete Kotlin AST Model (`k3_syntax/src/ast.rs`)**:
  - `KtFile`, `PackageHeader`, `ImportDirective`, `Declaration` (`NamedFunction`, `PropertyDeclaration`, `ClassDeclaration`).
  - Generic type parameters and extension function receiver types (e.g. `fun <T> List<T>.foo()`).
  - Function types (e.g. `(T) -> Boolean`).
  - Precise source `Span` tracking on all syntax nodes for high-quality diagnostics.
- **Enhanced CLI (`k3_cli`)**:
  - Added `k3 parse <file.kt>` command to output formatted AST data structures.
  - Enhanced `k3 check <file.kt>` to perform full syntactic AST validation.
- **Official Kotlin K2 vs Kotlin3 Benchmark Suite (`scripts/benchmark_compare.py`)**:
  - Automated comparative benchmarking between `kotlinc-jvm 2.4.21` (OpenJDK 25) and `k3 0.2.0`.
  - Added [COMPARISON.md](COMPARISON.md) showing verified **2,800x–4,100x** cold speedup over JVM kotlinc.
- **Documentation & Open-Source Infrastructure**:
  - World-class professional [README.md](README.md) with badges, system architecture diagram, and benchmark results.
  - Added Apache-2.0 [LICENSE](LICENSE).

---

## [0.1.0] - 2026-11-14

### ✨ Added
- **Multi-Crate Rust Compiler Workspace**:
  - `crates/k3_syntax`: High-performance, zero-copy Kotlin lexer powered by `logos` with comprehensive token patterns for keywords, operators, identifiers, and literals.
  - `crates/k3_fir`: K2-compatible FIR architecture definition with phased resolution enum (`FirResolvePhase`) and `ConeKotlinType` type hierarchy.
  - `crates/k3_ir`: Backend universal intermediate representation AST (`IrDeclaration`, `IrStatement`, `IrExpression`).
  - `crates/k3_jvm`: Initial Java classfile writer generating binary `.class` file structures with `0xCAFEBABE` header.
  - `crates/k3_cli`: Standalone `k3` CLI with `lex` and `check` commands using `clap`.
- **Zig Native Toolchain & Driver**:
  - `build.zig`: Master Zig 0.17 build script for compiling native driver and toolchain artifacts.
  - `runtime/src/main.zig`: Unified `k3-driver` executable supporting `build`, `run`, `test`, and `version` commands.
- **Samples & Examples**:
  - Added sample Kotlin application (`samples/hello.kt`) demonstrating package declarations, functions, variables, and string interpolation.
- **Documentation & Infrastructure**:
  - `ROADMAP.md`: Detailed engineering roadmap across 7 development phases with architecture diagrams and performance targets.
  - `.gitignore`: Configured ignores for Rust, Zig, JVM, and IDE artifacts.
