//! Production join from compiler-derived sequential/parallel facts to one
//! workload-neutral parallel-reference contract.

use std::{collections::BTreeSet, error::Error, fmt};

use dialect_kernel::OwnershipCoverageAttr;
use fe2o3_functional_proof::{
    MirPlironSemanticContractV1, ParallelCallKindV1, ParallelCallSummaryV1, ParallelFoldOrderV1,
    ParallelHierarchyLevelV1, ParallelNumericalPolicyV1, ParallelReferenceContractV1,
    ParallelScheduleRelationV1, SemanticCollectiveContractV1, SemanticCollectiveKindV1,
    SemanticEvaluationOrderV1, SemanticFiniteExtentV1, SemanticNumericalPolicyV1,
    SemanticOutputContractV1, SemanticScalarTypeV1, SemanticTypedRootV1,
};
use fe2o3_kernel_analysis::HierarchicalOwnershipLevelV1;
use fe2o3_proof_contracts::DigestV1;
use sha2::{Digest as _, Sha256};

use super::{
    ProductionMiddleEndEvidenceV5, ProductionMirPlironSemanticContractReportV1,
    ProductionRankedKernelLoweringInputV1, ProductionRankedOperationV1,
};

const TENSOR_SITE_DOMAIN_V1: &[u8] = b"FE2O3/PARALLEL-REFERENCE/TENSOR-SITE/V1\0";
const DYNAMIC_BOUND_DOMAIN_V1: &[u8] = b"FE2O3/PARALLEL-REFERENCE/DYNAMIC-BOUND/V1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionParallelReferenceContractReportV1 {
    contract_identity: DigestV1,
    output_relations: u64,
    pointwise_relations: u64,
    permutation_relations: u64,
    fold_relations: u64,
    bounded_recurrences: u64,
    call_summaries: u64,
    tensor_summaries: u64,
    error_bounded_relations: u64,
}

