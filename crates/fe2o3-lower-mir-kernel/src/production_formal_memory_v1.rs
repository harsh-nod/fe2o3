//! Owner-held formal memory admission for verified target-neutral Kernel IR.

use std::{error::Error, fmt};

use fe2o3_kernel_ir::{
    ExplicitLaunchExtent, FormalIndexWidth, FormalMemoryIncompleteReason,
    FormalMemoryObligationAnalysis, FormalMemoryObligationError, FormalMemoryObligations,
    InterInvocationConflictRequirement, LaunchDomain, LaunchExtent,
    derive_kernel_memory_obligations_for_launch,
};

use crate::{
    ProductionMemoryDischargeFailureV1, ProductionSemanticKirErrorV1, ProductionSemanticKirOwnerV1,
};

/// The per-active-axis extent of the smallest structural witness launch.
pub const PRODUCTION_FORMAL_MEMORY_WITNESS_EXTENT_V1: u64 = 2;
const FORMAL_REASON_DIAGNOSTIC_LIMIT_V1: usize = 16;

fn format_formal_reasons(
    formatter: &mut fmt::Formatter<'_>,
    reasons: &[FormalMemoryIncompleteReason],
) -> fmt::Result {
    formatter
        .debug_list()
        .entries(reasons.iter().take(FORMAL_REASON_DIAGNOSTIC_LIMIT_V1))
        .finish()?;
    if reasons.len() > FORMAL_REASON_DIAGNOSTIC_LIMIT_V1 {
        write!(
            formatter,
            " ({} more)",
            reasons.len() - FORMAL_REASON_DIAGNOSTIC_LIMIT_V1
        )?;
    }
    Ok(())
}

/// Fail-closed diagnostics from production formal-memory admission.
#[derive(Debug)]
pub enum ProductionFormalMemoryErrorV1 {
    /// The retained semantic-to-Kernel-IR owner no longer verifies.
    SemanticKir(ProductionSemanticKirErrorV1),
    /// Formal extraction requires a nonempty selected-kernel roster.
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
    /// Compiler-owned LDS effects no longer match their exact semantic lowering spans.
    CompilerOwnedWorkgroupDischarge {
        /// Canonically ordered internal workgroup-memory reasons.
        reasons: Box<[FormalMemoryIncompleteReason]>,
        /// Stable failure at the semantic/KIR composition boundary.
        detail: ProductionMemoryDischargeFailureV1,
    },
    /// Ranked checks could not correlate a dynamic index to its exact access.
    UnsupportedIndexDischarge {
        /// Canonically ordered formal reasons that required ranked discharge.
        reasons: Box<[FormalMemoryIncompleteReason]>,
        /// Stable failure at the ranked/Kernel-IR composition boundary.
        detail: ProductionMemoryDischargeFailureV1,
    },
    /// Ranked checks could not discharge structurally guarded accesses.
    GuardedAccessDischarge {
        /// Canonically ordered formal reasons that required ranked discharge.
        reasons: Box<[FormalMemoryIncompleteReason]>,
        /// Stable failure at the ranked/Kernel-IR composition boundary.
        detail: ProductionMemoryDischargeFailureV1,
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
                "formal memory admission requires a nonempty kernel roster; found {actual}",
            ),
            Self::Analysis(error) => write!(formatter, "formal memory extraction failed: {error}"),
            Self::Incomplete { reasons } => {
                write!(
                    formatter,
                    "formal memory extraction is incomplete for {} reason(s): ",
                    reasons.len(),
                )?;
                format_formal_reasons(formatter, reasons)
            }
            Self::CompilerOwnedWorkgroupDischarge { reasons, detail } => {
                write!(
                    formatter,
                    "exact compiler-owned workgroup lowering could not discharge {} internal reason(s): {detail}; locations: ",
                    reasons.len(),
                )?;
                format_formal_reasons(formatter, reasons)
            }
            Self::GuardedAccessDischarge { reasons, detail } => {
                write!(
                    formatter,
                    "ranked checks could not discharge {} guarded access reason(s): {detail}; locations: ",
                    reasons.len(),
                )?;
                format_formal_reasons(formatter, reasons)
            }
            Self::UnsupportedIndexDischarge { reasons, detail } => {
                write!(
                    formatter,
                    "ranked checks could not discharge {} unsupported index reason(s): {detail}; locations: ",
                    reasons.len(),
                )?;
                format_formal_reasons(formatter, reasons)
            }
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
            | Self::CompilerOwnedWorkgroupDischarge { .. }
            | Self::UnsupportedIndexDischarge { .. }
            | Self::GuardedAccessDischarge { .. }
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
    kernels: Box<[ProductionFormalMemoryKernelV1]>,
}

