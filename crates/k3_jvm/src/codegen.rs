use crate::bytecode::CodeBuilder;
use crate::classfile::{ClassFile, CodeAttr, ExceptionHandler};
use k3_syntax::ast::*;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JvmType {
    Int,            // I (also Byte, Short, Char)
    Boolean,        // Z (1 stack slot, distinct for StringBuilder/boxing)
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
            JvmType::Boolean => "Z".to_string(),
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
            JvmType::Int | JvmType::Boolean => code.iload(slot),
            JvmType::Long => code.lload(slot),
            JvmType::Float => code.fload(slot),
            JvmType::Double => code.dload(slot),
            JvmType::Object(_) => code.aload(slot),
            JvmType::Void => {}
        }
    }

    pub fn emit_store(&self, code: &mut CodeBuilder, slot: u16) {
        match self {
            JvmType::Int | JvmType::Boolean => code.istore(slot),
            JvmType::Long => code.lstore(slot),
            JvmType::Float => code.fstore(slot),
            JvmType::Double => code.dstore(slot),
            JvmType::Object(_) => code.astore(slot),
            JvmType::Void => {}
        }
    }

    pub fn emit_return(&self, code: &mut CodeBuilder) {
        match self {
            JvmType::Int | JvmType::Boolean => code.ireturn(),
            JvmType::Long => code.lreturn(),
            JvmType::Float => code.freturn(),
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

    pub fn is_integral_or_bool(&self) -> bool {
        matches!(self, JvmType::Int | JvmType::Boolean)
    }
}

pub fn type_ref_to_jvm(tr: &TypeRef, package_prefix: &str, class_infos: &HashMap<String, ClassInfo>) -> JvmType {
    match tr {
        TypeRef::User { name, .. } => {
            let n = name.to_string();
            match n.as_str() {
                "Int" | "Byte" | "Short" | "Char" => JvmType::Int,
                "Boolean" => JvmType::Boolean,
                "Long" => JvmType::Long,
                "Float" => JvmType::Float,
                "Double" => JvmType::Double,
                "String" => JvmType::Object("Ljava/lang/String;".to_string()),
                "Unit" => JvmType::Void,
                "IntArray" => JvmType::Object("[I".to_string()),
                "ByteArray" => JvmType::Object("[B".to_string()),
                "LongArray" => JvmType::Object("[J".to_string()),
                "DoubleArray" => JvmType::Object("[D".to_string()),
                "List" | "MutableList" | "Collection" | "Iterable" => {
                    JvmType::Object("Ljava/util/List;".to_string())
                }
                "Any" => JvmType::Object("Ljava/lang/Object;".to_string()),
                _ => {
                    if let Some(ci) = class_infos.get(&n) {
                        JvmType::Object(format!("L{};", ci.internal_name))
                    } else if n.len() == 1 && n.chars().all(|c| c.is_ascii_uppercase()) {
                        // Unbound generic type parameter erases to Object
                        JvmType::Object("Ljava/lang/Object;".to_string())
                    } else {
                        JvmType::Object(format!("L{}{};", package_prefix, n))
                    }
                }
            }
        }
        TypeRef::Nullable { inner, .. } => {
            let inner_ty = type_ref_to_jvm(inner, package_prefix, class_infos);
            match inner_ty {
                JvmType::Int => JvmType::Object("Ljava/lang/Integer;".to_string()),
                JvmType::Long => JvmType::Object("Ljava/lang/Long;".to_string()),
                JvmType::Double => JvmType::Object("Ljava/lang/Double;".to_string()),
                JvmType::Float => JvmType::Object("Ljava/lang/Float;".to_string()),
                JvmType::Boolean => JvmType::Object("Ljava/lang/Boolean;".to_string()),
                other => other,
            }
        }
        TypeRef::Function { .. } => JvmType::Object("Ljava/util/function/Function;".to_string()),
    }
}

pub fn type_ref_to_internal_class(tr: &TypeRef, package_prefix: &str, class_infos: &HashMap<String, ClassInfo>) -> String {
    match tr {
        TypeRef::User { name, .. } => {
            let n = name.to_string();
            match n.as_str() {
                "Int" => "java/lang/Integer".to_string(),
                "Long" => "java/lang/Long".to_string(),
                "Double" => "java/lang/Double".to_string(),
                "Float" => "java/lang/Float".to_string(),
                "Boolean" => "java/lang/Boolean".to_string(),
                "String" => "java/lang/String".to_string(),
                "List" | "MutableList" => "java/util/List".to_string(),
                "Any" => "java/lang/Object".to_string(),
                _ => class_infos
                    .get(&n)
                    .map(|c| c.internal_name.clone())
                    .unwrap_or_else(|| format!("{}{}", package_prefix, n)),
            }
        }
        TypeRef::Nullable { inner, .. } => type_ref_to_internal_class(inner, package_prefix, class_infos),
        TypeRef::Function { .. } => "java/util/function/Function".to_string(),
    }
}

#[derive(Debug, Clone)]
pub struct FunctionSignature {
    pub receiver_type: Option<JvmType>,
    pub param_types: Vec<JvmType>,
    pub param_defaults: Vec<Option<Expression>>,
    pub return_type: JvmType,
    pub is_inline: bool,
}

#[derive(Debug, Clone)]
pub struct ClassInfo {
    pub internal_name: String,
    pub is_interface: bool,
    pub interfaces: Vec<String>,
    pub fields: Vec<(String, JvmType)>,
    pub constructor_params: Vec<(String, JvmType, Option<Expression>)>,
    pub methods: HashMap<String, FunctionSignature>,
}

fn emit_iconst(code: &mut CodeBuilder, cf: &mut ClassFile, val: i32) {
    if (-32768..=32767).contains(&val) {
        code.iconst(val);
    } else {
        let idx = cf.cp.add_integer(val);
        code.ldc(idx);
    }
}

fn emit_box(ty: &JvmType, code: &mut CodeBuilder, cf: &mut ClassFile) -> JvmType {
    match ty {
        JvmType::Int => {
            let m = cf.cp.add_methodref("java/lang/Integer", "valueOf", "(I)Ljava/lang/Integer;");
            code.invokestatic_ret(m, 1, 1);
            JvmType::Object("Ljava/lang/Integer;".to_string())
        }
        JvmType::Boolean => {
            let m = cf.cp.add_methodref("java/lang/Boolean", "valueOf", "(Z)Ljava/lang/Boolean;");
            code.invokestatic_ret(m, 1, 1);
            JvmType::Object("Ljava/lang/Boolean;".to_string())
        }
        JvmType::Long => {
            let m = cf.cp.add_methodref("java/lang/Long", "valueOf", "(J)Ljava/lang/Long;");
            code.invokestatic_ret(m, 2, 1);
            JvmType::Object("Ljava/lang/Long;".to_string())
        }
        JvmType::Double => {
            let m = cf.cp.add_methodref("java/lang/Double", "valueOf", "(D)Ljava/lang/Double;");
            code.invokestatic_ret(m, 2, 1);
            JvmType::Object("Ljava/lang/Double;".to_string())
        }
        JvmType::Float => {
            let m = cf.cp.add_methodref("java/lang/Float", "valueOf", "(F)Ljava/lang/Float;");
            code.invokestatic_ret(m, 1, 1);
            JvmType::Object("Ljava/lang/Float;".to_string())
        }
        JvmType::Void => {
            code.aconst_null();
            JvmType::Object("Ljava/lang/Object;".to_string())
        }
        JvmType::Object(_) => ty.clone(),
    }
}

fn emit_unbox_if_needed(actual: &JvmType, expected: &JvmType, code: &mut CodeBuilder, cf: &mut ClassFile) {
    match (actual, expected) {
        (JvmType::Int, JvmType::Long) => code.i2l(),
        (JvmType::Int, JvmType::Double) => code.i2d(),
        (JvmType::Long, JvmType::Double) => code.l2d(),
        (_, JvmType::Object(exp_desc)) if !matches!(actual, JvmType::Object(_)) => {
            let boxed = emit_box(actual, code, cf);
            if exp_desc != "Ljava/lang/Object;" && boxed.descriptor() != *exp_desc {
                let target_cls = exp_desc.trim_start_matches('L').trim_end_matches(';');
                let c_idx = cf.cp.add_class(target_cls);
                code.checkcast(c_idx);
            }
        }
        (JvmType::Object(act_desc), JvmType::Int) => {
            if act_desc != "Ljava/lang/Integer;" {
                let c_idx = cf.cp.add_class("java/lang/Integer");
                code.checkcast(c_idx);
            }
            let m = cf.cp.add_methodref("java/lang/Integer", "intValue", "()I");
            code.invokevirtual_ret(m, 0, 1);
        }
        (JvmType::Object(act_desc), JvmType::Boolean) => {
            if act_desc != "Ljava/lang/Boolean;" {
                let c_idx = cf.cp.add_class("java/lang/Boolean");
                code.checkcast(c_idx);
            }
            let m = cf.cp.add_methodref("java/lang/Boolean", "booleanValue", "()Z");
            code.invokevirtual_ret(m, 0, 1);
        }
        (JvmType::Object(act_desc), JvmType::Long) => {
            if act_desc != "Ljava/lang/Long;" {
                let c_idx = cf.cp.add_class("java/lang/Long");
                code.checkcast(c_idx);
            }
            let m = cf.cp.add_methodref("java/lang/Long", "longValue", "()J");
            code.invokevirtual_ret(m, 0, 2);
        }
        (JvmType::Object(act_desc), JvmType::Double) => {
            if act_desc != "Ljava/lang/Double;" {
                let c_idx = cf.cp.add_class("java/lang/Double");
                code.checkcast(c_idx);
            }
            let m = cf.cp.add_methodref("java/lang/Double", "doubleValue", "()D");
            code.invokevirtual_ret(m, 0, 2);
        }
        (JvmType::Object(act_desc), JvmType::Object(exp_desc))
            if act_desc != exp_desc && exp_desc != "Ljava/lang/Object;" =>
        {
            let target_cls = exp_desc.trim_start_matches('L').trim_end_matches(';');
            let c_idx = cf.cp.add_class(target_cls);
            code.checkcast(c_idx);
        }
        _ => {}
    }
}

pub struct Codegen {
    pub classes: Vec<(String, ClassFile)>,
    pub current_package_prefix: String,
    pub current_facade_name: String,
    pub current_ret_type: JvmType,
    pub current_class_info: Option<ClassInfo>,
    pub functions: HashMap<String, FunctionSignature>,
    pub class_infos: HashMap<String, ClassInfo>,
    pub top_level_constants: HashMap<String, (i64, JvmType)>,
    pub list_elem_types: HashMap<String, JvmType>,
    pub last_elem_type: Option<JvmType>,
    pub lambda_counter: usize,
    pub needs_to_int_or_null: bool,
    pub needs_to_long_or_null: bool,
}

impl Codegen {
    pub fn new() -> Self {
        Self {
            classes: Vec::new(),
            current_package_prefix: String::new(),
            current_facade_name: String::new(),
            current_ret_type: JvmType::Void,
            current_class_info: None,
            functions: HashMap::new(),
            class_infos: HashMap::new(),
            top_level_constants: HashMap::new(),
            list_elem_types: HashMap::new(),
            last_elem_type: None,
            lambda_counter: 0,
            needs_to_int_or_null: false,
            needs_to_long_or_null: false,
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

        // Phase 1a: Pre-register class names so types can reference them
        for decl in &file.declarations {
            if let Declaration::Class(c) = decl {
                let internal_name = format!("{}{}", package_prefix, c.name.name);
                self.class_infos.insert(
                    c.name.name.clone(),
                    ClassInfo {
                        internal_name,
                        is_interface: c.kind == ClassKind::Interface,
                        interfaces: vec![],
                        fields: vec![],
                        constructor_params: vec![],
                        methods: HashMap::new(),
                    },
                );
            }
        }

        // Phase 1b: Populate class details, constants, and top-level function signatures
        for decl in &file.declarations {
            match decl {
                Declaration::Class(c) => {
                    let internal_name = format!("{}{}", package_prefix, c.name.name);
                    let mut interfaces = Vec::new();
                    for sup in &c.super_types {
                        if let TypeRef::User { name, .. } = &sup.type_ref {
                            let sname = name.to_string();
                            if let Some(ci) = self.class_infos.get(&sname) {
                                if ci.is_interface {
                                    interfaces.push(ci.internal_name.clone());
                                }
                            }
                        }
                    }

                    let mut fields = Vec::new();
                    let mut constructor_params = Vec::new();
                    if let Some(params) = &c.primary_constructor_params {
                        for p in params {
                            let ty = type_ref_to_jvm(&p.type_ref, &package_prefix, &self.class_infos);
                            fields.push((p.name.name.clone(), ty.clone()));
                            constructor_params.push((p.name.name.clone(), ty, p.default_value.clone()));
                        }
                    }

                    let mut methods = HashMap::new();
                    for inner in &c.declarations {
                        if let Declaration::Function(f) = inner {
                            let pts: Vec<JvmType> = f
                                .value_params
                                .iter()
                                .map(|p| type_ref_to_jvm(&p.type_ref, &package_prefix, &self.class_infos))
                                .collect();
                            let defs: Vec<Option<Expression>> =
                                f.value_params.iter().map(|p| p.default_value.clone()).collect();
                            let rt = f
                                .return_type
                                .as_ref()
                                .map(|tr| type_ref_to_jvm(tr, &package_prefix, &self.class_infos))
                                .unwrap_or(JvmType::Void);
                            methods.insert(
                                f.name.name.clone(),
                                FunctionSignature {
                                    receiver_type: None,
                                    param_types: pts,
                                    param_defaults: defs,
                                    return_type: rt,
                                    is_inline: f.modifiers.contains(&Modifier::Inline),
                                },
                            );
                        }
                    }

                    self.class_infos.insert(
                        c.name.name.clone(),
                        ClassInfo {
                            internal_name,
                            is_interface: c.kind == ClassKind::Interface,
                            interfaces,
                            fields,
                            constructor_params,
                            methods,
                        },
                    );
                }
                Declaration::Property(p) => {
                    if let Some(init) = &p.initializer {
                        if let Some(val) = self.eval_const_int(init) {
                            let ty = p
                                .type_ref
                                .as_ref()
                                .map(|tr| type_ref_to_jvm(tr, &package_prefix, &self.class_infos))
                                .unwrap_or(JvmType::Long);
                            self.top_level_constants.insert(p.name.name.clone(), (val, ty));
                        }
                    }
                }
                Declaration::Function(f) => {
                    let is_main = f.name.name == "main";
                    let sig = if is_main {
                        FunctionSignature {
                            receiver_type: None,
                            param_types: vec![JvmType::Object("[Ljava/lang/String;".to_string())],
                            param_defaults: vec![None],
                            return_type: JvmType::Void,
                            is_inline: false,
                        }
                    } else {
                        let receiver_type = f
                            .receiver_type
                            .as_ref()
                            .map(|r| type_ref_to_jvm(r, &package_prefix, &self.class_infos));
                        let pts: Vec<JvmType> = f
                            .value_params
                            .iter()
                            .map(|p| type_ref_to_jvm(&p.type_ref, &package_prefix, &self.class_infos))
                            .collect();
                        let defs: Vec<Option<Expression>> =
                            f.value_params.iter().map(|p| p.default_value.clone()).collect();
                        let rt = f
                            .return_type
                            .as_ref()
                            .map(|tr| type_ref_to_jvm(tr, &package_prefix, &self.class_infos))
                            .unwrap_or(JvmType::Void);
                        FunctionSignature {
                            receiver_type,
                            param_types: pts,
                            param_defaults: defs,
                            return_type: rt,
                            is_inline: f.modifiers.contains(&Modifier::Inline),
                        }
                    };
                    self.functions.insert(f.name.name.clone(), sig);
                }
            }
        }

        // Phase 2: Compile classes & interfaces
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

        let mut init_code = CodeBuilder::new();
        init_code.aload(0);
        let super_init = facade.cp.add_methodref("java/lang/Object", "<init>", "()V");
        init_code.invokespecial(super_init, 0);
        init_code.return_void();
        facade.add_method(
            0x0001,
            "<init>",
            "()V",
            Some(CodeAttr::new(
                init_code.max_stack.max(1),
                init_code.max_locals.max(1),
                init_code.bytes,
            )),
        );

        for decl in &file.declarations {
            if let Declaration::Function(f) = decl {
                self.compile_function(f, &mut facade)?;
            }
        }

        if self.needs_to_int_or_null {
            emit_safe_parse_helper(&mut facade, "$k3_toIntOrNull", "java/lang/Integer", "Ljava/lang/Integer;");
        }
        if self.needs_to_long_or_null {
            emit_safe_parse_helper(&mut facade, "$k3_toLongOrNull", "java/lang/Long", "Ljava/lang/Long;");
        }

        self.classes.push((facade_class_name, facade));

        let mut results = Vec::new();
        for (name, mut cf) in self.classes.drain(..) {
            results.push((name, cf.to_bytes()));
        }

        Ok(results)
    }

    fn eval_const_int(&self, expr: &Expression) -> Option<i64> {
        match expr {
            Expression::IntegerLiteral(v, _) | Expression::LongLiteral(v, _) => Some(*v),
            Expression::Binary { left, op, right, .. } => {
                let l = self.eval_const_int(left)?;
                let r = self.eval_const_int(right)?;
                match op {
                    BinaryOp::Add => Some(l + r),
                    BinaryOp::Sub => Some(l - r),
                    BinaryOp::Mul => Some(l * r),
                    BinaryOp::Div if r != 0 => Some(l / r),
                    BinaryOp::Shl => Some(l << r),
                    BinaryOp::Shr => Some(l >> r),
                    BinaryOp::BitAnd => Some(l & r),
                    BinaryOp::BitOr => Some(l | r),
                    BinaryOp::BitXor => Some(l ^ r),
                    _ => None,
                }
            }
            Expression::Identifier(id) => self.top_level_constants.get(&id.name).map(|(v, _)| *v),
            _ => None,
        }
    }

    fn compile_class_declaration(
        &mut self,
        c: &ClassDeclaration,
        info: &ClassInfo,
        cf: &mut ClassFile,
    ) -> Result<(), String> {
        if info.is_interface {
            cf.access_flags = 0x0601; // ACC_PUBLIC | ACC_INTERFACE | ACC_ABSTRACT
            for inner in &c.declarations {
                if let Declaration::Function(f) = inner {
                    let sig = info.methods.get(&f.name.name).unwrap();
                    let pdesc: String = sig.param_types.iter().map(|t| t.descriptor()).collect();
                    let mdesc = format!("({}){}", pdesc, sig.return_type.descriptor());
                    cf.add_method(0x0401 /* ACC_PUBLIC | ACC_ABSTRACT */, &f.name.name, &mdesc, None);
                }
            }
            return Ok(());
        }

        cf.access_flags = 0x0001 | 0x0020; // ACC_PUBLIC | ACC_SUPER
        for iface in &info.interfaces {
            cf.add_interface(iface);
        }

        for (fname, fty) in &info.fields {
            cf.add_field(0x0001, fname, &fty.descriptor());
        }

        // Constructor
        let param_desc: String = info
            .constructor_params
            .iter()
            .map(|(_, t, _)| t.descriptor())
            .collect();
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
            init_code.putfield_slots(f_idx, fty.slots());
            param_slot += fty.slots();
        }
        init_code.return_void();

        cf.add_method(
            0x0001,
            "<init>",
            &ctor_desc,
            Some(CodeAttr::new(
                init_code.max_stack.max(3),
                param_slot.max(1),
                init_code.bytes,
            )),
        );

        // Getters
        for (fname, fty) in &info.fields {
            let getter_name = format!("get{}", capitalize_first(fname));
            let getter_desc = format!("(){}", fty.descriptor());
            let mut g_code = CodeBuilder::new();
            g_code.aload(0);
            let f_idx = cf.cp.add_fieldref(&info.internal_name, fname, &fty.descriptor());
            g_code.getfield_slots(f_idx, fty.slots());
            fty.emit_return(&mut g_code);

            cf.add_method(
                0x0001,
                &getter_name,
                &getter_desc,
                Some(CodeAttr::new(1 + fty.slots(), 1, g_code.bytes)),
            );
        }

        // Data class toString()
        if c.kind == ClassKind::DataClass {
            let mut ts = CodeBuilder::new();
            let sb_cls = cf.cp.add_class("java/lang/StringBuilder");
            ts.new_instance(sb_cls);
            ts.dup();
            let sb_init = cf.cp.add_methodref("java/lang/StringBuilder", "<init>", "()V");
            ts.invokespecial(sb_init, 0);
            let head = cf.cp.add_string(&format!("{}(", c.name.name));
            ts.ldc(head);
            let app_s = cf.cp.add_methodref("java/lang/StringBuilder", "append", "(Ljava/lang/String;)Ljava/lang/StringBuilder;");
            ts.invokevirtual(app_s, 1, true);
            for (idx, (fname, fty)) in info.fields.iter().enumerate() {
                let prefix = if idx == 0 {
                    format!("{}=", fname)
                } else {
                    format!(", {}=", fname)
                };
                let p_idx = cf.cp.add_string(&prefix);
                ts.ldc(p_idx);
                ts.invokevirtual(app_s, 1, true);
                ts.aload(0);
                let f_idx = cf.cp.add_fieldref(&info.internal_name, fname, &fty.descriptor());
                ts.getfield_slots(f_idx, fty.slots());
                let (app_desc, slots) = sb_append_desc(fty);
                let app_m = cf.cp.add_methodref("java/lang/StringBuilder", "append", app_desc);
                ts.invokevirtual(app_m, slots, true);
            }
            let tail = cf.cp.add_string(")");
            ts.ldc(tail);
            ts.invokevirtual(app_s, 1, true);
            let to_s = cf.cp.add_methodref("java/lang/StringBuilder", "toString", "()Ljava/lang/String;");
            ts.invokevirtual(to_s, 0, true);
            ts.areturn();
            cf.add_method(
                0x0001,
                "toString",
                "()Ljava/lang/String;",
                Some(CodeAttr::new(ts.max_stack.max(4), 1, ts.bytes)),
            );
        }

        // Member functions
        let prev_cls = self.current_class_info.replace(info.clone());
        for inner in &c.declarations {
            if let Declaration::Function(f) = inner {
                let mut code = CodeBuilder::new();
                let mut locals = HashMap::new();
                locals.insert("this".to_string(), (0, JvmType::Object(format!("L{};", info.internal_name))));
                let mut next_slot = 1u16;
                let mut param_descs = Vec::new();
                for p in &f.value_params {
                    let pty = type_ref_to_jvm(&p.type_ref, &self.current_package_prefix, &self.class_infos);
                    param_descs.push(pty.descriptor());
                    locals.insert(p.name.name.clone(), (next_slot, pty.clone()));
                    next_slot += pty.slots();
                }
                let ret_ty = f
                    .return_type
                    .as_ref()
                    .map(|tr| type_ref_to_jvm(tr, &self.current_package_prefix, &self.class_infos))
                    .unwrap_or(JvmType::Void);
                self.current_ret_type = ret_ty.clone();

                let mut breaks = Vec::new();
                if let Some(body) = &f.body {
                    match body {
                        FunctionBody::Block(stmts) => {
                            let mut has_ret = false;
                            for s in stmts {
                                if matches!(s, Statement::Expression(Expression::Return { .. })) {
                                    has_ret = true;
                                }
                                self.compile_statement(s, &mut code, cf, &mut locals, &mut next_slot, &mut breaks)?;
                            }
                            if !has_ret {
                                ret_ty.emit_return(&mut code);
                            }
                        }
                        FunctionBody::Expression(e) => {
                            let ety = self.compile_expression(e, &mut code, cf, &mut locals, &mut next_slot, &mut breaks)?;
                            emit_unbox_if_needed(&ety, &ret_ty, &mut code, cf);
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
                    Some(CodeAttr::new(
                        code.max_stack.max(4),
                        code.max_locals.max(next_slot).max(1),
                        code.bytes,
                    )),
                );
            }
        }
        self.current_class_info = prev_cls;
        Ok(())
    }

    fn compile_function(&mut self, f: &NamedFunction, cf: &mut ClassFile) -> Result<(), String> {
        let is_main = f.name.name == "main";
        let sig = self.functions.get(&f.name.name).unwrap().clone();
        self.current_ret_type = sig.return_type.clone();
        self.list_elem_types.clear();

        let mut all_param_types = Vec::new();
        if let Some(rec_ty) = &sig.receiver_type {
            all_param_types.push(rec_ty.clone());
        }
        all_param_types.extend(sig.param_types.clone());

        let param_desc: String = all_param_types.iter().map(|t| t.descriptor()).collect();
        let descriptor = format!("({}){}", param_desc, sig.return_type.descriptor());

        let mut code = CodeBuilder::new();
        let mut locals = HashMap::new();
        let mut next_slot = 0u16;

        if is_main {
            locals.insert("args".to_string(), (0, JvmType::Object("[Ljava/lang/String;".to_string())));
            next_slot = 1;
        } else {
            if let Some(rec_ty) = &sig.receiver_type {
                locals.insert("this".to_string(), (next_slot, rec_ty.clone()));
                next_slot += rec_ty.slots();
            }
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
                        if matches!(stmt, Statement::Expression(Expression::Return { .. })) {
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
                    emit_unbox_if_needed(&ty, &sig.return_type, &mut code, cf);
                    sig.return_type.emit_return(&mut code);
                }
            }
        } else {
            sig.return_type.emit_return(&mut code);
        }

        cf.add_method(
            0x0009, // ACC_PUBLIC | ACC_STATIC
            &f.name.name,
            &descriptor,
            Some(CodeAttr::new(
                code.max_stack.max(8),
                code.max_locals.max(next_slot).max(1),
                code.bytes,
            )),
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
            Statement::Declaration(decl) => {
                if let Declaration::Property(p) = decl {
                    let slot = *next_slot;
                    let declared_ty = p
                        .type_ref
                        .as_ref()
                        .map(|tr| type_ref_to_jvm(tr, &self.current_package_prefix, &self.class_infos));
                    if let Some(init) = &p.initializer {
                        self.last_elem_type = None;
                        let init_ty = self.compile_expression(init, code, cf, locals, next_slot, break_targets)?;
                        if let Some(elem_ty) = self.last_elem_type.take() {
                            self.list_elem_types.insert(p.name.name.clone(), elem_ty);
                        }
                        let actual_ty = match declared_ty {
                            Some(d) => {
                                emit_unbox_if_needed(&init_ty, &d, code, cf);
                                d
                            }
                            None => init_ty,
                        };
                        *next_slot = (*next_slot).max(slot + actual_ty.slots());
                        actual_ty.emit_store(code, slot);
                        locals.insert(p.name.name.clone(), (slot, actual_ty));
                    } else {
                        let ty = declared_ty.unwrap_or(JvmType::Int);
                        *next_slot = (*next_slot).max(slot + ty.slots());
                        locals.insert(p.name.name.clone(), (slot, ty));
                    }
                }
            }
            Statement::Assignment { target, op, value, .. } => {
                self.compile_assignment(target, *op, value, code, cf, locals, next_slot, break_targets)?;
            }
            Statement::Expression(expr) => {
                let ty = self.compile_expression(expr, code, cf, locals, next_slot, break_targets)?;
                ty.emit_pop(code);
            }
        }
        Ok(())
    }

    fn compile_assignment(
        &mut self,
        target: &Expression,
        op: AssignOp,
        value: &Expression,
        code: &mut CodeBuilder,
        cf: &mut ClassFile,
        locals: &mut HashMap<String, (u16, JvmType)>,
        next_slot: &mut u16,
        break_targets: &mut Vec<usize>,
    ) -> Result<(), String> {
        match target {
            Expression::Identifier(id) => {
                if let Some((slot, var_ty)) = locals.get(&id.name).cloned() {
                    if op == AssignOp::Assign {
                        let val_ty = self.compile_expression(value, code, cf, locals, next_slot, break_targets)?;
                        emit_unbox_if_needed(&val_ty, &var_ty, code, cf);
                        var_ty.emit_store(code, slot);
                    } else {
                        var_ty.emit_load(code, slot);
                        let val_ty = self.compile_expression(value, code, cf, locals, next_slot, break_targets)?;
                        emit_unbox_if_needed(&val_ty, &var_ty, code, cf);
                        match (&var_ty, op) {
                            (JvmType::Long, AssignOp::PlusAssign) => code.ladd(),
                            (JvmType::Long, AssignOp::MinusAssign) => code.lsub(),
                            (JvmType::Long, AssignOp::MulAssign) => code.lmul(),
                            (JvmType::Long, AssignOp::DivAssign) => code.ldiv(),
                            (JvmType::Long, AssignOp::ModAssign) => code.lrem(),
                            (_, AssignOp::PlusAssign) => code.iadd(),
                            (_, AssignOp::MinusAssign) => code.isub(),
                            (_, AssignOp::MulAssign) => code.imul(),
                            (_, AssignOp::DivAssign) => code.idiv(),
                            (_, AssignOp::ModAssign) => code.irem(),
                            _ => {}
                        }
                        var_ty.emit_store(code, slot);
                    }
                }
            }
            Expression::IndexAccess { target: arr_expr, indices, .. } => {
                if let Some(idx_expr) = indices.first() {
                    let arr_ty = self.compile_expression(arr_expr, code, cf, locals, next_slot, break_targets)?;
                    let _ = self.compile_expression(idx_expr, code, cf, locals, next_slot, break_targets)?;

                    match op {
                        AssignOp::Assign => {
                            let _ = self.compile_expression(value, code, cf, locals, next_slot, break_targets)?;
                            if arr_ty == JvmType::Object("[B".to_string()) {
                                code.bastore();
                            } else if arr_ty == JvmType::Object("[J".to_string()) {
                                code.lastore();
                            } else {
                                code.iastore();
                            }
                        }
                        AssignOp::PlusAssign => {
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
                emit_long_const(code, cf, *val);
                Ok(JvmType::Long)
            }
            Expression::FloatLiteral(f, _) => {
                let idx = cf.cp.add_double(*f);
                code.ldc2_w(idx);
                Ok(JvmType::Double)
            }
            Expression::BooleanLiteral(b, _) => {
                code.iconst(if *b { 1 } else { 0 });
                Ok(JvmType::Boolean)
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
                    let (desc, slots) = sb_append_desc(&ty);
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
                if let Some((slot, ty)) = locals.get("this") {
                    ty.emit_load(code, *slot);
                    Ok(ty.clone())
                } else {
                    code.aload(0);
                    Ok(JvmType::Object("Ljava/lang/Object;".to_string()))
                }
            }
            Expression::Identifier(id) => {
                if let Some((slot, ty)) = locals.get(&id.name).cloned() {
                    ty.emit_load(code, slot);
                    Ok(ty)
                } else if let Some((cval, cty)) = self.top_level_constants.get(&id.name).cloned() {
                    if cty == JvmType::Long {
                        emit_long_const(code, cf, cval);
                        Ok(JvmType::Long)
                    } else {
                        emit_iconst(code, cf, cval as i32);
                        Ok(JvmType::Int)
                    }
                } else if let Some(cinfo) = &self.current_class_info {
                    if let Some((_, fty)) = cinfo.fields.iter().find(|(n, _)| n == &id.name) {
                        code.aload(0);
                        let f_idx = cf.cp.add_fieldref(&cinfo.internal_name, &id.name, &fty.descriptor());
                        code.getfield_slots(f_idx, fty.slots());
                        return Ok(fty.clone());
                    }
                    code.aconst_null();
                    Ok(JvmType::Object("Ljava/lang/Object;".to_string()))
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
                self.compile_binary(left, op, right, code, cf, locals, next_slot, break_targets)
            }
            Expression::Unary { op, expr, is_prefix, .. } => {
                self.compile_unary(*op, expr, *is_prefix, code, cf, locals, next_slot, break_targets)
            }
            Expression::Call { callee, args, trailing_lambda, .. } => {
                self.compile_call(callee, args, trailing_lambda.as_deref(), code, cf, locals, next_slot, break_targets)
            }
            Expression::MemberAccess { target, member, .. } => {
                if let Some(qname) = qualified_chain(expr) {
                    if qname == "java.util.Locale.US" {
                        let f_ref = cf.cp.add_fieldref("java/util/Locale", "US", "Ljava/util/Locale;");
                        code.getstatic(f_ref);
                        return Ok(JvmType::Object("Ljava/util/Locale;".to_string()));
                    }
                }
                let target_ty = self.compile_expression(target, code, cf, locals, next_slot, break_targets)?;
                if let JvmType::Object(ref desc) = target_ty {
                    let internal = desc.trim_start_matches('L').trim_end_matches(';');
                    let simple_name = internal.rsplit('/').next().unwrap_or(internal);
                    if let Some(cinfo) = self.class_infos.get(simple_name).cloned() {
                        if let Some((_, fty)) = cinfo.fields.iter().find(|(n, _)| n == &member.name) {
                            let f_idx = cf.cp.add_fieldref(&cinfo.internal_name, &member.name, &fty.descriptor());
                            code.getfield_slots(f_idx, fty.slots());
                            return Ok(fty.clone());
                        }
                    }
                }
                if member.name == "indices" {
                    return Ok(target_ty);
                }
                if member.name == "size" && target_ty == JvmType::Object("Ljava/util/List;".to_string()) {
                    let m_ref = cf.cp.add_interface_methodref("java/util/List", "size", "()I");
                    code.invokeinterface_ret(m_ref, 0, 1);
                    return Ok(JvmType::Int);
                }
                if member.name == "length" && target_ty == JvmType::Object("Ljava/lang/String;".to_string()) {
                    let m_ref = cf.cp.add_methodref("java/lang/String", "length", "()I");
                    code.invokevirtual_ret(m_ref, 0, 1);
                    return Ok(JvmType::Int);
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
                } else if arr_ty == JvmType::Object("[D".to_string()) {
                    code.pop2();
                    code.dconst_0();
                    Ok(JvmType::Double)
                } else if arr_ty == JvmType::Object("Ljava/util/List;".to_string()) {
                    let m_ref = cf.cp.add_interface_methodref("java/util/List", "get", "(I)Ljava/lang/Object;");
                    code.invokeinterface_ret(m_ref, 1, 1);
                    Ok(JvmType::Object("Ljava/lang/Object;".to_string()))
                } else {
                    code.iaload();
                    Ok(JvmType::Int)
                }
            }
            Expression::For { variable, iterable, body, .. } => {
                self.compile_for(&variable.name, iterable, body, code, cf, locals, next_slot)
            }
            Expression::While { condition, body, is_do_while, .. } => {
                let loop_start = code.bytes.len();
                let mut while_breaks = Vec::new();
                if *is_do_while {
                    let body_ty = self.compile_expression(body, code, cf, locals, next_slot, &mut while_breaks)?;
                    body_ty.emit_pop(code);
                    let _ = self.compile_expression(condition, code, cf, locals, next_slot, break_targets)?;
                    let exit_jump = code.emit_ifeq();
                    code.emit_goto_target(loop_start);
                    code.patch_jump(exit_jump);
                } else {
                    let _ = self.compile_expression(condition, code, cf, locals, next_slot, break_targets)?;
                    let exit_jump = code.emit_ifeq();
                    let body_ty = self.compile_expression(body, code, cf, locals, next_slot, &mut while_breaks)?;
                    body_ty.emit_pop(code);
                    code.emit_goto_target(loop_start);
                    code.patch_jump(exit_jump);
                }
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
                let stack_before = code.stack_depth();
                let then_ty = self.compile_expression(then_branch, code, cf, locals, next_slot, break_targets)?;
                if let Some(else_b) = else_branch {
                    let end_jump = code.emit_goto();
                    code.set_stack_depth(stack_before);
                    code.patch_jump(else_jump);
                    let else_ty = self.compile_expression(else_b, code, cf, locals, next_slot, break_targets)?;
                    code.patch_jump(end_jump);
                    Ok(if then_ty == JvmType::Void { else_ty } else { then_ty })
                } else {
                    then_ty.emit_pop(code);
                    code.patch_jump(else_jump);
                    Ok(JvmType::Void)
                }
            }
            Expression::When { subject, entries, .. } => {
                self.compile_when(subject.as_deref(), entries, code, cf, locals, next_slot, break_targets)
            }
            Expression::Block(statements, _) => {
                let mut last_ty = JvmType::Void;
                for (idx, stmt) in statements.iter().enumerate() {
                    let is_last = idx + 1 == statements.len();
                    match stmt {
                        Statement::Declaration(_) | Statement::Assignment { .. } => {
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
                    emit_unbox_if_needed(&ty, &self.current_ret_type.clone(), code, cf);
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

    fn compile_binary(
        &mut self,
        left: &Expression,
        op: &BinaryOp,
        right: &Expression,
        code: &mut CodeBuilder,
        cf: &mut ClassFile,
        locals: &mut HashMap<String, (u16, JvmType)>,
        next_slot: &mut u16,
        break_targets: &mut Vec<usize>,
    ) -> Result<JvmType, String> {
        if *op == BinaryOp::Elvis {
            let left_ty = self.compile_expression(left, code, cf, locals, next_slot, break_targets)?;
            code.dup();
            let not_null_jump = code.emit_ifnonnull();
            code.pop();
            let right_diverges = expr_always_diverges(right);
            let right_ty = self.compile_expression(right, code, cf, locals, next_slot, break_targets)?;
            let end_jump = if !right_diverges { Some(code.emit_goto()) } else { None };
            code.patch_jump(not_null_jump);
            let result_ty = match left_ty {
                JvmType::Object(ref d) if d == "Ljava/lang/Integer;" => {
                    let m = cf.cp.add_methodref("java/lang/Integer", "intValue", "()I");
                    code.invokevirtual_ret(m, 0, 1);
                    JvmType::Int
                }
                JvmType::Object(ref d) if d == "Ljava/lang/Long;" => {
                    let m = cf.cp.add_methodref("java/lang/Long", "longValue", "()J");
                    code.invokevirtual_ret(m, 0, 2);
                    JvmType::Long
                }
                other if other != JvmType::Void => other,
                _ => right_ty,
            };
            if let Some(ej) = end_jump {
                code.patch_jump(ej);
            }
            return Ok(result_ty);
        }

        if *op == BinaryOp::And {
            let _ = self.compile_expression(left, code, cf, locals, next_slot, break_targets)?;
            let false_j = code.emit_ifeq();
            let _ = self.compile_expression(right, code, cf, locals, next_slot, break_targets)?;
            let end_j = code.emit_goto();
            code.patch_jump(false_j);
            code.iconst(0);
            code.patch_jump(end_j);
            return Ok(JvmType::Boolean);
        }

        if *op == BinaryOp::Or {
            let _ = self.compile_expression(left, code, cf, locals, next_slot, break_targets)?;
            let true_j = code.emit_ifne();
            let _ = self.compile_expression(right, code, cf, locals, next_slot, break_targets)?;
            let end_j = code.emit_goto();
            code.patch_jump(true_j);
            code.iconst(1);
            code.patch_jump(end_j);
            return Ok(JvmType::Boolean);
        }

        let left_ty = self.compile_expression(left, code, cf, locals, next_slot, break_targets)?;
        let is_shift = matches!(op, BinaryOp::Shl | BinaryOp::Shr | BinaryOp::Ushr);
        let right_ty = self.compile_expression(right, code, cf, locals, next_slot, break_targets)?;

        if *op == BinaryOp::Add
            && (left_ty == JvmType::Object("Ljava/lang/String;".to_string())
                || right_ty == JvmType::Object("Ljava/lang/String;".to_string()))
        {
            // Store right, wrap StringBuilder around left, append right
            let r_slot = *next_slot;
            *next_slot += right_ty.slots();
            right_ty.emit_store(code, r_slot);

            let l_slot = *next_slot;
            *next_slot += left_ty.slots();
            left_ty.emit_store(code, l_slot);

            let sb_cls = cf.cp.add_class("java/lang/StringBuilder");
            code.new_instance(sb_cls);
            code.dup();
            let sb_init = cf.cp.add_methodref("java/lang/StringBuilder", "<init>", "()V");
            code.invokespecial(sb_init, 0);

            left_ty.emit_load(code, l_slot);
            let (ldesc, lslots) = sb_append_desc(&left_ty);
            let app_l = cf.cp.add_methodref("java/lang/StringBuilder", "append", ldesc);
            code.invokevirtual(app_l, lslots, true);

            right_ty.emit_load(code, r_slot);
            let (rdesc, rslots) = sb_append_desc(&right_ty);
            let app_r = cf.cp.add_methodref("java/lang/StringBuilder", "append", rdesc);
            code.invokevirtual(app_r, rslots, true);

            let to_s = cf.cp.add_methodref("java/lang/StringBuilder", "toString", "()Ljava/lang/String;");
            code.invokevirtual(to_s, 0, true);
            return Ok(JvmType::Object("Ljava/lang/String;".to_string()));
        }

        if is_shift {
            if right_ty == JvmType::Long {
                code.l2i();
            } else if right_ty == JvmType::Double {
                code.d2i();
            }
            return if left_ty == JvmType::Long {
                match op {
                    BinaryOp::Shl => code.lshl(),
                    BinaryOp::Shr => code.lshr(),
                    BinaryOp::Ushr => code.lushr(),
                    _ => unreachable!(),
                }
                Ok(JvmType::Long)
            } else {
                match op {
                    BinaryOp::Shl => code.ishl(),
                    BinaryOp::Shr => code.ishr(),
                    BinaryOp::Ushr => code.iushr(),
                    _ => unreachable!(),
                }
                Ok(JvmType::Int)
            };
        }

        let target_ty = if left_ty == JvmType::Double || right_ty == JvmType::Double {
            JvmType::Double
        } else if left_ty == JvmType::Long || right_ty == JvmType::Long {
            JvmType::Long
        } else {
            JvmType::Int
        };

        if left_ty != target_ty && !(left_ty.is_integral_or_bool() && target_ty == JvmType::Int) {
            let temp_slot = *next_slot;
            *next_slot += right_ty.slots();
            right_ty.emit_store(code, temp_slot);
            emit_unbox_if_needed(&left_ty, &target_ty, code, cf);
            right_ty.emit_load(code, temp_slot);
            emit_unbox_if_needed(&right_ty, &target_ty, code, cf);
        } else if right_ty != target_ty && !(right_ty.is_integral_or_bool() && target_ty == JvmType::Int) {
            emit_unbox_if_needed(&right_ty, &target_ty, code, cf);
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
                    emit_bool_result(code, jump);
                    Ok(JvmType::Boolean)
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
                    emit_bool_result(code, jump);
                    Ok(JvmType::Boolean)
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
                    emit_bool_result(code, jump);
                    Ok(JvmType::Boolean)
                }
                _ => Ok(JvmType::Int),
            },
        }
    }

    fn compile_unary(
        &mut self,
        op: UnaryOp,
        expr: &Expression,
        is_prefix: bool,
        code: &mut CodeBuilder,
        cf: &mut ClassFile,
        locals: &mut HashMap<String, (u16, JvmType)>,
        next_slot: &mut u16,
        break_targets: &mut Vec<usize>,
    ) -> Result<JvmType, String> {
        if matches!(op, UnaryOp::Inc | UnaryOp::Dec) {
            let delta: i8 = if op == UnaryOp::Inc { 1 } else { -1 };
            if let Expression::Identifier(id) = expr {
                if let Some((slot, ty)) = locals.get(&id.name).cloned() {
                    if ty == JvmType::Int {
                        if is_prefix {
                            code.iinc(slot, delta);
                            code.iload(slot);
                        } else {
                            code.iload(slot);
                            code.iinc(slot, delta);
                        }
                        return Ok(JvmType::Int);
                    } else if ty == JvmType::Long {
                        if is_prefix {
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
                let jump = code.emit_ifeq();
                emit_bool_result(code, jump);
                Ok(JvmType::Boolean)
            }
            _ => Ok(ty),
        }
    }

    fn compile_when(
        &mut self,
        subject: Option<&Expression>,
        entries: &[WhenEntry],
        code: &mut CodeBuilder,
        cf: &mut ClassFile,
        locals: &mut HashMap<String, (u16, JvmType)>,
        next_slot: &mut u16,
        break_targets: &mut Vec<usize>,
    ) -> Result<JvmType, String> {
        let mut subj_slot = None;
        let mut subj_ty = JvmType::Void;
        let subj_ident = subject.and_then(|s| match s {
            Expression::Identifier(id) => Some(id.name.clone()),
            _ => None,
        });

        if let Some(subj) = subject {
            let ty = self.compile_expression(subj, code, cf, locals, next_slot, break_targets)?;
            let slot = *next_slot;
            *next_slot += ty.slots();
            ty.emit_store(code, slot);
            subj_slot = Some(slot);
            subj_ty = ty;
        }

        let entry_stack = code.stack_depth();
        let mut end_jumps = Vec::new();
        let mut result_ty = JvmType::Void;

        for entry in entries {
            code.set_stack_depth(entry_stack);
            let mut branch_locals = locals.clone();
            let mut branch_next_slot = *next_slot;
            let mut skip_jumps = Vec::new();

            if !entry.conditions.is_empty() {
                let mut hit_jumps = Vec::new();
                let mut pending_casts: Vec<(String, TypeRef)> = Vec::new();
                for cond in &entry.conditions {
                    match cond {
                        WhenCondition::Expression(cond_expr) => {
                            if let Some(sslot) = subj_slot {
                                if matches!(subj_ty, JvmType::Object(_)) {
                                    subj_ty.emit_load(code, sslot);
                                    let cty = self.compile_expression(cond_expr, code, cf, locals, &mut branch_next_slot, break_targets)?;
                                    let _ = emit_box(&cty, code, cf);
                                    let eq_ref = cf.cp.add_methodref(
                                        "java/util/Objects",
                                        "equals",
                                        "(Ljava/lang/Object;Ljava/lang/Object;)Z",
                                    );
                                    code.invokestatic_ret(eq_ref, 2, 1);
                                    hit_jumps.push(code.emit_ifne());
                                } else if subj_ty == JvmType::Long {
                                    subj_ty.emit_load(code, sslot);
                                    let cty = self.compile_expression(cond_expr, code, cf, locals, &mut branch_next_slot, break_targets)?;
                                    emit_unbox_if_needed(&cty, &JvmType::Long, code, cf);
                                    code.lcmp();
                                    hit_jumps.push(code.emit_ifeq());
                                } else {
                                    subj_ty.emit_load(code, sslot);
                                    let _ = self.compile_expression(cond_expr, code, cf, locals, &mut branch_next_slot, break_targets)?;
                                    hit_jumps.push(code.emit_if_icmpeq());
                                }
                            } else {
                                let _ = self.compile_expression(cond_expr, code, cf, locals, &mut branch_next_slot, break_targets)?;
                                hit_jumps.push(code.emit_ifne());
                            }
                        }
                        WhenCondition::Is(type_ref, is_negated) => {
                            if let Some(sslot) = subj_slot {
                                let target_cls = type_ref_to_internal_class(type_ref, &self.current_package_prefix, &self.class_infos);
                                let cls_idx = cf.cp.add_class(&target_cls);
                                if matches!(subj_ty, JvmType::Object(_)) {
                                    subj_ty.emit_load(code, sslot);
                                    code.instanceof(cls_idx);
                                    let j = if *is_negated { code.emit_ifeq() } else { code.emit_ifne() };
                                    hit_jumps.push(j);

                                    if !is_negated && entry.conditions.len() == 1 {
                                        if let Some(var_name) = &subj_ident {
                                            pending_casts.push((var_name.clone(), type_ref.clone()));
                                        }
                                    }
                                } else {
                                    code.iconst(if *is_negated { 0 } else { 1 });
                                    hit_jumps.push(code.emit_ifne());
                                }
                            }
                        }
                        WhenCondition::In(range_expr, is_negated) => {
                            if let (Some(sslot), Expression::Binary { left, op, right, .. }) = (subj_slot, range_expr) {
                                if matches!(op, BinaryOp::Range | BinaryOp::RangeExclusive | BinaryOp::Until) {
                                    subj_ty.emit_load(code, sslot);
                                    let _ = self.compile_expression(left, code, cf, locals, &mut branch_next_slot, break_targets)?;
                                    let fail_low = code.emit_if_icmplt();
                                    subj_ty.emit_load(code, sslot);
                                    let _ = self.compile_expression(right, code, cf, locals, &mut branch_next_slot, break_targets)?;
                                    let fail_high = if *op == BinaryOp::Range {
                                        code.emit_if_icmpgt()
                                    } else {
                                        code.emit_if_icmpge()
                                    };
                                    if *is_negated {
                                        let skip_hit = code.emit_goto();
                                        code.patch_jump(fail_low);
                                        code.patch_jump(fail_high);
                                        hit_jumps.push(code.emit_goto());
                                        code.patch_jump(skip_hit);
                                    } else {
                                        hit_jumps.push(code.emit_goto());
                                        code.patch_jump(fail_low);
                                        code.patch_jump(fail_high);
                                    }
                                }
                            }
                        }
                    }
                }
                let miss_jump = code.emit_goto();
                skip_jumps.push(miss_jump);
                for h in hit_jumps {
                    code.patch_jump(h);
                }
                if let Some(sslot) = subj_slot {
                    for (var_name, type_ref) in pending_casts {
                        let cast_ty = type_ref_to_jvm(&type_ref, &self.current_package_prefix, &self.class_infos);
                        let cast_slot = branch_next_slot;
                        branch_next_slot += cast_ty.slots();
                        subj_ty.emit_load(code, sslot);
                        emit_unbox_if_needed(&subj_ty, &cast_ty, code, cf);
                        cast_ty.emit_store(code, cast_slot);
                        branch_locals.insert(var_name, (cast_slot, cast_ty));
                    }
                }
            }

            let bty = self.compile_expression(&entry.body, code, cf, &mut branch_locals, &mut branch_next_slot, break_targets)?;
            *next_slot = (*next_slot).max(branch_next_slot);
            if result_ty == JvmType::Void && bty != JvmType::Void {
                result_ty = bty;
            }
            end_jumps.push(code.emit_goto());
            for sj in skip_jumps {
                code.patch_jump(sj);
            }
        }

        if result_ty != JvmType::Void {
            code.set_stack_depth(entry_stack + result_ty.slots());
        } else {
            code.set_stack_depth(entry_stack);
        }
        for ej in end_jumps {
            code.patch_jump(ej);
        }
        Ok(result_ty)
    }

    fn compile_for(
        &mut self,
        var_name: &str,
        iterable: &Expression,
        body: &Expression,
        code: &mut CodeBuilder,
        cf: &mut ClassFile,
        locals: &mut HashMap<String, (u16, JvmType)>,
        next_slot: &mut u16,
    ) -> Result<JvmType, String> {
        if let Expression::Binary { left, op, right, .. } = iterable {
            if matches!(op, BinaryOp::Until | BinaryOp::RangeExclusive | BinaryOp::Range) {
                let is_until = *op == BinaryOp::Until || *op == BinaryOp::RangeExclusive;
                let var_slot = *next_slot;
                *next_slot += 1;
                locals.insert(var_name.to_string(), (var_slot, JvmType::Int));

                let mut dummy_breaks = Vec::new();
                self.compile_expression(left, code, cf, locals, next_slot, &mut dummy_breaks)?;
                code.istore(var_slot);

                let max_slot = *next_slot;
                *next_slot += 1;
                self.compile_expression(right, code, cf, locals, next_slot, &mut dummy_breaks)?;
                code.istore(max_slot);

                let loop_start = code.bytes.len();
                code.iload(var_slot);
                code.iload(max_slot);
                let exit_jump = if is_until { code.emit_if_icmpge() } else { code.emit_if_icmpgt() };

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
        }

        if let Expression::MemberAccess { target, member, .. } = iterable {
            if member.name == "indices" {
                let var_slot = *next_slot;
                *next_slot += 1;
                locals.insert(var_name.to_string(), (var_slot, JvmType::Int));
                code.iconst(0);
                code.istore(var_slot);

                let max_slot = *next_slot;
                *next_slot += 1;
                let mut dummy_breaks = Vec::new();
                self.compile_expression(target, code, cf, locals, next_slot, &mut dummy_breaks)?;
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
        }

        let elem_hint = match iterable {
            Expression::Identifier(id) => self.list_elem_types.get(&id.name).cloned(),
            _ => None,
        };
        let mut dummy_breaks = Vec::new();
        let iter_ty = self.compile_expression(iterable, code, cf, locals, next_slot, &mut dummy_breaks)?;

        if matches!(&iter_ty, JvmType::Object(d) if d.starts_with('[')) {
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

            let elem_ty = match &iter_ty {
                JvmType::Object(d) if d == "[J" => JvmType::Long,
                JvmType::Object(d) if d == "[I" || d == "[B" => JvmType::Int,
                _ => JvmType::Object("Ljava/lang/Object;".to_string()),
            };
            let val_slot = *next_slot;
            *next_slot += elem_ty.slots();
            locals.insert(var_name.to_string(), (val_slot, elem_ty.clone()));

            let loop_start = code.bytes.len();
            code.iload(idx_slot);
            code.iload(len_slot);
            let exit_jump = code.emit_if_icmpge();

            code.aload(arr_slot);
            code.iload(idx_slot);
            if elem_ty == JvmType::Long {
                code.laload();
                code.lstore(val_slot);
            } else if iter_ty == JvmType::Object("[B".to_string()) {
                code.baload();
                code.istore(val_slot);
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

        // Collection/Iterable loop via java.util.Iterator
        let iter_cls = cf.cp.add_class("java/lang/Iterable");
        code.checkcast(iter_cls);
        let iter_m = cf.cp.add_interface_methodref("java/lang/Iterable", "iterator", "()Ljava/util/Iterator;");
        code.invokeinterface_ret(iter_m, 0, 1);
        let iter_slot = *next_slot;
        *next_slot += 1;
        code.astore(iter_slot);

        let item_ty = elem_hint.unwrap_or_else(|| JvmType::Object("Ljava/lang/Object;".to_string()));
        let var_slot = *next_slot;
        *next_slot += item_ty.slots();
        locals.insert(var_name.to_string(), (var_slot, item_ty.clone()));

        let loop_start = code.bytes.len();
        code.aload(iter_slot);
        let has_next_m = cf.cp.add_interface_methodref("java/util/Iterator", "hasNext", "()Z");
        code.invokeinterface_ret(has_next_m, 0, 1);
        let exit_jump = code.emit_ifeq();

        code.aload(iter_slot);
        let next_m = cf.cp.add_interface_methodref("java/util/Iterator", "next", "()Ljava/lang/Object;");
        code.invokeinterface_ret(next_m, 0, 1);
        emit_unbox_if_needed(&JvmType::Object("Ljava/lang/Object;".to_string()), &item_ty, code, cf);
        item_ty.emit_store(code, var_slot);

        let mut loop_breaks = Vec::new();
        let body_ty = self.compile_expression(body, code, cf, locals, next_slot, &mut loop_breaks)?;
        body_ty.emit_pop(code);

        code.emit_goto_target(loop_start);
        code.patch_jump(exit_jump);
        for b in loop_breaks {
            code.patch_jump(b);
        }
        Ok(JvmType::Void)
    }

    fn compile_call(
        &mut self,
        callee: &Expression,
        args: &[ValueArgument],
        trailing_lambda: Option<&Expression>,
        code: &mut CodeBuilder,
        cf: &mut ClassFile,
        locals: &mut HashMap<String, (u16, JvmType)>,
        next_slot: &mut u16,
        break_targets: &mut Vec<usize>,
    ) -> Result<JvmType, String> {
        if let Expression::Identifier(id) = callee {
            if let Some(cinfo) = self.class_infos.get(&id.name).cloned() {
                let c_idx = cf.cp.add_class(&cinfo.internal_name);
                code.new_instance(c_idx);
                code.dup();

                let mut total_slots = 0u16;
                for (idx, (_, expected_ty, default_val)) in cinfo.constructor_params.iter().enumerate() {
                    if let Some(arg) = args.get(idx) {
                        let arg_ty = self.compile_expression(&arg.value, code, cf, locals, next_slot, break_targets)?;
                        emit_unbox_if_needed(&arg_ty, expected_ty, code, cf);
                    } else if let Some(def_expr) = default_val {
                        let def_ty = self.compile_expression(def_expr, code, cf, locals, next_slot, break_targets)?;
                        emit_unbox_if_needed(&def_ty, expected_ty, code, cf);
                    } else {
                        code.aconst_null();
                    }
                    total_slots += expected_ty.slots();
                }

                let p_desc: String = cinfo.constructor_params.iter().map(|(_, t, _)| t.descriptor()).collect();
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
                    if let Some(lambda_expr) = trailing_lambda {
                        let size_slot = *next_slot;
                        *next_slot += 1;
                        code.dup();
                        code.istore(size_slot);
                        code.newarray(8); // T_BYTE
                        let arr_slot = *next_slot;
                        *next_slot += 1;
                        code.astore(arr_slot);

                        let idx_slot = *next_slot;
                        *next_slot += 1;
                        code.iconst(0);
                        code.istore(idx_slot);

                        let loop_start = code.bytes.len();
                        code.iload(idx_slot);
                        code.iload(size_slot);
                        let exit_j = code.emit_if_icmpge();

                        code.aload(arr_slot);
                        code.iload(idx_slot);
                        let mut lambda_locals = locals.clone();
                        lambda_locals.insert("it".to_string(), (idx_slot, JvmType::Int));
                        let mut lambda_next = *next_slot;
                        let val_ty = self.compile_expression(lambda_expr, code, cf, &mut lambda_locals, &mut lambda_next, break_targets)?;
                        *next_slot = (*next_slot).max(lambda_next);
                        if val_ty == JvmType::Long {
                            code.l2i();
                        }
                        code.i2b();
                        code.bastore();

                        code.iinc(idx_slot, 1);
                        code.emit_goto_target(loop_start);
                        code.patch_jump(exit_j);
                        code.aload(arr_slot);
                    } else {
                        code.newarray(8);
                    }
                    return Ok(JvmType::Object("[B".to_string()));
                }
            } else if id.name == "listOf" || id.name == "mutableListOf" {
                let al_cls = cf.cp.add_class("java/util/ArrayList");
                code.new_instance(al_cls);
                code.dup();
                let al_init = cf.cp.add_methodref("java/util/ArrayList", "<init>", "()V");
                code.invokespecial(al_init, 0);
                let add_m = cf.cp.add_methodref("java/util/ArrayList", "add", "(Ljava/lang/Object;)Z");

                let mut inferred_elem: Option<JvmType> = None;
                for arg in args {
                    code.dup();
                    let aty = self.compile_expression(&arg.value, code, cf, locals, next_slot, break_targets)?;
                    if inferred_elem.is_none() {
                        inferred_elem = Some(aty.clone());
                    }
                    let _ = emit_box(&aty, code, cf);
                    code.invokevirtual_ret(add_m, 1, 1);
                    code.pop();
                }
                self.last_elem_type = inferred_elem;
                return Ok(JvmType::Object("Ljava/util/List;".to_string()));
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
                        JvmType::Boolean => ("(Z)V", 1),
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
                let nano_ref = cf.cp.add_methodref("java/lang/System", "nanoTime", "()J");
                code.invokestatic_ret(nano_ref, 0, 2);
                let start_slot = *next_slot;
                *next_slot += 2;
                code.lstore(start_slot);

                let lambda_expr = trailing_lambda.unwrap();
                let checksum_ty = self.compile_expression(lambda_expr, code, cf, locals, next_slot, break_targets)?;
                let checksum_slot = *next_slot;
                *next_slot += 1;
                if checksum_ty == JvmType::Int {
                    let to_s = cf.cp.add_methodref("java/lang/Integer", "toString", "(I)Ljava/lang/String;");
                    code.invokestatic_ret(to_s, 1, 1);
                } else if checksum_ty == JvmType::Long {
                    let to_s = cf.cp.add_methodref("java/lang/Long", "toString", "(J)Ljava/lang/String;");
                    code.invokestatic_ret(to_s, 2, 1);
                }
                code.astore(checksum_slot);

                code.invokestatic_ret(nano_ref, 0, 2);
                code.lload(start_slot);
                code.lsub();
                code.l2d();
                let cp_1m = cf.cp.add_double(1_000_000.0);
                code.ldc2_w(cp_1m);
                code.ddiv();
                let elapsed_slot = *next_slot;
                *next_slot += 2;
                code.dstore(elapsed_slot);

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

                if let Some(arg) = args.first() {
                    self.compile_expression(&arg.value, code, cf, locals, next_slot, break_targets)?;
                    code.invokevirtual(app_str, 1, true);
                }

                let pipe = cf.cp.add_string("|");
                code.ldc(pipe);
                code.invokevirtual(app_str, 1, true);

                code.aload(checksum_slot);
                code.invokevirtual(app_str, 1, true);

                code.ldc(pipe);
                code.invokevirtual(app_str, 1, true);

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
                    if let Some(arg) = args.get(idx) {
                        let aty = self.compile_expression(&arg.value, code, cf, locals, next_slot, break_targets)?;
                        emit_unbox_if_needed(&aty, pty, code, cf);
                    } else if idx == sig.param_types.len() - 1 && trailing_lambda.is_some() {
                        self.compile_lambda_object(trailing_lambda.unwrap(), None, code, cf)?;
                    } else if let Some(Some(def_expr)) = sig.param_defaults.get(idx) {
                        let dty = self.compile_expression(def_expr, code, cf, locals, next_slot, break_targets)?;
                        emit_unbox_if_needed(&dty, pty, code, cf);
                    } else {
                        code.aconst_null();
                    }
                    total_slots += pty.slots();
                }
                let p_desc: String = sig.param_types.iter().map(|t| t.descriptor()).collect();
                let m_desc = format!("({}){}", p_desc, sig.return_type.descriptor());
                let m_ref = cf.cp.add_methodref(&self.current_facade_name, &id.name, &m_desc);
                code.invokestatic_ret(m_ref, total_slots, sig.return_type.slots());
                return Ok(sig.return_type);
            } else if let Some((fn_slot, fn_ty)) = locals.get(&id.name).cloned() {
                if fn_ty == JvmType::Object("Ljava/util/function/Function;".to_string()) {
                    code.aload(fn_slot);
                    if let Some(arg) = args.first() {
                        let aty = self.compile_expression(&arg.value, code, cf, locals, next_slot, break_targets)?;
                        let _ = emit_box(&aty, code, cf);
                    } else {
                        code.aconst_null();
                    }
                    let apply_m = cf.cp.add_interface_methodref(
                        "java/util/function/Function",
                        "apply",
                        "(Ljava/lang/Object;)Ljava/lang/Object;",
                    );
                    code.invokeinterface_ret(apply_m, 1, 1);
                    let bool_cls = cf.cp.add_class("java/lang/Boolean");
                    code.checkcast(bool_cls);
                    let bv = cf.cp.add_methodref("java/lang/Boolean", "booleanValue", "()Z");
                    code.invokevirtual_ret(bv, 0, 1);
                    return Ok(JvmType::Boolean);
                }
            }
        }

        // Member calls
        if let Expression::MemberAccess { target, member, .. } = callee {
            if let Expression::Identifier(target_id) = &**target {
                if target_id.name == "System" && member.name == "getenv" {
                    if let Some(arg) = args.first() {
                        self.compile_expression(&arg.value, code, cf, locals, next_slot, break_targets)?;
                        let m_ref = cf.cp.add_methodref("java/lang/System", "getenv", "(Ljava/lang/String;)Ljava/lang/String;");
                        code.invokestatic_ret(m_ref, 1, 1);
                        return Ok(JvmType::Object("Ljava/lang/String;".to_string()));
                    }
                } else if target_id.name == "System" && member.name == "nanoTime" {
                    let m_ref = cf.cp.add_methodref("java/lang/System", "nanoTime", "()J");
                    code.invokestatic_ret(m_ref, 0, 2);
                    return Ok(JvmType::Long);
                } else if target_id.name == "Math" && member.name == "floorDiv" && args.len() >= 2 {
                    let a0 = self.compile_expression(&args[0].value, code, cf, locals, next_slot, break_targets)?;
                    emit_unbox_if_needed(&a0, &JvmType::Long, code, cf);
                    let a1 = self.compile_expression(&args[1].value, code, cf, locals, next_slot, break_targets)?;
                    emit_unbox_if_needed(&a1, &JvmType::Long, code, cf);
                    let m_ref = cf.cp.add_methodref("java/lang/Math", "floorDiv", "(JJ)J");
                    code.invokestatic_ret(m_ref, 4, 2);
                    return Ok(JvmType::Long);
                }
            }

            if member.name == "let" {
                let target_ty = self.compile_expression(target, code, cf, locals, next_slot, break_targets)?;
                let it_slot = *next_slot;
                *next_slot += target_ty.slots();
                target_ty.emit_store(code, it_slot);
                let prev_it = locals.insert("it".to_string(), (it_slot, target_ty));
                let res = if let Some(lambda) = trailing_lambda {
                    self.compile_expression(lambda, code, cf, locals, next_slot, break_targets)
                } else if let Some(arg) = args.first() {
                    self.compile_expression(&arg.value, code, cf, locals, next_slot, break_targets)
                } else {
                    Ok(JvmType::Void)
                };
                match prev_it {
                    Some(old) => { locals.insert("it".to_string(), old); }
                    None => { locals.remove("it"); }
                }
                return res;
            }

            if member.name == "format" {
                if args.len() >= 2 {
                    // "<fmt>".format(locale, args...)
                    let loc_ty = self.compile_expression(&args[0].value, code, cf, locals, next_slot, break_targets)?;
                    let _ = loc_ty;
                    let fmt_ty = self.compile_expression(target, code, cf, locals, next_slot, break_targets)?;
                    let _ = fmt_ty;
                    let fmt_args = &args[1..];
                    emit_iconst(code, cf, fmt_args.len() as i32);
                    let obj_cls = cf.cp.add_class("java/lang/Object");
                    code.anewarray(obj_cls);
                    for (i, a) in fmt_args.iter().enumerate() {
                        code.dup();
                        emit_iconst(code, cf, i as i32);
                        let aty = self.compile_expression(&a.value, code, cf, locals, next_slot, break_targets)?;
                        let _ = emit_box(&aty, code, cf);
                        code.aastore();
                    }
                    let m_ref = cf.cp.add_methodref(
                        "java/lang/String",
                        "format",
                        "(Ljava/util/Locale;Ljava/lang/String;[Ljava/lang/Object;)Ljava/lang/String;",
                    );
                    code.invokestatic_ret(m_ref, 3, 1);
                    return Ok(JvmType::Object("Ljava/lang/String;".to_string()));
                } else {
                    let _ = self.compile_expression(target, code, cf, locals, next_slot, break_targets)?;
                    emit_iconst(code, cf, args.len() as i32);
                    let obj_cls = cf.cp.add_class("java/lang/Object");
                    code.anewarray(obj_cls);
                    for (i, a) in args.iter().enumerate() {
                        code.dup();
                        emit_iconst(code, cf, i as i32);
                        let aty = self.compile_expression(&a.value, code, cf, locals, next_slot, break_targets)?;
                        let _ = emit_box(&aty, code, cf);
                        code.aastore();
                    }
                    let m_ref = cf.cp.add_methodref(
                        "java/lang/String",
                        "format",
                        "(Ljava/lang/String;[Ljava/lang/Object;)Ljava/lang/String;",
                    );
                    code.invokestatic_ret(m_ref, 2, 1);
                    return Ok(JvmType::Object("Ljava/lang/String;".to_string()));
                }
            }

            if member.name == "toUInt" {
                self.compile_expression(target, code, cf, locals, next_slot, break_targets)?;
                let to_u = cf.cp.add_methodref("java/lang/Integer", "toUnsignedString", "(I)Ljava/lang/String;");
                code.invokestatic_ret(to_u, 1, 1);
                return Ok(JvmType::Object("Ljava/lang/String;".to_string()));
            } else if member.name == "toInt" {
                let ty = self.compile_expression(target, code, cf, locals, next_slot, break_targets)?;
                if ty == JvmType::Long { code.l2i(); } else if ty == JvmType::Double { code.d2i(); }
                return Ok(JvmType::Int);
            } else if member.name == "toLong" {
                let ty = self.compile_expression(target, code, cf, locals, next_slot, break_targets)?;
                if ty.is_integral_or_bool() { code.i2l(); } else if ty == JvmType::Double { code.d2l(); }
                return Ok(JvmType::Long);
            } else if member.name == "toIntOrNull" {
                self.needs_to_int_or_null = true;
                self.compile_expression(target, code, cf, locals, next_slot, break_targets)?;
                let m = cf.cp.add_methodref(&self.current_facade_name, "$k3_toIntOrNull", "(Ljava/lang/String;)Ljava/lang/Integer;");
                code.invokestatic_ret(m, 1, 1);
                return Ok(JvmType::Object("Ljava/lang/Integer;".to_string()));
            } else if member.name == "toLongOrNull" {
                self.needs_to_long_or_null = true;
                self.compile_expression(target, code, cf, locals, next_slot, break_targets)?;
                let m = cf.cp.add_methodref(&self.current_facade_name, "$k3_toLongOrNull", "(Ljava/lang/String;)Ljava/lang/Long;");
                code.invokestatic_ret(m, 1, 1);
                return Ok(JvmType::Object("Ljava/lang/Long;".to_string()));
            } else if member.name == "toString" {
                let ty = self.compile_expression(target, code, cf, locals, next_slot, break_targets)?;
                match ty {
                    JvmType::Long => {
                        let m = cf.cp.add_methodref("java/lang/Long", "toString", "(J)Ljava/lang/String;");
                        code.invokestatic_ret(m, 2, 1);
                    }
                    JvmType::Int => {
                        let m = cf.cp.add_methodref("java/lang/Integer", "toString", "(I)Ljava/lang/String;");
                        code.invokestatic_ret(m, 1, 1);
                    }
                    JvmType::Double => {
                        let m = cf.cp.add_methodref("java/lang/Double", "toString", "(D)Ljava/lang/String;");
                        code.invokestatic_ret(m, 2, 1);
                    }
                    JvmType::Boolean => {
                        let m = cf.cp.add_methodref("java/lang/Boolean", "toString", "(Z)Ljava/lang/String;");
                        code.invokestatic_ret(m, 1, 1);
                    }
                    _ => {
                        let m = cf.cp.add_methodref("java/lang/Object", "toString", "()Ljava/lang/String;");
                        code.invokevirtual_ret(m, 0, 1);
                    }
                }
                return Ok(JvmType::Object("Ljava/lang/String;".to_string()));
            } else if member.name == "add" {
                let _ = self.compile_expression(target, code, cf, locals, next_slot, break_targets)?;
                if let Some(arg) = args.first() {
                    let aty = self.compile_expression(&arg.value, code, cf, locals, next_slot, break_targets)?;
                    let _ = emit_box(&aty, code, cf);
                } else {
                    code.aconst_null();
                }
                let add_m = cf.cp.add_interface_methodref("java/util/List", "add", "(Ljava/lang/Object;)Z");
                code.invokeinterface_ret(add_m, 1, 1);
                return Ok(JvmType::Boolean);
            } else if member.name == "map" || member.name == "filter" {
                if let Some(lambda_expr) = trailing_lambda {
                    let elem_hint = match &**target {
                        Expression::Identifier(id) => self.list_elem_types.get(&id.name).cloned(),
                        _ => None,
                    };
                    let _ = self.compile_expression(target, code, cf, locals, next_slot, break_targets)?;
                    return self.compile_inline_collection_op(
                        &member.name,
                        elem_hint,
                        lambda_expr,
                        code,
                        cf,
                        locals,
                        next_slot,
                        break_targets,
                    );
                }
            }

            // Extension function call (e.g. numbers.customFilter { ... })
            if let Some(sig) = self.functions.get(&member.name).cloned() {
                if let Some(rec_ty) = &sig.receiver_type {
                    let elem_hint = match &**target {
                        Expression::Identifier(id) => self.list_elem_types.get(&id.name).cloned(),
                        _ => None,
                    };
                    let target_ty = self.compile_expression(target, code, cf, locals, next_slot, break_targets)?;
                    emit_unbox_if_needed(&target_ty, rec_ty, code, cf);
                    let mut total_slots = rec_ty.slots();

                    for (idx, pty) in sig.param_types.iter().enumerate() {
                        if let Some(arg) = args.get(idx) {
                            let aty = self.compile_expression(&arg.value, code, cf, locals, next_slot, break_targets)?;
                            emit_unbox_if_needed(&aty, pty, code, cf);
                        } else if idx == sig.param_types.len() - 1 && trailing_lambda.is_some() {
                            self.compile_lambda_object(trailing_lambda.unwrap(), elem_hint.clone(), code, cf)?;
                        } else if let Some(Some(def_expr)) = sig.param_defaults.get(idx) {
                            let dty = self.compile_expression(def_expr, code, cf, locals, next_slot, break_targets)?;
                            emit_unbox_if_needed(&dty, pty, code, cf);
                        } else {
                            code.aconst_null();
                        }
                        total_slots += pty.slots();
                    }

                    let mut p_desc = rec_ty.descriptor();
                    for t in &sig.param_types {
                        p_desc.push_str(&t.descriptor());
                    }
                    let m_desc = format!("({}){}", p_desc, sig.return_type.descriptor());
                    let m_ref = cf.cp.add_methodref(&self.current_facade_name, &member.name, &m_desc);
                    code.invokestatic_ret(m_ref, total_slots, sig.return_type.slots());
                    self.last_elem_type = elem_hint;
                    return Ok(sig.return_type);
                }
            }

            // Class or interface instance method call
            let target_ty = self.compile_expression(target, code, cf, locals, next_slot, break_targets)?;
            if let JvmType::Object(ref desc) = target_ty {
                let internal = desc.trim_start_matches('L').trim_end_matches(';');
                let simple = internal.rsplit('/').next().unwrap_or(internal);
                if let Some(cinfo) = self.class_infos.get(simple).cloned() {
                    if let Some(sig) = cinfo.methods.get(&member.name).cloned() {
                        let mut arg_slots = 0u16;
                        for (idx, pty) in sig.param_types.iter().enumerate() {
                            if let Some(arg) = args.get(idx) {
                                let aty = self.compile_expression(&arg.value, code, cf, locals, next_slot, break_targets)?;
                                emit_unbox_if_needed(&aty, pty, code, cf);
                            } else if let Some(Some(def_expr)) = sig.param_defaults.get(idx) {
                                let dty = self.compile_expression(def_expr, code, cf, locals, next_slot, break_targets)?;
                                emit_unbox_if_needed(&dty, pty, code, cf);
                            }
                            arg_slots += pty.slots();
                        }
                        let p_desc: String = sig.param_types.iter().map(|t| t.descriptor()).collect();
                        let m_desc = format!("({}){}", p_desc, sig.return_type.descriptor());
                        if cinfo.is_interface {
                            let m_ref = cf.cp.add_interface_methodref(&cinfo.internal_name, &member.name, &m_desc);
                            code.invokeinterface_ret(m_ref, arg_slots, sig.return_type.slots());
                        } else {
                            let m_ref = cf.cp.add_methodref(&cinfo.internal_name, &member.name, &m_desc);
                            code.invokevirtual_ret(m_ref, arg_slots, sig.return_type.slots());
                        }
                        return Ok(sig.return_type);
                    }
                }
            }
        }

        code.aconst_null();
        Ok(JvmType::Object("Ljava/lang/Object;".to_string()))
    }

    fn compile_inline_collection_op(
        &mut self,
        op_name: &str,
        elem_hint: Option<JvmType>,
        lambda_expr: &Expression,
        code: &mut CodeBuilder,
        cf: &mut ClassFile,
        locals: &mut HashMap<String, (u16, JvmType)>,
        next_slot: &mut u16,
        break_targets: &mut Vec<usize>,
    ) -> Result<JvmType, String> {
        let iter_cls = cf.cp.add_class("java/lang/Iterable");
        code.checkcast(iter_cls);
        let iter_m = cf.cp.add_interface_methodref("java/lang/Iterable", "iterator", "()Ljava/util/Iterator;");
        code.invokeinterface_ret(iter_m, 0, 1);
        let iter_slot = *next_slot;
        *next_slot += 1;
        code.astore(iter_slot);

        let al_cls = cf.cp.add_class("java/util/ArrayList");
        code.new_instance(al_cls);
        code.dup();
        let al_init = cf.cp.add_methodref("java/util/ArrayList", "<init>", "()V");
        code.invokespecial(al_init, 0);
        let res_slot = *next_slot;
        *next_slot += 1;
        code.astore(res_slot);

        let item_ty = elem_hint.clone().unwrap_or_else(|| JvmType::Object("Ljava/lang/Object;".to_string()));
        let param_name = match lambda_expr {
            Expression::Lambda { params, .. } if !params.is_empty() => params[0].name.clone(),
            _ => "it".to_string(),
        };

        let loop_start = code.bytes.len();
        code.aload(iter_slot);
        let has_next_m = cf.cp.add_interface_methodref("java/util/Iterator", "hasNext", "()Z");
        code.invokeinterface_ret(has_next_m, 0, 1);
        let exit_jump = code.emit_ifeq();

        code.aload(iter_slot);
        let next_m = cf.cp.add_interface_methodref("java/util/Iterator", "next", "()Ljava/lang/Object;");
        code.invokeinterface_ret(next_m, 0, 1);
        emit_unbox_if_needed(&JvmType::Object("Ljava/lang/Object;".to_string()), &item_ty, code, cf);
        let item_slot = *next_slot;
        *next_slot += item_ty.slots();
        item_ty.emit_store(code, item_slot);

        let mut lambda_locals = locals.clone();
        lambda_locals.insert(param_name, (item_slot, item_ty.clone()));
        let mut lambda_next = *next_slot;

        if op_name == "map" {
            code.aload(res_slot);
            let mapped_ty = self.compile_expression(lambda_expr, code, cf, &mut lambda_locals, &mut lambda_next, break_targets)?;
            let _ = emit_box(&mapped_ty, code, cf);
            let add_m = cf.cp.add_methodref("java/util/ArrayList", "add", "(Ljava/lang/Object;)Z");
            code.invokevirtual_ret(add_m, 1, 1);
            code.pop();
            self.last_elem_type = Some(mapped_ty);
        } else {
            let pred_ty = self.compile_expression(lambda_expr, code, cf, &mut lambda_locals, &mut lambda_next, break_targets)?;
            let _ = pred_ty;
            let skip_add = code.emit_ifeq();
            code.aload(res_slot);
            item_ty.emit_load(code, item_slot);
            let _ = emit_box(&item_ty, code, cf);
            let add_m = cf.cp.add_methodref("java/util/ArrayList", "add", "(Ljava/lang/Object;)Z");
            code.invokevirtual_ret(add_m, 1, 1);
            code.pop();
            code.patch_jump(skip_add);
            self.last_elem_type = elem_hint;
        }
        *next_slot = (*next_slot).max(lambda_next);

        code.emit_goto_target(loop_start);
        code.patch_jump(exit_jump);

        code.aload(res_slot);
        Ok(JvmType::Object("Ljava/util/List;".to_string()))
    }

    fn compile_lambda_object(
        &mut self,
        lambda_expr: &Expression,
        elem_hint: Option<JvmType>,
        code: &mut CodeBuilder,
        cf: &mut ClassFile,
    ) -> Result<(), String> {
        let lambda_class_name = format!("{}$lambda${}", self.current_facade_name, self.lambda_counter);
        self.lambda_counter += 1;

        let mut lcf = ClassFile::new(&lambda_class_name, "java/lang/Object");
        lcf.add_interface("java/util/function/Function");

        let mut init_code = CodeBuilder::new();
        init_code.aload(0);
        let obj_init = lcf.cp.add_methodref("java/lang/Object", "<init>", "()V");
        init_code.invokespecial(obj_init, 0);
        init_code.return_void();
        lcf.add_method(0x0001, "<init>", "()V", Some(CodeAttr::new(1, 1, init_code.bytes)));

        let mut apply_code = CodeBuilder::new();
        let mut l_locals = HashMap::new();
        let mut l_next_slot = 2u16; // 0 = this, 1 = Object arg

        let param_name = match lambda_expr {
            Expression::Lambda { params, .. } if !params.is_empty() => params[0].name.clone(),
            _ => "it".to_string(),
        };

        if let Some(hint) = elem_hint {
            apply_code.aload(1);
            emit_unbox_if_needed(&JvmType::Object("Ljava/lang/Object;".to_string()), &hint, &mut apply_code, &mut lcf);
            let typed_slot = l_next_slot;
            l_next_slot += hint.slots();
            hint.emit_store(&mut apply_code, typed_slot);
            l_locals.insert(param_name, (typed_slot, hint));
        } else {
            l_locals.insert(param_name, (1, JvmType::Object("Ljava/lang/Object;".to_string())));
        }

        let mut dummy_breaks = Vec::new();
        let prev_cls = self.current_class_info.take();
        let ret_ty = self.compile_expression(lambda_expr, &mut apply_code, &mut lcf, &mut l_locals, &mut l_next_slot, &mut dummy_breaks)?;
        self.current_class_info = prev_cls;
        let _ = emit_box(&ret_ty, &mut apply_code, &mut lcf);
        apply_code.areturn();

        lcf.add_method(
            0x0001,
            "apply",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
            Some(CodeAttr::new(
                apply_code.max_stack.max(4),
                apply_code.max_locals.max(l_next_slot).max(2),
                apply_code.bytes,
            )),
        );

        self.classes.push((lambda_class_name.clone(), lcf));

        let c_idx = cf.cp.add_class(&lambda_class_name);
        code.new_instance(c_idx);
        code.dup();
        let ctor = cf.cp.add_methodref(&lambda_class_name, "<init>", "()V");
        code.invokespecial(ctor, 0);
        Ok(())
    }
}

fn emit_long_const(code: &mut CodeBuilder, cf: &mut ClassFile, val: i64) {
    if val == 0 {
        code.lconst_0();
    } else if val == 1 {
        code.lconst_1();
    } else {
        let idx = cf.cp.add_long(val);
        code.ldc2_w(idx);
    }
}

fn emit_bool_result(code: &mut CodeBuilder, true_jump: usize) {
    code.iconst(0);
    let end_j = code.emit_goto();
    code.patch_jump(true_jump);
    code.iconst(1);
    code.patch_jump(end_j);
}

fn expr_always_diverges(expr: &Expression) -> bool {
    match expr {
        Expression::Return { .. } => true,
        Expression::Call { callee, .. } => matches!(&**callee, Expression::Identifier(id) if id.name == "error"),
        _ => false,
    }
}

fn qualified_chain(expr: &Expression) -> Option<String> {
    match expr {
        Expression::Identifier(id) => Some(id.name.clone()),
        Expression::MemberAccess { target, member, .. } => {
            Some(format!("{}.{}", qualified_chain(target)?, member.name))
        }
        _ => None,
    }
}

fn sb_append_desc(ty: &JvmType) -> (&'static str, u16) {
    match ty {
        JvmType::Int => ("(I)Ljava/lang/StringBuilder;", 1),
        JvmType::Boolean => ("(Z)Ljava/lang/StringBuilder;", 1),
        JvmType::Long => ("(J)Ljava/lang/StringBuilder;", 2),
        JvmType::Double => ("(D)Ljava/lang/StringBuilder;", 2),
        JvmType::Float => ("(F)Ljava/lang/StringBuilder;", 1),
        JvmType::Object(d) if d == "Ljava/lang/String;" => ("(Ljava/lang/String;)Ljava/lang/StringBuilder;", 1),
        _ => ("(Ljava/lang/Object;)Ljava/lang/StringBuilder;", 1),
    }
}

fn emit_safe_parse_helper(cf: &mut ClassFile, fn_name: &str, wrapper_cls: &str, wrapper_desc: &str) {
    let mut code = CodeBuilder::new();
    let start_pc = code.bytes.len() as u16;
    code.aload(0);
    let value_of = cf.cp.add_methodref(wrapper_cls, "valueOf", &format!("(Ljava/lang/String;){}", wrapper_desc));
    code.invokestatic_ret(value_of, 1, 1);
    code.areturn();
    let end_pc = code.bytes.len() as u16;
    let handler_pc = code.bytes.len() as u16;
    code.astore(1);
    code.aconst_null();
    code.areturn();

    let nfe_idx = cf.cp.add_class("java/lang/NumberFormatException");
    let mut attr = CodeAttr::new(2, 2, code.bytes);
    attr.exception_table.push(ExceptionHandler {
        start_pc,
        end_pc,
        handler_pc,
        catch_type: nfe_idx,
    });
    cf.add_method(0x000A /* ACC_PRIVATE | ACC_STATIC */, fn_name, &format!("(Ljava/lang/String;){}", wrapper_desc), Some(attr));
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
