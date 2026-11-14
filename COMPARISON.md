# 📊 Compiler Benchmark & Compatibility Comparison: Kotlin K2 vs Kotlin3 (k3)

This report tracks side-by-side verification and performance comparisons between the official JetBrains Kotlin K2 compiler (`kotlinc-jvm 2.4.21` running on OpenJDK 25) and **Kotlin3 (`k3`)** native compiler.

---

## ⚡ Execution Environment

| Parameter | JetBrains Kotlin (K2) | Kotlin3 (k3) |
| :--- | :--- | :--- |
| **Compiler Version** | `kotlinc-jvm 2.4.21` | `k3 0.3.0` (Rust & Zig) |
| **Runtime / VM** | OpenJDK 64-Bit `25.0.5` | Native Native Binary (x86_64 ELF) |
| **Operating System** | Linux / Ubuntu 26.04 (WSL2) | Linux / Ubuntu 26.04 (WSL2) |
| **Build Optimization** | Standard JVM JIT | `--release` (LLVM LTO / Opt-Level 3) |

---

## 🏎️ Phase 3 Semantic Resolution Benchmark Results

Full syntactic parsing, K2 FIR symbol resolution, type inference, and Data-Flow Analysis (DFA) smart-casting run against idiomatic Kotlin source files in `examples/`:

| Benchmark Example | Source File | JetBrains `kotlinc` (K2) | Kotlin3 `k3` (FIR Analyzer) | Status (Both) | Speedup Factor |
| :--- | :--- | :---: | :---: | :---: | :---: |
| **01. Hello World** | `examples/01_hello_world/main.kt` | 4,722.10 ms | **1.60 ms** | ✅ Identical | **2,945x faster** |
| **02. Control Flow & When** | `examples/02_control_flow/main.kt` | 4,922.23 ms | **1.74 ms** | ✅ Identical | **2,821x faster** |
| **03. Classes & OOP** | `examples/03_classes_and_interfaces/main.kt` | 4,965.40 ms | **1.47 ms** | ✅ Identical | **3,379x faster** |
| **04. Lambdas & Collections** | `examples/04_lambdas_and_collections/main.kt` | 4,955.30 ms | **1.46 ms** | ✅ Identical | **3,387x faster** |
| **05. Coroutines** | `examples/05_coroutines/main.kt` | 4,373.14 ms | **1.42 ms** | ✅ Identical | **3,079x faster** |

```mermaid
xychart-beta
    title "Cold Execution Time (milliseconds) - Lower is Faster"
    x-axis ["Hello World", "Control Flow", "Classes/OOP", "Lambdas", "Coroutines"]
    y-axis "Time (ms)" 0 --> 5500
    bar [4722, 4922, 4965, 4955, 4373]
    bar [1.6, 1.7, 1.5, 1.5, 1.4]
```

---

## 🔍 Detailed Analysis & Architectural Drivers

### Why is Kotlin3 so much faster?
1. **Zero JVM Startup Latency**:
   - `kotlinc` must initialize the HotSpot JVM, load hundreds of compiler `.jar` files, initialize classloaders, and perform runtime bytecode verification.
   - `k3` starts executing native machine code in `< 1ms`.
2. **Zero-Copy Memory Model**:
   - The SIMD lexer in `k3_syntax` operates directly on UTF-8 byte slices without creating tens of thousands of intermediate string objects.
3. **Optimized Pratt Parser & Phased Scopes**:
   - Expression type inference and symbol lookups happen in contiguous memory scopes with hash-indexed declaration tables.
4. **Data-Flow Analysis (DFA)**:
   - Zero-allocation smart-cast propagation across `if`/`else` and `when` branches.

---

## 🧪 Reproducing the Benchmarks

To reproduce these measurements locally in your WSL environment:

```bash
# 1. Build optimized k3 release binary
cargo build --release

# 2. Run automated comparative benchmark
python3 scripts/benchmark_compare.py
```
