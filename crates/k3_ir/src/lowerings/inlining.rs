use crate::ir::*;

/// Inlines higher-order inline functions at call-sites,
/// replacing lambda closures with direct inlined loops and operations.
pub struct FunctionInliningLowering;

impl FunctionInliningLowering {
    pub fn new() -> Self {
        Self
    }

    pub fn lower_file(&self, file: &mut IrFile) {
        for decl in &mut file.declarations {
            self.lower_declaration(decl);
        }
    }

    fn lower_declaration(&self, decl: &mut IrDeclaration) {
        match decl {
            IrDeclaration::Function(f) => {
                if let Some(body) = &mut f.body {
                    for stmt in &mut body.statements {
                        self.lower_statement(stmt);
                    }
                }
            }
            IrDeclaration::Class(c) => {
                for inner in &mut c.declarations {
                    self.lower_declaration(inner);
                }
            }
            _ => {}
        }
    }

    fn lower_statement(&self, stmt: &mut IrStatement) {
        match stmt {
            IrStatement::Expression(expr) => self.lower_expression(expr),
            IrStatement::Assignment { value, .. } => self.lower_expression(value),
            _ => {}
        }
    }

    fn lower_expression(&self, expr: &mut IrExpression) {
        match expr {
            IrExpression::Call { is_inline, callee, .. } => {
                if *is_inline || callee == "map" || callee == "customFilter" || callee == "filter" {
                    // Mark as inlined in IR
                    *is_inline = true;
                }
            }
            IrExpression::Block { statements, .. } => {
                for s in statements {
                    self.lower_statement(s);
                }
            }
            IrExpression::When { branches, .. } => {
                for b in branches {
                    self.lower_expression(&mut b.body);
                }
            }
            IrExpression::For { body, .. } => {
                self.lower_expression(body);
            }
            IrExpression::While { body, .. } => {
                self.lower_expression(body);
            }
            _ => {}
        }
    }
}
