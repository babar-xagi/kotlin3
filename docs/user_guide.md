# Kotlin3 (`k3`) User Guide

Welcome to **Kotlin3 (`k3 0.7.0`)**, a single-binary CLI, package manager, compiler, and runtime for Kotlin built in **Rust & Kotlin** with zero Gradle and zero Maven.

---

## 1. Installing Toolchains & Building `k3`

### Prerequisites
- Rust (`cargo` / `rustc`)
- `curl` (used for Maven Central / toolchain downloads when needed)

Build and install `k3`:

```bash
git clone https://github.com/babar-xagi/kotlin3.git
cd kotlin3
cargo build --release
mkdir -p ~/.local/bin
cp target/release/k3 ~/.local/bin/k3
```

Check for existing JDK & Kotlin installations, or install OpenJDK 25 + latest Kotlin compiler automatically into `~/.k3/toolchains`:

```bash
k3 toolchain status
k3 toolchain install
```

Verified configuration:
- **JDK**: Eclipse Temurin OpenJDK `25.0.5.1+2-LTS`
- **Kotlin**: JetBrains Kotlin `2.4.21`

---

## 2. Running Single-File Kotlin Programs

Run standalone `.kt` files directly with zero project files:

```bash
k3 run examples/01_hello_world/main.kt
k3 run examples/04_lambdas_and_collections/main.kt
```

Force a specific compilation engine or pass runtime arguments:

```bash
# Force Rust direct-to-JVM bytecode compiler (~1.7–4 ms)
k3 run --fast benches/compute_benchmark.kt

# Force official kotlinc 2.4.21 + k3-runtime.jar (with SHA-256 JAR cache)
k3 run --kotlinc examples/02_control_flow/main.kt -- arg1 arg2

# Bypass cached JARs
k3 run --no-cache samples/hello.kt
```

---

## 3. Zero-Gradle Project Workflow (`uv` / `bun` Style)

### Initialize a Project

```bash
k3 init hello-k3
cd hello-k3
```

Generated layout:
```text
hello-k3/
├── k3.toml
├── src/
│   └── Main.kt
└── tests/
    └── MainTest.kt
```

### Type-Check, Run & Test

```bash
k3 check           # Fast Rust syntax & semantic FIR check across project files
k3 check --strict  # Also runs kotlinc type-checking with k3-runtime and locked deps
k3 run             # Runs project entry (or `k3 run <script-name>`)
k3 test            # Compiles and runs tests/**/*.kt with k3.test
```

### Manage Maven Central Dependencies

```bash
# Resolve latest version from Maven Central, update k3.toml, lock SHA-256 in k3.lock
k3 add org.json:json

# Pin a specific version or add a dev dependency
k3 add com.google.code.gson:gson:2.11.0
k3 add org.junit.jupiter:junit-jupiter-api --dev

# Refresh lockfile or remove a dependency
k3 sync --refresh
k3 remove org.json:json
```

### Build Distributable JARs

```bash
# Thin project JAR
k3 build -o build/app.jar

# Self-contained executable uber-JAR (includes Kotlin stdlib, k3-runtime, & Maven deps)
k3 build --fat -o dist/hello-k3-all.jar
java -jar dist/hello-k3-all.jar
```

---

## 4. Built-In Kotlin Runtime (`k3.runtime` & `k3.test`)

Every project managed by `k3` automatically receives `k3-runtime.jar` on its compile and runtime classpaths (see [`examples/06_k3_project_and_runtime`](../examples/06_k3_project_and_runtime)):

- **`K3Env`**: `get(name, default)`, `int(name, default)`, `requirePositiveInt(name, default)`
- **`K3Time`**: `measure { ... }` -> `TimedResult(value, elapsedMs)`, `formatMs(ms)`
- **`K3Fs`**: `readText`, `writeText`, `exists`, `listFiles`
- **`K3Process`**: `exec(vararg command, workDir)` -> `CommandOutput(exitCode, stdout, stderr)`
- **`K3Http`**: `get(url)`, `post(url, body, contentType)` -> `HttpResponseData(status, body)`
- **`K3Async`**: `parallel(tasks)` running tasks concurrently on Java virtual threads
- **`K3Json`**: lightweight dependency-free JSON `stringify(value)` and `parse(input)`
- **`k3.test`**: `runTests(suiteName) { test(name) { ... } }`, `expectEquals`, `expectTrue`, `expectThrows`

---

## 5. Cache & Workspace Inspection

```bash
k3 info          # Active toolchain, project manifest, and dependency summary
k3 cache status  # Cache sizes under ~/.k3/cache (builds, runtime, maven)
k3 cache clean   # Clear cached JARs and force clean rebuilds
```
