//! Production join from compiler-derived sequential/parallel facts to one
//! workload-neutral parallel-reference contract.

use std::{collections::BTreeMap, error::Error, fmt};

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
    call_summaries: u64,
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
    TensorFunctionalRefinementIncomplete { live_sites: usize, outputs: usize },
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
            Self::AuthenticatedProofIncomplete { identity } => write!(formatter, "error[FE2O3-PARALLEL-005]: no retained authenticated per-compilation proof has identity {identity:?}"),
            Self::ScheduleRelationIncomplete { index, detail } => write!(formatter, "error[FE2O3-PARALLEL-006]: schedule relation {index} is incomplete: {detail}"),
            Self::FoldOrderRejected { index, detail } => write!(formatter, "error[FE2O3-PARALLEL-007]: fold order for relation {index} is not justified: {detail}"),
            Self::DynamicBoundProofIncomplete { index } => write!(formatter, "error[FE2O3-PARALLEL-008]: dynamic bounded recurrence {index} does not match the compiler-derived finite-bound identity for the live canonical loop"),
            Self::NumericalPolicyRejected { index, detail } => write!(formatter, "error[FE2O3-PARALLEL-009]: numerical policy for relation {index} is invalid: {detail}"),
            Self::NumericalProofIncomplete { index } => write!(formatter, "error[FE2O3-PARALLEL-010]: finite error policy for relation {index} lacks a live typed witness or retained authenticated proof"),
            Self::CallSummaryDerivationIncomplete { index, kind } => write!(formatter, "error[FE2O3-PARALLEL-011]: compiler cannot independently derive {kind} call summary {index} from the current ranked IR; declaration-only summaries are not evidence"),
            Self::CallSummaryMismatch { index } => write!(formatter, "error[FE2O3-PARALLEL-012]: helper or intrinsic summary {index} differs from live typed roots, scope, callsite, or authenticated proof"),
            Self::TensorFunctionalRefinementIncomplete { live_sites, outputs } => write!(formatter, "error[FE2O3-PARALLEL-013]: functional refinement is incomplete for {live_sites} live cooperative tensor site(s) and {outputs} logical output(s): typed SSA def-use and claim-specific tensor arithmetic receipts are not implemented"),
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
    require_no_live_tensor_functional_sites_v1(ranked.kernel(), semantic_contract.outputs().len())?;
    let mut relations = Vec::with_capacity(semantic_contract.outputs().len());
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
                vec![],
                proof,
            )
            .map_err(ProductionParallelReferenceContractErrorV1::ContractConstruction)?,
        );
    }
    let contract = ParallelReferenceContractV1::new(
        semantic_contract.canonical_sha256(),
        output_product_identity,
        relations,
        vec![],
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
    require_parallel_boundary_subjects_v1(ranked, evidence, semantics, semantic_contract)?;
    if expected.semantic_contract_identity() != semantics.contract_identity() {
        return Err(ProductionParallelReferenceContractErrorV1::SemanticContractMismatch);
    }
    require_no_live_tensor_functional_sites_v1(ranked.kernel(), semantic_contract.outputs().len())?;

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
        if !relation.call_summaries().is_empty() {
            return Err(ProductionParallelReferenceContractErrorV1::CallSummaryMismatch { index });
        }
    }
    if !expected.call_summaries().is_empty() {
        return Err(ProductionParallelReferenceContractErrorV1::CallSummaryMismatch { index: 0 });
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
        call_summaries: 0,
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

fn tensor_site_count(kernel: &super::ProductionRankedKernelV1) -> usize {
    kernel
        .blocks()
        .iter()
        .flat_map(|block| block.operations())
        .filter(|operation| matches!(operation, ProductionRankedOperationV1::TensorLayout { .. }))
        .count()
}

fn require_no_live_tensor_functional_sites_v1(
    kernel: &super::ProductionRankedKernelV1,
    outputs: usize,
) -> Result<(), ProductionParallelReferenceContractErrorV1> {
    let live_sites = tensor_site_count(kernel);
    if live_sites != 0 {
        return Err(
            ProductionParallelReferenceContractErrorV1::TensorFunctionalRefinementIncomplete {
                live_sites,
                outputs,
            },
        );
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
            ProductionParallelReferenceContractErrorV1::TensorFunctionalRefinementIncomplete {
                live_sites: 1,
                outputs: 2,
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

    fn live_tensor_kernel(site_count: usize) -> super::super::ProductionRankedKernelV1 {
        use super::super::{
            ProductionRankedBlockV1, ProductionRankedKernelV1, ProductionRankedTerminatorV1,
        };

        let tensor = ProductionRankedOperationV1::TensorLayout {
            contract:
                fe2o3_kernel_ir::TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
            convergence: dialect_kernel::TensorConvergenceAttr::UniformSubgroup,
            active_lanes: 64,
            binding: Some(
                super::super::ProductionCooperativeTensorBindingV1::new(
                    digest(1),
                    digest(2),
                    digest(3),
                    digest(4),
                    digest(5),
                    digest(6),
                    4,
                )
                .unwrap(),
            ),
        };
        ProductionRankedKernelV1::new(
            "tensor_functional_boundary",
            0,
            vec![ProductionRankedBlockV1::new(
                vec![tensor; site_count],
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap()
    }

    #[test]
    fn single_output_tensor_site_fails_closed_at_the_functional_boundary() {
        let error =
            require_no_live_tensor_functional_sites_v1(&live_tensor_kernel(1), 1).unwrap_err();
        assert_eq!(
            error,
            ProductionParallelReferenceContractErrorV1::TensorFunctionalRefinementIncomplete {
                live_sites: 1,
                outputs: 1,
            }
        );
        assert!(error.is_incomplete());
        assert_eq!(
            error.to_string(),
            "error[FE2O3-PARALLEL-013]: functional refinement is incomplete for 1 live cooperative tensor site(s) and 1 logical output(s): typed SSA def-use and claim-specific tensor arithmetic receipts are not implemented"
        );
    }

    #[test]
    fn multi_output_tensor_sites_do_not_guess_an_output_association() {
        assert_eq!(
            require_no_live_tensor_functional_sites_v1(&live_tensor_kernel(2), 2),
            Err(
                ProductionParallelReferenceContractErrorV1::TensorFunctionalRefinementIncomplete {
                    live_sites: 2,
                    outputs: 2,
                }
            )
        );
    }
}
