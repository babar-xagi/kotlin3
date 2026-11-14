use crate::cone_types::{ConeKotlinType, ConeNullability, ConeProjection};
use k3_syntax::ast::{ClassKind, Modifier};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionSymbol {
    pub name: String,
    pub type_params: Vec<String>,
    pub receiver_type: Option<ConeKotlinType>,
    pub value_params: Vec<(String, ConeKotlinType)>,
    pub return_type: ConeKotlinType,
    pub modifiers: Vec<Modifier>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PropertySymbol {
    pub name: String,
    pub prop_type: ConeKotlinType,
    pub is_var: bool,
    pub modifiers: Vec<Modifier>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassSymbol {
    pub name: String,
    pub kind: ClassKind,
    pub super_types: Vec<ConeKotlinType>,
    pub methods: HashMap<String, Vec<FunctionSymbol>>,
    pub properties: HashMap<String, PropertySymbol>,
    pub modifiers: Vec<Modifier>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VariableSymbol {
    pub name: String,
    pub var_type: ConeKotlinType,
    pub is_var: bool,
}

#[derive(Debug, Clone)]
pub struct Scope {
    pub parent: Option<Box<Scope>>,
    pub variables: HashMap<String, VariableSymbol>,
    pub functions: HashMap<String, Vec<FunctionSymbol>>,
    pub classes: HashMap<String, ClassSymbol>,
}

impl Scope {
    pub fn new() -> Self {
        Self {
            parent: None,
            variables: HashMap::new(),
            functions: HashMap::new(),
            classes: HashMap::new(),
        }
    }

    pub fn with_parent(parent: Scope) -> Self {
        Self {
            parent: Some(Box::new(parent)),
            variables: HashMap::new(),
            functions: HashMap::new(),
            classes: HashMap::new(),
        }
    }

    pub fn insert_variable(&mut self, var: VariableSymbol) {
        self.variables.insert(var.name.clone(), var);
    }

    pub fn insert_function(&mut self, func: FunctionSymbol) {
        self.functions
            .entry(func.name.clone())
            .or_default()
            .push(func);
    }

    pub fn insert_class(&mut self, class: ClassSymbol) {
        self.classes.insert(class.name.clone(), class);
    }

    pub fn find_variable(&self, name: &str) -> Option<&VariableSymbol> {
        if let Some(var) = self.variables.get(name) {
            Some(var)
        } else if let Some(parent) = &self.parent {
            parent.find_variable(name)
        } else {
            None
        }
    }

    pub fn find_function(&self, name: &str) -> Option<&Vec<FunctionSymbol>> {
        if let Some(funcs) = self.functions.get(name) {
            Some(funcs)
        } else if let Some(parent) = &self.parent {
            parent.find_function(name)
        } else {
            None
        }
    }

    pub fn find_class(&self, name: &str) -> Option<&ClassSymbol> {
        if let Some(c) = self.classes.get(name) {
            Some(c)
        } else if let Some(parent) = &self.parent {
            parent.find_class(name)
        } else {
            None
        }
    }

    pub fn root() -> Self {
        let mut scope = Self::new();

        // Built-in standard library classes
        let builtins = [
            ("Any", ClassKind::Class),
            ("Int", ClassKind::Class),
            ("Long", ClassKind::Class),
            ("Float", ClassKind::Class),
            ("Double", ClassKind::Class),
            ("Boolean", ClassKind::Class),
            ("String", ClassKind::Class),
            ("Char", ClassKind::Class),
            ("Unit", ClassKind::Class),
            ("Nothing", ClassKind::Class),
            ("Array", ClassKind::Class),
            ("List", ClassKind::Interface),
            ("MutableList", ClassKind::Interface),
            ("Map", ClassKind::Interface),
            ("Set", ClassKind::Interface),
        ];

        for (name, kind) in builtins {
            let mut methods = HashMap::new();
            if name == "String" || name == "Int" || name == "Long" {
                // String / numeric common methods
                methods.insert(
                    "toString".to_string(),
                    vec![FunctionSymbol {
                        name: "toString".to_string(),
                        type_params: vec![],
                        receiver_type: None,
                        value_params: vec![],
                        return_type: ConeKotlinType::string(),
                        modifiers: vec![],
                    }],
                );
            } else if name == "List" || name == "MutableList" {
                methods.insert(
                    "add".to_string(),
                    vec![FunctionSymbol {
                        name: "add".to_string(),
                        type_params: vec![],
                        receiver_type: None,
                        value_params: vec![("element".to_string(), ConeKotlinType::any(true))],
                        return_type: ConeKotlinType::boolean(),
                        modifiers: vec![],
                    }],
                );
                methods.insert(
                    "map".to_string(),
                    vec![FunctionSymbol {
                        name: "map".to_string(),
                        type_params: vec!["R".to_string()],
                        receiver_type: None,
                        value_params: vec![("transform".to_string(), ConeKotlinType::any(false))],
                        return_type: ConeKotlinType::ClassLike {
                            name: "kotlin.collections.List".to_string(),
                            type_arguments: vec![ConeProjection::Star],
                            nullability: ConeNullability::NotNull,
                        },
                        modifiers: vec![],
                    }],
                );
                methods.insert(
                    "customFilter".to_string(),
                    vec![FunctionSymbol {
                        name: "customFilter".to_string(),
                        type_params: vec![],
                        receiver_type: None,
                        value_params: vec![("predicate".to_string(), ConeKotlinType::any(false))],
                        return_type: ConeKotlinType::ClassLike {
                            name: "kotlin.collections.List".to_string(),
                            type_arguments: vec![ConeProjection::Star],
                            nullability: ConeNullability::NotNull,
                        },
                        modifiers: vec![],
                    }],
                );
            }

            scope.insert_class(ClassSymbol {
                name: name.to_string(),
                kind,
                super_types: vec![ConeKotlinType::any(false)],
                methods,
                properties: HashMap::new(),
                modifiers: vec![],
            });
        }

        // Global functions: println, print, listOf, mutableListOf
        scope.insert_function(FunctionSymbol {
            name: "println".to_string(),
            type_params: vec![],
            receiver_type: None,
            value_params: vec![("message".to_string(), ConeKotlinType::any(true))],
            return_type: ConeKotlinType::unit(),
            modifiers: vec![],
        });
        scope.insert_function(FunctionSymbol {
            name: "println".to_string(),
            type_params: vec![],
            receiver_type: None,
            value_params: vec![],
            return_type: ConeKotlinType::unit(),
            modifiers: vec![],
        });
        scope.insert_function(FunctionSymbol {
            name: "print".to_string(),
            type_params: vec![],
            receiver_type: None,
            value_params: vec![("message".to_string(), ConeKotlinType::any(true))],
            return_type: ConeKotlinType::unit(),
            modifiers: vec![],
        });
        scope.insert_function(FunctionSymbol {
            name: "listOf".to_string(),
            type_params: vec!["T".to_string()],
            receiver_type: None,
            value_params: vec![("elements".to_string(), ConeKotlinType::any(true))],
            return_type: ConeKotlinType::ClassLike {
                name: "kotlin.collections.List".to_string(),
                type_arguments: vec![ConeProjection::Star],
                nullability: ConeNullability::NotNull,
            },
            modifiers: vec![],
        });
        scope.insert_function(FunctionSymbol {
            name: "mutableListOf".to_string(),
            type_params: vec!["T".to_string()],
            receiver_type: None,
            value_params: vec![],
            return_type: ConeKotlinType::ClassLike {
                name: "kotlin.collections.MutableList".to_string(),
                type_arguments: vec![ConeProjection::Star],
                nullability: ConeNullability::NotNull,
            },
            modifiers: vec![],
        });

        scope
    }
}
