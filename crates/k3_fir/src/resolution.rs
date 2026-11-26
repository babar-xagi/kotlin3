use crate::cone_types::{ConeKotlinType, ConeNullability, ConeProjection};
use crate::dfa::DfaState;
use crate::diagnostics::{Diagnostic, DiagnosticCode};
use crate::symbols::{ClassSymbol, FunctionSymbol, PropertySymbol, Scope, VariableSymbol};
use crate::FirResolvePhase;
use k3_syntax::ast::*;
use std::collections::{HashMap, HashSet};

pub struct Resolver {
    pub root_scope: Scope,
    pub diagnostics: Vec<Diagnostic>,
    pub dfa: DfaState,
    pub phase: FirResolvePhase,
    current_return_type: Option<ConeKotlinType>,
    in_suspend_context: bool,
    current_class_type: Option<ConeKotlinType>,
}

impl Resolver {
    pub fn new() -> Self {
        Self {
            root_scope: Scope::root(),
            diagnostics: Vec::new(),
            dfa: DfaState::new(),
            phase: FirResolvePhase::RawFir,
            current_return_type: None,
            in_suspend_context: false,
            current_class_type: None,
        }
    }

    pub fn resolve_file(&mut self, file: &KtFile) {
        self.resolve_files(&[file]);
    }

