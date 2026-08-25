//! Exact join between one workload-neutral semantic contract and live PLIRON.

use std::{collections::BTreeSet, error::Error, fmt};

use dialect_kernel::{DYNAMIC_EXTENT, SemanticCoverageBindingAttr, SemanticEvaluationOrderAttr};
use fe2o3_functional_proof::{
    FunctionalRefinementBoundaryV2, MirPlironSemanticContractV1, SemanticCollectiveKindV1,
    SemanticCoverageBindingV1, SemanticEvaluationOrderV1, SemanticFiniteExtentV1,
    SemanticIeeeExceptionalValueV1, SemanticIeeeRoundingV1, SemanticLoopContractV1,
    SemanticLoopDirectionV1, SemanticNumericalPolicyV1, SemanticScalarTypeV1,
};
use fe2o3_proof_contracts::DigestV1;
use sha2::{Digest as _, Sha256};

use super::{
    ProductionCollectiveSemanticKindV1, ProductionIeeeExceptionalValuePolicyV2,
    ProductionIeeeRoundingModeV2, ProductionMiddleEndEvidenceV5, ProductionNumericalContractV2,
    ProductionRankedKernelLoweringInputV1, ProductionRankedOperationV1,
    ProductionRankedTerminatorV1, ProductionRankedValueV1, ProductionSemanticScalarTypeV2,
    ProductionTotalOutputRefinementErrorV2, ProductionTotalOutputRefinementReportV2,
};

const EFFECT_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/MIR-PLIRON/EFFECT-IDENTITY/V1\0";
const VALUE_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/MIR-PLIRON/RANKED-VALUE/V1\0";
const LOOP_TRANSITION_DOMAIN_V1: &[u8] = b"FE2O3/MIR-PLIRON/LOOP-TRANSITION/V1\0";
const LOOP_VARIANT_DOMAIN_V1: &[u8] = b"FE2O3/MIR-PLIRON/LOOP-VARIANT/V1\0";
const LOOP_CONTRACT_DOMAIN_V1: &[u8] = b"FE2O3/MIR-PLIRON/LOOP-CONTRACT/V1\0";
const LOOP_ITERATION_DOMAIN_V1: &[u8] = b"FE2O3/MIR-PLIRON/LOOP-ITERATION-DOMAIN/V1\0";
const OUTPUT_DOMAIN_V1: &[u8] = b"FE2O3/MIR-PLIRON/OUTPUT-DOMAIN/V1\0";
const DYNAMIC_OUTPUT_SYMBOL_V1: &[u8] = b"FE2O3/MIR-PLIRON/DYNAMIC-OUTPUT-SYMBOL/V1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionMirPlironSemanticContractReportV1 {
    contract_identity: DigestV1,
    finite_domains: u64,
    bounded_loops: u64,
    finite_collectives: u64,
    total_outputs: u64,
    typed_roots: u64,
}

impl ProductionMirPlironSemanticContractReportV1 {
    pub const fn contract_identity(self) -> DigestV1 {
        self.contract_identity
    }
    pub const fn finite_domains(self) -> u64 {
        self.finite_domains
    }
    pub const fn bounded_loops(self) -> u64 {
        self.bounded_loops
    }
    pub const fn finite_collectives(self) -> u64 {
        self.finite_collectives
    }
    pub const fn total_outputs(self) -> u64 {
        self.total_outputs
    }
    pub const fn typed_roots(self) -> u64 {
        self.typed_roots
    }
    /// The accepted statement is bound to the exact MIR subjects and PLIRON
    /// evidence. Soundness of the MIR projector and mandatory analyses remains
    /// in the compiler trusted base.
    pub const fn binds_safe_reference_mir_to_live_pliron(self) -> bool {
        true
    }
    pub const fn proves_the_compiler_implementation_sound(self) -> bool {
        false
    }
    pub const fn has_authenticated_per_compilation_verus_execution(self) -> bool {
        false
    }
    pub const fn grants_llvm_or_later_authority(self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionMirPlironSemanticContractErrorV1 {
    TotalOutputEvidenceMismatch,
    MirSubjectMismatch,
    WrongRefinementBoundary,
    TypedRootMismatch,
    LoopCoverageMismatch,
    UnsupportedLoopShape {
        header: u32,
        latch: u32,
    },
    LoopStructureMismatch {
        header: u32,
        latch: u32,
        exit: u32,
    },
    LoopValueMismatch {
        header: u32,
        latch: u32,
    },
    DynamicLoopBoundUnproved {
        header: u32,
        latch: u32,
        inclusive_upper_bound: u64,
    },
    DynamicLoopStepUnproved {
        header: u32,
        latch: u32,
    },
    CollectiveCountMismatch,
    CollectiveMismatch {
        index: usize,
    },
    OutputCountMismatch,
    OutputMismatch {
        index: usize,
    },
    CounterOverflow,
}

impl fmt::Display for ProductionMirPlironSemanticContractErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TotalOutputEvidenceMismatch => formatter.write_str(
                "semantic contract does not identify the exact live total-output PLIRON evidence",
            ),
            Self::MirSubjectMismatch => formatter.write_str(
                "semantic contract MIR subjects differ from a retained functional-refinement receipt",
            ),
            Self::WrongRefinementBoundary => formatter.write_str(
                "semantic contract requires safe-reference-MIR to kernel-MIR receipts",
            ),
            Self::TypedRootMismatch => formatter.write_str(
                "semantic contract typed roots differ from the live PLIRON semantic roots",
            ),
            Self::LoopCoverageMismatch => formatter.write_str(
                "semantic contract does not cover every live PLIRON CFG backedge exactly once",
            ),
            Self::UnsupportedLoopShape { header, latch } => write!(
                formatter,
                "natural loop <header={header}, latch={latch}> is not a canonical finite induction loop",
            ),
            Self::LoopStructureMismatch { header, latch, exit } => write!(
                formatter,
                "bounded-loop contract <header={header}, latch={latch}, exit={exit}> does not match a live natural-loop edge",
            ),
            Self::LoopValueMismatch { header, latch } => write!(
                formatter,
                "bounded-loop contract <header={header}, latch={latch}> does not bind the live induction, bounds, step, transition, and variant",
            ),
            Self::DynamicLoopBoundUnproved {
                header,
                latch,
                inclusive_upper_bound,
            } => write!(
                formatter,
                "dynamic loop <header={header}, latch={latch}> claims inclusive upper bound {inclusive_upper_bound}, but no production range receipt proves a bound narrower than u64::MAX",
            ),
            Self::DynamicLoopStepUnproved { header, latch } => write!(
                formatter,
                "dynamic loop <header={header}, latch={latch}> does not have a constant unit step, so overflow-free progress to its u64 upper bound is unproved",
            ),
            Self::CollectiveCountMismatch => formatter.write_str(
                "semantic contract collective count differs from the live PLIRON graph",
            ),
            Self::CollectiveMismatch { index } => write!(
                formatter,
                "semantic contract collective {index} differs from the live PLIRON contract",
            ),
            Self::OutputCountMismatch => formatter.write_str(
                "semantic contract output count differs from the live effect-refinement bijection",
            ),
            Self::OutputMismatch { index } => write!(
                formatter,
                "semantic contract output {index} differs from its live PLIRON effect refinement",
            ),
            Self::CounterOverflow => formatter.write_str(
                "semantic contract count cannot be represented in the production report",
            ),
        }
    }
}

