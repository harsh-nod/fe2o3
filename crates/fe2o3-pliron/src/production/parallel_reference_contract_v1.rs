//! Production join from compiler-derived sequential/parallel facts to one
//! workload-neutral parallel-reference contract.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use dialect_kernel::OwnershipCoverageAttr;
use fe2o3_functional_proof::{
    COMPLETE_GPU_HIERARCHY_V1, MirPlironSemanticContractV1, ParallelFoldOrderV1,
    ParallelHierarchyLevelV1, ParallelNumericalPolicyV1, ParallelOutputRelationV1,
    ParallelReferenceContractErrorV1, ParallelReferenceContractV1, ParallelScheduleRelationV1,
    SemanticCollectiveContractV1, SemanticCollectiveKindV1, SemanticEvaluationOrderV1,
    SemanticFiniteExtentV1, SemanticNumericalPolicyV1, SemanticOutputContractV1,
    SemanticScalarTypeV1, SemanticTypedRootV1,
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
    OutputCoverageIncomplete {
        declared: usize,
        live: usize,
    },
    OutputRelationMismatch {
        index: usize,
    },
    StableRankedViewIdentityMissing {
        index: usize,
    },
    DuplicateOutputView {
        first: usize,
        second: usize,
    },
    OutputSeparationIncomplete {
        first: usize,
        second: usize,
    },
    OutputOwnershipMismatch {
        index: usize,
    },
    OutputProductMismatch,
    HierarchyCoverageIncomplete {
        level: ParallelHierarchyLevelV1,
    },
    PolicyCheckedStagingIncomplete {
        identity: DigestV1,
    },
    ScheduleRelationIncomplete {
        index: usize,
        detail: &'static str,
    },
    FoldOrderRejected {
        index: usize,
        detail: &'static str,
    },
    DynamicBoundProofIncomplete {
        index: usize,
    },
    NumericalPolicyRejected {
        index: usize,
        detail: &'static str,
    },
    NumericalProofIncomplete {
        index: usize,
    },
    NumericalSiteUnmatched {
        site: usize,
    },
    NumericalSiteAmbiguous {
        site: usize,
        outputs: usize,
    },
    DuplicateNumericalSite {
        index: usize,
    },
    NumericalCoverageIncomplete {
        index: usize,
        component: &'static str,
    },
    TensorFunctionalRefinementIncomplete {
        live_sites: usize,
        policy_checked_sites: usize,
    },
    TensorOutputUnmatched {
        site: usize,
    },
    TensorOutputAmbiguous {
        site: usize,
        outputs: usize,
    },
    DuplicateTensorOutput {
        index: usize,
    },
    TensorInstructionSiteUnmatched {
        block: u32,
        operation: u32,
    },
    DuplicateTensorInstructionSite {
        block: u32,
        operation: u32,
    },
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
                | Self::PolicyCheckedStagingIncomplete { .. }
                | Self::ScheduleRelationIncomplete { .. }
                | Self::DynamicBoundProofIncomplete { .. }
                | Self::NumericalProofIncomplete { .. }
                | Self::TensorFunctionalRefinementIncomplete { .. }
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
            Self::PolicyCheckedStagingIncomplete { identity } => write!(formatter, "error[FE2O3-PARALLEL-005]: no retained policy-checked staging record has identity {identity:?}"),
            Self::ScheduleRelationIncomplete { index, detail } => write!(formatter, "error[FE2O3-PARALLEL-006]: schedule relation {index} is incomplete: {detail}"),
            Self::FoldOrderRejected { index, detail } => write!(formatter, "error[FE2O3-PARALLEL-007]: fold order for relation {index} is not justified: {detail}"),
            Self::DynamicBoundProofIncomplete { index } => write!(formatter, "error[FE2O3-PARALLEL-008]: dynamic bounded recurrence {index} does not match the compiler-derived finite-bound identity for the live canonical loop"),
            Self::NumericalPolicyRejected { index, detail } => write!(formatter, "error[FE2O3-PARALLEL-009]: numerical policy for relation {index} is invalid: {detail}"),
            Self::NumericalProofIncomplete { index } => write!(formatter, "error[FE2O3-PARALLEL-010]: finite error policy for relation {index} lacks a live typed witness or policy-checked staging record"),
            Self::NumericalSiteUnmatched { site } => write!(formatter, "error[FE2O3-PARALLEL-023]: numerical refinement site {site} does not match any logical output actual/reference roots"),
            Self::NumericalSiteAmbiguous { site, outputs } => write!(formatter, "error[FE2O3-PARALLEL-024]: numerical refinement site {site} ambiguously matches {outputs} logical outputs"),
            Self::DuplicateNumericalSite { index } => write!(formatter, "error[FE2O3-PARALLEL-025]: logical output {index} has more than one numerical refinement site"),
            Self::NumericalCoverageIncomplete { index, component } => write!(formatter, "error[FE2O3-PARALLEL-026]: numerical refinement for logical output {index} is not total: {component} must be the canonical typed constant true"),
            Self::TensorFunctionalRefinementIncomplete { live_sites, policy_checked_sites } => write!(formatter, "error[FE2O3-PARALLEL-013]: cooperative tensor functional refinement is incomplete: {live_sites} live tensor site(s), but {policy_checked_sites} exact policy-checked result-component/output receipt binding(s)"),
            Self::TensorOutputUnmatched { site } => write!(formatter, "error[FE2O3-PARALLEL-027]: tensor refinement site {site} does not match any logical output view and actual/reference roots"),
            Self::TensorOutputAmbiguous { site, outputs } => write!(formatter, "error[FE2O3-PARALLEL-028]: tensor refinement site {site} ambiguously matches {outputs} logical outputs"),
            Self::DuplicateTensorOutput { index } => write!(formatter, "error[FE2O3-PARALLEL-029]: logical output {index} has more than one tensor refinement receipt"),
            Self::TensorInstructionSiteUnmatched { block, operation } => write!(formatter, "error[FE2O3-PARALLEL-030]: tensor refinement names non-live tensor instruction site ^bb{block}:{operation}"),
            Self::DuplicateTensorInstructionSite { block, operation } => write!(formatter, "error[FE2O3-PARALLEL-031]: tensor instruction site ^bb{block}:{operation} has more than one tensor refinement receipt"),
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
        require_parallel_boundary_subjects_v1(ranked, evidence, semantics, semantic_contract)?;
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
    require_parallel_boundary_subjects_v1(ranked, evidence, semantics, semantic_contract)?;
    let tensor = LiveTensorRefinementIndexV1::from_ranked(ranked, semantic_contract)?;
    let mut relations = Vec::with_capacity(semantic_contract.outputs().len());
    let bindings = derive_live_output_bindings_v1(ranked, evidence, semantic_contract)?;
    let numerical = LiveNumericalRefinementIndexV1::from_ranked(ranked, semantic_contract)?;
    let output_product_identity =
        output_product_identity_v1(semantic_contract, evidence, &bindings);
    for (index, output) in semantic_contract.outputs().iter().enumerate() {
        let binding = &bindings[index];
        let proof = binding.policy_checked_staging_identity;
        let (actual, reference) = output_roots(output, semantic_contract)
            .ok_or(ProductionParallelReferenceContractErrorV1::OutputRelationMismatch { index })?;
        let numerical_policy = match numerical.for_output(index) {
            Some(site) => ParallelNumericalPolicyV1::ErrorBounded {
                absolute_error_f64_bits: site.contract.absolute_error_f64_bits(),
                relative_error_f64_bits: site.contract.relative_error_f64_bits(),
                witness_root: site.contract.request_shape_hash(),
                proof: site.proof,
            },
            None => match (
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
            },
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
                tensor.for_output(index).map(|site| site.proof),
                proof,
            )
            .map_err(ProductionParallelReferenceContractErrorV1::ContractConstruction)?,
        );
    }
    let contract = ParallelReferenceContractV1::new(
        semantic_contract.canonical_sha256(),
        output_product_identity,
        relations,
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

fn require_parallel_boundary_subjects_v1(
    ranked: &ProductionRankedKernelLoweringInputV1,
    evidence: &ProductionMiddleEndEvidenceV5,
    semantics: ProductionMirPlironSemanticContractReportV1,
    semantic_contract: &MirPlironSemanticContractV1,
) -> Result<(), ProductionParallelReferenceContractErrorV1> {
    if semantics.contract_identity() != semantic_contract.canonical_sha256()
        || DigestV1::from_untrusted_bytes(*evidence.identity().sha256())
            != semantic_contract.pliron_evidence()
        || evidence.ranked_kernel_identity()
            != &super::middle_end_evidence_v4::derive_ranked_kernel_identity(ranked)
        || !ranked.all_mandatory_reports_are_clean()
    {
        return Err(ProductionParallelReferenceContractErrorV1::SemanticContractMismatch);
    }
    Ok(())
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
    policy_checked_staging_identity: DigestV1,
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
        let [(view, policy_checked_staging_identity)] = effect_matches.as_slice() else {
            return Err(
                ProductionParallelReferenceContractErrorV1::OutputRelationMismatch { index },
            );
        };
        let (view, policy_checked_staging_identity) = (*view, *policy_checked_staging_identity);
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
            policy_checked_staging_identity,
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
            policy_checked_staging_identity,
            ownership_identity,
        );
        bindings.push(LiveOutputBindingV1 {
            ranked_view_identity,
            live_view_name,
            allocation_origin,
            noalias_class,
            policy_checked_staging_identity,
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
    policy_checked_staging_identity: DigestV1,
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
    digest.update(policy_checked_staging_identity.as_bytes());
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
    policy_checked_staging_identity: DigestV1,
    ownership_identity: DigestV1,
) -> DigestV1 {
    let mut digest = Sha256::new();
    digest_blob(&mut digest, OUTPUT_FRAME_DOMAIN_V1);
    digest.update(semantic_contract.canonical_sha256().as_bytes());
    digest.update(evidence.identity().sha256());
    digest.update(output.identity().as_bytes());
    digest.update(ranked_view_identity.as_bytes());
    digest.update(policy_checked_staging_identity.as_bytes());
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
        digest.update(binding.policy_checked_staging_identity.as_bytes());
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
    require_parallel_boundary_subjects_v1(ranked, evidence, semantics, semantic_contract)?;
    if expected.semantic_contract_identity() != semantics.contract_identity() {
        return Err(ProductionParallelReferenceContractErrorV1::SemanticContractMismatch);
    }
    let tensor = LiveTensorRefinementIndexV1::from_ranked(ranked, semantic_contract)?;

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
    let numerical = LiveNumericalRefinementIndexV1::from_ranked(ranked, semantic_contract)?;
    let derived_product = output_product_identity_v1(semantic_contract, evidence, &bindings);
    if expected.output_product_identity() != derived_product {
        return Err(ProductionParallelReferenceContractErrorV1::OutputProductMismatch);
    }

    let mut counts = RelationCountsV1::default();
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
        let output_proof = binding.policy_checked_staging_identity;
        if relation.policy_checked_staging_identity() != output_proof {
            return Err(
                ProductionParallelReferenceContractErrorV1::PolicyCheckedStagingIncomplete {
                    identity: relation.policy_checked_staging_identity(),
                },
            );
        }
        require_numerical_policy(
            index,
            relation.numerical_policy(),
            output,
            semantic_contract,
            numerical.for_output(index),
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
        if relation.tensor_refinement_identity() != tensor.for_output(index).map(|site| site.proof)
        {
            return Err(
                ProductionParallelReferenceContractErrorV1::OutputRelationMismatch { index },
            );
        }
    }
    if counts.error_bounded != numerical.site_count() {
        return Err(
            ProductionParallelReferenceContractErrorV1::NumericalPolicyRejected {
                index: counts.error_bounded,
                detail: "live numerical refinement sites and error-bounded output relations are not one-to-one",
            },
        );
    }

    Ok(ProductionParallelReferenceContractReportV1 {
        contract_identity: expected.canonical_sha256(),
        output_product_identity: derived_product,
        output_relations: count(expected.relations().len())?,
        output_frames: count(bindings.len())?,
        pointwise_relations: count(counts.pointwise)?,
        permutation_relations: count(counts.permutation)?,
        fold_relations: count(counts.fold)?,
        bounded_recurrences: count(counts.recurrence)?,
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

#[derive(Clone, Copy)]
struct LiveNumericalRefinementV1 {
    contract: super::ProductionNumericalRefinementContractV2,
    proof: DigestV1,
    staging_is_policy_checked: bool,
}

struct LiveNumericalRefinementIndexV1 {
    by_output: Vec<Option<LiveNumericalRefinementV1>>,
    site_count: usize,
}

impl LiveNumericalRefinementIndexV1 {
    fn from_ranked(
        ranked: &ProductionRankedKernelLoweringInputV1,
        semantic_contract: &MirPlironSemanticContractV1,
    ) -> Result<Self, ProductionParallelReferenceContractErrorV1> {
        let mut outputs_by_roots = BTreeMap::<(DigestV1, DigestV1), Vec<usize>>::new();
        for (index, output) in semantic_contract.outputs().iter().enumerate() {
            outputs_by_roots
                .entry((output.actual(), output.reference()))
                .or_default()
                .push(index);
        }

        let staging_is_policy_checked = ranked
            .semantic_report()
            .all_numerical_obligations_are_policy_checked();
        let mut by_output = vec![None; semantic_contract.outputs().len()];
        let mut site_count = 0_usize;
        for operation in ranked
            .kernel()
            .blocks()
            .iter()
            .flat_map(|block| block.operations())
        {
            let ProductionRankedOperationV1::RequireNumericalRefinement { contract, proof } =
                operation
            else {
                continue;
            };
            let site = site_count;
            site_count = site_count
                .checked_add(1)
                .ok_or(ProductionParallelReferenceContractErrorV1::CounterOverflow)?;
            let roots = (
                super::production_ranked_value_identity_v1(contract.actual()),
                super::production_ranked_value_identity_v1(contract.reference()),
            );
            let Some(outputs) = outputs_by_roots.get(&roots) else {
                return Err(
                    ProductionParallelReferenceContractErrorV1::NumericalSiteUnmatched { site },
                );
            };
            let [index] = outputs.as_slice() else {
                return Err(
                    ProductionParallelReferenceContractErrorV1::NumericalSiteAmbiguous {
                        site,
                        outputs: outputs.len(),
                    },
                );
            };
            if by_output[*index].is_some() {
                return Err(
                    ProductionParallelReferenceContractErrorV1::DuplicateNumericalSite {
                        index: *index,
                    },
                );
            }
            for (component, value) in [
                ("domain", contract.domain()),
                ("precondition", contract.precondition()),
            ] {
                if !is_canonical_typed_true_v1(ranked, value) {
                    return Err(
                        ProductionParallelReferenceContractErrorV1::NumericalCoverageIncomplete {
                            index: *index,
                            component,
                        },
                    );
                }
            }
            by_output[*index] = Some(LiveNumericalRefinementV1 {
                contract: *contract,
                proof: proof.receipt_identity().digest(),
                staging_is_policy_checked,
            });
        }
        Ok(Self {
            by_output,
            site_count,
        })
    }

    fn for_output(&self, index: usize) -> Option<LiveNumericalRefinementV1> {
        self.by_output.get(index).copied().flatten()
    }

    const fn site_count(&self) -> usize {
        self.site_count
    }
}

fn is_canonical_typed_true_v1(
    ranked: &ProductionRankedKernelLoweringInputV1,
    value: ProductionRankedValueV1,
) -> bool {
    let ProductionRankedValueV1::Local(value) = value else {
        return false;
    };
    let mut definitions = ranked
        .kernel()
        .blocks()
        .iter()
        .flat_map(|block| block.operations())
        .filter(|operation| match operation {
            ProductionRankedOperationV1::SemanticExpression {
                result,
                expression:
                    super::ProductionSemanticExpressionV2::Constant {
                        scalar: super::ProductionSemanticScalarTypeV2::Bool,
                        bits: 1,
                    },
                numerical_contract:
                    super::ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
            } => *result == value,
            _ => false,
        });
    definitions.next().is_some() && definitions.next().is_none()
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
    numerical: Option<LiveNumericalRefinementV1>,
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
    if numerical.is_some() && !matches!(policy, ParallelNumericalPolicyV1::ErrorBounded { .. }) {
        return Err(
            ProductionParallelReferenceContractErrorV1::NumericalPolicyRejected {
                index,
                detail: "a live numerical refinement site must be represented by the output error-bounded policy",
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
        } => match numerical {
            Some(site)
                if matches!(actual.scalar(), SemanticScalarTypeV1::Float(_))
                    && site.contract.absolute_error_f64_bits() == absolute_error_f64_bits
                    && site.contract.relative_error_f64_bits() == relative_error_f64_bits
                    && site.contract.request_shape_hash() == witness_root
                    && site.proof == proof
                    && site.staging_is_policy_checked =>
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

fn tensor_instruction_sites(
    kernel: &super::ProductionRankedKernelV1,
) -> BTreeSet<super::ProductionTensorInstructionSiteV1> {
    kernel
        .blocks()
        .iter()
        .enumerate()
        .flat_map(|(block, body)| {
            body.operations()
                .iter()
                .enumerate()
                .filter(|(_, item)| {
                    matches!(item, ProductionRankedOperationV1::TensorLayout { .. })
                })
                .map(move |(operation, _)| {
                    super::ProductionTensorInstructionSiteV1::new(
                        u32::try_from(block).unwrap_or(u32::MAX),
                        u32::try_from(operation).unwrap_or(u32::MAX),
                    )
                })
        })
        .collect()
}

#[derive(Clone, Copy)]
struct LiveTensorRefinementV1 {
    proof: DigestV1,
}

struct LiveTensorRefinementIndexV1 {
    by_output: Vec<Option<LiveTensorRefinementV1>>,
}

fn require_tensor_site_bijection_v1(
    live_tensor_sites: &BTreeSet<super::ProductionTensorInstructionSiteV1>,
    claimed_tensor_sites: &[super::ProductionTensorInstructionSiteV1],
) -> Result<
    BTreeSet<super::ProductionTensorInstructionSiteV1>,
    ProductionParallelReferenceContractErrorV1,
> {
    if live_tensor_sites.len() != claimed_tensor_sites.len() {
        return Err(
            ProductionParallelReferenceContractErrorV1::TensorFunctionalRefinementIncomplete {
                live_sites: live_tensor_sites.len(),
                policy_checked_sites: claimed_tensor_sites.len(),
            },
        );
    }
    let mut policy_checked_tensor_sites = BTreeSet::new();
    for tensor_site in claimed_tensor_sites {
        if !live_tensor_sites.contains(tensor_site) {
            return Err(
                ProductionParallelReferenceContractErrorV1::TensorInstructionSiteUnmatched {
                    block: tensor_site.block(),
                    operation: tensor_site.operation(),
                },
            );
        }
        if !policy_checked_tensor_sites.insert(*tensor_site) {
            return Err(
                ProductionParallelReferenceContractErrorV1::DuplicateTensorInstructionSite {
                    block: tensor_site.block(),
                    operation: tensor_site.operation(),
                },
            );
        }
    }
    if policy_checked_tensor_sites != *live_tensor_sites {
        return Err(
            ProductionParallelReferenceContractErrorV1::TensorFunctionalRefinementIncomplete {
                live_sites: live_tensor_sites.len(),
                policy_checked_sites: policy_checked_tensor_sites.len(),
            },
        );
    }
    Ok(policy_checked_tensor_sites)
}

impl LiveTensorRefinementIndexV1 {
    fn from_ranked(
        ranked: &ProductionRankedKernelLoweringInputV1,
        semantic_contract: &MirPlironSemanticContractV1,
    ) -> Result<Self, ProductionParallelReferenceContractErrorV1> {
        let live_tensor_sites = tensor_instruction_sites(ranked.kernel());
        let live_sites = live_tensor_sites.len();
        let sites = ranked
            .kernel()
            .blocks()
            .iter()
            .flat_map(|block| block.operations())
            .filter_map(|operation| match operation {
                ProductionRankedOperationV1::RequireTensorRefinement { contract, proof } => {
                    Some((contract, proof))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let claimed_tensor_sites = sites
            .iter()
            .map(|(contract, _)| contract.tensor_site())
            .collect::<Vec<_>>();
        let policy_checked_tensor_sites =
            require_tensor_site_bijection_v1(&live_tensor_sites, &claimed_tensor_sites)?;
        let mut retained_receipts = BTreeMap::new();
        for receipt in ranked.retained_policy_checked_refinement_staging() {
            if receipt.is_policy_checked_untrusted_staging() {
                *retained_receipts
                    .entry((receipt.receipt_identity(), receipt.binding()))
                    .or_insert(0_usize) += 1;
            }
        }
        let mut outputs_by_relation = BTreeMap::new();
        for (index, output) in semantic_contract.outputs().iter().enumerate() {
            outputs_by_relation
                .entry((output.view_identity(), output.actual(), output.reference()))
                .or_insert_with(Vec::new)
                .push(index);
        }
        let mut by_output = vec![None; semantic_contract.outputs().len()];
        for (site, (tensor, proof)) in sites.into_iter().enumerate() {
            let retained = retained_receipts
                .get(&(proof.receipt_identity(), proof.binding()))
                .copied()
                .unwrap_or_default();
            if retained != 1 {
                return Err(
                    ProductionParallelReferenceContractErrorV1::TensorFunctionalRefinementIncomplete {
                        live_sites,
                        policy_checked_sites: site,
                    },
                );
            }
            let output_key = (
                super::production_ranked_value_identity_v1(tensor.output_view()),
                super::production_ranked_value_identity_v1(tensor.actual()),
                super::production_ranked_value_identity_v1(tensor.reference()),
            );
            let matches = outputs_by_relation
                .get(&output_key)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let [index] = matches else {
                return Err(if matches.is_empty() {
                    ProductionParallelReferenceContractErrorV1::TensorOutputUnmatched { site }
                } else {
                    ProductionParallelReferenceContractErrorV1::TensorOutputAmbiguous {
                        site,
                        outputs: matches.len(),
                    }
                });
            };
            if by_output[*index].is_some() {
                return Err(
                    ProductionParallelReferenceContractErrorV1::DuplicateTensorOutput {
                        index: *index,
                    },
                );
            }
            by_output[*index] = Some(LiveTensorRefinementV1 {
                proof: proof.receipt_identity().digest(),
            });
        }
        debug_assert_eq!(policy_checked_tensor_sites, live_tensor_sites);
        Ok(Self { by_output })
    }

    fn for_output(&self, index: usize) -> Option<LiveTensorRefinementV1> {
        self.by_output.get(index).copied().flatten()
    }
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
            ProductionParallelReferenceContractErrorV1::PolicyCheckedStagingIncomplete {
                identity: digest(1),
            },
            ProductionParallelReferenceContractErrorV1::ScheduleRelationIncomplete {
                index: 3,
                detail: "missing live recurrence",
            },
            ProductionParallelReferenceContractErrorV1::DynamicBoundProofIncomplete { index: 4 },
            ProductionParallelReferenceContractErrorV1::NumericalProofIncomplete { index: 5 },
            ProductionParallelReferenceContractErrorV1::TensorFunctionalRefinementIncomplete {
                live_sites: 1,
                policy_checked_sites: 0,
            },
        ] {
            assert!(error.is_incomplete(), "{error}");
            assert!(error.to_string().starts_with("error[FE2O3-PARALLEL-"));
        }
    }

    #[test]
    fn tensor_receipts_must_form_an_exact_live_instruction_site_bijection() {
        let first = super::super::ProductionTensorInstructionSiteV1::new(0, 3);
        let second = super::super::ProductionTensorInstructionSiteV1::new(0, 20);
        let live = [first, second].into_iter().collect::<BTreeSet<_>>();
        assert_eq!(
            require_tensor_site_bijection_v1(&live, &[first, second]).unwrap(),
            live
        );
        assert!(matches!(
            require_tensor_site_bijection_v1(&live, &[first]),
            Err(
                ProductionParallelReferenceContractErrorV1::TensorFunctionalRefinementIncomplete {
                    live_sites: 2,
                    policy_checked_sites: 1,
                }
            )
        ));
        assert!(matches!(
            require_tensor_site_bijection_v1(&live, &[first, first]),
            Err(
                ProductionParallelReferenceContractErrorV1::DuplicateTensorInstructionSite {
                    block: 0,
                    operation: 3,
                }
            )
        ));
        assert!(matches!(
            require_tensor_site_bijection_v1(
                &live,
                &[
                    first,
                    super::super::ProductionTensorInstructionSiteV1::new(1, 0)
                ],
            ),
            Err(
                ProductionParallelReferenceContractErrorV1::TensorInstructionSiteUnmatched {
                    block: 1,
                    operation: 0,
                }
            )
        ));
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
            ProductionParallelReferenceContractErrorV1::NumericalSiteUnmatched { site: 0 },
            ProductionParallelReferenceContractErrorV1::NumericalSiteAmbiguous {
                site: 0,
                outputs: 2,
            },
            ProductionParallelReferenceContractErrorV1::DuplicateNumericalSite { index: 0 },
            ProductionParallelReferenceContractErrorV1::NumericalCoverageIncomplete {
                index: 0,
                component: "domain",
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
    fn tensor_boundary_reports_live_and_policy_checked_site_counts() {
        let error =
            ProductionParallelReferenceContractErrorV1::TensorFunctionalRefinementIncomplete {
                live_sites: 1,
                policy_checked_sites: 0,
            };
        assert!(error.is_incomplete());
        assert_eq!(
            error.to_string(),
            "error[FE2O3-PARALLEL-013]: cooperative tensor functional refinement is incomplete: 1 live tensor site(s), but 0 exact policy-checked result-component/output receipt binding(s)"
        );
    }
}
