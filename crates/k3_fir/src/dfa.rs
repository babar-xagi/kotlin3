use crate::cone_types::ConeKotlinType;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct DfaState {
    pub smart_casts: HashMap<String, ConeKotlinType>,
}

impl DfaState {
    pub fn new() -> Self {
        Self {
            smart_casts: HashMap::new(),
        }
    }

    pub fn record_smart_cast(&mut self, var_name: &str, cast_type: ConeKotlinType) {
        self.smart_casts.insert(var_name.to_string(), cast_type);
    }

    pub fn get_smart_cast(&self, var_name: &str) -> Option<&ConeKotlinType> {
        self.smart_casts.get(var_name)
    }

    pub fn merge(a: &DfaState, b: &DfaState) -> DfaState {
        let mut merged = HashMap::new();
        for (var, ty_a) in &a.smart_casts {
            if let Some(ty_b) = b.smart_casts.get(var) {
                if ty_a == ty_b {
                    merged.insert(var.clone(), ty_a.clone());
                }
            }
        }
        DfaState {
            smart_casts: merged,
        }
    }
}
