use crate::cone_types::{ConeKotlinType, ConeNullability, ConeProjection};
use crate::dfa::DfaState;
use crate::diagnostics::{Diagnostic, DiagnosticCode};
use crate::symbols::{ClassSymbol, FunctionSymbol, PropertySymbol, Scope, VariableSymbol};
use k3_syntax::ast::*;
use std::collections::HashMap;

pub struct Resolver {
    pub root_scope: Scope,
    pub diagnostics: Vec<Diagnostic>,
    pub dfa: DfaState,
}

impl Resolver {
    pub fn new() -> Self {
        Self {
            root_scope: Scope::root(),
            diagnostics: Vec::new(),
            dfa: DfaState::new(),
        }
    }

    pub fn resolve_file(&mut self, file: &KtFile) {
        let mut file_scope = Scope::with_parent(self.root_scope.clone());

        // Phase 1: Index all top-level declarations into file scope
        for decl in &file.declarations {
            match decl {
                Declaration::Function(f) => {
                    let return_type = f
                        .return_type
                        .as_ref()
                        .map(|t| self.convert_type_ref(t, &file_scope))
                        .unwrap_or(ConeKotlinType::Unit);

                    let mut value_params = Vec::new();
                    for p in &f.value_params {
                        let ty = self.convert_type_ref(&p.type_ref, &file_scope);
                        value_params.push((p.name.name.clone(), ty));
                    }

                    let func_sym = FunctionSymbol {
                        name: f.name.name.clone(),
                        type_params: f.type_params.iter().map(|id| id.name.clone()).collect(),
                        receiver_type: f
                            .receiver_type
                            .as_ref()
                            .map(|t| self.convert_type_ref(t, &file_scope)),
                        value_params,
                        return_type,
                        modifiers: f.modifiers.clone(),
                    };
                    file_scope.insert_function(func_sym);
                }
                Declaration::Property(p) => {
                    let prop_type = p
                        .type_ref
                        .as_ref()
                        .map(|t| self.convert_type_ref(t, &file_scope))
                        .unwrap_or(ConeKotlinType::Unknown);

                    file_scope.insert_variable(VariableSymbol {
                        name: p.name.name.clone(),
                        var_type: prop_type,
                        is_var: p.is_var,
                    });
                }
                Declaration::Class(c) => {
                    let super_types = c
                        .super_types
                        .iter()
                        .map(|s| self.convert_type_ref(&s.type_ref, &file_scope))
                        .collect();

                    let mut methods = HashMap::new();
                    let mut properties = HashMap::new();

                    for inner in &c.declarations {
                        match inner {
                            Declaration::Function(f) => {
                                let ret = f
                                    .return_type
                                    .as_ref()
                                    .map(|t| self.convert_type_ref(t, &file_scope))
                                    .unwrap_or(ConeKotlinType::Unit);
                                let mut params = Vec::new();
                                for p in &f.value_params {
                                    params.push((
                                        p.name.name.clone(),
                                        self.convert_type_ref(&p.type_ref, &file_scope),
                                    ));
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
                                        return_type: ret,
                                        modifiers: f.modifiers.clone(),
                                    });
                            }
                            Declaration::Property(p) => {
                                let ptype = p
                                    .type_ref
                                    .as_ref()
                                    .map(|t| self.convert_type_ref(t, &file_scope))
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

                    file_scope.insert_class(ClassSymbol {
                        name: c.name.name.clone(),
                        kind: c.kind,
                        super_types,
                        methods,
                        properties,
                        modifiers: c.modifiers.clone(),
                    });
                }
            }
        }

        // Phase 2: Resolve and type-check declaration bodies
        for decl in &file.declarations {
            self.resolve_declaration(decl, &mut file_scope);
        }
    }

    pub fn resolve_declaration(&mut self, decl: &Declaration, scope: &mut Scope) {
        match decl {
            Declaration::Function(f) => {
                let mut fn_scope = Scope::with_parent(scope.clone());

                // Register parameters in local function scope
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
            }
        }
    }

    pub fn resolve_statement(&mut self, stmt: &Statement, scope: &mut Scope) -> ConeKotlinType {
        match stmt {
            Statement::Declaration(decl) => {
                self.resolve_declaration(decl, scope);
                if let Declaration::Property(p) = decl {
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
            Statement::Expression(expr) => {
                self.resolve_expression(expr, scope)
            }
        }
    }

    pub fn resolve_expression(&mut self, expr: &Expression, scope: &mut Scope) -> ConeKotlinType {
        match expr {
            Expression::IntegerLiteral(_, _) => ConeKotlinType::int(),
            Expression::LongLiteral(_, _) => ConeKotlinType::long(),
            Expression::FloatLiteral(_, _) => ConeKotlinType::double(),
            Expression::BooleanLiteral(_, _) => ConeKotlinType::boolean(),
            Expression::StringLiteral(_, _) => ConeKotlinType::string(),
            Expression::NullLiteral(_) => ConeKotlinType::nothing().make_nullable(),
            Expression::This(_) => ConeKotlinType::any(false),
            Expression::Super(_) => ConeKotlinType::any(false),
            Expression::Identifier(id) => {
                // Check if variable is smart-casted by DFA
                if let Some(smart_type) = self.dfa.get_smart_cast(&id.name) {
                    return smart_type.clone();
                }

                if let Some(var) = scope.find_variable(&id.name) {
                    var.var_type.clone()
                } else if scope.find_function(&id.name).is_some() || scope.find_class(&id.name).is_some() {
                    ConeKotlinType::any(false)
                } else {
                    self.diagnostics.push(Diagnostic::error(
                        DiagnosticCode::UnresolvedReference(id.name.clone()),
                        id.span,
                    ));
                    ConeKotlinType::Unknown
                }
            }
            Expression::Binary {
                left,
                op,
                right,
                span: _,
            } => {
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
                    | BinaryOp::NotIn => ConeKotlinType::boolean(),
                    BinaryOp::Is | BinaryOp::NotIs => ConeKotlinType::boolean(),
                    BinaryOp::Elvis => {
                        // `a ?: b` returns un-nullified `a` or `b`
                        left_type.make_not_null()
                    }
                    BinaryOp::Range | BinaryOp::RangeExclusive => ConeKotlinType::ClassLike {
                        name: "kotlin.ranges.IntRange".to_string(),
                        type_arguments: vec![],
                        nullability: ConeNullability::NotNull,
                    },
                    BinaryOp::As | BinaryOp::AsSafe => right_type,
                }
            }
            Expression::Unary {
                op,
                expr: inner,
                is_prefix: _,
                span: _,
            } => {
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
                type_args: _,
                args,
                trailing_lambda,
                span: _,
            } => {
                let mut arg_types = Vec::new();
                for arg in args {
                    arg_types.push(self.resolve_expression(&arg.value, scope));
                }

                if let Some(lambda) = trailing_lambda {
                    self.resolve_expression(lambda, scope);
                }

                if let Expression::Identifier(id) = &**callee {
                    if let Some(overloads) = scope.find_function(&id.name) {
                        if let Some(best) = overloads.first() {
                            return best.return_type.clone();
                        }
                    } else if let Some(class_sym) = scope.find_class(&id.name) {
                        return ConeKotlinType::ClassLike {
                            name: class_sym.name.clone(),
                            type_arguments: vec![],
                            nullability: ConeNullability::NotNull,
                        };
                    } else if let Some(var) = scope.find_variable(&id.name) {
                        // Function-typed variable or parameter e.g. predicate(item)
                        return match &var.var_type {
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
                } else if let Expression::MemberAccess {
                    target,
                    member,
                    is_safe: _,
                    span: _,
                } = &**callee
                {
                    let target_type = self.resolve_expression(target, scope);
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
            Expression::MemberAccess {
                target,
                member,
                is_safe: _,
                span: _,
            } => {
                let target_type = self.resolve_expression(target, scope);
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
            Expression::IndexAccess {
                target,
                indices,
                span: _,
            } => {
                self.resolve_expression(target, scope);
                for idx in indices {
                    self.resolve_expression(idx, scope);
                }
                ConeKotlinType::any(true)
            }
            Expression::If {
                condition,
                then_branch,
                else_branch,
                span: _,
            } => {
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
            Expression::When {
                subject,
                entries,
                span: _,
            } => {
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

                    // Apply smart-casts from when conditions (e.g. `is String`)
                    if let Some(var_name) = &subject_name {
                        for cond in &entry.conditions {
                            if let WhenCondition::Is(ty_ref, false) = cond {
                                let target_type = self.convert_type_ref(ty_ref, scope);
                                self.dfa.record_smart_cast(var_name, target_type);
                            }
                        }
                    }

                    let branch_type = self.resolve_expression(&entry.body, &mut entry_scope);
                    self.dfa = prev_dfa; // Reset DFA after branch

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
            Expression::Lambda {
                params,
                body,
                span: _,
            } => {
                let mut lambda_scope = Scope::with_parent(scope.clone());
                if params.is_empty() {
                    // Implicit `it` parameter
                    lambda_scope.insert_variable(VariableSymbol {
                        name: "it".to_string(),
                        var_type: ConeKotlinType::int(),
                        is_var: false,
                    });
                } else {
                    for p in params {
                        lambda_scope.insert_variable(VariableSymbol {
                            name: p.name.clone(),
                            var_type: ConeKotlinType::any(true),
                            is_var: false,
                        });
                    }
                }
                for stmt in body {
                    self.resolve_statement(stmt, &mut lambda_scope);
                }
                ConeKotlinType::any(false)
            }
            Expression::Return { value, span: _ } => {
                if let Some(val) = value {
                    self.resolve_expression(val, scope)
                } else {
                    ConeKotlinType::unit()
                }
            }
            Expression::Break(_) | Expression::Continue(_) => ConeKotlinType::nothing(),
            Expression::For {
                variable,
                iterable,
                body,
                span: _,
            } => {
                let iter_type = self.resolve_expression(iterable, scope);
                let elem_type = match iter_type {
                    ConeKotlinType::ClassLike { name, .. } if name.contains("IntRange") => {
                        ConeKotlinType::int()
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
            Expression::While {
                condition,
                body,
                is_do_while: _,
                span: _,
            } => {
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
            TypeRef::User {
                name,
                type_args,
                is_nullable,
                span: _,
            } => {
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
            TypeRef::Function {
                params: _,
                return_type: _,
                is_nullable,
                span: _,
            } => ConeKotlinType::ClassLike {
                name: "kotlin.Function".to_string(),
                type_arguments: vec![],
                nullability: if *is_nullable {
                    ConeNullability::Nullable
                } else {
                    ConeNullability::NotNull
                },
            },
            TypeRef::Nullable { inner, span: _ } => {
                self.convert_type_ref(inner, _scope).make_nullable()
            }
        }
    }
}

pub fn analyze(file: &KtFile) -> (Scope, Vec<Diagnostic>) {
    let mut resolver = Resolver::new();
    resolver.resolve_file(file);
    (resolver.root_scope, resolver.diagnostics)
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
        match &diagnostics[0].code {
            DiagnosticCode::ValReassignment(name) => assert_eq!(name, "x"),
            other => panic!("expected ValReassignment, got: {:?}", other),
        }
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
        match &diagnostics[0].code {
            DiagnosticCode::TypeMismatch { expected, actual } => {
                assert!(expected.contains("String"));
                assert!(actual.contains("Int"));
            }
            other => panic!("expected TypeMismatch, got: {:?}", other),
        }
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
        match &diagnostics[0].code {
            DiagnosticCode::UnresolvedReference(name) => assert_eq!(name, "unknownVariable"),
            other => panic!("expected UnresolvedReference, got: {:?}", other),
        }
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
}
