//! Checked resource upper bounds shared by the closed production analyses.
//!
//! These values are admission bounds, not measurements. An owning phase may
//! report a conservative value, but every input to its formula must have been
//! authenticated and bounded before the work or allocation it authorizes.

use super::pliron_invocation_trace::MAX_PLIRON_TRACE_TOTAL_STEPS_V1;
use crate::{
    MAX_PLIRON_IDENTITY_CANONICAL_BYTES_V1, MAX_PLIRON_MEMORY_VERSIONS_V1,
    MAX_PLIRON_RACE_EFFECT_INSTANCES_V1,
};
use dialect_kernel::MAX_RANKED_MEMORY_RANK;

/// Policy ceiling for the cumulative two-run production verifier.
///
/// Memory ordering and race analysis each admit a quadratic comparison space.
/// The factor covers both deterministic verifier runs plus the fixed nested
/// analysis schedule and report comparison. Individual phases still derive a
/// tighter input-sensitive bound and may reject structurally valid input whose
/// resource envelope exceeds this production policy.
pub(crate) const HARD_MAX_PRODUCTION_ANALYSIS_WORK_UPPER_BOUND_V1: usize =
    MAX_PLIRON_RACE_EFFECT_INSTANCES_V1 * MAX_PLIRON_RACE_EFFECT_INSTANCES_V1 * 256;

/// Policy ceiling for simultaneously live production-analysis state.
///
/// The terms reserve a maximal-rank trace payload, four structural identity
/// byte owners, and the complete memory-version cache. This is deliberately a
/// global compiler budget rather than a promise that every combination of
/// independently valid per-analysis maxima is admitted.
pub(crate) const HARD_MAX_PRODUCTION_ANALYSIS_STORAGE_UPPER_BOUND_V1: usize =
    MAX_PLIRON_TRACE_TOTAL_STEPS_V1 * (MAX_RANKED_MEMORY_RANK * 2 + 8)
        + MAX_PLIRON_IDENTITY_CANONICAL_BYTES_V1 * 4
        + MAX_PLIRON_MEMORY_VERSIONS_V1;

/// The analysis phase whose resource admission failed.
///
/// This is diagnostic metadata, not an analysis or refinement capability.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum ProductionAnalysisResourcePhaseV1 {
    StructuralIdentity,
    FunctionInventory,
    SparseIndex,
    Presburger,
    InvocationTrace,
    ProvenanceAlias,
    MemoryOrder,
    SimtProtocol,
    EffectRefinement,
    TensorLayout,
    MemoryBounds,
    Progress,
    LaunchContract,
    AtomicLegality,
    RaceFreedom,
    HierarchicalOwnership,
    BarrierConvergence,
    PipelineProtocol,
    WorkgroupMemory,
    SemanticRefinement,
    PassPreservation,
    ReportValidation,
    PipelineVerification,
}

