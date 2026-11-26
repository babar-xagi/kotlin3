# Kotlin3 (`k3 0.7.0`) Compute & Compile Benchmark Report

Recorded directly from [`scripts/benchmark_runner.py`](scripts/benchmark_runner.py) and [`scripts/benchmark_compare.py`](scripts/benchmark_compare.py) on Linux x86_64.

---

## Test Environment

| Component | Version / Configuration |
| :--- | :--- |
| **`k3` Binary** | `k3 0.7.0` (`cargo build --release`, Rust + Kotlin architecture) |
| **JDK / JVM** | Eclipse Temurin OpenJDK `25.0.5.1+2-LTS` (`java -Xverify:all`) |
| **Baseline Kotlin Compiler** | JetBrains `kotlinc-jvm 2.4.21` (`kotlinc ... -include-runtime`) |
| **Compute Suite** | [`benches/compute_benchmark.kt`](benches/compute_benchmark.kt), `BENCH_REPEATS=5` |

---

## 1. Compute Suite Summary (`benches/compute_benchmark.kt`)

Parameters (defaults in `scripts/benchmark_runner.py`):
- `BENCH_ALU_ITERS=20000000`
- `BENCH_MEM_ITERS=10000000` (`BENCH_MEM_SIZE=1048576`)
- `BENCH_SIEVE_LIMIT=5000000`
- `BENCH_MATRIX_N=192`
- `BENCH_FRACTAL_WIDTH=640`, `BENCH_FRACTAL_HEIGHT=360`, `BENCH_FRACTAL_MAX_ITER=192`

| Metric | `kotlinc 2.4.21` | `k3 0.7.0` (`--fast`) |
| :--- | :---: | :---: |
| **Compile Time (Median)** | 6460.5 ms | **2.63 ms** |
| **Artifact Size** | 5615.0 KB | **7.4 KB** |
| **Total Kernel Time (Median)** | 343.51 ms | **306.02 ms** |

### Kernel Checksums & Medians (Verified Identical)

| Kernel | Verified Checksum | `kotlinc 2.4.21` (ms) | `k3 --fast` (ms) |
| :--- | :--- | :---: | :---: |
| **ALU** | `850514577:3659208163` | 84.97 | 83.99 |
| **Memory** | `3257893589:564826105` | 106.67 | 108.94 |
| **Sieve** | `348513:838596693108` | 35.59 | 35.08 |
| **Matrix** | `398131200` | 16.35 | 16.99 |
| **Fractal** | `58365:12353477` | 58.39 | 57.78 |

---

## 2. Example Suite Compile & Verification (`examples/01`–`05`)

Recorded via `python3 scripts/benchmark_compare.py` with `java -Xverify:all`:

| Example | `kotlinc` Compile | `k3 --fast` Compile | Speedup | `kotlinc` Exec | `k3 --fast` Exec | Stdout Match |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| `01_hello_world/main.kt` | 4706.80 ms | **1.94 ms** | **2430.8x** | 157.81 ms | 155.80 ms | `true` |
| `02_control_flow/main.kt` | 4864.09 ms | **1.96 ms** | **2486.9x** | 177.56 ms | 120.27 ms | `true` |
| `03_classes_and_interfaces/main.kt` | 5186.49 ms | **2.27 ms** | **2286.9x** | 174.38 ms | 120.41 ms | `true` |
| `04_lambdas_and_collections/main.kt` | 5193.35 ms | **1.75 ms** | **2960.4x** | 191.99 ms | 126.54 ms | `true` |
| `05_coroutines/main.kt` | 4911.65 ms | **3.94 ms** | **1247.5x** | 152.21 ms | 118.24 ms | `true` |

---

## Reproducing Locally

```bash
cargo build --release
./scripts/run.sh
python3 scripts/benchmark_compare.py
```