impl ProductionParallelReferenceContractReportV1 {
    pub const fn contract_identity(self) -> DigestV1 {
        self.contract_identity
    }
    pub const fn output_relations(self) -> u64 {
        self.output_relations
    }
    pub const fn pointwise_relations(self) -> u64 {
        self.pointwise_relations
    }
    pub const fn permutation_relations(self) -> u64 {
        self.permutation_relations
    }
    pub const fn fold_relations(self) -> u64 {
        self.fold_relations
    }
    pub const fn bounded_recurrences(self) -> u64 {
        self.bounded_recurrences
    }
    pub const fn call_summaries(self) -> u64 {
        self.call_summaries
    }
    pub const fn tensor_summaries(self) -> u64 {
        self.tensor_summaries
    }
    pub const fn error_bounded_relations(self) -> u64 {
        self.error_bounded_relations
    }
    pub const fn binds_reference_domains_to_complete_gpu_hierarchy(self) -> bool {
        true
    }
    pub const fn grants_llvm_or_later_authority(self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionParallelReferenceContractErrorV1 {
    SemanticContractMismatch,
    OutputCoverageIncomplete { declared: usize, live: usize },
    OutputRelationMismatch { index: usize },
    HierarchyCoverageIncomplete { level: ParallelHierarchyLevelV1 },
    AuthenticatedProofIncomplete { identity: DigestV1 },
    ScheduleRelationIncomplete { index: usize, detail: &'static str },
    FoldOrderRejected { index: usize, detail: &'static str },
    DynamicBoundProofIncomplete { index: usize },
    NumericalPolicyRejected { index: usize, detail: &'static str },
    NumericalProofIncomplete { index: usize },
    CallSummaryDerivationIncomplete { index: usize, kind: &'static str },
    CallSummaryMismatch { index: usize },
    TensorFragmentOwnershipIncomplete { index: usize },
    UnmodeledTensorSites { declared: usize, live: usize },
    CounterOverflow,
}

impl ProductionParallelReferenceContractErrorV1 {
    /// `Incomplete` means the compiler lacks an independently derived fact;
    /// it is never converted to successful admission.
    pub const fn is_incomplete(&self) -> bool {
        matches!(
            self,
            Self::OutputCoverageIncomplete { .. }
                | Self::HierarchyCoverageIncomplete { .. }
                | Self::AuthenticatedProofIncomplete { .. }
                | Self::ScheduleRelationIncomplete { .. }
                | Self::DynamicBoundProofIncomplete { .. }
                | Self::NumericalProofIncomplete { .. }
                | Self::CallSummaryDerivationIncomplete { .. }
                | Self::TensorFragmentOwnershipIncomplete { .. }
        )
    }
}

impl fmt::Display for ProductionParallelReferenceContractErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SemanticContractMismatch => formatter.write_str("error[FE2O3-PARALLEL-001]: parallel relation does not bind the exact compiler-verified MIR/PLIRON semantic contract"),
            Self::OutputCoverageIncomplete { declared, live } => write!(formatter, "error[FE2O3-PARALLEL-002]: parallel reference coverage is incomplete: {declared} logical output relations were declared but {live} live total-output ownership proofs were derived"),
            Self::OutputRelationMismatch { index } => write!(formatter, "error[FE2O3-PARALLEL-003]: parallel output relation {index} does not match its compiler-derived output domain, view, values, or ownership contract"),
            Self::HierarchyCoverageIncomplete { level } => write!(formatter, "error[FE2O3-PARALLEL-004]: compiler could not derive nonempty {level:?} ownership while relating the sequential output domain to the complete GPU hierarchy"),
            Self::AuthenticatedProofIncomplete { identity } => write!(formatter, "error[FE2O3-PARALLEL-005]: no retained authenticated per-compilation proof has identity {identity:?}"),
            Self::ScheduleRelationIncomplete { index, detail } => write!(formatter, "error[FE2O3-PARALLEL-006]: schedule relation {index} is incomplete: {detail}"),
            Self::FoldOrderRejected { index, detail } => write!(formatter, "error[FE2O3-PARALLEL-007]: fold order for relation {index} is not justified: {detail}"),
            Self::DynamicBoundProofIncomplete { index } => write!(formatter, "error[FE2O3-PARALLEL-008]: dynamic bounded recurrence {index} does not match the compiler-derived finite-bound identity for the live canonical loop"),
            Self::NumericalPolicyRejected { index, detail } => write!(formatter, "error[FE2O3-PARALLEL-009]: numerical policy for relation {index} is invalid: {detail}"),
            Self::NumericalProofIncomplete { index } => write!(formatter, "error[FE2O3-PARALLEL-010]: finite error policy for relation {index} lacks a live typed witness or retained authenticated proof"),
            Self::CallSummaryDerivationIncomplete { index, kind } => write!(formatter, "error[FE2O3-PARALLEL-011]: compiler cannot independently derive {kind} call summary {index} from the current ranked IR; declaration-only summaries are not evidence"),
            Self::CallSummaryMismatch { index } => write!(formatter, "error[FE2O3-PARALLEL-012]: helper or intrinsic summary {index} differs from live typed roots, scope, callsite, or authenticated proof"),
            Self::TensorFragmentOwnershipIncomplete { index } => write!(formatter, "error[FE2O3-PARALLEL-013]: cooperative tensor summary {index} lacks a clean live fragment ownership and convergence proof"),
            Self::UnmodeledTensorSites { declared, live } => write!(formatter, "error[FE2O3-PARALLEL-014]: parallel contract models {declared} cooperative tensor sites but the live ranked graph contains {live}"),
            Self::CounterOverflow => formatter.write_str("error[FE2O3-PARALLEL-015]: parallel relation count cannot be represented in the production report"),
        }
    }
}

impl Error for ProductionParallelReferenceContractErrorV1 {}

/// Compiler-owned builder over immutable live ranked IR and mandatory reports.
/// The supplied contract is only an expected encoding checked against these
/// independently derived facts.
pub struct ProductionParallelReferenceContractBuilderV1<'a> {
    ranked: &'a ProductionRankedKernelLoweringInputV1,
    evidence: &'a ProductionMiddleEndEvidenceV5,
    semantics: ProductionMirPlironSemanticContractReportV1,
    semantic_contract: &'a MirPlironSemanticContractV1,
}

impl<'a> ProductionParallelReferenceContractBuilderV1<'a> {
    pub fn new(
        ranked: &'a ProductionRankedKernelLoweringInputV1,
        evidence: &'a ProductionMiddleEndEvidenceV5,
        semantics: ProductionMirPlironSemanticContractReportV1,
        semantic_contract: &'a MirPlironSemanticContractV1,
    ) -> Result<Self, ProductionParallelReferenceContractErrorV1> {
        if semantics.contract_identity() != semantic_contract.canonical_sha256()
            || DigestV1::from_untrusted_bytes(*evidence.identity().sha256())
                != semantic_contract.pliron_evidence()
            || evidence.ranked_kernel_identity()
                != &super::middle_end_evidence_v4::derive_ranked_kernel_identity(ranked)
            || !ranked.all_mandatory_reports_are_clean()
        {
            return Err(ProductionParallelReferenceContractErrorV1::SemanticContractMismatch);
        }
        Ok(Self {
            ranked,
            evidence,
            semantics,
            semantic_contract,
        })
    }

    pub fn require(
        self,
        expected: &ParallelReferenceContractV1,
    ) -> Result<
        ProductionParallelReferenceContractReportV1,
        ProductionParallelReferenceContractErrorV1,
    > {
        require_parallel_reference_contract_v1(
            self.ranked,
            self.evidence,
            self.semantics,
            self.semantic_contract,
            expected,
        )
    }
}

#[allow(clippy::too_many_lines)]
pub fn require_parallel_reference_contract_v1(
    ranked: &ProductionRankedKernelLoweringInputV1,
    evidence: &ProductionMiddleEndEvidenceV5,
    semantics: ProductionMirPlironSemanticContractReportV1,
    semantic_contract: &MirPlironSemanticContractV1,
    expected: &ParallelReferenceContractV1,
) -> Result<ProductionParallelReferenceContractReportV1, ProductionParallelReferenceContractErrorV1>
{
    if semantics.contract_identity() != semantic_contract.canonical_sha256()
        || expected.semantic_contract_identity() != semantics.contract_identity()
        || DigestV1::from_untrusted_bytes(*evidence.identity().sha256())
            != semantic_contract.pliron_evidence()
        || evidence.ranked_kernel_identity()
            != &super::middle_end_evidence_v4::derive_ranked_kernel_identity(ranked)
        || !ranked.all_mandatory_reports_are_clean()
    {
        return Err(ProductionParallelReferenceContractErrorV1::SemanticContractMismatch);
    }

    let live_total = usize::try_from(evidence.coverage_summary().total_view_proved())
        .map_err(|_| ProductionParallelReferenceContractErrorV1::CounterOverflow)?;
    if expected.relations().len() != semantic_contract.outputs().len()
        || live_total != semantic_contract.outputs().len()
        || evidence.coverage_summary().total_view_declared()
            != evidence.coverage_summary().total_view_proved()
    {
        return Err(
            ProductionParallelReferenceContractErrorV1::OutputCoverageIncomplete {
                declared: expected.relations().len(),
                live: live_total,
            },
        );
    }

    for required in [
        ParallelHierarchyLevelV1::Invocation,
        ParallelHierarchyLevelV1::Subgroup,
        ParallelHierarchyLevelV1::Workgroup,
        ParallelHierarchyLevelV1::Grid,
    ] {
        let present = ranked.ownership_report().regions().iter().any(|region| {
            region.coverage() == OwnershipCoverageAttr::TotalView
                && hierarchy_level(region.identity().level()) == required
                && region.element_count() != 0
        });
        if !present {
            return Err(
                ProductionParallelReferenceContractErrorV1::HierarchyCoverageIncomplete {
                    level: required,
                },
            );
        }
    }

    let receipt_ids = ranked
        .retained_functional_refinement_receipts()
        .iter()
        .map(|receipt| receipt.receipt_identity().digest())
        .collect::<BTreeSet<_>>();
    let mut counts = RelationCountsV1::default();
    for (index, output) in semantic_contract.outputs().iter().enumerate() {
        let Some(relation) = expected
            .relations()
            .iter()
            .find(|relation| relation.output_contract() == output.identity())
        else {
            return Err(
                ProductionParallelReferenceContractErrorV1::OutputRelationMismatch { index },
            );
        };
        if relation.logical_domain() != output.output_domain()
            || !has_total_ownership_contract(ranked, output)
            || relation.hierarchy() != fe2o3_functional_proof::COMPLETE_GPU_HIERARCHY_V1
        {
            return Err(
                ProductionParallelReferenceContractErrorV1::OutputRelationMismatch { index },
            );
        }
        require_receipt(&receipt_ids, relation.authenticated_proof())?;
        require_numerical_policy(
            index,
            relation.numerical_policy(),
            output,
            semantic_contract,
            &receipt_ids,
        )?;
        if matches!(
            relation.numerical_policy(),
            ParallelNumericalPolicyV1::ErrorBounded { .. }
        ) {
            counts.error_bounded += 1;
        }
        match relation.schedule() {
            ParallelScheduleRelationV1::PointwiseBijection => {
                let (actual, reference) = output_roots(output, semantic_contract).ok_or(
                    ProductionParallelReferenceContractErrorV1::OutputRelationMismatch { index },
                )?;
                let has_collective = semantic_contract.collectives().iter().any(|collective| {
                    collective.actual() == output.actual()
                        && collective.expected() == output.reference()
                });
                if actual.commitment() != reference.commitment() || has_collective {
                    return Err(
                        ProductionParallelReferenceContractErrorV1::ScheduleRelationIncomplete {
                            index,
                            detail: "compiler-derived facts require either unequal pointwise expressions or a live collective schedule, so pointwise bijection was not derived",
                        },
                    );
                }
                counts.pointwise += 1;
            }
            ParallelScheduleRelationV1::Permutation { collective } => {
                let collective = require_collective(
                    index,
                    collective,
                    SemanticCollectiveKindV1::PermutationGather,
                    output,
                    semantic_contract,
                )?;
                if collective.target_domain() != output.output_domain() {
                    return Err(
                        ProductionParallelReferenceContractErrorV1::ScheduleRelationIncomplete {
                            index,
                            detail: "live permutation target domain differs from the logical output domain",
                        },
                    );
                }
                counts.permutation += 1;
            }
            ParallelScheduleRelationV1::Fold {
                collective,
                order,
                reference_order,
            } => {
                let collective = require_collective(
                    index,
                    collective,
                    SemanticCollectiveKindV1::FiniteFold,
                    output,
                    semantic_contract,
                )?;
                require_fold_order(
                    index,
                    order,
                    reference_order,
                    collective,
                    relation.numerical_policy(),
                    &receipt_ids,
                )?;
                counts.fold += 1;
            }
            ParallelScheduleRelationV1::BoundedRecurrence {
                collective,
                loop_contract,
                dynamic_bound_proof,
                reference_order,
            } => {
                let collective = require_collective(
                    index,
                    collective,
                    SemanticCollectiveKindV1::FiniteRecurrence,
                    output,
                    semantic_contract,
                )?;
                if collective.order() != reference_order {
                    return Err(
                        ProductionParallelReferenceContractErrorV1::FoldOrderRejected {
                            index,
                            detail: "live recurrence evaluation order differs from the sequential reference order",
                        },
                    );
                }
                let Some(loop_contract) = semantic_contract
                    .loops()
                    .iter()
                    .find(|item| item.identity() == loop_contract)
                else {
                    return Err(
                        ProductionParallelReferenceContractErrorV1::ScheduleRelationIncomplete {
                            index,
                            detail: "no live canonical loop has the declared recurrence identity",
                        },
                    );
                };
                if loop_contract.iteration_domain() != collective.source_domain() {
                    return Err(
                        ProductionParallelReferenceContractErrorV1::ScheduleRelationIncomplete {
                            index,
                            detail: "canonical loop domain differs from the recurrence contribution domain",
                        },
                    );
                }
                let dynamic = semantic_contract
                    .domains()
                    .iter()
                    .find(|domain| domain.identity() == loop_contract.iteration_domain())
                    .is_some_and(|domain| {
                        domain
                            .extents()
                            .iter()
                            .any(|extent| matches!(extent, SemanticFiniteExtentV1::Dynamic { .. }))
                    });
                let derived_bound =
                    production_dynamic_loop_bound_identity_v1(semantic_contract, loop_contract);
                match (dynamic, dynamic_bound_proof) {
                    (true, Some(proof)) if proof == derived_bound => {}
                    (true, _) => return Err(
                        ProductionParallelReferenceContractErrorV1::DynamicBoundProofIncomplete {
                            index,
                        },
                    ),
                    (false, Some(_)) => return Err(
                        ProductionParallelReferenceContractErrorV1::ScheduleRelationIncomplete {
                            index,
                            detail: "static recurrence carries a stale dynamic-bound proof",
                        },
                    ),
                    (false, None) => {}
                }
                counts.recurrence += 1;
            }
        }
    }

    let live_tensor_sites = tensor_site_count(ranked);
    let declared_tensor_sites = expected
        .call_summaries()
        .iter()
        .filter(|summary| {
            matches!(
                summary.kind(),
                ParallelCallKindV1::CooperativeTensorIntrinsic { .. }
            )
        })
        .count();
    if live_tensor_sites != declared_tensor_sites {
        return Err(
            ProductionParallelReferenceContractErrorV1::UnmodeledTensorSites {
                declared: declared_tensor_sites,
                live: live_tensor_sites,
            },
        );
    }
    for (index, summary) in expected.call_summaries().iter().enumerate() {
        require_call_summary(index, summary, ranked, semantic_contract, &receipt_ids)?;
    }

    Ok(ProductionParallelReferenceContractReportV1 {
        contract_identity: expected.canonical_sha256(),
        output_relations: count(expected.relations().len())?,
        pointwise_relations: count(counts.pointwise)?,
        permutation_relations: count(counts.permutation)?,
        fold_relations: count(counts.fold)?,
        bounded_recurrences: count(counts.recurrence)?,
        call_summaries: count(expected.call_summaries().len())?,
        tensor_summaries: count(declared_tensor_sites)?,
        error_bounded_relations: count(counts.error_bounded)?,
    })
}

#[derive(Default)]
struct RelationCountsV1 {
    pointwise: usize,
    permutation: usize,
    fold: usize,
    recurrence: usize,
    error_bounded: usize,
}

fn hierarchy_level(level: HierarchicalOwnershipLevelV1) -> ParallelHierarchyLevelV1 {
    match level {
        HierarchicalOwnershipLevelV1::Invocation => ParallelHierarchyLevelV1::Invocation,
        HierarchicalOwnershipLevelV1::Subgroup => ParallelHierarchyLevelV1::Subgroup,
        HierarchicalOwnershipLevelV1::Workgroup => ParallelHierarchyLevelV1::Workgroup,
        HierarchicalOwnershipLevelV1::Grid => ParallelHierarchyLevelV1::Grid,
    }
}

fn require_receipt(
    receipts: &BTreeSet<DigestV1>,
    identity: DigestV1,
) -> Result<(), ProductionParallelReferenceContractErrorV1> {
    if receipts.contains(&identity) {
        Ok(())
    } else {
        Err(ProductionParallelReferenceContractErrorV1::AuthenticatedProofIncomplete { identity })
    }
}

fn has_total_ownership_contract(
    ranked: &ProductionRankedKernelLoweringInputV1,
    output: &SemanticOutputContractV1,
) -> bool {
    ranked.kernel().blocks().iter().flat_map(|block| block.operations()).filter(|operation| {
        matches!(operation, ProductionRankedOperationV1::OwnershipContract { view, coverage: OwnershipCoverageAttr::TotalView, .. } if super::production_ranked_value_identity_v1(*view) == output.view_identity())
    }).count() == 1
}

fn output_roots<'a>(
    output: &SemanticOutputContractV1,
    contract: &'a MirPlironSemanticContractV1,
) -> Option<(&'a SemanticTypedRootV1, &'a SemanticTypedRootV1)> {
    let actual = contract
        .typed_roots()
        .iter()
        .find(|root| root.identity() == output.actual())?;
    let reference = contract
        .typed_roots()
        .iter()
        .find(|root| root.identity() == output.reference())?;
    Some((actual, reference))
}

fn require_collective<'a>(
    index: usize,
    identity: DigestV1,
    kind: SemanticCollectiveKindV1,
    output: &SemanticOutputContractV1,
    contract: &'a MirPlironSemanticContractV1,
) -> Result<&'a SemanticCollectiveContractV1, ProductionParallelReferenceContractErrorV1> {
    let Some(collective) = contract
        .collectives()
        .iter()
        .find(|item| item.identity() == identity)
    else {
        return Err(
            ProductionParallelReferenceContractErrorV1::ScheduleRelationIncomplete {
                index,
                detail: "declared collective is absent from the compiler-derived semantic contract",
            },
        );
    };
    if collective.kind() != kind
        || collective.actual() != output.actual()
        || collective.expected() != output.reference()
        || contract
            .collectives()
            .iter()
            .filter(|item| {
                item.actual() == output.actual() && item.expected() == output.reference()
            })
            .count()
            != 1
    {
        return Err(
            ProductionParallelReferenceContractErrorV1::ScheduleRelationIncomplete {
                index,
                detail: "collective kind or actual/reference roots differ from the live output relation",
            },
        );
    }
    Ok(collective)
}