impl Error for ProductionMirPlironSemanticContractErrorV1 {}

/// Move-only owner of one kernel admitted at the safe-reference-MIR to PLIRON
/// boundary.
///
/// ```compile_fail
/// use fe2o3_pliron::ProductionVerifiedMirPlironKernelV1;
///
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ProductionVerifiedMirPlironKernelV1>();
/// ```
#[derive(Debug)]
pub struct ProductionVerifiedMirPlironKernelV1 {
    ranked: ProductionRankedKernelLoweringInputV1,
    evidence: ProductionMiddleEndEvidenceV5,
    total_output: ProductionTotalOutputRefinementReportV2,
    semantics: ProductionMirPlironSemanticContractReportV1,
    contract: MirPlironSemanticContractV1,
}

impl ProductionVerifiedMirPlironKernelV1 {
    pub const fn ranked(&self) -> &ProductionRankedKernelLoweringInputV1 {
        &self.ranked
    }
    pub const fn evidence(&self) -> &ProductionMiddleEndEvidenceV5 {
        &self.evidence
    }
    pub const fn total_output_report(&self) -> ProductionTotalOutputRefinementReportV2 {
        self.total_output
    }
    pub const fn semantic_contract_report(&self) -> ProductionMirPlironSemanticContractReportV1 {
        self.semantics
    }
    /// Returns the exact validated contract retained under this move-only owner.
    ///
    /// The contract is data, not proof authority. Its fields have already been
    /// reconciled against the owned MIR receipts and live PLIRON evidence.
    pub const fn semantic_contract(&self) -> &MirPlironSemanticContractV1 {
        &self.contract
    }
    pub const fn establishes_total_output_refinement_at_mir_pliron_boundary(&self) -> bool {
        true
    }
    pub const fn compiler_projection_and_pass_soundness_remain_trusted(&self) -> bool {
        true
    }
    pub const fn grants_llvm_or_later_authority(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub enum ProductionMirPlironVerificationErrorV1 {
    TotalOutput(ProductionTotalOutputRefinementErrorV2),
    SemanticContract(ProductionMirPlironSemanticContractErrorV1),
}

impl fmt::Display for ProductionMirPlironVerificationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TotalOutput(error) => {
                write!(formatter, "total-output refinement failed: {error}")
            }
            Self::SemanticContract(error) => {
                write!(formatter, "MIR/PLIRON semantic contract failed: {error}")
            }
        }
    }
}

impl Error for ProductionMirPlironVerificationErrorV1 {}

pub fn verify_ranked_kernel_against_safe_reference_mir_v1(
    ranked: ProductionRankedKernelLoweringInputV1,
    evidence: ProductionMiddleEndEvidenceV5,
    contract: &MirPlironSemanticContractV1,
) -> Result<ProductionVerifiedMirPlironKernelV1, ProductionMirPlironVerificationErrorV1> {
    let total_output = super::require_total_output_refinement_v2(&ranked, &evidence)
        .map_err(ProductionMirPlironVerificationErrorV1::TotalOutput)?;
    let semantics =
        require_mir_pliron_semantic_contract_v1(&ranked, &evidence, total_output, contract)
            .map_err(ProductionMirPlironVerificationErrorV1::SemanticContract)?;
    Ok(ProductionVerifiedMirPlironKernelV1 {
        ranked,
        evidence,
        total_output,
        semantics,
        contract: contract.clone(),
    })
}

