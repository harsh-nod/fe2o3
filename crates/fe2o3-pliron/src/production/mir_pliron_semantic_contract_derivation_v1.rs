//! Compiler-owned derivation of expected MIR/PLIRON semantic contract data.
//!
//! Derivation is deliberately not proof authority: every derived field is fed
//! back through the independent live-graph reconciliation before this module
//! returns a report.

use std::{collections::BTreeMap, error::Error, fmt};

use dialect_kernel::{DYNAMIC_EXTENT, OwnershipCoverageAttr};
use fe2o3_functional_proof::{
    FunctionalRefinementBoundaryV2, HARD_MAX_SEMANTIC_COLLECTIVES_V1, HARD_MAX_SEMANTIC_DOMAINS_V1,
    HARD_MAX_SEMANTIC_LOOPS_V1, HARD_MAX_SEMANTIC_OUTPUTS_V1, HARD_MAX_SEMANTIC_ROOTS_V1,
    MirPlironSemanticContractErrorV1, MirPlironSemanticContractV1, SemanticCollectiveContractV1,
    SemanticFiniteDomainV1, SemanticFiniteExtentV1, SemanticLoopContractV1,
    SemanticOutputContractV1, SemanticTypedRootV1,
};
use fe2o3_proof_contracts::DigestV1;

use super::mir_pliron_semantic_contract_v1::{
    LiveTypedRootV1, canonical_finite_loop_v1, coverage, evaluation_order, live_typed_roots,
    production_dynamic_output_symbol_v1, production_output_domain_identity_v1,
};
use super::{
    ProductionMiddleEndEvidenceV5, ProductionMirPlironSemanticContractErrorV1,
    ProductionMirPlironSemanticContractReportV1, ProductionNonCanonicalLoopProofErrorV1,
    ProductionNonCanonicalLoopProofRequirementV1, ProductionRankedKernelLoweringInputV1,
    ProductionRankedOperationV1, ProductionRankedValueV1, ProductionTotalOutputRefinementErrorV2,
    ProductionTotalOutputRefinementReportV2, derive_noncanonical_loop_proof_requirement_v1,
    production_effect_contract_identity_v1, production_ranked_value_identity_v1,
    require_mir_pliron_semantic_contract_v1, require_total_output_refinement_v2,
};

/// Compiler-derived contract data after independent reconciliation with the
/// exact borrowed ranked graph and V5 evidence.
///
/// The contract remains data. This owner does not prove compiler projection or
/// pass soundness and grants no LLVM, artifact, launch, or hardware authority.
#[derive(Debug)]
pub struct ProductionReconciledMirPlironSemanticContractV1 {
    contract: MirPlironSemanticContractV1,
    total_output: ProductionTotalOutputRefinementReportV2,
    semantics: ProductionMirPlironSemanticContractReportV1,
}

impl ProductionReconciledMirPlironSemanticContractV1 {
    pub const fn contract(&self) -> &MirPlironSemanticContractV1 {
        &self.contract
    }

    pub const fn total_output_report(&self) -> ProductionTotalOutputRefinementReportV2 {
        self.total_output
    }

    pub const fn semantic_contract_report(&self) -> ProductionMirPlironSemanticContractReportV1 {
        self.semantics
    }

    pub const fn compiler_projection_and_pass_soundness_remain_trusted(&self) -> bool {
        true
    }

