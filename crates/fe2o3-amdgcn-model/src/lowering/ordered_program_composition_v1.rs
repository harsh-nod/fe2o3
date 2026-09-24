//! Typed inert complete-module emission for a checked structural composition.
use super::*;
use fe2o3_kernel_ir::{
    AssemblySourceIdentity, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, Gfx942OrderedProgramRegistersV1,
    Gfx942U32ProgramV1, OrderedProgramCallKeyV1, OrderedProgramCompositionErrorV1,
    OrderedProgramDefinitionKeyV1, OrderedProgramHelperKeyV1, OrderedProgramSiteV1,
    VerifiedCanonicalKernelIrIdentityV17, VerifiedOrderedProgramCompositionV1,
};
use std::mem::size_of;

#[path = "ordered_program_composition_context_v1.rs"]
mod context;
pub(super) use context::validate_owner_context;
#[path = "ordered_program_composition_resources_v1.rs"]
mod resources;
#[cfg(test)]
#[path = "ordered_program_composition_v1_tests.rs"]
mod tests;

/// Actual canonical definition and the existing renderer's authored program.
/// No LLVM byte range, native PC, runtime register value or source authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderedProgramCompositionRegionEmissionV1 {
    key: OrderedProgramDefinitionKeyV1,
    site: OrderedProgramSiteV1,
    source: AssemblySourceIdentity,
    inputs: [ValueId; 3],
    result: ValueId,
    registers: Gfx942OrderedProgramRegistersV1,
    program: Gfx942U32ProgramV1,
}
impl OrderedProgramCompositionRegionEmissionV1 {
    pub const fn key(self) -> OrderedProgramDefinitionKeyV1 {
        self.key
    }
    pub const fn site(self) -> OrderedProgramSiteV1 {
        self.site
    }
    /// Declared canonical source identity, not authenticated Rust custody.
    pub const fn source(self) -> AssemblySourceIdentity {
        self.source
    }
    pub const fn inputs(&self) -> &[ValueId; 3] {
        &self.inputs
    }
    pub const fn result(self) -> ValueId {
        self.result
    }
    pub const fn registers(self) -> Gfx942OrderedProgramRegistersV1 {
        self.registers
    }
    pub const fn program(&self) -> &Gfx942U32ProgramV1 {
        &self.program
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderedProgramCompositionCallEmissionV1 {
    key: OrderedProgramCallKeyV1,
    site: OrderedProgramSiteV1,
    callee: OrderedProgramHelperKeyV1,
    callee_function: u32,
    arguments: [ValueId; 3],
    result: ValueId,
}
impl OrderedProgramCompositionCallEmissionV1 {
    pub const fn key(self) -> OrderedProgramCallKeyV1 {
        self.key
    }
    pub const fn site(self) -> OrderedProgramSiteV1 {
        self.site
    }
    pub const fn callee(self) -> OrderedProgramHelperKeyV1 {
        self.callee
    }
    pub const fn callee_function_ordinal(self) -> u32 {
        self.callee_function
    }
    pub const fn arguments(&self) -> &[ValueId; 3] {
        &self.arguments
    }
    pub const fn result(self) -> ValueId {
        self.result
    }
}

/// Full LLVM output and immutable same-owner emission relation. Not a source owner.
#[derive(Debug, Eq, PartialEq)]
pub struct OrderedProgramCompositionCanonicalEmissionV1 {
    identity: VerifiedCanonicalKernelIrIdentityV17,
    llvm: String,
    regions: [Option<OrderedProgramCompositionRegionEmissionV1>; 8],
    calls: [Option<OrderedProgramCompositionCallEmissionV1>; 8],
}
impl OrderedProgramCompositionCanonicalEmissionV1 {
    pub const fn canonical_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV17 {
        &self.identity
    }
    pub fn llvm_ir(&self) -> &str {
        &self.llvm
    }
    pub fn region_definitions(
        &self,
    ) -> impl Iterator<Item = &OrderedProgramCompositionRegionEmissionV1> {
        self.regions.iter().flatten()
    }
    pub fn helper_calls(&self) -> impl Iterator<Item = &OrderedProgramCompositionCallEmissionV1> {
        self.calls.iter().flatten()
    }
    /// Inert demotion only. Caller retains the prior receipt while the text lives.
    pub fn into_llvm_ir(self) -> String {
        self.llvm
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderedProgramCompositionEmissionStorageV1 {
    retained: usize,
}
impl OrderedProgramCompositionEmissionStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}
#[derive(Debug, Eq, PartialEq)]
pub enum OrderedProgramCompositionEmissionErrorV1 {
    Resource(Resource),
    Structural(OrderedProgramCompositionErrorV1),
    /// Complete original located diagnostics, never a lossy diagnostic remap.
    Lowering(LoweringErrors),
    Relation(&'static str),
}
impl From<Resource> for OrderedProgramCompositionEmissionErrorV1 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl From<OrderedProgramCompositionErrorV1> for OrderedProgramCompositionEmissionErrorV1 {
    fn from(value: OrderedProgramCompositionErrorV1) -> Self {
        Self::Structural(value)
    }
}
impl fmt::Display for OrderedProgramCompositionEmissionErrorV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(out),
            Self::Structural(error) => error.fmt(out),
            Self::Lowering(error) => error.fmt(out),
            Self::Relation(message) => write!(out, "ordered composition emission: {message}"),
        }
    }
}
impl Error for OrderedProgramCompositionEmissionErrorV1 {}
type E = OrderedProgramCompositionEmissionErrorV1;

/// Reuses the complete-module renderer and existing ordered-program renderer,
/// preserving all ordinary function definitions, calls, scalar operations and
/// root memory/CFG. No caller target, raw Module, LLVM, or assembly plan is accepted.
///
/// The same caller ledger prepays a canonical-size/CFG-derived logical engine
/// envelope plus bounded exact text storage before invoking the existing engine.
/// This is not allocator/RSS accounting or a new semantic interpreter. Success
/// returns an unreserved retained-output receipt; every Result exit restores the
/// incoming storage floor without refunding accepted work/peak/denial history.
/// A failed lowering returns its complete ordinary located diagnostics; these
/// remain inert caller-owned error data, never a retained successful emission.
pub fn lower_ordered_program_composition_to_gfx942_xnack_minus_llvm_ir_v1(
    owner: &VerifiedOrderedProgramCompositionV1,
    budget: &mut Budget<'_>,
) -> Result<
    (
        OrderedProgramCompositionCanonicalEmissionV1,
        OrderedProgramCompositionEmissionStorageV1,
    ),
    E,
> {
    let floor = budget.storage();
    let envelope = resources::envelope(owner, budget)?;
    budget.with_prepaid_scope(floor, 1, envelope.work, envelope.storage, |budget| {
        let mut regions = [None; 8];
        let mut calls = [None; 8];
        for (index, definition) in owner.definitions().iter().enumerate() {
            let operation = owner.definition_operation(
                owner.canonical().identity(),
                definition.key(),
                budget,
            )?;
            let OperationKind::Gfx942OrderedProgram(program) = &operation.kind else {
                return Err(E::Relation("definition no longer names ordered program"));
            };
            let [result] = operation.results.as_slice() else {
                return Err(E::Relation("ordered result arity"));
            };
            regions[index] = Some(OrderedProgramCompositionRegionEmissionV1 {
                key: definition.key(),
                site: definition.site(),
                source: program.source(),
                inputs: *program.inputs(),
                result: result.id,
                registers: program.registers(),
                program: *program.program(),
            });
        }
        for (index, call) in owner.calls().iter().enumerate() {
            let operation =
                owner.call_operation(owner.canonical().identity(), call.key(), budget)?;
            let helper =
                owner.helper_function(owner.canonical().identity(), call.callee(), budget)?;
            let OperationKind::Call { callee, arguments } = &operation.kind else {
                return Err(E::Relation("call no longer names call operation"));
            };
            let [result] = operation.results.as_slice() else {
                return Err(E::Relation("helper return arity"));
            };
            // Exact structural owner has already joined this name. No foreign row is accepted.
            if callee != &helper.id {
                return Err(E::Relation("helper callee identity"));
            }
            let callee_row = owner
                .helpers()
                .get(call.callee().ordinal() as usize)
                .ok_or(E::Relation("helper coordinate"))?;
            calls[index] = Some(OrderedProgramCompositionCallEmissionV1 {
                key: call.key(),
                site: call.site(),
                callee: call.callee(),
                callee_function: callee_row.function_ordinal(),
                arguments: arguments
                    .as_slice()
                    .try_into()
                    .map_err(|_| E::Relation("helper argument arity"))?,
                result: result.id,
            });
        }
        let llvm = lower_compiler_module_with_ordered_context(
            owner.canonical().module(),
            LoweringTarget::Gfx942XnackMinusV1,
            None,
            None,
            true,
            Some(OrderedModuleOwner::CompositionV1(owner)),
        )
        .map_err(E::Lowering)?;
        if llvm.capacity() != MAX_COMPILER_MODULE_TEXT_BYTES
            || llvm.len() > MAX_COMPILER_MODULE_TEXT_BYTES
        {
            return Err(E::Relation("exact bounded text capacity"));
        }
        let retained = size_of::<OrderedProgramCompositionCanonicalEmissionV1>()
            .checked_add(llvm.capacity())
            .ok_or(Resource::Arithmetic)?;
        Ok((
            OrderedProgramCompositionCanonicalEmissionV1 {
                identity: *owner.canonical().identity(),
                llvm,
                regions,
                calls,
            },
            OrderedProgramCompositionEmissionStorageV1 { retained },
        ))
    })
}
