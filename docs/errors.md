# Kotlin3 Error Codes & Diagnostics Reference

Kotlin3 (`k3 0.7.0`) emits structured CLI errors (`[E0010]`, `[E0030]`, `[E0031]`, `[E0112]`), lexer/syntax diagnostics (`k3_syntax`), semantic FIR diagnostics (`k3_fir`), and package/Maven resolver errors (`k3_pkg`).

---

## Toolchain & I/O Error Codes

### `[E0010]` File Not Found / I/O Error
Emitted when a target `.kt` file, project directory, or output path cannot be opened, read, or written.

- **Common Causes**: Typo in source path, running `k3 check` / `k3 run` without arguments outside a `k3.toml` directory, or insufficient permissions on the destination directory.
- **Resolution**: Verify the file path or run `k3 init` in an empty directory first.

### `[E0030]` Java Runtime Environment (JRE) Not Found
Emitted when `java` cannot be found across `JAVA_HOME`, `~/.k3/toolchains/jdk`, `~/.local/opt/jdk`, `~/.local/bin`, `~/.sdkman`, or `PATH`.

- **Resolution**: Run `k3 toolchain install`, set `JAVA_HOME`, or install OpenJDK 21+ / 25 LTS and add `java` to `PATH`.

### `[E0031]` Kotlin Compiler (`kotlinc`) Not Found
Emitted when a project, `import`, `--kotlinc`, `--strict`, `--fat`, or `k3 test` invocation requires `kotlinc` and no Kotlin installation can be located across `KOTLIN_HOME`, `~/.k3/toolchains/kotlinc`, `~/.local/opt/kotlinc`, `~/.local/bin`, `~/.sdkman`, or `PATH`.

- **Resolution**: Run `k3 toolchain install` (installs latest JetBrains `kotlinc` + Temurin JDK into `~/.k3/toolchains`), or set `KOTLIN_HOME`.

### `[E0112]` Storage Full
Emitted when the OS reports `ENOSPC` / `StorageFull` while writing JARs, lockfiles, or cache entries.

- **Resolution**: Free disk space or run `k3 cache clean` to reclaim space under `~/.k3/cache`.

---

## Semantic Frontend Diagnostics (`k3_fir`)

`k3 check` and `--fast` builds run `k3_fir` before bytecode emission and render line/column underlines for:

| Diagnostic | Meaning |
| :--- | :--- |
| `Unresolved reference: '<name>'` | Identifier, function, or class could not be resolved in current file, sibling `src/` project files, or known imports (`k3.runtime`, `k3.test`). |
| `Type mismatch: inferred type is '<A>' but '<B>' was expected` | Value or expression return type is not a subtype of expected type. |
| `Val cannot be reassigned: '<name>'` | Reassigning an immutable `val` binding; change to `var` if mutation is intended. |
| `Condition must be of type 'Boolean', found '<T>'` | Non-boolean condition passed to `if`, `while`, or `!`. |
| `Wrong number of arguments: expected N, found M` | Too few or too many arguments supplied to a function or primary constructor call. |
| `Conflicting declaration: '<name>'` | Duplicate class, top-level property, or matching-signature function in the same scope. |
| `Cannot infer type for '<name>', please specify type explicitly` | Variable or property declared without initializer or explicit type annotation. |
| `Suspend function '<name>' should be called only from a coroutine or another suspend function` | Calling a `suspend fun` from a regular non-`suspend` function body. |
| `Class '<C>' is not abstract and does not implement abstract member '<m>'` | Concrete class implements an `interface` without defining one of its abstract methods. |

---

## Package & Maven Errors (`k3_pkg`)

- **Invalid coordinate**: `k3 add` expects `group:artifact` or `group:artifact:version` (e.g., `org.json:json`).
- **Download / SHA-256 verification failure**: `k3 sync` / `k3 add` verifies `k3.lock` hashes against downloaded JARs. Re-run `k3 sync --refresh` if upstream coordinates or network connectivity changed.
