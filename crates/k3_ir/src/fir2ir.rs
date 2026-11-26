use crate::ir::*;
use k3_fir::ConeKotlinType;
use k3_syntax::ast::*;
use k3_syntax::Span;

pub struct Fir2Ir {
    pub package_name: String,
}

impl Fir2Ir {
    pub fn new() -> Self {
        Self {
            package_name: String::new(),
        }
    }

    pub fn convert_file(&mut self, file: &KtFile, file_name: &str) -> IrFile {
        let package_name = file
            .package
            .as_ref()
            .map(|p| p.name.to_string())
            .unwrap_or_default();
        self.package_name = package_name.clone();

        let mut declarations = Vec::new();
        for decl in &file.declarations {
            if let Some(ir_decl) = self.convert_declaration(decl) {
                declarations.push(ir_decl);
            }
        }

        IrFile {
            file_name: file_name.to_string(),
            package_name,
            declarations,
        }
    }

    pub fn convert_declaration(&mut self, decl: &Declaration) -> Option<IrDeclaration> {
        match decl {
            Declaration::Function(f) => Some(IrDeclaration::Function(self.convert_function(f))),
            Declaration::Class(c) => Some(IrDeclaration::Class(self.convert_class(c))),
            Declaration::Property(p) => Some(IrDeclaration::Property(self.convert_property(p))),
        }
    }

    pub fn convert_function(&mut self, f: &NamedFunction) -> IrFunction {
        let is_suspend = f.modifiers.iter().any(|m| matches!(m, Modifier::Suspend));
        let is_inline = f.modifiers.iter().any(|m| matches!(m, Modifier::Inline));

        let receiver_type = f.receiver_type.as_ref().map(|tr| self.convert_type_ref(tr));
        let return_type = f
            .return_type
            .as_ref()
            .map(|tr| self.convert_type_ref(tr))
            .unwrap_or_else(ConeKotlinType::unit);

        let mut value_params = Vec::new();
        for (idx, p) in f.value_params.iter().enumerate() {
            let ptype = self.convert_type_ref(&p.type_ref);
            let default_val = p.default_value.as_ref().map(|e| self.convert_expression(e));
            value_params.push(IrValueParam {
                name: p.name.name.clone(),
                param_type: ptype,
                default_value: default_val,
                index: idx as u32,
                span: p.span,
            });
        }

        let body = f.body.as_ref().map(|b| match b {
            FunctionBody::Block(stmts) => IrBody {
                statements: stmts.iter().map(|s| self.convert_statement(s)).collect(),
            },
            FunctionBody::Expression(expr) => IrBody {
                statements: vec![IrStatement::Expression(IrExpression::Return {
                    value: Some(Box::new(self.convert_expression(expr))),
                    span: expr.span(),
                })],
            },
        });

        IrFunction {
            name: f.name.name.clone(),
            is_suspend,
            is_inline,
            is_static: false,
            visibility: IrVisibility::Public,
            receiver_type,
            value_params,
            return_type,
            body,
            origin: IrDeclarationOrigin::Defined,
            span: f.span,
        }
    }

    pub fn convert_class(&mut self, c: &ClassDeclaration) -> IrClass {
        let kind = match c.kind {
            ClassKind::Interface => IrClassKind::Interface,
            ClassKind::DataClass => IrClassKind::DataClass,
            ClassKind::EnumClass => IrClassKind::Enum,
            _ => {
                if c.modifiers.iter().any(|m| matches!(m, Modifier::Inline)) {
                    IrClassKind::ValueClass
                } else {
                    IrClassKind::Class
                }
            }
        };

        let super_types = c
            .super_types
            .iter()
            .map(|st| self.convert_type_ref(&st.type_ref))
            .collect();

        let primary_constructor_params = c.primary_constructor_params.as_ref().map(|params| {
            params
                .iter()
                .enumerate()
                .map(|(idx, p)| {
                    let ptype = self.convert_type_ref(&p.type_ref);
                    let def = p.default_value.as_ref().map(|e| self.convert_expression(e));
                    IrValueParam {
                        name: p.name.name.clone(),
                        param_type: ptype,
                        default_value: def,
                        index: idx as u32,
                        span: p.span,
                    }
                })
                .collect()
        });

        let mut declarations = Vec::new();
        for d in &c.declarations {
            if let Some(ir_d) = self.convert_declaration(d) {
                declarations.push(ir_d);
            }
        }

        IrClass {
            name: c.name.name.clone(),
            kind,
            super_types,
            primary_constructor_params,
            declarations,
            span: c.span,
        }
    }

