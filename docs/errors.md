# 🚨 Kotlin3 (k3) Diagnostic Error Codes & Troubleshooting Guide

Kotlin3 provides structured, machine-parsable error codes (prefixed with `[E....]`) accompanied by clear explanations, root cause analysis, and actionable remediation steps.

---

## 📋 Error Code Catalog

| Error Code | Category | Name | Summary |
|---|---|---|---|
| [`[E0001]`](#e0001---zig-native-compiler-not-found) | Toolchain | Zig Native Compiler Not Found | Attempted standalone `.exe`/native compilation without Zig installed |
| [`[E0002]`](#e0002---cli-argument-or-flag-syntax-mismatch) | CLI | CLI Argument Syntax Mismatch | Provided unrecognized or incorrectly positioned CLI options |
| [`[E0010]`](#e0010---io-filesystem-access-error) | Filesystem | File I/O Access Error | Could not read or write file due to permissions or lock |
| [`[E0020]`](#e0020---codegen-failure) | Compiler | Codegen Synthesis Error | Failure during JVM bytecode or Native IR generation |
| [`[E0030]`](#e0030---java-runtime-environment-not-found) | Runtime | Java Runtime Not Found | JVM execution requested but no Java runtime (JBR/JDK) detected |
| [`[E0112]`](#e0112---storage-full--disk-exhaustion) | Filesystem | Storage Full / Disk Exhaustion | Operating system reported zero available storage space on disk |

---

## 🔍 Detailed Explanations & Actionable Solutions

### `[E0001]` - Zig Native Compiler Not Found

#### ⚠️ Symptoms
```
❌ Native compilation error:
[E0001] Zig Native Compiler Not Found

Explanation: 'k3 native' compiles Kotlin directly into standalone native machine-code executables using Zig's zero-dependency linker.
```

#### 💡 Root Cause
You invoked `k3 native <file.kt>` or `k3 run --native <file.kt>`, which compiles Kotlin code directly to standalone, zero-JVM native machine binaries (`.exe` on Windows, ELF on Linux, Mach-O on macOS). Kotlin3 uses the lightweight Zig cross-linker instead of a bloated 2 GB LLVM toolchain. If Zig (`zig.exe` or `zig`) is not found in your system `PATH`, this diagnostic is raised.

#### 🛠️ Actionable Solutions
- **Option A (Zero-dependency JVM execution)**: You **do not** need Zig if you simply want to build or run Kotlin programs. Use:
  ```powershell
  # Auto-detects JVM and runs in < 2ms without Zig
  k3 run app.kt

  # Or build a standard runnable JAR
  k3 build app.kt -o app.jar
  java -jar app.jar
  ```
- **Option B (Install Zig for standalone native `.exe` generation)**:
  - **Windows (Winget)**:
    ```powershell
    winget install zig.zig
    ```
  - **Windows (Direct Download)**: Download from [ziglang.org/download](https://ziglang.org/download/) and extract to `C:\zig` (already auto-scanned by `k3`).
  - **Linux (Ubuntu/Debian)**:
    ```bash
    sudo snap install zig --classic --beta
    # or download from https://ziglang.org/download/
    ```

---

### `[E0002]` - CLI Argument or Flag Syntax Mismatch

#### ⚠️ Symptoms
```
error: unexpected argument '-o' found
  tip: to pass '-o' as a value, use '-- -o'
Usage: k3.exe run [OPTIONS] <FILE>
```

#### 💡 Root Cause
Older releases of `k3 run` only accepted the source file without the optional output file flag `-o` / `--output`.

#### 🛠️ Actionable Solutions
Update to Kotlin3 `v0.6.0+` where `-o` is fully supported on both `k3 build` and `k3 run`:
```powershell
# Compile and immediately run, saving compiled output:
k3 run app.kt -o app.jar

# Or simply build without running:
k3 build app.kt -o app.jar
```

---

### `[E0112]` - Storage Full / Disk Exhaustion

#### ⚠️ Symptoms
```
Error: Os { code: 112, kind: StorageFull, message: "There is not enough space on the disk." }
```

#### 💡 Root Cause
Windows OS error code 112 (`ERROR_DISK_FULL`) indicates the target drive (e.g., `C:`) or temporary directory has run out of physical disk space (`0.00 GB free`). Kotlin3 cannot write compilation artifacts, JAR files, or cache entries.

#### 🛠️ Actionable Solutions
1. Check free space using PowerShell:
   ```powershell
   Get-PSDrive C | Select-Object Used, Free
   ```
2. Free up disk space by clearing temporary folders:
   ```powershell
   Remove-Item -Path "$env:TEMP\*" -Recurse -Force -ErrorAction SilentlyContinue
   ```
3. Empty the Windows Recycle Bin or remove large unused files. Once free space is above 1 GB, rerun `k3 build app.kt`.

---

### `[E0030]` - Java Runtime Environment Not Found

#### ⚠️ Symptoms
```
❌ [E0030] Java Runtime Not Found: 'java' could not be located to execute .jar files.
```

#### 💡 Root Cause
`k3 run <file.kt>` built the program into JVM bytecode, but neither `java.exe` in `PATH`, nor `JAVA_HOME`, nor JetBrains IDE JBR (`C:\Program Files\JetBrains\IntelliJ IDEA...\jbr\bin\java.exe`) was found on your system.

#### 🛠️ Actionable Solutions
1. Install OpenJDK on Windows via Winget:
   ```powershell
   winget install Microsoft.OpenJDK.21
   ```
2. Or set `JAVA_HOME` pointing to your existing JDK directory:
   ```powershell
   [Environment]::SetEnvironmentVariable("JAVA_HOME", "C:\Program Files\Eclipse Adoptium\jdk-21-hotspot", "User")
   ```
3. Alternatively, compile to standalone native executable using Zig (`winget install zig.zig`) to bypass Java completely!