pub fn require_mir_pliron_semantic_contract_v1(
    ranked: &ProductionRankedKernelLoweringInputV1,
    evidence: &ProductionMiddleEndEvidenceV5,
    total_output: ProductionTotalOutputRefinementReportV2,
    contract: &MirPlironSemanticContractV1,
) -> Result<ProductionMirPlironSemanticContractReportV1, ProductionMirPlironSemanticContractErrorV1>
{
    let evidence_identity = DigestV1::from_untrusted_bytes(*evidence.identity().sha256());
    if contract.pliron_evidence() != evidence_identity
        || total_output.evidence_identity() != evidence.identity().sha256()
    {
        return Err(ProductionMirPlironSemanticContractErrorV1::TotalOutputEvidenceMismatch);
    }
    for receipt in ranked.retained_functional_refinement_receipts() {
        let binding = receipt.binding();
        if binding.safe_reference_mir_hash() != contract.safe_reference_mir()
            || binding.kernel_mir_hash() != contract.kernel_mir()
        {
            return Err(ProductionMirPlironSemanticContractErrorV1::MirSubjectMismatch);
        }
        if receipt.boundary() != FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir {
            return Err(ProductionMirPlironSemanticContractErrorV1::WrongRefinementBoundary);
        }
    }

    let live_roots = live_typed_roots(ranked);
    if live_roots.len() != contract.typed_roots().len()
        || live_roots
            .iter()
            .zip(contract.typed_roots())
            .any(|(live, declared)| {
                production_ranked_value_identity_v1(live.value) != declared.identity()
                    || live.commitment != declared.commitment()
                    || live.scalar != declared.scalar()
                    || live.numerical_policy != declared.numerical_policy()
            })
    {
        return Err(ProductionMirPlironSemanticContractErrorV1::TypedRootMismatch);
    }

    let backedges = natural_backedges(ranked.kernel());
    if backedges.len() != contract.loops().len() {
        return Err(ProductionMirPlironSemanticContractErrorV1::LoopCoverageMismatch);
    }
    let backedges = backedges.into_iter().collect::<BTreeSet<_>>();
    for loop_contract in contract.loops() {
        let edge = (loop_contract.latch_block(), loop_contract.header_block());
        if !backedges.contains(&edge) {
            return Err(
                ProductionMirPlironSemanticContractErrorV1::LoopStructureMismatch {
                    header: loop_contract.header_block(),
                    latch: loop_contract.latch_block(),
                    exit: loop_contract.exit_block(),
                },
            );
        }
        require_live_loop_contract(ranked, contract, loop_contract)?;
    }

    let values = live_typed_values(ranked);
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
                ..
            } => Some((contract, view, actual, expected, witness0, witness1)),
            _ => None,
        })
        .collect::<Vec<_>>();
    if live_collectives.len() != contract.collectives().len()
        || total_output.collective_contracts() as usize != live_collectives.len()
    {
        return Err(ProductionMirPlironSemanticContractErrorV1::CollectiveCountMismatch);
    }
    for (index, (live, declared)) in live_collectives
        .iter()
        .zip(contract.collectives())
        .enumerate()
    {
        let (live, view, actual, expected, witness0, witness1) = *live;
        let matches = words_digest(live.contract_identity()) == declared.identity()
            && production_ranked_value_identity_v1(*view) == declared.view_identity()
            && words_digest(live.source_domain_identity()) == declared.source_domain()
            && words_digest(live.target_domain_identity()) == declared.target_domain()
            && live.domain_bound() == declared.domain_bound()
            && live.step_bound() == declared.step_bound()
            && collective_kind(live.kind()) == declared.kind()
            && evaluation_order(live.order()) == declared.order()
            && coverage(live.coverage()) == declared.coverage()
            && typed_value_identity(&values, *actual) == Some(declared.actual())
            && typed_value_identity(&values, *expected) == Some(declared.expected())
            && typed_value_identity(&values, *witness0) == Some(declared.witness0())
            && typed_value_identity(&values, *witness1) == Some(declared.witness1());
        if !matches {
            return Err(ProductionMirPlironSemanticContractErrorV1::CollectiveMismatch { index });
        }
    }

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
    if live_outputs.len() != contract.outputs().len()
        || total_output.effect_contracts() as usize != live_outputs.len()
    {
        return Err(ProductionMirPlironSemanticContractErrorV1::OutputCountMismatch);
    }
    for (index, (live, declared)) in live_outputs.iter().zip(contract.outputs()).enumerate() {
        let view_shape = live_view_shape(ranked, live.view());
        let live_auxiliary = live
            .gpu_coordinates()
            .iter()
            .chain(live.reference_coordinates())
            .copied()
            .chain([
                live.gpu_domain(),
                live.reference_domain(),
                live.gpu_precondition(),
                live.reference_precondition(),
            ])
            .map(|value| typed_value_identity(&values, value))
            .collect::<Option<Vec<_>>>();
        let domain = contract
            .domains()
            .iter()
            .find(|domain| domain.identity() == declared.output_domain());
        let matches = declared.identity()
            == production_effect_contract_identity_v1(live.contract_identity())
            && declared.view_identity() == production_ranked_value_identity_v1(live.view())
            && typed_value_identity(&values, live.gpu_value()) == Some(declared.actual())
            && typed_value_identity(&values, live.reference_value()) == Some(declared.reference())
            && live_auxiliary.as_deref() == Some(declared.auxiliary_roots())
            && domain.is_some_and(|domain| {
                shape_matches(domain.extents(), view_shape, live.view(), evidence_identity)
            });
        if !matches {
            return Err(ProductionMirPlironSemanticContractErrorV1::OutputMismatch { index });
        }
    }

    Ok(ProductionMirPlironSemanticContractReportV1 {
        contract_identity: contract.canonical_sha256(),
        finite_domains: count(contract.domains().len())?,
        bounded_loops: count(contract.loops().len())?,
        finite_collectives: count(contract.collectives().len())?,
        total_outputs: count(contract.outputs().len())?,
        typed_roots: count(contract.typed_roots().len())?,
    })
}

pub fn production_effect_contract_identity_v1(contract_identity: u64) -> DigestV1 {
    domain_digest(EFFECT_IDENTITY_DOMAIN_V1, &contract_identity.to_le_bytes())
}

pub fn production_ranked_value_identity_v1(value: ProductionRankedValueV1) -> DigestV1 {
    let mut bytes = [0_u8; 9];
    match value {
        ProductionRankedValueV1::Argument(index) => {
            bytes[0] = 1;
            bytes[1..5].copy_from_slice(&index.to_le_bytes());
        }
        ProductionRankedValueV1::BlockArgument { block, argument } => {
            bytes[0] = 2;
            bytes[1..5].copy_from_slice(&block.to_le_bytes());
            bytes[5..9].copy_from_slice(&argument.to_le_bytes());
        }
        ProductionRankedValueV1::Local(identity) => {
            bytes[0] = 3;
            bytes[1..5].copy_from_slice(&identity.get().to_le_bytes());
        }
    }
    domain_digest(VALUE_IDENTITY_DOMAIN_V1, &bytes)
}

/// Canonical identity for one exact output shape. Dynamic shapes are
/// additionally bound to the view and exact V5 evidence record.
pub(super) fn production_output_domain_identity_v1(
    view: ProductionRankedValueV1,
    evidence: DigestV1,
    shape: &[u64],
) -> DigestV1 {
    let dynamic = shape.contains(&DYNAMIC_EXTENT);
    let mut bytes = Vec::with_capacity(8 + shape.len() * 8 + usize::from(dynamic) * 64);
    if dynamic {
        bytes.extend_from_slice(production_ranked_value_identity_v1(view).as_bytes());
        bytes.extend_from_slice(evidence.as_bytes());
    }
    bytes.extend_from_slice(&(shape.len() as u64).to_le_bytes());
    for extent in shape {
        bytes.extend_from_slice(&extent.to_le_bytes());
    }
    domain_digest(OUTPUT_DOMAIN_V1, &bytes)
}

