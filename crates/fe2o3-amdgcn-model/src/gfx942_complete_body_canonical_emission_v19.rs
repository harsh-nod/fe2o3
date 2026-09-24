//! Exact V19 canonical graph to the existing checked complete-body renderer.
//!
//! This is inert LLVM text with a content-bound correspondence, not authentic
//! source custody, native ABI qualification, proof, or artifact authority.
//! There is no caller-supplied plan and no packed-source reinterpretation.
use crate::{
    GFX942_COMPLETE_BODY_ASSEMBLY_BYTES_V1, GFX942_COMPLETE_BODY_LLVM_BYTES_V1,
    GFX942_COMPLETE_BODY_RENDER_WORK_V1, GFX942_COMPLETE_BODY_VALIDATION_WORK_V1,
    Gfx942CompleteBodyEmissionErrorV1, Gfx942CompleteBodyEmissionV1, Gfx942CompleteBodyErrorV1,
    Gfx942CompleteBodyPlanV1, Gfx942CompleteBodySymbolV1,
};
use fe2o3_kernel_ir::{
    BlockId, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, ValueId,
    VerifiedCanonicalKernelIrIdentityV19, VerifiedCanonicalKernelIrModuleV19,
};
use std::{error::Error, fmt};

#[path = "gfx942_complete_body_canonical_extract_v19.rs"]
mod extract;
#[cfg(test)]
#[path = "gfx942_complete_body_canonical_emission_v19_tests.rs"]
mod tests;

/// Fixed upper debit for closed graph extraction, checking, and rendering.
/// No new Work meter is created; the caller's cumulative ledger is charged.
pub const GFX942_COMPLETE_BODY_CANONICAL_EMISSION_WORK_V19: usize =
    16_384 + GFX942_COMPLETE_BODY_VALIDATION_WORK_V1 + GFX942_COMPLETE_BODY_RENDER_WORK_V1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942CompleteBodyCanonicalEmissionErrorV19 {
    Resource(Resource),
    Profile(&'static str),
    Plan(Gfx942CompleteBodyErrorV1),
    Render(Gfx942CompleteBodyEmissionErrorV1),
}
impl From<Resource> for Gfx942CompleteBodyCanonicalEmissionErrorV19 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for Gfx942CompleteBodyCanonicalEmissionErrorV19 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "canonical V19 complete-body emission refused: {self:?}"
        )
    }
}
impl Error for Gfx942CompleteBodyCanonicalEmissionErrorV19 {}
type Result<T> = std::result::Result<T, Gfx942CompleteBodyCanonicalEmissionErrorV19>;

