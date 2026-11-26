# Compiler & Build Comparison: `k3 0.7.0` vs Official `kotlinc 2.4.21` / Gradle

All measurements below were recorded on Linux x86_64 with **OpenJDK 25.0.5.1+2-LTS (Eclipse Temurin)** and **Kotlin `2.4.21`** using [`scripts/benchmark_compare.py`](scripts/benchmark_compare.py) and [`scripts/benchmark_runner.py`](scripts/benchmark_runner.py). All generated JARs were executed with strict bytecode verification (`java -Xverify:all -jar ...`).

---

## 1. Workflow & Project Overhead Comparison

| Aspect | Gradle + Kotlin | Maven + Kotlin | Kotlin3 (`k3 0.7.0`) |
| :--- | :--- | :--- | :--- |
| **Config Files Required** | `build.gradle.kts`, `settings.gradle.kts`, `gradlew`, `gradle-wrapper.properties`, `.jar` | `pom.xml`, `mvnw` / system Maven | `k3.toml` (+ auto-generated `k3.lock`) |
| **Single-File Script / Run** | Needs full Gradle project setup | Needs `pom.xml` setup | `k3 run file.kt` (~2 ms `--fast` or cached `kotlinc`) |
| **Dependency Add & Lock** | Manual edit + Gradle sync | Manual XML edit | `k3 add group:artifact` (POM resolve + SHA-256 lock) |
| **Built-in Runtime & Tests** | Requires manual test/JSON/HTTP deps | Requires manual test/JSON/HTTP deps | Bundled `k3.runtime` + `k3.test` (`k3 test`) |
| **Fast Subset Compiler** | Always runs full JVM compiler / daemon | Always runs full JVM compiler | `k3 --fast` (~1.7–4.0 ms) or auto `--kotlinc` fallback |

---

## 2. Example Suite (`examples/01`–`05`): `kotlinc 2.4.21` vs `k3 --fast`

Measured by [`scripts/benchmark_compare.py`](scripts/benchmark_compare.py):
- **Baseline (`kotlinc 2.4.21`)**: `kotlinc <file> -include-runtime -d out.jar` (cold invocation, bundles Kotlin stdlib ~5.6 MB)
- **Kotlin3 (`k3 --fast`)**: `k3 build <file> --fast -o out.jar` (direct Rust JVM classfile emitter targeting Java stdlib)

| Example | `kotlinc` Compile (ms) | `k3 --fast` Compile (ms) | Speedup | `kotlinc` Exec (ms) | `k3 --fast` Exec (ms) | `kotlinc` JAR (KB) | `k3 --fast` JAR (KB) | Output Match |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| `01_Hello_World` | 4706.80 | **1.94** | **2430.8x** | 157.81 | 155.80 | 5606.33 | **1.00** | `true` |
| `02_Control_Flow` | 4864.09 | **1.96** | **2486.9x** | 177.56 | 120.27 | 5606.87 | **1.71** | `true` |
| `03_Classes_and_OOP` | 5186.49 | **2.27** | **2286.9x** | 174.38 | 120.41 | 5608.64 | **2.54** | `true` |
| `04_Lambdas_and_Collections` | 5193.35 | **1.75** | **2960.4x** | 191.99 | 126.54 | 5607.76 | **2.44** | `true` |
| `05_Coroutines` | 4911.65 | **3.94** | **1247.5x** | 152.21 | 118.24 | 5606.53 | **1.02** | `true` |

> [!NOTE]
> Why the JAR size difference? `kotlinc -include-runtime` bundles `kotlin-stdlib` (~5.6 MB) into the standalone JAR. `k3 --fast` emits pure-JDK bytecode (`1.0`–`2.5 KB`) backed directly by JVM standard library classes. When packaging full Kotlin projects with dependencies and `k3-runtime`, `k3 build --fat` also bundles `kotlin-stdlib` + `k3-runtime` (measured at **5.5 MB** on `examples/06_k3_project_and_runtime`).

---

## 3. Compute Kernel Suite (`benches/compute_benchmark.kt`)

5-run median with strict JVM verification (`java -Xverify:all`):

| Metric | `kotlinc 2.4.21` (`-include-runtime`) | `k3 0.7.0` (`--fast`) |
| :--- | :---: | :---: |
| **Compile Median** | 6460.5 ms | **2.63 ms** |
| **Output JAR Size** | 5615.0 KB | **7.4 KB** |
| **Total Kernel Exec Median** | 343.51 ms | **306.02 ms** |

### Per-Workload Breakdown & Checksums

| Workload | Checksum Match | `kotlinc 2.4.21` (ms) | `k3 --fast` (ms) |
| :--- | :--- | :---: | :---: |
| `alu` (20M iters) | `850514577:3659208163` | 84.97 | 83.99 |
| `memory` (10M iters, 1M elems) | `3257893589:564826105` | 106.67 | 108.94 |
| `sieve` (limit 5,000,000) | `348513:838596693108` | 35.59 | 35.08 |
| `matrix` (192x192 int matmul) | `398131200` | 16.35 | 16.99 |
| `fractal` (640x360 x 192 iters) | `58365:12353477` | 58.39 | 57.78 |

**Takeaway**: On supported JVM compute kernels, `k3 --fast` compiles **~2,450x faster** than cold `kotlinc 2.4.21` while emitting standard verified JVM bytecode that matches `kotlinc`'s HotSpot JIT execution speed (~306 ms vs ~344 ms) and exact checksums. For multi-file projects, Maven dependencies, or full Kotlin stdlib/coroutine features, `k3` seamlessly uses `kotlinc 2.4.21` with SHA-256 content-addressed caching under `~/.k3/cache`.
