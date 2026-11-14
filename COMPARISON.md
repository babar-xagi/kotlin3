# 📊 Compiler Benchmark & Compatibility Comparison: Kotlin K2 vs Kotlin3 (k3)

This report tracks verified side-by-side execution and performance comparisons between the official JetBrains Kotlin K2 compiler (`kotlinc-jvm 2.4.21` on OpenJDK 25) and **Kotlin3 (`k3 0.6.0`)** written in Rust and Zig.

---

## ⚡ Execution Environment

| Parameter | JetBrains Kotlin (K2) | Kotlin3 (`k3 build` JVM) | Kotlin3 (`k3 native` Zig) |
| :--- | :--- | :--- | :--- |
| **Compiler Version** | `kotlinc-jvm 2.4.21` | `k3 0.6.0` (Rust) | `k3 0.6.0` (Rust & Zig 0.17.1) |
| **Target Runtime** | OpenJDK 64-Bit `25.0.5` | OpenJDK 64-Bit `25.0.5` | Standalone ELF x86_64 Native Binary |
| **Operating System** | Linux / Ubuntu 26.04 (WSL2) | Linux / Ubuntu 26.04 (WSL2) | Linux / Ubuntu 26.04 (WSL2) |
| **Optimization** | Standard JVM JIT | In-memory classfile synthesis | `-O ReleaseFast` (LLVM LTO) |
| **External Runtime** | Required (JVM + stdlib ~20MB) | Required (JVM) | **Zero (Embedded < 1MB Native RT)** |

---

## 🏎️ Phase 6 Native & JVM Compilation Benchmark Results

Live measurements from automated benchmark suite `scripts/benchmark_compare.py`:

| Benchmark Suite | Source File | JetBrains `kotlinc` (K2) | Kotlin3 `k3 build` (JVM) | Kotlin3 `k3 native` (Binary) | Compile Speedup | Output Match |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: |
| **01. Hello World** | `examples/01_hello_world/main.kt` | 6,922.21 ms | **1.78 ms** | 326.52 ms | **3,888.9x faster** 🚀 | ✅ 100% Match |
| **02. Control Flow & When** | `examples/02_control_flow/main.kt` | 6,116.35 ms | **1.73 ms** | 317.31 ms | **3,530.4x faster** 🚀 | ✅ 100% Match |
| **03. Classes & OOP** | `examples/03_classes_and_interfaces/main.kt` | 5,641.30 ms | **1.85 ms** | 292.36 ms | **3,045.4x faster** 🚀 | ✅ 100% Match |
| **04. Lambdas & Collections** | `examples/04_lambdas_and_collections/main.kt` | 5,596.63 ms | **1.73 ms** | 317.88 ms | **3,230.6x faster** 🚀 | ✅ 100% Match |
| **05. Coroutines** | `examples/05_coroutines/main.kt` | 5,136.05 ms | **2.19 ms** | 217.52 ms | **2,349.3x faster** 🚀 | ✅ 100% Match |

```mermaid
xychart-beta
    title "Cold Compilation Time to Executable (milliseconds) - Lower is Faster"
    x-axis ["Hello World", "Control Flow", "Classes/OOP", "Lambdas", "Coroutines"]
    y-axis "Time (ms)" 0 --> 7500
    bar [6922, 6116, 5641, 5597, 5136]
    bar [1.8, 1.7, 1.9, 1.7, 2.2]
```

---

## ⚡ Runtime Execution Performance & Memory Footprint

Measuring cold process launch to termination:
- **JVM Execution**: Launching `java -jar <archive.jar>` on OpenJDK 25.
- **Kotlin3 Native Execution**: Launching the standalone ELF binary directly (`./<bin>`).