    pub const fn grants_llvm_or_later_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionMirPlironSemanticContractDerivationErrorV1 {
    TotalOutput(ProductionTotalOutputRefinementErrorV2),
    MissingRetainedReceipt,
    WrongRefinementBoundary,
    InconsistentMirSubjects,
    ResourceLimit {
        resource: &'static str,
        limit: usize,
        actual: usize,
    },
    MissingOutputView {
        output: usize,
    },
    MissingTotalViewOwnership {
        output: usize,
    },
    AmbiguousTotalViewOwnership {
        output: usize,
        contracts: usize,
    },
    MissingTypedSemanticValue {
        value: DigestV1,
    },
    UnusedTypedSemanticRoot {
        value: DigestV1,
    },
    AmbiguousTypedRootDomain,
    UnsupportedNumericalPolicy,
    NonCanonicalLoopProofRequired {
        requirement: Box<ProductionNonCanonicalLoopProofRequirementV1>,
        detail: &'static str,
    },
    NonCanonicalLoopBoundary(ProductionNonCanonicalLoopProofErrorV1),
    AmbiguousFiniteDomain,
    Contract(MirPlironSemanticContractErrorV1),
    Reconciliation(ProductionMirPlironSemanticContractErrorV1),
}

impl fmt::Display for ProductionMirPlironSemanticContractDerivationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TotalOutput(error) => write!(formatter, "total-output gate failed: {error}"),
            Self::MissingRetainedReceipt => formatter.write_str(
                "compiler-derived semantic contract requires at least one retained functional-refinement receipt",
            ),
            Self::WrongRefinementBoundary => formatter.write_str(
                "compiler-derived semantic contract requires only safe-reference-MIR to kernel-MIR receipts",
            ),
            Self::InconsistentMirSubjects => formatter.write_str(
                "retained functional-refinement receipts do not identify one exact safe-reference MIR and kernel MIR pair",
            ),
            Self::ResourceLimit {
                resource,
                limit,
                actual,
            } => write!(
                formatter,
                "compiler-derived semantic contract {resource} count {actual} exceeds hard limit {limit}",
            ),
            Self::MissingOutputView { output } => write!(
                formatter,
                "effect output {output} does not refer to one live ranked view",
            ),
            Self::MissingTotalViewOwnership { output } => write!(
                formatter,
                "effect output {output} has no independently proved TotalView ownership contract",
            ),
            Self::AmbiguousTotalViewOwnership { output, contracts } => write!(
                formatter,
                "effect output {output} has {contracts} ownership contracts; exactly one proved TotalView contract is required",
            ),
            Self::MissingTypedSemanticValue { value } => write!(
                formatter,
                "loop, collective, or output value {value:?} has no typed semantic-expression commitment",
            ),
            Self::UnusedTypedSemanticRoot { value } => write!(
                formatter,
                "typed semantic-expression root {value:?} is not used by any loop, collective, or output",
            ),
            Self::AmbiguousTypedRootDomain => formatter.write_str(
                "one typed semantic root is used over incompatible finite domains",
            ),
            Self::UnsupportedNumericalPolicy => formatter.write_str(
                "a typed semantic root uses a numerical policy outside exact bitvectors or IEEE operator congruence",
            ),
            Self::NonCanonicalLoopProofRequired {
                requirement,
                detail,
            } => write!(
                formatter,
                "live loop SCC <header={}> is outside automatic finite-loop derivation: {detail}; exact proof request context {:?} binds the live CFG, MIR subjects, and PLIRON evidence, but imported invariant/variant receipts remain fail-closed until aggregate formula replay supports them",
                requirement.header_block(),
                requirement.exact_ranked_graph_identity(),
            ),
            Self::NonCanonicalLoopBoundary(error) => write!(
                formatter,
                "failed to derive the exact noncanonical-loop proof boundary: {error}"
            ),
            Self::AmbiguousFiniteDomain => formatter.write_str(
                "one finite-domain identity was derived with incompatible extents",
            ),
            Self::Contract(error) => write!(formatter, "derived contract is invalid: {error}"),
            Self::Reconciliation(error) => {
                write!(formatter, "derived contract failed live reconciliation: {error}")
            }
        }
    }
}

impl Error for ProductionMirPlironSemanticContractDerivationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::TotalOutput(error) => Some(error),
            Self::NonCanonicalLoopBoundary(error) => Some(error),
            Self::Contract(error) => Some(error),
            Self::Reconciliation(error) => Some(error),
            _ => None,
        }
    }
}

/// Derives expected contract data only from compiler-retained receipts and the
/// exact live ranked graph, then independently reconciles every field.
///
/// No workload declaration or caller-provided semantic contract is accepted.
pub fn derive_and_reconcile_mir_pliron_semantic_contract_v1(
    ranked: &ProductionRankedKernelLoweringInputV1,
    evidence: &ProductionMiddleEndEvidenceV5,
) -> Result<
    ProductionReconciledMirPlironSemanticContractV1,
    ProductionMirPlironSemanticContractDerivationErrorV1,