pub(super) fn production_dynamic_output_symbol_v1(
    view: ProductionRankedValueV1,
    evidence: DigestV1,
    dimension: usize,
) -> u32 {
    let mut bytes = Vec::with_capacity(32 * 2 + 8);
    bytes.extend_from_slice(production_ranked_value_identity_v1(view).as_bytes());
    bytes.extend_from_slice(evidence.as_bytes());
    bytes.extend_from_slice(&(dimension as u64).to_le_bytes());
    let digest = domain_digest(DYNAMIC_OUTPUT_SYMBOL_V1, &bytes);
    let mut symbol = [0_u8; 4];
    symbol.copy_from_slice(&digest.as_bytes()[..4]);
    u32::from_le_bytes(symbol)
}

fn production_loop_latch_identity_v1(
    terminator: &ProductionRankedTerminatorV1,
) -> Option<DigestV1> {
    let mut bytes = Vec::new();
    match terminator {
        ProductionRankedTerminatorV1::BranchArgsAdd {
            value,
            step,
            target,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&target.to_le_bytes());
            put_value_identity(&mut bytes, *value);
            put_value_identity(&mut bytes, *step);
        }
        ProductionRankedTerminatorV1::BranchArgsAddAt {
            arguments,
            add_argument,
            step,
            target,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&target.to_le_bytes());
            bytes.extend_from_slice(&add_argument.to_le_bytes());
            bytes.extend_from_slice(&(arguments.len() as u64).to_le_bytes());
            for argument in arguments {
                put_value_identity(&mut bytes, *argument);
            }
            put_value_identity(&mut bytes, *step);
        }
        _ => return None,
    }
    Some(domain_digest(LOOP_TRANSITION_DOMAIN_V1, &bytes))
}

/// Identifies one live loop transition inside the exact ranked kernel.
///
/// The latch digest identifies the induction update while the ranked-kernel
/// identity binds every operation and edge in the admitted PLIRON graph. This
/// is structural correspondence, not proof of a selected recurrence.
pub fn production_loop_transition_identity_v1(
    ranked: &ProductionRankedKernelLoweringInputV1,
    header: u32,
    latch: u32,
    exit: u32,
) -> Option<DigestV1> {
    let latch_block = ranked.kernel().blocks().get(latch as usize)?;
    let latch_identity = production_loop_latch_identity_v1(latch_block.terminator())?;
    let mut bytes = Vec::with_capacity(32 * 2 + 12);
    bytes.extend_from_slice(&super::middle_end_evidence_v4::derive_ranked_kernel_identity(ranked));
    bytes.extend_from_slice(latch_identity.as_bytes());
    for block in [header, latch, exit] {
        bytes.extend_from_slice(&block.to_le_bytes());
    }
    Some(domain_digest(LOOP_TRANSITION_DOMAIN_V1, &bytes))
}

#[allow(clippy::too_many_arguments)]
pub fn production_loop_variant_identity_v1(
    header: u32,
    latch: u32,
    exit: u32,
    induction: DigestV1,
    lower_bound: DigestV1,
    upper_bound: DigestV1,
    step: DigestV1,
    transition: DigestV1,
    direction: SemanticLoopDirectionV1,
) -> DigestV1 {
    let mut bytes = Vec::with_capacity(4 * 3 + 32 * 5 + 1);
    for block in [header, latch, exit] {
        bytes.extend_from_slice(&block.to_le_bytes());
    }
    for identity in [induction, lower_bound, upper_bound, step, transition] {
        bytes.extend_from_slice(identity.as_bytes());
    }
    bytes.push(match direction {
        SemanticLoopDirectionV1::Increasing => 1,
        SemanticLoopDirectionV1::Decreasing => 2,
    });
    domain_digest(LOOP_VARIANT_DOMAIN_V1, &bytes)
}

pub(super) struct CanonicalFiniteLoopV1 {
    pub(super) identity: DigestV1,
    pub(super) exit: u32,
    pub(super) iteration_domain: DigestV1,
    pub(super) induction_value: ProductionRankedValueV1,
    pub(super) lower_value: ProductionRankedValueV1,
    pub(super) upper_value: ProductionRankedValueV1,
    pub(super) step_value: ProductionRankedValueV1,
    pub(super) transition: DigestV1,
    pub(super) variant: DigestV1,
    pub(super) direction: SemanticLoopDirectionV1,
    pub(super) maximum_steps: u64,
    pub(super) extent: SemanticFiniteExtentV1,
}

struct LiveLoopDescriptorV1 {
    exit: u32,
    induction_value: ProductionRankedValueV1,
    lower_value: ProductionRankedValueV1,
    upper_value: ProductionRankedValueV1,
    step_value: ProductionRankedValueV1,
    transition: DigestV1,
}

enum LiveLoopDescriptorErrorV1 {
    Structure,
    Unsupported,
}

fn live_loop_descriptor_v1(
    ranked: &ProductionRankedKernelLoweringInputV1,
    header: u32,
    latch: u32,
) -> Result<LiveLoopDescriptorV1, LiveLoopDescriptorErrorV1> {
    let kernel = ranked.kernel();
    let header_block = kernel
        .blocks()
        .get(header as usize)
        .ok_or(LiveLoopDescriptorErrorV1::Structure)?;
    let latch_block = kernel
        .blocks()
        .get(latch as usize)
        .ok_or(LiveLoopDescriptorErrorV1::Structure)?;
    let (induction_value, upper_value, continue_block, exit) = match header_block.terminator() {
        ProductionRankedTerminatorV1::IndexLessThan {
            lhs,
            rhs,
            true_block,
            false_block,
        }
        | ProductionRankedTerminatorV1::IndexLessThanArgs {
            lhs,
            rhs,
            true_block,
            false_block,
            ..
        } => (*lhs, *rhs, *true_block, *false_block),
        _ => return Err(LiveLoopDescriptorErrorV1::Unsupported),
    };
    let ProductionRankedValueV1::BlockArgument {
        block,
        argument: induction_argument,
    } = induction_value
    else {
        return Err(LiveLoopDescriptorErrorV1::Unsupported);
    };
    if block != header
        || !can_reach_without_header(kernel, continue_block, latch, header)
        || can_reach_without_header(kernel, exit, latch, header)
    {
        return Err(LiveLoopDescriptorErrorV1::Structure);
    }
    let preheaders = predecessors(kernel, header)
        .into_iter()
        .filter(|predecessor| *predecessor != latch)
        .collect::<Vec<_>>();
    let [preheader] = preheaders.as_slice() else {
        return Err(LiveLoopDescriptorErrorV1::Unsupported);
    };
    let lower_value = incoming_argument(
        kernel.blocks()[*preheader as usize].terminator(),
        header,
        induction_argument,
    )
    .ok_or(LiveLoopDescriptorErrorV1::Unsupported)?;
    let step_value = loop_step(latch_block.terminator(), header, induction_argument)
        .ok_or(LiveLoopDescriptorErrorV1::Unsupported)?;
    let transition = production_loop_transition_identity_v1(ranked, header, latch, exit)
        .ok_or(LiveLoopDescriptorErrorV1::Unsupported)?;
    Ok(LiveLoopDescriptorV1 {
        exit,
        induction_value,
        lower_value,
        upper_value,
        step_value,
        transition,
    })
}

