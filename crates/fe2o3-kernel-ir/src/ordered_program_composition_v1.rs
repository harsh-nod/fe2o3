//! Structural V17 composition custody, never source, proof or execution authority.
use crate::{
    BlockId, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, Function, Operation,
    VerifiedCanonicalKernelIrIdentityV17, VerifiedCanonicalKernelIrModuleV17,
};
use std::{error::Error, fmt};
#[path = "ordered_program_composition_profile_v1.rs"]
mod profile;
#[cfg(test)]
#[path = "ordered_program_composition_v1_tests.rs"]
mod tests;

pub const ORDERED_PROGRAM_COMPOSITION_MAX_HELPERS_V1: usize = 2;
pub const ORDERED_PROGRAM_COMPOSITION_MAX_CALLS_V1: usize = 8;
pub const ORDERED_PROGRAM_COMPOSITION_MAX_DEFINITIONS_V1: usize = 8;
pub const ORDERED_PROGRAM_COMPOSITION_MAX_OCCURRENCES_V1: usize = 8;
pub const ORDERED_PROGRAM_COMPOSITION_MAX_INSTRUCTIONS_V1: usize = 128;

macro_rules! key {
    ($name:ident) => {
        /// Owner-local inert coordinate. No public unchecked construction or authority.
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(u32);
        impl $name {
            pub const fn ordinal(self) -> u32 {
                self.0
            }
        }
    };
}
key!(OrderedProgramDefinitionKeyV1);
key!(OrderedProgramHelperKeyV1);
key!(OrderedProgramCallKeyV1);
key!(OrderedProgramOccurrenceKeyV1);

