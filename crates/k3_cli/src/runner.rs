use k3_pkg::{
    build_fat_jar, cached_jar_lookup, collect_kt_files, compute_sources_hash,
    detect_toolchain_fast, ensure_runtime_jar, java_home_for, runtime_sources_hash,
    store_cached_jar, sync_project, K3Manifest, ToolchainInfo,
};
use k3_syntax::ast::*;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilerMode {
    Auto,
    Fast,
    Kotlinc,
}

pub fn run_target(
    target: Option<&Path>,
    script_or_file: Option<&str>,
    mode: CompilerMode,
    no_cache: bool,
    program_args: &[String],
) -> Result<(), String> {
    let tc = detect_toolchain_fast();
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let manifest_path = cwd.join("k3.toml");

    // 1. If target matches a named script in k3.toml [scripts]
    if let Some(name) = script_or_file {
        if manifest_path.exists() && !Path::new(name).exists() {
            let manifest = K3Manifest::load(&manifest_path)?;
            if let Some(script_val) = manifest.scripts.get(name) {
                let script_path = cwd.join(script_val);
                if script_path.exists() {
                    return run_single_file(&script_path, mode, no_cache, program_args, &tc);
                }
                // Shell-style script fallback
                let status = Command::new("sh")
                    .arg("-c")
                    .arg(script_val)
                    .status()
                    .map_err(|e| format!("Failed executing script '{}': {}", name, e))?;
                if !status.success() {
                    process::exit(status.code().unwrap_or(1));
                }
                return Ok(());
            }
        }
    }

    // 2. Explicit file argument
    if let Some(file) = target {
        if manifest_path.exists() && file.starts_with("src") {
            return run_project(&cwd, Some(file), mode, no_cache, program_args, &tc);
        }
        return run_single_file(file, mode, no_cache, program_args, &tc);
    }

    // 3. Project run via k3.toml in current directory
    if manifest_path.exists() {
        return run_project(&cwd, None, mode, no_cache, program_args, &tc);
    }

    Err("No file specified and no k3.toml found in current directory. Try 'k3 init' or 'k3 run <file.kt>'.".to_string())
}