impl ProductionAnalysisResourcePhaseV1 {
    /// Stable diagnostic code for this phase, independent of debug formatting.
    pub const fn code(self) -> &'static str {
        match self {
            Self::StructuralIdentity => "structural-identity",
            Self::FunctionInventory => "function-inventory",
            Self::SparseIndex => "sparse-index",
            Self::Presburger => "presburger",
            Self::InvocationTrace => "invocation-trace",
            Self::ProvenanceAlias => "provenance-alias",
            Self::MemoryOrder => "memory-order",
            Self::SimtProtocol => "simt-protocol",
            Self::EffectRefinement => "effect-refinement",
            Self::TensorLayout => "tensor-layout",
            Self::MemoryBounds => "memory-bounds",
            Self::Progress => "progress",
            Self::LaunchContract => "launch-contract",
            Self::AtomicLegality => "atomic-legality",
            Self::RaceFreedom => "race-freedom",
            Self::HierarchicalOwnership => "hierarchical-ownership",
            Self::BarrierConvergence => "barrier-convergence",
            Self::PipelineProtocol => "pipeline-protocol",
            Self::WorkgroupMemory => "workgroup-memory",
            Self::SemanticRefinement => "semantic-refinement",
            Self::PassPreservation => "pass-preservation",
            Self::ReportValidation => "report-validation",
            Self::PipelineVerification => "pipeline-verification",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductionAnalysisResourceLimitV1 {
    pub(crate) phase: ProductionAnalysisResourcePhaseV1,
    pub(crate) resource: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductionAnalysisResourceLimitsV1 {
    max_work: usize,
    max_peak_storage: usize,
}

impl ProductionAnalysisResourceLimitsV1 {
    pub(crate) const fn new(max_work: usize, max_peak_storage: usize) -> Self {
        Self {
            max_work,
            max_peak_storage,
        }
    }

    pub(crate) const fn production_hard_ceiling() -> Self {
        Self::new(
            HARD_MAX_PRODUCTION_ANALYSIS_WORK_UPPER_BOUND_V1,
            HARD_MAX_PRODUCTION_ANALYSIS_STORAGE_UPPER_BOUND_V1,
        )
    }

    pub(crate) const fn max_work(self) -> usize {
        self.max_work
    }

    pub(crate) const fn max_peak_storage(self) -> usize {
        self.max_peak_storage
    }

    pub(crate) fn require(
        self,
        phase: ProductionAnalysisResourcePhaseV1,
        upper_bound: ProductionAnalysisResourceUpperBoundV1,
    ) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
        if upper_bound.work_upper_bound > self.max_work {
            return Err(ProductionAnalysisResourceLimitV1 {
                phase,
                resource: "work upper bound",
            });
        }
        if upper_bound.peak_storage_upper_bound > self.max_peak_storage {
            return Err(ProductionAnalysisResourceLimitV1 {
                phase,
                resource: "peak storage upper bound",
            });
        }
        Ok(upper_bound)
    }

    #[cfg(test)]
    pub(crate) const fn admits(self, upper_bound: ProductionAnalysisResourceUpperBoundV1) -> bool {
        upper_bound.work_upper_bound <= self.max_work
            && upper_bound.peak_storage_upper_bound <= self.max_peak_storage
    }

    pub(crate) fn remaining_after_retained(
        self,
        phase: ProductionAnalysisResourcePhaseV1,
        prefix: ProductionAnalysisResourceUpperBoundV1,
    ) -> Result<Self, ProductionAnalysisResourceLimitV1> {
        self.require(phase, prefix)?;
        Ok(Self {
            max_work: self.max_work.checked_sub(prefix.work_upper_bound).ok_or(
                ProductionAnalysisResourceLimitV1 {
                    phase,
                    resource: "remaining work upper bound",
                },
            )?,
            max_peak_storage: self
                .max_peak_storage
                .checked_sub(prefix.retained_storage_upper_bound)
                .ok_or(ProductionAnalysisResourceLimitV1 {
                    phase,
                    resource: "remaining peak storage upper bound",
                })?,
        })
    }
}

/// Separate allowances for a nonreplacing input capture and an output phase
/// that includes the outgoing owner in its own overlap. This is resource
/// bookkeeping, not an identity or preservation capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductionAnalysisReplacementLimitsV1 {
    pub(crate) input: ProductionAnalysisResourceLimitsV1,
    pub(crate) output: ProductionAnalysisResourceLimitsV1,
}

impl From<ProductionAnalysisResourceLimitsV1> for ProductionAnalysisReplacementLimitsV1 {
    fn from(limits: ProductionAnalysisResourceLimitsV1) -> Self {
        Self {
            input: limits,
            output: limits,
        }
    }
}

/// Authenticated structural cardinalities shared by every analysis phase.
///
/// The structural-identity builder populates this value while enforcing its
/// public block/operation/value/operand/successor/attribute/text/type limits.
/// Pass-specific expansion counts (for example invocations or memory effects)
/// remain owned and bounded by the phase that creates them.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ProductionAnalysisInputCensusV1 {
    pub(crate) blocks: usize,
    pub(crate) operations: usize,
    pub(crate) operands: usize,
    pub(crate) results: usize,
    pub(crate) successors: usize,
    pub(crate) block_arguments: usize,
    pub(crate) attributes: usize,
    pub(crate) type_nodes: usize,
    pub(crate) identifier_bytes: usize,
    pub(crate) canonical_bytes: usize,
    pub(crate) max_operation_arity: usize,
    pub(crate) max_successor_arity: usize,
    pub(crate) pipeline_creates: usize,
    pub(crate) pipeline_events: usize,
    pub(crate) ranked_accesses: usize,
    pub(crate) workgroup_ranked_accesses: usize,
    pub(crate) allocation_effects: usize,
    pub(crate) collective_transpose_candidates: usize,
    pub(crate) ownership_contracts: usize,
    pub(crate) effect_refinement_contracts: usize,
    pub(crate) index_lt_branch_candidates: usize,
    /// Operations that can populate the semantic expression table, including
    /// malformed candidates that the later semantic verifier rejects.
    pub(crate) semantic_definitions: usize,
    /// Proof, refinement, tensor, ownership, and collective records consumed
    /// by semantic refinement, independent of whether their payload is valid.
    pub(crate) semantic_refinement_contracts: usize,
    /// Native callback subtotals only; generic PLIRON verification is separate.
    pub(crate) native_switch_verification_work: usize,
    pub(crate) native_switch_verification_scratch: usize,
}