    pub fn resolve_files(&mut self, files: &[&KtFile]) -> Vec<(usize, Diagnostic)> {
        let mut project_scope = Scope::with_parent(self.root_scope.clone());
        let mut indexed_diags: Vec<(usize, Diagnostic)> = Vec::new();

        // Phase 1: Imports + RawFir/Types — Index declarations across files and register k3 runtime/test imports
        self.phase = FirResolvePhase::Types;
        let mut seen_classes = HashSet::new();
        let mut seen_props = HashSet::new();
        let mut seen_fns: HashSet<(usize, String, Vec<String>)> = HashSet::new();

        for (file_idx, file) in files.iter().enumerate() {
            let start_idx = self.diagnostics.len();
            self.register_known_imports(&file.imports, &mut project_scope);

            for decl in &file.declarations {
                match decl {
                    Declaration::Function(f) => {
                        let return_type = f
                            .return_type
                            .as_ref()
                            .map(|t| self.convert_type_ref(t, &project_scope))
                            .unwrap_or(ConeKotlinType::Unit);

                        let mut value_params = Vec::new();
                        let mut param_sig = Vec::new();
                        let mut required_params = 0usize;
                        let mut has_default = false;
                        for p in &f.value_params {
                            let ty = self.convert_type_ref(&p.type_ref, &project_scope);
                            param_sig.push(ty.to_string());
                            value_params.push((p.name.name.clone(), ty));
                            if p.default_value.is_some() {
                                has_default = true;
                            } else if !has_default {
                                required_params += 1;
                            }
                        }

                        let receiver_type = f
                            .receiver_type
                            .as_ref()
                            .map(|t| self.convert_type_ref(t, &project_scope));
                        let fn_scope_key = if f.name.name == "main" { file_idx } else { 0 };
                        let fn_key = (
                            fn_scope_key,
                            format!(
                                "{}.{}",
                                receiver_type.as_ref().map(|r| r.to_string()).unwrap_or_default(),
                                f.name.name
                            ),
                            param_sig,
                        );
                        if !seen_fns.insert(fn_key) {
                            self.diagnostics.push(Diagnostic::error(
                                DiagnosticCode::DuplicateDeclaration(f.name.name.clone()),
                                f.name.span,
                            ));
                        }

                        let func_sym = FunctionSymbol {
                            name: f.name.name.clone(),
                            type_params: f.type_params.iter().map(|id| id.name.clone()).collect(),
                            receiver_type: receiver_type.clone(),
                            value_params,
                            required_params,
                            is_vararg: false,
                            is_suspend: f.modifiers.contains(&Modifier::Suspend),
                            has_body: f.body.is_some(),
                            return_type,
                            modifiers: f.modifiers.clone(),
                        };

                        if let Some(ConeKotlinType::ClassLike { name: rec_name, .. }) = &receiver_type {
                            let short = rec_name.rsplit('.').next().unwrap_or(rec_name).to_string();
                            if let Some(cls) = project_scope.classes.get_mut(&short) {
                                cls.methods.entry(f.name.name.clone()).or_default().push(func_sym.clone());
                            } else if let Some(mut parent_cls) = self.root_scope.find_class(&short).cloned() {
                                parent_cls.methods.entry(f.name.name.clone()).or_default().push(func_sym.clone());
                                project_scope.insert_class(parent_cls);
                            }
                        }

                        project_scope.insert_function(func_sym);
                    }
                    Declaration::Property(p) => {
                        if !seen_props.insert(p.name.name.clone()) {
                            self.diagnostics.push(Diagnostic::error(
                                DiagnosticCode::DuplicateDeclaration(p.name.name.clone()),
                                p.name.span,
                            ));
                        }
                        let prop_type = p
                            .type_ref
                            .as_ref()
                            .map(|t| self.convert_type_ref(t, &project_scope))
                            .unwrap_or(ConeKotlinType::Unknown);

                        project_scope.insert_variable(VariableSymbol {
                            name: p.name.name.clone(),
                            var_type: prop_type,
                            is_var: p.is_var,
                        });
                    }
                    Declaration::Class(c) => {
                        if !seen_classes.insert(c.name.name.clone()) {
                            self.diagnostics.push(Diagnostic::error(
                                DiagnosticCode::DuplicateDeclaration(c.name.name.clone()),
                                c.name.span,
                            ));
                        }
                        let super_types = c
                            .super_types
                            .iter()
                            .map(|s| self.convert_type_ref(&s.type_ref, &project_scope))
                            .collect();

                        let mut primary_params = Vec::new();
                        let mut properties = HashMap::new();
                        if let Some(params) = &c.primary_constructor_params {
                            for p in params {
                                let ty = self.convert_type_ref(&p.type_ref, &project_scope);
                                primary_params.push((p.name.name.clone(), ty.clone(), p.default_value.is_some()));
                                properties.insert(
                                    p.name.name.clone(),
                                    PropertySymbol {
                                        name: p.name.name.clone(),
                                        prop_type: ty,
                                        is_var: p.is_var,
                                        modifiers: vec![],
                                    },
                                );
                            }
                        }

                        let mut methods = HashMap::new();
                        for inner in &c.declarations {
                            match inner {
                                Declaration::Function(f) => {
                                    let ret = f
                                        .return_type
                                        .as_ref()
                                        .map(|t| self.convert_type_ref(t, &project_scope))
                                        .unwrap_or(ConeKotlinType::Unit);
                                    let mut params = Vec::new();
                                    let mut req = 0usize;
                                    let mut seen_def = false;
                                    for p in &f.value_params {
                                        params.push((
                                            p.name.name.clone(),
                                            self.convert_type_ref(&p.type_ref, &project_scope),
                                        ));
                                        if p.default_value.is_some() {
                                            seen_def = true;
                                        } else if !seen_def {
                                            req += 1;
                                        }
                                    }
                                    methods
                                        .entry(f.name.name.clone())
                                        .or_insert_with(Vec::new)
                                        .push(FunctionSymbol {
                                            name: f.name.name.clone(),
                                            type_params: f
                                                .type_params
                                                .iter()
                                                .map(|id| id.name.clone())
                                                .collect(),
                                            receiver_type: None,
                                            value_params: params,
                                            required_params: req,
                                            is_vararg: false,
                                            is_suspend: f.modifiers.contains(&Modifier::Suspend),
                                            has_body: f.body.is_some(),
                                            return_type: ret,
                                            modifiers: f.modifiers.clone(),
                                        });
                                }
                                Declaration::Property(p) => {
                                    let ptype = p
                                        .type_ref
                                        .as_ref()
                                        .map(|t| self.convert_type_ref(t, &project_scope))
                                        .unwrap_or(ConeKotlinType::Unknown);
                                    properties.insert(
                                        p.name.name.clone(),
                                        PropertySymbol {
                                            name: p.name.name.clone(),
                                            prop_type: ptype,
                                            is_var: p.is_var,
                                            modifiers: p.modifiers.clone(),
                                        },
                                    );
                                }
                                _ => {}
                            }
                        }

                        project_scope.insert_class(ClassSymbol {
                            name: c.name.name.clone(),
                            kind: c.kind,
                            super_types,
                            primary_params,
                            methods,
                            properties,
                            modifiers: c.modifiers.clone(),
                        });
                    }
                }
            }

            for diag in &self.diagnostics[start_idx..] {
                indexed_diags.push((file_idx, diag.clone()));
            }
        }

        // Phase 2 & 3: BodyResolve + Checkers per file against the shared project scope
        for (file_idx, file) in files.iter().enumerate() {
            let start_idx = self.diagnostics.len();
            self.phase = FirResolvePhase::BodyResolve;
            for decl in &file.declarations {
                self.resolve_declaration(decl, &mut project_scope);
            }

            self.phase = FirResolvePhase::Checkers;
            for decl in &file.declarations {
                if let Declaration::Class(c) = decl {
                    if c.kind != ClassKind::Interface && !c.modifiers.contains(&Modifier::Abstract) {
                        if let Some(cls_sym) = project_scope.find_class(&c.name.name).cloned() {
                            for sup in &cls_sym.super_types {
                                if let ConeKotlinType::ClassLike { name: sup_name, .. } = sup {
                                    let short = sup_name.rsplit('.').next().unwrap_or(sup_name);
                                    if let Some(iface_sym) = project_scope.find_class(short) {
                                        if iface_sym.kind == ClassKind::Interface {
                                            for (mname, overloads) in &iface_sym.methods {
                                                if overloads.iter().any(|m| !m.has_body)
                                                    && !cls_sym.methods.contains_key(mname)
                                                {
                                                    self.diagnostics.push(Diagnostic::error(
                                                        DiagnosticCode::AbstractMemberNotImplemented {
                                                            class_name: c.name.name.clone(),
                                                            member: mname.clone(),
                                                        },
                                                        c.name.span,
                                                    ));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            for diag in &self.diagnostics[start_idx..] {
                indexed_diags.push((file_idx, diag.clone()));
            }
        }

        indexed_diags
    }

    fn register_known_imports(&self, imports: &[ImportDirective], scope: &mut Scope) {
        fn insert_rt_class(scope: &mut Scope, name: &str) {
            let mut methods = HashMap::new();
            let mut properties = HashMap::new();
            let mk_fn = |fname: &str, req: usize, total: usize, ret: ConeKotlinType| FunctionSymbol {
                name: fname.to_string(),
                type_params: vec![],
                receiver_type: None,
                value_params: (0..total).map(|i| (format!("p{i}"), ConeKotlinType::any(true))).collect(),
                required_params: req,
                is_vararg: false,
                is_suspend: false,
                has_body: true,
                return_type: ret,
                modifiers: vec![],
            };
            let timed_ty = ConeKotlinType::ClassLike {
                name: "k3.runtime.TimedResult".to_string(),
                type_arguments: vec![],
                nullability: ConeNullability::NotNull,
            };
            match name {
                "K3Env" => {
                    methods.insert("get".into(), vec![mk_fn("get", 1, 2, ConeKotlinType::string())]);
                    methods.insert("int".into(), vec![mk_fn("int", 2, 2, ConeKotlinType::int())]);
                    methods.insert("requirePositiveInt".into(), vec![mk_fn("requirePositiveInt", 2, 2, ConeKotlinType::int())]);
                }
                "K3Time" => {
                    methods.insert("measure".into(), vec![mk_fn("measure", 1, 1, timed_ty.clone())]);
                    methods.insert("formatMs".into(), vec![mk_fn("formatMs", 1, 1, ConeKotlinType::string())]);
                }
                "K3Fs" => {
                    methods.insert("readText".into(), vec![mk_fn("readText", 1, 1, ConeKotlinType::string())]);
                    methods.insert("writeText".into(), vec![mk_fn("writeText", 2, 2, ConeKotlinType::unit())]);
                    methods.insert("exists".into(), vec![mk_fn("exists", 1, 1, ConeKotlinType::boolean())]);
                    methods.insert("listFiles".into(), vec![mk_fn("listFiles", 1, 1, ConeKotlinType::ClassLike { name: "kotlin.collections.List".into(), type_arguments: vec![], nullability: ConeNullability::NotNull })]);
                }
                "K3Process" => {
                    methods.insert("exec".into(), vec![mk_fn("exec", 1, 2, ConeKotlinType::ClassLike { name: "k3.runtime.CommandOutput".into(), type_arguments: vec![], nullability: ConeNullability::NotNull })]);
                }
                "K3Http" => {
                    let resp_ty = ConeKotlinType::ClassLike { name: "k3.runtime.HttpResponseData".into(), type_arguments: vec![], nullability: ConeNullability::NotNull };
                    methods.insert("get".into(), vec![mk_fn("get", 1, 1, resp_ty.clone())]);
                    methods.insert("post".into(), vec![mk_fn("post", 2, 3, resp_ty)]);
                }
                "K3Async" => {
                    methods.insert("parallel".into(), vec![mk_fn("parallel", 1, 1, ConeKotlinType::ClassLike { name: "kotlin.collections.List".into(), type_arguments: vec![], nullability: ConeNullability::NotNull })]);
                }
                "K3Json" => {
                    methods.insert("stringify".into(), vec![mk_fn("stringify", 1, 1, ConeKotlinType::string())]);
                    methods.insert("parse".into(), vec![mk_fn("parse", 1, 1, ConeKotlinType::any(true))]);
                }
                "TimedResult" => {
                    properties.insert("value".into(), PropertySymbol { name: "value".into(), prop_type: ConeKotlinType::any(true), is_var: false, modifiers: vec![] });
                    properties.insert("elapsedMs".into(), PropertySymbol { name: "elapsedMs".into(), prop_type: ConeKotlinType::double(), is_var: false, modifiers: vec![] });
                }
                "CommandOutput" => {
                    properties.insert("exitCode".into(), PropertySymbol { name: "exitCode".into(), prop_type: ConeKotlinType::int(), is_var: false, modifiers: vec![] });
                    properties.insert("stdout".into(), PropertySymbol { name: "stdout".into(), prop_type: ConeKotlinType::string(), is_var: false, modifiers: vec![] });
                    properties.insert("stderr".into(), PropertySymbol { name: "stderr".into(), prop_type: ConeKotlinType::string(), is_var: false, modifiers: vec![] });
                }
                "HttpResponseData" => {
                    properties.insert("status".into(), PropertySymbol { name: "status".into(), prop_type: ConeKotlinType::int(), is_var: false, modifiers: vec![] });
                    properties.insert("body".into(), PropertySymbol { name: "body".into(), prop_type: ConeKotlinType::string(), is_var: false, modifiers: vec![] });
                }
                _ => return,
            }
            scope.insert_class(ClassSymbol {
                name: name.to_string(),
                kind: ClassKind::Class,
                super_types: vec![ConeKotlinType::any(false)],
                primary_params: vec![],
                methods,
                properties,
                modifiers: vec![],
            });
        }

        fn insert_test_fn(scope: &mut Scope, name: &str) {
            let (req, total) = match name {
                "test" => (2, 2),
                "runTests" => (1, 2),
                "expectEquals" => (2, 3),
                "expectTrue" => (1, 2),
                "expectThrows" => (1, 1),
                _ => return,
            };
            scope.insert_function(FunctionSymbol {
                name: name.to_string(),
                type_params: vec![],
                receiver_type: None,
                value_params: (0..total).map(|i| (format!("p{i}"), ConeKotlinType::any(true))).collect(),
                required_params: req,
                is_vararg: false,
                is_suspend: false,
                has_body: true,
                return_type: ConeKotlinType::unit(),
                modifiers: vec![],
            });
        }

        for imp in imports {
            let fq = imp.path.to_string();
            if fq == "k3.runtime" && imp.is_star {
                for n in ["K3Env", "K3Time", "K3Fs", "K3Process", "K3Http", "K3Async", "K3Json", "TimedResult", "CommandOutput", "HttpResponseData"] {
                    insert_rt_class(scope, n);
                }
            } else if let Some(name) = fq.strip_prefix("k3.runtime.") {
                insert_rt_class(scope, name);
            } else if fq == "k3.test" && imp.is_star {
                for n in ["test", "runTests", "expectEquals", "expectTrue", "expectThrows"] {
                    insert_test_fn(scope, n);
                }
            } else if let Some(name) = fq.strip_prefix("k3.test.") {
                insert_test_fn(scope, name);
            }
        }
    }

    pub fn resolve_declaration(&mut self, decl: &Declaration, scope: &mut Scope) {
        match decl {
            Declaration::Function(f) => {
                let mut fn_scope = Scope::with_parent(scope.clone());

                if let Some(rec) = &f.receiver_type {
                    let rec_ty = self.convert_type_ref(rec, scope);
                    fn_scope.insert_variable(VariableSymbol {
                        name: "this".to_string(),
                        var_type: rec_ty,
                        is_var: false,
                    });
                } else if let Some(cls_ty) = &self.current_class_type {
                    fn_scope.insert_variable(VariableSymbol {
                        name: "this".to_string(),
                        var_type: cls_ty.clone(),
                        is_var: false,
                    });
                }

                for p in &f.value_params {
                    let param_type = self.convert_type_ref(&p.type_ref, scope);
                    fn_scope.insert_variable(VariableSymbol {
                        name: p.name.name.clone(),
                        var_type: param_type,
                        is_var: false,
                    });
                }

                let expected_return = f
                    .return_type
                    .as_ref()
                    .map(|t| self.convert_type_ref(t, scope))
                    .unwrap_or(ConeKotlinType::Unit);

                let prev_ret = self.current_return_type.replace(expected_return.clone());
                let prev_suspend = self.in_suspend_context;
                self.in_suspend_context = f.modifiers.contains(&Modifier::Suspend);

                if let Some(body) = &f.body {
                    match body {
                        FunctionBody::Block(statements) => {
                            let mut block_scope = Scope::with_parent(fn_scope);
                            for stmt in statements {
                                self.resolve_statement(stmt, &mut block_scope);
                            }
                        }
                        FunctionBody::Expression(expr) => {
                            let expr_type = self.resolve_expression(expr, &mut fn_scope);
                            if f.return_type.is_some() && !expr_type.is_subtype_of(&expected_return) {
                                self.diagnostics.push(Diagnostic::error(
                                    DiagnosticCode::TypeMismatch {
                                        expected: expected_return.to_string(),
                                        actual: expr_type.to_string(),
                                    },
                                    expr.span(),
                                ));
                            }
                        }
                    }
                }

                self.current_return_type = prev_ret;
                self.in_suspend_context = prev_suspend;
            }
            Declaration::Property(p) => {
                let expected_type = p
                    .type_ref
                    .as_ref()
                    .map(|t| self.convert_type_ref(t, scope));

                if let Some(init) = &p.initializer {
                    let init_type = self.resolve_expression(init, scope);
                    if let Some(exp) = expected_type {
                        if !init_type.is_subtype_of(&exp) {
                            self.diagnostics.push(Diagnostic::error(
                                DiagnosticCode::TypeMismatch {
                                    expected: exp.to_string(),
                                    actual: init_type.to_string(),
                                },
                                init.span(),
                            ));
                        }
                    }
                }
            }
            Declaration::Class(c) => {
                let mut class_scope = Scope::with_parent(scope.clone());
                let prev_class = self.current_class_type.replace(ConeKotlinType::ClassLike {
                    name: c.name.name.clone(),
                    type_arguments: vec![],
                    nullability: ConeNullability::NotNull,
                });

                if let Some(params) = &c.primary_constructor_params {
                    for p in params {
                        let ty = self.convert_type_ref(&p.type_ref, scope);
                        class_scope.insert_variable(VariableSymbol {
                            name: p.name.name.clone(),
                            var_type: ty,
                            is_var: p.is_var,
                        });
                    }
                }
                for inner in &c.declarations {
                    self.resolve_declaration(inner, &mut class_scope);
                }

                self.current_class_type = prev_class;
            }
        }
    }

    pub fn resolve_statement(&mut self, stmt: &Statement, scope: &mut Scope) -> ConeKotlinType {
        match stmt {
            Statement::Declaration(decl) => {
                self.resolve_declaration(decl, scope);
                if let Declaration::Property(p) = decl {
                    if scope.variables.contains_key(&p.name.name) {
                        self.diagnostics.push(Diagnostic::error(
                            DiagnosticCode::DuplicateDeclaration(p.name.name.clone()),
                            p.name.span,
                        ));
                    }
                    let ty = p
                        .type_ref
                        .as_ref()
                        .map(|t| self.convert_type_ref(t, scope))
                        .unwrap_or_else(|| {
                            if let Some(init) = &p.initializer {
                                self.resolve_expression(init, scope)
                            } else {
                                ConeKotlinType::Unknown
                            }
                        });

                    scope.insert_variable(VariableSymbol {
                        name: p.name.name.clone(),
                        var_type: ty,
                        is_var: p.is_var,
                    });
                }
                ConeKotlinType::unit()
            }
            Statement::Assignment {
                target,
                op: _,
                value,
                span: _,
            } => {
                let target_type = self.resolve_expression(target, scope);
                let value_type = self.resolve_expression(value, scope);

                if let Expression::Identifier(id) = target {
                    if let Some(var) = scope.find_variable(&id.name) {
                        if !var.is_var {
                            self.diagnostics.push(Diagnostic::error(
                                DiagnosticCode::ValReassignment(id.name.clone()),
                                id.span,
                            ));
                        }
                    }
                }

                if !value_type.is_subtype_of(&target_type) {
                    self.diagnostics.push(Diagnostic::error(
                        DiagnosticCode::TypeMismatch {
                            expected: target_type.to_string(),
                            actual: value_type.to_string(),
                        },
                        value.span(),
                    ));
                }
                ConeKotlinType::unit()
            }
            Statement::Expression(expr) => self.resolve_expression(expr, scope),
        }
    }

    pub fn resolve_expression(&mut self, expr: &Expression, scope: &mut Scope) -> ConeKotlinType {
        match expr {
            Expression::IntegerLiteral(_, _) => ConeKotlinType::int(),
            Expression::LongLiteral(_, _) => ConeKotlinType::long(),
            Expression::FloatLiteral(_, _) => ConeKotlinType::double(),
            Expression::BooleanLiteral(_, _) => ConeKotlinType::boolean(),
            Expression::StringLiteral(_, _) => ConeKotlinType::string(),
            Expression::StringTemplate(parts, _) => {
                for part in parts {
                    self.resolve_expression(part, scope);
                }
                ConeKotlinType::string()
            }
            Expression::NullLiteral(_) => ConeKotlinType::nothing().make_nullable(),
            Expression::This(_) => scope
                .find_variable("this")
                .map(|v| v.var_type.clone())
                .unwrap_or_else(|| ConeKotlinType::any(false)),
            Expression::Super(_) => ConeKotlinType::any(false),
            Expression::Identifier(id) => {
                if let Some(smart_type) = self.dfa.get_smart_cast(&id.name) {
                    return smart_type.clone();
                }

                if let Some(var) = scope.find_variable(&id.name) {
                    var.var_type.clone()
                } else if let Some(c) = scope.find_class(&id.name) {
                    ConeKotlinType::ClassLike {
                        name: c.name.clone(),
                        type_arguments: vec![],
                        nullability: ConeNullability::NotNull,
                    }
                } else if scope.find_function(&id.name).is_some() {
                    ConeKotlinType::any(false)
                } else if matches!(
                    id.name.as_str(),
                    "System" | "Math" | "java" | "require" | "error" | "check" | "IntArray" | "ByteArray" | "LongArray"
                ) {
                    ConeKotlinType::ClassLike {
                        name: id.name.clone(),
                        type_arguments: vec![],
                        nullability: ConeNullability::NotNull,
                    }
                } else {
                    self.diagnostics.push(Diagnostic::error(
                        DiagnosticCode::UnresolvedReference(id.name.clone()),
                        id.span,
                    ));
                    ConeKotlinType::Unknown
                }
            }
            Expression::Binary { left, op, right, .. } => {
                let left_type = self.resolve_expression(left, scope);
                let right_type = self.resolve_expression(right, scope);

                match op {
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
                        if left_type == ConeKotlinType::string() || right_type == ConeKotlinType::string() {
                            ConeKotlinType::string()
                        } else if left_type == ConeKotlinType::double() || right_type == ConeKotlinType::double() {
                            ConeKotlinType::double()
                        } else if left_type == ConeKotlinType::long() || right_type == ConeKotlinType::long() {
                            ConeKotlinType::long()
                        } else {
                            ConeKotlinType::int()
                        }
                    }
                    BinaryOp::Eq
                    | BinaryOp::NotEq
                    | BinaryOp::IdentityEq
                    | BinaryOp::IdentityNotEq
                    | BinaryOp::Less
                    | BinaryOp::LessEq
                    | BinaryOp::Greater
                    | BinaryOp::GreaterEq
                    | BinaryOp::And
                    | BinaryOp::Or
                    | BinaryOp::In
                    | BinaryOp::NotIn
                    | BinaryOp::Is
                    | BinaryOp::NotIs => ConeKotlinType::boolean(),
                    BinaryOp::Elvis => {
                        let non_null_left = left_type.make_not_null();
                        if matches!(right_type, ConeKotlinType::Nothing { .. }) {
                            non_null_left
                        } else if right_type.is_subtype_of(&non_null_left) {
                            non_null_left
                        } else if non_null_left.is_subtype_of(&right_type) {
                            right_type
                        } else {
                            ConeKotlinType::any(right_type.is_nullable())
                        }
                    }
                    BinaryOp::Range | BinaryOp::RangeExclusive | BinaryOp::Until | BinaryOp::DownTo | BinaryOp::Step => {
                        ConeKotlinType::ClassLike {
                            name: "kotlin.ranges.IntRange".to_string(),
                            type_arguments: vec![],
                            nullability: ConeNullability::NotNull,
                        }
                    }
                    BinaryOp::Shl
                    | BinaryOp::Shr
                    | BinaryOp::Ushr
                    | BinaryOp::BitAnd
                    | BinaryOp::BitOr
                    | BinaryOp::BitXor
                    | BinaryOp::Infix(_) => left_type,
                    BinaryOp::As | BinaryOp::AsSafe => right_type,
                }
            }
            Expression::Unary { op, expr: inner, .. } => {
                let inner_type = self.resolve_expression(inner, scope);
                match op {
                    UnaryOp::Not => {
                        if inner_type != ConeKotlinType::boolean() && inner_type != ConeKotlinType::Unknown {
                            self.diagnostics.push(Diagnostic::error(
                                DiagnosticCode::ConditionMustBeBoolean(inner_type.to_string()),
                                inner.span(),
                            ));
                        }
                        ConeKotlinType::boolean()
                    }
                    UnaryOp::Neg | UnaryOp::Pos | UnaryOp::Inc | UnaryOp::Dec => inner_type,
                }
            }
            Expression::Call {
                callee,
                type_args,
                args,
                trailing_lambda,
                span,
            } => {
                let mut arg_types = Vec::new();
                for arg in args {
                    arg_types.push(self.resolve_expression(&arg.value, scope));
                }

                if let Some(lambda) = trailing_lambda {
                    self.resolve_expression(lambda, scope);
                }

                let total_args = args.len() + usize::from(trailing_lambda.is_some());

                if let Expression::Identifier(id) = &**callee {
                    if id.name == "listOf" || id.name == "mutableListOf" {
                        let elem = if let Some(first_targ) = type_args.first() {
                            self.convert_type_ref(first_targ, scope)
                        } else if let Some(first_arg_ty) = arg_types.first() {
                            first_arg_ty.clone()
                        } else {
                            ConeKotlinType::any(true)
                        };
                        let name = if id.name == "listOf" {
                            "kotlin.collections.List"
                        } else {
                            "kotlin.collections.MutableList"
                        };
                        return ConeKotlinType::ClassLike {
                            name: name.to_string(),
                            type_arguments: vec![ConeProjection::Invariant(Box::new(elem))],
                            nullability: ConeNullability::NotNull,
                        };
                    }

                    if let Some(overloads) = scope.find_function(&id.name).cloned() {
                        if let Some(best) = select_overload(&overloads, total_args) {
                            if best.is_suspend && !self.in_suspend_context {
                                self.diagnostics.push(Diagnostic::error(
                                    DiagnosticCode::SuspendCallOutsideCoroutine(id.name.clone()),
                                    *span,
                                ));
                            }
                            return best.return_type.clone();
                        } else if let Some(first) = overloads.first() {
                            self.diagnostics.push(Diagnostic::error(
                                DiagnosticCode::WrongNumberOfArguments {
                                    expected: first.value_params.len(),
                                    actual: total_args,
                                },
                                *span,
                            ));
                            return first.return_type.clone();
                        }
                    } else if let Some(class_sym) = scope.find_class(&id.name).cloned() {
                        let req = class_sym.primary_params.iter().filter(|(_, _, def)| !*def).count();
                        let max = class_sym.primary_params.len();
                        if !class_sym.primary_params.is_empty() && (args.len() < req || args.len() > max) {
                            self.diagnostics.push(Diagnostic::error(
                                DiagnosticCode::WrongNumberOfArguments {
                                    expected: max,
                                    actual: args.len(),
                                },
                                *span,
                            ));
                        }
                        return ConeKotlinType::ClassLike {
                            name: class_sym.name,
                            type_arguments: vec![],
                            nullability: ConeNullability::NotNull,
                        };
                    } else if matches!(id.name.as_str(), "require" | "check") {
                        return ConeKotlinType::unit();
                    } else if id.name == "error" {
                        return ConeKotlinType::nothing();
                    } else if id.name == "IntArray" {
                        return ConeKotlinType::ClassLike {
                            name: "kotlin.IntArray".to_string(),
                            type_arguments: vec![],
                            nullability: ConeNullability::NotNull,
                        };
                    } else if id.name == "ByteArray" {
                        return ConeKotlinType::ClassLike {
                            name: "kotlin.ByteArray".to_string(),
                            type_arguments: vec![],
                            nullability: ConeNullability::NotNull,
                        };
                    } else if let Some(var) = scope.find_variable(&id.name) {
                        return match &var.var_type {
                            ConeKotlinType::ClassLike { name, type_arguments, .. } if name == "kotlin.Function" => {
                                if let Some(ConeProjection::Invariant(ret)) = type_arguments.last() {
                                    (**ret).clone()
                                } else {
                                    ConeKotlinType::boolean()
                                }
                            }
                            ConeKotlinType::ClassLike { name, .. } if name.contains("Function") || name == "Boolean" => {
                                ConeKotlinType::boolean()
                            }
                            other => other.clone(),
                        };
                    } else {
                        self.diagnostics.push(Diagnostic::error(
                            DiagnosticCode::UnresolvedReference(id.name.clone()),
                            id.span,
                        ));
                    }
                } else if let Expression::MemberAccess { target, member, .. } = &**callee {
                    let target_type = self.resolve_expression(target, scope);
                    match member.name.as_str() {
                        "toLong" | "nanoTime" | "floorDiv" => return ConeKotlinType::long(),
                        "toInt" => return ConeKotlinType::int(),
                        "toIntOrNull" => {
                            return ConeKotlinType::ClassLike {
                                name: "kotlin.Int".to_string(),
                                type_arguments: vec![],
                                nullability: ConeNullability::Nullable,
                            };
                        }
                        "toDouble" => return ConeKotlinType::double(),
                        "toString" | "format" => return ConeKotlinType::string(),
                        "getenv" => {
                            return ConeKotlinType::ClassLike {
                                name: "kotlin.String".to_string(),
                                type_arguments: vec![],
                                nullability: ConeNullability::Nullable,
                            };
                        }
                        "toUInt" => {
                            return ConeKotlinType::ClassLike {
                                name: "kotlin.UInt".to_string(),
                                type_arguments: vec![],
                                nullability: ConeNullability::NotNull,
                            };
                        }
                        _ => {}
                    }
                    if let ConeKotlinType::ClassLike { name, .. } = &target_type {
                        let short_name = name.rsplit('.').next().unwrap_or(name);
                        if let Some(c) = scope.find_class(short_name) {
                            if let Some(funcs) = c.methods.get(&member.name) {
                                if let Some(first) = funcs.first() {
                                    return first.return_type.clone();
                                }
                            }
                        }
                    }
                }

                ConeKotlinType::any(true)
            }
            Expression::MemberAccess { target, member, .. } => {
                let target_type = self.resolve_expression(target, scope);
                match member.name.as_str() {
                    "indices" => {
                        return ConeKotlinType::ClassLike {
                            name: "kotlin.ranges.IntRange".to_string(),
                            type_arguments: vec![],
                            nullability: ConeNullability::NotNull,
                        };
                    }
                    "size" | "length" | "count" => return ConeKotlinType::int(),
                    "first" | "second" => return ConeKotlinType::int(),
                    _ => {}
                }
                if let ConeKotlinType::ClassLike { name, .. } = &target_type {
                    let short_name = name.rsplit('.').next().unwrap_or(name);
                    if let Some(c) = scope.find_class(short_name) {
                        if let Some(p) = c.properties.get(&member.name) {
                            return p.prop_type.clone();
                        }
                    }
                }
                ConeKotlinType::any(true)
            }
            Expression::IndexAccess { target, indices, .. } => {
                let target_ty = self.resolve_expression(target, scope);
                for idx in indices {
                    self.resolve_expression(idx, scope);
                }
                match target_ty {
                    ConeKotlinType::ClassLike { name, .. } if name == "kotlin.IntArray" => ConeKotlinType::int(),
                    ConeKotlinType::ClassLike { name, .. } if name == "kotlin.ByteArray" => ConeKotlinType::int(),
                    ConeKotlinType::ClassLike { name, .. } if name == "kotlin.LongArray" => ConeKotlinType::long(),
                    _ => ConeKotlinType::any(true),
                }
            }
            Expression::If { condition, then_branch, else_branch, .. } => {
                let cond_type = self.resolve_expression(condition, scope);
                if cond_type != ConeKotlinType::boolean() && cond_type != ConeKotlinType::Unknown {
                    self.diagnostics.push(Diagnostic::error(
                        DiagnosticCode::ConditionMustBeBoolean(cond_type.to_string()),
                        condition.span(),
                    ));
                }

                let then_type = self.resolve_expression(then_branch, scope);
                if let Some(else_expr) = else_branch {
                    let else_type = self.resolve_expression(else_expr, scope);
                    if then_type == else_type {
                        then_type
                    } else {
                        ConeKotlinType::any(true)
                    }
                } else {
                    ConeKotlinType::unit()
                }
            }
            Expression::When { subject, entries, .. } => {
                let subject_name = if let Some(subj) = subject {
                    self.resolve_expression(subj, scope);
                    if let Expression::Identifier(id) = &**subj {
                        Some(id.name.clone())
                    } else {
                        None
                    }
                } else {
                    None
                };

                let mut result_type = ConeKotlinType::Unknown;

                for entry in entries {
                    let mut entry_scope = Scope::with_parent(scope.clone());
                    let prev_dfa = self.dfa.clone();

                    if let Some(var_name) = &subject_name {
                        for cond in &entry.conditions {
                            if let WhenCondition::Is(ty_ref, false) = cond {
                                let target_type = self.convert_type_ref(ty_ref, scope);
                                self.dfa.record_smart_cast(var_name, target_type);
                            }
                        }
                    }

                    let branch_type = self.resolve_expression(&entry.body, &mut entry_scope);
                    self.dfa = prev_dfa;

                    if result_type == ConeKotlinType::Unknown {
                        result_type = branch_type;
                    } else if result_type != branch_type {
                        result_type = ConeKotlinType::any(true);
                    }
                }

                if result_type == ConeKotlinType::Unknown {
                    ConeKotlinType::unit()
                } else {
                    result_type
                }
            }
            Expression::Block(statements, _) => {
                let mut block_scope = Scope::with_parent(scope.clone());
                let mut last_type = ConeKotlinType::unit();
                for stmt in statements {
                    last_type = self.resolve_statement(stmt, &mut block_scope);
                }
                last_type
            }
            Expression::Lambda { params, body, .. } => {
                let mut lambda_scope = Scope::with_parent(scope.clone());
                let mut param_projections = Vec::new();
                if params.is_empty() {
                    lambda_scope.insert_variable(VariableSymbol {
                        name: "it".to_string(),
                        var_type: ConeKotlinType::int(),
                        is_var: false,
                    });
                    param_projections.push(ConeProjection::Invariant(Box::new(ConeKotlinType::int())));
                } else {
                    for p in params {
                        lambda_scope.insert_variable(VariableSymbol {
                            name: p.name.clone(),
                            var_type: ConeKotlinType::any(true),
                            is_var: false,
                        });
                        param_projections.push(ConeProjection::Invariant(Box::new(ConeKotlinType::any(true))));
                    }
                }
                let mut last_type = ConeKotlinType::unit();
                for stmt in body {
                    last_type = self.resolve_statement(stmt, &mut lambda_scope);
                }
                param_projections.push(ConeProjection::Invariant(Box::new(last_type)));
                ConeKotlinType::ClassLike {
                    name: "kotlin.Function".to_string(),
                    type_arguments: param_projections,
                    nullability: ConeNullability::NotNull,
                }
            }
            Expression::Return { value, span } => {
                let actual = if let Some(val) = value {
                    self.resolve_expression(val, scope)
                } else {
                    ConeKotlinType::unit()
                };
                if let Some(expected) = &self.current_return_type {
                    if !actual.is_subtype_of(expected) {
                        self.diagnostics.push(Diagnostic::error(
                            DiagnosticCode::TypeMismatch {
                                expected: expected.to_string(),
                                actual: actual.to_string(),
                            },
                            *span,
                        ));
                    }
                }
                ConeKotlinType::nothing()
            }
            Expression::Break(_) | Expression::Continue(_) => ConeKotlinType::nothing(),
            Expression::For { variable, iterable, body, .. } => {
                let iter_type = self.resolve_expression(iterable, scope);
                let elem_type = match &iter_type {
                    ConeKotlinType::ClassLike { name, .. } if name.contains("IntRange") => ConeKotlinType::int(),
                    ConeKotlinType::ClassLike { name, .. } if name == "kotlin.IntArray" => ConeKotlinType::int(),
                    ConeKotlinType::ClassLike { name, .. } if name == "kotlin.ByteArray" => ConeKotlinType::int(),
                    ConeKotlinType::ClassLike { name, .. } if name == "kotlin.LongArray" => ConeKotlinType::long(),
                    ConeKotlinType::ClassLike { type_arguments, .. } => {
                        if let Some(ConeProjection::Invariant(inner)) = type_arguments.first() {
                            (**inner).clone()
                        } else {
                            ConeKotlinType::any(true)
                        }
                    }
                    _ => ConeKotlinType::any(true),
                };

                let mut for_scope = Scope::with_parent(scope.clone());
                for_scope.insert_variable(VariableSymbol {
                    name: variable.name.clone(),
                    var_type: elem_type,
                    is_var: false,
                });

                self.resolve_expression(body, &mut for_scope);
                ConeKotlinType::unit()
            }
            Expression::While { condition, body, .. } => {
                let cond_type = self.resolve_expression(condition, scope);
                if cond_type != ConeKotlinType::boolean() && cond_type != ConeKotlinType::Unknown {
                    self.diagnostics.push(Diagnostic::error(
                        DiagnosticCode::ConditionMustBeBoolean(cond_type.to_string()),
                        condition.span(),
                    ));
                }
                self.resolve_expression(body, scope);
                ConeKotlinType::unit()
            }
        }
    }

    pub fn convert_type_ref(&self, type_ref: &TypeRef, _scope: &Scope) -> ConeKotlinType {
        match type_ref {
            TypeRef::User { name, type_args, is_nullable, .. } => {
                let raw_name = name.to_string();
                let fq_name = match raw_name.as_str() {
                    "Int" => "kotlin.Int",
                    "Long" => "kotlin.Long",
                    "Float" => "kotlin.Float",
                    "Double" => "kotlin.Double",
                    "Boolean" => "kotlin.Boolean",
                    "String" => "kotlin.String",
                    "Char" => "kotlin.Char",
                    "Unit" => "kotlin.Unit",
                    "Nothing" => "kotlin.Nothing",
                    "Any" => "kotlin.Any",
                    "List" => "kotlin.collections.List",
                    "MutableList" => "kotlin.collections.MutableList",
                    "Array" => "kotlin.Array",
                    _ => &raw_name,
                };

                let projections = type_args
                    .iter()
                    .map(|t| ConeProjection::Invariant(Box::new(self.convert_type_ref(t, _scope))))
                    .collect();

                ConeKotlinType::ClassLike {
                    name: fq_name.to_string(),
                    type_arguments: projections,
                    nullability: if *is_nullable {
                        ConeNullability::Nullable
                    } else {
                        ConeNullability::NotNull
                    },
                }
            }
            TypeRef::Function { params, return_type, is_nullable, .. } => {
                let mut projs: Vec<ConeProjection> = params
                    .iter()
                    .map(|p| ConeProjection::Invariant(Box::new(self.convert_type_ref(p, _scope))))
                    .collect();
                projs.push(ConeProjection::Invariant(Box::new(
                    self.convert_type_ref(return_type, _scope),
                )));
                ConeKotlinType::ClassLike {
                    name: "kotlin.Function".to_string(),
                    type_arguments: projs,
                    nullability: if *is_nullable {
                        ConeNullability::Nullable
                    } else {
                        ConeNullability::NotNull
                    },
                }
            }
            TypeRef::Nullable { inner, .. } => self.convert_type_ref(inner, _scope).make_nullable(),
        }
    }
}

fn select_overload<'a>(overloads: &'a [FunctionSymbol], arg_count: usize) -> Option<&'a FunctionSymbol> {
    overloads.iter().find(|f| {
        if f.is_vararg {
            arg_count >= f.required_params
        } else {
            arg_count >= f.required_params && arg_count <= f.value_params.len()
        }
    })
}

pub fn analyze(file: &KtFile) -> (Scope, Vec<Diagnostic>) {
    let mut resolver = Resolver::new();
    resolver.resolve_file(file);
    (resolver.root_scope, resolver.diagnostics)
}

pub fn analyze_files(files: &[&KtFile]) -> (Scope, Vec<(usize, Diagnostic)>) {
    let mut resolver = Resolver::new();
    let indexed = resolver.resolve_files(files);
    (resolver.root_scope, indexed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use k3_syntax::lexer::Lexer;
    use k3_syntax::parser::Parser;

    fn parse_code(code: &str) -> KtFile {
        let mut lexer = Lexer::new(code);
        let tokens = lexer.tokenize().expect("lexing failed");
        let mut parser = Parser::new(tokens, code.len() as u32);
        parser.parse_file().expect("parsing failed")
    }

    #[test]
    fn test_valid_types_and_functions() {
        let code = r#"
            fun add(a: Int, b: Int): Int {
                val sum: Int = a + b
                return sum
            }
        "#;
        let file = parse_code(code);
        let (_, diagnostics) = analyze(&file);
        assert_eq!(diagnostics.len(), 0, "expected 0 errors, got: {:?}", diagnostics);
    }

    #[test]
    fn test_val_reassignment_error() {
        let code = r#"
            fun test() {
                val x = 10
                x = 20
            }
        "#;
        let file = parse_code(code);
        let (_, diagnostics) = analyze(&file);
        assert_eq!(diagnostics.len(), 1);
        assert!(matches!(&diagnostics[0].code, DiagnosticCode::ValReassignment(n) if n == "x"));
    }

    #[test]
    fn test_type_mismatch_error() {
        let code = r#"
            fun test(): Int {
                val s: String = 42
                return 0
            }
        "#;
        let file = parse_code(code);
        let (_, diagnostics) = analyze(&file);
        assert_eq!(diagnostics.len(), 1);
        assert!(matches!(&diagnostics[0].code, DiagnosticCode::TypeMismatch { .. }));
    }

    #[test]
    fn test_unresolved_reference_error() {
        let code = r#"
            fun test() {
                println(unknownVariable)
            }
        "#;
        let file = parse_code(code);
        let (_, diagnostics) = analyze(&file);
        assert_eq!(diagnostics.len(), 1);
        assert!(matches!(&diagnostics[0].code, DiagnosticCode::UnresolvedReference(n) if n == "unknownVariable"));
    }

    #[test]
    fn test_dfa_when_smart_cast() {
        let code = r#"
            fun describe(x: Any): String = when (x) {
                is String -> x
                else -> "unknown"
            }
        "#;
        let file = parse_code(code);
        let (_, diagnostics) = analyze(&file);
        assert_eq!(diagnostics.len(), 0, "smart cast should allow returning x as String");
    }

    #[test]
    fn test_wrong_arg_count_and_duplicate_decl() {
        let code = r#"
            fun add(a: Int, b: Int): Int = a + b
            fun dup() {}
            fun dup() {}
            fun main() {
                add(1)
            }
        "#;
        let file = parse_code(code);
        let (_, diagnostics) = analyze(&file);
        assert!(diagnostics.iter().any(|d| matches!(d.code, DiagnosticCode::DuplicateDeclaration(_))));
        assert!(diagnostics.iter().any(|d| matches!(d.code, DiagnosticCode::WrongNumberOfArguments { expected: 2, actual: 1 })));
    }

    #[test]
    fn test_block_return_type_and_suspend_check() {
        let code = r#"
            suspend fun work(): Int = 1
            fun bad(): String {
                work()
                return 42
            }
        "#;
        let file = parse_code(code);
        let (_, diagnostics) = analyze(&file);
        assert!(diagnostics.iter().any(|d| matches!(d.code, DiagnosticCode::SuspendCallOutsideCoroutine(_))));
        assert!(diagnostics.iter().any(|d| matches!(d.code, DiagnosticCode::TypeMismatch { .. })));
    }
}
