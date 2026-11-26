use crate::ir::*;
use std::collections::HashMap;

/// Inlines `inline fun` bodies at call sites when the callee is defined in the same file
/// with a single-expression or single-return body, and marks collection inline operations.
pub struct FunctionInliningLowering;

impl FunctionInliningLowering {
    pub fn new() -> Self {
        Self
    }

    pub fn lower_file(&self, file: &mut IrFile) {
        let mut inline_defs: HashMap<String, IrFunction> = HashMap::new();
        for decl in &file.declarations {
            if let IrDeclaration::Function(f) = decl {
                if f.is_inline && f.body.is_some() {
                    inline_defs.insert(f.name.clone(), f.clone());
                }
            }
        }

        for decl in &mut file.declarations {
            self.lower_declaration(decl, &inline_defs);
        }
    }

    fn lower_declaration(&self, decl: &mut IrDeclaration, inline_defs: &HashMap<String, IrFunction>) {
        match decl {
            IrDeclaration::Function(f) => {
                if let Some(body) = &mut f.body {
                    for stmt in &mut body.statements {
                        self.lower_statement(stmt, inline_defs);
                    }
                }
            }
            IrDeclaration::Class(c) => {
                for inner in &mut c.declarations {
                    self.lower_declaration(inner, inline_defs);
                }
            }
            _ => {}
        }
    }

    fn lower_statement(&self, stmt: &mut IrStatement, inline_defs: &HashMap<String, IrFunction>) {
        match stmt {
            IrStatement::Declaration(d) => self.lower_declaration(d, inline_defs),
            IrStatement::Expression(expr) => self.lower_expression(expr, inline_defs),
            IrStatement::Assignment { target, value, .. } => {
                self.lower_expression(target, inline_defs);
                self.lower_expression(value, inline_defs);
            }
        }
    }

    fn lower_expression(&self, expr: &mut IrExpression, inline_defs: &HashMap<String, IrFunction>) {
        match expr {
            IrExpression::Call { is_inline, callee, receiver, args, span, .. } => {
                if let Some(recv) = receiver {
                    self.lower_expression(recv, inline_defs);
                }
                for a in args.iter_mut() {
                    self.lower_expression(a, inline_defs);
                }

                if let Some(target_fn) = inline_defs.get(callee.as_str()) {
                    if let Some(inlined) = try_inline_call(target_fn, args, *span) {
                        *expr = inlined;
                        return;
                    }
                }

                if *is_inline || callee == "map" || callee == "customFilter" || callee == "filter" {
                    *is_inline = true;
                }
            }
            IrExpression::Block { statements, result, .. } => {
                for s in statements {
                    self.lower_statement(s, inline_defs);
                }
                if let Some(res) = result {
                    self.lower_expression(res, inline_defs);
                }
            }
            IrExpression::When { subject, branches, .. } => {
                if let Some(s) = subject {
                    self.lower_expression(s, inline_defs);
                }
                for b in branches {
                    self.lower_expression(&mut b.body, inline_defs);
                }
            }
            IrExpression::For { iterable, body, .. } => {
                self.lower_expression(iterable, inline_defs);
                self.lower_expression(body, inline_defs);
            }
            IrExpression::While { condition, body, .. } => {
                self.lower_expression(condition, inline_defs);
                self.lower_expression(body, inline_defs);
            }
            IrExpression::Return { value, .. } => {
                if let Some(v) = value {
                    self.lower_expression(v, inline_defs);
                }
            }
            _ => {}
        }
    }
}

fn try_inline_call(
    target_fn: &IrFunction,
    args: &[IrExpression],
    call_span: k3_syntax::Span,
) -> Option<IrExpression> {
    let body = target_fn.body.as_ref()?;
    if target_fn.value_params.len() != args.len() {
        return None;
    }

    let mut bindings: HashMap<String, IrExpression> = HashMap::new();
    for (param, arg) in target_fn.value_params.iter().zip(args.iter()) {
        bindings.insert(param.name.clone(), arg.clone());
    }

    if body.statements.len() == 1 {
        if let IrStatement::Expression(IrExpression::Return { value: Some(inner), .. }) = &body.statements[0] {
            return Some(substitute_expr(inner, &bindings));
        }
    }

    let mut stmts = Vec::new();
    for (param, arg) in target_fn.value_params.iter().zip(args.iter()) {
        stmts.push(IrStatement::Declaration(IrDeclaration::Property(IrProperty {
            name: param.name.clone(),
            prop_type: param.param_type.clone(),
            is_val: true,
            initializer: Some(arg.clone()),
            span: call_span,
        })));
    }
    for s in &body.statements {
        stmts.push(s.clone());
    }
    Some(IrExpression::Block {
        statements: stmts,
        result: None,
        span: call_span,
    })
}

fn substitute_expr(expr: &IrExpression, bindings: &HashMap<String, IrExpression>) -> IrExpression {
    match expr {
        IrExpression::GetLocal(name, _, _) => {
            if let Some(replacement) = bindings.get(name) {
                replacement.clone()
            } else {
                expr.clone()
            }
        }
        IrExpression::Call { callee, receiver, args, return_type, is_inline, span } => IrExpression::Call {
            callee: callee.clone(),
            receiver: receiver.as_ref().map(|r| Box::new(substitute_expr(r, bindings))),
            args: args.iter().map(|a| substitute_expr(a, bindings)).collect(),
            return_type: return_type.clone(),
            is_inline: *is_inline,
            span: *span,
        },
        _ => expr.clone(),
    }
}
