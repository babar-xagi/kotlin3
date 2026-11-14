use crate::ir::*;
use k3_fir::ConeKotlinType;

/// Lowers `suspend fun` into Continuation-Passing Style (CPS) state-machines.
/// Appends `Continuation<T>` parameter, adjusts return type to `Any?`,
/// and creates state switch labels for resumption points.
pub struct AddContinuationLowering;

impl AddContinuationLowering {
    pub fn new() -> Self {
        Self
    }

    pub fn lower_file(&self, file: &mut IrFile) {
        let mut new_declarations = Vec::new();

        for mut decl in file.declarations.drain(..) {
            match &mut decl {
                IrDeclaration::Function(f) => {
                    if f.is_suspend {
                        self.transform_suspend_function(f, &mut new_declarations, &file.package_name);
                    }
                    new_declarations.push(decl);
                }
                IrDeclaration::Class(c) => {
                    self.lower_class(c, &file.package_name, &mut new_declarations);
                    new_declarations.push(decl);
                }
                _ => {
                    new_declarations.push(decl);
                }
            }
        }

        file.declarations = new_declarations;
    }

    fn lower_class(&self, c: &mut IrClass, package_name: &str, file_decls: &mut Vec<IrDeclaration>) {
        for decl in &mut c.declarations {
            if let IrDeclaration::Function(f) = decl {
                if f.is_suspend {
                    self.transform_suspend_function(f, file_decls, package_name);
                }
            }
        }
    }

    fn transform_suspend_function(
        &self,
        f: &mut IrFunction,
        generated_decls: &mut Vec<IrDeclaration>,
        package_prefix: &str,
    ) {
        let original_return_type = f.return_type.clone();

        // 1. Append Continuation parameter: `completion: kotlin.coroutines.Continuation<T>`
        let continuation_type = make_class_type(
            "kotlin.coroutines.Continuation",
            vec![original_return_type],
            false,
        );

        let continuation_param = IrValueParam {
            name: "$completion".to_string(),
            param_type: continuation_type,
            default_value: None,
            index: f.value_params.len() as u32,
            span: f.span,
        };
        f.value_params.push(continuation_param);

        // 2. Suspend functions can return either T or COROUTINE_SUSPENDED sentinel, so return type is Any?
        f.return_type = ConeKotlinType::any(true);

        // 3. Generate synthetic State Machine Continuation Class
        // e.g. `<package>.<funcName>$Continuation`
        let cont_class_name = format!("{}${}$Continuation", package_prefix.replace('/', "."), f.name);
        let cont_class = IrClass {
            name: cont_class_name.clone(),
            kind: IrClassKind::Class,
            super_types: vec![make_class_type(
                "kotlin.coroutines.jvm.internal.ContinuationImpl",
                vec![],
                false,
            )],
            primary_constructor_params: Some(vec![IrValueParam {
                name: "completion".to_string(),
                param_type: ConeKotlinType::any(true),
                default_value: None,
                index: 0,
                span: f.span,
            }]),
            declarations: vec![
                IrDeclaration::Field(IrField {
                    name: "result".to_string(),
                    field_type: ConeKotlinType::any(true),
                    is_final: false,
                    is_static: false,
                    span: f.span,
                }),
                IrDeclaration::Field(IrField {
                    name: "label".to_string(),
                    field_type: ConeKotlinType::int(),
                    is_final: false,
                    is_static: false,
                    span: f.span,
                }),
            ],
            span: f.span,
        };
        generated_decls.push(IrDeclaration::Class(cont_class));

        // 4. Wrap body into StateSwitch (state machine states)
        if let Some(body) = &mut f.body {
            let mut state_0_stmts = Vec::new();
            for stmt in body.statements.drain(..) {
                state_0_stmts.push(stmt);
            }

            let state_switch = IrExpression::StateSwitch {
                label_expr: Box::new(IrExpression::GetLocal(
                    "$label".to_string(),
                    ConeKotlinType::int(),
                    f.span,
                )),
                cases: vec![(0, state_0_stmts)],
                span: f.span,
            };

            body.statements.push(IrStatement::Expression(state_switch));
        }

        f.origin = IrDeclarationOrigin::SyntheticContinuation;
    }
}
