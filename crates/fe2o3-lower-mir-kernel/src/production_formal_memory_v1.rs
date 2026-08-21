//! Owner-held formal memory admission for verified target-neutral Kernel IR.

use std::{error::Error, fmt};

use fe2o3_kernel_ir::{
    ExplicitLaunchExtent1d, FormalIndexWidth, FormalMemoryIncompleteReason,
    FormalMemoryObligationAnalysis, FormalMemoryObligationError, FormalMemoryObligations,
    InterInvocationConflictRequirement, derive_kernel_memory_obligations,
};

use crate::{ProductionSemanticKirErrorV1, ProductionSemanticKirOwnerV1};

/// The smallest launch that exposes behavior between distinct invocations.
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

/// Move-only owner of exact semantic KIR and complete formal memory obligations.
///
/// Admission uses a two-invocation witness so cross-invocation affine overlap
/// is observable. The retained bounds and alias records are runtime obligations,
/// not evidence about any concrete launch or allocation.
#[must_use = "dropping formal admission abandons the target-neutral safety witness"]
pub struct ProductionFormalMemoryOwnerV1 {
    semantic_kir: ProductionSemanticKirOwnerV1,
    obligations: FormalMemoryObligations,
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
        let obligations = derive_complete_obligations(&semantic_kir)?;
        let owner = Self {
            semantic_kir,
            obligations,
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
        let obligations = derive_complete_obligations(&self.semantic_kir)?;
        if obligations != self.obligations {
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

    /// Returns the fixed two-invocation structural witness extent.
    pub const fn witness_extent(&self) -> u64 {
        PRODUCTION_FORMAL_MEMORY_WITNESS_EXTENT_V1
    }

    /// Formal admission alone never grants artifact or launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

fn derive_complete_obligations(
    semantic_kir: &ProductionSemanticKirOwnerV1,
) -> Result<FormalMemoryObligations, ProductionFormalMemoryErrorV1> {
    let module = semantic_kir.module();
    if module.kernels.len() != 1 {
        return Err(ProductionFormalMemoryErrorV1::KernelCount {
            actual: module.kernels.len(),
        });
    }
    let analysis = derive_kernel_memory_obligations(
        module,
        &module.kernels[0].id,
        ExplicitLaunchExtent1d::Exact(PRODUCTION_FORMAL_MEMORY_WITNESS_EXTENT_V1),
        FormalIndexWidth::Bits64,
    )
    .map_err(ProductionFormalMemoryErrorV1::Analysis)?;
    let FormalMemoryObligationAnalysis::Complete(obligations) = analysis else {
        return Err(ProductionFormalMemoryErrorV1::Incomplete {
            reasons: analysis.incomplete_reasons().to_vec().into_boxed_slice(),
        });
    };
    if !obligations.inter_invocation_conflicts().is_empty() {
        return Err(ProductionFormalMemoryErrorV1::InterInvocationConflicts {
            conflicts: obligations
                .inter_invocation_conflicts()
                .to_vec()
                .into_boxed_slice(),
        });
    }
    Ok(obligations)
}
