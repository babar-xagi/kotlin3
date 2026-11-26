#!/usr/bin/env python3
import os
import shutil
import statistics
import subprocess
import sys
import time
from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent.parent
BENCH_SRC = BASE_DIR / "benches" / "compute_benchmark.kt"
K3_BIN = BASE_DIR / "target" / "release" / "k3"
K2_JAR = Path("/tmp/bench_k2.jar")
K3_JAR = Path("/tmp/bench_k3.jar")

def find_tool(name, fallback):
    p = shutil.which(name)
    if p:
        return p
    if Path(fallback).exists():
        return fallback
    return name

JAVA = find_tool("java", os.path.expanduser("~/.local/opt/jdk/bin/java"))
KOTLINC = find_tool("kotlinc", os.path.expanduser("~/.local/opt/kotlinc/bin/kotlinc"))

def get_env_int(key, default):
    return int(os.environ.get(key, default))

def parse_benchmark_output(stdout):
    results = {}
    config = {}
    total_ms = 0.0
    for line in stdout.strip().splitlines():
        line = line.strip()
        if line.startswith("CONFIG|"):
            for p in line.split("|")[1:]:
                if "=" in p:
                    k, v = p.split("=", 1)
                    config[k] = v
        elif line.startswith("RESULT|"):
            parts = line.split("|")
            if len(parts) >= 4:
                results[parts[1]] = {"checksum": parts[2], "elapsed_ms": float(parts[3])}
        elif line.startswith("TOTAL_MS|"):
            total_ms = float(line.split("|")[1])
    return config, results, total_ms

def run_suite(jar_path, repeats, env):
    runs = []
    for r in range(1, repeats + 1):
        p = subprocess.run([JAVA, "-Xverify:all", "-jar", str(jar_path)], env=env, capture_output=True, text=True)
        if p.returncode != 0:
            print(f"❌ Execution failed for {jar_path} on run {r}:\n{p.stderr}")
            sys.exit(1)
        _, res, tot = parse_benchmark_output(p.stdout)
        runs.append({"run": r, "results": res, "total_ms": tot})
    return runs

def main():
    repeats = get_env_int("BENCH_REPEATS", 5)
    env = os.environ.copy()
    env.setdefault("BENCH_ALU_ITERS", "20000000")
    env.setdefault("BENCH_MEM_ITERS", "10000000")
    env.setdefault("BENCH_MEM_SIZE", str(1 << 20))
    env.setdefault("BENCH_SIEVE_LIMIT", "5000000")
    env.setdefault("BENCH_MATRIX_N", "192")
    env.setdefault("BENCH_FRACTAL_WIDTH", "640")
    env.setdefault("BENCH_FRACTAL_HEIGHT", "360")
    env.setdefault("BENCH_FRACTAL_MAX_ITER", "192")

    k2_times = []
    for _ in range(3):
        t0 = time.perf_counter()
        p = subprocess.run([KOTLINC, str(BENCH_SRC), "-include-runtime", "-d", str(K2_JAR)], capture_output=True, text=True)
        if p.returncode != 0:
            print(f"❌ kotlinc failed:\n{p.stderr}")
            sys.exit(1)
        k2_times.append((time.perf_counter() - t0) * 1000)
    k2_compile_ms = statistics.median(k2_times)

    k3_times = []
    for _ in range(5):
        t0 = time.perf_counter()
        p = subprocess.run([str(K3_BIN), "build", str(BENCH_SRC), "--fast", "-o", str(K3_JAR)], capture_output=True, text=True)
        if p.returncode != 0:
            print(f"❌ k3 build failed:\n{p.stderr}")
            sys.exit(1)
        k3_times.append((time.perf_counter() - t0) * 1000)
    k3_compile_ms = statistics.median(k3_times)

    k2_runs = run_suite(K2_JAR, repeats, env)
    k3_runs = run_suite(K3_JAR, repeats, env)

    bench_names = ["alu", "memory", "sieve", "matrix", "fractal"]
    for b in bench_names:
        c2 = k2_runs[0]["results"][b]["checksum"]
        c3 = k3_runs[0]["results"][b]["checksum"]
        if c2 != c3:
            print(f"❌ Checksum mismatch on {b}: kotlinc={c2}, k3={c3}")
            sys.exit(1)

    print(f"kotlinc compile median: {k2_compile_ms:.1f} ms ({K2_JAR.stat().st_size / 1024:.1f} KB)")
    print(f"k3 fast compile median: {k3_compile_ms:.2f} ms ({K3_JAR.stat().st_size / 1024:.1f} KB)")
    print(f"kotlinc exec median:    {statistics.median([r['total_ms'] for r in k2_runs]):.2f} ms")
    print(f"k3 fast exec median:    {statistics.median([r['total_ms'] for r in k3_runs]):.2f} ms")
    for b in bench_names:
        m2 = statistics.median([r["results"][b]["elapsed_ms"] for r in k2_runs])
        m3 = statistics.median([r["results"][b]["elapsed_ms"] for r in k3_runs])
        print(f"  {b:<8}: checksum={k3_runs[0]['results'][b]['checksum']} | kotlinc={m2:.2f} ms | k3={m3:.2f} ms")

if __name__ == "__main__":
    main()
