pub mod cone_types;
pub mod symbols;
pub mod diagnostics;
pub mod dfa;
pub mod resolution;

pub use cone_types::{ConeKotlinType, ConeNullability, ConeProjection};
pub use symbols::{ClassSymbol, FunctionSymbol, PropertySymbol, Scope, VariableSymbol};
pub use diagnostics::{Diagnostic, DiagnosticCode, DiagnosticSeverity};
pub use dfa::DfaState;
pub use resolution::{analyze, Resolver};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FirResolvePhase {
    RawFir,
    Imports,
    CompilerRequiredAnnotations,
    CompanionGeneration,
    SuperTypes,
    SealedClassInheritors,
    Types,
    Status,
    ExpectActualMatching,
    Contracts,
    ImplicitTypesBodyResolve,
    ConstantEvaluation,
    AnnotationArguments,
    BodyResolve,
    Checkers,
}
