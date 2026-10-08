//! Inert whole-F copy classification for original-owner capture diagnostics.
//!
//! No captured copy machine family is assumed. This neither proves machine
//! execution nor accepts ABI offsets, native DATA, a launch, or a rebind.
//! Only a straight-line chain with one predicated input read and one write
//! guarded by both slice bounds is recognized. Other compiler spellings remain
//! Unsupported until separately reviewed against an actual protected capture.

use crate::RecoveredCompilerConditionalNativeSemanticHandoffV5;
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, Module,
};

mod shape;

const ENTRY_WORK: usize = 8;
const FIXED_WORK: usize = 64 * 1024;
const SCRATCH: usize = 16 * 1024;

/// Copyable diagnostic classification, deliberately constructible and inert.
/// It cannot replace the recovered source owner or any machine/runtime gate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeCopyProgramClassificationV1 {
    /// The complete final program is outside this narrow recognizer.
    Unsupported,
    /// One guarded read and guarded same-coordinate write in final F.
    GuardedU32 {
        /// Final-F block and operation of the unique input load.
        load: [u32; 2],
        /// Final-F block and operation of the unique output store.
        store: [u32; 2],
    },
}

/// Classifies an actual recovered V5 source/F owner with an accounted borrowed
/// storage floor. The caller must supply its original resource account; this
/// check does not authenticate account identity. The source replay and its
/// formula stay retained; no inert bytes or
/// caller-supplied success can stand in for them here. The result is merely an
/// observation for the existing build-only capture manifest, not a new proof.
///
/// This local check uses bounded fixed scratch and restores that scratch even
/// on refusal. It does not wrap or refund prior source recovery. No source/F
/// graph is copied. The by-value diagnostic header belongs to the caller's
/// existing frame. Unsupported shape is not an account denial; resource errors
/// propagate and must not be relabeled as a successful classification.
pub fn classify_native_copy_program_v1(
    owner: &RecoveredCompilerConditionalNativeSemanticHandoffV5,
    budget: &mut Budget<'_>,
) -> Result<NativeCopyProgramClassificationV1, Resource> {
    let floor = crate::conditional_fill_program_v1::native_v1::input_storage(owner)?;
    let source = owner.source();
    classify_prepaid(
        owner.profile(),
        source.root_count(),
        source.canonical_kernel_order(),
        source.formula_report(0).is_some(),
        owner.output().module(),
        owner.output().canonical().canonical_bytes().len(),
        floor,
        budget,
    )
}

#[allow(clippy::too_many_arguments)]
fn classify_prepaid(
    profile: Profile,
    roots: usize,
    order: &[u32],
    formula: bool,
    module: &Module,
    canonical_length: usize,
    floor: usize,
    budget: &mut Budget<'_>,
) -> Result<NativeCopyProgramClassificationV1, Resource> {
    let work = canonical_length
        .checked_add(FIXED_WORK)
        .ok_or(Resource::Arithmetic)?;
    budget.with_prepaid_scope(floor, ENTRY_WORK, work, SCRATCH, |_| {
        if profile != Profile::Gfx942 || roots != 1 || order != [0] || !formula {
            return Ok(NativeCopyProgramClassificationV1::Unsupported);
        }
        Ok(shape::classify(module))
    })
}

#[cfg(test)]
mod tests;
