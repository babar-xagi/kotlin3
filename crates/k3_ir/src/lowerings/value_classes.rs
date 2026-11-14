use crate::ir::*;
use std::collections::HashMap;

/// Lowers `@JvmInline value class` into unboxed primitives
/// and mangles function signatures to avoid JVM signature clashes.
pub struct ValueClassLowering;

impl ValueClassLowering {
    pub fn new() -> Self {
        Self
    }

    pub fn lower_file(&self, file: &mut IrFile) {
        let mut value_class_types = HashMap::new();

        // 1. Collect all value classes and their underlying unboxed types
        for decl in &file.declarations {
            if let IrDeclaration::Class(c) = decl {
                if c.kind == IrClassKind::ValueClass {
                    if let Some(params) = &c.primary_constructor_params {
                        if let Some(first) = params.first() {
                            value_class_types.insert(c.name.clone(), first.param_type.clone());
                        }
                    }
                }
            }
        }

        if value_class_types.is_empty() {
            return;
        }

        // 2. Unbox value class parameters in functions
        for decl in &mut file.declarations {
            if let IrDeclaration::Function(f) = decl {
                let mut needs_mangling = false;
                for p in &mut f.value_params {
                    if let k3_fir::ConeKotlinType::ClassLike { name, .. } = &p.param_type {
                        if let Some(unboxed) = value_class_types.get(name) {
                            p.param_type = unboxed.clone();
                            needs_mangling = true;
                        }
                    }
                }
                if needs_mangling {
                    f.name = format!("{}-impl", f.name);
                }
            }
        }
    }
}
