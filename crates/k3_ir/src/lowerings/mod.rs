pub mod coroutines;
pub mod default_args;
pub mod inlining;
pub mod value_classes;

use crate::ir::IrFile;
pub use coroutines::AddContinuationLowering;
pub use default_args::DefaultArgumentStubLowering;
pub use inlining::FunctionInliningLowering;
pub use value_classes::ValueClassLowering;

/// The central Lowering Pipeline in Kotlin3.
/// Orchestrates AST-level and IR-level transformations before backend emission.
pub struct LoweringPipeline {
    pub add_continuation: AddContinuationLowering,
    pub default_args: DefaultArgumentStubLowering,
    pub inlining: FunctionInliningLowering,
    pub value_classes: ValueClassLowering,
}

impl LoweringPipeline {
    pub fn new() -> Self {
        Self {
            add_continuation: AddContinuationLowering::new(),
            default_args: DefaultArgumentStubLowering::new(),
            inlining: FunctionInliningLowering::new(),
            value_classes: ValueClassLowering::new(),
        }
    }

    /// Executes all lowering transformations on the given IR file sequentially.
    pub fn run(&self, file: &mut IrFile) {
        // 1. Coroutines CPS state-machine lowering
        self.add_continuation.lower_file(file);

        // 2. Default argument synthetic $default stubs
        self.default_args.lower_file(file);

        // 3. Inline functions & lambdas
        self.inlining.lower_file(file);

        // 4. Value classes primitive unboxing
        self.value_classes.lower_file(file);
    }
}

impl Default for LoweringPipeline {
    fn default() -> Self {
        Self::new()
    }
}