> {
    let total_output = require_total_output_refinement_v2(ranked, evidence)
        .map_err(ProductionMirPlironSemanticContractDerivationErrorV1::TotalOutput)?;
    let contract = derive_contract_data_v1(ranked, evidence)?;
    let semantics =
        require_mir_pliron_semantic_contract_v1(ranked, evidence, total_output, &contract)
            .map_err(ProductionMirPlironSemanticContractDerivationErrorV1::Reconciliation)?;
    Ok(ProductionReconciledMirPlironSemanticContractV1 {
        contract,
        total_output,
        semantics,
    })
}

fn derive_contract_data_v1(
    ranked: &ProductionRankedKernelLoweringInputV1,
    evidence: &ProductionMiddleEndEvidenceV5,
) -> Result<MirPlironSemanticContractV1, ProductionMirPlironSemanticContractDerivationErrorV1> {
    let receipts = ranked.retained_functional_refinement_receipts();
    let first = receipts
        .first()
        .ok_or(ProductionMirPlironSemanticContractDerivationErrorV1::MissingRetainedReceipt)?;
    if receipts.iter().any(|receipt| {
        receipt.boundary() != FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir
    }) {
        return Err(ProductionMirPlironSemanticContractDerivationErrorV1::WrongRefinementBoundary);
    }
    let first_binding = first.binding();
    if receipts.iter().any(|receipt| {
        let binding = receipt.binding();
        binding.safe_reference_mir_hash() != first_binding.safe_reference_mir_hash()
            || binding.kernel_mir_hash() != first_binding.kernel_mir_hash()
    }) {
        return Err(ProductionMirPlironSemanticContractDerivationErrorV1::InconsistentMirSubjects);
    }

    let live_roots = live_typed_roots(ranked);
    require_limit("typed root", live_roots.len(), HARD_MAX_SEMANTIC_ROOTS_V1)?;
    let semantic_expression_count = ranked
        .kernel()
        .blocks()
        .iter()
        .flat_map(|block| block.operations())
        .filter(|operation| {
            matches!(
                operation,
                ProductionRankedOperationV1::SemanticExpression { .. }
            )
        })
        .count();
    if live_roots.len() != semantic_expression_count {
        return Err(
            ProductionMirPlironSemanticContractDerivationErrorV1::UnsupportedNumericalPolicy,
        );
    }
    let mut domains = BTreeMap::<DigestV1, Vec<SemanticFiniteExtentV1>>::new();
    let mut root_domains = BTreeMap::<ProductionRankedValueV1, DigestV1>::new();
    let evidence_identity = DigestV1::from_untrusted_bytes(*evidence.identity().sha256());

    let live_collectives = ranked
        .kernel()
        .blocks()
        .iter()
        .flat_map(|block| block.operations())
        .filter_map(|operation| match operation {
            ProductionRankedOperationV1::CollectiveSemantics {
                contract,
                view,
                actual,
                expected,
                witness0,
                witness1,
            } => Some((contract, *view, *actual, *expected, *witness0, *witness1)),
            _ => None,
        })
        .collect::<Vec<_>>();
    require_limit(
        "collective",
        live_collectives.len(),
        HARD_MAX_SEMANTIC_COLLECTIVES_V1,
    )?;

    let live_outputs = ranked
        .kernel()
        .blocks()
        .iter()
        .flat_map(|block| block.operations())
        .filter_map(|operation| match operation {
            ProductionRankedOperationV1::RequireEffectRefinement { contract, .. } => Some(contract),
            _ => None,
        })
        .collect::<Vec<_>>();
    require_limit("output", live_outputs.len(), HARD_MAX_SEMANTIC_OUTPUTS_V1)?;
    let mut outputs = Vec::with_capacity(live_outputs.len());
    for (output_index, output) in live_outputs.into_iter().enumerate() {
        let shape = live_view_shape(ranked, output.view()).ok_or(
            ProductionMirPlironSemanticContractDerivationErrorV1::MissingOutputView {
                output: output_index,
            },
        )?;
        let mut extents = Vec::with_capacity(shape.len());
        for (dimension, extent) in shape.iter().copied().enumerate() {
            extents.push(if extent == DYNAMIC_EXTENT {
                SemanticFiniteExtentV1::Dynamic {
                    symbol: production_dynamic_output_symbol_v1(
                        output.view(),
                        evidence_identity,
                        dimension,
                    ),
                    inclusive_upper_bound: u64::MAX,
                }
            } else {
                SemanticFiniteExtentV1::Static(extent)
            });
        }
        let maximum_cardinality = extents.iter().try_fold(1_u64, |product, extent| {
            product.checked_mul(extent.inclusive_upper_bound())
        });
        let collective_domains = live_collectives
            .iter()
            .filter(|(contract, view, ..)| {
                *view == output.view() && Some(contract.domain_bound()) == maximum_cardinality
            })
            .map(|(contract, ..)| words_digest(contract.target_domain_identity()))
            .collect::<std::collections::BTreeSet<_>>();
        let domain = match collective_domains.len() {
            0 => production_output_domain_identity_v1(output.view(), evidence_identity, shape),
            1 => *collective_domains
                .iter()
                .next()
                .expect("one domain was counted"),
            _ => {
                return Err(
                    ProductionMirPlironSemanticContractDerivationErrorV1::AmbiguousFiniteDomain,
                );
            }
        };
        insert_domain(&mut domains, domain, extents)?;
        require_total_view_ownership(ranked, output.view(), output_index)?;

        let auxiliary_values = output
            .gpu_coordinates()
            .iter()
            .chain(output.reference_coordinates())
            .copied()
            .chain([
                output.gpu_domain(),
                output.reference_domain(),
                output.gpu_precondition(),
                output.reference_precondition(),
            ])
            .collect::<Vec<_>>();
        for value in [output.gpu_value(), output.reference_value()]
            .into_iter()
            .chain(auxiliary_values.iter().copied())
        {
            assign_root_domain(&live_roots, &mut root_domains, value, domain)?;
        }
        outputs.push(
            SemanticOutputContractV1::new(
                production_effect_contract_identity_v1(output.contract_identity()),
                production_ranked_value_identity_v1(output.view()),
                domain,
                production_ranked_value_identity_v1(output.gpu_value()),
                production_ranked_value_identity_v1(output.reference_value()),
                auxiliary_values
                    .into_iter()
                    .map(production_ranked_value_identity_v1)
                    .collect(),
            )
            .map_err(ProductionMirPlironSemanticContractDerivationErrorV1::Contract)?,
        );
    }

    let mut collectives = Vec::with_capacity(live_collectives.len());
    for (live, view, actual, expected, witness0, witness1) in live_collectives {
        let source_domain = words_digest(live.source_domain_identity());
        let target_domain = words_digest(live.target_domain_identity());
        insert_collective_domain(&mut domains, source_domain, live.domain_bound())?;
        insert_collective_domain(&mut domains, target_domain, live.domain_bound())?;
        for value in [actual, expected] {
            assign_root_domain(&live_roots, &mut root_domains, value, target_domain)?;
        }
        for value in [witness0, witness1] {
            assign_root_domain(&live_roots, &mut root_domains, value, source_domain)?;
        }
        collectives.push(
            SemanticCollectiveContractV1::new(
                words_digest(live.contract_identity()),
                super::mir_pliron_semantic_contract_v1::collective_kind(live.kind()),
                production_ranked_value_identity_v1(view),
                source_domain,
                target_domain,
                production_ranked_value_identity_v1(actual),
                production_ranked_value_identity_v1(expected),
                production_ranked_value_identity_v1(witness0),
                production_ranked_value_identity_v1(witness1),
                live.domain_bound(),
                live.step_bound(),
                evaluation_order(live.order()),
                coverage(live.coverage()),
            )
            .map_err(ProductionMirPlironSemanticContractDerivationErrorV1::Contract)?,
        );
    }

    let backedges = super::mir_pliron_semantic_contract_v1::natural_backedges(ranked.kernel());
    require_limit("loop", backedges.len(), HARD_MAX_SEMANTIC_LOOPS_V1)?;
    let backedge_set = backedges
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let mut loops = Vec::with_capacity(backedges.len());
    for (latch, header) in backedges {
        let live = match canonical_finite_loop_v1(ranked, header, latch) {
            Ok(live) => live,
            Err(detail) => {
                let requirement = derive_noncanonical_loop_proof_requirement_v1(
                    ranked.kernel(),
                    header,
                    first_binding.subjects(),
                    evidence_identity,
                )
                .map_err(
                    ProductionMirPlironSemanticContractDerivationErrorV1::NonCanonicalLoopBoundary,
                )?;
                return Err(
                    ProductionMirPlironSemanticContractDerivationErrorV1::NonCanonicalLoopProofRequired {
                        requirement: Box::new(requirement),
                        detail,
                    },
                );
            }
        };
        insert_domain(&mut domains, live.iteration_domain, vec![live.extent])?;
        for value in [
            live.induction_value,
            live.lower_value,
            live.upper_value,
            live.step_value,
        ] {
            if live_roots.iter().any(|root| root.value == value) {
                assign_root_domain(&live_roots, &mut root_domains, value, live.iteration_domain)?;
            }
        }
        loops.push(
            SemanticLoopContractV1::new(
                live.identity,
                header,
                latch,
                live.exit,
                live.iteration_domain,
                production_ranked_value_identity_v1(live.induction_value),
                production_ranked_value_identity_v1(live.lower_value),
                production_ranked_value_identity_v1(live.upper_value),
                production_ranked_value_identity_v1(live.step_value),
                live.transition,
                live.variant,
                live.direction,
                live.maximum_steps,
            )
            .map_err(ProductionMirPlironSemanticContractDerivationErrorV1::Contract)?,
        );
    }
    if let Some(header) = super::noncanonical_loop_proof_v1::noncanonical_cyclic_scc_headers_v1(
        ranked.kernel(),
        &backedge_set,
    )
    .into_iter()
    .next()
    {
        let requirement = derive_noncanonical_loop_proof_requirement_v1(
            ranked.kernel(),
            header,
            first_binding.subjects(),
            evidence_identity,
        )
        .map_err(ProductionMirPlironSemanticContractDerivationErrorV1::NonCanonicalLoopBoundary)?;
        return Err(
            ProductionMirPlironSemanticContractDerivationErrorV1::NonCanonicalLoopProofRequired {
                requirement: Box::new(requirement),
                detail: "the cyclic SCC has no single compiler-admitted natural-loop entry",
            },
        );
    }

    require_limit("domain", domains.len(), HARD_MAX_SEMANTIC_DOMAINS_V1)?;
    let domains = domains
        .into_iter()
        .map(|(identity, extents)| SemanticFiniteDomainV1::new(identity, extents))
        .collect::<Result<Vec<_>, _>>()
        .map_err(ProductionMirPlironSemanticContractDerivationErrorV1::Contract)?;
    let typed_roots = live_roots
        .into_iter()
        .map(|root| {
            let domain = root_domains.get(&root.value).copied().ok_or(
                ProductionMirPlironSemanticContractDerivationErrorV1::UnusedTypedSemanticRoot {
                    value: production_ranked_value_identity_v1(root.value),
                },
            )?;
            SemanticTypedRootV1::new(
                production_ranked_value_identity_v1(root.value),
                root.commitment,
                domain,
                root.scalar,
                root.numerical_policy,
            )
            .map_err(ProductionMirPlironSemanticContractDerivationErrorV1::Contract)
        })
        .collect::<Result<Vec<_>, _>>()?;

    MirPlironSemanticContractV1::new(
        first_binding.safe_reference_mir_hash(),
        first_binding.kernel_mir_hash(),
        evidence_identity,
        domains,
        typed_roots,
        loops,
        collectives,
        outputs,
    )
    .map_err(ProductionMirPlironSemanticContractDerivationErrorV1::Contract)
}

