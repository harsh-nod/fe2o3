//! Owner-held formal memory admission for verified target-neutral Kernel IR.

use std::{error::Error, fmt};

use fe2o3_kernel_ir::{
    ExplicitLaunchExtent, FormalIndexWidth, FormalMemoryIncompleteReason,
    FormalMemoryObligationAnalysis, FormalMemoryObligationError, FormalMemoryObligations,
    InterInvocationConflictRequirement, LaunchDomain, LaunchExtent,
    derive_kernel_memory_obligations_for_launch,
};

use crate::{ProductionSemanticKirErrorV1, ProductionSemanticKirOwnerV1};

/// The per-active-axis extent of the smallest structural witness launch.
pub const PRODUCTION_FORMAL_MEMORY_WITNESS_EXTENT_V1: u64 = 2;

/// Fail-closed diagnostics from production formal-memory admission.
#[derive(Debug)]
pub enum ProductionFormalMemoryErrorV1 {
    /// The retained semantic-to-Kernel-IR owner no longer verifies.
    SemanticKir(ProductionSemanticKirErrorV1),
    /// Formal extraction requires exactly one selected kernel.
    KernelCount {
        /// Number of kernels present in the verified module.
        actual: usize,
    },
    /// Formal extraction rejected the verified module or selected kernel.
    Analysis(FormalMemoryObligationError),
    /// At least one memory effect has no complete formal derivation.
    Incomplete {
        /// Canonically ordered reasons formal extraction was incomplete.
        reasons: Box<[FormalMemoryIncompleteReason]>,
    },
    /// The modeled memory accesses contain an inherent cross-invocation conflict.
    InterInvocationConflicts {
        /// Canonically ordered conflicts that prevent race-free admission.
        conflicts: Box<[InterInvocationConflictRequirement]>,
    },
    /// Re-derived obligations no longer match the retained admission witness.
    ObligationMismatch,
}

impl fmt::Display for ProductionFormalMemoryErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SemanticKir(error) => write!(formatter, "verified semantic KIR failed: {error}"),
            Self::KernelCount { actual } => write!(
                formatter,
                "formal memory admission requires exactly one kernel; found {actual}",
            ),
            Self::Analysis(error) => write!(formatter, "formal memory extraction failed: {error}"),
            Self::Incomplete { reasons } => write!(
                formatter,
                "formal memory extraction is incomplete for {} reason(s): {:?}",
                reasons.len(),
                reasons.first(),
            ),
            Self::InterInvocationConflicts { conflicts } => write!(
                formatter,
                "formal memory admission found {} inter-invocation conflict(s)",
                conflicts.len(),
            ),
            Self::ObligationMismatch => formatter.write_str(
                "re-derived formal memory obligations differ from the retained admission witness",
            ),
        }
    }
}

impl Error for ProductionFormalMemoryErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::SemanticKir(error) => Some(error),
            Self::Analysis(error) => Some(error),
            Self::KernelCount { .. }
            | Self::Incomplete { .. }
            | Self::InterInvocationConflicts { .. }
            | Self::ObligationMismatch => None,
        }
    }
}

/// Move-only owner of exact semantic KIR and composed memory-safety evidence.
///
/// Admission uses extent two on every active launch axis so cross-invocation
/// affine overlap is observable in each dimension. Affine effects retain
/// complete formal obligations. Dynamic index expressions may instead be
/// discharged by the exact owner-held ranked bounds/race receipt; no other
/// incomplete formal reason is admitted. Retained bounds and alias records are
/// runtime obligations, not evidence about any concrete launch or allocation.
#[must_use = "dropping formal admission abandons the target-neutral safety witness"]
pub struct ProductionFormalMemoryOwnerV1 {
    semantic_kir: ProductionSemanticKirOwnerV1,
    obligations: FormalMemoryObligations,
    ranked_discharged_reasons: Box<[FormalMemoryIncompleteReason]>,
}

impl fmt::Debug for ProductionFormalMemoryOwnerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionFormalMemoryOwnerV1")
            .field("kernel", self.obligations.kernel())
            .field("allocations", &self.obligations.allocations().len())
            .field("accesses", &self.obligations.accesses().len())
            .field(
                "bounds_requirements",
                &self.obligations.bounds_requirements().len(),
            )
            .field(
                "runtime_alias_requirements",
                &self.obligations.runtime_alias_requirements().len(),
            )
            .field(
                "ranked_discharged_reasons",
                &self.ranked_discharged_reasons.len(),
            )
            .finish_non_exhaustive()
    }
}

impl ProductionFormalMemoryOwnerV1 {
    /// Consumes verified semantic KIR and requires complete, conflict-free
    /// formal extraction for the production witness extent.
    pub fn try_admit(
        semantic_kir: ProductionSemanticKirOwnerV1,
    ) -> Result<Self, ProductionFormalMemoryErrorV1> {
        semantic_kir
            .verify_equivalence()
            .map_err(ProductionFormalMemoryErrorV1::SemanticKir)?;
        let (obligations, ranked_discharged_reasons) = derive_admitted_obligations(&semantic_kir)?;
        let owner = Self {
            semantic_kir,
            obligations,
            ranked_discharged_reasons,
        };
        owner.verify_equivalence()?;
        Ok(owner)
    }