/// Exact formal-memory admission retained for one canonical module kernel.
#[derive(Debug, Eq, PartialEq)]
pub struct ProductionFormalMemoryKernelV1 {
    obligations: FormalMemoryObligations,
    ranked_discharged_reasons: Box<[FormalMemoryIncompleteReason]>,
    compiler_discharged_reasons: Box<[FormalMemoryIncompleteReason]>,
}

impl fmt::Debug for ProductionFormalMemoryOwnerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionFormalMemoryOwnerV1")
            .field("kernels", &self.kernels)
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
        let kernels = derive_admitted_obligations(&semantic_kir)?;
        let owner = Self {
            semantic_kir,
            kernels,
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
        let kernels = derive_admitted_obligations(&self.semantic_kir)?;
        if kernels != self.kernels {
            return Err(ProductionFormalMemoryErrorV1::ObligationMismatch);
        }
        Ok(())
    }

    /// Borrows the exact semantic-to-Kernel-IR owner.
    pub const fn semantic_kir(&self) -> &ProductionSemanticKirOwnerV1 {
        &self.semantic_kir
    }

    /// Borrows complete compiler-derived obligations for the witness extent.
    pub fn obligations(&self) -> Option<&FormalMemoryObligations> {
        let [kernel] = self.kernels.as_ref() else {
            return None;
        };
        Some(&kernel.obligations)
    }

    /// Borrows the complete canonical per-kernel formal roster.
    pub fn kernels(&self) -> &[ProductionFormalMemoryKernelV1] {
        &self.kernels
    }

    /// Resolves exact formal obligations for one kernel identity.
    pub fn obligations_for_kernel(&self, kernel: &str) -> Option<&FormalMemoryObligations> {
        self.kernels
            .iter()
            .find(|evidence| evidence.obligations.kernel().as_str() == kernel)
            .map(|evidence| &evidence.obligations)
    }

    /// Returns dynamic index derivations discharged by the retained, exact
    /// ranked bounds/race receipt rather than the affine formal engine.
    pub fn ranked_discharged_reasons(&self) -> Option<&[FormalMemoryIncompleteReason]> {
        let [kernel] = self.kernels.as_ref() else {
            return None;
        };
        Some(&kernel.ranked_discharged_reasons)
    }

    /// Returns internal LDS effects discharged only by replaying the exact,
    /// compiler-owned collective lowering and its source correspondence.
    pub fn compiler_discharged_reasons(&self) -> Option<&[FormalMemoryIncompleteReason]> {
        let [kernel] = self.kernels.as_ref() else {
            return None;
        };
        Some(&kernel.compiler_discharged_reasons)
    }

    /// Returns the structural fallback extent used for every dynamic axis.
    pub const fn witness_extent(&self) -> u64 {
        PRODUCTION_FORMAL_MEMORY_WITNESS_EXTENT_V1
    }

    /// Returns exact per-axis extents of the admitted structural witness.
    pub fn witness_extents(&self) -> Option<[u64; 3]> {
        let [kernel] = self.semantic_kir.module().kernels.as_slice() else {
            return None;
        };
        Some(witness_extents(&kernel.domain))
    }

    /// Returns the exact flattened invocation count in the structural witness.
    pub fn witness_invocation_count(&self) -> Option<u64> {
        self.obligations().map(|obligations| {
            obligations.invocations().map_or(0, |invocations| {
                invocations.end_exclusive() - invocations.start()
            })
        })
    }

    /// Formal admission alone never grants artifact or launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

