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
    jar_out = f"/tmp/{name}.jar"
    
    # 1. Benchmark Official Kotlinc K2
    k2_cmd = f'cd {base_dir} && {sdkman_init} && kotlinc {full_path} -include-runtime -d {jar_out}'
    start_k2 = time.perf_counter()
    k2_proc = subprocess.run(["bash", "-c", k2_cmd], capture_output=True, text=True)
    k2_duration = (time.perf_counter() - start_k2) * 1000  # ms
    k2_ok = (k2_proc.returncode == 0)

    # 2. Benchmark Kotlin3 (k3)
    start_k3 = time.perf_counter()
    k3_proc = subprocess.run([k3_bin, "check", full_path], capture_output=True, text=True)
    k3_duration = (time.perf_counter() - start_k3) * 1000  # ms
    k3_ok = (k3_proc.returncode == 0)

    speedup = (k2_duration / k3_duration) if k3_duration > 0 else 0

    results.append({
        "name": name,
        "file": rel_path,
        "k2_time_ms": round(k2_duration, 2),
        "k2_ok": k2_ok,
        "k3_time_ms": round(k3_duration, 2),
        "k3_ok": k3_ok,
        "speedup": round(speedup, 1)
    })

print(json.dumps(results, indent=2))