/// Extracts the finite canonical-loop subset admitted by automatic contract
/// derivation. A dynamic ranked index is bounded by its exact machine type;
/// no narrower caller-provided range is accepted without a retained receipt.
pub(super) fn canonical_finite_loop_v1(
    ranked: &ProductionRankedKernelLoweringInputV1,
    header: u32,
    latch: u32,
) -> Result<CanonicalFiniteLoopV1, &'static str> {
    let kernel = ranked.kernel();
    let live = live_loop_descriptor_v1(ranked, header, latch).map_err(|error| match error {
        LiveLoopDescriptorErrorV1::Structure => "the natural-loop CFG is inconsistent",
        LiveLoopDescriptorErrorV1::Unsupported => {
            "the loop is not a canonical increasing induction loop"
        }
    })?;
    let step = index_constant(kernel, live.step_value)
        .filter(|step| *step != 0)
        .ok_or("the step is dynamic or zero")?;
    let induction = production_ranked_value_identity_v1(live.induction_value);
    let lower_identity = production_ranked_value_identity_v1(live.lower_value);
    let upper_identity = production_ranked_value_identity_v1(live.upper_value);
    let step_identity = production_ranked_value_identity_v1(live.step_value);
    let direction = SemanticLoopDirectionV1::Increasing;
    let variant = production_loop_variant_identity_v1(
        header,
        latch,
        live.exit,
        induction,
        lower_identity,
        upper_identity,
        step_identity,
        live.transition,
        direction,
    );
    let (maximum_steps, extent) = finite_loop_extent_v1(
        ranked,
        header,
        latch,
        live.exit,
        live.lower_value,
        live.upper_value,
        step,
        induction,
        lower_identity,
        upper_identity,
        step_identity,
        live.transition,
        variant,
    )?;
    let mut domain_bytes = Vec::with_capacity(32 * 6 + 8 * 2 + 12);
    domain_bytes
        .extend_from_slice(&super::middle_end_evidence_v4::derive_ranked_kernel_identity(ranked));
    for block in [header, latch, live.exit] {
        domain_bytes.extend_from_slice(&block.to_le_bytes());
    }
    for value in [
        induction,
        lower_identity,
        upper_identity,
        step_identity,
        live.transition,
        variant,
    ] {
        domain_bytes.extend_from_slice(value.as_bytes());
    }
    domain_bytes.extend_from_slice(&maximum_steps.to_le_bytes());
    let iteration_domain = domain_digest(LOOP_ITERATION_DOMAIN_V1, &domain_bytes);
    let mut identity_bytes = domain_bytes;
    identity_bytes.extend_from_slice(iteration_domain.as_bytes());
    let identity = domain_digest(LOOP_CONTRACT_DOMAIN_V1, &identity_bytes);
    Ok(CanonicalFiniteLoopV1 {
        identity,
        exit: live.exit,
        iteration_domain,
        induction_value: live.induction_value,
        lower_value: live.lower_value,
        upper_value: live.upper_value,
        step_value: live.step_value,
        transition: live.transition,
        variant,
        direction,
        maximum_steps,
        extent,
    })
}

