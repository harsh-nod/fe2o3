//! Additional fresh-envelope checks, never a replacement for V4 admission.

use super::*;

/// Fresh obligations for a caller-supplied mathematical launch envelope.
///
/// The fixed production pipeline establishes the actual target, original source
/// descriptor, selected root and legal coordinate bounds before supplying this
/// envelope. This lowerer record alone authenticates none of those external
/// launch inputs. It retains the actual owner's full report and all discharged
/// reasons, without deleting conflicts or granting new publication authority.
#[derive(Debug, Eq, PartialEq)]
pub struct ProductionFormalMemoryEnvelopeV2 {
    extents: [u64; 3],
    kernel: ProductionFormalMemoryKernelV1,
}

impl ProductionFormalMemoryEnvelopeV2 {
    /// Mathematical per-axis extent used by fresh extraction, not a runtime launch.
    pub const fn extents(&self) -> [u64; 3] {
        self.extents
    }

    /// Unmodified report and exact original source/ranked discharge partition.
    ///
    /// For physical coverage, use this envelope's [`Self::extents`]. The returned
    /// kernel's legacy `witness_extents(module)` helper still derives structural
    /// witness syntax from the source module, not this physical envelope.
    pub const fn kernel(&self) -> &ProductionFormalMemoryKernelV1 {
        &self.kernel
    }
}

impl ProductionFormalMemoryOwnerV1 {
    /// Requires fresh conflict-free extraction over every supplied envelope
    /// before entering the unchanged structural-witness admission policy.
    ///
    /// Envelopes are in the complete original module kernel order. Rank, inactive
    /// axes, static domain coverage, and checked flattened extent use the shared
    /// physical-envelope extractor. Static syntax never truncates the supplied
    /// bound; smaller-than-static bounds remain refused. The source module is
    /// not cloned or changed, and ordinary Exact launch semantics are unchanged.
    /// Every incomplete reason still
    /// requires its existing exact source/ranked discharge. Failure is terminal;
    /// there is no fallback to the smaller witness after a failed envelope.
    ///
    /// These are descriptive mathematical inputs, not authenticated target or
    /// launch authority. The fixed backend obtains them from original descriptor
    /// custody and an authenticated target. Reports are retained and re-derived
    /// with this exact owner; no caller-authored report can enter the result.
    /// The existing V4 evidence remains the original structural-witness policy.
    /// In particular, path-excluded conflict rows are not admitted by this API.
    /// Formal extraction retains its existing resource contract; this method
    /// does not claim full shared-ledger accounting of that legacy extraction.
    pub fn try_admit_for_launch_envelopes_v2(
        semantic_kir: ProductionSemanticKirOwnerV1,
        extents: &[[u64; 3]],
    ) -> Result<Self, ProductionFormalMemoryErrorV1> {
        semantic_kir
            .verify_equivalence()
            .map_err(ProductionFormalMemoryErrorV1::SemanticKir)?;
        if extents.len() != semantic_kir.module().kernels.len() {
            return Err(ProductionFormalMemoryErrorV1::LaunchEnvelopeCount {
                expected: semantic_kir.module().kernels.len(),
                actual: extents.len(),
            });
        }
        let mut envelopes = Vec::with_capacity(extents.len());
        for (kernel, extents) in semantic_kir.module().kernels.iter().zip(extents) {
            envelopes.push(ProductionFormalMemoryEnvelopeV2 {
                extents: *extents,
                kernel: derive_admitted_obligations_for_kernel_at_physical_envelope(
                    &semantic_kir,
                    kernel,
                    *extents,
                )?,
            });
        }
        let kernels = derive_admitted_obligations(&semantic_kir)?;
        let owner = Self {
            semantic_kir,
            kernels,
            launch_envelopes: Some(envelopes.into_boxed_slice()),
        };
        owner.verify_equivalence()?;
        Ok(owner)
    }

    /// Additional original-owner envelope preflight, if the fixed caller used it.
    /// Its presence is not evidence authenticating caller-selected launch bounds.
    pub fn launch_envelopes_v2(&self) -> Option<&[ProductionFormalMemoryEnvelopeV2]> {
        self.launch_envelopes.as_deref()
    }
}

pub(super) fn verify_envelopes(
    owner: &ProductionSemanticKirOwnerV1,
    envelopes: &[ProductionFormalMemoryEnvelopeV2],
) -> Result<(), ProductionFormalMemoryErrorV1> {
    if owner.module().kernels.len() != envelopes.len() {
        return Err(ProductionFormalMemoryErrorV1::ObligationMismatch);
    }
    for (kernel, envelope) in owner.module().kernels.iter().zip(envelopes) {
        let current = derive_admitted_obligations_for_kernel_at_physical_envelope(
            owner,
            kernel,
            envelope.extents,
        )?;
        if current != envelope.kernel {
            return Err(ProductionFormalMemoryErrorV1::ObligationMismatch);
        }
    }
    Ok(())
}
