pub mod cone_types;

pub use cone_types::{ConeKotlinType, ConeNullability, ConeProjection};

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
