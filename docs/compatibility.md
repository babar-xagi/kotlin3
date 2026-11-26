# Kotlin3 Compatibility Matrix

Kotlin3 (`k3 0.7.0`) combines a **fast Rust direct-to-JVM subset compiler** (`--fast`) with **official JetBrains `kotlinc` (`2.4.21`) orchestration** (`--kotlinc` / project mode).

---

## Execution Routing Overview

| Workload | Auto Mode Behavior | `--fast` Flag | `--kotlinc` Flag |
| :--- | :--- | :--- | :--- |
| Standalone `.kt` file without external `import`s (`examples/01`–`05`, `benches/compute_benchmark.kt`) | Uses Rust `k3_jvm` fast path (~1.7–4 ms) | Forces `k3_jvm` | Forces `kotlinc` + `k3-runtime` |
| Files with `import`s, `suspend` bodies using coroutine stdlib, generics, or multi-file `k3.toml` projects | Uses `kotlinc 2.4.21` + content cache | Attempts `k3_jvm` subset compilation | Forces `kotlinc` + `k3-runtime` |
| Projects with Maven dependencies (`k3.toml`) or `--fat` builds | Uses `kotlinc` + locked JARs + `k3-runtime` | Supported only for single-file zero-dep non-fat JARs | Uses `kotlinc` + `jar cfm` fat JAR |

---

## Language & Frontend Support (`k3_syntax`, `k3_fir`, `k3_jvm`)

| Feature | `k3_syntax` / `k3_fir` | `--fast` (`k3_jvm`) | Project / `--kotlinc` (`2.4.21`) |
| :--- | :--- | :--- | :--- |
| **Top-level functions, default args, recursion** | Supported | Supported (emits Java-verified bytecode) | Full Kotlin 2.4 support |
| **`val`/`var`, primitives (`Int`, `Long`, `Float`, `Double`, `Boolean`, `String`)** | Supported | Supported | Full support |
| **Control flow (`if`/`else`, `when`, ranges `1..5`, `until`, `downTo`, `step`, `while`)** | Supported | Supported (with `is`/`!is` smart casts in `when`) | Full support |
| **Classes, `data class`, constructors, `interface` & `override`** | Supported | Supported (real JVM interfaces & `data class` `toString`) | Full support |
| **Lambdas, `listOf`, `mutableListOf`, `.map`, `.filter`, `.forEach`** | Supported | Supported (`ArrayList` + synthetic `Function` classes) | Full support |
| **Extension functions** | Supported | Supported for supported receivers/types | Full support |
| **Null-safety (`?`, `?:`, `?.`, `toIntOrNull`, `toLongOrNull`)** | Supported | Supported (`Objects`/boxed nulls & try-catch tables) | Full support |
| **String interpolation (`$x`, `${expr}`, `"""raw"""`)** | Supported | Supported for `$x`, `${expr}`, `toString` | Full support |
| **User generics (`class Box<T>`), `try`/`catch`, `sealed`, `enum`, `object`** | Partial (`fun <T>` / `as Map<K, V>`) | Delegates via Auto mode to `kotlinc` | Full Kotlin 2.4 support |
| **Full `kotlinx.coroutines` & reflection** | Syntax & `suspend` FIR check | Stubbed unless routed to `kotlinc` | Full Kotlin 2.4 + Maven Central deps |

---

## Platform, JVM & Package Manager Support (`k3_pkg`)

| Component | Status | Notes |
| :--- | :--- | :--- |
| **Host OS (CLI)** | Verified on Linux x86_64 | Windows/macOS paths supported in Rust (`k3_pkg`), WiX installer source in `installer/windows/` |
| **JDK Target** | Verified on OpenJDK 25.0.5.1+2-LTS (Temurin) | Emits Classfile v55 (Java 11+) strictly passing `java -Xverify:all` |
| **Kotlin Version** | Verified on Kotlin `2.4.21` | `k3 toolchain install` fetches latest Adoptium JDK + JetBrains Kotlin release |
| **Maven Repositories (`k3_pkg`)** | Verified against Maven Central (`repo1.maven.org`) & `file://` repos | Supports `maven-metadata.xml`, `<parent>` POMs, `<properties>`, `<dependencyManagement>`, exclusions, SHA-256 `k3.lock` |
| **Maven Edge Features** | Partial | Multi-platform Gradle `.module` metadata, BOM `import` scope, and custom `<classifier>` artifacts are not yet implemented |