fn require_fold_order(
    index: usize,
    declared: ParallelFoldOrderV1,
    reference_order: SemanticEvaluationOrderV1,
    collective: &SemanticCollectiveContractV1,
    numerical: ParallelNumericalPolicyV1,
    receipts: &BTreeSet<DigestV1>,
) -> Result<(), ProductionParallelReferenceContractErrorV1> {
    match declared {
        ParallelFoldOrderV1::Preserved if collective.order() == reference_order => Ok(()),
        ParallelFoldOrderV1::Preserved => Err(
            ProductionParallelReferenceContractErrorV1::FoldOrderRejected {
                index,
                detail: "live fold order differs from the sequential reference order",
            },
        ),
        ParallelFoldOrderV1::AlgebraicallyReordered {
            associativity_proof,
            commutativity_proof,
        } => {
            if matches!(
                numerical,
                ParallelNumericalPolicyV1::IeeeOperatorCongruence { .. }
            ) {
                return Err(
                    ProductionParallelReferenceContractErrorV1::FoldOrderRejected {
                        index,
                        detail: "IEEE operator congruence does not justify reassociation; preserve order or provide an explicit finite error policy",
                    },
                );
            }
            require_receipt(receipts, associativity_proof)?;
            require_receipt(receipts, commutativity_proof)
        }
        ParallelFoldOrderV1::ErrorBoundedReordering { proof } => {
            if !matches!(numerical, ParallelNumericalPolicyV1::ErrorBounded { .. }) {
                return Err(
                    ProductionParallelReferenceContractErrorV1::FoldOrderRejected {
                        index,
                        detail: "error-bounded reordering requires an explicit finite numerical error policy",
                    },
                );
            }
            require_receipt(receipts, proof)
        }
    }
}

