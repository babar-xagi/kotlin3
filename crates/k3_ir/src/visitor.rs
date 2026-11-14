use crate::ir::*;

pub trait IrVisitor<R, D> {
    fn visit_file(&mut self, file: &IrFile, data: D) -> R;
    fn visit_declaration(&mut self, decl: &IrDeclaration, data: D) -> R;
    fn visit_function(&mut self, func: &IrFunction, data: D) -> R;
    fn visit_class(&mut self, class: &IrClass, data: D) -> R;
    fn visit_statement(&mut self, stmt: &IrStatement, data: D) -> R;
    fn visit_expression(&mut self, expr: &IrExpression, data: D) -> R;
}

pub trait IrTransformer {
    fn transform_file(&mut self, file: &mut IrFile) {
        for decl in &mut file.declarations {
            self.transform_declaration(decl);
        }
    }

    fn transform_declaration(&mut self, decl: &mut IrDeclaration) {
        match decl {
            IrDeclaration::Function(f) => self.transform_function(f),
            IrDeclaration::Class(c) => self.transform_class(c),
            IrDeclaration::Property(p) => self.transform_property(p),
            IrDeclaration::Field(f) => self.transform_field(f),
        }
    }

    fn transform_function(&mut self, func: &mut IrFunction) {
        if let Some(body) = &mut func.body {
            for stmt in &mut body.statements {
                self.transform_statement(stmt);
            }
        }
    }

    fn transform_class(&mut self, class: &mut IrClass) {
        for decl in &mut class.declarations {
            self.transform_declaration(decl);
        }
    }

    fn transform_property(&mut self, _prop: &mut IrProperty) {}
    fn transform_field(&mut self, _field: &mut IrField) {}

    fn transform_statement(&mut self, stmt: &mut IrStatement) {
        match stmt {
            IrStatement::Declaration(d) => self.transform_declaration(d),
            IrStatement::Assignment { target, value, .. } => {
                self.transform_expression(target);
                self.transform_expression(value);
            }
            IrStatement::Expression(e) => self.transform_expression(e),
        }
    }

    fn transform_expression(&mut self, expr: &mut IrExpression) {
        match expr {
            IrExpression::Call { args, receiver, .. } => {
                if let Some(recv) = receiver {
                    self.transform_expression(recv);
                }
                for a in args {
                    self.transform_expression(a);
                }
            }
            IrExpression::Block { statements, result, .. } => {
                for s in statements {
                    self.transform_statement(s);
                }
                if let Some(res) = result {
                    self.transform_expression(res);
                }
            }
            IrExpression::When { subject, branches, .. } => {
                if let Some(s) = subject {
                    self.transform_expression(s);
                }
                for b in branches {
                    self.transform_expression(&mut b.body);
                }
            }
            IrExpression::For { iterable, body, .. } => {
                self.transform_expression(iterable);
                self.transform_expression(body);
            }
            IrExpression::While { condition, body, .. } => {
                self.transform_expression(condition);
                self.transform_expression(body);
            }
            IrExpression::Return { value, .. } => {
                if let Some(v) = value {
                    self.transform_expression(v);
                }
            }
            _ => {}
        }
    }
}
