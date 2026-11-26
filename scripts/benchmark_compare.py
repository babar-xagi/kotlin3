#!/usr/bin/env python3
import json
import os
import shutil
import subprocess
import time
from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent.parent
K3_BIN = BASE_DIR / "target" / "release" / "k3"

def find_tool(name, fallback):
    p = shutil.which(name)
    if p:
        return p
    if Path(fallback).exists():
        return fallback
    return name

JAVA = find_tool("java", os.path.expanduser("~/.local/opt/jdk/bin/java"))
KOTLINC = find_tool("kotlinc", os.path.expanduser("~/.local/opt/kotlinc/bin/kotlinc"))

examples = [
    ("01_Hello_World", "examples/01_hello_world/main.kt"),
    ("02_Control_Flow", "examples/02_control_flow/main.kt"),
    ("03_Classes_and_OOP", "examples/03_classes_and_interfaces/main.kt"),
    ("04_Lambdas_and_Collections", "examples/04_lambdas_and_collections/main.kt"),
    ("05_Coroutines", "examples/05_coroutines/main.kt"),
]

results = []
for name, rel_path in examples:
    full_path = BASE_DIR / rel_path
    k2_jar = f"/tmp/{name}_k2.jar"
    k3_jar = f"/tmp/{name}_k3.jar"

    t0 = time.perf_counter()
    k2_proc = subprocess.run([KOTLINC, str(full_path), "-include-runtime", "-d", k2_jar], capture_output=True, text=True)
    k2_compile_ms = (time.perf_counter() - t0) * 1000

    t0 = time.perf_counter()
    k2_run = subprocess.run([JAVA, "-Xverify:all", "-jar", k2_jar], capture_output=True, text=True)
    k2_exec_ms = (time.perf_counter() - t0) * 1000

    t0 = time.perf_counter()
    k3_proc = subprocess.run([str(K3_BIN), "build", str(full_path), "--fast", "-o", k3_jar], capture_output=True, text=True)
    k3_compile_ms = (time.perf_counter() - t0) * 1000

    t0 = time.perf_counter()
    k3_run = subprocess.run([JAVA, "-Xverify:all", "-jar", k3_jar], capture_output=True, text=True)
    k3_exec_ms = (time.perf_counter() - t0) * 1000

    k2_out = k2_run.stdout.strip()
    k3_out = k3_run.stdout.strip()
    output_match = (k2_proc.returncode == 0 and k2_run.returncode == 0 and k3_proc.returncode == 0 and k3_run.returncode == 0 and k2_out == k3_out)

    results.append({
        "name": name,
        "file": rel_path,
        "k2_compile_ms": round(k2_compile_ms, 2),
        "k3_fast_compile_ms": round(k3_compile_ms, 2),
        "compile_speedup": round(k2_compile_ms / k3_compile_ms, 1) if k3_compile_ms > 0 else 0,
        "k2_exec_ms": round(k2_exec_ms, 2),
        "k3_exec_ms": round(k3_exec_ms, 2),
        "k2_jar_kb": round(os.path.getsize(k2_jar) / 1024, 2) if os.path.exists(k2_jar) else 0,
        "k3_jar_kb": round(os.path.getsize(k3_jar) / 1024, 2) if os.path.exists(k3_jar) else 0,
        "output_match": output_match,
        "output": k3_out,
    })

print(json.dumps(results, indent=2))
