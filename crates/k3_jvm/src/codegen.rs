use crate::bytecode::CodeBuilder;
use crate::classfile::{ClassFile, CodeAttr};
use k3_syntax::ast::*;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JvmType {
    Int,            // I (also Boolean, Byte, Short, Char)
    Long,           // J (2 slots, category 2)
    Float,          // F (1 slot)
    Double,         // D (2 slots, category 2)
    Object(String), // L...; or [...]
    Void,           // V
}

impl JvmType {
    pub fn descriptor(&self) -> String {
        match self {
            JvmType::Int => "I".to_string(),
            JvmType::Long => "J".to_string(),
            JvmType::Float => "F".to_string(),
            JvmType::Double => "D".to_string(),
            JvmType::Object(desc) => desc.clone(),
            JvmType::Void => "V".to_string(),
        }
    }

    pub fn slots(&self) -> u16 {
        match self {
            JvmType::Long | JvmType::Double => 2,
            JvmType::Void => 0,
            _ => 1,
        }
    }

    pub fn emit_load(&self, code: &mut CodeBuilder, slot: u16) {
        match self {
            JvmType::Int => code.iload(slot),
            JvmType::Long => code.lload(slot),
            JvmType::Float => code.fload(slot),
            JvmType::Double => code.dload(slot),
            JvmType::Object(_) => code.aload(slot),
            JvmType::Void => {}
        }
    }

    pub fn emit_store(&self, code: &mut CodeBuilder, slot: u16) {
        match self {
            JvmType::Int => code.istore(slot),
            JvmType::Long => code.lstore(slot),
            JvmType::Float => code.fstore(slot),
            JvmType::Double => code.dstore(slot),
            JvmType::Object(_) => code.astore(slot),
            JvmType::Void => {}
        }
    }

    pub fn emit_return(&self, code: &mut CodeBuilder) {
        match self {
            JvmType::Int => code.ireturn(),
            JvmType::Long => code.lreturn(),
            JvmType::Float => code.ireturn(),
            JvmType::Double => code.dreturn(),
            JvmType::Object(_) => code.areturn(),
            JvmType::Void => code.return_void(),
        }
    }

    pub fn emit_pop(&self, code: &mut CodeBuilder) {
        match self {
            JvmType::Long | JvmType::Double => code.pop2(),
            JvmType::Void => {}
            _ => code.pop(),
        }
    }
}

pub fn type_ref_to_jvm(tr: &TypeRef, package_prefix: &str) -> JvmType {
    match tr {
        TypeRef::User { name, .. } => {
            let n = name.to_string();
            match n.as_str() {
                "Int" => JvmType::Int,
                "Long" => JvmType::Long,
                "Boolean" => JvmType::Int,
                "Byte" => JvmType::Int,
                "Short" => JvmType::Int,
                "Char" => JvmType::Int,
                "Float" => JvmType::Float,
                "Double" => JvmType::Double,
                "String" => JvmType::Object("Ljava/lang/String;".to_string()),
                "Unit" => JvmType::Void,
                "IntArray" => JvmType::Object("[I".to_string()),
                "ByteArray" => JvmType::Object("[B".to_string()),
                "LongArray" => JvmType::Object("[J".to_string()),
                "DoubleArray" => JvmType::Object("[D".to_string()),
                "Any" => JvmType::Object("Ljava/lang/Object;".to_string()),
                _ => JvmType::Object(format!("L{}{};", package_prefix, n)),
            }
        }
        TypeRef::Nullable { inner, .. } => type_ref_to_jvm(inner, package_prefix),
        TypeRef::Function { .. } => JvmType::Object("Ljava/lang/Object;".to_string()),
    }
}

#[derive(Debug, Clone)]
pub struct FunctionSignature {
    pub param_types: Vec<JvmType>,
    pub return_type: JvmType,
    pub is_inline: bool,
}

#[derive(Debug, Clone)]
pub struct ClassInfo {
    pub internal_name: String,
    pub fields: Vec<(String, JvmType)>,
    pub constructor_params: Vec<JvmType>,
}

fn emit_iconst(code: &mut CodeBuilder, cf: &mut ClassFile, val: i32) {
    if (-32768..=32767).contains(&val) {
        code.iconst(val);
    } else {
        let idx = cf.cp.add_integer(val);
        code.ldc(idx);
    }
}

pub struct Codegen {
    pub classes: Vec<(String, ClassFile)>,
    pub current_package_prefix: String,
    pub current_facade_name: String,
    pub current_ret_type: JvmType,
    pub functions: HashMap<String, FunctionSignature>,
    pub class_infos: HashMap<String, ClassInfo>,
    pub top_level_constants: HashMap<String, (i64, JvmType)>,
}

impl Codegen {
    pub fn new() -> Self {
        Self {
            classes: Vec::new(),
            current_package_prefix: String::new(),
            current_facade_name: String::new(),
            current_ret_type: JvmType::Void,
            functions: HashMap::new(),
            class_infos: HashMap::new(),
            top_level_constants: HashMap::new(),
        }
    }

    pub fn compile_file(
        &mut self,
        file: &KtFile,
        source_file_name: &str,
    ) -> Result<Vec<(String, Vec<u8>)>, String> {
        let package_prefix = file
            .package
            .as_ref()
            .map(|p| p.name.to_string().replace('.', "/") + "/")
            .unwrap_or_default();

        self.current_package_prefix = package_prefix.clone();

        let stem = source_file_name
            .trim_end_matches(".kt")
            .trim_end_matches(".kts");
        let class_base = capitalize_first(stem) + "Kt";
        let facade_class_name = format!("{}{}", package_prefix, class_base);
        self.current_facade_name = facade_class_name.clone();

        // Phase 1: Index all classes, top-level constants and functions
        for decl in &file.declarations {
            match decl {
                Declaration::Class(c) => {
                    let internal_name = format!("{}{}", package_prefix, c.name.name);
                    let mut fields = Vec::new();
                    let mut constructor_params = Vec::new();
                    if let Some(params) = &c.primary_constructor_params {
                        for p in params {
                            let ty = type_ref_to_jvm(&p.type_ref, &package_prefix);
                            fields.push((p.name.name.clone(), ty.clone()));
                            constructor_params.push(ty);
                        }
                    }
                    self.class_infos.insert(
                        c.name.name.clone(),
                        ClassInfo {
                            internal_name,
                            fields,
                            constructor_params,
                        },
                    );
                }
                Declaration::Property(p) => {
                    // Check top-level const val ONE = 1L shl 16 (or integer/long literal)
                    if let Some(init) = &p.initializer {
                        if let Some(val) = self.eval_const_int(init) {
                            let ty = p.type_ref.as_ref()
                                .map(|tr| type_ref_to_jvm(tr, &package_prefix))
                                .unwrap_or(JvmType::Long);
                            self.top_level_constants.insert(p.name.name.clone(), (val, ty));
                        }
                    }
                }
                Declaration::Function(f) => {
                    let is_main = f.name.name == "main";
                    let (param_types, return_type) = if is_main {
                        (
                            vec![JvmType::Object("[Ljava/lang/String;".to_string())],
                            JvmType::Void,
                        )
                    } else {
                        let mut pts = Vec::new();
                        for p in &f.value_params {
                            pts.push(type_ref_to_jvm(&p.type_ref, &package_prefix));
                        }
                        let rt = f
                            .return_type
                            .as_ref()
                            .map(|tr| type_ref_to_jvm(tr, &package_prefix))
                            .unwrap_or(JvmType::Void);
                        (pts, rt)
                    };
                    let is_inline = f.modifiers.contains(&Modifier::Inline);
                    self.functions.insert(
                        f.name.name.clone(),
                        FunctionSignature {
                            param_types,
                            return_type,
                            is_inline,
                        },
                    );
                }
            }
        }

        // Phase 2: Compile class declarations (e.g. data classes)
        for decl in &file.declarations {
            if let Declaration::Class(c) = decl {
                let class_info = self.class_infos.get(&c.name.name).unwrap().clone();
                let mut cf = ClassFile::new(&class_info.internal_name, "java/lang/Object");
                cf.source_file = Some(source_file_name.to_string());
                self.compile_class_declaration(c, &class_info, &mut cf)?;
                self.classes.push((class_info.internal_name.clone(), cf));
            }
        }

        // Phase 3: Compile facade class with top-level functions
        let mut facade = ClassFile::new(&facade_class_name, "java/lang/Object");
        facade.source_file = Some(source_file_name.to_string());

        // Default <init>() constructor
        let mut init_code = CodeBuilder::new();
        init_code.aload(0);
        let super_init = facade.cp.add_methodref("java/lang/Object", "<init>", "()V");
        init_code.invokespecial(super_init, 0);
        init_code.return_void();
        facade.add_method(
            0x0001, // ACC_PUBLIC
            "<init>",
            "()V",
            Some(CodeAttr {
                max_stack: init_code.max_stack.max(1),
                max_locals: init_code.max_locals.max(1),
                bytecode: init_code.bytes,
            }),
        );

        for decl in &file.declarations {
            if let Declaration::Function(f) = decl {
                self.compile_function(f, &mut facade)?;
            }
        }

        self.classes.push((facade_class_name, facade));

        // Serialize all classfiles to byte vectors
        let mut results = Vec::new();
        for (name, mut cf) in self.classes.drain(..) {
            let bytes = cf.to_bytes();
            results.push((name, bytes));
        }

        Ok(results)
    }