fn require_numerical_policy(
    index: usize,
    policy: ParallelNumericalPolicyV1,
    output: &SemanticOutputContractV1,
    contract: &MirPlironSemanticContractV1,
    receipts: &BTreeSet<DigestV1>,
) -> Result<(), ProductionParallelReferenceContractErrorV1> {
    let Some((actual, reference)) = output_roots(output, contract) else {
        return Err(
            ProductionParallelReferenceContractErrorV1::NumericalPolicyRejected {
                index,
                detail: "output has no compiler-derived typed actual/reference roots",
            },
        );
    };
    if actual.scalar() != reference.scalar()
        || actual.domain() != output.output_domain()
        || reference.domain() != output.output_domain()
    {
        return Err(
            ProductionParallelReferenceContractErrorV1::NumericalPolicyRejected {
                index,
                detail: "actual/reference scalar types or logical domains differ",
            },
        );
    }
    match policy {
        ParallelNumericalPolicyV1::ExactBitVector => {
            if actual.numerical_policy() == SemanticNumericalPolicyV1::ExactBitVector
                && reference.numerical_policy() == SemanticNumericalPolicyV1::ExactBitVector
                && !matches!(actual.scalar(), SemanticScalarTypeV1::Float(_))
            {
                Ok(())
            } else {
                Err(
                    ProductionParallelReferenceContractErrorV1::NumericalPolicyRejected {
                        index,
                        detail: "exact bit-vector policy requires equal non-floating typed roots with exact bit-vector operator congruence",
                    },
                )
            }
        }
        ParallelNumericalPolicyV1::IeeeOperatorCongruence {
            rounding,
            exceptional_values,
        } => {
            let expected = SemanticNumericalPolicyV1::IeeeOperatorCongruence {
                rounding,
                exceptional_values,
            };
            if matches!(actual.scalar(), SemanticScalarTypeV1::Float(_))
                && actual.numerical_policy() == expected
                && reference.numerical_policy() == expected
            {
                Ok(())
            } else {
                Err(
                    ProductionParallelReferenceContractErrorV1::NumericalPolicyRejected {
                        index,
                        detail: "IEEE operator-congruence mode or exceptional-value policy differs from the live floating typed roots",
                    },
                )
            }
        }
        ParallelNumericalPolicyV1::ErrorBounded {
            witness_root,
            proof,
            ..
        } => {
            let witness = contract
                .typed_roots()
                .iter()
                .find(|root| root.identity() == witness_root);
            let live_float = matches!(actual.scalar(), SemanticScalarTypeV1::Float(_))
                && matches!(reference.scalar(), SemanticScalarTypeV1::Float(_));
            let witness_matches = output.auxiliary_roots().contains(&witness_root)
                && witness.is_some_and(|root| {
                    root.domain() == output.output_domain()
                        && matches!(root.scalar(), SemanticScalarTypeV1::Float(_))
                });
            let roots_have_ieee_model = matches!(
                actual.numerical_policy(),
                SemanticNumericalPolicyV1::IeeeOperatorCongruence { .. }
            ) && actual.numerical_policy()
                == reference.numerical_policy();
            if live_float && witness_matches && roots_have_ieee_model && receipts.contains(&proof) {
                Ok(())
            } else {
                Err(ProductionParallelReferenceContractErrorV1::NumericalProofIncomplete { index })
            }
        }
        ParallelNumericalPolicyV1::UnboundedRelaxed => Err(
            ProductionParallelReferenceContractErrorV1::NumericalPolicyRejected {
                index,
                detail: "unbounded relaxed floating-point semantics cannot establish functional correctness",
            },
        ),
    }
}

