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
    
    # 1. Benchmark Official Kotlinc K2 (JVM compilation + pack runtime)
    k2_cmd = f'{sdkman_init} && kotlinc {full_path} -include-runtime -d {k2_jar}'
    start_k2 = time.perf_counter()
    k2_proc = subprocess.run(["bash", "-c", k2_cmd], capture_output=True, text=True)
    k2_compile_ms = (time.perf_counter() - start_k2) * 1000
    k2_ok = (k2_proc.returncode == 0)

    # 1b. Run K2 JAR
    k2_run_proc = subprocess.run(["java", "-jar", k2_jar], capture_output=True, text=True)
    k2_output = k2_run_proc.stdout.strip()

    # 2. Benchmark Kotlin3 IR Lowering (k3 ir)
    start_ir = time.perf_counter()
    k3_ir_proc = subprocess.run([k3_bin, "ir", full_path], capture_output=True, text=True)
    k3_ir_ms = (time.perf_counter() - start_ir) * 1000
    ir_ok = (k3_ir_proc.returncode == 0)

    # 3. Benchmark Kotlin3 Full Build to Runnable JAR (k3 build)
    start_k3 = time.perf_counter()
    k3_proc = subprocess.run([k3_bin, "build", full_path, "-o", k3_jar], capture_output=True, text=True)
    k3_compile_ms = (time.perf_counter() - start_k3) * 1000
    k3_ok = (k3_proc.returncode == 0)

    # 3b. Run k3 JAR
    k3_run_proc = subprocess.run(["java", "-jar", k3_jar], capture_output=True, text=True)
    k3_output = k3_run_proc.stdout.strip()

    speedup = (k2_compile_ms / k3_compile_ms) if k3_compile_ms > 0 else 0
    outputs_match = (k2_output == k3_output)

    results.append({
        "name": name,
        "file": rel_path,
        "k2_compile_ms": round(k2_compile_ms, 2),
        "k2_ok": k2_ok,
        "k3_ir_ms": round(k3_ir_ms, 2),
        "k3_ir_ok": ir_ok,
        "k3_compile_ms": round(k3_compile_ms, 2),
        "k3_ok": k3_ok,
        "speedup": round(speedup, 1),
        "outputs_match": outputs_match,
        "output": k3_output,
    })

print(json.dumps(results, indent=2))