    pub fn convert_property(&mut self, p: &PropertyDeclaration) -> IrProperty {
        let prop_type = p
            .type_ref
            .as_ref()
            .map(|tr| self.convert_type_ref(tr))
            .unwrap_or_else(|| ConeKotlinType::any(false));
        let initializer = p.initializer.as_ref().map(|e| self.convert_expression(e));

        IrProperty {
            name: p.name.name.clone(),
            prop_type,
            is_val: !p.is_var,
            initializer,
            span: p.span,
        }
    }

    pub fn convert_statement(&mut self, stmt: &Statement) -> IrStatement {
        match stmt {
            Statement::Declaration(d) => {
                if let Some(ir_d) = self.convert_declaration(d) {
                    IrStatement::Declaration(ir_d)
                } else {
                    IrStatement::Expression(IrExpression::ConstNull(Span { start: 0, end: 0 }))
                }
            }
            Statement::Assignment { target, op: _, value, span } => IrStatement::Assignment {
                target: self.convert_expression(target),
                value: self.convert_expression(value),
                span: *span,
            },
            Statement::Expression(expr) => IrStatement::Expression(self.convert_expression(expr)),
        }
    }

    pub fn convert_expression(&mut self, expr: &Expression) -> IrExpression {
        match expr {
            Expression::IntegerLiteral(val, span) => {
                IrExpression::ConstInt(*val, ConeKotlinType::int(), *span)
            }
            Expression::LongLiteral(val, span) => {
                IrExpression::ConstInt(*val, ConeKotlinType::long(), *span)
            }
            Expression::FloatLiteral(val, span) => {
                IrExpression::ConstFloat(*val, ConeKotlinType::double(), *span)
            }
            Expression::BooleanLiteral(val, span) => IrExpression::ConstBoolean(*val, *span),
            Expression::StringLiteral(s, span) => {
                if s.contains('$') {
                    self.convert_interpolated_string(s, *span)
                } else {
                    IrExpression::ConstString(s.clone(), *span)
                }
            }
            Expression::StringTemplate(parts, span) => {
                let ir_parts = parts.iter().map(|p| self.convert_expression(p)).collect();
                IrExpression::StringTemplate(ir_parts, *span)
            }
            Expression::NullLiteral(span) => IrExpression::ConstNull(*span),
            Expression::This(span) => {
                IrExpression::GetLocal("this".to_string(), ConeKotlinType::any(false), *span)
            }
            Expression::Identifier(id) => {
                IrExpression::GetLocal(id.name.clone(), ConeKotlinType::any(true), id.span)
            }
            Expression::Unary { op, expr, span, .. } => {
                let inner_ir = self.convert_expression(expr);
                let callee_name = match op {
                    UnaryOp::Neg => "unaryMinus",
                    UnaryOp::Pos => "unaryPlus",
                    UnaryOp::Not => "not",
                    UnaryOp::Inc => "inc",
                    UnaryOp::Dec => "dec",
                };
                IrExpression::Call {
                    callee: callee_name.to_string(),
                    receiver: Some(Box::new(inner_ir)),
                    args: vec![],
                    return_type: ConeKotlinType::any(true),
                    is_inline: false,
                    span: *span,
                }
            }
            Expression::MemberAccess { target, member, span, .. } => {
                let target_ir = self.convert_expression(target);
                IrExpression::GetField {
                    receiver: Some(Box::new(target_ir)),
                    class_name: "".to_string(),
                    field_name: member.name.clone(),
                    field_type: ConeKotlinType::any(true),
                    span: *span,
                }
            }
            Expression::IndexAccess { target, indices, span } => {
                let target_ir = self.convert_expression(target);
                let args = indices.iter().map(|i| self.convert_expression(i)).collect();
                IrExpression::Call {
                    callee: "get".to_string(),
                    receiver: Some(Box::new(target_ir)),
                    args,
                    return_type: ConeKotlinType::any(true),
                    is_inline: false,
                    span: *span,
                }
            }
            Expression::Lambda { params, body, span } => IrExpression::Lambda {
                params: params.iter().map(|p| p.name.clone()).collect(),
                body: body.iter().map(|s| self.convert_statement(s)).collect(),
                return_type: ConeKotlinType::any(true),
                span: *span,
            },
            Expression::Binary { left, op, right, span } => {
                let left_ir = self.convert_expression(left);
                let right_ir = self.convert_expression(right);
                let callee_name = match op {
                    BinaryOp::Add => "plus",
                    BinaryOp::Sub => "minus",
                    BinaryOp::Mul => "times",
                    BinaryOp::Div => "div",
                    BinaryOp::Mod => "rem",
                    BinaryOp::Range => "rangeTo",
                    BinaryOp::Eq => "equals",
                    BinaryOp::NotEq => "notEquals",
                    _ => "binaryOp",
                };
                IrExpression::Call {
                    callee: callee_name.to_string(),
                    receiver: Some(Box::new(left_ir)),
                    args: vec![right_ir],
                    return_type: ConeKotlinType::any(true),
                    is_inline: false,
                    span: *span,
                }
            }
            Expression::Call { callee, args, trailing_lambda, span, .. } => {
                let mut ir_args = Vec::new();
                for arg in args {
                    ir_args.push(self.convert_expression(&arg.value));
                }
                if let Some(lambda) = trailing_lambda {
                    ir_args.push(self.convert_expression(lambda));
                }

                match &**callee {
                    Expression::MemberAccess { target, member, .. } => {
                        let target_ir = self.convert_expression(target);
                        IrExpression::Call {
                            callee: member.name.clone(),
                            receiver: Some(Box::new(target_ir)),
                            args: ir_args,
                            return_type: ConeKotlinType::any(true),
                            is_inline: member.name == "map" || member.name == "filter",
                            span: *span,
                        }
                    }
                    Expression::Identifier(id) => {
                        IrExpression::Call {
                            callee: id.name.clone(),
                            receiver: None,
                            args: ir_args,
                            return_type: ConeKotlinType::any(true),
                            is_inline: false,
                            span: *span,
                        }
                    }
                    _ => {
                        let callee_ir = self.convert_expression(callee);
                        IrExpression::Call {
                            callee: "invoke".to_string(),
                            receiver: Some(Box::new(callee_ir)),
                            args: ir_args,
                            return_type: ConeKotlinType::any(true),
                            is_inline: false,
                            span: *span,
                        }
                    }
                }
            }
            Expression::When { subject, entries, span } => {
                let subject_ir = subject.as_ref().map(|s| Box::new(self.convert_expression(s)));
                let mut branches = Vec::new();

                for entry in entries {
                    let mut conditions = Vec::new();
                    if entry.conditions.is_empty() {
                        conditions.push(IrBranchCondition::Else);
                    } else {
                        for c in &entry.conditions {
                            match c {
                                WhenCondition::Expression(e) => {
                                    conditions.push(IrBranchCondition::Expression(
                                        self.convert_expression(e),
                                    ));
                                }
                                WhenCondition::Is(tr, is_negated) => {
                                    let target_type = self.convert_type_ref(tr);
                                    conditions.push(IrBranchCondition::TypeCheck {
                                        target_type,
                                        is_negated: *is_negated,
                                    });
                                }
                                WhenCondition::In(e, is_negated) => {
                                    let range_ir = self.convert_expression(e);
                                    conditions.push(IrBranchCondition::Expression(
                                        IrExpression::Call {
                                            callee: if *is_negated { "notContains" } else { "contains" }.to_string(),
                                            receiver: Some(Box::new(range_ir)),
                                            args: vec![],
                                            return_type: ConeKotlinType::boolean(),
                                            is_inline: false,
                                            span: e.span(),
                                        },
                                    ));
                                }
                            }
                        }
                    }
                    let body_ir = self.convert_expression(&entry.body);
                    branches.push(IrBranch {
                        conditions,
                        body: body_ir,
                        span: entry.span,
                    });
                }

                IrExpression::When {
                    subject: subject_ir,
                    branches,
                    span: *span,
                }
            }
            Expression::For { variable, iterable, body, span } => IrExpression::For {
                variable: variable.name.clone(),
                iterable: Box::new(self.convert_expression(iterable)),
                body: Box::new(self.convert_expression(body)),
                span: *span,
            },
            Expression::While { condition, body, span, .. } => IrExpression::While {
                condition: Box::new(self.convert_expression(condition)),
                body: Box::new(self.convert_expression(body)),
                span: *span,
            },
            Expression::If { condition, then_branch, else_branch, span } => {
                let cond_ir = self.convert_expression(condition);
                let then_ir = self.convert_expression(then_branch);
                let mut branches = vec![IrBranch {
                    conditions: vec![IrBranchCondition::Expression(cond_ir)],
                    body: then_ir,
                    span: then_branch.span(),
                }];
                if let Some(eb) = else_branch {
                    branches.push(IrBranch {
                        conditions: vec![IrBranchCondition::Else],
                        body: self.convert_expression(eb),
                        span: eb.span(),
                    });
                }
                IrExpression::When {
                    subject: None,
                    branches,
                    span: *span,
                }
            }
            Expression::Block(stmts, span) => {
                let ir_stmts: Vec<IrStatement> =
                    stmts.iter().map(|s| self.convert_statement(s)).collect();
                IrExpression::Block {
                    statements: ir_stmts,
                    result: None,
                    span: *span,
                }
            }
            Expression::Return { value, span } => IrExpression::Return {
                value: value.as_ref().map(|v| Box::new(self.convert_expression(v))),
                span: *span,
            },
            _ => IrExpression::ConstNull(expr.span()),
        }
    }

