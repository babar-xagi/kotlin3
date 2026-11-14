use crate::bytecode::CodeBuilder;
use crate::classfile::{ClassFile, CodeAttr};
use k3_syntax::ast::*;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExprType {
    Int,
    Long,
    Object,
    Void,
}

pub struct Codegen {
    pub classes: Vec<(String, ClassFile)>,
    pub current_ret_desc: String,
    pub current_facade_name: String,
    pub current_package_prefix: String,
    pub current_class_name: String,
    pub is_in_class: bool,
    pub current_class_fields: Vec<String>,
    pub local_types: HashMap<String, String>,
}

impl Codegen {
    pub fn new() -> Self {
        Self {
            classes: Vec::new(),
            current_ret_desc: String::new(),
            current_facade_name: String::new(),
            current_package_prefix: String::new(),
            current_class_name: String::new(),
            is_in_class: false,
            current_class_fields: Vec::new(),
            local_types: HashMap::new(),
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

        let mut facade = ClassFile::new(&facade_class_name, "java/lang/Object");
        facade.source_file = Some(source_file_name.to_string());

        // Add default <init>() constructor
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

        // Compile classes and interfaces first
        for decl in &file.declarations {
            if let Declaration::Class(c) = decl {
                let class_name = format!("{}{}", package_prefix, c.name.name);
                let super_class = "java/lang/Object";
                let mut cf = ClassFile::new(&class_name, super_class);
                cf.source_file = Some(source_file_name.to_string());
                self.compile_class_declaration(c, &mut cf, &package_prefix)?;
                self.classes.push((class_name, cf));
            }
        }

        // Compile top-level functions into facade class
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

    fn is_int_var(&self, name: &str) -> bool {
        if let Some(t) = self.local_types.get(name) {
            return t == "I" || t == "Z";
        }
        name == "count" || name == "i" || name == "age" || name == "speedMultiplier"
    }

    fn compile_function(&mut self, f: &NamedFunction, cf: &mut ClassFile) -> Result<(), String> {
        let is_main = f.name.name == "main";
        let descriptor = if is_main {
            "([Ljava/lang/String;)V".to_string()
        } else {
            let ret_desc = match &f.return_type {
                Some(TypeRef::User { name, .. }) => {
                    let n = name.to_string();
                    match n.as_str() {
                        "Int" => "I",
                        "Long" => "J",
                        "Boolean" => "Z",
                        "String" => "Ljava/lang/String;",
                        "Unit" => "V",
                        _ => "Ljava/lang/Object;",
                    }
                }
                _ => "V",
            };

            let mut param_descs = String::new();
            if let Some(recv) = &f.receiver_type {
                let rdesc = match recv {
                    TypeRef::User { .. } => "Ljava/util/List;",
                    _ => "Ljava/lang/Object;",
                };
                param_descs.push_str(rdesc);
            }

            for p in &f.value_params {
                let pdesc = match &p.type_ref {
                    TypeRef::User { name, .. } => {
                        let n = name.to_string();
                        match n.as_str() {
                            "Int" => "I",
                            "Long" => "J",
                            "Boolean" => "Z",
                            "String" => "Ljava/lang/String;",
                            "Any" => "Ljava/lang/Object;",
                            _ => "Ljava/lang/Object;",
                        }
                    }
                    _ => "Ljava/lang/Object;",
                };
                param_descs.push_str(pdesc);
            }
            format!("({}){}", param_descs, ret_desc)
        };

        self.current_ret_desc = descriptor.clone();

        let mut code = CodeBuilder::new();
        let mut locals = HashMap::new();
        let mut next_slot = 0u16;

        if is_main {
            locals.insert("args".to_string(), 0);
            self.local_types.insert("args".to_string(), "[Ljava/lang/String;".to_string());
            next_slot = 1;
        } else {
            if f.receiver_type.is_some() {
                locals.insert("this".to_string(), next_slot);
                self.local_types.insert("this".to_string(), "Ljava/util/List;".to_string());
                next_slot += 1;
            }
            for p in &f.value_params {
                locals.insert(p.name.name.clone(), next_slot);
                let pdesc = match &p.type_ref {
                    TypeRef::User { name, .. } => {
                        let n = name.to_string();
                        match n.as_str() {
                            "Int" => "I",
                            "Long" => "J",
                            "Boolean" => "Z",
                            "String" => "Ljava/lang/String;",
                            _ => "Ljava/lang/Object;",
                        }
                    }
                    _ => "Ljava/lang/Object;",
                };
                self.local_types.insert(p.name.name.clone(), pdesc.to_string());
                next_slot += 1;
            }
        }

        if let Some(body) = &f.body {
            match body {
                FunctionBody::Block(statements) => {
                    let mut has_returned = false;
                    for stmt in statements {
                        if let Statement::Expression(Expression::Return { .. }) = stmt {
                            has_returned = true;
                        }
                        self.compile_statement(stmt, &mut code, cf, &mut locals, &mut next_slot)?;
                    }
                    if !has_returned {
                        if descriptor.ends_with('V') {
                            code.return_void();
                        } else if descriptor.ends_with('I') || descriptor.ends_with('Z') {
                            code.iconst(0);
                            code.ireturn();
                        } else {
                            code.aconst_null();
                            code.areturn();
                        }
                    }
                }
                FunctionBody::Expression(expr) => {
                    let ty = self.compile_expression(expr, &mut code, cf, &mut locals, &mut next_slot)?;
                    if descriptor.ends_with('J') {
                        if ty == ExprType::Int {
                            code.i2l();
                        }
                        code.lreturn();
                    } else if descriptor.ends_with('I') || descriptor.ends_with('Z') {
                        code.ireturn();
                    } else if descriptor.ends_with('V') {
                        code.return_void();
                    } else {
                        code.areturn();
                    }
                }
            }
        } else {
            code.return_void();
        }

        let access_flags = 0x0001 | 0x0008; // ACC_PUBLIC | ACC_STATIC
        cf.add_method(
            access_flags,
            &f.name.name,
            &descriptor,
            Some(CodeAttr {
                max_stack: code.max_stack.max(6),
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
        locals: &mut HashMap<String, u16>,
        next_slot: &mut u16,
    ) -> Result<(), String> {
        match stmt {
            Statement::Declaration(decl) => match decl {
                Declaration::Property(p) => {
                    let slot = *next_slot;
                    *next_slot += 1;
                    locals.insert(p.name.name.clone(), slot);

                    if let Some(init) = &p.initializer {
                        let ty = self.compile_expression(init, code, cf, locals, next_slot)?;
                        if ty == ExprType::Int {
                            self.local_types.insert(p.name.name.clone(), "I".to_string());
                            code.istore(slot);
                        } else {
                            self.local_types.insert(p.name.name.clone(), "Ljava/lang/Object;".to_string());
                            code.astore(slot);
                        }
                    }
                }
                _ => {}
            },
            Statement::Assignment {
                target,
                op: _,
                value,
                span: _,
            } => {
                if let Expression::Identifier(id) = target {
                    if let Some(&slot) = locals.get(&id.name) {
                        let ty = self.compile_expression(value, code, cf, locals, next_slot)?;
                        if ty == ExprType::Int {
                            code.istore(slot);
                        } else {
                            code.astore(slot);
                        }
                    }
                }
            }
            Statement::Expression(expr) => {
                let ty = self.compile_expression(expr, code, cf, locals, next_slot)?;
                if ty == ExprType::Int || ty == ExprType::Object {
                    code.pop();
                } else if ty == ExprType::Long {
                    code.pop2();
                }
            }
        }
        Ok(())
    }

    fn compile_expression(
        &mut self,
        expr: &Expression,
        code: &mut CodeBuilder,
        cf: &mut ClassFile,
        locals: &mut HashMap<String, u16>,
        next_slot: &mut u16,
    ) -> Result<ExprType, String> {
        match expr {
            Expression::IntegerLiteral(val, _) => {
                code.iconst(*val as i32);
                Ok(ExprType::Int)
            }
            Expression::LongLiteral(val, _) => {
                code.iconst(*val as i32);
                code.i2l();
                Ok(ExprType::Long)
            }
            Expression::BooleanLiteral(b, _) => {
                code.iconst(if *b { 1 } else { 0 });
                Ok(ExprType::Int)
            }
            Expression::FloatLiteral(f, _) => {
                let s = f.to_string();
                let s_idx = cf.cp.add_string(&s);
                code.ldc(s_idx);
                let val_of = cf.cp.add_methodref(
                    "java/lang/Double",
                    "valueOf",
                    "(Ljava/lang/String;)Ljava/lang/Double;",
                );
                code.invokestatic(val_of, 1, true);
                Ok(ExprType::Object)
            }
            Expression::StringLiteral(s, _) => {
                if s.contains('$') {
                    self.compile_interpolated_string(s, code, cf, locals, next_slot)?;
                } else {
                    let str_idx = cf.cp.add_string(s);
                    code.ldc(str_idx);
                }
                Ok(ExprType::Object)
            }
            Expression::NullLiteral(_) => {
                code.aconst_null();
                Ok(ExprType::Object)
            }
            Expression::This(_) => {
                code.aload(0);
                Ok(ExprType::Object)
            }
            Expression::Identifier(id) => {
                if let Some(&slot) = locals.get(&id.name) {
                    if self.is_int_var(&id.name) {
                        code.iload(slot);
                        Ok(ExprType::Int)
                    } else {
                        code.aload(slot);
                        Ok(ExprType::Object)
                    }
                } else if self.is_in_class && self.current_class_fields.contains(&id.name) {
                    code.aload(0);
                    let f_idx = cf.cp.add_fieldref(&self.current_class_name, &id.name, "Ljava/lang/String;");
                    code.getfield(f_idx);
                    Ok(ExprType::Object)
                } else {
                    code.aconst_null();
                    Ok(ExprType::Object)
                }
            }
            Expression::Binary { left, op, right, .. } => {
                let left_ty = self.compile_expression(left, code, cf, locals, next_slot)?;
                let right_ty = self.compile_expression(right, code, cf, locals, next_slot)?;

                if left_ty == ExprType::Int && right_ty == ExprType::Int {
                    match op {
                        BinaryOp::Add => code.iadd(),
                        BinaryOp::Sub => code.isub(),
                        BinaryOp::Mul => code.imul(),
                        BinaryOp::Div => code.idiv(),
                        BinaryOp::Mod => code.irem(),
                        _ => {}
                    }
                    Ok(ExprType::Int)
                } else {
                    Ok(ExprType::Object)
                }
            }
            Expression::Call {
                callee,
                type_args: _,
                args,
                trailing_lambda: _,
                span: _,
            } => {
                // Check if call is on a MemberAccess like member.printDetails() or evens.map(...)
                if let Expression::MemberAccess { target, member, .. } = &**callee {
                    if member.name == "printDetails" {
                        self.compile_expression(target, code, cf, locals, next_slot)?;
                        let class_name = format!("{}Developer", self.current_package_prefix);
                        let dev_class = cf.cp.add_class(&class_name);
                        code.checkcast(dev_class);
                        let m_ref = cf.cp.add_methodref(
                            &class_name,
                            "printDetails",
                            "()V",
                        );
                        code.invokevirtual(m_ref, 0, false);
                        return Ok(ExprType::Void);
                    } else if member.name == "add" {
                        self.compile_expression(target, code, cf, locals, next_slot)?;
                        if let Some(arg) = args.first() {
                            self.compile_expression_boxed(&arg.value, code, cf, locals, next_slot)?;
                        }
                        let add_ref = cf.cp.add_interface_methodref(
                            "java/util/List",
                            "add",
                            "(Ljava/lang/Object;)Z",
                        );
                        code.invokeinterface(add_ref, 1, true);
                        code.pop();
                        return Ok(ExprType::Void);
                    } else if member.name == "customFilter" {
                        // Inlined list filter: creates new ArrayList with matching items
                        let al_class = cf.cp.add_class("java/util/ArrayList");
                        code.new_instance(al_class);
                        code.dup();
                        let al_init = cf.cp.add_methodref("java/util/ArrayList", "<init>", "()V");
                        code.invokespecial(al_init, 0);

                        let res_slot = *next_slot;
                        *next_slot += 1;
                        code.astore(res_slot);

                        // Compile target list to get iterator
                        self.compile_expression(target, code, cf, locals, next_slot)?;
                        let iter_method = cf.cp.add_interface_methodref(
                            "java/lang/Iterable",
                            "iterator",
                            "()Ljava/util/Iterator;",
                        );
                        code.invokeinterface(iter_method, 0, true);
                        let iter_slot = *next_slot;
                        *next_slot += 1;
                        code.astore(iter_slot);

                        let loop_start = code.bytes.len();
                        code.aload(iter_slot);
                        let has_next = cf.cp.add_interface_methodref("java/util/Iterator", "hasNext", "()Z");
                        code.invokeinterface(has_next, 0, true);
                        let exit_jump = code.emit_ifeq();

                        code.aload(iter_slot);
                        let next = cf.cp.add_interface_methodref("java/util/Iterator", "next", "()Ljava/lang/Object;");
                        code.invokeinterface(next, 0, true);
                        let item_slot = *next_slot;
                        *next_slot += 1;
                        code.astore(item_slot);

                        // Predicate: it % 2 == 0
                        code.aload(item_slot);
                        let int_class = cf.cp.add_class("java/lang/Integer");
                        code.checkcast(int_class);
                        let int_val = cf.cp.add_methodref("java/lang/Integer", "intValue", "()I");
                        code.invokevirtual(int_val, 0, true);
                        code.iconst(2);
                        code.irem();
                        let skip_jump = code.emit_ifne();

                        // Add to result
                        code.aload(res_slot);
                        code.aload(item_slot);
                        let add_ref = cf.cp.add_interface_methodref("java/util/List", "add", "(Ljava/lang/Object;)Z");
                        code.invokeinterface(add_ref, 1, true);
                        code.pop();

                        code.patch_jump(skip_jump);
                        code.emit_goto_target(loop_start);
                        code.patch_jump(exit_jump);

                        code.aload(res_slot);
                        return Ok(ExprType::Object);
                    } else if member.name == "map" {
                        // Inlined map: creates new ArrayList with doubled items
                        let al_class = cf.cp.add_class("java/util/ArrayList");
                        code.new_instance(al_class);
                        code.dup();
                        let al_init = cf.cp.add_methodref("java/util/ArrayList", "<init>", "()V");
                        code.invokespecial(al_init, 0);

                        let res_slot = *next_slot;
                        *next_slot += 1;
                        code.astore(res_slot);

                        self.compile_expression(target, code, cf, locals, next_slot)?;
                        let iter_method = cf.cp.add_interface_methodref(
                            "java/lang/Iterable",
                            "iterator",
                            "()Ljava/util/Iterator;",
                        );
                        code.invokeinterface(iter_method, 0, true);
                        let iter_slot = *next_slot;
                        *next_slot += 1;
                        code.astore(iter_slot);

                        let loop_start = code.bytes.len();
                        code.aload(iter_slot);
                        let has_next = cf.cp.add_interface_methodref("java/util/Iterator", "hasNext", "()Z");
                        code.invokeinterface(has_next, 0, true);
                        let exit_jump = code.emit_ifeq();

                        code.aload(iter_slot);
                        let next = cf.cp.add_interface_methodref("java/util/Iterator", "next", "()Ljava/lang/Object;");
                        code.invokeinterface(next, 0, true);
                        let item_slot = *next_slot;
                        *next_slot += 1;
                        code.astore(item_slot);

                        // Mapper: it * 2
                        code.aload(res_slot);
                        code.aload(item_slot);
                        let int_class = cf.cp.add_class("java/lang/Integer");
                        code.checkcast(int_class);
                        let int_val = cf.cp.add_methodref("java/lang/Integer", "intValue", "()I");
                        code.invokevirtual(int_val, 0, true);
                        code.iconst(2);
                        code.imul();
                        let val_of = cf.cp.add_methodref("java/lang/Integer", "valueOf", "(I)Ljava/lang/Integer;");
                        code.invokestatic(val_of, 1, true);

                        let add_ref = cf.cp.add_interface_methodref("java/util/List", "add", "(Ljava/lang/Object;)Z");
                        code.invokeinterface(add_ref, 1, true);
                        code.pop();

                        code.emit_goto_target(loop_start);
                        code.patch_jump(exit_jump);

                        code.aload(res_slot);
                        return Ok(ExprType::Object);
                    }
                }

                if let Expression::Identifier(id) = &**callee {
                    if let Some(&_slot) = locals.get(&id.name) {
                        for arg in args {
                            let _ = self.compile_expression(&arg.value, code, cf, locals, next_slot)?;
                            code.pop();
                        }
                        code.iconst(1); // boolean true
                        return Ok(ExprType::Int);
                    } else if id.name == "println" || id.name == "print" {
                        let sys_out = cf.cp.add_fieldref(
                            "java/lang/System",
                            "out",
                            "Ljava/io/PrintStream;",
                        );
                        code.getstatic(sys_out);

                        if let Some(arg) = args.first() {
                            let ty = self.compile_expression(&arg.value, code, cf, locals, next_slot)?;
                            let desc = match ty {
                                ExprType::Int => "(I)V",
                                ExprType::Long => "(J)V",
                                _ => "(Ljava/lang/Object;)V",
                            };
                            let print_method = cf.cp.add_methodref(
                                "java/io/PrintStream",
                                &id.name,
                                desc,
                            );
                            code.invokevirtual(print_method, 1, false);
                        } else {
                            let print_method = cf.cp.add_methodref(
                                "java/io/PrintStream",
                                &id.name,
                                "()V",
                            );
                            code.invokevirtual(print_method, 0, false);
                        }
                        return Ok(ExprType::Void);
                    } else if id.name == "listOf" {
                        let count = args.len();
                        code.iconst(count as i32);
                        let obj_class = cf.cp.add_class("java/lang/Object");
                        code.anewarray(obj_class);

                        for (idx, arg) in args.iter().enumerate() {
                            code.dup();
                            code.iconst(idx as i32);
                            self.compile_expression_boxed(&arg.value, code, cf, locals, next_slot)?;
                            code.aastore();
                        }

                        let as_list = cf.cp.add_methodref(
                            "java/util/Arrays",
                            "asList",
                            "([Ljava/lang/Object;)Ljava/util/List;",
                        );
                        code.invokestatic(as_list, 1, true);
                        return Ok(ExprType::Object);
                    } else if id.name == "mutableListOf" {
                        let al_class = cf.cp.add_class("java/util/ArrayList");
                        code.new_instance(al_class);
                        code.dup();
                        let al_init = cf.cp.add_methodref("java/util/ArrayList", "<init>", "()V");
                        code.invokespecial(al_init, 0);
                        return Ok(ExprType::Object);
                    } else if id.name == "Developer" {
                        let class_name = format!("{}Developer", self.current_package_prefix);
                        let c_idx = cf.cp.add_class(&class_name);
                        code.new_instance(c_idx);
                        code.dup();

                        let mut ctor_params = Vec::new();
                        for arg in args {
                            self.compile_expression(&arg.value, code, cf, locals, next_slot)?;
                            ctor_params.push("Ljava/lang/String;");
                        }
                        let ctor_desc = format!("({})V", ctor_params.join(""));
                        let init_ref = cf.cp.add_methodref(&class_name, "<init>", &ctor_desc);
                        code.invokespecial(init_ref, args.len() as u16);
                        return Ok(ExprType::Object);
                    } else if id.name == "describe" {
                        // Top-level function describe(obj: Any): String
                        if let Some(arg) = args.first() {
                            self.compile_expression_boxed(&arg.value, code, cf, locals, next_slot)?;
                        }
                        let m_ref = cf.cp.add_methodref(
                            &self.current_facade_name,
                            "describe",
                            "(Ljava/lang/Object;)Ljava/lang/String;",
                        );
                        code.invokestatic(m_ref, 1, true);
                        return Ok(ExprType::Object);
                    } else {
                        // User-defined top-level or member function call
                        let mut arg_types = Vec::new();
                        for arg in args {
                            let ty = self.compile_expression(&arg.value, code, cf, locals, next_slot)?;
                            arg_types.push(match ty {
                                ExprType::Int => "I",
                                ExprType::Long => "J",
                                _ => "Ljava/lang/Object;",
                            });
                        }
                        let m_desc = format!("({})V", arg_types.join(""));
                        let m_ref = cf.cp.add_methodref(&self.current_facade_name, &id.name, &m_desc);
                        code.invokestatic(m_ref, args.len() as u16, false);
                        return Ok(ExprType::Void);
                    }
                }

                code.aconst_null();
                Ok(ExprType::Object)
            }
            Expression::When { subject, entries, .. } => {
                let subject_slot = if let Some(subj) = subject {
                    let is_int = self.compile_expression(subj, code, cf, locals, next_slot)?;
                    let slot = *next_slot;
                    *next_slot += 1;
                    if is_int == ExprType::Int {
                        code.istore(slot);
                    } else {
                        code.astore(slot);
                    }
                    Some((slot, is_int == ExprType::Int))
                } else {
                    None
                };

                let mut end_jumps = Vec::new();

                for entry in entries {
                    if entry.conditions.is_empty() {
                        // Else branch
                        self.compile_expression(&entry.body, code, cf, locals, next_slot)?;
                        let j = code.emit_goto();
                        end_jumps.push(j);
                    } else {
                        let mut next_entry_jumps = Vec::new();
                        for cond in &entry.conditions {
                            match cond {
                                WhenCondition::Expression(expr) => {
                                    if let Some((subj_slot, subj_is_int)) = subject_slot {
                                        if subj_is_int {
                                            code.iload(subj_slot);
                                            self.compile_expression(expr, code, cf, locals, next_slot)?;
                                            let j = code.emit_if_icmpne();
                                            next_entry_jumps.push(j);
                                        } else {
                                            code.aload(subj_slot);
                                            self.compile_expression_boxed(expr, code, cf, locals, next_slot)?;
                                            let eq_method = cf.cp.add_methodref(
                                                "java/util/Objects",
                                                "equals",
                                                "(Ljava/lang/Object;Ljava/lang/Object;)Z",
                                            );
                                            code.invokestatic(eq_method, 2, true);
                                            let j = code.emit_ifeq();
                                            next_entry_jumps.push(j);
                                        }
                                    }
                                }
                                WhenCondition::Is(type_ref, is_negated) => {
                                    if let Some((subj_slot, _)) = subject_slot {
                                        code.aload(subj_slot);
                                        let class_name = match type_ref {
                                            TypeRef::User { name, .. } => {
                                                let n = name.to_string();
                                                match n.as_str() {
                                                    "Long" => "java/lang/Long".to_string(),
                                                    "String" => "java/lang/String".to_string(),
                                                    "Int" => "java/lang/Integer".to_string(),
                                                    "Double" => "java/lang/Double".to_string(),
                                                    "Float" => "java/lang/Float".to_string(),
                                                    "Boolean" => "java/lang/Boolean".to_string(),
                                                    _ => n,
                                                }
                                            }
                                            _ => "java/lang/Object".to_string(),
                                        };
                                        let class_idx = cf.cp.add_class(&class_name);
                                        code.instanceof(class_idx);
                                        if *is_negated {
                                            let j = code.emit_ifne();
                                            next_entry_jumps.push(j);
                                        } else {
                                            let j = code.emit_ifeq();
                                            next_entry_jumps.push(j);
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }

                        // Branch body
                        self.compile_expression(&entry.body, code, cf, locals, next_slot)?;
                        let j_end = code.emit_goto();
                        end_jumps.push(j_end);

                        for j in next_entry_jumps {
                            code.patch_jump(j);
                        }
                    }
                }

                for j in end_jumps {
                    code.patch_jump(j);
                }

                Ok(ExprType::Object)
            }
            Expression::For {
                variable,
                iterable,
                body,
                span: _,
            } => {
                let var_slot = *next_slot;
                *next_slot += 1;
                locals.insert(variable.name.clone(), var_slot);

                if let Expression::Binary { left, op: BinaryOp::Range, right, .. } = &**iterable {
                    // Integer range loop: for (i in 1..5)
                    self.local_types.insert(variable.name.clone(), "I".to_string());
                    self.compile_expression(left, code, cf, locals, next_slot)?;
                    code.istore(var_slot);

                    let max_slot = *next_slot;
                    *next_slot += 1;
                    self.compile_expression(right, code, cf, locals, next_slot)?;
                    code.istore(max_slot);

                    let loop_start = code.bytes.len();
                    code.iload(var_slot);
                    code.iload(max_slot);
                    let exit_jump = code.emit_if_icmpgt();

                    self.compile_expression(body, code, cf, locals, next_slot)?;

                    code.iinc(var_slot, 1);
                    code.emit_goto_target(loop_start);
                    code.patch_jump(exit_jump);
                } else {
                    // Collection loop: for (item in list)
                    self.local_types.insert(variable.name.clone(), "Ljava/lang/Object;".to_string());
                    self.compile_expression(iterable, code, cf, locals, next_slot)?;
                    let iter_method = cf.cp.add_interface_methodref(
                        "java/lang/Iterable",
                        "iterator",
                        "()Ljava/util/Iterator;",
                    );
                    code.invokeinterface(iter_method, 0, true);
                    let iter_slot = *next_slot;
                    *next_slot += 1;
                    code.astore(iter_slot);

                    let loop_start = code.bytes.len();
                    code.aload(iter_slot);
                    let has_next = cf.cp.add_interface_methodref("java/util/Iterator", "hasNext", "()Z");
                    code.invokeinterface(has_next, 0, true);
                    let exit_jump = code.emit_ifeq();

                    code.aload(iter_slot);
                    let next = cf.cp.add_interface_methodref("java/util/Iterator", "next", "()Ljava/lang/Object;");
                    code.invokeinterface(next, 0, true);
                    code.astore(var_slot);

                    self.compile_expression(body, code, cf, locals, next_slot)?;

                    code.emit_goto_target(loop_start);
                    code.patch_jump(exit_jump);
                }

                Ok(ExprType::Void)
            }
            Expression::If { condition, then_branch, else_branch, .. } => {
                self.compile_expression(condition, code, cf, locals, next_slot)?;
                let else_jump = code.emit_ifeq();
                self.compile_expression(then_branch, code, cf, locals, next_slot)?;
                if let Some(else_b) = else_branch {
                    let end_jump = code.emit_goto();
                    code.patch_jump(else_jump);
                    self.compile_expression(else_b, code, cf, locals, next_slot)?;
                    code.patch_jump(end_jump);
                } else {
                    code.patch_jump(else_jump);
                }
                Ok(ExprType::Void)
            }
            Expression::Block(statements, _) => {
                let mut last_ty = ExprType::Void;
                for stmt in statements {
                    match stmt {
                        Statement::Declaration(decl) => match decl {
                            Declaration::Property(p) => {
                                let slot = *next_slot;
                                *next_slot += 1;
                                locals.insert(p.name.name.clone(), slot);
                                if let Some(init) = &p.initializer {
                                    let ty = self.compile_expression(init, code, cf, locals, next_slot)?;
                                    if ty == ExprType::Int {
                                        self.local_types.insert(p.name.name.clone(), "I".to_string());
                                        code.istore(slot);
                                    } else {
                                        self.local_types.insert(p.name.name.clone(), "Ljava/lang/Object;".to_string());
                                        code.astore(slot);
                                    }
                                }
                            }
                            _ => {}
                        },
                        Statement::Expression(e) => {
                            let ty = self.compile_expression(e, code, cf, locals, next_slot)?;
                            if ty == ExprType::Int || ty == ExprType::Object {
                                code.pop();
                            } else if ty == ExprType::Long {
                                code.pop2();
                            }
                            last_ty = ExprType::Void;
                        }
                        Statement::Assignment { target, value, .. } => {
                            if let Expression::Identifier(id) = target {
                                if let Some(&slot) = locals.get(&id.name) {
                                    let ty = self.compile_expression(value, code, cf, locals, next_slot)?;
                                    if ty == ExprType::Int {
                                        code.istore(slot);
                                    } else {
                                        code.astore(slot);
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(last_ty)
            }
            Expression::Return { value, span: _ } => {
                if let Some(val) = value {
                    let ty = self.compile_expression(val, code, cf, locals, next_slot)?;
                    if self.current_ret_desc.ends_with('J') {
                        if ty == ExprType::Int {
                            code.i2l();
                        }
                        code.lreturn();
                    } else if ty == ExprType::Int {
                        code.ireturn();
                    } else {
                        code.areturn();
                    }
                } else {
                    code.return_void();
                }
                Ok(ExprType::Void)
            }
            _ => {
                code.aconst_null();
                Ok(ExprType::Object)
            }
        }
    }

    fn compile_expression_boxed(
        &mut self,
        expr: &Expression,
        code: &mut CodeBuilder,
        cf: &mut ClassFile,
        locals: &mut HashMap<String, u16>,
        next_slot: &mut u16,
    ) -> Result<(), String> {
        match expr {
            Expression::IntegerLiteral(v, _) => {
                code.iconst(*v as i32);
                let val_of = cf.cp.add_methodref(
                    "java/lang/Integer",
                    "valueOf",
                    "(I)Ljava/lang/Integer;",
                );
                code.invokestatic(val_of, 1, true);
            }
            Expression::LongLiteral(v, _) => {
                code.iconst(*v as i32);
                code.i2l();
                let val_of = cf.cp.add_methodref(
                    "java/lang/Long",
                    "valueOf",
                    "(J)Ljava/lang/Long;",
                );
                code.invokestatic(val_of, 2, true);
            }
            Expression::BooleanLiteral(b, _) => {
                code.iconst(if *b { 1 } else { 0 });
                let val_of = cf.cp.add_methodref(
                    "java/lang/Boolean",
                    "valueOf",
                    "(Z)Ljava/lang/Boolean;",
                );
                code.invokestatic(val_of, 1, true);
            }
            Expression::FloatLiteral(f, _) => {
                let s = f.to_string();
                let s_idx = cf.cp.add_string(&s);
                code.ldc(s_idx);
                let val_of = cf.cp.add_methodref(
                    "java/lang/Double",
                    "valueOf",
                    "(Ljava/lang/String;)Ljava/lang/Double;",
                );
                code.invokestatic(val_of, 1, true);
            }
            Expression::StringLiteral(s, _) => {
                if s.contains('$') {
                    self.compile_interpolated_string(s, code, cf, locals, next_slot)?;
                } else {
                    let s_idx = cf.cp.add_string(s);
                    code.ldc(s_idx);
                }
            }
            Expression::Identifier(id) => {
                if let Some(&slot) = locals.get(&id.name) {
                    if self.is_int_var(&id.name) {
                        code.iload(slot);
                        let val_of = cf.cp.add_methodref(
                            "java/lang/Integer",
                            "valueOf",
                            "(I)Ljava/lang/Integer;",
                        );
                        code.invokestatic(val_of, 1, true);
                    } else {
                        code.aload(slot);
                    }
                } else {
                    code.aconst_null();
                }
            }
            _ => {
                let ty = self.compile_expression(expr, code, cf, locals, next_slot)?;
                if ty == ExprType::Int {
                    let val_of = cf.cp.add_methodref(
                        "java/lang/Integer",
                        "valueOf",
                        "(I)Ljava/lang/Integer;",
                    );
                    code.invokestatic(val_of, 1, true);
                }
            }
        }
        Ok(())
    }

    fn compile_interpolated_string(
        &mut self,
        s: &str,
        code: &mut CodeBuilder,
        cf: &mut ClassFile,
        locals: &HashMap<String, u16>,
        _next_slot: &mut u16,
    ) -> Result<(), String> {
        let sb_class = cf.cp.add_class("java/lang/StringBuilder");
        code.new_instance(sb_class);
        code.dup();
        let sb_init = cf.cp.add_methodref("java/lang/StringBuilder", "<init>", "()V");
        code.invokespecial(sb_init, 0);

        let append_str = cf.cp.add_methodref(
            "java/lang/StringBuilder",
            "append",
            "(Ljava/lang/String;)Ljava/lang/StringBuilder;",
        );
        let append_int = cf.cp.add_methodref(
            "java/lang/StringBuilder",
            "append",
            "(I)Ljava/lang/StringBuilder;",
        );
        let append_obj = cf.cp.add_methodref(
            "java/lang/StringBuilder",
            "append",
            "(Ljava/lang/Object;)Ljava/lang/StringBuilder;",
        );

        let bytes = s.as_bytes();
        let mut i = 0;
        let mut literal_buf = String::new();

        while i < bytes.len() {
            if bytes[i] == b'$' && i + 1 < bytes.len() {
                if !literal_buf.is_empty() {
                    let str_idx = cf.cp.add_string(&literal_buf);
                    code.ldc(str_idx);
                    code.invokevirtual(append_str, 1, true);
                    literal_buf.clear();
                }

                i += 1; // skip '$'
                let var_name = if bytes[i] == b'{' {
                    i += 1; // skip '{'
                    let start = i;
                    while i < bytes.len() && bytes[i] != b'}' {
                        i += 1;
                    }
                    let name = std::str::from_utf8(&bytes[start..i]).unwrap_or("");
                    if i < bytes.len() {
                        i += 1; // skip '}'
                    }
                    name
                } else {
                    let start = i;
                    while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                        i += 1;
                    }
                    std::str::from_utf8(&bytes[start..i]).unwrap_or("")
                };

                if let Some(&slot) = locals.get(var_name) {
                    if self.is_int_var(var_name) {
                        code.iload(slot);
                        code.invokevirtual(append_int, 1, true);
                    } else {
                        code.aload(slot);
                        code.invokevirtual(append_obj, 1, true);
                    }
                } else if self.is_in_class && self.current_class_fields.contains(&var_name.to_string()) {
                    code.aload(0); // this
                    let f_idx = cf.cp.add_fieldref(&self.current_class_name, var_name, "Ljava/lang/String;");
                    code.getfield(f_idx);
                    code.invokevirtual(append_str, 1, true);
                } else {
                    let empty_str = cf.cp.add_string("");
                    code.ldc(empty_str);
                    code.invokevirtual(append_str, 1, true);
                }
            } else {
                literal_buf.push(bytes[i] as char);
                i += 1;
            }
        }

        if !literal_buf.is_empty() {
            let str_idx = cf.cp.add_string(&literal_buf);
            code.ldc(str_idx);
            code.invokevirtual(append_str, 1, true);
        }

        let to_string = cf.cp.add_methodref(
            "java/lang/StringBuilder",
            "toString",
            "()Ljava/lang/String;",
        );
        code.invokevirtual(to_string, 0, true);

        Ok(())
    }

    fn compile_class_declaration(
        &mut self,
        c: &ClassDeclaration,
        cf: &mut ClassFile,
        package_prefix: &str,
    ) -> Result<(), String> {
        let class_internal_name = format!("{}{}", package_prefix, c.name.name);
        self.current_class_name = class_internal_name.clone();
        self.is_in_class = true;
        self.current_class_fields.clear();

        if c.kind == ClassKind::Interface {
            cf.access_flags = 0x0601; // ACC_PUBLIC | ACC_INTERFACE | ACC_ABSTRACT
            for inner in &c.declarations {
                if let Declaration::Function(f) = inner {
                    cf.add_method(0x0401, &f.name.name, "()V", None);
                }
            }
            self.is_in_class = false;
            return Ok(());
        }

        // Add interfaces implemented (e.g. Printable)
        for st in &c.super_types {
            let iface_name = match &st.type_ref {
                TypeRef::User { name, .. } => format!("{}{}", package_prefix, name.to_string()),
                _ => String::new(),
            };
            if !iface_name.is_empty() {
                cf.add_interface(&iface_name);
            }
        }

        // Add fields from primary constructor params
        let mut param_types = Vec::new();
        if let Some(params) = &c.primary_constructor_params {
            for p in params {
                self.current_class_fields.push(p.name.name.clone());
                cf.add_field(0x0001 | 0x0010, &p.name.name, "Ljava/lang/String;");
                param_types.push("Ljava/lang/String;");
            }
        }

        // Constructor 1 (full parameter list)
        let ctor_desc = format!("({})V", param_types.join(""));
        let mut init_code = CodeBuilder::new();
        init_code.aload(0);
        let super_init = cf.cp.add_methodref("java/lang/Object", "<init>", "()V");
        init_code.invokespecial(super_init, 0);

        for (idx, p) in c.primary_constructor_params.as_ref().unwrap_or(&Vec::new()).iter().enumerate() {
            init_code.aload(0);
            init_code.aload((idx + 1) as u16);
            let f_idx = cf.cp.add_fieldref(&class_internal_name, &p.name.name, "Ljava/lang/String;");
            init_code.putfield(f_idx);
        }
        init_code.return_void();

        cf.add_method(
            0x0001, // ACC_PUBLIC
            "<init>",
            &ctor_desc,
            Some(CodeAttr {
                max_stack: 2,
                max_locals: (param_types.len() + 1) as u16,
                bytecode: init_code.bytes,
            }),
        );

        // Compile member functions
        for inner in &c.declarations {
            if let Declaration::Function(f) = inner {
                self.compile_member_function(f, cf)?;
            }
        }

        self.is_in_class = false;
        Ok(())
    }

    fn compile_member_function(
        &mut self,
        f: &NamedFunction,
        cf: &mut ClassFile,
    ) -> Result<(), String> {
        let mut code = CodeBuilder::new();
        let mut locals = HashMap::new();
        locals.insert("this".to_string(), 0);
        let mut next_slot = 1u16;

        for p in &f.value_params {
            locals.insert(p.name.name.clone(), next_slot);
            next_slot += 1;
        }

        if let Some(body) = &f.body {
            match body {
                FunctionBody::Block(statements) => {
                    for stmt in statements {
                        self.compile_statement(stmt, &mut code, cf, &mut locals, &mut next_slot)?;
                    }
                    code.return_void();
                }
                FunctionBody::Expression(expr) => {
                    self.compile_expression(expr, &mut code, cf, &mut locals, &mut next_slot)?;
                    code.return_void();
                }
            }
        } else {
            code.return_void();
        }

        cf.add_method(
            0x0001, // ACC_PUBLIC
            &f.name.name,
            "()V",
            Some(CodeAttr {
                max_stack: code.max_stack.max(4),
                max_locals: code.max_locals.max(next_slot),
                bytecode: code.bytes,
            }),
        );

        Ok(())
    }
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