fn require_call_summary(
    index: usize,
    summary: &ParallelCallSummaryV1,
    ranked: &ProductionRankedKernelLoweringInputV1,
    contract: &MirPlironSemanticContractV1,
    receipts: &BTreeSet<DigestV1>,
) -> Result<(), ProductionParallelReferenceContractErrorV1> {
    require_receipt(receipts, summary.authenticated_proof())?;
    let actual = contract
        .typed_roots()
        .iter()
        .find(|root| root.identity() == summary.actual_root());
    let reference = contract
        .typed_roots()
        .iter()
        .find(|root| root.identity() == summary.reference_root());
    let (Some(actual), Some(reference)) = (actual, reference) else {
        return Err(ProductionParallelReferenceContractErrorV1::CallSummaryMismatch { index });
    };
    if actual.scalar() != reference.scalar()
        || actual.domain() != reference.domain()
        || !summary_policy_matches(
            summary.numerical_policy(),
            actual,
            reference,
            contract,
            receipts,
        )
    {
        return Err(ProductionParallelReferenceContractErrorV1::CallSummaryMismatch { index });
    }
    match summary.kind() {
        ParallelCallKindV1::SafeRustHelper => Err(
            ProductionParallelReferenceContractErrorV1::CallSummaryDerivationIncomplete {
                index,
                kind: "safe Rust helper",
            },
        ),
        ParallelCallKindV1::CompilerIntrinsic => Err(
            ProductionParallelReferenceContractErrorV1::CallSummaryDerivationIncomplete {
                index,
                kind: "non-tensor compiler intrinsic",
            },
        ),
        ParallelCallKindV1::CooperativeTensorIntrinsic {
            site_ordinal,
            layout_identity,
        } => {
            if summary.argument_count() != 3 || !ranked.tensor_layout_report().is_clean() {
                return Err(
                    ProductionParallelReferenceContractErrorV1::TensorFragmentOwnershipIncomplete {
                        index,
                    },
                );
            }
            let Some(live) = production_tensor_site_identity_v1(ranked, site_ordinal) else {
                return Err(
                    ProductionParallelReferenceContractErrorV1::TensorFragmentOwnershipIncomplete {
                        index,
                    },
                );
            };
            if live != layout_identity || summary.callsite_identity() != live {
                return Err(
                    ProductionParallelReferenceContractErrorV1::CallSummaryMismatch { index },
                );
            }
            Ok(())
        }
    }
}