impl ProductionFormalMemoryKernelV1 {
    /// Borrows this kernel's complete formal obligations.
    pub const fn obligations(&self) -> &FormalMemoryObligations {
        &self.obligations
    }

    /// Returns dynamic-index reasons discharged by this kernel's ranked proof.
    pub fn ranked_discharged_reasons(&self) -> &[FormalMemoryIncompleteReason] {
        &self.ranked_discharged_reasons
    }

    /// Returns compiler-owned LDS reasons discharged for this kernel.
    pub fn compiler_discharged_reasons(&self) -> &[FormalMemoryIncompleteReason] {
        &self.compiler_discharged_reasons
    }

    /// Returns exact structural witness extents for this kernel.
    pub fn witness_extents(&self, module: &fe2o3_kernel_ir::Module) -> Option<[u64; 3]> {
        module
            .kernels
            .iter()
            .find(|kernel| kernel.id == *self.obligations.kernel())
            .map(|kernel| witness_extents(&kernel.domain))
    }

    /// Returns the flattened structural witness invocation count.
    pub fn witness_invocation_count(&self) -> u64 {
        self.obligations.invocations().map_or(0, |invocations| {
            invocations.end_exclusive() - invocations.start()
        })
    }
}

fn derive_admitted_obligations(
    semantic_kir: &ProductionSemanticKirOwnerV1,
) -> Result<Box<[ProductionFormalMemoryKernelV1]>, ProductionFormalMemoryErrorV1> {
    let module = semantic_kir.module();
    if module.kernels.is_empty() {
        return Err(ProductionFormalMemoryErrorV1::KernelCount {
            actual: module.kernels.len(),
        });
    }
    let mut admitted = Vec::with_capacity(module.kernels.len());
    for kernel in &module.kernels {
        admitted.push(derive_admitted_obligations_for_kernel(
            semantic_kir,
            kernel,
        )?);
    }
    Ok(admitted.into_boxed_slice())
}