fn live_view_shape(
    ranked: &ProductionRankedKernelLoweringInputV1,
    view: ProductionRankedValueV1,
) -> Option<&[u64]> {
    let ProductionRankedValueV1::Local(view) = view else {
        return None;
    };
    ranked
        .kernel()
        .blocks()
        .iter()
        .flat_map(|block| block.operations())
        .find_map(|operation| match operation {
            ProductionRankedOperationV1::View { result, shape, .. }
            | ProductionRankedOperationV1::ViewInSpace { result, shape, .. }
                if *result == view =>
            {
                Some(shape.as_slice())
            }
            _ => None,
        })
}

fn require_total_view_ownership(
    ranked: &ProductionRankedKernelLoweringInputV1,
    view: ProductionRankedValueV1,
    output: usize,
) -> Result<(), ProductionMirPlironSemanticContractDerivationErrorV1> {
    let contracts = ranked
        .kernel()
        .blocks()
        .iter()
        .flat_map(|block| block.operations())
        .filter_map(|operation| match operation {
            ProductionRankedOperationV1::OwnershipContract {
                view: candidate,
                coverage,
                ..
            } if *candidate == view => Some(*coverage),
            _ => None,
        })
        .collect::<Vec<_>>();
    match contracts.as_slice() {
        [OwnershipCoverageAttr::TotalView] => Ok(()),
        [] | [OwnershipCoverageAttr::ExactEffectDomain] => Err(
            ProductionMirPlironSemanticContractDerivationErrorV1::MissingTotalViewOwnership {
                output,
            },
        ),
        _ => Err(
            ProductionMirPlironSemanticContractDerivationErrorV1::AmbiguousTotalViewOwnership {
                output,
                contracts: contracts.len(),
            },
        ),
    }
}

