//! Production join from compiler-derived sequential/parallel facts to one
//! workload-neutral parallel-reference contract.

use std::{collections::BTreeMap, error::Error, fmt};

use dialect_kernel::OwnershipCoverageAttr;
use fe2o3_functional_proof::{
    COMPLETE_GPU_HIERARCHY_V1, MirPlironSemanticContractV1, ParallelCallKindV1,
    ParallelCallSummaryV1, ParallelExecutionScopeV1, ParallelFoldOrderV1, ParallelHierarchyLevelV1,
    ParallelNumericalPolicyV1, ParallelOutputRelationV1, ParallelReferenceContractErrorV1,
    ParallelReferenceContractV1, ParallelScheduleRelationV1, SemanticCollectiveContractV1,
    SemanticCollectiveKindV1, SemanticEvaluationOrderV1, SemanticFiniteExtentV1,
    SemanticNumericalPolicyV1, SemanticOutputContractV1, SemanticScalarTypeV1, SemanticTypedRootV1,
};
use fe2o3_kernel_analysis::{
    HierarchicalOwnershipLevelV1, HierarchicalOwnershipRegionV1, HierarchicalRegionIdentityV1,
};
use fe2o3_proof_contracts::DigestV1;
use sha2::{Digest as _, Sha256};

use super::{
    ProductionMiddleEndEvidenceV5, ProductionMirPlironSemanticContractReportV1,
    ProductionRankedKernelLoweringInputV1, ProductionRankedOperationV1, ProductionRankedValueV1,
};

const DYNAMIC_BOUND_DOMAIN_V1: &[u8] = b"FE2O3/PARALLEL-REFERENCE/DYNAMIC-BOUND/V1\0";
const DERIVED_RELATION_DOMAIN_V1: &[u8] = b"FE2O3/PARALLEL-REFERENCE/DERIVED-RELATION/V1\0";
const TENSOR_LAYOUT_DOMAIN_V1: &[u8] = b"FE2O3/PARALLEL-REFERENCE/TENSOR-LAYOUT/V1\0";
const TENSOR_CALLSITE_DOMAIN_V1: &[u8] = b"FE2O3/PARALLEL-REFERENCE/TENSOR-CALLSITE/V1\0";
const TENSOR_SUMMARY_DOMAIN_V1: &[u8] = b"FE2O3/PARALLEL-REFERENCE/TENSOR-SUMMARY/V1\0";
const TENSOR_BARRIER_DOMAIN_V1: &[u8] = b"FE2O3/PARALLEL-REFERENCE/TENSOR-BARRIER/V1\0";
const TENSOR_DIRECT_DOMAIN_V1: &[u8] = b"FE2O3/PARALLEL-REFERENCE/TENSOR-DIRECT/V1\0";
const TENSOR_CONTRIBUTION_DOMAIN_V1: &[u8] = b"FE2O3/PARALLEL-REFERENCE/TENSOR-CONTRIBUTION/V1\0";
const TENSOR_SCATTER_DOMAIN_V1: &[u8] = b"FE2O3/PARALLEL-REFERENCE/TENSOR-SCATTER/V1\0";
const OUTPUT_OWNERSHIP_DOMAIN_V1: &[u8] = b"FE2O3/PARALLEL-REFERENCE/OUTPUT-OWNERSHIP/V1\0";
const OUTPUT_FRAME_DOMAIN_V1: &[u8] = b"FE2O3/PARALLEL-REFERENCE/OUTPUT-FRAME/V1\0";
const OUTPUT_PRODUCT_DOMAIN_V1: &[u8] = b"FE2O3/PARALLEL-REFERENCE/OUTPUT-PRODUCT/V1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionParallelReferenceContractReportV1 {
    contract_identity: DigestV1,
    output_product_identity: DigestV1,
    output_relations: u64,
    output_frames: u64,
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
    pub const fn output_product_identity(self) -> DigestV1 {
        self.output_product_identity
    }
    pub const fn output_frames(self) -> u64 {
        self.output_frames
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
    StableRankedViewIdentityMissing { index: usize },
    DuplicateOutputView { first: usize, second: usize },
    OutputSeparationIncomplete { first: usize, second: usize },
    OutputOwnershipMismatch { index: usize },
    OutputProductMismatch,
    HierarchyCoverageIncomplete { level: ParallelHierarchyLevelV1 },
    AuthenticatedProofIncomplete { identity: DigestV1 },
    ScheduleRelationIncomplete { index: usize, detail: &'static str },
    FoldOrderRejected { index: usize, detail: &'static str },
    DynamicBoundProofIncomplete { index: usize },
    NumericalPolicyRejected { index: usize, detail: &'static str },
    NumericalProofIncomplete { index: usize },
    CallSummaryDerivationIncomplete { index: usize, kind: &'static str },
    CallSummaryMismatch { index: usize },
    TensorFragmentOwnershipIncomplete { index: usize, detail: &'static str },
    UnmodeledTensorSites { declared: usize, live: usize },
    TensorOutputAssociationIncomplete { outputs: usize },
    ContractConstruction(ParallelReferenceContractErrorV1),
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
                | Self::StableRankedViewIdentityMissing { .. }
                | Self::OutputSeparationIncomplete { .. }
                | Self::OutputOwnershipMismatch { .. }
                | Self::AuthenticatedProofIncomplete { .. }
                | Self::ScheduleRelationIncomplete { .. }
                | Self::DynamicBoundProofIncomplete { .. }
                | Self::NumericalProofIncomplete { .. }
                | Self::CallSummaryDerivationIncomplete { .. }
                | Self::TensorFragmentOwnershipIncomplete { .. }
                | Self::TensorOutputAssociationIncomplete { .. }
        )
    }
}

impl fmt::Display for ProductionParallelReferenceContractErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SemanticContractMismatch => formatter.write_str("error[FE2O3-PARALLEL-001]: parallel relation does not bind the exact compiler-verified MIR/PLIRON semantic contract"),
            Self::OutputCoverageIncomplete { declared, live } => write!(formatter, "error[FE2O3-PARALLEL-002]: parallel reference coverage is incomplete: {declared} logical output relations were declared but {live} live total-output ownership proofs were derived"),
            Self::OutputRelationMismatch { index } => write!(formatter, "error[FE2O3-PARALLEL-003]: parallel output relation {index} does not match its compiler-derived output domain, view, values, or ownership contract"),
            Self::StableRankedViewIdentityMissing { index } => write!(formatter, "error[FE2O3-PARALLEL-016]: output {index} does not resolve to exactly one compiler-materialized ranked view identity"),
            Self::DuplicateOutputView { first, second } => write!(formatter, "error[FE2O3-PARALLEL-018]: outputs {first} and {second} resolve to the same ranked view"),
            Self::OutputSeparationIncomplete { first, second } => write!(formatter, "error[FE2O3-PARALLEL-019]: compiler-derived memory facts do not prove outputs {first} and {second} are disjoint"),
            Self::OutputOwnershipMismatch { index } => write!(formatter, "error[FE2O3-PARALLEL-020]: output {index} lacks an exact output-specific TotalView ownership and complete hierarchy binding"),
            Self::OutputProductMismatch => formatter.write_str("error[FE2O3-PARALLEL-021]: parallel output product does not match the compiler-derived ordered output frames"),
            Self::HierarchyCoverageIncomplete { level } => write!(formatter, "error[FE2O3-PARALLEL-004]: compiler could not derive nonempty {level:?} ownership while relating the sequential output domain to the complete GPU hierarchy"),
            Self::AuthenticatedProofIncomplete { identity } => write!(formatter, "error[FE2O3-PARALLEL-005]: no retained authenticated per-compilation proof has identity {identity:?}"),
            Self::ScheduleRelationIncomplete { index, detail } => write!(formatter, "error[FE2O3-PARALLEL-006]: schedule relation {index} is incomplete: {detail}"),
            Self::FoldOrderRejected { index, detail } => write!(formatter, "error[FE2O3-PARALLEL-007]: fold order for relation {index} is not justified: {detail}"),
            Self::DynamicBoundProofIncomplete { index } => write!(formatter, "error[FE2O3-PARALLEL-008]: dynamic bounded recurrence {index} does not match the compiler-derived finite-bound identity for the live canonical loop"),
            Self::NumericalPolicyRejected { index, detail } => write!(formatter, "error[FE2O3-PARALLEL-009]: numerical policy for relation {index} is invalid: {detail}"),
            Self::NumericalProofIncomplete { index } => write!(formatter, "error[FE2O3-PARALLEL-010]: finite error policy for relation {index} lacks a live typed witness or retained authenticated proof"),
            Self::CallSummaryDerivationIncomplete { index, kind } => write!(formatter, "error[FE2O3-PARALLEL-011]: compiler cannot independently derive {kind} call summary {index} from the current ranked IR; declaration-only summaries are not evidence"),
            Self::CallSummaryMismatch { index } => write!(formatter, "error[FE2O3-PARALLEL-012]: helper or intrinsic summary {index} differs from live typed roots, scope, callsite, or authenticated proof"),
            Self::TensorFragmentOwnershipIncomplete { index, detail } => write!(formatter, "error[FE2O3-PARALLEL-013]: cooperative tensor summary {index} is incomplete: {detail}"),
            Self::UnmodeledTensorSites { declared, live } => write!(formatter, "error[FE2O3-PARALLEL-014]: parallel contract models {declared} cooperative tensor sites but the live ranked graph contains {live}"),
            Self::TensorOutputAssociationIncomplete { outputs } => write!(formatter, "error[FE2O3-PARALLEL-022]: compiler-derived tensor dataflow does not uniquely associate each cooperative tensor site with one of {outputs} output relations"),
            Self::CounterOverflow => formatter.write_str("error[FE2O3-PARALLEL-015]: parallel relation count cannot be represented in the production report"),
            Self::ContractConstruction(error) => write!(formatter, "error[FE2O3-PARALLEL-017]: compiler-derived parallel contract was invalid: {error}"),
        }
    }
}

impl Error for ProductionParallelReferenceContractErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ContractConstruction(error) => Some(error),
            _ => None,
        }
    }
}

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

/// Derives the strongest currently supported parallel relation from immutable
/// compiler-owned facts, then independently reconciles the result.
pub fn derive_and_require_parallel_reference_contract_v1(
    ranked: &ProductionRankedKernelLoweringInputV1,
    evidence: &ProductionMiddleEndEvidenceV5,
    semantics: ProductionMirPlironSemanticContractReportV1,
    semantic_contract: &MirPlironSemanticContractV1,
) -> Result<
    (
        ParallelReferenceContractV1,
        ProductionParallelReferenceContractReportV1,
    ),
    ProductionParallelReferenceContractErrorV1,
> {
    let mut relations = Vec::with_capacity(semantic_contract.outputs().len());
    let mut call_summaries = Vec::new();
    let bindings = derive_live_output_bindings_v1(ranked, evidence, semantic_contract)?;
    let output_product_identity =
        output_product_identity_v1(semantic_contract, evidence, &bindings);
    for (index, output) in semantic_contract.outputs().iter().enumerate() {
        let binding = &bindings[index];
        let proof = binding.authenticated_proof;
        let (actual, reference) = output_roots(output, semantic_contract)
            .ok_or(ProductionParallelReferenceContractErrorV1::OutputRelationMismatch { index })?;
        let exact_numerical_policy = match (
            actual.scalar(),
            actual.numerical_policy(),
            reference.numerical_policy(),
        ) {
            (
                SemanticScalarTypeV1::Boolean
                | SemanticScalarTypeV1::Signed(_)
                | SemanticScalarTypeV1::Unsigned(_),
                SemanticNumericalPolicyV1::ExactBitVector,
                SemanticNumericalPolicyV1::ExactBitVector,
            ) => ParallelNumericalPolicyV1::ExactBitVector,
            (
                SemanticScalarTypeV1::Float(_),
                SemanticNumericalPolicyV1::IeeeOperatorCongruence {
                    rounding,
                    exceptional_values,
                },
                SemanticNumericalPolicyV1::IeeeOperatorCongruence {
                    rounding: reference_rounding,
                    exceptional_values: reference_exceptional_values,
                },
            ) if rounding == reference_rounding
                && exceptional_values == reference_exceptional_values =>
            {
                ParallelNumericalPolicyV1::IeeeOperatorCongruence {
                    rounding,
                    exceptional_values,
                }
            }
            _ => {
                return Err(
                    ProductionParallelReferenceContractErrorV1::NumericalPolicyRejected {
                        index,
                        detail: "compiler-derived actual and reference roots do not share one admitted exact numerical policy",
                    },
                );
            }
        };
        let numerical_policy = match numerical_refinement_for_output(ranked, output) {
            Some((contract, proof)) => ParallelNumericalPolicyV1::ErrorBounded {
                absolute_error_f64_bits: contract.absolute_error_f64_bits(),
                relative_error_f64_bits: contract.relative_error_f64_bits(),
                witness_root: contract.request_shape_hash(),
                proof,
            },
            None => exact_numerical_policy,
        };
        let matching_collectives = semantic_contract
            .collectives()
            .iter()
            .filter(|collective| {
                collective.view_identity() == output.view_identity()
                    && collective.actual() == output.actual()
                    && collective.expected() == output.reference()
                    && collective.target_domain() == output.output_domain()
            })
            .collect::<Vec<_>>();
        let schedule = match matching_collectives.as_slice() {
            [] if actual.commitment() == reference.commitment() => {
                ParallelScheduleRelationV1::PointwiseBijection
            }
            [] => {
                return Err(
                    ProductionParallelReferenceContractErrorV1::ScheduleRelationIncomplete {
                        index,
                        detail: "distinct actual/reference expressions have no unique live collective schedule",
                    },
                );
            }
            [collective] => match collective.kind() {
                SemanticCollectiveKindV1::PermutationGather => {
                    ParallelScheduleRelationV1::Permutation {
                        collective: collective.identity(),
                    }
                }
                SemanticCollectiveKindV1::FiniteFold => ParallelScheduleRelationV1::Fold {
                    collective: collective.identity(),
                    order: ParallelFoldOrderV1::Preserved,
                    reference_order: collective.order(),
                },
                SemanticCollectiveKindV1::FiniteRecurrence => {
                    let loops = semantic_contract
                        .loops()
                        .iter()
                        .filter(|loop_contract| {
                            loop_contract.iteration_domain() == collective.source_domain()
                        })
                        .collect::<Vec<_>>();
                    let [loop_contract] = loops.as_slice() else {
                        return Err(
                            ProductionParallelReferenceContractErrorV1::ScheduleRelationIncomplete {
                                index,
                                detail: "finite recurrence does not have exactly one canonical loop over its contribution domain",
                            },
                        );
                    };
                    let dynamic = semantic_contract
                        .domains()
                        .iter()
                        .find(|domain| domain.identity() == loop_contract.iteration_domain())
                        .is_some_and(|domain| {
                            domain.extents().iter().any(|extent| {
                                matches!(extent, SemanticFiniteExtentV1::Dynamic { .. })
                            })
                        });
                    ParallelScheduleRelationV1::BoundedRecurrence {
                        collective: collective.identity(),
                        loop_contract: loop_contract.identity(),
                        dynamic_bound_proof: dynamic.then(|| {
                            production_dynamic_loop_bound_identity_v1(
                                semantic_contract,
                                loop_contract,
                            )
                        }),
                        reference_order: collective.order(),
                    }
                }
            },
            _ => {
                return Err(
                    ProductionParallelReferenceContractErrorV1::ScheduleRelationIncomplete {
                        index,
                        detail: "output has more than one candidate live collective schedule",
                    },
                );
            }
        };
        let tensor_summaries = derive_tensor_call_summaries_v1(
            ranked,
            evidence,
            semantic_contract,
            output,
            index,
            schedule,
            numerical_policy,
            proof,
        )?;
        let tensor_summary_ids = tensor_summaries
            .iter()
            .map(ParallelCallSummaryV1::identity)
            .collect();
        call_summaries.extend(tensor_summaries);
        relations.push(
            ParallelOutputRelationV1::new(
                derived_relation_identity(semantic_contract, output, index),
                output.identity(),
                output.output_domain(),
                binding.ranked_view_identity,
                binding.ownership_identity,
                binding.frame_identity,
                schedule,
                numerical_policy,
                COMPLETE_GPU_HIERARCHY_V1.to_vec(),
                tensor_summary_ids,
                proof,
            )
            .map_err(ProductionParallelReferenceContractErrorV1::ContractConstruction)?,
        );
    }
    let contract = ParallelReferenceContractV1::new(
        semantic_contract.canonical_sha256(),
        output_product_identity,
        relations,
        call_summaries,
    )
    .map_err(ProductionParallelReferenceContractErrorV1::ContractConstruction)?;
    let report = require_parallel_reference_contract_v1(
        ranked,
        evidence,
        semantics,
        semantic_contract,
        &contract,
    )?;
    Ok((contract, report))
}