    fn convert_interpolated_string(&mut self, s: &str, span: Span) -> IrExpression {
        let bytes = s.as_bytes();
        let mut i = 0;
        let mut parts = Vec::new();
        let mut literal_buf = String::new();

        while i < bytes.len() {
            if bytes[i] == b'$' && i + 1 < bytes.len() {
                if !literal_buf.is_empty() {
                    parts.push(IrExpression::ConstString(literal_buf.clone(), span));
                    literal_buf.clear();
                }
                i += 1;
                let var_name = if bytes[i] == b'{' {
                    i += 1;
                    let start = i;
                    while i < bytes.len() && bytes[i] != b'}' {
                        i += 1;
                    }
                    let n = std::str::from_utf8(&bytes[start..i]).unwrap_or("");
                    if i < bytes.len() {
                        i += 1;
                    }
                    n
                } else {
                    let start = i;
                    while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                        i += 1;
                    }
                    std::str::from_utf8(&bytes[start..i]).unwrap_or("")
                };
                parts.push(IrExpression::GetLocal(
                    var_name.to_string(),
                    ConeKotlinType::any(true),
                    span,
                ));
            } else {
                literal_buf.push(bytes[i] as char);
                i += 1;
            }
        }

        if !literal_buf.is_empty() {
            parts.push(IrExpression::ConstString(literal_buf, span));
        }