    fn eval_const_int(&self, expr: &Expression) -> Option<i64> {
        match expr {
            Expression::IntegerLiteral(v, _) => Some(*v),
            Expression::LongLiteral(v, _) => Some(*v),
            Expression::Binary { left, op, right, .. } => {
                let l = self.eval_const_int(left)?;
                let r = self.eval_const_int(right)?;
                match op {
                    BinaryOp::Add => Some(l + r),
                    BinaryOp::Sub => Some(l - r),
                    BinaryOp::Mul => Some(l * r),
                    BinaryOp::Div => if r != 0 { Some(l / r) } else { None },
                    BinaryOp::Shl => Some(l << r),
                    BinaryOp::Shr => Some(l >> r),
                    BinaryOp::BitAnd => Some(l & r),
                    BinaryOp::BitOr => Some(l | r),
                    BinaryOp::BitXor => Some(l ^ r),
                    _ => None,
                }
            }
            Expression::Identifier(id) => {
                if let Some((v, _)) = self.top_level_constants.get(&id.name) {
                    Some(*v)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn compile_class_declaration(
        &mut self,
        c: &ClassDeclaration,
        info: &ClassInfo,
        cf: &mut ClassFile,
    ) -> Result<(), String> {
        cf.access_flags = 0x0001 | 0x0020; // ACC_PUBLIC | ACC_SUPER

        // Add fields
        for (fname, fty) in &info.fields {
            cf.add_field(0x0001 /* ACC_PUBLIC */, fname, &fty.descriptor());
        }

        // Add constructor <init>(...)V
        let param_desc = info.constructor_params.iter().map(|t| t.descriptor()).collect::<Vec<_>>().join("");
        let ctor_desc = format!("({})V", param_desc);
        let mut init_code = CodeBuilder::new();
        init_code.aload(0);
        let super_init = cf.cp.add_methodref("java/lang/Object", "<init>", "()V");
        init_code.invokespecial(super_init, 0);

        let mut param_slot = 1u16;
        for (fname, fty) in &info.fields {
            init_code.aload(0);
            fty.emit_load(&mut init_code, param_slot);
            let f_idx = cf.cp.add_fieldref(&info.internal_name, fname, &fty.descriptor());
            init_code.putfield(f_idx);
            param_slot += fty.slots();
        }
        init_code.return_void();

        cf.add_method(
            0x0001, // ACC_PUBLIC
            "<init>",
            &ctor_desc,
            Some(CodeAttr {
                max_stack: 3,
                max_locals: param_slot.max(1),
                bytecode: init_code.bytes,
            }),
        );

        // Add getters for properties
        for (fname, fty) in &info.fields {
            let getter_name = format!("get{}", capitalize_first(fname));
            let getter_desc = format!("(){}", fty.descriptor());
            let mut g_code = CodeBuilder::new();
            g_code.aload(0);
            let f_idx = cf.cp.add_fieldref(&info.internal_name, fname, &fty.descriptor());
            g_code.getfield(f_idx);
            fty.emit_return(&mut g_code);

            cf.add_method(
                0x0001, // ACC_PUBLIC
                &getter_name,
                &getter_desc,
                Some(CodeAttr {
                    max_stack: 1 + fty.slots(),
                    max_locals: 1,
                    bytecode: g_code.bytes,
                }),
            );
        }

        // Compile member functions if any
        for inner in &c.declarations {
            if let Declaration::Function(f) = inner {
                let mut code = CodeBuilder::new();
                let mut locals = HashMap::new();
                locals.insert("this".to_string(), (0, JvmType::Object(format!("L{};", info.internal_name))));
                let mut next_slot = 1u16;
                let mut param_descs = Vec::new();
                for p in &f.value_params {
                    let pty = type_ref_to_jvm(&p.type_ref, &self.current_package_prefix);
                    param_descs.push(pty.descriptor());
                    locals.insert(p.name.name.clone(), (next_slot, pty.clone()));
                    next_slot += pty.slots();
                }
                let ret_ty = f.return_type.as_ref()
                    .map(|tr| type_ref_to_jvm(tr, &self.current_package_prefix))
                    .unwrap_or(JvmType::Void);
                self.current_ret_type = ret_ty.clone();

                let mut breaks = Vec::new();
                if let Some(body) = &f.body {
                    match body {
                        FunctionBody::Block(stmts) => {
                            for s in stmts {
                                self.compile_statement(s, &mut code, cf, &mut locals, &mut next_slot, &mut breaks)?;
                            }
                            ret_ty.emit_return(&mut code);
                        }
                        FunctionBody::Expression(e) => {
                            let ety = self.compile_expression(e, &mut code, cf, &mut locals, &mut next_slot, &mut breaks)?;
                            if ret_ty == JvmType::Long && ety == JvmType::Int {
                                code.i2l();
                            }
                            ret_ty.emit_return(&mut code);
                        }
                    }
                } else {
                    ret_ty.emit_return(&mut code);
                }

                let m_desc = format!("({}){}", param_descs.join(""), ret_ty.descriptor());
                cf.add_method(
                    0x0001,
                    &f.name.name,
                    &m_desc,
                    Some(CodeAttr {
                        max_stack: code.max_stack.max(4),
                        max_locals: code.max_locals.max(next_slot).max(1),
                        bytecode: code.bytes,
                    }),
                );
            }
        }

        Ok(())
    }

    fn compile_function(&mut self, f: &NamedFunction, cf: &mut ClassFile) -> Result<(), String> {
        let is_main = f.name.name == "main";
        let sig = self.functions.get(&f.name.name).unwrap().clone();
        self.current_ret_type = sig.return_type.clone();

        let param_desc = sig.param_types.iter().map(|t| t.descriptor()).collect::<Vec<_>>().join("");
        let descriptor = format!("({}){}", param_desc, sig.return_type.descriptor());

        let mut code = CodeBuilder::new();
        let mut locals = HashMap::new();
        let mut next_slot = 0u16;

        if is_main {
            locals.insert("args".to_string(), (0, JvmType::Object("[Ljava/lang/String;".to_string())));
            next_slot = 1;
        } else {
            for (idx, p) in f.value_params.iter().enumerate() {
                let pty = sig.param_types[idx].clone();
                let slot = next_slot;
                next_slot += pty.slots();
                locals.insert(p.name.name.clone(), (slot, pty));
            }
        }

        let mut breaks = Vec::new();

        if let Some(body) = &f.body {
            match body {
                FunctionBody::Block(statements) => {
                    let mut has_returned = false;
                    for stmt in statements {
                        if let Statement::Expression(Expression::Return { .. }) = stmt {
                            has_returned = true;
                        }
                        self.compile_statement(stmt, &mut code, cf, &mut locals, &mut next_slot, &mut breaks)?;
                    }
                    if !has_returned {
                        sig.return_type.emit_return(&mut code);
                    }
                }
                FunctionBody::Expression(expr) => {
                    let ty = self.compile_expression(expr, &mut code, cf, &mut locals, &mut next_slot, &mut breaks)?;
                    if sig.return_type == JvmType::Long && ty == JvmType::Int {
                        code.i2l();
                    }
                    sig.return_type.emit_return(&mut code);
                }
            }
        } else {
            sig.return_type.emit_return(&mut code);
        }

        let access_flags = 0x0001 | 0x0008; // ACC_PUBLIC | ACC_STATIC
        cf.add_method(
            access_flags,
            &f.name.name,
            &descriptor,
            Some(CodeAttr {
                max_stack: code.max_stack.max(8),
                max_locals: code.max_locals.max(next_slot).max(1),
                bytecode: code.bytes,
            }),
        );

        Ok(())
    }

    fn compile_statement(
        &mut self,
        stmt: &Statement,
        code: &mut CodeBuilder,
        cf: &mut ClassFile,
        locals: &mut HashMap<String, (u16, JvmType)>,
        next_slot: &mut u16,
        break_targets: &mut Vec<usize>,
    ) -> Result<(), String> {
        match stmt {
            Statement::Declaration(decl) => match decl {
                Declaration::Property(p) => {
                    let slot = *next_slot;
                    let declared_ty = p.type_ref.as_ref().map(|tr| type_ref_to_jvm(tr, &self.current_package_prefix));
                    if let Some(init) = &p.initializer {
                        let init_ty = self.compile_expression(init, code, cf, locals, next_slot, break_targets)?;
                        let actual_ty = match declared_ty {
                            Some(JvmType::Long) if init_ty == JvmType::Int => {
                                code.i2l();
                                JvmType::Long
                            }
                            Some(d) => d,
                            None => init_ty,
                        };
                        *next_slot = slot + actual_ty.slots();
                        actual_ty.emit_store(code, slot);
                        locals.insert(p.name.name.clone(), (slot, actual_ty));
                    } else {
                        let ty = declared_ty.unwrap_or(JvmType::Int);
                        *next_slot = slot + ty.slots();
                        locals.insert(p.name.name.clone(), (slot, ty));
                    }
                }
                _ => {}
            },
            Statement::Assignment { target, op, value, .. } => {
                match target {
                    Expression::Identifier(id) => {
                        if let Some((slot, var_ty)) = locals.get(&id.name).cloned() {
                            match op {
                                AssignOp::Assign => {
                                    let val_ty = self.compile_expression(value, code, cf, locals, next_slot, break_targets)?;
                                    if var_ty == JvmType::Long && val_ty == JvmType::Int {
                                        code.i2l();
                                    }
                                    var_ty.emit_store(code, slot);
                                }
                                AssignOp::PlusAssign => {
                                    var_ty.emit_load(code, slot);
                                    let val_ty = self.compile_expression(value, code, cf, locals, next_slot, break_targets)?;
                                    if var_ty == JvmType::Long && val_ty == JvmType::Int {
                                        code.i2l();
                                    }
                                    if var_ty == JvmType::Long {
                                        code.ladd();
                                    } else {
                                        code.iadd();
                                    }
                                    var_ty.emit_store(code, slot);
                                }
                                AssignOp::MinusAssign => {
                                    var_ty.emit_load(code, slot);
                                    let val_ty = self.compile_expression(value, code, cf, locals, next_slot, break_targets)?;
                                    if var_ty == JvmType::Long && val_ty == JvmType::Int {
                                        code.i2l();
                                    }
                                    if var_ty == JvmType::Long {
                                        code.lsub();
                                    } else {
                                        code.isub();
                                    }
                                    var_ty.emit_store(code, slot);
                                }
                                AssignOp::MulAssign => {
                                    var_ty.emit_load(code, slot);
                                    let val_ty = self.compile_expression(value, code, cf, locals, next_slot, break_targets)?;
                                    if var_ty == JvmType::Long && val_ty == JvmType::Int {
                                        code.i2l();
                                    }
                                    if var_ty == JvmType::Long {
                                        code.lmul();
                                    } else {
                                        code.imul();
                                    }
                                    var_ty.emit_store(code, slot);
                                }
                                _ => {}
                            }
                        }
                    }
                    Expression::IndexAccess { target: arr_expr, indices, .. } => {
                        if let Some(idx_expr) = indices.first() {
                            let arr_ty = self.compile_expression(arr_expr, code, cf, locals, next_slot, break_targets)?;
                            let _ = self.compile_expression(idx_expr, code, cf, locals, next_slot, break_targets)?;

                            match op {
                                AssignOp::Assign => {
                                    let val_ty = self.compile_expression(value, code, cf, locals, next_slot, break_targets)?;
                                    if arr_ty == JvmType::Object("[B".to_string()) {
                                        if val_ty == JvmType::Int {
                                            code.bastore();
                                        } else {
                                            code.bastore();
                                        }
                                    } else if arr_ty == JvmType::Object("[J".to_string()) {
                                        code.lastore();
                                    } else {
                                        code.iastore();
                                    }
                                }
                                AssignOp::PlusAssign => {
                                    // Duplicate arrayref and index on stack: [arr, idx] -> dup2 -> [arr, idx, arr, idx]
                                    code.dup2();
                                    if arr_ty == JvmType::Object("[B".to_string()) {
                                        code.baload();
                                        let _ = self.compile_expression(value, code, cf, locals, next_slot, break_targets)?;
                                        code.iadd();
                                        code.bastore();
                                    } else if arr_ty == JvmType::Object("[J".to_string()) {
                                        code.laload();
                                        let _ = self.compile_expression(value, code, cf, locals, next_slot, break_targets)?;
                                        code.ladd();
                                        code.lastore();
                                    } else {
                                        code.iaload();
                                        let _ = self.compile_expression(value, code, cf, locals, next_slot, break_targets)?;
                                        code.iadd();
                                        code.iastore();
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
            Statement::Expression(expr) => {
                let ty = self.compile_expression(expr, code, cf, locals, next_slot, break_targets)?;
                ty.emit_pop(code);
            }
        }
        Ok(())
    }

    fn compile_expression(
        &mut self,
        expr: &Expression,
        code: &mut CodeBuilder,
        cf: &mut ClassFile,
        locals: &mut HashMap<String, (u16, JvmType)>,
        next_slot: &mut u16,
        break_targets: &mut Vec<usize>,
    ) -> Result<JvmType, String> {
        match expr {
            Expression::IntegerLiteral(val, _) => {
                emit_iconst(code, cf, *val as i32);
                Ok(JvmType::Int)
            }
            Expression::LongLiteral(val, _) => {
                if *val == 0 {
                    code.lconst_0();
                } else if *val == 1 {
                    code.lconst_1();
                } else {
                    let idx = cf.cp.add_long(*val);
                    code.ldc2_w(idx);
                }
                Ok(JvmType::Long)
            }
            Expression::FloatLiteral(f, _) => {
                let idx = cf.cp.add_double(*f);
                code.ldc2_w(idx);
                Ok(JvmType::Double)
            }
            Expression::BooleanLiteral(b, _) => {
                code.iconst(if *b { 1 } else { 0 });
                Ok(JvmType::Int)
            }
            Expression::StringLiteral(s, _) => {
                let str_idx = cf.cp.add_string(s);
                code.ldc(str_idx);
                Ok(JvmType::Object("Ljava/lang/String;".to_string()))
            }
            Expression::StringTemplate(parts, _) => {
                let sb_class = cf.cp.add_class("java/lang/StringBuilder");
                code.new_instance(sb_class);
                code.dup();
                let sb_init = cf.cp.add_methodref("java/lang/StringBuilder", "<init>", "()V");
                code.invokespecial(sb_init, 0);

                for part in parts {
                    let ty = self.compile_expression(part, code, cf, locals, next_slot, break_targets)?;
                    let (desc, slots) = match ty {
                        JvmType::Int => ("(I)Ljava/lang/StringBuilder;", 1),
                        JvmType::Long => ("(J)Ljava/lang/StringBuilder;", 2),
                        JvmType::Double => ("(D)Ljava/lang/StringBuilder;", 2),
                        JvmType::Float => ("(F)Ljava/lang/StringBuilder;", 1),
                        JvmType::Object(ref d) if d == "Ljava/lang/String;" => {
                            ("(Ljava/lang/String;)Ljava/lang/StringBuilder;", 1)
                        }
                        _ => ("(Ljava/lang/Object;)Ljava/lang/StringBuilder;", 1),
                    };
                    let app_ref = cf.cp.add_methodref("java/lang/StringBuilder", "append", desc);
                    code.invokevirtual(app_ref, slots, true);
                }

                let to_string = cf.cp.add_methodref("java/lang/StringBuilder", "toString", "()Ljava/lang/String;");
                code.invokevirtual(to_string, 0, true);
                Ok(JvmType::Object("Ljava/lang/String;".to_string()))
            }
            Expression::NullLiteral(_) => {
                code.aconst_null();
                Ok(JvmType::Object("Ljava/lang/Object;".to_string()))
            }
            Expression::This(_) => {
                code.aload(0);
                Ok(JvmType::Object("Ljava/lang/Object;".to_string()))
            }
            Expression::Identifier(id) => {
                if let Some((slot, ty)) = locals.get(&id.name).cloned() {
                    ty.emit_load(code, slot);
                    Ok(ty)
                } else if let Some((cval, cty)) = self.top_level_constants.get(&id.name).cloned() {
                    if cty == JvmType::Long {
                        if cval == 0 {
                            code.lconst_0();
                        } else if cval == 1 {
                            code.lconst_1();
                        } else {
                            let idx = cf.cp.add_long(cval);
                            code.ldc2_w(idx);
                        }
                        Ok(JvmType::Long)
                    } else {
                        emit_iconst(code, cf, cval as i32);
                        Ok(JvmType::Int)
                    }
                } else if id.name == "System" {
                    Ok(JvmType::Object("Ljava/lang/System;".to_string()))
                } else if id.name == "Math" {
                    Ok(JvmType::Object("Ljava/lang/Math;".to_string()))
                } else {
                    code.aconst_null();
                    Ok(JvmType::Object("Ljava/lang/Object;".to_string()))
                }
            }
            Expression::Binary { left, op, right, .. } => {
                if *op == BinaryOp::Elvis {
                    // Left ?: Right
                    let left_ty = self.compile_expression(left, code, cf, locals, next_slot, break_targets)?;
                    code.dup();
                    let not_null_jump = code.emit_ifnonnull();
                    code.pop();
                    let right_ty = self.compile_expression(right, code, cf, locals, next_slot, break_targets)?;
                    let end_jump = code.emit_goto();
                    code.patch_jump(not_null_jump);
                    if left_ty == JvmType::Object("Ljava/lang/Integer;".to_string()) {
                        let int_val = cf.cp.add_methodref("java/lang/Integer", "intValue", "()I");
                        code.invokevirtual(int_val, 0, false);
                        code.patch_jump(end_jump);
                        return Ok(JvmType::Int);
                    } else if left_ty == JvmType::Object("Ljava/lang/Long;".to_string()) {
                        let long_val = cf.cp.add_methodref("java/lang/Long", "longValue", "()J");
                        code.invokevirtual(long_val, 0, false);
                        code.patch_jump(end_jump);
                        return Ok(JvmType::Long);
                    }
                    code.patch_jump(end_jump);
                    return Ok(if left_ty != JvmType::Void { left_ty } else { right_ty });
                }

                let left_ty = self.compile_expression(left, code, cf, locals, next_slot, break_targets)?;
                let is_shift = matches!(op, BinaryOp::Shl | BinaryOp::Shr | BinaryOp::Ushr);
                let right_ty = self.compile_expression(right, code, cf, locals, next_slot, break_targets)?;

                if is_shift {
                    // For shifts, right MUST be Int
                    if right_ty == JvmType::Long {
                        code.l2i();
                    } else if right_ty == JvmType::Double {
                        code.d2i();
                    }
                    if left_ty == JvmType::Long {
                        match op {
                            BinaryOp::Shl => { code.lshl(); Ok(JvmType::Long) }
                            BinaryOp::Shr => { code.lshr(); Ok(JvmType::Long) }
                            BinaryOp::Ushr => { code.lushr(); Ok(JvmType::Long) }
                            _ => unreachable!(),
                        }
                    } else {
                        match op {
                            BinaryOp::Shl => { code.ishl(); Ok(JvmType::Int) }
                            BinaryOp::Shr => { code.ishr(); Ok(JvmType::Int) }
                            BinaryOp::Ushr => { code.iushr(); Ok(JvmType::Int) }
                            _ => unreachable!(),
                        }
                    }
                } else {
                    let target_ty = if left_ty == JvmType::Double || right_ty == JvmType::Double {
                        JvmType::Double
                    } else if left_ty == JvmType::Long || right_ty == JvmType::Long {
                        JvmType::Long
                    } else {
                        JvmType::Int
                    };

                    // Stack has: [left (left_ty), right (right_ty)]
                    if left_ty != target_ty {
                        // right is on top of left, so save right to a temporary slot
                        let temp_slot = *next_slot;
                        *next_slot += match right_ty {
                            JvmType::Long | JvmType::Double => 2,
                            _ => 1,
                        };
                        match right_ty {
                            JvmType::Double => code.dstore(temp_slot),
                            JvmType::Long => code.lstore(temp_slot),
                            JvmType::Int => code.istore(temp_slot),
                            _ => code.astore(temp_slot),
                        }
                        // Now left is on top: convert left to target_ty
                        match (&left_ty, &target_ty) {
                            (JvmType::Long, JvmType::Double) => code.l2d(),
                            (JvmType::Int, JvmType::Double) => code.i2d(),
                            (JvmType::Int, JvmType::Long) => code.i2l(),
                            _ => {}
                        }
                        // Reload right
                        match right_ty {
                            JvmType::Double => code.dload(temp_slot),
                            JvmType::Long => code.lload(temp_slot),
                            JvmType::Int => code.iload(temp_slot),
                            _ => code.aload(temp_slot),
                        }
                        // Convert right if needed
                        match (&right_ty, &target_ty) {
                            (JvmType::Long, JvmType::Double) => code.l2d(),
                            (JvmType::Int, JvmType::Double) => code.i2d(),
                            (JvmType::Int, JvmType::Long) => code.i2l(),
                            _ => {}
                        }
                    } else if right_ty != target_ty {
                        // left is already target_ty, right is on top, convert right directly
                        match (&right_ty, &target_ty) {
                            (JvmType::Long, JvmType::Double) => code.l2d(),
                            (JvmType::Int, JvmType::Double) => code.i2d(),
                            (JvmType::Int, JvmType::Long) => code.i2l(),
                            _ => {}
                        }
                    }

                    match target_ty {
                        JvmType::Double => match op {
                            BinaryOp::Add => { code.dadd(); Ok(JvmType::Double) }
                            BinaryOp::Sub => { code.dsub(); Ok(JvmType::Double) }
                            BinaryOp::Mul => { code.dmul(); Ok(JvmType::Double) }
                            BinaryOp::Div => { code.ddiv(); Ok(JvmType::Double) }
                            BinaryOp::Mod => { code.drem(); Ok(JvmType::Double) }
                            BinaryOp::Eq | BinaryOp::NotEq | BinaryOp::Less | BinaryOp::LessEq | BinaryOp::Greater | BinaryOp::GreaterEq => {
                                code.dcmpg();
                                let jump = match op {
                                    BinaryOp::Eq => code.emit_ifeq(),
                                    BinaryOp::NotEq => code.emit_ifne(),
                                    BinaryOp::Less => code.emit_iflt(),
                                    BinaryOp::LessEq => code.emit_ifle(),
                                    BinaryOp::Greater => code.emit_ifgt(),
                                    BinaryOp::GreaterEq => code.emit_ifge(),
                                    _ => unreachable!(),
                                };
                                code.iconst(0);
                                let end_j = code.emit_goto();
                                code.patch_jump(jump);
                                code.iconst(1);
                                code.patch_jump(end_j);
                                Ok(JvmType::Int)
                            }
                            _ => Ok(JvmType::Double),
                        },
                        JvmType::Long => match op {
                            BinaryOp::Add => { code.ladd(); Ok(JvmType::Long) }
                            BinaryOp::Sub => { code.lsub(); Ok(JvmType::Long) }
                            BinaryOp::Mul => { code.lmul(); Ok(JvmType::Long) }
                            BinaryOp::Div => { code.ldiv(); Ok(JvmType::Long) }
                            BinaryOp::Mod => { code.lrem(); Ok(JvmType::Long) }
                            BinaryOp::BitAnd => { code.land(); Ok(JvmType::Long) }
                            BinaryOp::BitOr => { code.lor(); Ok(JvmType::Long) }
                            BinaryOp::BitXor => { code.lxor(); Ok(JvmType::Long) }
                            BinaryOp::Eq | BinaryOp::NotEq | BinaryOp::Less | BinaryOp::LessEq | BinaryOp::Greater | BinaryOp::GreaterEq => {
                                code.lcmp();
                                let jump = match op {
                                    BinaryOp::Eq => code.emit_ifeq(),
                                    BinaryOp::NotEq => code.emit_ifne(),
                                    BinaryOp::Less => code.emit_iflt(),
                                    BinaryOp::LessEq => code.emit_ifle(),
                                    BinaryOp::Greater => code.emit_ifgt(),
                                    BinaryOp::GreaterEq => code.emit_ifge(),
                                    _ => unreachable!(),
                                };
                                code.iconst(0);
                                let end_j = code.emit_goto();
                                code.patch_jump(jump);
                                code.iconst(1);
                                code.patch_jump(end_j);
                                Ok(JvmType::Int)
                            }
                            _ => Ok(JvmType::Long),
                        },
                        _ => match op {
                            BinaryOp::Add => { code.iadd(); Ok(JvmType::Int) }
                            BinaryOp::Sub => { code.isub(); Ok(JvmType::Int) }
                            BinaryOp::Mul => { code.imul(); Ok(JvmType::Int) }
                            BinaryOp::Div => { code.idiv(); Ok(JvmType::Int) }
                            BinaryOp::Mod => { code.irem(); Ok(JvmType::Int) }
                            BinaryOp::BitAnd => { code.iand(); Ok(JvmType::Int) }
                            BinaryOp::BitOr => { code.ior(); Ok(JvmType::Int) }
                            BinaryOp::BitXor => { code.ixor(); Ok(JvmType::Int) }
                            BinaryOp::Eq | BinaryOp::NotEq | BinaryOp::Less | BinaryOp::LessEq | BinaryOp::Greater | BinaryOp::GreaterEq => {
                                let jump = match op {
                                    BinaryOp::Eq => code.emit_if_icmpeq(),
                                    BinaryOp::NotEq => code.emit_if_icmpne(),
                                    BinaryOp::Less => code.emit_if_icmplt(),
                                    BinaryOp::LessEq => code.emit_if_icmple(),
                                    BinaryOp::Greater => code.emit_if_icmpgt(),
                                    BinaryOp::GreaterEq => code.emit_if_icmpge(),
                                    _ => unreachable!(),
                                };
                                code.iconst(0);
                                let end_j = code.emit_goto();
                                code.patch_jump(jump);
                                code.iconst(1);
                                code.patch_jump(end_j);
                                Ok(JvmType::Int)
                            }
                            _ => Ok(JvmType::Int),
                        },
                    }
                }
            }
            Expression::Unary { op, expr, is_prefix, .. } => {
                if matches!(op, UnaryOp::Inc | UnaryOp::Dec) {
                    let delta: i8 = if *op == UnaryOp::Inc { 1 } else { -1 };
                    if let Expression::Identifier(id) = &**expr {
                        if let Some((slot, ty)) = locals.get(&id.name).cloned() {
                            if ty == JvmType::Int {
                                if *is_prefix {
                                    code.iinc(slot, delta);
                                    code.iload(slot);
                                } else {
                                    code.iload(slot);
                                    code.iinc(slot, delta);
                                }
                                return Ok(JvmType::Int);
                            } else if ty == JvmType::Long {
                                if *is_prefix {
                                    code.lload(slot);
                                    code.lconst_1();
                                    if delta > 0 { code.ladd(); } else { code.lsub(); }
                                    code.dup2();
                                    code.lstore(slot);
                                } else {
                                    code.lload(slot);
                                    code.lload(slot);
                                    code.lconst_1();
                                    if delta > 0 { code.ladd(); } else { code.lsub(); }
                                    code.lstore(slot);
                                }
                                return Ok(JvmType::Long);
                            }
                        }
                    }
                }
                let ty = self.compile_expression(expr, code, cf, locals, next_slot, break_targets)?;
                match op {
                    UnaryOp::Neg => {
                        if ty == JvmType::Long {
                            code.lneg();
                            Ok(JvmType::Long)
                        } else if ty == JvmType::Double {
                            code.dneg();
                            Ok(JvmType::Double)
                        } else {
                            code.ineg();
                            Ok(JvmType::Int)
                        }
                    }
                    UnaryOp::Not => {
                        // boolean not (!x): if 0 then 1 else 0
                        let jump = code.emit_ifeq();
                        code.iconst(0);
                        let end_j = code.emit_goto();
                        code.patch_jump(jump);
                        code.iconst(1);
                        code.patch_jump(end_j);
                        Ok(JvmType::Int)
                    }
                    _ => Ok(ty),
                }
            }
            Expression::Call { callee, args, trailing_lambda, .. } => {
                // 1. Is callee a class constructor (e.g. Pair32, SieveResult, FractalResult)?
                if let Expression::Identifier(id) = &**callee {
                    if let Some(cinfo) = self.class_infos.get(&id.name).cloned() {
                        let c_idx = cf.cp.add_class(&cinfo.internal_name);
                        code.new_instance(c_idx);
                        code.dup();

                        let mut total_slots = 0u16;
                        for (idx, arg) in args.iter().enumerate() {
                            let expected_ty = &cinfo.constructor_params[idx];
                            let arg_ty = self.compile_expression(&arg.value, code, cf, locals, next_slot, break_targets)?;
                            if *expected_ty == JvmType::Long && arg_ty == JvmType::Int {
                                code.i2l();
                            }
                            total_slots += expected_ty.slots();
                        }

                        let p_desc = cinfo.constructor_params.iter().map(|t| t.descriptor()).collect::<Vec<_>>().join("");
                        let ctor_desc = format!("({})V", p_desc);
                        let init_ref = cf.cp.add_methodref(&cinfo.internal_name, "<init>", &ctor_desc);
                        code.invokespecial(init_ref, total_slots);
                        return Ok(JvmType::Object(format!("L{};", cinfo.internal_name)));
                    } else if id.name == "IntArray" {
                        if let Some(size_arg) = args.first() {
                            self.compile_expression(&size_arg.value, code, cf, locals, next_slot, break_targets)?;
                            code.newarray(10); // T_INT
                            return Ok(JvmType::Object("[I".to_string()));
                        }
                    } else if id.name == "ByteArray" {
                        if let Some(size_arg) = args.first() {
                            self.compile_expression(&size_arg.value, code, cf, locals, next_slot, break_targets)?;
                            code.newarray(8); // T_BYTE
                            if trailing_lambda.is_some() {
                                // Initialize with fill((byte)1)
                                code.dup();
                                code.iconst(1);
                                code.i2b();
                                let fill_ref = cf.cp.add_methodref("java/util/Arrays", "fill", "([BB)V");
                                code.invokestatic(fill_ref, 2, false);
                            }
                            return Ok(JvmType::Object("[B".to_string()));
                        }
                    } else if id.name == "require" {
                        if let Some(cond_arg) = args.first() {
                            self.compile_expression(&cond_arg.value, code, cf, locals, next_slot, break_targets)?;
                            let ok_jump = code.emit_ifne();
                            let ex_class = cf.cp.add_class("java/lang/IllegalArgumentException");
                            code.new_instance(ex_class);
                            code.dup();
                            let ex_init = cf.cp.add_methodref("java/lang/IllegalArgumentException", "<init>", "()V");
                            code.invokespecial(ex_init, 0);
                            code.athrow();
                            code.patch_jump(ok_jump);
                            return Ok(JvmType::Void);
                        }
                    } else if id.name == "error" {
                        let ex_class = cf.cp.add_class("java/lang/IllegalStateException");
                        code.new_instance(ex_class);
                        code.dup();
                        if let Some(msg_arg) = args.first() {
                            self.compile_expression(&msg_arg.value, code, cf, locals, next_slot, break_targets)?;
                            let ex_init = cf.cp.add_methodref("java/lang/IllegalStateException", "<init>", "(Ljava/lang/String;)V");
                            code.invokespecial(ex_init, 1);
                        } else {
                            let ex_init = cf.cp.add_methodref("java/lang/IllegalStateException", "<init>", "()V");
                            code.invokespecial(ex_init, 0);
                        }
                        code.athrow();
                        return Ok(JvmType::Void);
                    } else if id.name == "println" || id.name == "print" {
                        let sys_out = cf.cp.add_fieldref("java/lang/System", "out", "Ljava/io/PrintStream;");
                        code.getstatic(sys_out);
                        if let Some(arg) = args.first() {
                            let arg_ty = self.compile_expression(&arg.value, code, cf, locals, next_slot, break_targets)?;
                            let (m_desc, slots) = match arg_ty {
                                JvmType::Int => ("(I)V", 1),
                                JvmType::Long => ("(J)V", 2),
                                JvmType::Double => ("(D)V", 2),
                                JvmType::Float => ("(F)V", 1),
                                JvmType::Object(ref d) if d == "Ljava/lang/String;" => ("(Ljava/lang/String;)V", 1),
                                _ => ("(Ljava/lang/Object;)V", 1),
                            };
                            let print_m = cf.cp.add_methodref("java/io/PrintStream", &id.name, m_desc);
                            code.invokevirtual(print_m, slots, false);
                        } else {
                            let print_m = cf.cp.add_methodref("java/io/PrintStream", &id.name, "()V");
                            code.invokevirtual(print_m, 0, false);
                        }
                        return Ok(JvmType::Void);
                    } else if id.name == "measure" && trailing_lambda.is_some() {
                        // Inlined measure(name) { block }:
                        // Start time:
                        let nano_ref = cf.cp.add_methodref("java/lang/System", "nanoTime", "()J");
                        code.invokestatic(nano_ref, 0, true);
                        let start_slot = *next_slot;
                        *next_slot += 2;
                        code.lstore(start_slot);

                        // Execute block (trailing lambda):
                        let lambda_expr = trailing_lambda.as_ref().unwrap();
                        let checksum_ty = self.compile_expression(lambda_expr, code, cf, locals, next_slot, break_targets)?;
                        let checksum_slot = *next_slot;
                        *next_slot += 1;
                        if checksum_ty == JvmType::Int {
                            let to_s = cf.cp.add_methodref("java/lang/Integer", "toString", "(I)Ljava/lang/String;");
                            code.invokestatic(to_s, 1, true);
                        } else if checksum_ty == JvmType::Long {
                            let to_s = cf.cp.add_methodref("java/lang/Long", "toString", "(J)Ljava/lang/String;");
                            code.invokestatic(to_s, 2, true);
                        }
                        code.astore(checksum_slot);

                        // Elapsed ms: (System.nanoTime() - start) / 1_000_000.0
                        code.invokestatic(nano_ref, 0, true);
                        code.lload(start_slot);
                        code.lsub();
                        code.l2d();
                        let cp_1m = cf.cp.add_double(1_000_000.0);
                        code.ldc2_w(cp_1m);
                        code.ddiv();
                        let elapsed_slot = *next_slot;
                        *next_slot += 2;
                        code.dstore(elapsed_slot);

                        // Print RESULT line
                        let sys_out = cf.cp.add_fieldref("java/lang/System", "out", "Ljava/io/PrintStream;");
                        code.getstatic(sys_out);

                        let sb_class = cf.cp.add_class("java/lang/StringBuilder");
                        code.new_instance(sb_class);
                        code.dup();
                        let sb_init = cf.cp.add_methodref("java/lang/StringBuilder", "<init>", "()V");
                        code.invokespecial(sb_init, 0);

                        let app_str = cf.cp.add_methodref("java/lang/StringBuilder", "append", "(Ljava/lang/String;)Ljava/lang/StringBuilder;");
                        let app_dbl = cf.cp.add_methodref("java/lang/StringBuilder", "append", "(D)Ljava/lang/StringBuilder;");

                        let res_prefix = cf.cp.add_string("RESULT|");
                        code.ldc(res_prefix);
                        code.invokevirtual(app_str, 1, true);

                        // name arg
                        if let Some(arg) = args.first() {
                            self.compile_expression(&arg.value, code, cf, locals, next_slot, break_targets)?;
                            code.invokevirtual(app_str, 1, true);
                        }

                        let pipe = cf.cp.add_string("|");
                        code.ldc(pipe);
                        code.invokevirtual(app_str, 1, true);

                        // checksum
                        code.aload(checksum_slot);
                        code.invokevirtual(app_str, 1, true);

                        code.ldc(pipe);
                        code.invokevirtual(app_str, 1, true);

                        // elapsedMs
                        code.dload(elapsed_slot);
                        code.invokevirtual(app_dbl, 2, true);

                        let to_s = cf.cp.add_methodref("java/lang/StringBuilder", "toString", "()Ljava/lang/String;");
                        code.invokevirtual(to_s, 0, true);

                        let print_m = cf.cp.add_methodref("java/io/PrintStream", "println", "(Ljava/lang/String;)V");
                        code.invokevirtual(print_m, 1, false);

                        code.dload(elapsed_slot);
                        return Ok(JvmType::Double);
                    } else if let Some(sig) = self.functions.get(&id.name).cloned() {
                        let mut total_slots = 0u16;
                        for (idx, pty) in sig.param_types.iter().enumerate() {
                            let aty = self.compile_expression(&args[idx].value, code, cf, locals, next_slot, break_targets)?;
                            if *pty == JvmType::Long && aty == JvmType::Int {
                                code.i2l();
                            }
                            total_slots += pty.slots();
                        }
                        let p_desc = sig.param_types.iter().map(|t| t.descriptor()).collect::<Vec<_>>().join("");
                        let m_desc = format!("({}){}", p_desc, sig.return_type.descriptor());
                        let m_ref = cf.cp.add_methodref(&self.current_facade_name, &id.name, &m_desc);
                        code.invokestatic(m_ref, total_slots, sig.return_type != JvmType::Void);
                        return Ok(sig.return_type);
                    }
                }

                // 2. Member call (e.g. System.getenv, Math.floorDiv, target.let, etc.)
                if let Expression::MemberAccess { target, member, .. } = &**callee {
                    if let Expression::Identifier(target_id) = &**target {
                        if target_id.name == "System" && member.name == "getenv" {
                            if let Some(arg) = args.first() {
                                self.compile_expression(&arg.value, code, cf, locals, next_slot, break_targets)?;
                                let m_ref = cf.cp.add_methodref("java/lang/System", "getenv", "(Ljava/lang/String;)Ljava/lang/String;");
                                code.invokestatic(m_ref, 1, true);
                                return Ok(JvmType::Object("Ljava/lang/String;".to_string()));
                            }
                        } else if target_id.name == "System" && member.name == "nanoTime" {
                            let m_ref = cf.cp.add_methodref("java/lang/System", "nanoTime", "()J");
                            code.invokestatic(m_ref, 0, true);
                            return Ok(JvmType::Long);
                        } else if target_id.name == "Math" && member.name == "floorDiv" {
                            if args.len() >= 2 {
                                self.compile_expression(&args[0].value, code, cf, locals, next_slot, break_targets)?;
                                self.compile_expression(&args[1].value, code, cf, locals, next_slot, break_targets)?;
                                let m_ref = cf.cp.add_methodref("java/lang/Math", "floorDiv", "(JJ)J");
                                code.invokestatic(m_ref, 4, true);
                                return Ok(JvmType::Long);
                            }
                        }
                    }

                    if member.name == "let" {
                        let target_ty = self.compile_expression(target, code, cf, locals, next_slot, break_targets)?;
                        let it_slot = *next_slot;
                        *next_slot += target_ty.slots();
                        target_ty.emit_store(code, it_slot);
                        locals.insert("it".to_string(), (it_slot, target_ty));

                        if let Some(lambda) = trailing_lambda {
                            return self.compile_expression(lambda, code, cf, locals, next_slot, break_targets);
                        } else if let Some(arg) = args.first() {
                            return self.compile_expression(&arg.value, code, cf, locals, next_slot, break_targets);
                        }
                    } else if member.name == "toUInt" {
                        self.compile_expression(target, code, cf, locals, next_slot, break_targets)?;
                        let to_u = cf.cp.add_methodref("java/lang/Integer", "toUnsignedString", "(I)Ljava/lang/String;");
                        code.invokestatic(to_u, 1, true);
                        return Ok(JvmType::Object("Ljava/lang/String;".to_string()));
                    } else if member.name == "toInt" {
                        let ty = self.compile_expression(target, code, cf, locals, next_slot, break_targets)?;
                        if ty == JvmType::Long {
                            code.l2i();
                        }
                        return Ok(JvmType::Int);
                    } else if member.name == "toLong" {
                        let ty = self.compile_expression(target, code, cf, locals, next_slot, break_targets)?;
                        if ty == JvmType::Int {
                            code.i2l();
                        }
                        return Ok(JvmType::Long);
                    } else if member.name == "toIntOrNull" {
                        self.compile_expression(target, code, cf, locals, next_slot, break_targets)?;
                        let p_int = cf.cp.add_methodref("java/lang/Integer", "valueOf", "(Ljava/lang/String;)Ljava/lang/Integer;");
                        code.invokestatic(p_int, 1, true);
                        return Ok(JvmType::Object("Ljava/lang/Integer;".to_string()));
                    } else if member.name == "toLongOrNull" {
                        self.compile_expression(target, code, cf, locals, next_slot, break_targets)?;
                        let p_long = cf.cp.add_methodref("java/lang/Long", "valueOf", "(Ljava/lang/String;)Ljava/lang/Long;");
                        code.invokestatic(p_long, 1, true);
                        return Ok(JvmType::Object("Ljava/lang/Long;".to_string()));
                    } else if member.name == "toString" {
                        let ty = self.compile_expression(target, code, cf, locals, next_slot, break_targets)?;
                        if ty == JvmType::Long {
                            let to_s = cf.cp.add_methodref("java/lang/Long", "toString", "(J)Ljava/lang/String;");
                            code.invokestatic(to_s, 2, true);
                        } else if ty == JvmType::Int {
                            let to_s = cf.cp.add_methodref("java/lang/Integer", "toString", "(I)Ljava/lang/String;");
                            code.invokestatic(to_s, 1, true);
                        } else {
                            let to_s = cf.cp.add_methodref("java/lang/Object", "toString", "()Ljava/lang/String;");
                            code.invokevirtual(to_s, 0, true);
                        }
                        return Ok(JvmType::Object("Ljava/lang/String;".to_string()));
                    }
                }

                code.aconst_null();
                Ok(JvmType::Object("Ljava/lang/Object;".to_string()))
            }
            Expression::MemberAccess { target, member, .. } => {
                let target_ty = self.compile_expression(target, code, cf, locals, next_slot, break_targets)?;

                // Check if target is a data class instance
                if let JvmType::Object(ref desc) = target_ty {
                    let internal = desc.trim_start_matches('L').trim_end_matches(';');
                    let simple_name = internal.rsplit('/').next().unwrap_or(internal);
                    if let Some(cinfo) = self.class_infos.get(simple_name).cloned() {
                        if let Some((_, fty)) = cinfo.fields.iter().find(|(n, _)| n == &member.name) {
                            let f_idx = cf.cp.add_fieldref(&cinfo.internal_name, &member.name, &fty.descriptor());
                            code.getfield(f_idx);
                            return Ok(fty.clone());
                        }
                    }
                }

                if member.name == "indices" {
                    // Array indices range: returns arrayref on stack for loop handling
                    return Ok(target_ty);
                }

                Ok(JvmType::Object("Ljava/lang/Object;".to_string()))
            }
            Expression::IndexAccess { target, indices, .. } => {
                let arr_ty = self.compile_expression(target, code, cf, locals, next_slot, break_targets)?;
                if let Some(idx_expr) = indices.first() {
                    self.compile_expression(idx_expr, code, cf, locals, next_slot, break_targets)?;
                }
                if arr_ty == JvmType::Object("[B".to_string()) {
                    code.baload();
                    Ok(JvmType::Int)
                } else if arr_ty == JvmType::Object("[J".to_string()) {
                    code.laload();
                    Ok(JvmType::Long)
                } else {
                    code.iaload();
                    Ok(JvmType::Int)
                }
            }
            Expression::For { variable, iterable, body, .. } => {
                if let Expression::Binary { left, op, right, .. } = &**iterable {
                    if *op == BinaryOp::Until || *op == BinaryOp::RangeExclusive || *op == BinaryOp::Range {
                        let is_until = *op == BinaryOp::Until || *op == BinaryOp::RangeExclusive;
                        let var_slot = *next_slot;
                        *next_slot += 1;
                        locals.insert(variable.name.clone(), (var_slot, JvmType::Int));

                        self.compile_expression(left, code, cf, locals, next_slot, break_targets)?;
                        code.istore(var_slot);

                        let max_slot = *next_slot;
                        *next_slot += 1;
                        self.compile_expression(right, code, cf, locals, next_slot, break_targets)?;
                        code.istore(max_slot);

                        let loop_start = code.bytes.len();
                        code.iload(var_slot);
                        code.iload(max_slot);
                        let exit_jump = if is_until {
                            code.emit_if_icmpge()
                        } else {
                            code.emit_if_icmpgt()
                        };

                        let mut loop_breaks = Vec::new();
                        let body_ty = self.compile_expression(body, code, cf, locals, next_slot, &mut loop_breaks)?;
                        body_ty.emit_pop(code);

                        code.iinc(var_slot, 1);
                        code.emit_goto_target(loop_start);
                        code.patch_jump(exit_jump);
                        for b in loop_breaks {
                            code.patch_jump(b);
                        }
                        return Ok(JvmType::Void);
                    }
                } else if let Expression::MemberAccess { target, member, .. } = &**iterable {
                    if member.name == "indices" {
                        // for (i in values.indices) -> 0 until values.length
                        let var_slot = *next_slot;
                        *next_slot += 1;
                        locals.insert(variable.name.clone(), (var_slot, JvmType::Int));
                        code.iconst(0);
                        code.istore(var_slot);

                        let max_slot = *next_slot;
                        *next_slot += 1;
                        self.compile_expression(target, code, cf, locals, next_slot, break_targets)?;
                        code.arraylength();
                        code.istore(max_slot);

                        let loop_start = code.bytes.len();
                        code.iload(var_slot);
                        code.iload(max_slot);
                        let exit_jump = code.emit_if_icmpge();

                        let mut loop_breaks = Vec::new();
                        let body_ty = self.compile_expression(body, code, cf, locals, next_slot, &mut loop_breaks)?;
                        body_ty.emit_pop(code);

                        code.iinc(var_slot, 1);
                        code.emit_goto_target(loop_start);
                        code.patch_jump(exit_jump);
                        for b in loop_breaks {
                            code.patch_jump(b);
                        }
                        return Ok(JvmType::Void);
                    }
                } else {
                    // Array iteration: for (value in c)
                    let arr_ty = self.compile_expression(iterable, code, cf, locals, next_slot, break_targets)?;
                    let arr_slot = *next_slot;
                    *next_slot += 1;
                    code.dup();
                    code.astore(arr_slot);
                    code.arraylength();
                    let len_slot = *next_slot;
                    *next_slot += 1;
                    code.istore(len_slot);

                    let idx_slot = *next_slot;
                    *next_slot += 1;
                    code.iconst(0);
                    code.istore(idx_slot);

                    let val_slot = *next_slot;
                    let elem_ty = if arr_ty == JvmType::Object("[J".to_string()) { JvmType::Long } else { JvmType::Int };
                    *next_slot += elem_ty.slots();
                    locals.insert(variable.name.clone(), (val_slot, elem_ty.clone()));

                    let loop_start = code.bytes.len();
                    code.iload(idx_slot);
                    code.iload(len_slot);
                    let exit_jump = code.emit_if_icmpge();

                    code.aload(arr_slot);
                    code.iload(idx_slot);
                    if elem_ty == JvmType::Long {
                        code.laload();
                        code.lstore(val_slot);
                    } else {
                        code.iaload();
                        code.istore(val_slot);
                    }

                    let mut loop_breaks = Vec::new();
                    let body_ty = self.compile_expression(body, code, cf, locals, next_slot, &mut loop_breaks)?;
                    body_ty.emit_pop(code);

                    code.iinc(idx_slot, 1);
                    code.emit_goto_target(loop_start);
                    code.patch_jump(exit_jump);
                    for b in loop_breaks {
                        code.patch_jump(b);
                    }
                    return Ok(JvmType::Void);
                }
                Ok(JvmType::Void)
            }
            Expression::While { condition, body, .. } => {
                let loop_start = code.bytes.len();
                let cond_ty = self.compile_expression(condition, code, cf, locals, next_slot, break_targets)?;
                let exit_jump = if cond_ty == JvmType::Int {
                    code.emit_ifeq()
                } else {
                    code.emit_ifeq()
                };

                let mut while_breaks = Vec::new();
                let body_ty = self.compile_expression(body, code, cf, locals, next_slot, &mut while_breaks)?;
                body_ty.emit_pop(code);

                code.emit_goto_target(loop_start);
                code.patch_jump(exit_jump);
                for b in while_breaks {
                    code.patch_jump(b);
                }
                Ok(JvmType::Void)
            }
            Expression::Break(_) => {
                let j = code.emit_goto();
                break_targets.push(j);
                Ok(JvmType::Void)
            }
            Expression::If { condition, then_branch, else_branch, .. } => {
                self.compile_expression(condition, code, cf, locals, next_slot, break_targets)?;
                let else_jump = code.emit_ifeq();
                let then_ty = self.compile_expression(then_branch, code, cf, locals, next_slot, break_targets)?;
                if let Some(else_b) = else_branch {
                    let end_jump = code.emit_goto();
                    code.patch_jump(else_jump);
                    let _ = self.compile_expression(else_b, code, cf, locals, next_slot, break_targets)?;
                    code.patch_jump(end_jump);
                    Ok(then_ty)
                } else {
                    then_ty.emit_pop(code);
                    code.patch_jump(else_jump);
                    Ok(JvmType::Void)
                }
            }
            Expression::Block(statements, _) => {
                let mut last_ty = JvmType::Void;
                for (idx, stmt) in statements.iter().enumerate() {
                    let is_last = idx + 1 == statements.len();
                    match stmt {
                        Statement::Declaration(decl) => match decl {
                            Declaration::Property(p) => {
                                let slot = *next_slot;
                                let declared_ty = p.type_ref.as_ref().map(|tr| type_ref_to_jvm(tr, &self.current_package_prefix));
                                if let Some(init) = &p.initializer {
                                    let init_ty = self.compile_expression(init, code, cf, locals, next_slot, break_targets)?;
                                    let actual_ty = match declared_ty {
                                        Some(JvmType::Long) if init_ty == JvmType::Int => {
                                            code.i2l();
                                            JvmType::Long
                                        }
                                        Some(d) => d,
                                        None => init_ty,
                                    };
                                    *next_slot = slot + actual_ty.slots();
                                    actual_ty.emit_store(code, slot);
                                    locals.insert(p.name.name.clone(), (slot, actual_ty));
                                } else {
                                    let ty = declared_ty.unwrap_or(JvmType::Int);
                                    *next_slot = slot + ty.slots();
                                    locals.insert(p.name.name.clone(), (slot, ty));
                                }
                                last_ty = JvmType::Void;
                            }
                            _ => {}
                        },
                        Statement::Assignment { .. } => {
                            self.compile_statement(stmt, code, cf, locals, next_slot, break_targets)?;
                            last_ty = JvmType::Void;
                        }
                        Statement::Expression(e) => {
                            if is_last {
                                last_ty = self.compile_expression(e, code, cf, locals, next_slot, break_targets)?;
                            } else {
                                let ty = self.compile_expression(e, code, cf, locals, next_slot, break_targets)?;
                                ty.emit_pop(code);
                                last_ty = JvmType::Void;
                            }
                        }
                    }
                }
                Ok(last_ty)
            }
            Expression::Lambda { body, .. } => {
                let mut last_ty = JvmType::Void;
                for (idx, stmt) in body.iter().enumerate() {
                    let is_last = idx + 1 == body.len();
                    match stmt {
                        Statement::Expression(e) if is_last => {
                            last_ty = self.compile_expression(e, code, cf, locals, next_slot, break_targets)?;
                        }
                        _ => {
                            self.compile_statement(stmt, code, cf, locals, next_slot, break_targets)?;
                        }
                    }
                }
                Ok(last_ty)
            }
            Expression::Return { value, .. } => {
                if let Some(val) = value {
                    let ty = self.compile_expression(val, code, cf, locals, next_slot, break_targets)?;
                    if self.current_ret_type == JvmType::Long && ty == JvmType::Int {
                        code.i2l();
                    }
                    self.current_ret_type.emit_return(code);
                } else {
                    code.return_void();
                }
                Ok(JvmType::Void)
            }
            _ => {
                code.aconst_null();
                Ok(JvmType::Object("Ljava/lang/Object;".to_string()))
            }
        }
    }
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
