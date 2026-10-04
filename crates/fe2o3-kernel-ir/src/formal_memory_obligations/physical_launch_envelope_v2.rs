//! Descriptive physical-coordinate coverage without changing the source domain.

use super::*;

/// Caller-supplied mathematical bounds for physical invocation coordinates.
///
/// These values do not authenticate a target, dispatch, descriptor or runtime
/// launch. A compiler consumer must independently establish that every actual
/// executing invocation lies in this envelope, with matching target arithmetic.
/// In particular, the caller-selected [`FormalIndexWidth`] is not target custody.
/// This input and its resulting analysis confer no publication authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormalPhysicalLaunchEnvelopeV2 {
    rank: u8,
    extents: [u64; 3],
}

impl FormalPhysicalLaunchEnvelopeV2 {
    /// Creates descriptive bounds; invalid shape or insufficient static coverage
    /// is retained as an incomplete reason by the formal extractor.
    pub const fn new(rank: u8, extents: [u64; 3]) -> Self {
        Self { rank, extents }
    }

    /// Number of active axes in the mathematical coordinate envelope.
    pub const fn rank(self) -> u8 {
        self.rank
    }

    /// Exclusive positive upper bounds, with inactive axes equal to one.
    pub const fn extents(self) -> [u64; 3] {
        self.extents
    }
}

/// Derives fresh obligations over a descriptive physical envelope after ordinary
/// verification of the same immutable module.
///
/// Unlike Exact launch analysis, a Static source extent is a minimum coverage
/// requirement here, never permission to truncate the supplied physical bound.
/// No source is cloned, rewritten or re-encoded. The complete existing access,
/// incomplete-reason and conflict derivation is shared with Exact analysis.
/// This API does not authenticate the supplied envelope or grant launch rights.
pub fn derive_kernel_memory_obligations_for_physical_envelope_v2(
    module: &Module,
    kernel_id: &KernelId,
    envelope: FormalPhysicalLaunchEnvelopeV2,
    index_width: FormalIndexWidth,
) -> Result<FormalMemoryObligationAnalysis, FormalMemoryObligationError> {
    let verified = verify_module_ref(module).map_err(FormalMemoryObligationError::InvalidModule)?;
    derive_kernel_memory_obligations_from_verified_for_physical_envelope_v2(
        verified,
        kernel_id,
        envelope,
        index_width,
    )
}

/// Derives obligations from an existing verification token over the exact same
/// immutable module, using descriptive physical coverage instead of Exact launch
/// equality. All old Exact entry points retain their original behavior.
///
/// A larger bound includes all accesses and pair candidates from that full
/// coordinate interval, including padded or additional workgroups. It does not
/// discharge conflicts, unsupported effects or an unknown target index width.
pub fn derive_kernel_memory_obligations_from_verified_for_physical_envelope_v2(
    verified: VerifiedKernelIrModuleV1<'_>,
    kernel_id: &KernelId,
    envelope: FormalPhysicalLaunchEnvelopeV2,
    index_width: FormalIndexWidth,
) -> Result<FormalMemoryObligationAnalysis, FormalMemoryObligationError> {
    let module = verified.module();
    let effects = analyze_interprocedural_effects_from_verified_v1(verified)
        .expect("verified module remains valid while deriving effect summaries");
    derive_kernel_memory_obligations_with_launch_interpretation(
        module,
        kernel_id,
        ExplicitLaunchExtent::Exact {
            rank: envelope.rank,
            extents: envelope.extents,
        },
        index_width,
        None,
        None,
        &effects,
        PhysicalLaunchInterpretationV2::Envelope,
    )
}

#[cfg(test)]
#[path = "physical_launch_envelope_v2_tests.rs"]
mod tests;
