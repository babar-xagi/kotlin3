# Kotlin3 (`k3`) Project Roadmap

Kotlin3 has pivoted to a focused **Rust + Kotlin** architecture: delivering a `uv`/`bun`-style unified Kotlin CLI, dependency manager, cache, fast direct-to-JVM subset compiler, and Kotlin runtime without Gradle or Maven.

---

## Completed Milestones

- [x] **Lexer & Parser (`k3_syntax`)**: `logos` tokenizer (string interpolation, triple-quoted strings, Kotlin operators) and Pratt/recursive-descent parser covering packages, imports, classes, data classes, interfaces, `when`, loops, lambdas, infix calls, and `as`/`is` casts.
- [x] **K2-Style Semantic Frontend (`k3_fir`)**: Multi-file project symbol indexing (`analyze_files`), subtyping (including `MutableList` <: `List` <: `Iterable`), dataflow smart casts (`is`/`!is`), duplicate/arity/return/suspend/abstract-member diagnostics, and built-in recognition for `k3.runtime` & `k3.test`.
- [x] **Universal IR (`k3_ir`)**: FIR-to-IR lowering with expression/single-return function inlining, default argument stub generation, and coroutine CPS marker lowering.
- [x] **Direct JVM Backend (`k3_jvm`)**: Classfile v55 (Java 11) writer, runnable JAR packager, `data class` synthesis, real `interface` bytecode & `invokeinterface`, `when` type/range dispatch, `ArrayList` collections, synthetic `MainKt$lambda$N` classes, and exception-table handlers (`toIntOrNull`).
- [x] **Zero-Gradle Package & Cache Manager (`k3_pkg`)**:
  - `k3.toml` manifest & `k3.lock` SHA-256 lockfile parser/serializer
  - Pure-Rust POM/parent-POM/properties/`dependencyManagement` & `maven-metadata.xml` resolver for Maven Central & custom repositories
  - Content-addressed build cache (`~/.k3/cache/builds`) and fat-JAR builder (`k3 build --fat`)
  - JDK + Kotlin toolchain discovery (instant `release` / `build.txt` inspection + `k3 toolchain install`)
- [x] **Kotlin Runtime & Test Framework (`runtime/`)**: `K3Env`, `K3Time`, `K3Fs`, `K3Process`, `K3Http`, virtual-thread `K3Async`, `K3Json`, and `k3.test` (`runTests`, `test`, `expectEquals`, `expectTrue`, `expectThrows`), compiled and cached cleanly as `k3-runtime.jar`.
- [x] **Zig Removal (v0.7.0)**: Removed the experimental Zig runtime/AOT crate (`k3_native`, `build.zig`, `runtime/*.zig`) in favor of pure Rust + Kotlin.

---

## Near-Term Roadmap (v0.8.x)

- [ ] **Compiler Daemon / Incremental `kotlinc` Client**: Keep a warm Kotlin compiler daemon or nailgun-style worker alive across `k3 run --kotlinc` / multi-file project builds to eliminate cold `kotlinc` JVM startup latency (~4.7–6.5 s) when the fast Rust backend delegates to `kotlinc`.
- [ ] **Generics, Top-Level Extension Signatures & JVM `StackMapTable` Verification Polish**: Expand `k3_fir` and `k3_jvm` to support user generics, `mapOf`/`setOf` codegen natively in `--fast`, `try/catch/finally` in `k3_jvm`, and `Class<T>` generic type parameters in `k3_syntax`.
- [ ] **Maven Classifier & Relocation Support + Parallel POM/JAR Downloads**: Support `<classifier>`, `<relocation>`, BOM imports (`<scope>import</scope>`), and concurrent downloads in `k3_pkg`.
- [ ] **Script Files (`*.main.kts`) & Inline Dependency Directives**: Allow `// @file:DependsOn("group:artifact:version")` in standalone scripts so `k3 run script.kt` resolves libraries on the fly.

---

## Long-Term Goals (v0.9.x – v1.0)

- [ ] **IDE Language Server (`k3 lsp`)**: Expose `k3_syntax` and `k3_fir` diagnostics, hover types, and `k3.toml` dependency indexing over LSP.
- [ ] **Custom Build Profiles & Multi-Module Workspaces**: Workspace member support (`[workspace]`) in `k3.toml` for multi-package Kotlin monorepos without Gradle subprojects.
- [ ] **jlink / jpackage App Bundling**: `k3 bundle` command producing minimized custom JRE app images using OpenJDK `jlink` / `jpackage`.