fn assign_root_domain(
    roots: &[LiveTypedRootV1],
    domains: &mut BTreeMap<ProductionRankedValueV1, DigestV1>,
    value: ProductionRankedValueV1,
    domain: DigestV1,
) -> Result<(), ProductionMirPlironSemanticContractDerivationErrorV1> {
    if !roots.iter().any(|root| root.value == value) {
        return Err(
            ProductionMirPlironSemanticContractDerivationErrorV1::MissingTypedSemanticValue {
                value: production_ranked_value_identity_v1(value),
            },
        );
    }
    match domains.insert(value, domain) {
        Some(previous) if previous != domain => {
            Err(ProductionMirPlironSemanticContractDerivationErrorV1::AmbiguousTypedRootDomain)
        }
        _ => Ok(()),
    }
}

fn insert_domain(
    domains: &mut BTreeMap<DigestV1, Vec<SemanticFiniteExtentV1>>,
    identity: DigestV1,
    extents: Vec<SemanticFiniteExtentV1>,
) -> Result<(), ProductionMirPlironSemanticContractDerivationErrorV1> {
    match domains.insert(identity, extents.clone()) {
        Some(previous) if previous != extents => {
            Err(ProductionMirPlironSemanticContractDerivationErrorV1::AmbiguousFiniteDomain)
        }
        _ => Ok(()),
    }
}

