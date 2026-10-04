//! Exact V18/V53 native text content, never semantic or artifact authority.
use crate::{
    MAX_COMPILER_MODULE_TEXT_BYTES, NativeV12TextDescriptorReplayErrorV3 as TextError,
    lower_canonical_v18_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1 as lower_942,
    lower_canonical_v18_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1 as lower_950,
    native_v12_text_descriptor_replay_v3::{compare_text_for, engine_text},
};
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    VerifiedCanonicalKernelIrModuleV18 as Owner,
};
use std::{mem::size_of, panic::AssertUnwindSafe};

const PREFIX: &[u8] =
    b"\nmodule asm \".section .fe2o3.kd.v53,\\22\\22,@progbits\"\nmodule asm \".balign 8\"\n";

/// A content replay refusal. No variant represents a proof decision.
#[derive(Debug)]
pub enum NativeV18TextDescriptorReplayErrorV60 {
    /// Original sticky resource refusal.
    Resource(Resource),
    /// Existing bounded emitter or exact text-comparison refusal.
    Text(TextError),
    /// Actual lowering receipt or complete text differs.
    Binding(&'static str),
}
type Error = NativeV18TextDescriptorReplayErrorV60;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "native V18/V53 text replay: {self:?}")
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Text(error) => Some(error),
            Self::Binding(_) => None,
        }
    }
}

fn headers() -> usize {
    size_of::<(&Owner, Profile, &[u8], &[u8], &str)>()
        + size_of::<AssertUnwindSafe<&mut Budget<'_>>>()
        + size_of::<Budget<'_>>()
        + 8 * size_of::<usize>()
        + 2 * size_of::<Result<(), Error>>()
        + size_of::<std::thread::Result<Result<(), Error>>>()
        + size_of::<Result<(String, usize), TextError>>()
}

/// Replays the actual borrowed V18 owner once, compares the complete lowering
/// receipt, then streams the exact V53 descriptor suffix without rebuilding it.
/// The caller independently admits the profile, descriptor and their graph join.
/// Borrowed backing stays prepaid on the original ledger. Returned text capacity
/// and new comparison scratch use that ledger; the native engine retains its
/// existing separate bounded allocation/algorithm policy. Unit grants no proof,
/// producer authentication, currentness, publication, load or launch authority.
pub fn check_native_v18_text_descriptor_relation_v60(
    owner: &Owner,
    profile: Profile,
    lowering: &[u8],
    descriptor: &[u8],
    final_llvm: &str,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    budget.check_prior_denials_v1()?;
    let backing = [
        owner.canonical_bytes().len(),
        lowering.len(),
        descriptor.len(),
        final_llvm.len(),
    ]
    .into_iter()
    .try_fold(0usize, |a, b| a.checked_add(b))
    .ok_or(Resource::Arithmetic)?;
    budget.with_prepaid_scope(backing, 1, 1, headers(), |budget| {
        budget.charge_work(3)?;
        if final_llvm.is_empty()
            || final_llvm.len() > MAX_COMPILER_MODULE_TEXT_BYTES
            || lowering.is_empty()
            || lowering.len() > MAX_COMPILER_MODULE_TEXT_BYTES
        {
            return Err(Error::Binding("bounded complete native text"));
        }
        let (text, retained) = engine_text(MAX_COMPILER_MODULE_TEXT_BYTES, budget, || {
            match profile {
                Profile::Gfx942 => lower_942(owner),
                Profile::Gfx950 => lower_950(owner),
            }
            .map_err(TextError::Lowering)
        })
        .map_err(Error::Text)?;
        budget.charge_work(
            text.len()
                .checked_mul(2)
                .and_then(|n| n.checked_add(lowering.len()))
                .ok_or(Resource::Arithmetic)?,
        )?;
        if text.as_bytes() != lowering || text.contains(".fe2o3.kd.") {
            return Err(Error::Binding("exact actual-owner lowering receipt"));
        }
        compare_text_for(
            text.as_bytes(),
            descriptor,
            final_llvm.as_bytes(),
            PREFIX,
            budget,
        )
        .map_err(Error::Text)?;
        drop(text);
        budget.release_storage(retained)?;
        Ok(())
    })
}

#[cfg(test)]
#[path = "native_v18_text_descriptor_replay_v60_tests.rs"]
mod tests;