fn derived_relation_identity(
    contract: &MirPlironSemanticContractV1,
    output: &SemanticOutputContractV1,
    index: usize,
) -> DigestV1 {
    let mut digest = Sha256::new();
    digest.update((DERIVED_RELATION_DOMAIN_V1.len() as u64).to_le_bytes());
    digest.update(DERIVED_RELATION_DOMAIN_V1);
    digest.update(contract.canonical_sha256().as_bytes());
    digest.update(output.identity().as_bytes());
    digest.update((index as u64).to_le_bytes());
    DigestV1::from_untrusted_bytes(digest.finalize().into())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LiveOutputBindingV1 {
    ranked_view_identity: DigestV1,
    live_view_name: String,
    allocation_origin: u64,
    noalias_class: u64,
    authenticated_proof: DigestV1,
    ownership_identity: DigestV1,
    frame_identity: DigestV1,
}
struct LiveOutputFactsIndexV1 {
    effects: BTreeMap<DigestV1, Vec<(ProductionRankedValueV1, DigestV1)>>,
    storage: BTreeMap<ProductionRankedValueV1, Vec<(u64, u64)>>,
    total_ownership: BTreeMap<DigestV1, usize>,
}

impl LiveOutputFactsIndexV1 {
    fn from_ranked(ranked: &ProductionRankedKernelLoweringInputV1) -> Self {
        let mut facts = Self {
            effects: BTreeMap::new(),
            storage: BTreeMap::new(),
            total_ownership: BTreeMap::new(),
        };
        for operation in ranked
            .kernel()
            .blocks()
            .iter()
            .flat_map(|block| block.operations())
        {
            match operation {
                ProductionRankedOperationV1::RequireEffectRefinement { contract, proof } => {
                    facts
                        .effects
                        .entry(super::production_effect_contract_identity_v1(
                            contract.contract_identity(),
                        ))
                        .or_default()
                        .push((contract.view(), proof.receipt_identity().digest()));
                }
                ProductionRankedOperationV1::View {
                    result,
                    allocation_origin,
                    noalias_class,
                    ..
                }
                | ProductionRankedOperationV1::ViewInSpace {
                    result,
                    allocation_origin,
                    noalias_class,
                    ..
                } => {
                    facts
                        .storage
                        .entry(ProductionRankedValueV1::Local(*result))
                        .or_default()
                        .push((*allocation_origin, *noalias_class));
                }
                ProductionRankedOperationV1::OwnershipContract {
                    view,
                    coverage: OwnershipCoverageAttr::TotalView,
                    ..
                } => {
                    *facts
                        .total_ownership
                        .entry(super::production_ranked_value_identity_v1(*view))
                        .or_default() += 1;
                }
                _ => {}
            }
        }
        facts
    }
}

fn derive_live_output_bindings_v1(
    ranked: &ProductionRankedKernelLoweringInputV1,
    evidence: &ProductionMiddleEndEvidenceV5,
    semantic_contract: &MirPlironSemanticContractV1,
) -> Result<Vec<LiveOutputBindingV1>, ProductionParallelReferenceContractErrorV1> {
    let mut bindings = Vec::with_capacity(semantic_contract.outputs().len());
    let facts = LiveOutputFactsIndexV1::from_ranked(ranked);
    let mut regions_by_view = BTreeMap::<&str, Vec<&HierarchicalOwnershipRegionV1>>::new();
    for region in ranked.ownership_report().regions().iter().filter(|region| {
        region.coverage() == OwnershipCoverageAttr::TotalView && region.element_count() != 0
    }) {
        regions_by_view
            .entry(region.view())
            .or_default()
            .push(region);
    }
    for regions in regions_by_view.values_mut() {
        regions.sort_by(|left, right| left.identity().cmp(right.identity()));
    }
    for (index, output) in semantic_contract.outputs().iter().enumerate() {
        let Some(effect_matches) = facts.effects.get(&output.identity()) else {
            return Err(
                ProductionParallelReferenceContractErrorV1::OutputRelationMismatch { index },
            );
        };
        let [(view, authenticated_proof)] = effect_matches.as_slice() else {
            return Err(
                ProductionParallelReferenceContractErrorV1::OutputRelationMismatch { index },
            );
        };
        let (view, authenticated_proof) = (*view, *authenticated_proof);
        let ranked_view_identity = super::production_ranked_value_identity_v1(view);
        if ranked_view_identity != output.view_identity() {
            return Err(
                ProductionParallelReferenceContractErrorV1::OutputRelationMismatch { index },
            );
        }
        let live_view_name = ranked
            .live_ranked_view_name(view)
            .ok_or(
                ProductionParallelReferenceContractErrorV1::StableRankedViewIdentityMissing {
                    index,
                },
            )?
            .to_owned();
        let Some(storage_matches) = facts.storage.get(&view) else {
            return Err(
                ProductionParallelReferenceContractErrorV1::StableRankedViewIdentityMissing {
                    index,
                },
            );
        };
        let [(allocation_origin, noalias_class)] = storage_matches.as_slice() else {
            return Err(
                ProductionParallelReferenceContractErrorV1::StableRankedViewIdentityMissing {
                    index,
                },
            );
        };
        let (allocation_origin, noalias_class) = (*allocation_origin, *noalias_class);
        if facts.total_ownership.get(&output.view_identity()) != Some(&1) {
            return Err(
                ProductionParallelReferenceContractErrorV1::OutputOwnershipMismatch { index },
            );
        }
        let regions = regions_by_view
            .get(live_view_name.as_str())
            .ok_or(ProductionParallelReferenceContractErrorV1::OutputOwnershipMismatch { index })?;
        for required in [
            ParallelHierarchyLevelV1::Invocation,
            ParallelHierarchyLevelV1::Subgroup,
            ParallelHierarchyLevelV1::Workgroup,
            ParallelHierarchyLevelV1::Grid,
        ] {
            if !regions
                .iter()
                .any(|region| hierarchy_level(region.identity().level()) == required)
            {
                return Err(
                    ProductionParallelReferenceContractErrorV1::HierarchyCoverageIncomplete {
                        level: required,
                    },
                );
            }
        }
        let ownership_identity = output_ownership_identity_v1(
            semantic_contract,
            evidence,
            output,
            ranked_view_identity,
            authenticated_proof,
            allocation_origin,
            noalias_class,
            &live_view_name,
            regions,
        );
        let frame_identity = output_frame_identity_v1(
            semantic_contract,
            evidence,
            output,
            ranked_view_identity,
            authenticated_proof,
            ownership_identity,
        );
        bindings.push(LiveOutputBindingV1 {
            ranked_view_identity,
            live_view_name,
            allocation_origin,
            noalias_class,
            authenticated_proof,
            ownership_identity,
            frame_identity,
        });
    }
    let mut ranked_views = BTreeMap::new();
    let mut live_views = BTreeMap::new();
    let mut noalias_classes = BTreeMap::new();
    // The mandatory alias and ownership passes currently grant separation only
    // to distinct nonzero noalias classes. Same-allocation subviews therefore
    // fail closed until ranked IR carries independently proved subview ranges.
    for (second, binding) in bindings.iter().enumerate() {
        if let Some(first) = ranked_views.insert(binding.ranked_view_identity, second) {
            return Err(
                ProductionParallelReferenceContractErrorV1::DuplicateOutputView { first, second },
            );
        }
        if let Some(first) = live_views.insert(binding.live_view_name.clone(), second) {
            return Err(
                ProductionParallelReferenceContractErrorV1::DuplicateOutputView { first, second },
            );
        }
        if bindings.len() > 1 && binding.noalias_class == 0 {
            let other = usize::from(second == 0);
            return Err(
                ProductionParallelReferenceContractErrorV1::OutputSeparationIncomplete {
                    first: other.min(second),
                    second: other.max(second),
                },
            );
        }
        if let Some(first) = noalias_classes.insert(binding.noalias_class, second) {
            return Err(
                ProductionParallelReferenceContractErrorV1::OutputSeparationIncomplete {
                    first,
                    second,
                },
            );
        }
    }
    Ok(bindings)
}

#[allow(clippy::too_many_arguments)]
fn output_ownership_identity_v1(
    semantic_contract: &MirPlironSemanticContractV1,
    evidence: &ProductionMiddleEndEvidenceV5,
    output: &SemanticOutputContractV1,
    ranked_view_identity: DigestV1,
    authenticated_proof: DigestV1,
    allocation_origin: u64,
    noalias_class: u64,
    live_view_name: &str,
    regions: &[&HierarchicalOwnershipRegionV1],
) -> DigestV1 {
    let mut digest = Sha256::new();
    digest_blob(&mut digest, OUTPUT_OWNERSHIP_DOMAIN_V1);
    digest.update(semantic_contract.canonical_sha256().as_bytes());
    digest.update(evidence.identity().sha256());
    digest.update(output.identity().as_bytes());
    digest.update(ranked_view_identity.as_bytes());
    digest.update(authenticated_proof.as_bytes());
    digest.update(allocation_origin.to_le_bytes());
    digest.update(noalias_class.to_le_bytes());
    digest_blob(&mut digest, live_view_name.as_bytes());
    digest.update((regions.len() as u64).to_le_bytes());
    for region in regions {
        digest_region(&mut digest, region);
    }
    DigestV1::from_untrusted_bytes(digest.finalize().into())
}

// A clean middle-end evidence identity binds the full ranked graph and mandatory
// pass sequence. Joining it with exact-once TotalView ownership and the exact
// effect receipt makes this the final-value/frame identity for one output; no
// declaration contributes evidence to this digest.
fn output_frame_identity_v1(
    semantic_contract: &MirPlironSemanticContractV1,
    evidence: &ProductionMiddleEndEvidenceV5,
    output: &SemanticOutputContractV1,
    ranked_view_identity: DigestV1,
    authenticated_proof: DigestV1,
    ownership_identity: DigestV1,
) -> DigestV1 {
    let mut digest = Sha256::new();
    digest_blob(&mut digest, OUTPUT_FRAME_DOMAIN_V1);
    digest.update(semantic_contract.canonical_sha256().as_bytes());
    digest.update(evidence.identity().sha256());
    digest.update(output.identity().as_bytes());
    digest.update(ranked_view_identity.as_bytes());
    digest.update(authenticated_proof.as_bytes());
    digest.update(ownership_identity.as_bytes());
    digest.update(
        evidence
            .coverage_summary()
            .total_view_declared()
            .to_le_bytes(),
    );
    digest.update(
        evidence
            .coverage_summary()
            .total_view_proved()
            .to_le_bytes(),
    );
    DigestV1::from_untrusted_bytes(digest.finalize().into())
}

fn output_product_identity_v1(
    semantic_contract: &MirPlironSemanticContractV1,
    evidence: &ProductionMiddleEndEvidenceV5,
    bindings: &[LiveOutputBindingV1],
) -> DigestV1 {
    let mut digest = Sha256::new();
    digest_blob(&mut digest, OUTPUT_PRODUCT_DOMAIN_V1);
    digest.update(semantic_contract.canonical_sha256().as_bytes());
    digest.update(evidence.identity().sha256());
    digest.update((bindings.len() as u64).to_le_bytes());
    for (output, binding) in semantic_contract.outputs().iter().zip(bindings) {
        digest.update(output.identity().as_bytes());
        digest.update(binding.ranked_view_identity.as_bytes());
        digest.update(binding.authenticated_proof.as_bytes());
        digest.update(binding.ownership_identity.as_bytes());
        digest.update(binding.frame_identity.as_bytes());
        digest.update(binding.allocation_origin.to_le_bytes());
        digest.update(binding.noalias_class.to_le_bytes());
    }
    DigestV1::from_untrusted_bytes(digest.finalize().into())
}

fn digest_region(digest: &mut Sha256, region: &HierarchicalOwnershipRegionV1) {
    match region.identity() {
        HierarchicalRegionIdentityV1::Invocation(coordinates) => {
            digest.update([1]);
            digest.update((coordinates.len() as u64).to_le_bytes());
            for coordinate in coordinates {
                digest.update(coordinate.to_le_bytes());
            }
        }
        HierarchicalRegionIdentityV1::Subgroup {
            workgroup,
            subgroup,
        } => {
            digest.update([2]);
            digest.update(workgroup.to_le_bytes());
            digest.update(subgroup.to_le_bytes());
        }
        HierarchicalRegionIdentityV1::Workgroup(workgroup) => {
            digest.update([3]);
            digest.update(workgroup.to_le_bytes());
        }
        HierarchicalRegionIdentityV1::Grid(grid) => {
            digest.update([4]);
            digest.update(grid.to_le_bytes());
        }
    }
    digest.update((region.element_count() as u64).to_le_bytes());
    digest.update((region.bounds().len() as u64).to_le_bytes());
    for bound in region.bounds() {
        digest.update(bound.minimum().to_le_bytes());
        digest.update(bound.maximum().to_le_bytes());
    }
    digest.update([u8::from(region.is_dense_rectangle())]);
}

fn digest_blob(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
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

    let bindings = derive_live_output_bindings_v1(ranked, evidence, semantic_contract)?;
    let derived_product = output_product_identity_v1(semantic_contract, evidence, &bindings);
    if expected.output_product_identity() != derived_product {
        return Err(ProductionParallelReferenceContractErrorV1::OutputProductMismatch);
    }

    let mut counts = RelationCountsV1::default();
    let mut derived_call_summaries = Vec::new();
    for (index, output) in semantic_contract.outputs().iter().enumerate() {
        let Some(relation) = expected.relations().get(index) else {
            return Err(
                ProductionParallelReferenceContractErrorV1::OutputRelationMismatch { index },
            );
        };
        let binding = &bindings[index];
        if relation.output_contract() != output.identity()
            || relation.logical_domain() != output.output_domain()
            || relation.ranked_view_identity() != binding.ranked_view_identity
            || relation.ownership_identity() != binding.ownership_identity
            || relation.frame_identity() != binding.frame_identity
            || relation.hierarchy() != fe2o3_functional_proof::COMPLETE_GPU_HIERARCHY_V1
        {
            return Err(
                ProductionParallelReferenceContractErrorV1::OutputRelationMismatch { index },
            );
        }
        let output_proof = binding.authenticated_proof;
        if relation.authenticated_proof() != output_proof {
            return Err(
                ProductionParallelReferenceContractErrorV1::AuthenticatedProofIncomplete {
                    identity: relation.authenticated_proof(),
                },
            );
        }
        require_numerical_policy(
            index,
            relation.numerical_policy(),
            output,
            semantic_contract,
            Some(ranked),
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
                require_fold_order(index, order, reference_order, collective)?;
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
        let tensor_summaries = derive_tensor_call_summaries_v1(
            ranked,
            evidence,
            semantic_contract,
            output,
            index,
            relation.schedule(),
            relation.numerical_policy(),
            output_proof,
        )?;
        let derived_ids = tensor_summaries
            .iter()
            .map(ParallelCallSummaryV1::identity)
            .collect::<Vec<_>>();
        if relation.call_summaries() != derived_ids {
            return Err(ProductionParallelReferenceContractErrorV1::CallSummaryMismatch { index });
        }
        derived_call_summaries.extend(tensor_summaries);
    }

    let live_tensor_sites = tensor_site_count(ranked);
    reconcile_tensor_summaries_v1(
        expected.call_summaries(),
        &derived_call_summaries,
        live_tensor_sites,
    )?;

    Ok(ProductionParallelReferenceContractReportV1 {
        contract_identity: expected.canonical_sha256(),
        output_product_identity: derived_product,
        output_relations: count(expected.relations().len())?,
        output_frames: count(bindings.len())?,
        pointwise_relations: count(counts.pointwise)?,
        permutation_relations: count(counts.permutation)?,
        fold_relations: count(counts.fold)?,
        bounded_recurrences: count(counts.recurrence)?,
        call_summaries: count(expected.call_summaries().len())?,
        tensor_summaries: count(live_tensor_sites)?,
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

fn numerical_refinement_for_output(
    ranked: &ProductionRankedKernelLoweringInputV1,
    output: &SemanticOutputContractV1,
) -> Option<(super::ProductionNumericalRefinementContractV2, DigestV1)> {
    let mut matches = ranked
        .kernel()
        .blocks()
        .iter()
        .flat_map(|block| block.operations())
        .filter_map(|operation| match operation {
            ProductionRankedOperationV1::RequireNumericalRefinement { contract, proof }
                if super::production_ranked_value_identity_v1(contract.actual())
                    == output.actual()
                    && super::production_ranked_value_identity_v1(contract.reference())
                        == output.reference() =>
            {
                Some((*contract, proof.receipt_identity().digest()))
            }
            _ => None,
        });
    let relation = matches.next()?;
    matches.next().is_none().then_some(relation)
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
        || collective.view_identity() != output.view_identity()
        || collective.target_domain() != output.output_domain()
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
) -> Result<(), ProductionParallelReferenceContractErrorV1> {
    match declared {
        ParallelFoldOrderV1::Preserved if collective.order() == reference_order => Ok(()),
        ParallelFoldOrderV1::Preserved => Err(
            ProductionParallelReferenceContractErrorV1::FoldOrderRejected {
                index,
                detail: "live fold order differs from the sequential reference order",
            },
        ),
        ParallelFoldOrderV1::AlgebraicallyReordered { .. } => Err(
            ProductionParallelReferenceContractErrorV1::FoldOrderRejected {
                index,
                detail: "algebraic reordering requires a claim-specific associativity and commutativity receipt; effect-refinement receipts cannot prove algebraic laws",
            },
        ),
        ParallelFoldOrderV1::ErrorBoundedReordering { .. } => Err(
            ProductionParallelReferenceContractErrorV1::FoldOrderRejected {
                index,
                detail: "error-bounded reordering requires a claim-specific finite-error receipt; effect-refinement receipts cannot prove error bounds",
            },
        ),
    }
}

fn require_numerical_policy(
    index: usize,
    policy: ParallelNumericalPolicyV1,
    output: &SemanticOutputContractV1,
    contract: &MirPlironSemanticContractV1,
    ranked: Option<&ProductionRankedKernelLoweringInputV1>,
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
            absolute_error_f64_bits,
            relative_error_f64_bits,
            witness_root,
            proof,
        } => match ranked.and_then(|ranked| {
            numerical_refinement_for_output(ranked, output).map(|relation| (ranked, relation))
        }) {
            Some((ranked, (numerical, retained_proof)))
                if matches!(actual.scalar(), SemanticScalarTypeV1::Float(_))
                    && numerical.absolute_error_f64_bits() == absolute_error_f64_bits
                    && numerical.relative_error_f64_bits() == relative_error_f64_bits
                    && numerical.request_shape_hash() == witness_root
                    && retained_proof == proof
                    && ranked
                        .semantic_report()
                        .all_numerical_obligations_are_proved() =>
            {
                Ok(())
            }
            _ => {
                Err(ProductionParallelReferenceContractErrorV1::NumericalProofIncomplete { index })
            }
        },
        ParallelNumericalPolicyV1::UnboundedRelaxed => Err(
            ProductionParallelReferenceContractErrorV1::NumericalPolicyRejected {
                index,
                detail: "unbounded relaxed floating-point semantics cannot establish functional correctness",
            },
        ),
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn derive_tensor_call_summaries_v1(
    ranked: &ProductionRankedKernelLoweringInputV1,
    evidence: &ProductionMiddleEndEvidenceV5,
    semantic_contract: &MirPlironSemanticContractV1,
    output: &SemanticOutputContractV1,
    output_index: usize,
    schedule: ParallelScheduleRelationV1,
    numerical_policy: ParallelNumericalPolicyV1,
    proof: DigestV1,
) -> Result<Vec<ParallelCallSummaryV1>, ProductionParallelReferenceContractErrorV1> {
    let sites = ranked
        .kernel()
        .blocks()
        .iter()
        .enumerate()
        .flat_map(|(block, body)| {
            body.operations()
                .iter()
                .enumerate()
                .filter_map(move |(operation, item)| match item {
                    ProductionRankedOperationV1::TensorLayout {
                        contract,
                        convergence,
                        active_lanes,
                        binding,
                    } => Some((
                        block,
                        operation,
                        contract,
                        *convergence,
                        *active_lanes,
                        *binding,
                    )),
                    _ => None,
                })
        })
        .collect::<Vec<_>>();
    if sites.is_empty() {
        return Ok(vec![]);
    }
    if semantic_contract.outputs().len() != 1 {
        return Err(
            ProductionParallelReferenceContractErrorV1::TensorOutputAssociationIncomplete {
                outputs: semantic_contract.outputs().len(),
            },
        );
    }
    if !ranked.tensor_layout_report().is_clean() {
        return Err(
            ProductionParallelReferenceContractErrorV1::TensorFragmentOwnershipIncomplete {
                index: 0,
                detail: "the mandatory tensor-layout verifier did not establish exact fragment ownership and convergence",
            },
        );
    }
    let mut summaries = Vec::with_capacity(sites.len());
    for (ordinal, (block, operation, contract, convergence, active_lanes, binding)) in
        sites.into_iter().enumerate()
    {
        let binding = binding.ok_or(
            ProductionParallelReferenceContractErrorV1::TensorFragmentOwnershipIncomplete {
                index: ordinal,
                detail: "the live tensor site has no compiler-derived typed capability binding",
            },
        )?;
        let descriptor = contract.profile.semantic_descriptor().ok_or(
            ProductionParallelReferenceContractErrorV1::CallSummaryDerivationIncomplete {
                index: ordinal,
                kind: "cooperative tensor target profile",
            },
        )?;
        if convergence != dialect_kernel::TensorConvergenceAttr::UniformSubgroup
            || active_lanes != u32::from(contract.subgroup_width)
            || contract.subgroup_width != descriptor.subgroup_width
            || binding.argument_count() != descriptor.call_argument_count
            || descriptor.contribution_shape
                != [
                    contract.a.shape[0],
                    contract.a.shape[1],
                    contract.b.shape[1],
                ]
            || descriptor.output_shape != contract.accumulator.shape
            || !matches!(
                contract.tail_mask,
                fe2o3_kernel_ir::TensorTailMaskV1::ExactPhysicalTile
                    | fe2o3_kernel_ir::TensorTailMaskV1::ZeroFilledPredicateInputs
            )
            || binding.accumulator_root() == binding.result_root()
            || binding.context_root() == binding.lane_root()
        {
            return Err(
                ProductionParallelReferenceContractErrorV1::TensorFragmentOwnershipIncomplete {
                    index: ordinal,
                    detail: "the live tensor call has incompatible scope, convergence, arity, or capability-root provenance",
                },
            );
        }
        let layout_identity = tensor_layout_identity_v1(contract, ordinal)?;
        let staging_identity =
            tensor_staging_identity_v1(ranked, contract, binding, block, operation, ordinal)?;
        let contribution_identity =
            tensor_contribution_identity_v1(semantic_contract, output, schedule, descriptor);
        let scatter_identity = tensor_scatter_identity_v1(ranked, output, proof).ok_or(
            ProductionParallelReferenceContractErrorV1::TensorFragmentOwnershipIncomplete {
                index: ordinal,
                detail: "the tensor result has no unique total-view output scatter contract",
            },
        )?;
        let mut callsite = Sha256::new();
        hash_domain(&mut callsite, TENSOR_CALLSITE_DOMAIN_V1);
        callsite.update(semantic_contract.canonical_sha256().as_bytes());
        callsite.update(evidence.identity().sha256());
        callsite.update((block as u64).to_le_bytes());
        callsite.update((operation as u64).to_le_bytes());
        callsite.update((ordinal as u64).to_le_bytes());
        for identity in [
            binding.context_root(),
            binding.lane_root(),
            binding.lhs_root(),
            binding.rhs_root(),
            binding.accumulator_root(),
            binding.result_root(),
            layout_identity,
            staging_identity,
            contribution_identity,
            scatter_identity,
            proof,
        ] {
            callsite.update(identity.as_bytes());
        }
        callsite.update(binding.argument_count().to_le_bytes());
        let callsite_identity = DigestV1::from_untrusted_bytes(callsite.finalize().into());
        let mut summary = Sha256::new();
        hash_domain(&mut summary, TENSOR_SUMMARY_DOMAIN_V1);
        summary.update(callsite_identity.as_bytes());
        summary.update(output.actual().as_bytes());
        summary.update(output.reference().as_bytes());
        summary.update(proof.as_bytes());
        summary.update((output_index as u64).to_le_bytes());
        let identity = DigestV1::from_untrusted_bytes(summary.finalize().into());
        summaries.push(
            ParallelCallSummaryV1::new(
                identity,
                callsite_identity,
                output.actual(),
                output.reference(),
                proof,
                binding.argument_count(),
                ParallelExecutionScopeV1::Subgroup,
                ParallelCallKindV1::CooperativeTensorIntrinsic {
                    site_ordinal: u32::try_from(ordinal)
                        .map_err(|_| ProductionParallelReferenceContractErrorV1::CounterOverflow)?,
                    layout_identity,
                },
                numerical_policy,
            )
            .map_err(ProductionParallelReferenceContractErrorV1::ContractConstruction)?,
        );
    }
    Ok(summaries)
}

fn tensor_layout_identity_v1(
    contract: &fe2o3_kernel_ir::TensorLayoutContractV1,
    index: usize,
) -> Result<DigestV1, ProductionParallelReferenceContractErrorV1> {
    let mut digest = Sha256::new();
    hash_domain(&mut digest, TENSOR_LAYOUT_DOMAIN_V1);
    super::middle_end_evidence_v4::hash_tensor_layout_contract(&mut digest, contract);
    for fragment in [contract.a, contract.b, contract.accumulator] {
        for lane in 0..contract.subgroup_width {
            for component in 0..fragment.fragment_elements {
                let coordinate = fragment.logical_coordinate(lane, component).ok_or(
                    ProductionParallelReferenceContractErrorV1::TensorFragmentOwnershipIncomplete {
                        index,
                        detail: "a lane/component fragment mapping did not produce a logical coordinate",
                    },
                )?;
                digest.update(coordinate[0].to_le_bytes());
                digest.update(coordinate[1].to_le_bytes());
            }
        }
    }
    Ok(DigestV1::from_untrusted_bytes(digest.finalize().into()))
}

fn tensor_staging_identity_v1(
    ranked: &ProductionRankedKernelLoweringInputV1,
    contract: &fe2o3_kernel_ir::TensorLayoutContractV1,
    binding: super::ProductionCooperativeTensorBindingV1,
    tensor_block: usize,
    tensor_operation: usize,
    index: usize,
) -> Result<DigestV1, ProductionParallelReferenceContractErrorV1> {
    let uses_workgroup_staging = [contract.a.lds_swizzle, contract.b.lds_swizzle]
        .into_iter()
        .any(|swizzle| swizzle != fe2o3_kernel_ir::TensorLdsSwizzleV1::None);
    let mut digest = Sha256::new();
    if !uses_workgroup_staging {
        hash_domain(&mut digest, TENSOR_DIRECT_DOMAIN_V1);
    } else {
        hash_domain(&mut digest, TENSOR_BARRIER_DOMAIN_V1);
        let barriers =
            dominating_tensor_barriers_v1(ranked.kernel(), tensor_block, tensor_operation);
        if barriers.is_empty() {
            return Err(
                ProductionParallelReferenceContractErrorV1::TensorFragmentOwnershipIncomplete {
                    index,
                    detail: "workgroup-staged tensor operands have no dominating release/acquire workgroup barrier",
                },
            );
        }
        digest.update((barriers.len() as u64).to_le_bytes());
        for (barrier_block, barrier_operation, memory_scope, order) in barriers {
            digest.update((barrier_block as u64).to_le_bytes());
            digest.update((barrier_operation as u64).to_le_bytes());
            digest.update([match memory_scope {
                dialect_gpu::MemoryScopeAttr::Workgroup => 1,
                dialect_gpu::MemoryScopeAttr::Device => 2,
                dialect_gpu::MemoryScopeAttr::System => 3,
                dialect_gpu::MemoryScopeAttr::Subgroup => 4,
            }]);
            digest.update([match order {
                dialect_gpu::MemoryOrderAttr::AcquireRelease => 1,
                dialect_gpu::MemoryOrderAttr::SequentiallyConsistent => 2,
                dialect_gpu::MemoryOrderAttr::Acquire => 3,
                dialect_gpu::MemoryOrderAttr::Release => 4,
            }]);
        }
    }
    digest.update(binding.lhs_root().as_bytes());
    digest.update(binding.rhs_root().as_bytes());
    digest.update([match contract.a.lds_swizzle {
        fe2o3_kernel_ir::TensorLdsSwizzleV1::None => 1,
        fe2o3_kernel_ir::TensorLdsSwizzleV1::Xor4 => 2,
        fe2o3_kernel_ir::TensorLdsSwizzleV1::Unsupported(code) => 3_u8.saturating_add(code),
    }]);
    digest.update([match contract.b.lds_swizzle {
        fe2o3_kernel_ir::TensorLdsSwizzleV1::None => 1,
        fe2o3_kernel_ir::TensorLdsSwizzleV1::Xor4 => 2,
        fe2o3_kernel_ir::TensorLdsSwizzleV1::Unsupported(code) => 3_u8.saturating_add(code),
    }]);
    Ok(DigestV1::from_untrusted_bytes(digest.finalize().into()))
}

fn dominating_tensor_barriers_v1(
    kernel: &super::ProductionRankedKernelV1,
    tensor_block: usize,
    tensor_operation: usize,
) -> Vec<(
    usize,
    usize,
    dialect_gpu::MemoryScopeAttr,
    dialect_gpu::MemoryOrderAttr,
)> {
    kernel
        .blocks()
        .iter()
        .enumerate()
        .flat_map(|(block, body)| {
            body.operations()
                .iter()
                .enumerate()
                .filter_map(move |(operation, item)| match item {
                    ProductionRankedOperationV1::Barrier {
                        execution_scope: dialect_gpu::HierarchyAttr::Workgroup,
                        memory_scope:
                            memory_scope @ (dialect_gpu::MemoryScopeAttr::Workgroup
                            | dialect_gpu::MemoryScopeAttr::Device
                            | dialect_gpu::MemoryScopeAttr::System),
                        address_space: dialect_gpu::AddressSpaceAttr::Workgroup,
                        order:
                            order @ (dialect_gpu::MemoryOrderAttr::AcquireRelease
                            | dialect_gpu::MemoryOrderAttr::SequentiallyConsistent),
                    } if (block != tensor_block
                        && block_dominates_v1(kernel, block, tensor_block))
                        || (block == tensor_block && operation < tensor_operation) =>
                    {
                        Some((block, operation, *memory_scope, *order))
                    }
                    _ => None,
                })
        })
        .collect()
}

fn tensor_contribution_identity_v1(
    semantic_contract: &MirPlironSemanticContractV1,
    output: &SemanticOutputContractV1,
    schedule: ParallelScheduleRelationV1,
    descriptor: fe2o3_kernel_ir::TensorInstructionSemanticDescriptorV1,
) -> DigestV1 {
    let mut digest = Sha256::new();
    hash_domain(&mut digest, TENSOR_CONTRIBUTION_DOMAIN_V1);
    digest.update(semantic_contract.canonical_sha256().as_bytes());
    digest.update(output.output_domain().as_bytes());
    for extent in descriptor.contribution_shape {
        digest.update(extent.to_le_bytes());
    }
    for extent in descriptor.output_shape {
        digest.update(extent.to_le_bytes());
    }
    hash_schedule_v1(&mut digest, schedule);
    DigestV1::from_untrusted_bytes(digest.finalize().into())
}

fn tensor_scatter_identity_v1(
    ranked: &ProductionRankedKernelLoweringInputV1,
    output: &SemanticOutputContractV1,
    proof: DigestV1,
) -> Option<DigestV1> {
    let mut ownership = ranked
        .kernel()
        .blocks()
        .iter()
        .flat_map(|block| block.operations())
        .filter_map(|operation| match operation {
            ProductionRankedOperationV1::OwnershipContract {
                view,
                coverage: OwnershipCoverageAttr::TotalView,
                partition,
            } if super::production_ranked_value_identity_v1(*view) == output.view_identity() => {
                Some(*partition)
            }
            _ => None,
        });
    let partition = ownership.next()?;
    if ownership.next().is_some() {
        return None;
    }
    let mut digest = Sha256::new();
    hash_domain(&mut digest, TENSOR_SCATTER_DOMAIN_V1);
    for identity in [
        output.identity(),
        output.view_identity(),
        output.output_domain(),
        output.actual(),
        output.reference(),
        proof,
    ] {
        digest.update(identity.as_bytes());
    }
    digest.update([match partition {
        dialect_kernel::OwnershipPartitionAttr::ExactSets => 1,
        dialect_kernel::OwnershipPartitionAttr::DenseRectangles => 2,
    }]);
    Some(DigestV1::from_untrusted_bytes(digest.finalize().into()))
}

fn hash_schedule_v1(digest: &mut Sha256, schedule: ParallelScheduleRelationV1) {
    match schedule {
        ParallelScheduleRelationV1::PointwiseBijection => digest.update([1]),
        ParallelScheduleRelationV1::Permutation { collective } => {
            digest.update([2]);
            digest.update(collective.as_bytes());
        }
        ParallelScheduleRelationV1::Fold {
            collective,
            order,
            reference_order,
        } => {
            digest.update([3]);
            digest.update(collective.as_bytes());
            match order {
                ParallelFoldOrderV1::Preserved => digest.update([1]),
                ParallelFoldOrderV1::AlgebraicallyReordered {
                    associativity_proof,
                    commutativity_proof,
                } => {
                    digest.update([2]);
                    digest.update(associativity_proof.as_bytes());
                    digest.update(commutativity_proof.as_bytes());
                }
                ParallelFoldOrderV1::ErrorBoundedReordering { proof } => {
                    digest.update([3]);
                    digest.update(proof.as_bytes());
                }
            }
            digest.update([evaluation_order_tag_v1(reference_order)]);
        }
        ParallelScheduleRelationV1::BoundedRecurrence {
            collective,
            loop_contract,
            dynamic_bound_proof,
            reference_order,
        } => {
            digest.update([4]);
            digest.update(collective.as_bytes());
            digest.update(loop_contract.as_bytes());
            match dynamic_bound_proof {
                Some(proof) => {
                    digest.update([1]);
                    digest.update(proof.as_bytes());
                }
                None => digest.update([0]),
            }
            digest.update([evaluation_order_tag_v1(reference_order)]);
        }
    }
}

fn evaluation_order_tag_v1(order: SemanticEvaluationOrderV1) -> u8 {
    match order {
        SemanticEvaluationOrderV1::SequentialAscending => 1,
        SemanticEvaluationOrderV1::SequentialDescending => 2,
        SemanticEvaluationOrderV1::Lexicographic => 3,
        SemanticEvaluationOrderV1::ExplicitTree => 4,
    }
}

fn hash_domain(digest: &mut Sha256, domain: &[u8]) {
    digest.update((domain.len() as u64).to_le_bytes());
    digest.update(domain);
}

fn block_dominates_v1(
    kernel: &super::ProductionRankedKernelV1,
    candidate: usize,
    target: usize,
) -> bool {
    if candidate == target {
        return true;
    }
    if candidate == 0 {
        return true;
    }
    let mut visited = vec![false; kernel.blocks().len()];
    let mut work = vec![0_usize];
    while let Some(block) = work.pop() {
        if block == candidate || visited.get(block).copied().unwrap_or(true) {
            continue;
        }
        visited[block] = true;
        if block == target {
            return false;
        }
        for successor in ranked_successors_v1(kernel.blocks()[block].terminator()) {
            work.push(successor);
        }
    }
    true
}

fn ranked_successors_v1(terminator: &super::ProductionRankedTerminatorV1) -> Vec<usize> {
    use super::ProductionRankedTerminatorV1 as T;
    match terminator {
        T::IndexLessThan {
            true_block,
            false_block,
            ..
        }
        | T::IndexLessThanArgs {
            true_block,
            false_block,
            ..
        }
        | T::IndexEqual {
            true_block,
            false_block,
            ..
        }
        | T::IndexEqualArgs {
            true_block,
            false_block,
            ..
        } => {
            vec![*true_block as usize, *false_block as usize]
        }
        T::AnalysisSplit {
            first_block,
            second_block,
            ..
        }
        | T::AnalysisSplitArgs {
            first_block,
            second_block,
            ..
        } => {
            vec![*first_block as usize, *second_block as usize]
        }
        T::Branch { target }
        | T::BranchArgs { target, .. }
        | T::BranchArgsAdd { target, .. }
        | T::BranchArgsAddAt { target, .. } => vec![*target as usize],
        T::Return | T::Trap => vec![],
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

fn reconcile_tensor_summaries_v1(
    declared: &[ParallelCallSummaryV1],
    derived: &[ParallelCallSummaryV1],
    live_tensor_sites: usize,
) -> Result<(), ProductionParallelReferenceContractErrorV1> {
    if declared.len() != live_tensor_sites {
        return Err(
            ProductionParallelReferenceContractErrorV1::UnmodeledTensorSites {
                declared: declared.len(),
                live: live_tensor_sites,
            },
        );
    }
    if declared != derived {
        let index = declared
            .iter()
            .zip(derived)
            .position(|(declared, derived)| declared != derived)
            .unwrap_or(declared.len().min(derived.len()));
        return Err(ProductionParallelReferenceContractErrorV1::CallSummaryMismatch { index });
    }
    Ok(())
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
            ProductionParallelReferenceContractErrorV1::StableRankedViewIdentityMissing {
                index: 1,
            },
            ProductionParallelReferenceContractErrorV1::OutputSeparationIncomplete {
                first: 0,
                second: 1,
            },
            ProductionParallelReferenceContractErrorV1::OutputOwnershipMismatch { index: 1 },
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
                detail: "missing fragment ownership",
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
            ProductionParallelReferenceContractErrorV1::DuplicateOutputView {
                first: 0,
                second: 1,
            },
            ProductionParallelReferenceContractErrorV1::OutputProductMismatch,
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

    #[test]
    fn effect_receipt_digests_cannot_authorize_reassociation_or_error_bounds() {
        let collective = SemanticCollectiveContractV1::new(
            digest(1),
            SemanticCollectiveKindV1::FiniteFold,
            digest(2),
            digest(3),
            digest(3),
            digest(4),
            digest(5),
            digest(6),
            digest(7),
            16,
            16,
            SemanticEvaluationOrderV1::SequentialAscending,
            fe2o3_functional_proof::SemanticCoverageBindingV1::TotalView,
        )
        .unwrap();
        let error = require_fold_order(
            0,
            ParallelFoldOrderV1::AlgebraicallyReordered {
                associativity_proof: digest(9),
                commutativity_proof: digest(9),
            },
            SemanticEvaluationOrderV1::SequentialAscending,
            &collective,
        )
        .unwrap_err();
        assert!(error.to_string().contains("claim-specific"));

        let domain = digest(10);
        let ieee = SemanticNumericalPolicyV1::IeeeOperatorCongruence {
            rounding: fe2o3_functional_proof::SemanticIeeeRoundingV1::NearestTiesEven,
            exceptional_values: fe2o3_functional_proof::SemanticIeeeExceptionalValueV1::ExactBits,
        };
        let roots = [11_u8, 12, 13]
            .map(|identity| {
                SemanticTypedRootV1::new(
                    digest(identity),
                    digest(if identity == 12 { 14 } else { identity }),
                    domain,
                    SemanticScalarTypeV1::Float(32),
                    ieee,
                )
                .unwrap()
            })
            .to_vec();
        let output = SemanticOutputContractV1::new(
            digest(15),
            digest(16),
            domain,
            digest(11),
            digest(12),
            vec![digest(13)],
        )
        .unwrap();
        let contract = MirPlironSemanticContractV1::new(
            digest(17),
            digest(18),
            digest(19),
            vec![
                fe2o3_functional_proof::SemanticFiniteDomainV1::new(
                    domain,
                    vec![SemanticFiniteExtentV1::Static(16)],
                )
                .unwrap(),
            ],
            roots,
            vec![],
            vec![],
            vec![output],
        )
        .unwrap();
        let error = require_numerical_policy(
            0,
            ParallelNumericalPolicyV1::ErrorBounded {
                absolute_error_f64_bits: 1.0_f64.to_bits(),
                relative_error_f64_bits: 1.0_f64.to_bits(),
                witness_root: digest(13),
                proof: digest(9),
            },
            &contract.outputs()[0],
            &contract,
            None,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ProductionParallelReferenceContractErrorV1::NumericalProofIncomplete { index: 0 }
        ));
    }

    #[test]
    fn tensor_layout_identity_binds_layout_swizzle_tail_and_lane_coordinates() {
        let canonical =
            fe2o3_kernel_ir::TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64();
        let base = tensor_layout_identity_v1(&canonical, 0).unwrap();

        let mut lds = canonical.with_a_lds_xor4();
        assert_ne!(base, tensor_layout_identity_v1(&lds, 0).unwrap());
        lds.tail_mask = fe2o3_kernel_ir::TensorTailMaskV1::ZeroFilledPredicateInputs;
        assert_ne!(base, tensor_layout_identity_v1(&lds, 0).unwrap());

        let mut substituted = canonical;
        let fe2o3_kernel_ir::TensorSymbolicMapV1::LaneComponentAffine { ref mut axes, .. } =
            substituted.accumulator.mapping
        else {
            panic!("canonical layout must have an affine accumulator map");
        };
        axes[0].constant = 1;
        assert_ne!(base, tensor_layout_identity_v1(&substituted, 0).unwrap());
    }

    #[test]
    fn tensor_capability_binding_rejects_missing_roots_and_argument_overflow() {
        use super::super::ProductionCooperativeTensorBindingV1;

        assert!(
            ProductionCooperativeTensorBindingV1::new(
                DigestV1::ZERO,
                digest(2),
                digest(3),
                digest(4),
                digest(5),
                digest(6),
                4,
            )
            .is_none()
        );
        assert!(
            ProductionCooperativeTensorBindingV1::new(
                digest(1),
                digest(2),
                digest(3),
                digest(4),
                digest(5),
                digest(6),
                257,
            )
            .is_none()
        );
        let binding = ProductionCooperativeTensorBindingV1::new(
            digest(1),
            digest(2),
            digest(3),
            digest(4),
            digest(5),
            digest(6),
            4,
        )
        .unwrap();
        assert_eq!(binding.argument_count(), 4);
        assert_eq!(binding.result_root(), digest(6));
    }

    #[test]
    fn tensor_contribution_identity_rejects_schedule_substitution() {
        fn identity(schedule: ParallelScheduleRelationV1) -> DigestV1 {
            let mut digest = Sha256::new();
            hash_domain(&mut digest, TENSOR_CONTRIBUTION_DOMAIN_V1);
            hash_schedule_v1(&mut digest, schedule);
            DigestV1::from_untrusted_bytes(digest.finalize().into())
        }

        assert_ne!(
            identity(ParallelScheduleRelationV1::PointwiseBijection),
            identity(ParallelScheduleRelationV1::Permutation {
                collective: digest(9),
            })
        );
        assert_ne!(
            identity(ParallelScheduleRelationV1::Fold {
                collective: digest(9),
                order: ParallelFoldOrderV1::Preserved,
                reference_order: SemanticEvaluationOrderV1::SequentialAscending,
            }),
            identity(ParallelScheduleRelationV1::Fold {
                collective: digest(9),
                order: ParallelFoldOrderV1::Preserved,
                reference_order: SemanticEvaluationOrderV1::SequentialDescending,
            })
        );
        assert_ne!(
            identity(ParallelScheduleRelationV1::Fold {
                collective: digest(9),
                order: ParallelFoldOrderV1::AlgebraicallyReordered {
                    associativity_proof: digest(10),
                    commutativity_proof: digest(11),
                },
                reference_order: SemanticEvaluationOrderV1::SequentialAscending,
            }),
            identity(ParallelScheduleRelationV1::Fold {
                collective: digest(9),
                order: ParallelFoldOrderV1::AlgebraicallyReordered {
                    associativity_proof: digest(12),
                    commutativity_proof: digest(11),
                },
                reference_order: SemanticEvaluationOrderV1::SequentialAscending,
            })
        );
    }

    #[test]
    fn tensor_summary_reconciliation_rejects_count_and_identity_substitution() {
        let summary = |tag| {
            ParallelCallSummaryV1::new(
                digest(tag),
                digest(20),
                digest(21),
                digest(22),
                digest(23),
                4,
                ParallelExecutionScopeV1::Subgroup,
                ParallelCallKindV1::CooperativeTensorIntrinsic {
                    site_ordinal: 0,
                    layout_identity: digest(24),
                },
                ParallelNumericalPolicyV1::ExactBitVector,
            )
            .unwrap()
        };
        let derived = vec![summary(1)];
        assert!(matches!(
            reconcile_tensor_summaries_v1(&[], &derived, 1),
            Err(
                ProductionParallelReferenceContractErrorV1::UnmodeledTensorSites {
                    declared: 0,
                    live: 1,
                }
            )
        ));
        assert!(matches!(
            reconcile_tensor_summaries_v1(&[summary(2)], &derived, 1),
            Err(ProductionParallelReferenceContractErrorV1::CallSummaryMismatch { index: 0 })
        ));
        reconcile_tensor_summaries_v1(&derived, &derived, 1).unwrap();
    }

    #[test]
    fn tensor_failures_have_stable_production_diagnostics() {
        assert!(
            ProductionParallelReferenceContractErrorV1::TensorFragmentOwnershipIncomplete {
                index: 3,
                detail: "subgroup convergence was not proved",
            }
            .to_string()
            .starts_with("error[FE2O3-PARALLEL-013]")
        );
        assert!(
            ProductionParallelReferenceContractErrorV1::UnmodeledTensorSites {
                declared: 1,
                live: 2,
            }
            .to_string()
            .contains("models 1 cooperative tensor sites")
        );
        assert!(
            ProductionParallelReferenceContractErrorV1::CallSummaryMismatch { index: 4 }
                .to_string()
                .contains("summary 4")
        );
    }

    #[test]
    fn tensor_staging_requires_a_dominating_workgroup_publish_barrier() {
        use super::super::{
            ProductionRankedBlockV1, ProductionRankedKernelV1, ProductionRankedTerminatorV1,
        };

        let tensor = ProductionRankedOperationV1::TensorLayout {
            contract:
                fe2o3_kernel_ir::TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64_lds_xor4(),
            convergence: dialect_kernel::TensorConvergenceAttr::UniformSubgroup,
            active_lanes: 64,
            binding: None,
        };
        let barrier = |execution_scope| ProductionRankedOperationV1::Barrier {
            execution_scope,
            memory_scope: dialect_gpu::MemoryScopeAttr::Workgroup,
            address_space: dialect_gpu::AddressSpaceAttr::Workgroup,
            order: dialect_gpu::MemoryOrderAttr::AcquireRelease,
        };
        let kernel = ProductionRankedKernelV1::new(
            "tensor_barrier",
            0,
            vec![
                ProductionRankedBlockV1::new(
                    vec![barrier(dialect_gpu::HierarchyAttr::Workgroup)],
                    ProductionRankedTerminatorV1::Branch { target: 1 },
                ),
                ProductionRankedBlockV1::new(
                    vec![tensor.clone()],
                    ProductionRankedTerminatorV1::Return,
                ),
            ],
        )
        .unwrap();
        assert_eq!(
            dominating_tensor_barriers_v1(&kernel, 1, 0),
            vec![(
                0,
                0,
                dialect_gpu::MemoryScopeAttr::Workgroup,
                dialect_gpu::MemoryOrderAttr::AcquireRelease,
            )]
        );

        let wrong_scope = ProductionRankedKernelV1::new(
            "tensor_wrong_scope",
            0,
            vec![
                ProductionRankedBlockV1::new(
                    vec![barrier(dialect_gpu::HierarchyAttr::Subgroup)],
                    ProductionRankedTerminatorV1::Branch { target: 1 },
                ),
                ProductionRankedBlockV1::new(
                    vec![tensor.clone()],
                    ProductionRankedTerminatorV1::Return,
                ),
            ],
        )
        .unwrap();
        assert!(dominating_tensor_barriers_v1(&wrong_scope, 1, 0).is_empty());

        let too_late = ProductionRankedKernelV1::new(
            "tensor_late_barrier",
            0,
            vec![ProductionRankedBlockV1::new(
                vec![tensor, barrier(dialect_gpu::HierarchyAttr::Workgroup)],
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap();
        assert!(dominating_tensor_barriers_v1(&too_late, 0, 0).is_empty());
    }
}
