//! Closed fill recognition on the recovered native V5 source/F owner.

use super::{ConditionalFillProgramErrorV1, check_module};
use crate::{
    ProductionConditionalFormulaReportV2, RecoveredCompilerConditionalNativeSemanticHandoffV5,
};
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_compiler_ffi::INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V5;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, Module,
    VerifiedCanonicalKernelIrIdentityV12,
};
use std::{fmt, mem::size_of};

// The recognizer admits at most 64 blocks and 64 operations. This conservative
// logical allowance covers its bounded maps, operand recipe and result headers;
// it does not claim allocator/RSS accounting for the existing recognizer.
const SCRATCH: usize = 128 * 1024;
const ENTRY_WORK: usize = 8;
const FIXED_WORK: usize = 64 * 1024;

/// A complete singleton fill recognized on the actual recovered V5 owner.
///
/// The owner retains strict conditional CPU/source/F replay and its V2 formula.
/// This adds an exact final-program restriction; it neither replaces that source
/// relation with the older singleton recipe nor authenticates compiler origin.
/// Machine refinement, currentness, invocation premises and runtime admission
/// remain separate obligations. There is deliberately no legacy conversion.
///
/// ```compile_fail
/// use fe2o3_verifier::{CheckedNativeConditionalFillProgramV1 as Native,
///     CheckedConditionalFillProgramV1 as Legacy};
/// fn erase<'a>(value: Native<'a>) -> Legacy<'a> { value.into() }
/// ```
/// ```compile_fail
/// use fe2o3_verifier::CheckedNativeConditionalFillProgramV1 as Checked;
/// fn duplicate(value: Checked<'_>) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_verifier::CheckedNativeConditionalFillProgramV1 as Checked;
/// fn forge<'a>() -> Checked<'a> { Checked::default() }
/// ```
#[must_use = "reserve the returned additional storage while retaining this view"]
pub struct CheckedNativeConditionalFillProgramV1<'a> {
    owner: &'a RecoveredCompilerConditionalNativeSemanticHandoffV5,
    formula: ProductionConditionalFormulaReportV2,
    store: (u32, u32),
}

impl CheckedNativeConditionalFillProgramV1<'_> {
    pub(crate) fn final_recipe(
        &self,
    ) -> Result<super::recipe::Recipe, ConditionalFillProgramErrorV1> {
        check_module(self.owner.output().module()).map(|shape| shape.recipe)
    }

    pub const fn owner(&self) -> &RecoveredCompilerConditionalNativeSemanticHandoffV5 {
        self.owner
    }

    pub const fn formula_report(&self) -> ProductionConditionalFormulaReportV2 {
        self.formula
    }

    pub const fn final_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV12 {
        self.owner.output().canonical().identity()
    }

    pub fn function_symbol(&self) -> &str {
        self.owner.output().module().kernels[0].entry.as_str()
    }

    pub const fn store_location(&self) -> (u32, u32) {
        self.store
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn authenticates_currentness(&self) -> bool {
        false
    }

    pub const fn proves_machine_execution(&self) -> bool {
        false
    }

    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }
}

/// Additional view header storage, unreserved on return. No graph or source is
/// copied. The original recovered owner and complete handoff backing stay paid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeConditionalFillProgramStorageV1(usize);

impl NativeConditionalFillProgramStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeConditionalFillProgramErrorV1 {
    Resource(Resource),
    Profile,
    Program(ConditionalFillProgramErrorV1),
}

impl From<Resource> for NativeConditionalFillProgramErrorV1 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}

impl fmt::Display for NativeConditionalFillProgramErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native conditional fill program rejected: {self:?}")
    }
}

impl std::error::Error for NativeConditionalFillProgramErrorV1 {}

type Error = NativeConditionalFillProgramErrorV1;

/// Checks the complete already-recovered V5 source/F owner on the caller's same
/// account. Caller must retain the owner and prepay its full recovery charge,
/// complete transport backing and decode metadata before calling. No recovery,
/// optimizer, signature import, machine analysis or external callback is rerun.
///
/// This bounded local check restores temporary storage on success/refusal/unwind
/// while preserving work and denial history. It must not wrap the preceding
/// terminal V5 recovery in a refundable scope. Reserve the returned header charge
/// before retaining the view. This is accounting, not original-ledger authority.
pub fn check_native_conditional_fill_program_v1<'a>(
    owner: &'a RecoveredCompilerConditionalNativeSemanticHandoffV5,
    budget: &mut Budget<'_>,
) -> Result<
    (
        CheckedNativeConditionalFillProgramV1<'a>,
        NativeConditionalFillProgramStorageV1,
    ),
    Error,
> {
    let input_floor = input_storage(owner)?;
    let source = owner.source();
    let formula = source.formula_report(0);
    let store = check_prepaid(
        owner.profile(),
        source.root_count(),
        source.canonical_kernel_order(),
        formula.is_some(),
        owner.output().module(),
        owner.output().canonical().canonical_bytes().len(),
        input_floor,
        budget,
    )?;
    Ok((
        CheckedNativeConditionalFillProgramV1 {
            owner,
            formula: formula.ok_or(Error::Profile)?,
            store,
        },
        NativeConditionalFillProgramStorageV1(size_of::<CheckedNativeConditionalFillProgramV1>()),
    ))
}

pub(crate) fn input_storage(
    owner: &RecoveredCompilerConditionalNativeSemanticHandoffV5,
) -> Result<usize, Resource> {
    owner
        .storage()
        .retained_storage()
        .checked_add(owner.handoff().backing_capacity())
        .and_then(|n| {
            n.checked_add(INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V5)
        })
        .ok_or(Resource::Arithmetic)
}

#[allow(clippy::too_many_arguments)]
fn check_prepaid(
    profile: Profile,
    roots: usize,
    order: &[u32],
    has_formula: bool,
    module: &Module,
    canonical_length: usize,
    input_floor: usize,
    budget: &mut Budget<'_>,
) -> Result<(u32, u32), Error> {
    let work = canonical_length
        .checked_add(FIXED_WORK)
        .ok_or(Resource::Arithmetic)?;
    budget.with_prepaid_scope(input_floor, ENTRY_WORK, work, SCRATCH, |_| {
        if profile != Profile::Gfx942 || roots != 1 || order != [0] || !has_formula {
            return Err(Error::Profile);
        }
        // The retained V5 replay already binds the complete source roster to F.
        // Reject any additional helper, kernel, effect or non-profile operation.
        let shape = check_module(module).map_err(Error::Program)?;
        Ok(shape.store)
    })
}

#[cfg(test)]
mod tests;