/// Coordinates resolve only against the immutable canonical module.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderedProgramSiteV1 {
    function: u32,
    block_ordinal: u32,
    block: BlockId,
    operation: u32,
}
impl OrderedProgramSiteV1 {
    pub const fn function_ordinal(self) -> u32 {
        self.function
    }
    pub const fn block_ordinal(self) -> u32 {
        self.block_ordinal
    }
    pub const fn block(self) -> BlockId {
        self.block
    }
    pub const fn operation_ordinal(self) -> u32 {
        self.operation
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderedProgramDefinitionV1 {
    key: OrderedProgramDefinitionKeyV1,
    site: OrderedProgramSiteV1,
}
impl OrderedProgramDefinitionV1 {
    pub const fn key(self) -> OrderedProgramDefinitionKeyV1 {
        self.key
    }
    pub const fn site(self) -> OrderedProgramSiteV1 {
        self.site
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderedProgramHelperV1 {
    key: OrderedProgramHelperKeyV1,
    function: u32,
}
impl OrderedProgramHelperV1 {
    pub const fn key(self) -> OrderedProgramHelperKeyV1 {
        self.key
    }
    pub const fn function_ordinal(self) -> u32 {
        self.function
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderedProgramCallV1 {
    key: OrderedProgramCallKeyV1,
    site: OrderedProgramSiteV1,
    callee: OrderedProgramHelperKeyV1,
}
impl OrderedProgramCallV1 {
    pub const fn key(self) -> OrderedProgramCallKeyV1 {
        self.key
    }
    pub const fn site(self) -> OrderedProgramSiteV1 {
        self.site
    }
    pub const fn callee(self) -> OrderedProgramHelperKeyV1 {
        self.callee
    }
}
/// One actual prefix execution occurrence. A shared helper definition is not cloned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderedProgramOccurrenceV1 {
    key: OrderedProgramOccurrenceKeyV1,
    root: u32,
    incoming: Option<OrderedProgramCallKeyV1>,
    definition: OrderedProgramDefinitionKeyV1,
}
impl OrderedProgramOccurrenceV1 {
    pub const fn key(self) -> OrderedProgramOccurrenceKeyV1 {
        self.key
    }
    pub const fn root_function_ordinal(self) -> u32 {
        self.root
    }
    pub const fn incoming_call(self) -> Option<OrderedProgramCallKeyV1> {
        self.incoming
    }
    pub const fn definition(self) -> OrderedProgramDefinitionKeyV1 {
        self.definition
    }
}

/// Move-only structural owner over one exact V17 canonical module.
///
/// It validates the finite composition profile, not authenticated source or Rust
/// FnABI, output-memory safety, physical allocation, native execution or launch.
///
/// ```compile_fail
/// use fe2o3_kernel_ir::VerifiedOrderedProgramCompositionV1;
/// fn clone_it(owner: VerifiedOrderedProgramCompositionV1) { let _ = owner.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_ir::VerifiedOrderedProgramCompositionV1;
/// fn mutate(owner: &mut VerifiedOrderedProgramCompositionV1) {
///     owner.canonical().module().functions.clear();
/// }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct VerifiedOrderedProgramCompositionV1 {
    canonical: VerifiedCanonicalKernelIrModuleV17,
    root: u32,
    definitions: Vec<OrderedProgramDefinitionV1>,
    helpers: Vec<OrderedProgramHelperV1>,
    calls: Vec<OrderedProgramCallV1>,
    occurrences: Vec<OrderedProgramOccurrenceV1>,
    instructions: usize,
}
impl VerifiedOrderedProgramCompositionV1 {
    /// Consumes the exact canonical owner without cloning its graph or programs.
    /// The caller retains its existing canonical receipt/reservation. This call
    /// restores its additional storage floor on every Result exit; accepted work,
    /// peak and denial history remain cumulative. On failure the consumed owner
    /// is dropped, so the caller must release its preexisting canonical charge.
    /// On success reserve the returned *extra* roster receipt before retaining it.
    pub fn try_from_canonical_v17(
        canonical: VerifiedCanonicalKernelIrModuleV17,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, OrderedProgramCompositionStorageV1), OrderedProgramCompositionErrorV1> {
        profile::build(canonical, budget)
    }
    pub const fn canonical(&self) -> &VerifiedCanonicalKernelIrModuleV17 {
        &self.canonical
    }
    pub const fn root_function_ordinal(&self) -> u32 {
        self.root
    }
    pub fn definitions(&self) -> &[OrderedProgramDefinitionV1] {
        &self.definitions
    }
    pub fn helpers(&self) -> &[OrderedProgramHelperV1] {
        &self.helpers
    }
    pub fn calls(&self) -> &[OrderedProgramCallV1] {
        &self.calls
    }
    pub fn occurrences(&self) -> &[OrderedProgramOccurrenceV1] {
        &self.occurrences
    }
    pub const fn expanded_instruction_count(&self) -> usize {
        self.instructions
    }

    fn identity(
        &self,
        expected: &VerifiedCanonicalKernelIrIdentityV17,
        budget: &mut Budget<'_>,
    ) -> Result<(), OrderedProgramCompositionErrorV1> {
        budget.charge_work(33)?;
        if expected != self.canonical.identity() {
            return Err(OrderedProgramCompositionErrorV1::Identity);
        }
        Ok(())
    }
    fn operation(
        &self,
        site: OrderedProgramSiteV1,
    ) -> Result<&Operation, OrderedProgramCompositionErrorV1> {
        let block = self
            .canonical
            .module()
            .functions
            .get(site.function as usize)
            .and_then(|f| f.body.as_ref())
            .and_then(|body| body.blocks.get(site.block_ordinal as usize))
            .filter(|block| block.id == site.block)
            .ok_or(OrderedProgramCompositionErrorV1::Coordinate)?;
        block
            .operations
            .get(site.operation as usize)
            .ok_or(OrderedProgramCompositionErrorV1::Coordinate)
    }
    /// Expected identity plus private key reacquire only this structural view.
    /// Equal bytes do not authenticate source or a previously selected source span.
    pub fn definition_operation(
        &self,
        expected: &VerifiedCanonicalKernelIrIdentityV17,
        key: OrderedProgramDefinitionKeyV1,
        budget: &mut Budget<'_>,
    ) -> Result<&Operation, OrderedProgramCompositionErrorV1> {
        self.identity(expected, budget)?;
        let row = self
            .definitions
            .get(key.0 as usize)
            .ok_or(OrderedProgramCompositionErrorV1::Coordinate)?;
        self.operation(row.site)
    }
    pub fn call_operation(
        &self,
        expected: &VerifiedCanonicalKernelIrIdentityV17,
        key: OrderedProgramCallKeyV1,
        budget: &mut Budget<'_>,
    ) -> Result<&Operation, OrderedProgramCompositionErrorV1> {
        self.identity(expected, budget)?;
        let row = self
            .calls
            .get(key.0 as usize)
            .ok_or(OrderedProgramCompositionErrorV1::Coordinate)?;
        self.operation(row.site)
    }
    pub fn helper_function(
        &self,
        expected: &VerifiedCanonicalKernelIrIdentityV17,
        key: OrderedProgramHelperKeyV1,
        budget: &mut Budget<'_>,
    ) -> Result<&Function, OrderedProgramCompositionErrorV1> {
        self.identity(expected, budget)?;
        let row = self
            .helpers
            .get(key.0 as usize)
            .ok_or(OrderedProgramCompositionErrorV1::Coordinate)?;
        self.canonical
            .module()
            .functions
            .get(row.function as usize)
            .ok_or(OrderedProgramCompositionErrorV1::Coordinate)
    }
    pub fn occurrence(
        &self,
        expected: &VerifiedCanonicalKernelIrIdentityV17,
        key: OrderedProgramOccurrenceKeyV1,
        budget: &mut Budget<'_>,
    ) -> Result<&OrderedProgramOccurrenceV1, OrderedProgramCompositionErrorV1> {
        self.identity(expected, budget)?;
        self.occurrences
            .get(key.0 as usize)
            .ok_or(OrderedProgramCompositionErrorV1::Coordinate)
    }
}

/// Extra inline roster headers and heap row capacities only; canonical storage excluded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderedProgramCompositionStorageV1 {
    retained: usize,
}
impl OrderedProgramCompositionStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrderedProgramCompositionErrorV1 {
    Resource(Resource),
    Root,
    FunctionRole,
    HelperSignature,
    HelperEffect,
    UnsupportedOperation,
    TargetCapabilities,
    Launch,
    DefinitionLimit,
    CallLimit,
    OccurrenceLimit,
    InstructionLimit,
    MissingProgram,
    UnreferencedHelper,
    CallTarget,
    PrefixControlFlow,
    HelperControlFlow,
    ProgramShape,
    Coordinate,
    Identity,
}
impl From<Resource> for OrderedProgramCompositionErrorV1 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for OrderedProgramCompositionErrorV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(out),
            _ => write!(out, "ordered-program composition profile refused: {self:?}"),
        }
    }
}
impl Error for OrderedProgramCompositionErrorV1 {}
