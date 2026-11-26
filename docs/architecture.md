# Kotlin3 Architecture Overview

Kotlin3 (`k3 0.7.0`) replaces Gradle and Maven with a fast **Rust** orchestrator and compiler frontend/backend paired with an embedded **Kotlin** runtime (`runtime/`).

---

## High-Level Design (`uv` / `bun` Model)

```text
User CLI (`k3` in Rust)
  │
  ├── Package & Project Layer (`k3_pkg`)
  │     ├── k3.toml & k3.lock parser/serializer
  │     ├── Maven Central / custom POM & metadata resolver + SHA-256 verifier
  │     ├── Toolchain discovery (`~/.k3/toolchains`, `~/.local/opt`, `JAVA_HOME`, `KOTLIN_HOME`, `PATH`)
  │     ├── Embedded Kotlin runtime compiler & cache (`k3-runtime.jar`)
  │     └── Content-addressed JAR build cache (`~/.k3/cache/builds`) + Fat-JAR builder
  │
  ├── Native Rust Compiler Pipeline (`k3_syntax -> k3_fir -> k3_ir -> k3_jvm`)
  │     ├── Lexer (`logos`) & Pratt/Recursive-Descent Parser (`KtFile` AST)
  │     ├── Multi-file Semantic Checker (`k3_fir`, K2-inspired FIR, smart-cast DFA, arity/return/suspend/abstract checkers)
  │     ├── Universal IR & Lowerings (`inlining`, `default_args`, `coroutines`)
  │     └── Direct JVM Classfile v55 (Java 11) & runnable JAR emitter (~1.7–4 ms)
  │
  └── Official Kotlin Integration (`kotlinc 2.4.21`) + Kotlin Runtime (`runtime/`)
        ├── Automatic fallback for multi-file projects, Maven dependencies, imports, generics & coroutines
        └── `k3.runtime` (Env, Time, Fs, Process, Http, Virtual-Thread Async, Json) & `k3.test`
```

---

## Crate & Module Breakdown

### 1. [`crates/k3_syntax`](../crates/k3_syntax) (Rust)
- Uses `logos` in `token.rs` / `lexer.rs` for zero-copy tokenization including nested `${...}` string templates, triple-quoted strings (`"""..."""`), Kotlin operators (`?.`, `?:`, `!!`, `..`, `..<`, `as?`, `!in`, `!is`), and hex/binary/underscore literals.
- `parser.rs` builds a typed Kotlin AST (`ast.rs`) covering packages, imports, classes/interfaces/data classes, primary constructors, extension receivers, `when`, loops, lambdas `{ x -> ... }`, infix `to`/`zip`/`until`/`downTo`/`step`, statement annotations, and `is`/`as` type expressions.

### 2. [`crates/k3_fir`](../crates/k3_fir) (Rust)
- Resolves single-file (`analyze`) or multi-file project (`analyze_files`) ASTs in phases: top-level indexing, body resolution, and post-resolution checkers.
- Tracks collection subtyping (`MutableList` <: `List` <: `Collection` <: `Iterable`, `Set`, `Map`), dataflow smart casts (`is`/`!is`), duplicate declarations, call argument counts, function return types, `suspend` call context, unimplemented interface methods, and `k3.runtime` / `k3.test` symbols.

### 3. [`crates/k3_ir`](../crates/k3_ir) (Rust)
- Lowers validated AST declarations into Universal Backend IR (`IrFile`, `IrClass`, `IrFunction`, `IrExpression`, `IrStatement`).
- Includes passes in `lowerings/`:
  - `inlining.rs`: call-site substitution for simple single-expression / single-return functions.
  - `default_args.rs`: `$default` stub generation for functions with default parameters.
  - `coroutines.rs`: `suspend` function continuation metadata lowering.

### 4. [`crates/k3_jvm`](../crates/k3_jvm) (Rust)
- Directly emits JVM `.class` bytecode (major version 55 / Java 11) and packages compressed runnable `.jar` archives (`jar.rs`) with zero external build tools.
- Generates real JVM interfaces (`0x0601`), concrete classes with implemented interfaces, `data class` `toString()` methods, synthetic `MainKt$lambda$N` classes implementing `java/util/function/Function`, inline `.map`/`.filter` loops, `ByteArray` init loops, `ArrayList` / `Iterable.iterator()` loops, `when` type/range dispatch, `Locale.US`, and exception-table handlers (`toIntOrNull` / `toLongOrNull`).
- Note: `k3_jvm` operates directly from the `KtFile` AST & symbol collection while `k3_fir` provides diagnostics and `k3_ir` provides inspectable IR (`k3 ir`) and lowering passes.

### 5. [`crates/k3_pkg`](../crates/k3_pkg) (Rust, `std`-only)
- **Zero third-party Rust dependencies**: compiles offline cleanly and drives `curl` / `jar` / `kotlinc` / `java` subprocesses where needed.
- **`manifest.rs` & `lockfile.rs`**: `k3.toml` and deterministic `k3.lock` with SHA-256 checksums.
- **`maven.rs`**: Fetches `maven-metadata.xml` and `.pom` XML from configured repositories (defaulting to Maven Central), resolves `<parent>` chains, `${property}` interpolation, `<dependencyManagement>`, transitive BFS dependencies, exclusions, and checks `~/.k3/cache/maven` and `~/.m2/repository`.
- **`toolchain.rs`**: Discovers JDK and Kotlin binaries across `JAVA_HOME`, `KOTLIN_HOME`, `~/.k3/toolchains`, `~/.local/opt`, `~/.local/bin`, `~/.sdkman`, and `PATH`. Reads JDK `release` and Kotlin `build.txt` directly (~0.05 ms) to avoid JVM startup latency on fast paths.
- **`cache.rs` & `runtime_support.rs`**: Content-addressed JAR caching (`~/.k3/cache/builds`), `k3-runtime.jar` compilation caching (`~/.k3/cache/runtime`), and uber-JAR assembly (`build_fat_jar`).

### 6. [`crates/k3_cli`](../crates/k3_cli) (Rust)
- Routes `k3 run` and `k3 build`:
  - Uses `--fast` (`k3_syntax -> k3_fir -> k3_jvm`) automatically for compatible standalone `.kt` files without external imports/dependencies.
  - Uses `kotlinc` + `k3-runtime.jar` + locked Maven JARs automatically for projects (`k3.toml`), external imports, or `--kotlinc` / `--fat` builds, caching compiled JARs by source + lockfile + runtime hash.

### 7. [`runtime/`](../runtime) (Kotlin)
- Pure Kotlin standard utilities packaged automatically as `k3-runtime.jar`:
  - `k3.runtime`: `K3Env`, `K3Time`, `K3Fs`, `K3Process`, `K3Http`, `K3Async` (Java virtual threads), `K3Json`.
  - `k3.test`: `runTests`, `test`, `expectEquals`, `expectTrue`, `expectThrows`.