fn run_single_file(
    file: &Path,
    mode: CompilerMode,
    no_cache: bool,
    program_args: &[String],
    tc: &ToolchainInfo,
) -> Result<(), String> {
    if !file.exists() {
        return Err(format!("[E0010] Source file not found: {}", file.display()));
    }
    let content = fs::read_to_string(file).map_err(|e| format!("[E0010] {}", e))?;
    let ast = parse_kotlin_source(&content, file)?;

    let use_fast = match mode {
        CompilerMode::Fast => true,
        CompilerMode::Kotlinc => false,
        CompilerMode::Auto => is_fast_jvm_compatible(&ast),
    };

    let java_exe = tc.java_exe.as_ref().ok_or_else(jre_not_found_msg)?;

    if use_fast {
        let hash = compute_sources_hash(&[file.to_path_buf()], &["fast-v0.7.0"])?;
        let jar_path = if !no_cache {
            if let Some(cached) = cached_jar_lookup(&hash) {
                cached
            } else {
                let bytes = build_fast_jar_bytes(&ast, file)?;
                let tmp = std::env::temp_dir().join(format!("k3_fast_{}_{}.jar", process::id(), &hash[..8]));
                fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
                let cached = store_cached_jar(&hash, &tmp)?;
                let _ = fs::remove_file(&tmp);
                cached
            }
        } else {
            let bytes = build_fast_jar_bytes(&ast, file)?;
            let tmp = std::env::temp_dir().join(format!("k3_fast_{}.jar", process::id()));
            fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
            tmp
        };

        let status = Command::new(java_exe)
            .arg("-jar")
            .arg(&jar_path)
            .args(program_args)
            .status()
            .map_err(|e| format!("Failed running java: {}", e))?;

        if no_cache {
            let _ = fs::remove_file(&jar_path);
        }
        if !status.success() {
            process::exit(status.code().unwrap_or(1));
        }
        return Ok(());
    }

    // Kotlinc path with content-hash caching + k3-runtime
    let kotlinc_exe = tc.kotlinc_exe.as_ref().ok_or_else(kotlinc_not_found_msg)?;
    let rt_jar = ensure_runtime_jar(kotlinc_exe, tc.java_exe.as_ref(), tc.kotlin_version.as_deref().unwrap_or("2.4.21"))?;
    let hash = compute_sources_hash(
        &[file.to_path_buf()],
        &["kotlinc", tc.kotlin_version.as_deref().unwrap_or("unknown")],
    )?;

    let jar_path = if !no_cache {
        if let Some(cached) = cached_jar_lookup(&hash) {
            cached
        } else {
            let tmp = std::env::temp_dir().join(format!("k3_kc_{}.jar", process::id()));
            compile_with_kotlinc(kotlinc_exe, java_exe, &[file.to_path_buf()], &[rt_jar.clone()], &tmp)?;
            let cached = store_cached_jar(&hash, &tmp)?;
            let _ = fs::remove_file(&tmp);
            cached
        }
    } else {
        let tmp = std::env::temp_dir().join(format!("k3_kc_{}.jar", process::id()));
        compile_with_kotlinc(kotlinc_exe, java_exe, &[file.to_path_buf()], &[rt_jar.clone()], &tmp)?;
        tmp
    };

    let main_class = infer_main_class(file, &ast);
    let mut cp_list = vec![jar_path.clone(), rt_jar];
    cp_list.extend(kotlin_stdlib_jars(kotlinc_exe));
    let cp = join_classpath(&cp_list);

    let status = Command::new(java_exe)
        .arg("-cp")
        .arg(&cp)
        .arg(&main_class)
        .args(program_args)
        .status()
        .map_err(|e| format!("Failed running java: {}", e))?;

    if no_cache {
        let _ = fs::remove_file(&jar_path);
    }
    if !status.success() {
        process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

pub fn run_project(
    project_dir: &Path,
    entry_override: Option<&Path>,
    mode: CompilerMode,
    no_cache: bool,
    program_args: &[String],
    tc: &ToolchainInfo,
) -> Result<(), String> {
    let (manifest, lock, mut dep_jars) = sync_project(project_dir, false)?;
    let src_dir = project_dir.join("src");
    let mut sources = collect_kt_files(&src_dir);
    let entry_path = entry_override
        .map(PathBuf::from)
        .unwrap_or_else(|| project_dir.join(&manifest.package.entry));

    if sources.is_empty() && entry_path.exists() {
        sources.push(entry_path.clone());
    }
    if sources.is_empty() {
        return Err(format!("No Kotlin source files found in {}", src_dir.display()));
    }

    if sources.len() == 1 && dep_jars.is_empty() && mode != CompilerMode::Kotlinc {
        let content = fs::read_to_string(&sources[0]).map_err(|e| e.to_string())?;
        if let Ok(ast) = parse_kotlin_source(&content, &sources[0]) {
            if mode == CompilerMode::Fast || is_fast_jvm_compatible(&ast) {
                return run_single_file(&sources[0], CompilerMode::Fast, no_cache, program_args, tc);
            }
        }
    }

    let java_exe = tc.java_exe.as_ref().ok_or_else(jre_not_found_msg)?;
    let kotlinc_exe = tc.kotlinc_exe.as_ref().ok_or_else(kotlinc_not_found_msg)?;
    let rt_jar = ensure_runtime_jar(kotlinc_exe, tc.java_exe.as_ref(), &manifest.package.kotlin)?;
    dep_jars.push(rt_jar);

    let lock_tag = lock.to_toml_string();
    let rt_tag = runtime_sources_hash(&manifest.package.kotlin);
    let hash = compute_sources_hash(&sources, &["project", &lock_tag, &rt_tag])?;
    let app_jar = if !no_cache {
        if let Some(cached) = cached_jar_lookup(&hash) {
            cached
        } else {
            let tmp = std::env::temp_dir().join(format!("k3_proj_{}.jar", process::id()));
            compile_with_kotlinc(kotlinc_exe, java_exe, &sources, &dep_jars, &tmp)?;
            let cached = store_cached_jar(&hash, &tmp)?;
            let _ = fs::remove_file(&tmp);
            cached
        }
    } else {
        let tmp = std::env::temp_dir().join(format!("k3_proj_{}.jar", process::id()));
        compile_with_kotlinc(kotlinc_exe, java_exe, &sources, &dep_jars, &tmp)?;
        tmp
    };

    let main_class = if let Some(mc) = &manifest.package.main_class {
        mc.clone()
    } else {
        let entry_content = fs::read_to_string(&entry_path).map_err(|e| e.to_string())?;
        let entry_ast = parse_kotlin_source(&entry_content, &entry_path)?;
        infer_main_class(&entry_path, &entry_ast)
    };

    let mut cp_list = vec![app_jar.clone()];
    cp_list.extend(dep_jars);
    cp_list.extend(kotlin_stdlib_jars(kotlinc_exe));
    let cp = join_classpath(&cp_list);

    let status = Command::new(java_exe)
        .arg("-cp")
        .arg(&cp)
        .arg(&main_class)
        .args(program_args)
        .status()
        .map_err(|e| format!("Failed executing JVM: {}", e))?;

    if no_cache {
        let _ = fs::remove_file(&app_jar);
    }
    if !status.success() {
        process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

pub fn build_target(
    file: Option<&Path>,
    output: Option<&Path>,
    mode: CompilerMode,
    fat: bool,
) -> Result<(PathBuf, String), String> {
    let tc = detect_toolchain_fast();
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let manifest_path = cwd.join("k3.toml");

    if file.is_none() && manifest_path.exists() {
        let (manifest, _lock, mut dep_jars) = sync_project(&cwd, false)?;
        let src_dir = cwd.join("src");
        let sources = collect_kt_files(&src_dir);
        if sources.is_empty() {
            return Err("No .kt files found under src/".to_string());
        }
        let out_path = output
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(format!("{}.jar", manifest.package.name)));
        let entry_path = cwd.join(&manifest.package.entry);
        let entry_content = fs::read_to_string(&entry_path).map_err(|e| e.to_string())?;
        let entry_ast = parse_kotlin_source(&entry_content, &entry_path)?;
        let main_class = manifest
            .package
            .main_class
            .clone()
            .unwrap_or_else(|| infer_main_class(&entry_path, &entry_ast));

        if sources.len() == 1 && dep_jars.is_empty() && !fat && mode != CompilerMode::Kotlinc
            && (mode == CompilerMode::Fast || is_fast_jvm_compatible(&entry_ast))
        {
            let (_, bytes) = k3_jvm::compile_to_jar(&entry_ast, sources[0].file_name().unwrap().to_str().unwrap())?;
            fs::write(&out_path, bytes).map_err(|e| e.to_string())?;
            return Ok((out_path, main_class));
        }

        let java_exe = tc.java_exe.as_ref().ok_or_else(jre_not_found_msg)?;
        let kotlinc_exe = tc.kotlinc_exe.as_ref().ok_or_else(kotlinc_not_found_msg)?;
        let jar_exe = tc.jar_exe.as_ref().ok_or_else(|| "JDK 'jar' tool not found".to_string())?;
        let rt_jar = ensure_runtime_jar(kotlinc_exe, tc.java_exe.as_ref(), &manifest.package.kotlin)?;
        dep_jars.push(rt_jar);

        let raw_jar = std::env::temp_dir().join(format!("k3_build_raw_{}.jar", process::id()));
        compile_with_kotlinc(kotlinc_exe, java_exe, &sources, &dep_jars, &raw_jar)?;

        if fat {
            let mut all_deps = dep_jars;
            all_deps.extend(kotlin_stdlib_jars(kotlinc_exe));
            build_fat_jar(jar_exe, &raw_jar, &all_deps, Some(&main_class), &out_path)?;
            let _ = fs::remove_file(&raw_jar);
        } else {
            if let Some(p) = out_path.parent() {
                if !p.as_os_str().is_empty() {
                    fs::create_dir_all(p).map_err(|e| e.to_string())?;
                }
            }
            fs::rename(&raw_jar, &out_path).or_else(|_| fs::copy(&raw_jar, &out_path).map(|_| ())).map_err(|e| e.to_string())?;
            let _ = fs::remove_file(&raw_jar);
        }
        return Ok((out_path, main_class));
    }

    let file = file.ok_or_else(|| "Please pass a .kt file or run inside a k3.toml project".to_string())?;
    let content = fs::read_to_string(file).map_err(|e| format!("[E0010] {}", e))?;
    let ast = parse_kotlin_source(&content, file)?;
    let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("app");
    let out_path = output
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("{}.jar", stem)));

    let use_fast = match mode {
        CompilerMode::Fast => true,
        CompilerMode::Kotlinc => false,
        CompilerMode::Auto => !fat && is_fast_jvm_compatible(&ast),
    };

    if use_fast {
        let (mc, bytes) = k3_jvm::compile_to_jar(&ast, file.file_name().unwrap().to_str().unwrap())?;
        if let Some(p) = out_path.parent() {
            if !p.as_os_str().is_empty() {
                fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
        }
        fs::write(&out_path, bytes).map_err(|e| format!("[E0010] {}", e))?;
        return Ok((out_path, mc.replace('/', ".")));
    }

    let java_exe = tc.java_exe.as_ref().ok_or_else(jre_not_found_msg)?;
    let kotlinc_exe = tc.kotlinc_exe.as_ref().ok_or_else(kotlinc_not_found_msg)?;
    let rt_jar = ensure_runtime_jar(kotlinc_exe, tc.java_exe.as_ref(), tc.kotlin_version.as_deref().unwrap_or("2.4.21"))?;
    let main_class = infer_main_class(file, &ast);

    if fat {
        let jar_exe = tc.jar_exe.as_ref().ok_or_else(|| "JDK 'jar' tool not found".to_string())?;
        let raw_jar = std::env::temp_dir().join(format!("k3_build_raw_{}.jar", process::id()));
        compile_with_kotlinc(kotlinc_exe, java_exe, &[file.to_path_buf()], &[rt_jar.clone()], &raw_jar)?;
        let mut all_deps = vec![rt_jar];
        all_deps.extend(kotlin_stdlib_jars(kotlinc_exe));
        build_fat_jar(jar_exe, &raw_jar, &all_deps, Some(&main_class), &out_path)?;
        let _ = fs::remove_file(&raw_jar);
    } else {
        compile_with_kotlinc(kotlinc_exe, java_exe, &[file.to_path_buf()], &[rt_jar], &out_path)?;
    }

    Ok((out_path, main_class))
}

pub fn run_tests(filter: Option<&str>) -> Result<(), String> {
    let tc = detect_toolchain_fast();
    let java_exe = tc.java_exe.as_ref().ok_or_else(jre_not_found_msg)?;
    let kotlinc_exe = tc.kotlinc_exe.as_ref().ok_or_else(kotlinc_not_found_msg)?;
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let manifest_path = cwd.join("k3.toml");

    let (kotlin_ver, mut dep_jars, src_files, mut test_files) = if manifest_path.exists() {
        let (manifest, _lock, deps) = sync_project(&cwd, false)?;
        let src_dir = cwd.join("src");
        let tests_dir = cwd.join("tests");
        let entry_path = cwd.join(&manifest.package.entry);
        let mut srcs = collect_kt_files(&src_dir);
        // Keep non-entry sources or all src files when entry is distinct
        srcs.retain(|p| p != &entry_path || has_non_main_declarations(p));
        let tests = if let Some(f) = filter {
            let p = PathBuf::from(f);
            if p.exists() { vec![p] } else { collect_kt_files(&tests_dir) }
        } else {
            collect_kt_files(&tests_dir)
        };
        (manifest.package.kotlin, deps, srcs, tests)
    } else if let Some(f) = filter {
        let p = PathBuf::from(f);
        let tests = if p.is_dir() { collect_kt_files(&p) } else { vec![p] };
        ("2.4.21".to_string(), vec![], vec![], tests)
    } else {
        let tests = collect_kt_files(&cwd.join("tests"));
        ("2.4.21".to_string(), vec![], vec![], tests)
    };

    if let Some(f) = filter {
        if !Path::new(f).exists() {
            test_files.retain(|p| p.to_string_lossy().contains(f));
        }
    }

    if test_files.is_empty() {
        return Err("No test files found (expected tests/**/*.kt or explicit test file).".to_string());
    }

    let rt_jar = ensure_runtime_jar(kotlinc_exe, tc.java_exe.as_ref(), &kotlin_ver)?;
    dep_jars.push(rt_jar);

    let mut all_sources = src_files;
    all_sources.extend(test_files.clone());

    let test_jar = std::env::temp_dir().join(format!("k3_test_{}.jar", process::id()));
    compile_with_kotlinc(kotlinc_exe, java_exe, &all_sources, &dep_jars, &test_jar)?;

    let mut cp_list = vec![test_jar.clone()];
    cp_list.extend(dep_jars);
    cp_list.extend(kotlin_stdlib_jars(kotlinc_exe));
    let cp = join_classpath(&cp_list);

    let mut failed = 0usize;
    for tf in &test_files {
        let content = fs::read_to_string(tf).map_err(|e| e.to_string())?;
        let ast = parse_kotlin_source(&content, tf)?;
        let main_cls = infer_main_class(tf, &ast);

        let status = Command::new(java_exe)
            .arg("-cp")
            .arg(&cp)
            .arg(&main_cls)
            .status()
            .map_err(|e| format!("Failed launching test {}: {}", tf.display(), e))?;
        if !status.success() {
            failed += 1;
        }
    }

    let _ = fs::remove_file(&test_jar);
    if failed > 0 {
        return Err(format!("{} test suite(s) failed", failed));
    }
    Ok(())
}

fn has_non_main_declarations(path: &Path) -> bool {
    let Ok(content) = fs::read_to_string(path) else {
        return true;
    };
    let Ok(ast) = parse_kotlin_source(&content, path) else {
        return true;
    };
    ast.declarations.iter().any(|d| !matches!(d, Declaration::Function(f) if f.name.name == "main"))
}

pub fn parse_kotlin_source(content: &str, file: &Path) -> Result<KtFile, String> {
    let file_str = file.to_string_lossy();
    let mut lexer = k3_syntax::Lexer::new(content);
    let tokens = lexer
        .tokenize()
        .map_err(|err| format!("{}:{}: lex error: {}", file_str, err.span, err))?;
    let mut parser = k3_syntax::Parser::new(tokens, content.len() as u32);
    parser
        .parse_file()
        .map_err(|err| format!("{}:{}: syntax error: {}", file_str, err.span, err.message))
}

fn build_fast_jar_bytes(ast: &KtFile, file: &Path) -> Result<Vec<u8>, String> {
    let content = fs::read_to_string(file).map_err(|e| e.to_string())?;
    let file_str = file.to_string_lossy();
    let (_, diagnostics) = k3_fir::analyze(ast);
    let mut has_errors = false;
    for diag in &diagnostics {
        eprintln!("{}", diag.format_with_source(&content, &file_str));
        if diag.severity == k3_fir::DiagnosticSeverity::Error {
            has_errors = true;
        }
    }
    if has_errors {
        return Err(format!("Compilation aborted due to {} semantic error(s)", diagnostics.len()));
    }
    let file_name = file.file_name().unwrap().to_str().unwrap();
    let (_, jar_bytes) = k3_jvm::compile_to_jar(ast, file_name)?;
    Ok(jar_bytes)
}

pub fn compile_with_kotlinc(
    kotlinc_exe: &Path,
    java_exe: &Path,
    sources: &[PathBuf],
    classpath: &[PathBuf],
    out_jar: &Path,
) -> Result<(), String> {
    let mut cmd = Command::new(kotlinc_exe);
    if let Some(jh) = java_home_for(&java_exe.to_path_buf()) {
        cmd.env("JAVA_HOME", jh);
    }
    for src in sources {
        cmd.arg(src);
    }
    if !classpath.is_empty() {
        cmd.arg("-cp").arg(join_classpath(classpath));
    }
    cmd.arg("-d").arg(out_jar);

    let out = cmd
        .output()
        .map_err(|e| format!("Failed launching kotlinc: {}", e))?;
    if !out.status.success() {
        return Err(format!(
            "kotlinc compilation failed:\n{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

pub fn is_fast_jvm_compatible(file: &KtFile) -> bool {
    if !file.imports.is_empty() {
        return false;
    }
    let mut suspend_fns = HashSet::new();
    for decl in &file.declarations {
        if let Declaration::Function(f) = decl {
            if f.modifiers.contains(&Modifier::Suspend) {
                suspend_fns.insert(f.name.name.clone());
            }
        }
        if let Declaration::Class(c) = decl {
            if !matches!(c.kind, ClassKind::Class | ClassKind::Interface | ClassKind::DataClass) {
                return false;
            }
        }
    }
    for decl in &file.declarations {
        if let Declaration::Function(f) = decl {
            if let Some(body) = &f.body {
                match body {
                    FunctionBody::Block(stmts) => {
                        if !stmts.iter().all(|s| stmt_fast_compatible(s, &suspend_fns)) {
                            return false;
                        }
                    }
                    FunctionBody::Expression(e) => {
                        if !expr_fast_compatible(e, &suspend_fns) {
                            return false;
                        }
                    }
                }
            }
        }
    }
    true
}

fn stmt_fast_compatible(stmt: &Statement, suspend_fns: &HashSet<String>) -> bool {
    match stmt {
        Statement::Declaration(Declaration::Property(p)) => p
            .initializer
            .as_ref()
            .map_or(true, |e| expr_fast_compatible(e, suspend_fns)),
        Statement::Declaration(_) => false,
        Statement::Assignment { target, value, .. } => {
            expr_fast_compatible(target, suspend_fns) && expr_fast_compatible(value, suspend_fns)
        }
        Statement::Expression(e) => expr_fast_compatible(e, suspend_fns),
    }
}

fn expr_fast_compatible(expr: &Expression, suspend_fns: &HashSet<String>) -> bool {
    match expr {
        Expression::Super(_) | Expression::Continue(_) => false,
        Expression::Call { callee, args, trailing_lambda, .. } => {
            if let Expression::Identifier(id) = &**callee {
                if suspend_fns.contains(&id.name) {
                    return false;
                }
            }
            expr_fast_compatible(callee, suspend_fns)
                && args.iter().all(|a| expr_fast_compatible(&a.value, suspend_fns))
                && trailing_lambda
                    .as_ref()
                    .map_or(true, |l| expr_fast_compatible(l, suspend_fns))
        }
        Expression::Binary { left, right, .. } => {
            expr_fast_compatible(left, suspend_fns) && expr_fast_compatible(right, suspend_fns)
        }
        Expression::Unary { expr, .. } => expr_fast_compatible(expr, suspend_fns),
        Expression::MemberAccess { target, .. } => expr_fast_compatible(target, suspend_fns),
        Expression::IndexAccess { target, indices, .. } => {
            expr_fast_compatible(target, suspend_fns)
                && indices.iter().all(|i| expr_fast_compatible(i, suspend_fns))
        }
        Expression::If { condition, then_branch, else_branch, .. } => {
            expr_fast_compatible(condition, suspend_fns)
                && expr_fast_compatible(then_branch, suspend_fns)
                && else_branch
                    .as_ref()
                    .map_or(true, |e| expr_fast_compatible(e, suspend_fns))
        }
        Expression::When { subject, entries, .. } => {
            subject
                .as_ref()
                .map_or(true, |s| expr_fast_compatible(s, suspend_fns))
                && entries
                    .iter()
                    .all(|e| expr_fast_compatible(&e.body, suspend_fns))
        }
        Expression::Block(stmts, _) | Expression::Lambda { body: stmts, .. } => {
            stmts.iter().all(|s| stmt_fast_compatible(s, suspend_fns))
        }
        Expression::Return { value, .. } => value
            .as_ref()
            .map_or(true, |v| expr_fast_compatible(v, suspend_fns)),
        Expression::For { iterable, body, .. } => {
            expr_fast_compatible(iterable, suspend_fns) && expr_fast_compatible(body, suspend_fns)
        }
        Expression::While { condition, body, .. } => {
            expr_fast_compatible(condition, suspend_fns) && expr_fast_compatible(body, suspend_fns)
        }
        Expression::StringTemplate(parts, _) => parts.iter().all(|p| expr_fast_compatible(p, suspend_fns)),
        _ => true,
    }
}

pub fn infer_main_class(file: &Path, ast: &KtFile) -> String {
    let stem = file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Main");
    let mut chars = stem.chars();
    let cap = match chars.next() {
        None => "Main".to_string(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    };
    let cls_base = format!("{}Kt", cap);
    if let Some(pkg) = &ast.package {
        format!("{}.{}", pkg.name.to_string(), cls_base)
    } else {
        cls_base
    }
}

pub fn kotlin_stdlib_jars(kotlinc_exe: &Path) -> Vec<PathBuf> {
    let canonical = fs::canonicalize(kotlinc_exe).unwrap_or_else(|_| kotlinc_exe.to_path_buf());
    let Some(lib_dir) = canonical.parent().and_then(|b| b.parent()).map(|h| h.join("lib")) else {
        return vec![];
    };
    let mut jars = Vec::new();
    for name in ["kotlin-stdlib.jar", "kotlin-reflect.jar", "annotations-13.0.jar"] {
        let p = lib_dir.join(name);
        if p.exists() {
            jars.push(p);
        }
    }
    jars
}

pub fn join_classpath(paths: &[PathBuf]) -> String {
    let sep = if cfg!(windows) { ";" } else { ":" };
    paths
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(sep)
}

pub fn jre_not_found_msg() -> String {
    "[E0030] Java Runtime Environment (JRE) Not Found: 'java' could not be located.\n\nFix: Run 'k3 toolchain install' or set JAVA_HOME / install OpenJDK 21+.".to_string()
}

pub fn kotlinc_not_found_msg() -> String {
    "[E0031] Kotlin Compiler (kotlinc) Not Found: 'kotlinc' could not be located.\n\nFix: Run 'k3 toolchain install' or install Kotlin 2.4+ and add it to PATH.".to_string()
}