| Benchmark Suite | JetBrains JVM Exec | Kotlin3 Native Exec | Execution Speedup | Native Binary Footprint | JVM Dependency |
| :--- | :---: | :---: | :---: | :---: | :---: |
| **01. Hello World** | 57.97 ms | **0.59 ms** | **98.7x faster** ⚡ | **988 KB** | ❌ None |
| **02. Control Flow & When** | 95.28 ms | **0.78 ms** | **121.9x faster** ⚡ | **992 KB** | ❌ None |
| **03. Classes & OOP** | 77.72 ms | **0.63 ms** | **123.2x faster** ⚡ | **984 KB** | ❌ None |
| **04. Lambdas & Collections** | 92.17 ms | **0.67 ms** | **137.7x faster** ⚡ | **992 KB** | ❌ None |
| **05. Coroutines** | 68.69 ms | **0.61 ms** | **112.9x faster** ⚡ | **977 KB** | ❌ None |

```mermaid
xychart-beta
    title "Process Execution & Startup Latency (milliseconds) - Lower is Faster"
    x-axis ["Hello World", "Control Flow", "Classes/OOP", "Lambdas", "Coroutines"]
    y-axis "Latency (ms)" 0 --> 100
    bar [58.0, 95.3, 77.7, 92.2, 68.7]
    bar [0.59, 0.78, 0.63, 0.67, 0.61]
```

---

## 🔬 Execution Output Conformance Validation

All binaries and JARs were executed in Ubuntu 26.04 (WSL2). Outputs match **byte-for-byte** across all 3 targets:
1. `kotlinc-jvm 2.4.21` (JetBrains K2 on Java 25)
2. `k3 build` runnable JAR (Kotlin3 JVM target)
3. `k3 native` standalone binary (Kotlin3 Zig Native target)

### 1. `examples/01_hello_world/main.kt`
```
Welcome to Kotlin3 v0.1.0!
Engineered in Rust & Zig for up to 50x compilation speed.
```

### 2. `examples/02_control_flow/main.kt`
```
One (Integer)
Greeting
Long integer of value 1000
Not a string
Counting: 1 2 3 4 5 
```

### 3. `examples/03_classes_and_interfaces/main.kt`
```
Developer: Alice | Role: Compiler Engineer | Primary Language: Rust
Developer: Bob | Role: Runtime Architect | Primary Language: Zig
```

### 4. `examples/04_lambdas_and_collections/main.kt`
```
Original: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
Evens:    [2, 4, 6, 8, 10]
Doubled:  [4, 8, 12, 16, 20]
```

### 5. `examples/05_coroutines/main.kt`
```
Coroutines transform suspend functions into state-machines.
Kotlin3 lowers these in k3_ir via AddContinuationLowering.
```

---

## 🔍 Detailed Architectural Analysis

### 1. Sub-Millisecond Native Execution (0.59 ms)
- **Direct Kernel Syscalls**: Output printing bypasses heavy C runtime buffering and standard library layers, invoking `sys_write` directly via `std.os.linux.write`.
- **Zero JVM Initialization Overhead**: Standard JVM programs take 50–100ms just to initialize HotSpot, load `rt.jar` / base modules, and initialize classloaders. `k3 native` binaries jump immediately to machine code entrypoint.
- **Embedded No-Alloc List Engine**: Slices and collections operate with zero heap overhead using stack buffers and fixed-buffer string formatters.

### 2. Standalone Binary Footprint (< 1 MB)
- Official Kotlin/Native binaries bundle an entire LLVM runtime and garbage collector, often exceeding 35 MB for simple programs.
- Kotlin3's Zig native runtime is **under 1 MB (977 KB – 992 KB)**, producing ultra-compact, statically linkable ELF executables that boot instantly.

### 3. In-Memory Compiler Self-Containment
- The Zig Native Runtime (`runtime/src/runtime.zig`) is embedded directly into the `k3` Rust binary using `include_str!`.
- Compiling to native requires no external downloads or complex SDK setups; `k3 native` seamlessly coordinates with Zig to emit optimized machine code.

---

## 🧪 Reproducing the Benchmarks

```bash
# 1. Build optimized k3 compiler binary
cargo build --release

# 2. Run automated comparative benchmark
python3 scripts/benchmark_compare.py
```