        IrExpression::StringTemplate(parts, span)
    }

    pub fn convert_type_ref(&self, tr: &TypeRef) -> ConeKotlinType {
        match tr {
            TypeRef::User { name, type_args, is_nullable, .. } => {
                let n = name.to_string();
                let is_nullable = *is_nullable;
                match n.as_str() {
                    "Int" => ConeKotlinType::int(),
                    "Long" => ConeKotlinType::long(),
                    "Float" => ConeKotlinType::float(),
                    "Double" => ConeKotlinType::double(),
                    "Boolean" => ConeKotlinType::boolean(),
                    "String" => ConeKotlinType::string(),
                    "Unit" => ConeKotlinType::unit(),
                    "Any" => ConeKotlinType::any(is_nullable),
                    "Nothing" => ConeKotlinType::nothing(),
                    _ => {
                        let args = type_args.iter().map(|a| self.convert_type_ref(a)).collect();
                        make_class_type(&n, args, is_nullable)
                    }
                }
            }
            TypeRef::Function { params, return_type, .. } => {
                let mut all_types = params.iter().map(|p| self.convert_type_ref(p)).collect::<Vec<_>>();
                all_types.push(self.convert_type_ref(return_type));
                make_class_type("kotlin.Function", all_types, false)
            }
            TypeRef::Nullable { inner, .. } => self.convert_type_ref(inner).make_nullable(),
        }
    }
}