fn summary_policy_matches(
    policy: ParallelNumericalPolicyV1,
    actual: &SemanticTypedRootV1,
    reference: &SemanticTypedRootV1,
    contract: &MirPlironSemanticContractV1,
    receipts: &BTreeSet<DigestV1>,
) -> bool {
    match policy {
        ParallelNumericalPolicyV1::ExactBitVector => {
            actual.numerical_policy() == SemanticNumericalPolicyV1::ExactBitVector
                && reference.numerical_policy() == SemanticNumericalPolicyV1::ExactBitVector
        }
        ParallelNumericalPolicyV1::IeeeOperatorCongruence {
            rounding,
            exceptional_values,
        } => {
            let policy = SemanticNumericalPolicyV1::IeeeOperatorCongruence {
                rounding,
                exceptional_values,
            };
            actual.numerical_policy() == policy && reference.numerical_policy() == policy
        }
        ParallelNumericalPolicyV1::ErrorBounded {
            witness_root,
            proof,
            ..
        } => {
            contract
                .typed_roots()
                .iter()
                .any(|root| root.identity() == witness_root && root.domain() == actual.domain())
                && receipts.contains(&proof)
        }
        ParallelNumericalPolicyV1::UnboundedRelaxed => false,
    }
}

fn tensor_site_count(ranked: &ProductionRankedKernelLoweringInputV1) -> usize {
    ranked
        .kernel()
        .blocks()
        .iter()
        .flat_map(|block| block.operations())
        .filter(|operation| matches!(operation, ProductionRankedOperationV1::TensorLayout { .. }))
        .count()
}