    /// Re-verifies exact semantic KIR and deterministically re-derives the
    /// retained formal obligations.
    pub fn verify_equivalence(&self) -> Result<(), ProductionFormalMemoryErrorV1> {
        self.semantic_kir
            .verify_equivalence()
            .map_err(ProductionFormalMemoryErrorV1::SemanticKir)?;
        let (obligations, ranked_discharged_reasons) =
            derive_admitted_obligations(&self.semantic_kir)?;
        if obligations != self.obligations
            || ranked_discharged_reasons != self.ranked_discharged_reasons
        {
            return Err(ProductionFormalMemoryErrorV1::ObligationMismatch);
        }
        Ok(())
    }

    /// Borrows the exact semantic-to-Kernel-IR owner.
    pub const fn semantic_kir(&self) -> &ProductionSemanticKirOwnerV1 {
        &self.semantic_kir
    }

    /// Borrows complete compiler-derived obligations for the witness extent.
    pub const fn obligations(&self) -> &FormalMemoryObligations {
        &self.obligations
    }

    /// Returns dynamic index derivations discharged by the retained, exact
    /// ranked bounds/race receipt rather than the affine formal engine.
    pub fn ranked_discharged_reasons(&self) -> &[FormalMemoryIncompleteReason] {
        &self.ranked_discharged_reasons
    }

    /// Returns the structural fallback extent used for every dynamic axis.
    pub const fn witness_extent(&self) -> u64 {
        PRODUCTION_FORMAL_MEMORY_WITNESS_EXTENT_V1
    }

    /// Returns exact per-axis extents of the admitted structural witness.
    pub fn witness_extents(&self) -> [u64; 3] {
        witness_extents(&self.semantic_kir.module().kernels[0].domain)
    }

    /// Returns the exact flattened invocation count in the structural witness.
    pub fn witness_invocation_count(&self) -> u64 {
        let Some(invocations) = self.obligations.invocations() else {
            return 0;
        };
        invocations.end_exclusive() - invocations.start()
    }

    /// Formal admission alone never grants artifact or launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

fn derive_admitted_obligations(
    semantic_kir: &ProductionSemanticKirOwnerV1,
) -> Result<
    (FormalMemoryObligations, Box<[FormalMemoryIncompleteReason]>),
    ProductionFormalMemoryErrorV1,
> {
    let module = semantic_kir.module();
    if module.kernels.len() != 1 {
        return Err(ProductionFormalMemoryErrorV1::KernelCount {
            actual: module.kernels.len(),
        });
    }
    let domain = &module.kernels[0].domain;
    let rank = domain.rank();
    let witness = ExplicitLaunchExtent::Exact {
        rank,
        extents: witness_extents(domain),
    };
    let analysis = derive_kernel_memory_obligations_for_launch(
        module,
        &module.kernels[0].id,
        witness,
        FormalIndexWidth::Bits64,
    )
    .map_err(ProductionFormalMemoryErrorV1::Analysis)?;
    let (obligations, ranked_discharged_reasons) = match analysis {
        FormalMemoryObligationAnalysis::Complete(obligations) => {
            (obligations, Vec::new().into_boxed_slice())
        }
        FormalMemoryObligationAnalysis::Incomplete { partial, reasons } => {
            let guarded_locations = reasons
                .iter()
                .filter_map(|reason| match reason {
                    FormalMemoryIncompleteReason::GuardedAccessRequiresRankedProof { location } => {
                        Some(*location)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let unsupported_indices = reasons
                .iter()
                .filter(|reason| {
                    matches!(
                        reason,
                        FormalMemoryIncompleteReason::UnsupportedIndexExpression { .. }
                    )
                })
                .cloned()
                .collect::<Vec<_>>();
            let reasons_are_ranked_dischargeable = reasons.iter().all(|reason| {
                matches!(
                    reason,
                    FormalMemoryIncompleteReason::UnsupportedIndexExpression { .. }
                        | FormalMemoryIncompleteReason::GuardedAccessRequiresRankedProof { .. }
                )
            });
            if !reasons_are_ranked_dischargeable
                || (!unsupported_indices.is_empty()
                    && !semantic_kir.retained_generic_checks_discharge_unsupported_indices(
                        &unsupported_indices,
                    ))
                || (!guarded_locations.is_empty()
                    && !semantic_kir
                        .retained_generic_checks_discharge_guarded_accesses(&guarded_locations))
            {
                return Err(ProductionFormalMemoryErrorV1::Incomplete {
                    reasons: reasons.into_boxed_slice(),
                });
            }
            (partial, reasons.into_boxed_slice())
        }
    };
    if !obligations.inter_invocation_conflicts().is_empty() {
        return Err(ProductionFormalMemoryErrorV1::InterInvocationConflicts {
            conflicts: obligations
                .inter_invocation_conflicts()
                .to_vec()
                .into_boxed_slice(),
        });
    }
    Ok((obligations, ranked_discharged_reasons))
}

fn witness_extents(domain: &LaunchDomain) -> [u64; 3] {
    let mut witness = [1_u64; 3];
    for (axis, extent) in domain.extents().enumerate() {
        witness[axis] = match extent {
            LaunchExtent::Static(extent) => u64::from(extent),
            LaunchExtent::Dynamic => PRODUCTION_FORMAL_MEMORY_WITNESS_EXTENT_V1,
        };
    }
    witness
}
