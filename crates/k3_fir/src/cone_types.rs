#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConeNullability {
    NotNull,
    Nullable,
    Flexible,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConeProjection {
    Star,
    Invariant(Box<ConeKotlinType>),
    In(Box<ConeKotlinType>),
    Out(Box<ConeKotlinType>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConeKotlinType {
    ClassLike {
        name: String,
        type_arguments: Vec<ConeProjection>,
        nullability: ConeNullability,
    },
    TypeParameter {
        name: String,
        nullability: ConeNullability,
    },
    Flexible {
        lower_bound: Box<ConeKotlinType>,
        upper_bound: Box<ConeKotlinType>,
    },
    Intersection {
        intersected_types: Vec<ConeKotlinType>,
    },
    Unit,
    Nothing {
        nullability: ConeNullability,
    },
}

impl ConeKotlinType {
    pub fn int() -> Self {
        Self::ClassLike {
            name: "kotlin.Int".to_string(),
            type_arguments: vec![],
            nullability: ConeNullability::NotNull,
        }
    }

    pub fn string() -> Self {
        Self::ClassLike {
            name: "kotlin.String".to_string(),
            type_arguments: vec![],
            nullability: ConeNullability::NotNull,
        }
    }

    pub fn boolean() -> Self {
        Self::ClassLike {
            name: "kotlin.Boolean".to_string(),
            type_arguments: vec![],
            nullability: ConeNullability::NotNull,
        }
    }

    pub fn is_nullable(&self) -> bool {
        match self {
            ConeKotlinType::ClassLike { nullability, .. } => *nullability == ConeNullability::Nullable,
            ConeKotlinType::TypeParameter { nullability, .. } => *nullability == ConeNullability::Nullable,
            ConeKotlinType::Nothing { nullability } => *nullability == ConeNullability::Nullable,
            ConeKotlinType::Flexible { upper_bound, .. } => upper_bound.is_nullable(),
            ConeKotlinType::Intersection { intersected_types } => {
                intersected_types.iter().all(|t| t.is_nullable())
            }
            ConeKotlinType::Unit => false,
        }
    }
}