/// Compiler-derived identity of one exact cooperative tensor site. The whole
/// ranked-kernel identity binds its fragment maps, convergence, CFG, and every
/// other operation; the ordinal selects one live tensor site without naming a
/// target instruction or workload.
pub fn production_tensor_site_identity_v1(
    ranked: &ProductionRankedKernelLoweringInputV1,
    site_ordinal: u32,
) -> Option<DigestV1> {
    let live = tensor_site_count(ranked);
    let ordinal = usize::try_from(site_ordinal).ok()?;
    if ordinal >= live {
        return None;
    }
    let mut digest = Sha256::new();
    digest.update((TENSOR_SITE_DOMAIN_V1.len() as u64).to_le_bytes());
    digest.update(TENSOR_SITE_DOMAIN_V1);
    digest.update(super::middle_end_evidence_v4::derive_ranked_kernel_identity(ranked));
    digest.update(site_ordinal.to_le_bytes());
    Some(DigestV1::from_untrusted_bytes(digest.finalize().into()))
}

/// Compiler-derived identity of the finite bound already established for one
/// canonical dynamic loop by the MIR/PLIRON semantic gate. Current production
/// support derives only the full finite scalar-type bound; a future narrower
/// range analysis must use a new domain rather than changing this identity.
pub fn production_dynamic_loop_bound_identity_v1(
    contract: &MirPlironSemanticContractV1,
    loop_contract: &fe2o3_functional_proof::SemanticLoopContractV1,
) -> DigestV1 {
    let mut digest = Sha256::new();
    digest.update((DYNAMIC_BOUND_DOMAIN_V1.len() as u64).to_le_bytes());
    digest.update(DYNAMIC_BOUND_DOMAIN_V1);
    digest.update(contract.canonical_sha256().as_bytes());
    digest.update(loop_contract.identity().as_bytes());
    digest.update(loop_contract.iteration_domain().as_bytes());
    digest.update(loop_contract.variant().as_bytes());
    digest.update(loop_contract.maximum_steps().to_le_bytes());
    DigestV1::from_untrusted_bytes(digest.finalize().into())
}

