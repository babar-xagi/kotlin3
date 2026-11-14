#!/usr/bin/env python3
import os
import time
import subprocess
import json

base_dir = "/home/xagi/Kotlin3/kotlin3"
k3_bin = f"{base_dir}/target/release/k3"
sdkman_init = 'export SDKMAN_DIR="/home/xagi/.sdkman" && source "/home/xagi/.sdkman/bin/sdkman-init.sh"'

examples = [
    ("01_Hello_World", "examples/01_hello_world/main.kt"),
    ("02_Control_Flow", "examples/02_control_flow/main.kt"),
    ("03_Classes_and_OOP", "examples/03_classes_and_interfaces/main.kt"),
    ("04_Lambdas_and_Collections", "examples/04_lambdas_and_collections/main.kt"),
    ("05_Coroutines", "examples/05_coroutines/main.kt"),
]

results = []

for name, rel_path in examples:
    full_path = os.path.join(base_dir, rel_path)
    k2_jar = f"/tmp/{name}_k2.jar"
    k3_jar = f"/tmp/{name}_k3.jar"
    k3_native_bin = f"/tmp/{name}_native_bin"
    
    # 1. Official Kotlinc K2 (JVM compilation + runtime JAR)
    k2_cmd = f'{sdkman_init} && kotlinc {full_path} -include-runtime -d {k2_jar}'
    start_k2 = time.perf_counter()
    k2_proc = subprocess.run(["bash", "-c", k2_cmd], capture_output=True, text=True)
    k2_compile_ms = (time.perf_counter() - start_k2) * 1000
    k2_ok = (k2_proc.returncode == 0)

    # 1b. Run K2 JAR
    start_k2_run = time.perf_counter()
    k2_run_proc = subprocess.run(["java", "-jar", k2_jar], capture_output=True, text=True)
    k2_exec_ms = (time.perf_counter() - start_k2_run) * 1000
    k2_output = k2_run_proc.stdout.strip()

    # 2. Kotlin3 IR Lowering (k3 ir)
    start_ir = time.perf_counter()
    k3_ir_proc = subprocess.run([k3_bin, "ir", full_path], capture_output=True, text=True)
    k3_ir_ms = (time.perf_counter() - start_ir) * 1000
    ir_ok = (k3_ir_proc.returncode == 0)

    # 3. Kotlin3 Full Build to Runnable JAR (k3 build)
    start_k3 = time.perf_counter()
    k3_proc = subprocess.run([k3_bin, "build", full_path, "-o", k3_jar], capture_output=True, text=True)
    k3_compile_ms = (time.perf_counter() - start_k3) * 1000
    k3_ok = (k3_proc.returncode == 0)

    # 3b. Run k3 JAR
    start_k3_run = time.perf_counter()
    k3_run_proc = subprocess.run(["java", "-jar", k3_jar], capture_output=True, text=True)
    k3_jvm_exec_ms = (time.perf_counter() - start_k3_run) * 1000
    k3_output = k3_run_proc.stdout.strip()

    # 4. Kotlin3 Standalone Native Binary (k3 native)
    start_native = time.perf_counter()
    k3_native_proc = subprocess.run([k3_bin, "native", full_path, "-o", k3_native_bin], capture_output=True, text=True)
    k3_native_compile_ms = (time.perf_counter() - start_native) * 1000
    native_ok = (k3_native_proc.returncode == 0)

    # 4b. Run Native Binary (measure sub-millisecond execution)
    start_native_run = time.perf_counter()
    k3_native_run_proc = subprocess.run([k3_native_bin], capture_output=True, text=True)
    k3_native_exec_ms = (time.perf_counter() - start_native_run) * 1000
    native_output = k3_native_run_proc.stdout.strip()

    # Metrics
    compile_speedup = (k2_compile_ms / k3_compile_ms) if k3_compile_ms > 0 else 0
    exec_speedup = (k2_exec_ms / k3_native_exec_ms) if k3_native_exec_ms > 0 else 0
    all_match = (k2_output == k3_output == native_output)

    native_size_kb = os.path.getsize(k3_native_bin) // 1024 if os.path.exists(k3_native_bin) else 0

    results.append({
        "name": name,
        "file": rel_path,
        "k2_compile_ms": round(k2_compile_ms, 2),
        "k2_exec_ms": round(k2_exec_ms, 2),
        "k3_ir_ms": round(k3_ir_ms, 2),
        "k3_compile_ms": round(k3_compile_ms, 2),
        "k3_jvm_exec_ms": round(k3_jvm_exec_ms, 2),
        "k3_native_compile_ms": round(k3_native_compile_ms, 2),
        "k3_native_exec_ms": round(k3_native_exec_ms, 2),
        "native_size_kb": native_size_kb,
        "compile_speedup": round(compile_speedup, 1),
        "exec_speedup": round(exec_speedup, 1),
        "all_match": all_match,
        "output": native_output,
    })

print(json.dumps(results, indent=2))