fn insert_collective_domain(
    domains: &mut BTreeMap<DigestV1, Vec<SemanticFiniteExtentV1>>,
    identity: DigestV1,
    bound: u64,
) -> Result<(), ProductionMirPlironSemanticContractDerivationErrorV1> {
    if let Some(extents) = domains.get(&identity) {
        let cardinality = extents.iter().try_fold(1_u64, |product, extent| {
            product.checked_mul(extent.inclusive_upper_bound())
        });
        return if cardinality == Some(bound) {
            Ok(())
        } else {
            Err(ProductionMirPlironSemanticContractDerivationErrorV1::AmbiguousFiniteDomain)
        };
    }
    insert_domain(
        domains,
        identity,
        vec![SemanticFiniteExtentV1::Static(bound)],
    )
}

fn require_limit(
    resource: &'static str,
    actual: usize,
    limit: usize,
) -> Result<(), ProductionMirPlironSemanticContractDerivationErrorV1> {
    if actual > limit {
        return Err(
            ProductionMirPlironSemanticContractDerivationErrorV1::ResourceLimit {
                resource,
                limit,
                actual,
            },
        );
    }
    Ok(())
}

fn words_digest(words: [u64; 4]) -> DigestV1 {
    let mut bytes = [0_u8; 32];
    for (index, word) in words.into_iter().enumerate() {
        bytes[index * 8..index * 8 + 8].copy_from_slice(&word.to_le_bytes());
    }
    DigestV1::from_untrusted_bytes(bytes)
}