impl ProductionAnalysisInputCensusV1 {
    pub(crate) fn checked_structural_items(
        self,
        phase: ProductionAnalysisResourcePhaseV1,
    ) -> Result<usize, ProductionAnalysisResourceLimitV1> {
        [
            self.blocks,
            self.operations,
            self.operands,
            self.results,
            self.successors,
            self.block_arguments,
            self.attributes,
            self.type_nodes,
            self.identifier_bytes,
            self.canonical_bytes,
        ]
        .into_iter()
        .try_fold(0_usize, |total, items| {
            total
                .checked_add(items)
                .ok_or(ProductionAnalysisResourceLimitV1 {
                    phase,
                    resource: "structural input census",
                })
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ProductionAnalysisResourceUpperBoundV1 {
    work_upper_bound: usize,
    retained_storage_upper_bound: usize,
    peak_storage_upper_bound: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductionAnalysisResourceContractV1 {
    limits: ProductionAnalysisResourceLimitsV1,
    cumulative: ProductionAnalysisResourceUpperBoundV1,
}

impl ProductionAnalysisResourceContractV1 {
    pub(crate) const fn new(limits: ProductionAnalysisResourceLimitsV1) -> Self {
        Self {
            limits,
            cumulative: ProductionAnalysisResourceUpperBoundV1 {
                work_upper_bound: 0,
                retained_storage_upper_bound: 0,
                peak_storage_upper_bound: 0,
            },
        }
    }

    pub(crate) const fn cumulative(self) -> ProductionAnalysisResourceUpperBoundV1 {
        self.cumulative
    }

    pub(crate) fn remaining(
        self,
        phase: ProductionAnalysisResourcePhaseV1,
    ) -> Result<ProductionAnalysisResourceLimitsV1, ProductionAnalysisResourceLimitV1> {
        self.limits.remaining_after_retained(phase, self.cumulative)
    }

    pub(crate) fn admit_retained(
        &mut self,
        phase: ProductionAnalysisResourcePhaseV1,
        upper_bound: ProductionAnalysisResourceUpperBoundV1,
    ) -> Result<(), ProductionAnalysisResourceLimitV1> {
        let cumulative = self.cumulative.checked_then_retain(upper_bound, phase)?;
        self.limits.require(phase, cumulative)?;
        self.cumulative = cumulative;
        Ok(())
    }

    /// Returns a phase allowance for replacing an already-admitted owner.
    /// The caller must establish that owner's identity. The replacement's
    /// own peak must include both outgoing and incoming ownership. This only
    /// discounts the outgoing owner in the prefix; it releases nothing and
    /// preserves cumulative work and historical peak admission.
    pub(crate) fn remaining_for_replacement(
        self,
        phase: ProductionAnalysisResourcePhaseV1,
        replaced_retained_storage: usize,
    ) -> Result<ProductionAnalysisResourceLimitsV1, ProductionAnalysisResourceLimitV1> {
        self.limits.require(phase, self.cumulative)?;
        let retained_storage_upper_bound = self
            .cumulative
            .retained_storage_upper_bound
            .checked_sub(replaced_retained_storage)
            .ok_or(ProductionAnalysisResourceLimitV1 {
                phase,
                resource: "replaced retained storage upper bound",
            })?;
        self.limits.remaining_after_retained(
            phase,
            ProductionAnalysisResourceUpperBoundV1 {
                retained_storage_upper_bound,
                ..self.cumulative
            },
        )
    }

    /// Admits a phase that atomically replaces one previously retained owner.
    ///
    /// The replacement phase's peak already includes the outgoing owner, so
    /// that owner is removed from the prefix before the phase peak is composed.
    pub(crate) fn admit_replacement(
        &mut self,
        phase: ProductionAnalysisResourcePhaseV1,
        replaced_retained_storage: usize,
        upper_bound: ProductionAnalysisResourceUpperBoundV1,
    ) -> Result<(), ProductionAnalysisResourceLimitV1> {
        let cumulative = self.cumulative.checked_then_replace_retained(
            replaced_retained_storage,
            upper_bound,
            phase,
        )?;
        self.limits.require(phase, cumulative)?;
        self.cumulative = cumulative;
        Ok(())
    }
}

impl ProductionAnalysisResourceUpperBoundV1 {
    #[cfg(test)]
    pub(crate) const fn zero() -> Self {
        Self {
            work_upper_bound: 0,
            retained_storage_upper_bound: 0,
            peak_storage_upper_bound: 0,
        }
    }

    pub(crate) const fn work_upper_bound(self) -> usize {
        self.work_upper_bound
    }

    pub(crate) const fn retained_storage_upper_bound(self) -> usize {
        self.retained_storage_upper_bound
    }

    pub(crate) const fn peak_storage_upper_bound(self) -> usize {
        self.peak_storage_upper_bound
    }

    pub(crate) fn checked_phase(
        phase: ProductionAnalysisResourcePhaseV1,
        work_upper_bound: usize,
        retained_storage_upper_bound: usize,
        temporary_storage_upper_bound: usize,
    ) -> Result<Self, ProductionAnalysisResourceLimitV1> {
        let peak_storage_upper_bound = retained_storage_upper_bound
            .checked_add(temporary_storage_upper_bound)
            .ok_or(ProductionAnalysisResourceLimitV1 {
                phase,
                resource: "peak storage upper bound",
            })?;
        Ok(Self {
            work_upper_bound,
            retained_storage_upper_bound,
            peak_storage_upper_bound,
        })
    }

    /// Sequentially composes a phase while retaining both phase outputs.
    pub(crate) fn checked_then_retain(
        self,
        phase: Self,
        owner: ProductionAnalysisResourcePhaseV1,
    ) -> Result<Self, ProductionAnalysisResourceLimitV1> {
        let work_upper_bound = self
            .work_upper_bound
            .checked_add(phase.work_upper_bound)
            .ok_or(ProductionAnalysisResourceLimitV1 {
                phase: owner,
                resource: "work upper bound",
            })?;
        let peak_during_phase = self
            .retained_storage_upper_bound
            .checked_add(phase.peak_storage_upper_bound)
            .ok_or(ProductionAnalysisResourceLimitV1 {
                phase: owner,
                resource: "peak storage upper bound",
            })?;
        let retained_storage_upper_bound = self
            .retained_storage_upper_bound
            .checked_add(phase.retained_storage_upper_bound)
            .ok_or(ProductionAnalysisResourceLimitV1 {
                phase: owner,
                resource: "retained storage upper bound",
            })?;
        Ok(Self {
            work_upper_bound,
            retained_storage_upper_bound,
            peak_storage_upper_bound: self
                .peak_storage_upper_bound
                .max(peak_during_phase)
                .max(retained_storage_upper_bound),
        })
    }

    /// Sequentially composes a phase whose outputs are discarded afterward.
    pub(crate) fn checked_then_discard(
        self,
        phase: Self,
        owner: ProductionAnalysisResourcePhaseV1,
    ) -> Result<Self, ProductionAnalysisResourceLimitV1> {
        let work_upper_bound = self
            .work_upper_bound
            .checked_add(phase.work_upper_bound)
            .ok_or(ProductionAnalysisResourceLimitV1 {
                phase: owner,
                resource: "work upper bound",
            })?;
        let peak_during_phase = self
            .retained_storage_upper_bound
            .checked_add(phase.peak_storage_upper_bound)
            .ok_or(ProductionAnalysisResourceLimitV1 {
                phase: owner,
                resource: "peak storage upper bound",
            })?;
        Ok(Self {
            work_upper_bound,
            retained_storage_upper_bound: self.retained_storage_upper_bound,
            peak_storage_upper_bound: self.peak_storage_upper_bound.max(peak_during_phase),
        })
    }

    /// Composes nested analyses whose outputs are discarded while the outer
    /// analysis state remains live. Nested analyses execute sequentially, so
    /// only their widest peak overlaps the outer peak.
    pub(crate) fn checked_with_nested_sequence_discard(
        self,
        nested: &[Self],
        owner: ProductionAnalysisResourcePhaseV1,
    ) -> Result<Self, ProductionAnalysisResourceLimitV1> {
        let mut work_upper_bound = self.work_upper_bound;
        let mut nested_peak_storage_upper_bound = 0;
        for phase in nested {
            work_upper_bound = work_upper_bound.checked_add(phase.work_upper_bound).ok_or(
                ProductionAnalysisResourceLimitV1 {
                    phase: owner,
                    resource: "nested work upper bound",
                },
            )?;
            nested_peak_storage_upper_bound =
                nested_peak_storage_upper_bound.max(phase.peak_storage_upper_bound);
        }
        let peak_storage_upper_bound = self
            .peak_storage_upper_bound
            .checked_add(nested_peak_storage_upper_bound)
            .ok_or(ProductionAnalysisResourceLimitV1 {
                phase: owner,
                resource: "nested peak storage upper bound",
            })?;
        Ok(Self {
            work_upper_bound,
            retained_storage_upper_bound: self.retained_storage_upper_bound,
            peak_storage_upper_bound,
        })
    }

    /// Composes nested analyses whose reports remain owned by the outer
    /// report. Previously returned nested reports remain live while the next
    /// nested analysis runs.
    pub(crate) fn checked_with_nested_sequence_retain(
        self,
        nested: &[Self],
        owner: ProductionAnalysisResourcePhaseV1,
    ) -> Result<Self, ProductionAnalysisResourceLimitV1> {
        let mut work_upper_bound = self.work_upper_bound;
        let mut nested_retained_storage_upper_bound = 0_usize;
        let mut peak_storage_upper_bound = self.peak_storage_upper_bound;
        for phase in nested {
            work_upper_bound = work_upper_bound.checked_add(phase.work_upper_bound).ok_or(
                ProductionAnalysisResourceLimitV1 {
                    phase: owner,
                    resource: "nested work upper bound",
                },
            )?;
            let peak_during_phase = self
                .peak_storage_upper_bound
                .checked_add(nested_retained_storage_upper_bound)
                .and_then(|peak| peak.checked_add(phase.peak_storage_upper_bound))
                .ok_or(ProductionAnalysisResourceLimitV1 {
                    phase: owner,
                    resource: "nested peak storage upper bound",
                })?;
            peak_storage_upper_bound = peak_storage_upper_bound.max(peak_during_phase);
            nested_retained_storage_upper_bound = nested_retained_storage_upper_bound
                .checked_add(phase.retained_storage_upper_bound)
                .ok_or(ProductionAnalysisResourceLimitV1 {
                    phase: owner,
                    resource: "nested retained storage upper bound",
                })?;
        }
        let retained_storage_upper_bound = self
            .retained_storage_upper_bound
            .checked_add(nested_retained_storage_upper_bound)
            .ok_or(ProductionAnalysisResourceLimitV1 {
                phase: owner,
                resource: "nested retained storage upper bound",
            })?;
        Ok(Self {
            work_upper_bound,
            retained_storage_upper_bound,
            peak_storage_upper_bound: peak_storage_upper_bound.max(retained_storage_upper_bound),
        })
    }

    /// Sequentially composes a phase that replaces one live retained owner.
    pub(crate) fn checked_then_replace_retained(
        self,
        replaced_retained_storage: usize,
        phase: Self,
        owner: ProductionAnalysisResourcePhaseV1,
    ) -> Result<Self, ProductionAnalysisResourceLimitV1> {
        let prefix_retained = self
            .retained_storage_upper_bound
            .checked_sub(replaced_retained_storage)
            .ok_or(ProductionAnalysisResourceLimitV1 {
                phase: owner,
                resource: "replaced retained storage upper bound",
            })?;
        let work_upper_bound = self
            .work_upper_bound
            .checked_add(phase.work_upper_bound)
            .ok_or(ProductionAnalysisResourceLimitV1 {
                phase: owner,
                resource: "work upper bound",
            })?;
        let peak_during_phase = prefix_retained
            .checked_add(phase.peak_storage_upper_bound)
            .ok_or(ProductionAnalysisResourceLimitV1 {
                phase: owner,
                resource: "peak storage upper bound",
            })?;
        let retained_storage_upper_bound = prefix_retained
            .checked_add(phase.retained_storage_upper_bound)
            .ok_or(ProductionAnalysisResourceLimitV1 {
                phase: owner,
                resource: "retained storage upper bound",
            })?;
        Ok(Self {
            work_upper_bound,
            retained_storage_upper_bound,
            peak_storage_upper_bound: self
                .peak_storage_upper_bound
                .max(peak_during_phase)
                .max(retained_storage_upper_bound),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    include!("pliron_resource_envelope/replacement_limits_v1_tests.rs");

    #[test]
    fn sequential_composition_tracks_lifetimes_and_overflow() {
        let retained = ProductionAnalysisResourceUpperBoundV1::checked_phase(
            ProductionAnalysisResourcePhaseV1::StructuralIdentity,
            5,
            7,
            3,
        )
        .unwrap();
        let temporary = ProductionAnalysisResourceUpperBoundV1::checked_phase(
            ProductionAnalysisResourcePhaseV1::StructuralIdentity,
            11,
            0,
            13,
        )
        .unwrap();
        let combined = retained
            .checked_then_discard(
                temporary,
                ProductionAnalysisResourcePhaseV1::StructuralIdentity,
            )
            .unwrap();
        assert_eq!(combined.work_upper_bound(), 16);
        assert_eq!(combined.retained_storage_upper_bound(), 7);
        assert_eq!(combined.peak_storage_upper_bound(), 20);

        let overflow = ProductionAnalysisResourceUpperBoundV1::checked_phase(
            ProductionAnalysisResourcePhaseV1::StructuralIdentity,
            usize::MAX,
            0,
            0,
        )
        .unwrap()
        .checked_then_discard(
            ProductionAnalysisResourceUpperBoundV1::checked_phase(
                ProductionAnalysisResourcePhaseV1::FunctionInventory,
                1,
                0,
                0,
            )
            .unwrap(),
            ProductionAnalysisResourcePhaseV1::FunctionInventory,
        );
        assert_eq!(
            overflow,
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::FunctionInventory,
                resource: "work upper bound",
            })
        );
    }

    #[test]
    fn limits_admit_exact_work_and_peak_and_reject_one_under() {
        let phase = ProductionAnalysisResourceUpperBoundV1::checked_phase(
            ProductionAnalysisResourcePhaseV1::FunctionInventory,
            37,
            11,
            19,
        )
        .unwrap();
        assert_eq!(phase.peak_storage_upper_bound(), 30);
        assert_eq!(
            ProductionAnalysisResourceLimitsV1::new(37, 30)
                .require(ProductionAnalysisResourcePhaseV1::FunctionInventory, phase,),
            Ok(phase)
        );
        assert_eq!(
            ProductionAnalysisResourceLimitsV1::new(36, 30)
                .require(ProductionAnalysisResourcePhaseV1::FunctionInventory, phase,),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::FunctionInventory,
                resource: "work upper bound",
            })
        );
        assert_eq!(
            ProductionAnalysisResourceLimitsV1::new(37, 29)
                .require(ProductionAnalysisResourcePhaseV1::FunctionInventory, phase,),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::FunctionInventory,
                resource: "peak storage upper bound",
            })
        );
    }

    #[test]
    fn replacement_composition_retires_one_live_owner_without_hiding_peak() {
        let prefix = ProductionAnalysisResourceUpperBoundV1::checked_phase(
            ProductionAnalysisResourcePhaseV1::StructuralIdentity,
            5,
            17,
            3,
        )
        .unwrap()
        .checked_then_retain(
            ProductionAnalysisResourceUpperBoundV1::checked_phase(
                ProductionAnalysisResourcePhaseV1::FunctionInventory,
                7,
                11,
                0,
            )
            .unwrap(),
            ProductionAnalysisResourcePhaseV1::FunctionInventory,
        )
        .unwrap();
        let replacement = ProductionAnalysisResourceUpperBoundV1::checked_phase(
            ProductionAnalysisResourcePhaseV1::PassPreservation,
            13,
            20,
            19,
        )
        .unwrap();
        let combined = prefix
            .checked_then_replace_retained(
                17,
                replacement,
                ProductionAnalysisResourcePhaseV1::PassPreservation,
            )
            .unwrap();
        assert_eq!(combined.work_upper_bound(), 25);
        assert_eq!(combined.retained_storage_upper_bound(), 31);
        assert_eq!(combined.peak_storage_upper_bound(), 50);
        assert!(
            prefix
                .checked_then_replace_retained(
                    29,
                    replacement,
                    ProductionAnalysisResourcePhaseV1::PassPreservation,
                )
                .is_err()
        );
    }

    #[test]
    fn nested_composition_accounts_for_outer_live_state_and_retained_reports() {
        let outer = ProductionAnalysisResourceUpperBoundV1::checked_phase(
            ProductionAnalysisResourcePhaseV1::SemanticRefinement,
            5,
            7,
            3,
        )
        .unwrap();
        let first = ProductionAnalysisResourceUpperBoundV1::checked_phase(
            ProductionAnalysisResourcePhaseV1::Progress,
            11,
            2,
            11,
        )
        .unwrap();
        let second = ProductionAnalysisResourceUpperBoundV1::checked_phase(
            ProductionAnalysisResourcePhaseV1::EffectRefinement,
            17,
            5,
            15,
        )
        .unwrap();

        let discarded = outer
            .checked_with_nested_sequence_discard(
                &[first, second],
                ProductionAnalysisResourcePhaseV1::SemanticRefinement,
            )
            .unwrap();
        assert_eq!(discarded.work_upper_bound(), 33);
        assert_eq!(discarded.retained_storage_upper_bound(), 7);
        assert_eq!(discarded.peak_storage_upper_bound(), 30);

        let retained = outer
            .checked_with_nested_sequence_retain(
                &[first, second],
                ProductionAnalysisResourcePhaseV1::SemanticRefinement,
            )
            .unwrap();
        assert_eq!(retained.work_upper_bound(), 33);
        assert_eq!(retained.retained_storage_upper_bound(), 14);
        assert_eq!(retained.peak_storage_upper_bound(), 32);

        let overflowing = outer.checked_with_nested_sequence_discard(
            &[ProductionAnalysisResourceUpperBoundV1 {
                work_upper_bound: 0,
                retained_storage_upper_bound: 0,
                peak_storage_upper_bound: usize::MAX,
            }],
            ProductionAnalysisResourcePhaseV1::SemanticRefinement,
        );
        assert_eq!(
            overflowing,
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::SemanticRefinement,
                resource: "nested peak storage upper bound",
            })
        );
    }
}