#[allow(clippy::too_many_arguments)]
fn finite_loop_extent_v1(
    ranked: &ProductionRankedKernelLoweringInputV1,
    header: u32,
    latch: u32,
    exit: u32,
    lower_value: ProductionRankedValueV1,
    upper_value: ProductionRankedValueV1,
    step: u64,
    induction: DigestV1,
    lower_bound: DigestV1,
    upper_bound: DigestV1,
    step_identity: DigestV1,
    transition: DigestV1,
    variant: DigestV1,
) -> Result<(u64, SemanticFiniteExtentV1), &'static str> {
    match (
        index_constant(ranked.kernel(), lower_value),
        index_constant(ranked.kernel(), upper_value),
    ) {
        (Some(lower), Some(upper)) => {
            let span = upper
                .checked_sub(lower)
                .ok_or("the increasing-loop upper bound is below its lower bound")?;
            let maximum_steps = span.div_ceil(step);
            if maximum_steps == 0 {
                return Err("the iteration domain is empty");
            }
            Ok((maximum_steps, SemanticFiniteExtentV1::Static(maximum_steps)))
        }
        _ if step == 1 => Ok((
            u64::MAX,
            SemanticFiniteExtentV1::Dynamic {
                symbol: production_dynamic_loop_symbol_v1(
                    ranked,
                    header,
                    latch,
                    exit,
                    induction,
                    lower_bound,
                    upper_bound,
                    step_identity,
                    transition,
                    variant,
                ),
                inclusive_upper_bound: u64::MAX,
            },
        )),
        _ => Err(
            "a dynamic finite loop requires a constant unit step until a narrower range receipt is retained",
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn production_dynamic_loop_symbol_v1(
    ranked: &ProductionRankedKernelLoweringInputV1,
    header: u32,
    latch: u32,
    exit: u32,
    induction: DigestV1,
    lower_bound: DigestV1,
    upper_bound: DigestV1,
    step: DigestV1,
    transition: DigestV1,
    variant: DigestV1,
) -> u32 {
    let mut bytes = Vec::with_capacity(32 * 7 + 12);
    bytes.extend_from_slice(&super::middle_end_evidence_v4::derive_ranked_kernel_identity(ranked));
    for block in [header, latch, exit] {
        bytes.extend_from_slice(&block.to_le_bytes());
    }
    for identity in [
        induction,
        lower_bound,
        upper_bound,
        step,
        transition,
        variant,
    ] {
        bytes.extend_from_slice(identity.as_bytes());
    }
    let digest = domain_digest(b"FE2O3/PRODUCTION-DYNAMIC-LOOP-SYMBOL/V1\0", &bytes);
    let mut symbol = [0_u8; 4];
    symbol.copy_from_slice(&digest.as_bytes()[..4]);
    u32::from_le_bytes(symbol)
}

fn require_live_loop_contract(
    ranked: &ProductionRankedKernelLoweringInputV1,
    contract: &MirPlironSemanticContractV1,
    declared: &SemanticLoopContractV1,
) -> Result<(), ProductionMirPlironSemanticContractErrorV1> {
    let kernel = ranked.kernel();
    let live =
        match live_loop_descriptor_v1(ranked, declared.header_block(), declared.latch_block()) {
            Ok(live) => live,
            Err(LiveLoopDescriptorErrorV1::Structure) => {
                return Err(loop_structure_error(declared));
            }
            Err(LiveLoopDescriptorErrorV1::Unsupported) => {
                return Err(
                    ProductionMirPlironSemanticContractErrorV1::UnsupportedLoopShape {
                        header: declared.header_block(),
                        latch: declared.latch_block(),
                    },
                );
            }
        };
    if live.exit != declared.exit_block() {
        return Err(loop_structure_error(declared));
    }
    let induction = production_ranked_value_identity_v1(live.induction_value);
    let lower_bound = production_ranked_value_identity_v1(live.lower_value);
    let upper_bound_identity = production_ranked_value_identity_v1(live.upper_value);
    let step_identity = production_ranked_value_identity_v1(live.step_value);
    let variant = production_loop_variant_identity_v1(
        declared.header_block(),
        declared.latch_block(),
        declared.exit_block(),
        induction,
        lower_bound,
        upper_bound_identity,
        step_identity,
        live.transition,
        declared.direction(),
    );
    let step = index_constant(kernel, live.step_value)
        .filter(|step| *step != 0)
        .ok_or(
            ProductionMirPlironSemanticContractErrorV1::DynamicLoopStepUnproved {
                header: declared.header_block(),
                latch: declared.latch_block(),
            },
        )?;
    let expected_extent = finite_loop_extent_v1(
        ranked,
        declared.header_block(),
        declared.latch_block(),
        declared.exit_block(),
        live.lower_value,
        live.upper_value,
        step,
        induction,
        lower_bound,
        upper_bound_identity,
        step_identity,
        live.transition,
        variant,
    )
    .map_err(
        |_| ProductionMirPlironSemanticContractErrorV1::DynamicLoopStepUnproved {
            header: declared.header_block(),
            latch: declared.latch_block(),
        },
    )?;
    let Some(domain) = contract
        .domains()
        .iter()
        .find(|domain| domain.identity() == declared.iteration_domain())
    else {
        return Err(
            ProductionMirPlironSemanticContractErrorV1::LoopValueMismatch {
                header: declared.header_block(),
                latch: declared.latch_block(),
            },
        );
    };
    if let SemanticFiniteExtentV1::Dynamic {
        inclusive_upper_bound,
        ..
    } = domain.extents()[0]
    {
        if inclusive_upper_bound != u64::MAX {
            return Err(
                ProductionMirPlironSemanticContractErrorV1::DynamicLoopBoundUnproved {
                    header: declared.header_block(),
                    latch: declared.latch_block(),
                    inclusive_upper_bound,
                },
            );
        }
        if index_constant(kernel, live.step_value) != Some(1) {
            return Err(
                ProductionMirPlironSemanticContractErrorV1::DynamicLoopStepUnproved {
                    header: declared.header_block(),
                    latch: declared.latch_block(),
                },
            );
        }
    }
    let extent_matches = domain.extents().first().copied() == Some(expected_extent.1);
    if domain.extents().len() != 1
        || domain.maximum_cardinality() != Some(declared.maximum_steps())
        || declared.maximum_steps() != expected_extent.0
        || declared.direction() != SemanticLoopDirectionV1::Increasing
        || !extent_matches
        || declared.induction() != induction
        || declared.lower_bound() != lower_bound
        || declared.upper_bound() != upper_bound_identity
        || declared.step() != step_identity
        || declared.transition() != live.transition
        || declared.variant() != variant
    {
        return Err(
            ProductionMirPlironSemanticContractErrorV1::LoopValueMismatch {
                header: declared.header_block(),
                latch: declared.latch_block(),
            },
        );
    }
    Ok(())
}

fn loop_structure_error(
    declared: &SemanticLoopContractV1,
) -> ProductionMirPlironSemanticContractErrorV1 {
    ProductionMirPlironSemanticContractErrorV1::LoopStructureMismatch {
        header: declared.header_block(),
        latch: declared.latch_block(),
        exit: declared.exit_block(),
    }
}

fn predecessors(kernel: &super::ProductionRankedKernelV1, target: u32) -> Vec<u32> {
    kernel
        .blocks()
        .iter()
        .enumerate()
        .filter_map(|(source, block)| {
            successors(block.terminator())
                .contains(&target)
                .then_some(source as u32)
        })
        .collect()
}

fn incoming_argument(
    terminator: &ProductionRankedTerminatorV1,
    target: u32,
    argument: u32,
) -> Option<ProductionRankedValueV1> {
    let argument = usize::try_from(argument).ok()?;
    let arguments = match terminator {
        ProductionRankedTerminatorV1::BranchArgs {
            arguments,
            target: destination,
        } if *destination == target => arguments,
        ProductionRankedTerminatorV1::IndexLessThanArgs {
            true_arguments,
            false_arguments,
            true_block,
            false_block,
            ..
        }
        | ProductionRankedTerminatorV1::IndexEqualArgs {
            true_arguments,
            false_arguments,
            true_block,
            false_block,
            ..
        } => match (*true_block == target, *false_block == target) {
            (true, false) => true_arguments,
            (false, true) => false_arguments,
            _ => return None,
        },
        ProductionRankedTerminatorV1::AnalysisSplitArgs {
            first_arguments,
            second_arguments,
            first_block,
            second_block,
            ..
        } => match (*first_block == target, *second_block == target) {
            (true, false) => first_arguments,
            (false, true) => second_arguments,
            _ => return None,
        },
        _ => return None,
    };
    arguments.get(argument).copied()
}

fn loop_step(
    terminator: &ProductionRankedTerminatorV1,
    header: u32,
    induction_argument: u32,
) -> Option<ProductionRankedValueV1> {
    let step = match terminator {
        ProductionRankedTerminatorV1::BranchArgsAdd { step, target, .. }
            if *target == header && induction_argument == 0 =>
        {
            *step
        }
        ProductionRankedTerminatorV1::BranchArgsAddAt {
            add_argument,
            step,
            target,
            ..
        } if *target == header && *add_argument == induction_argument => *step,
        _ => return None,
    };
    Some(step)
}

fn can_reach_without_header(
    kernel: &super::ProductionRankedKernelV1,
    start: u32,
    target: u32,
    header: u32,
) -> bool {
    let mut pending = vec![start];
    let mut visited = BTreeSet::new();
    while let Some(block) = pending.pop() {
        if block == target {
            return true;
        }
        if block == header || !visited.insert(block) {
            continue;
        }
        let Some(block) = kernel.blocks().get(block as usize) else {
            continue;
        };
        pending.extend(successors(block.terminator()));
    }
    false
}

fn index_constant(
    kernel: &super::ProductionRankedKernelV1,
    value: ProductionRankedValueV1,
) -> Option<u64> {
    let ProductionRankedValueV1::Local(identity) = value else {
        return None;
    };
    kernel
        .blocks()
        .iter()
        .flat_map(|block| block.operations())
        .find_map(|operation| match operation {
            ProductionRankedOperationV1::IndexConstant { result, value } if *result == identity => {
                Some(*value)
            }
            _ => None,
        })
}

fn put_value_identity(bytes: &mut Vec<u8>, value: ProductionRankedValueV1) {
    bytes.extend_from_slice(production_ranked_value_identity_v1(value).as_bytes());
}

#[derive(Clone, Copy)]
pub(super) struct LiveTypedRootV1 {
    pub(super) value: ProductionRankedValueV1,
    pub(super) commitment: DigestV1,
    pub(super) scalar: SemanticScalarTypeV1,
    pub(super) numerical_policy: SemanticNumericalPolicyV1,
}

pub(super) fn live_typed_roots(
    ranked: &ProductionRankedKernelLoweringInputV1,
) -> Vec<LiveTypedRootV1> {
    ranked
        .kernel()
        .blocks()
        .iter()
        .flat_map(|block| block.operations())
        .filter_map(|operation| match operation {
            ProductionRankedOperationV1::SemanticExpression {
                result,
                expression,
                numerical_contract,
            } => numerical_policy(*numerical_contract).map(|numerical_policy| LiveTypedRootV1 {
                value: ProductionRankedValueV1::Local(*result),
                commitment: DigestV1::from_untrusted_bytes(
                    expression.canonical_transcript_sha256(*numerical_contract),
                ),
                scalar: scalar(expression.scalar()),
                numerical_policy,
            }),
            _ => None,
        })
        .collect()
}

fn live_typed_values(
    ranked: &ProductionRankedKernelLoweringInputV1,
) -> Vec<(ProductionRankedValueV1, DigestV1)> {
    live_typed_roots(ranked)
        .into_iter()
        .map(|root| (root.value, root.commitment))
        .collect()
}

fn typed_value_identity(
    values: &[(ProductionRankedValueV1, DigestV1)],
    value: ProductionRankedValueV1,
) -> Option<DigestV1> {
    values
        .iter()
        .any(|(candidate, _)| *candidate == value)
        .then(|| production_ranked_value_identity_v1(value))
}

fn live_view_shape(
    ranked: &ProductionRankedKernelLoweringInputV1,
    value: ProductionRankedValueV1,
) -> Option<&[u64]> {
    let ProductionRankedValueV1::Local(identity) = value else {
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
                if *result == identity =>
            {
                Some(shape.as_slice())
            }
            _ => None,
        })
}

fn shape_matches(
    extents: &[SemanticFiniteExtentV1],
    shape: Option<&[u64]>,
    view: ProductionRankedValueV1,
    evidence: DigestV1,
) -> bool {
    let Some(shape) = shape else { return false };
    shape.len() == extents.len()
        && shape
            .iter()
            .zip(extents)
            .enumerate()
            .all(|(dimension, (live, declared))| match declared {
                SemanticFiniteExtentV1::Static(extent) => live == extent,
                SemanticFiniteExtentV1::Dynamic {
                    symbol,
                    inclusive_upper_bound,
                } => {
                    *live == DYNAMIC_EXTENT
                        && *inclusive_upper_bound == u64::MAX
                        && *symbol == production_dynamic_output_symbol_v1(view, evidence, dimension)
                }
            })
}

pub(super) fn natural_backedges(kernel: &super::ProductionRankedKernelV1) -> Vec<(u32, u32)> {
    let block_count = kernel.blocks().len();
    let successors = kernel
        .blocks()
        .iter()
        .map(|block| successors(block.terminator()))
        .collect::<Vec<_>>();
    let mut predecessors = vec![Vec::new(); block_count];
    for (source, targets) in successors.iter().enumerate() {
        for target in targets {
            if let Some(slot) = predecessors.get_mut(*target as usize) {
                slot.push(source);
            }
        }
    }
    let mut reachable = BTreeSet::new();
    let mut pending = vec![0_usize];
    while let Some(block) = pending.pop() {
        if block >= block_count || !reachable.insert(block) {
            continue;
        }
        pending.extend(successors[block].iter().map(|target| *target as usize));
    }
    let mut dominators = vec![BTreeSet::new(); block_count];
    for block in &reachable {
        dominators[*block] = reachable.clone();
    }
    dominators[0] = [0].into_iter().collect();
    loop {
        let mut changed = false;
        for block in 1..block_count {
            if !reachable.contains(&block) {
                continue;
            }
            let mut next = predecessors[block]
                .iter()
                .filter(|predecessor| reachable.contains(predecessor))
                .map(|predecessor| dominators[*predecessor].clone())
                .reduce(|left, right| left.intersection(&right).copied().collect())
                .unwrap_or_default();
            next.insert(block);
            if next != dominators[block] {
                dominators[block] = next;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    successors
        .iter()
        .enumerate()
        .filter(|(source, _)| reachable.contains(source))
        .flat_map(|(source, targets)| {
            let dominators = &dominators;
            targets.iter().filter_map(move |target| {
                dominators[source]
                    .contains(&(*target as usize))
                    .then_some((source as u32, *target))
            })
        })
        .collect()
}

fn successors(terminator: &ProductionRankedTerminatorV1) -> Vec<u32> {
    match terminator {
        ProductionRankedTerminatorV1::IndexLessThan {
            true_block,
            false_block,
            ..
        }
        | ProductionRankedTerminatorV1::IndexLessThanArgs {
            true_block,
            false_block,
            ..
        }
        | ProductionRankedTerminatorV1::IndexEqual {
            true_block,
            false_block,
            ..
        }
        | ProductionRankedTerminatorV1::IndexEqualArgs {
            true_block,
            false_block,
            ..
        } => {
            vec![*true_block, *false_block]
        }
        ProductionRankedTerminatorV1::AnalysisSplit {
            first_block,
            second_block,
            ..
        }
        | ProductionRankedTerminatorV1::AnalysisSplitArgs {
            first_block,
            second_block,
            ..
        } => {
            vec![*first_block, *second_block]
        }
        ProductionRankedTerminatorV1::Branch { target }
        | ProductionRankedTerminatorV1::BranchArgs { target, .. }
        | ProductionRankedTerminatorV1::BranchArgsAdd { target, .. }
        | ProductionRankedTerminatorV1::BranchArgsAddAt { target, .. } => vec![*target],
        ProductionRankedTerminatorV1::Return | ProductionRankedTerminatorV1::Trap => Vec::new(),
    }
}

pub(super) fn collective_kind(
    kind: ProductionCollectiveSemanticKindV1,
) -> SemanticCollectiveKindV1 {
    match kind {
        ProductionCollectiveSemanticKindV1::FiniteFold => SemanticCollectiveKindV1::FiniteFold,
        ProductionCollectiveSemanticKindV1::FiniteRecurrence => {
            SemanticCollectiveKindV1::FiniteRecurrence
        }
        ProductionCollectiveSemanticKindV1::PermutationGather => {
            SemanticCollectiveKindV1::PermutationGather
        }
    }
}

pub(super) fn evaluation_order(order: SemanticEvaluationOrderAttr) -> SemanticEvaluationOrderV1 {
    match order {
        SemanticEvaluationOrderAttr::Ascending => SemanticEvaluationOrderV1::SequentialAscending,
        SemanticEvaluationOrderAttr::Descending => SemanticEvaluationOrderV1::SequentialDescending,
        SemanticEvaluationOrderAttr::Lexicographic => SemanticEvaluationOrderV1::Lexicographic,
        SemanticEvaluationOrderAttr::Explicit => SemanticEvaluationOrderV1::ExplicitTree,
    }
}

pub(super) fn coverage(coverage: SemanticCoverageBindingAttr) -> SemanticCoverageBindingV1 {
    match coverage {
        SemanticCoverageBindingAttr::TotalView => SemanticCoverageBindingV1::TotalView,
        SemanticCoverageBindingAttr::CollectiveContributions => {
            SemanticCoverageBindingV1::CollectiveContributions
        }
    }
}

fn scalar(scalar: ProductionSemanticScalarTypeV2) -> SemanticScalarTypeV1 {
    match scalar {
        ProductionSemanticScalarTypeV2::Bool => SemanticScalarTypeV1::Boolean,
        ProductionSemanticScalarTypeV2::Integer { signed: true, bits } => {
            SemanticScalarTypeV1::Signed(bits)
        }
        ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits,
        } => SemanticScalarTypeV1::Unsigned(bits),
        ProductionSemanticScalarTypeV2::Float { bits } => SemanticScalarTypeV1::Float(bits),
    }
}

fn numerical_policy(policy: ProductionNumericalContractV2) -> Option<SemanticNumericalPolicyV1> {
    match policy {
        ProductionNumericalContractV2::ExactBitVectorOperatorCongruence => {
            Some(SemanticNumericalPolicyV1::ExactBitVector)
        }
        ProductionNumericalContractV2::ExactIeee754OperatorCongruence {
            rounding,
            exceptional_values,
        } => Some(SemanticNumericalPolicyV1::IeeeOperatorCongruence {
            rounding: match rounding {
                ProductionIeeeRoundingModeV2::NearestTiesToEven => {
                    SemanticIeeeRoundingV1::NearestTiesEven
                }
                ProductionIeeeRoundingModeV2::TowardZero => SemanticIeeeRoundingV1::TowardZero,
                ProductionIeeeRoundingModeV2::TowardPositive => {
                    SemanticIeeeRoundingV1::TowardPositive
                }
                ProductionIeeeRoundingModeV2::TowardNegative => {
                    SemanticIeeeRoundingV1::TowardNegative
                }
            },
            exceptional_values: match exceptional_values {
                ProductionIeeeExceptionalValuePolicyV2::PreserveExactBits => {
                    SemanticIeeeExceptionalValueV1::ExactBits
                }
                ProductionIeeeExceptionalValuePolicyV2::CanonicalNan => {
                    SemanticIeeeExceptionalValueV1::CanonicalNan
                }
            },
        }),
        ProductionNumericalContractV2::Relaxed
        | ProductionNumericalContractV2::ErrorBounded { .. } => None,
    }
}

fn words_digest(words: [u64; 4]) -> DigestV1 {
    let mut bytes = [0_u8; 32];
    for (index, word) in words.into_iter().enumerate() {
        bytes[index * 8..(index + 1) * 8].copy_from_slice(&word.to_le_bytes());
    }
    DigestV1::from_untrusted_bytes(bytes)
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> DigestV1 {
    let mut digest = Sha256::new();
    digest.update((domain.len() as u64).to_le_bytes());
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    DigestV1::from_untrusted_bytes(digest.finalize().into())
}

fn count(value: usize) -> Result<u64, ProductionMirPlironSemanticContractErrorV1> {
    u64::try_from(value).map_err(|_| ProductionMirPlironSemanticContractErrorV1::CounterOverflow)
}
