//! Memory-only interpretation selected by the exact immutable V19 subject.
//! This never authenticates source/launch bindings or changes ordered effects.
use super::{
    ExplicitLaunchExtent, FormalIndexWidth, FormalMemoryObligationAnalysis,
    FormalMemoryObligationError, derive_kernel_memory_obligations_with_v19_context,
};
use crate::{KernelId, OperationKind, VerifiedCanonicalKernelIrModuleV19};

/// Derives ordinary guarded-tail obligations from the actual verified V19 graph.
///
/// Only a canonical owner's actual complete-body declaration/step profile
/// activates the additional no-memory/no-address-effect cases. Those typed U32
/// instructions remain ordered; this function proves neither their arithmetic
/// result nor physical register behavior. Unknown/other operations retain every
/// existing refusal. Ordinary V19 subjects get the unchanged generic analysis,
/// which is not evidence of complete-body profile membership.
///
/// The real store, slice/pointer derivation, affine/race analysis and their
/// resource limits use the existing implementation. The direct-index guarded
/// tail currently retains a conservative LaunchEnvelope and fixed runtime
/// allocation minimum; it does not gain a precise slice-bounded guard domain. A
/// Complete result still carries unauthenticated launch/index inputs and
/// unresolved runtime bounds/alias obligations. It grants no runtime safety,
/// source custody, ranked-proof discharge, native or artifact authority.
pub fn derive_complete_body_memory_obligations_v19(
    owner: &VerifiedCanonicalKernelIrModuleV19,
    kernel_id: &KernelId,
    launch: ExplicitLaunchExtent,
    index_width: FormalIndexWidth,
) -> Result<FormalMemoryObligationAnalysis, FormalMemoryObligationError> {
    derive_kernel_memory_obligations_with_v19_context(
        owner.verified_module_ref_v1(),
        kernel_id,
        launch,
        index_width,
        Some(owner),
    )
}

/// Exact typed custody already ran whole-profile verification. This additionally
/// binds activation to the actual selected complete-body entry, never just a V19
/// header, caller boolean, nonzero origin digest or another equal graph.
pub(super) fn contains_verified_complete_body(
    owner: &VerifiedCanonicalKernelIrModuleV19,
    kernel_id: &KernelId,
) -> bool {
    let module = owner.module();
    let [kernel] = module.kernels.as_slice() else {
        return false;
    };
    let [function] = module.functions.as_slice() else {
        return false;
    };
    if &kernel.id != kernel_id || kernel.entry != function.id {
        return false;
    }
    let Some(body) = &function.body else {
        return false;
    };
    if !matches!(body.blocks.len(), 1 | 4) {
        return false;
    }
    matches!(body.blocks[0].operations.first().map(|operation| &operation.kind),
        Some(OperationKind::Gfx942CompleteBodyDeclaration(declaration))
            if declaration.validate_shape().is_ok()
                && usize::from(declaration.block_count) == body.blocks.len())
}

#[cfg(test)]
#[path = "complete_body_v19_tests.rs"]
mod tests;
