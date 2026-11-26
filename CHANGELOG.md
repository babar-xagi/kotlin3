# Changelog

All notable changes to **Kotlin3 (`k3`)** are documented here.

---

## [0.7.0] — Rust + Kotlin Zero-Gradle Reimplementation

### Architecture & Language Changes
- **Removed Zig completely**: Deleted `crates/k3_native`, `build.zig`, `runtime/src/main.zig`, and `runtime/src/runtime.zig`. Kotlin3 is now implemented strictly in **Rust** (CLI, resolver, compiler pipeline) and **Kotlin** (`runtime/`).
- **Updated bundled toolchains**: Configured and verified against **OpenJDK 25.0.5.1+2 LTS** (Eclipse Temurin) and **Kotlin `2.4.21`**.

### Added (`k3_pkg`, `k3_cli`, `runtime/`)
- Added [`crates/k3_pkg`](crates/k3_pkg) with zero third-party Cargo dependencies:
  - [`k3.toml`](crates/k3_pkg/src/manifest.rs) project manifest & [`k3.lock`](crates/k3_pkg/src/lockfile.rs) SHA-256 lockfile.
  - [Maven POM & metadata resolver](crates/k3_pkg/src/maven.rs) supporting `maven-metadata.xml`, `<parent>` inheritance, `<properties>` substitution, `<dependencyManagement>`, exclusions, and `~/.k3/cache/maven` + `~/.m2/repository` caching.
  - [SHA-256 content-addressed JAR cache & fat-JAR packager](crates/k3_pkg/src/cache.rs) with `META-INF/services` merging and signature stripping.
  - [Instant toolchain detector & installer](crates/k3_pkg/src/toolchain.rs) reading JDK `release` and Kotlin `build.txt` directly (avoiding ~2 s `kotlinc -version` startup delays on `--fast` paths).
- Added Kotlin runtime & test framework in [`runtime/`](runtime/src/main/kotlin/k3):
  - `k3.runtime`: `K3Env`, `K3Time`, `K3Fs`, `K3Process`, `K3Http`, `K3Async`, `K3Json`
  - `k3.test`: `runTests`, `test`, `expectEquals`, `expectTrue`, `expectThrows`
- Replaced legacy CLI with unified `uv`/`bun`-style subcommands in [`crates/k3_cli`](crates/k3_cli/src/main.rs): `init`, `add`, `remove`, `sync`, `run`, `build`, `test`, `check`, `ir`, `parse`, `lex`, `toolchain`, `cache`, `info`.
- Added [`examples/06_k3_project_and_runtime`](examples/06_k3_project_and_runtime) demonstrating `k3.toml`, `k3.runtime`, and `k3.test`.

### Fixed (`k3_syntax`, `k3_fir`, `k3_ir`, `k3_jvm`)
- **Syntax (`k3_syntax`)**: Fixed triple-quoted raw strings (`TokenKind::TripleQuote`), Kotlin escape sequences in string literals/templates, lambda block disambiguation (`{ x -> ... }`), soft-keyword identifiers (`out`, `inner`), infix `to`/`zip`, statement annotations (`@Suppress(...)`), and type RHS parsing for `as`/`as?`/`is`/`!is`.
- **Frontend IR (`k3_fir`)**: Added multi-file `analyze_files` resolution, duplicate declaration checks, call arity checks, `return`/`suspend`/interface abstract-member checkers, `MutableList` <: `List` <: `Collection` <: `Iterable` subtyping, `listOf`/`mutableListOf`/`mapOf`/`setOf`, and `k3.runtime`/`k3.test` import indexing.
- **Backend IR (`k3_ir`)**: Implemented `This`, `Unary`, `MemberAccess`, `IndexAccess`, and `Lambda` lowering, plus real single-expression/single-return call-site inlining.
- **JVM Backend (`k3_jvm`)**: Fixed `when`/`is` smart-cast branch patching order, real `ClassKind::Interface` (`0x0601`) and `interfaces` bytecode, `data class` `toString()`, default argument filling, `ArrayList` collections & `Iterable.iterator()` loops, synthetic `MainKt$lambda$N` classes (`Function`), inline `.map`/`.filter`, `Locale.US`, `ByteArray` init loops, and JVM exception-table handlers (`toIntOrNull`/`toLongOrNull`).

### Verified Behavior
- `cargo test --workspace --offline`: **27 passed, 0 failed**.
- `cd runtime && k3 test`: **5 passed, 0 failed**.
- `examples/01`–`05` + `samples/hello.kt` + `benches/compute_benchmark.kt`: all compile cleanly via `k3 --fast`, pass `java -Xverify:all`, and match `kotlinc 2.4.21` stdout/checksums identically.
- Real Maven Central dependency flow tested (`k3 add org.json:json` -> `20260814` -> `k3 run` -> `k3 remove`).

---

## [0.1.0 – 0.6.x] — Historical Prototyping

Early prototype versions explored a custom Rust frontend/JVM bytecode emitter paired with an experimental Zig native runtime (`k3_native`). See git history for pre-v0.7.0 commits.
