use std::fmt;

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
    Unknown,
}

impl ConeKotlinType {
    pub fn any(nullable: bool) -> Self {
        Self::ClassLike {
            name: "kotlin.Any".to_string(),
            type_arguments: vec![],
            nullability: if nullable {
                ConeNullability::Nullable
            } else {
                ConeNullability::NotNull
            },
        }
    }

    pub fn int() -> Self {
        Self::ClassLike {
            name: "kotlin.Int".to_string(),
            type_arguments: vec![],
            nullability: ConeNullability::NotNull,
        }
    }

    pub fn long() -> Self {
        Self::ClassLike {
            name: "kotlin.Long".to_string(),
            type_arguments: vec![],
            nullability: ConeNullability::NotNull,
        }
    }

    pub fn float() -> Self {
        Self::ClassLike {
            name: "kotlin.Float".to_string(),
            type_arguments: vec![],
            nullability: ConeNullability::NotNull,
        }
    }

    pub fn double() -> Self {
        Self::ClassLike {
            name: "kotlin.Double".to_string(),
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

    pub fn string() -> Self {
        Self::ClassLike {
            name: "kotlin.String".to_string(),
            type_arguments: vec![],
            nullability: ConeNullability::NotNull,
        }
    }

    pub fn char() -> Self {
        Self::ClassLike {
            name: "kotlin.Char".to_string(),
            type_arguments: vec![],
            nullability: ConeNullability::NotNull,
        }
    }

    pub fn unit() -> Self {
        Self::Unit
    }

    pub fn nothing() -> Self {
        Self::Nothing {
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
            ConeKotlinType::Unknown => false,
        }
    }

    pub fn make_nullable(&self) -> Self {
        match self {
            ConeKotlinType::ClassLike {
                name,
                type_arguments,
                ..
            } => ConeKotlinType::ClassLike {
                name: name.clone(),
                type_arguments: type_arguments.clone(),
                nullability: ConeNullability::Nullable,
            },
            ConeKotlinType::TypeParameter { name, .. } => ConeKotlinType::TypeParameter {
                name: name.clone(),
                nullability: ConeNullability::Nullable,
            },
            ConeKotlinType::Nothing { .. } => ConeKotlinType::Nothing {
                nullability: ConeNullability::Nullable,
            },
            _ => self.clone(),
        }
    }

    pub fn make_not_null(&self) -> Self {
        match self {
            ConeKotlinType::ClassLike {
                name,
                type_arguments,
                ..
            } => ConeKotlinType::ClassLike {
                name: name.clone(),
                type_arguments: type_arguments.clone(),
                nullability: ConeNullability::NotNull,
            },
            ConeKotlinType::TypeParameter { name, .. } => ConeKotlinType::TypeParameter {
                name: name.clone(),
                nullability: ConeNullability::NotNull,
            },
            ConeKotlinType::Nothing { .. } => ConeKotlinType::Nothing {
                nullability: ConeNullability::NotNull,
            },
            _ => self.clone(),
        }
    }

    pub fn is_subtype_of(&self, expected: &ConeKotlinType) -> bool {
        if self == expected || *expected == ConeKotlinType::Unknown || *self == ConeKotlinType::Unknown {
            return true;
        }

        // Nothing is subtype of everything
        if let ConeKotlinType::Nothing { nullability } = self {
            if *nullability == ConeNullability::NotNull {
                return true;
            }
            if *nullability == ConeNullability::Nullable && expected.is_nullable() {
                return true;
            }
        }

        // Everything is subtype of Any?
        if let ConeKotlinType::ClassLike { name, nullability, .. } = expected {
            if name == "kotlin.Any" && *nullability == ConeNullability::Nullable {
                return true;
            }
            if name == "kotlin.Any" && *nullability == ConeNullability::NotNull && !self.is_nullable() {
                return true;
            }
        }

        // Check class-like subtyping
        if let (
            ConeKotlinType::ClassLike {
                name: actual_name,
                type_arguments: actual_args,
                nullability: actual_null,
            },
            ConeKotlinType::ClassLike {
                name: exp_name,
                type_arguments: exp_args,
                nullability: exp_null,
            },
        ) = (self, expected)
        {
            if *actual_null == ConeNullability::Nullable && *exp_null == ConeNullability::NotNull {
                return false;
            }

            let is_collection_subtype = (actual_name == "kotlin.collections.MutableList"
                && matches!(exp_name.as_str(), "kotlin.collections.List" | "kotlin.collections.Collection" | "kotlin.collections.Iterable"))
                || (actual_name == "kotlin.collections.List"
                    && matches!(exp_name.as_str(), "kotlin.collections.Collection" | "kotlin.collections.Iterable"))
                || (actual_name == "kotlin.collections.MutableSet"
                    && matches!(exp_name.as_str(), "kotlin.collections.Set" | "kotlin.collections.Collection" | "kotlin.collections.Iterable"))
                || (actual_name == "kotlin.collections.MutableMap" && exp_name == "kotlin.collections.Map");

            if actual_name == exp_name || is_collection_subtype {
                if actual_args.len() != exp_args.len() {
                    return true;
                }
                for (a, e) in actual_args.iter().zip(exp_args.iter()) {
                    match (a, e) {
                        (ConeProjection::Invariant(act), ConeProjection::Invariant(exp)) => {
                            if !act.is_subtype_of(exp) {
                                return false;
                            }
                        }
                        (ConeProjection::Star, _) | (_, ConeProjection::Star) => {}
                        _ => {}
                    }
                }
                return true;
            }

            // Numeric promotions & collections subtyping
            if (actual_name == "kotlin.Int" && exp_name == "kotlin.Long")
                || (actual_name == "kotlin.Int" && exp_name == "kotlin.Number")
                || (actual_name == "kotlin.Long" && exp_name == "kotlin.Number")
                || (actual_name == "kotlin.Float" && exp_name == "kotlin.Double")
            {
                return true;
            }
        }

        false
    }
}

impl fmt::Display for ConeKotlinType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConeKotlinType::ClassLike {
                name,
                type_arguments,
                nullability,
            } => {
                let short_name = name.rsplit('.').next().unwrap_or(name);
                write!(f, "{}", short_name)?;
                if !type_arguments.is_empty() {
                    write!(f, "<")?;
                    for (i, arg) in type_arguments.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        match arg {
                            ConeProjection::Star => write!(f, "*")?,
                            ConeProjection::Invariant(t) => write!(f, "{}", t)?,
                            ConeProjection::In(t) => write!(f, "in {}", t)?,
                            ConeProjection::Out(t) => write!(f, "out {}", t)?,
                        }
                    }
                    write!(f, ">")?;
                }
                if *nullability == ConeNullability::Nullable {
                    write!(f, "?")?;
                }
                Ok(())
            }
            ConeKotlinType::TypeParameter { name, nullability } => {
                write!(f, "{}", name)?;
                if *nullability == ConeNullability::Nullable {
                    write!(f, "?")?;
                }
                Ok(())
            }
            ConeKotlinType::Flexible {
                lower_bound,
                upper_bound,
            } => write!(f, "{}..{}", lower_bound, upper_bound),
            ConeKotlinType::Intersection { intersected_types } => {
                for (i, t) in intersected_types.iter().enumerate() {
                    if i > 0 {
                        write!(f, " & ")?;
                    }
                    write!(f, "{}", t)?;
                }
                Ok(())
            }
            ConeKotlinType::Unit => write!(f, "Unit"),
            ConeKotlinType::Nothing { nullability } => {
                if *nullability == ConeNullability::Nullable {
                    write!(f, "Nothing?")
                } else {
                    write!(f, "Nothing")
                }
            }
            ConeKotlinType::Unknown => write!(f, "<unknown>"),
        }
    }
}