fn derive_admitted_obligations_for_kernel(
    semantic_kir: &ProductionSemanticKirOwnerV1,
    kernel: &fe2o3_kernel_ir::Kernel,
) -> Result<ProductionFormalMemoryKernelV1, ProductionFormalMemoryErrorV1> {
    let module = semantic_kir.module();
    let domain = &kernel.domain;
    let rank = domain.rank();
    let witness = ExplicitLaunchExtent::Exact {
        rank,
        extents: witness_extents(domain),
    };
    let analysis = derive_kernel_memory_obligations_for_launch(
        module,
        &kernel.id,
        witness,
        FormalIndexWidth::Bits64,
    )
    .map_err(ProductionFormalMemoryErrorV1::Analysis)?;
    let (obligations, ranked_discharged_reasons, compiler_discharged_reasons) = match analysis {
        FormalMemoryObligationAnalysis::Complete(obligations) => (
            obligations,
            Vec::new().into_boxed_slice(),
            Vec::new().into_boxed_slice(),
        ),
        FormalMemoryObligationAnalysis::Incomplete { partial, reasons } => {
            let mut compiler_reasons = Vec::new();
            let mut remaining_reasons = Vec::new();
            for reason in reasons {
                if matches!(
                    reason,
                    FormalMemoryIncompleteReason::UnsupportedMemoryEffect { .. }
                        | FormalMemoryIncompleteReason::UnsupportedPointerDerivation { .. }
                ) {
                    compiler_reasons.push(reason);
                } else {
                    remaining_reasons.push(reason);
                }
            }
            if !compiler_reasons.is_empty()
                && let Err(detail) = semantic_kir
                    .retained_collective_lowering_discharges_workgroup_memory(
                        kernel.id.as_str(),
                        &compiler_reasons,
                    )
            {
                return Err(
                    ProductionFormalMemoryErrorV1::CompilerOwnedWorkgroupDischarge {
                        reasons: compiler_reasons.into_boxed_slice(),
                        detail,
                    },
                );
            }
            let mut guarded_reasons = Vec::new();
            let mut guarded_locations = Vec::new();
            let mut unsupported_indices = Vec::new();
            let mut reasons_are_ranked_dischargeable = true;
            for reason in &remaining_reasons {
                match reason {
                    FormalMemoryIncompleteReason::GuardedAccessRequiresRankedProof { location } => {
                        guarded_reasons.push(reason.clone());
                        guarded_locations.push(*location);
                    }
                    FormalMemoryIncompleteReason::UnsupportedIndexExpression { .. } => {
                        unsupported_indices.push(reason.clone());
                    }
                    _ => reasons_are_ranked_dischargeable = false,
                }
            }
            if !reasons_are_ranked_dischargeable {
                return Err(ProductionFormalMemoryErrorV1::Incomplete {
                    reasons: remaining_reasons.into_boxed_slice(),
                });
            }
            if !unsupported_indices.is_empty()
                && let Err(detail) = semantic_kir
                    .retained_generic_checks_discharge_unsupported_indices(
                        kernel.id.as_str(),
                        &unsupported_indices,
                    )
            {
                return Err(ProductionFormalMemoryErrorV1::UnsupportedIndexDischarge {
                    reasons: unsupported_indices.into_boxed_slice(),
                    detail,
                });
            }
            if !guarded_locations.is_empty()
                && let Err(detail) = semantic_kir
                    .retained_generic_checks_discharge_guarded_accesses(
                        kernel.id.as_str(),
                        &guarded_locations,
                    )
            {
                return Err(ProductionFormalMemoryErrorV1::GuardedAccessDischarge {
                    reasons: guarded_reasons.into_boxed_slice(),
                    detail,
                });
            }
            (
                partial,
                remaining_reasons.into_boxed_slice(),
                compiler_reasons.into_boxed_slice(),
            )
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
    Ok(ProductionFormalMemoryKernelV1 {
        obligations,
        ranked_discharged_reasons,
        compiler_discharged_reasons,
    })
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

#[cfg(test)]
mod tests {
    use fe2o3_kernel_ir::{BlockId, FunctionOperationLocation, ValueId};

    use super::*;

    fn guarded_reason(operation_index: usize) -> FormalMemoryIncompleteReason {
        FormalMemoryIncompleteReason::GuardedAccessRequiresRankedProof {
            location: FunctionOperationLocation::new(BlockId(2), operation_index),
        }
    }

    #[test]
    fn discharge_diagnostics_name_exact_access_bound_and_bounded_reason_set() {
        let reasons = (0..17).map(guarded_reason).collect::<Vec<_>>();
        let error = ProductionFormalMemoryErrorV1::GuardedAccessDischarge {
            reasons: reasons.into_boxed_slice(),
            detail: ProductionMemoryDischargeFailureV1::GuardedBound {
                location: FunctionOperationLocation::new(BlockId(2), 3),
                index: ValueId(41),
                slice: ValueId(7),
                detail: "guard predicate does not prove the selected index is in bounds",
            },
        };

        let diagnostic = error.to_string();
        assert!(diagnostic.contains("17 guarded access reason(s)"));
        assert!(diagnostic.contains("operation_index: 3"));
        assert!(diagnostic.contains("index ValueId(41)"));
        assert!(diagnostic.contains("slice ValueId(7)"));
        assert!(diagnostic.contains("(1 more)"));
    }

    #[test]
    fn unsupported_index_diagnostic_names_the_exact_consumer() {
        let location = FunctionOperationLocation::new(BlockId(5), 8);
        let error = ProductionFormalMemoryErrorV1::UnsupportedIndexDischarge {
            reasons: vec![guarded_reason(0)].into_boxed_slice(),
            detail: ProductionMemoryDischargeFailureV1::Access {
                location,
                detail: "semantic memory access site has no ranked access receipt",
            },
        };

        let diagnostic = error.to_string();
        assert!(diagnostic.contains("unsupported index reason(s)"));
        assert!(diagnostic.contains("semantic memory access site has no ranked access receipt"));
        assert!(diagnostic.contains("block: BlockId(5)"));
        assert!(diagnostic.contains("operation_index: 8"));
    }
}
