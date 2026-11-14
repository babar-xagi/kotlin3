use crate::ir::*;
use k3_fir::ConeKotlinType;

/// Generates synthetic `$default` methods with integer bitmasks
/// and rewrites call-sites omitting optional parameters.
pub struct DefaultArgumentStubLowering;

impl DefaultArgumentStubLowering {
    pub fn new() -> Self {
        Self
    }

    pub fn lower_file(&self, file: &mut IrFile) {
        let mut new_declarations = Vec::new();

        for mut decl in file.declarations.drain(..) {
            match &mut decl {
                IrDeclaration::Function(f) => {
                    let has_defaults = f.value_params.iter().any(|p| p.default_value.is_some());
                    if has_defaults {
                        let stub = self.create_default_stub_function(f);
                        new_declarations.push(IrDeclaration::Function(stub));
                    }
                    new_declarations.push(decl);
                }
                IrDeclaration::Class(c) => {
                    self.lower_class(c, &mut new_declarations);
                    new_declarations.push(decl);
                }
                _ => {
                    new_declarations.push(decl);
                }
            }
        }

        file.declarations = new_declarations;
    }

    fn lower_class(&self, c: &mut IrClass, _extra_decls: &mut Vec<IrDeclaration>) {
        let mut class_extra = Vec::new();

        // Check primary constructor default parameters
        if let Some(params) = &c.primary_constructor_params {
            let has_defaults = params.iter().any(|p| p.default_value.is_some());
            if has_defaults {
                // Synthetic constructor with bitmask
                let mut stub_params = params.clone();
                stub_params.push(IrValueParam {
                    name: "$mask0".to_string(),
                    param_type: ConeKotlinType::int(),
                    default_value: None,
                    index: stub_params.len() as u32,
                    span: c.span,
                });
                stub_params.push(IrValueParam {
                    name: "$marker".to_string(),
                    param_type: make_class_type(
                        "kotlin.jvm.internal.DefaultConstructorMarker",
                        vec![],
                        true,
                    ),
                    default_value: None,
                    index: stub_params.len() as u32,
                    span: c.span,
                });

                let ctor_stub = IrFunction {
                    name: "<init>$default".to_string(),
                    is_suspend: false,
                    is_inline: false,
                    is_static: false,
                    visibility: IrVisibility::Public,
                    receiver_type: None,
                    value_params: stub_params,
                    return_type: ConeKotlinType::unit(),
                    body: Some(IrBody { statements: vec![] }),
                    origin: IrDeclarationOrigin::SyntheticDefaultStub,
                    span: c.span,
                };
                class_extra.push(IrDeclaration::Function(ctor_stub));
            }
        }

        for decl in &mut c.declarations {
            if let IrDeclaration::Function(f) = decl {
                let has_defaults = f.value_params.iter().any(|p| p.default_value.is_some());
                if has_defaults {
                    let stub = self.create_default_stub_function(f);
                    class_extra.push(IrDeclaration::Function(stub));
                }
            }
        }

        c.declarations.extend(class_extra);
    }

    fn create_default_stub_function(&self, f: &IrFunction) -> IrFunction {
        let stub_name = format!("{}$default", f.name);
        let mut stub_params = f.value_params.clone();

        // Append $mask0 bitmask
        stub_params.push(IrValueParam {
            name: "$mask0".to_string(),
            param_type: ConeKotlinType::int(),
            default_value: None,
            index: stub_params.len() as u32,
            span: f.span,
        });

        // Append $handler (Object for future extension / interface dispatch)
        stub_params.push(IrValueParam {
            name: "$handler".to_string(),
            param_type: ConeKotlinType::any(true),
            default_value: None,
            index: stub_params.len() as u32,
            span: f.span,
        });

        // Build statements: for each param with default, if (mask & (1 << i) != 0) param = default
        let mut stmts = Vec::new();
        let mut call_args = Vec::new();

        for (i, p) in f.value_params.iter().enumerate() {
            if let Some(default_expr) = &p.default_value {
                let bit = 1i64 << i;
                let cond = IrExpression::Call {
                    callee: "notEquals".to_string(),
                    receiver: Some(Box::new(IrExpression::Call {
                        callee: "and".to_string(),
                        receiver: Some(Box::new(IrExpression::GetLocal(
                            "$mask0".to_string(),
                            ConeKotlinType::int(),
                            p.span,
                        ))),
                        args: vec![IrExpression::ConstInt(bit, ConeKotlinType::int(), p.span)],
                        return_type: ConeKotlinType::int(),
                        is_inline: false,
                        span: p.span,
                    })),
                    args: vec![IrExpression::ConstInt(0, ConeKotlinType::int(), p.span)],
                    return_type: ConeKotlinType::boolean(),
                    is_inline: false,
                    span: p.span,
                };

                let assign_default = IrStatement::Assignment {
                    target: IrExpression::GetLocal(p.name.clone(), p.param_type.clone(), p.span),
                    value: default_expr.clone(),
                    span: p.span,
                };

                stmts.push(IrStatement::Expression(IrExpression::When {
                    subject: None,
                    branches: vec![IrBranch {
                        conditions: vec![IrBranchCondition::Expression(cond)],
                        body: IrExpression::Block {
                            statements: vec![assign_default],
                            result: None,
                            span: p.span,
                        },
                        span: p.span,
                    }],
                    span: p.span,
                }));
            }

            call_args.push(IrExpression::GetLocal(p.name.clone(), p.param_type.clone(), p.span));
        }

        // Delegate to original function
        let delegate_call = IrExpression::Call {
            callee: f.name.clone(),
            receiver: None,
            args: call_args,
            return_type: f.return_type.clone(),
            is_inline: f.is_inline,
            span: f.span,
        };

        stmts.push(IrStatement::Expression(IrExpression::Return {
            value: Some(Box::new(delegate_call)),
            span: f.span,
        }));

        IrFunction {
            name: stub_name,
            is_suspend: f.is_suspend,
            is_inline: f.is_inline,
            is_static: true,
            visibility: IrVisibility::Public,
            receiver_type: f.receiver_type.clone(),
            value_params: stub_params,
            return_type: f.return_type.clone(),
            body: Some(IrBody { statements: stmts }),
            origin: IrDeclarationOrigin::SyntheticDefaultStub,
            span: f.span,
        }
    }
}