fn count(value: usize) -> Result<u64, ProductionParallelReferenceContractErrorV1> {
    u64::try_from(value).map_err(|_| ProductionParallelReferenceContractErrorV1::CounterOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(tag: u8) -> DigestV1 {
        DigestV1::from_untrusted_bytes([tag; 32])
    }

    #[test]
    fn missing_compiler_facts_are_incomplete_never_success() {
        for error in [
            ProductionParallelReferenceContractErrorV1::OutputCoverageIncomplete {
                declared: 2,
                live: 1,
            },
            ProductionParallelReferenceContractErrorV1::HierarchyCoverageIncomplete {
                level: ParallelHierarchyLevelV1::Workgroup,
            },
            ProductionParallelReferenceContractErrorV1::AuthenticatedProofIncomplete {
                identity: digest(1),
            },
            ProductionParallelReferenceContractErrorV1::ScheduleRelationIncomplete {
                index: 3,
                detail: "missing live recurrence",
            },
            ProductionParallelReferenceContractErrorV1::DynamicBoundProofIncomplete { index: 4 },
            ProductionParallelReferenceContractErrorV1::NumericalProofIncomplete { index: 5 },
            ProductionParallelReferenceContractErrorV1::CallSummaryDerivationIncomplete {
                index: 6,
                kind: "safe Rust helper",
            },
            ProductionParallelReferenceContractErrorV1::TensorFragmentOwnershipIncomplete {
                index: 7,
            },
        ] {
            assert!(error.is_incomplete(), "{error}");
            assert!(error.to_string().starts_with("error[FE2O3-PARALLEL-"));
        }
    }

    #[test]
    fn contradictions_are_rejections_not_incomplete_results() {
        for error in [
            ProductionParallelReferenceContractErrorV1::SemanticContractMismatch,
            ProductionParallelReferenceContractErrorV1::OutputRelationMismatch { index: 0 },
            ProductionParallelReferenceContractErrorV1::FoldOrderRejected {
                index: 0,
                detail: "IEEE reassociation",
            },
            ProductionParallelReferenceContractErrorV1::NumericalPolicyRejected {
                index: 0,
                detail: "unbounded relaxed",
            },
            ProductionParallelReferenceContractErrorV1::CallSummaryMismatch { index: 0 },
            ProductionParallelReferenceContractErrorV1::UnmodeledTensorSites {
                declared: 0,
                live: 1,
            },
        ] {
            assert!(!error.is_incomplete(), "{error}");
        }
    }
}