/// Distinguishes authored assembly from compiler-owned ABI/SSA plumbing.
/// Assembly line coordinates are zero based, not machine instructions or PCs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942CompleteBodyCanonicalOperationLoweringV19 {
    Declaration,
    AuthoredInstruction {
        descriptor: u16,
        assembly_line: u16,
    },
    SelectorZero {
        assembly_compare_line: u16,
    },
    SelectorCompare {
        assembly_compare_line: u16,
    },
    GlobalIndexInLlvmShell,
    OutputLengthAbiParameter,
    BoundsCompare {
        assembly_line: u16,
    },
    OutputPointerAbiParameter,
    OutputElementAddress {
        first_assembly_line: u16,
        assembly_line_count: u8,
    },
    GuardedOutputStore {
        assembly_line: u16,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942CompleteBodyCanonicalOperationCorrespondenceV19 {
    pub block: BlockId,
    pub operation_ordinal: u8,
    pub result: Option<ValueId>,
    pub lowering: Gfx942CompleteBodyCanonicalOperationLoweringV19,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942CompleteBodyCanonicalBlockCorrespondenceV19 {
    pub block: BlockId,
    pub authored_label: u8,
    pub assembly_label_line: u16,
    pub terminator_first_assembly_line: u16,
    pub terminator_assembly_line_count: u8,
}

/// Read-only output retains the exact input content identity. It does not own or
/// authenticate source provenance. No mutable graph or plan can be substituted.
#[derive(Debug, Eq, PartialEq)]
pub struct Gfx942CompleteBodyCanonicalEmissionV19 {
    identity: VerifiedCanonicalKernelIrIdentityV19,
    emission: Gfx942CompleteBodyEmissionV1,
    operations: [Option<Gfx942CompleteBodyCanonicalOperationCorrespondenceV19>; 25],
    blocks: [Option<Gfx942CompleteBodyCanonicalBlockCorrespondenceV19>; 4],
}
impl Gfx942CompleteBodyCanonicalEmissionV19 {
    pub const fn canonical_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV19 {
        &self.identity
    }
    pub fn llvm_ir(&self) -> &str {
        self.emission.llvm_ir()
    }
    pub const fn emission(&self) -> &Gfx942CompleteBodyEmissionV1 {
        &self.emission
    }
    pub fn operations(
        &self,
    ) -> impl Iterator<Item = &Gfx942CompleteBodyCanonicalOperationCorrespondenceV19> {
        self.operations.iter().flatten()
    }
    pub fn blocks(
        &self,
    ) -> impl Iterator<Item = &Gfx942CompleteBodyCanonicalBlockCorrespondenceV19> {
        self.blocks.iter().flatten()
    }
    /// Demotes this inert correspondence to ordinary owned text. Any caller
    /// retaining the original receipt remains conservatively over-accounted
    /// until releasing it; this does not grant a protected continuation.
    pub fn into_llvm_ir(self) -> String {
        self.emission.into_llvm_ir_v19()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942CompleteBodyCanonicalEmissionStorageV19 {
    retained: usize,
}
impl Gfx942CompleteBodyCanonicalEmissionStorageV19 {
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}

/// Emits the actual immutable V19 subject, for one root with exact gfx942:xnack-,
/// Wave64 and workgroup64x1x1. Source custody and later native checks stay with
/// the caller; an arbitrary verified V19 graph is not an authenticated source.
///
/// The caller must already reserve the V19 owner's retained receipt while it
/// lives. This operation preserves that whole incoming storage floor on every
/// Result exit and unwind. Success returns an UNRESERVED output receipt; reserve
/// it before retaining the output alongside later allocations. Work, peaks and
/// denied reservations remain cumulative. Logical payload is not allocator RSS.
pub fn lower_canonical_v19_compiler_module_to_gfx942_xnack_minus_llvm_ir(
    owner: &VerifiedCanonicalKernelIrModuleV19,
    budget: &mut Budget<'_>,
) -> Result<(
    Gfx942CompleteBodyCanonicalEmissionV19,
    Gfx942CompleteBodyCanonicalEmissionStorageV19,
)> {
    let floor = budget.storage();
    let retained = std::mem::size_of::<Gfx942CompleteBodyCanonicalEmissionV19>()
        .checked_add(GFX942_COMPLETE_BODY_ASSEMBLY_BYTES_V1)
        .and_then(|value| value.checked_add(GFX942_COMPLETE_BODY_LLVM_BYTES_V1))
        .ok_or(Resource::Arithmetic)?;
    // Fixed arrays, the checked plan, borrowed block views and returned payload
    // coexist. No scratch Vec/String or canonical graph clone is constructed.
    let scratch = extract::scratch_storage()
        .checked_add(std::mem::size_of::<Gfx942CompleteBodyPlanV1>())
        .and_then(|value| value.checked_add(retained))
        .ok_or(Resource::Arithmetic)?;
    budget.with_prepaid_scope(
        floor,
        1,
        GFX942_COMPLETE_BODY_CANONICAL_EMISSION_WORK_V19,
        scratch,
        |_budget| {
            let extracted = extract::extract(owner.module())?;
            let symbol = Gfx942CompleteBodySymbolV1::new(extracted.symbol)
                .map_err(Gfx942CompleteBodyCanonicalEmissionErrorV19::Render)?;
            // These private entry points perform the identical checks/rendering
            // as the existing public inert APIs. Their full work debit was made
            // above on this same ledger; no reset/local budget is introduced.
            let plan = extracted.checked_plan()?;
            let emission =
                crate::gfx942_complete_body_emission_v1::render_prepaid_v19(&plan, symbol)
                    .map_err(Gfx942CompleteBodyCanonicalEmissionErrorV19::Render)?;
            if emission.retained_text_capacity_v19()
                != GFX942_COMPLETE_BODY_ASSEMBLY_BYTES_V1 + GFX942_COMPLETE_BODY_LLVM_BYTES_V1
            {
                return Err(Gfx942CompleteBodyCanonicalEmissionErrorV19::Render(
                    Gfx942CompleteBodyEmissionErrorV1::Allocation,
                ));
            }
            let (operations, blocks) = extracted.correspondence(&emission)?;
            Ok((
                Gfx942CompleteBodyCanonicalEmissionV19 {
                    identity: *owner.identity(),
                    emission,
                    operations,
                    blocks,
                },
                Gfx942CompleteBodyCanonicalEmissionStorageV19 { retained },
            ))
        },
    )
}
