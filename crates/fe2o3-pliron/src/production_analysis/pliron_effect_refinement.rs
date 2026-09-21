//! Functional effect refinement between sequential references and GPU writes.
//!
//! The checker is workload neutral. It correlates inert proof contracts with
//! real guarded writes, consumes hierarchy ownership, and compares canonical
//! domain, precondition, and value expressions for every logical output.

use std::{
    collections::{HashMap, HashSet},
    fmt::{self, Write as _},
};

use dialect_kernel::{
    DYNAMIC_EXTENT, MAX_SEMANTIC_TYPED_EXPRESSION_NODES_V1, MemorySpaceAttr, OwnershipContractOp,
    RankedAccessOp, RankedViewOp, ranked_view_type,
};
use dialect_proof::{
    CoveredBoundaryAttr, EvidenceRefOp, EvidenceStatusAttr, ObligationOp, PropertyAttr,
    RequireEffectRefinementOp,
};
use pliron::{
    builtin::ops::FuncOp, common_traits::Named, context::Context, operation::Operation,
    value::Value,
};

use crate::KernelCheckStatusV1;
use crate::production_analysis::pliron_analysis_manager::PlironAnalysisManagerV1;
use crate::production_analysis::pliron_hierarchical_ownership::{
    HierarchicalOwnershipFindingV1, run_pliron_hierarchical_ownership_with_observation_v1,
};
use crate::production_analysis::pliron_invocation_trace::PlironTraceLocationV1;
use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::InvocationObserverV1;
use crate::production_analysis::pliron_resource_envelope::{
    ProductionAnalysisInputCensusV1, ProductionAnalysisResourceLimitV1,
    ProductionAnalysisResourceLimitsV1, ProductionAnalysisResourcePhaseV1,
    ProductionAnalysisResourceUpperBoundV1,
};
use crate::production_analysis::pliron_semantic_refinement::MAX_PLIRON_SEMANTIC_DIAGNOSTIC_BYTES_V1;
use crate::production_analysis::pliron_semantic_refinement::SemanticExpressionTableV1;

pub const MAX_EFFECT_REFINEMENT_CONTRACTS_V1: usize = 4_096;

// A mismatch retains a view name and two independently rendered semantic
// descriptions. Other finding variants retain no more dynamic text.
const EFFECT_FINDING_NAME_SLOTS_V1: usize = 1;
const EFFECT_FINDING_DESCRIPTION_SLOTS_V1: usize = 2;
const EFFECT_FINDING_FIXED_STORAGE_V1: usize = 32;
const EFFECT_FINDING_WITNESS_STORAGE_V1: usize = dialect_kernel::MAX_RANKED_MEMORY_RANK * 2;

fn effect_finding_dynamic_text_storage_v1() -> Result<usize, ProductionAnalysisResourceLimitV1> {
    checked_effect_product_v1(
        EFFECT_FINDING_NAME_SLOTS_V1
            .checked_add(EFFECT_FINDING_DESCRIPTION_SLOTS_V1)
            .ok_or_else(effect_resource_overflow_v1)?,
        MAX_PLIRON_SEMANTIC_DIAGNOSTIC_BYTES_V1,
    )
}

fn effect_finding_retained_storage_v1() -> Result<usize, ProductionAnalysisResourceLimitV1> {
    checked_effect_sum_v1(&[
        effect_finding_dynamic_text_storage_v1()?,
        EFFECT_FINDING_FIXED_STORAGE_V1,
        EFFECT_FINDING_WITNESS_STORAGE_V1,
    ])
}

struct BoundedEffectDiagnosticWriterV1 {
    text: String,
    truncated: bool,
}

impl fmt::Write for BoundedEffectDiagnosticWriterV1 {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self.truncated {
            return Err(fmt::Error);
        }
        let payload_limit = MAX_PLIRON_SEMANTIC_DIAGNOSTIC_BYTES_V1 - 3;
        let remaining = payload_limit.saturating_sub(self.text.len());
        if value.len() <= remaining {
            self.text.push_str(value);
            return Ok(());
        }
        let mut end = remaining;
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        self.text.push_str(&value[..end]);
        self.truncated = true;
        Err(fmt::Error)
    }
}

fn bounded_effect_diagnostic_v1(value: impl fmt::Display) -> String {
    let mut writer = BoundedEffectDiagnosticWriterV1 {
        text: String::with_capacity(MAX_PLIRON_SEMANTIC_DIAGNOSTIC_BYTES_V1),
        truncated: false,
    };
    let _ = write!(&mut writer, "{value}");
    if writer.truncated {
        writer.text.push_str("...");
    }
    writer.text
}

fn bounded_effect_owned_diagnostic_v1(mut value: String) -> String {
    if value.len() <= MAX_PLIRON_SEMANTIC_DIAGNOSTIC_BYTES_V1 {
        return value;
    }
    let mut end = MAX_PLIRON_SEMANTIC_DIAGNOSTIC_BYTES_V1 - 3;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value.truncate(end);
    value.push_str("...");
    value
}

fn effect_resource_overflow_v1() -> ProductionAnalysisResourceLimitV1 {
    ProductionAnalysisResourceLimitV1 {
        phase: ProductionAnalysisResourcePhaseV1::EffectRefinement,
        resource: "effect refinement resource upper bound",
    }
}

fn checked_effect_sum_v1(values: &[usize]) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    values.iter().try_fold(0_usize, |total, value| {
        total
            .checked_add(*value)
            .ok_or_else(effect_resource_overflow_v1)
    })
}

fn checked_effect_product_v1(
    lhs: usize,
    rhs: usize,
) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    lhs.checked_mul(rhs).ok_or_else(effect_resource_overflow_v1)
}

pub(crate) fn is_effect_refinement_contract_v1(operation: &dyn pliron::op::Op) -> bool {
    operation
        .downcast_ref::<RequireEffectRefinementOp>()
        .is_some()
}

/// Bounds proof-contract correlation and the independently rebuilt canonical
/// semantic-expression table. Nested ownership verification is separately
/// charged at its own call boundary.
pub(crate) fn preflight_effect_refinement_resource_upper_bound_v1(
    census: ProductionAnalysisInputCensusV1,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    let phase = ProductionAnalysisResourcePhaseV1::EffectRefinement;
    let contracts = census.effect_refinement_contracts;
    if contracts == 0 {
        // The runtime performs one allocation-free inventory scan before
        // returning the empty report. No expression or correlation state is
        // constructed when the authenticated census contains no contract.
        let bound =
            ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, census.operations, 0, 0)?;
        return limits.require(phase, bound);
    }
    let nodes = census.operations.min(crate::MAX_PLIRON_SEMANTIC_NODES_V1);
    let semantic_node_cost = MAX_SEMANTIC_TYPED_EXPRESSION_NODES_V1
        .checked_mul(2)
        .and_then(|items| items.checked_add(16))
        .ok_or_else(effect_resource_overflow_v1)?;
    let semantic_work = checked_effect_product_v1(
        checked_effect_product_v1(
            nodes,
            nodes
                .checked_add(1)
                .ok_or_else(effect_resource_overflow_v1)?,
        )?,
        semantic_node_cost,
    )?;
    let correlation_work = checked_effect_product_v1(
        census.operations,
        contracts
            .checked_mul(8)
            .and_then(|n| n.checked_add(4))
            .ok_or_else(effect_resource_overflow_v1)?,
    )?;
    let findings = if contracts == 0 {
        0
    } else {
        census.operations.max(
            contracts
                .checked_mul(dialect_kernel::MAX_RANKED_MEMORY_RANK + 3)
                .ok_or_else(effect_resource_overflow_v1)?,
        )
    };
    let per_finding_storage = effect_finding_retained_storage_v1()?;
    let retained = checked_effect_product_v1(findings, per_finding_storage)?;
    let work = checked_effect_sum_v1(&[
        // One allocation-free scan selects the empty-contract fast path before
        // `collect` performs the independently bounded nonempty analysis.
        census.operations,
        checked_effect_product_v1(census.operations, 8)?,
        semantic_work,
        correlation_work,
        // One bounded render/copy for every retained text byte and fixed field.
        retained,
        // Each operation can originate one independently rendered view/value name.
        checked_effect_product_v1(census.operations, MAX_PLIRON_SEMANTIC_DIAGNOSTIC_BYTES_V1)?,
    ])?;
    let semantic_storage = checked_effect_product_v1(nodes, semantic_node_cost)?;
    let temporary = checked_effect_sum_v1(&[
        semantic_storage,
        checked_effect_product_v1(
            census.operations,
            census
                .max_operation_arity
                .checked_mul(3)
                .and_then(|items| items.checked_add(32))
                .ok_or_else(effect_resource_overflow_v1)?,
        )?,
        checked_effect_product_v1(contracts, census.operations)?,
        // Contract view names coexist with retained findings until the report returns.
        checked_effect_product_v1(contracts, MAX_PLIRON_SEMANTIC_DIAGNOSTIC_BYTES_V1)?,
    ])?;
    let bound =
        ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, work, retained, temporary)?;
    limits.require(phase, bound)
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct EffectRefinementLocationV1 {
    block: usize,
    operation: usize,
}

impl EffectRefinementLocationV1 {
    pub const fn block(self) -> usize {
        self.block
    }
    pub const fn operation(self) -> usize {
        self.operation
    }
}

impl From<PlironTraceLocationV1> for EffectRefinementLocationV1 {
    fn from(value: PlironTraceLocationV1) -> Self {
        Self {
            block: value.block,
            operation: value.operation,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectRefinementWitnessV1 {
    coordinate: Vec<u64>,
    invocation: Vec<u64>,
    workgroup: u64,
    subgroup: u64,
    lane: u64,
    location: EffectRefinementLocationV1,
}

impl EffectRefinementWitnessV1 {
    pub fn coordinate(&self) -> &[u64] {
        &self.coordinate
    }
    pub fn invocation(&self) -> &[u64] {
        &self.invocation
    }
    pub const fn workgroup(&self) -> u64 {
        self.workgroup
    }
    pub const fn subgroup(&self) -> u64 {
        self.subgroup
    }
    pub const fn lane(&self) -> u64 {
        self.lane
    }
    pub const fn location(&self) -> EffectRefinementLocationV1 {
        self.location
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlironEffectRefinementFindingV1 {
    ResourceLimitExceeded {
        actual: usize,
        limit: usize,
    },
    MissingOwnershipContract {
        view: String,
        location: EffectRefinementLocationV1,
    },
    DynamicOwnershipIncomplete {
        view: String,
        dimension: Option<usize>,
        detail: String,
    },
    OwnershipIncomplete {
        detail: String,
    },
    OwnershipRejected {
        detail: String,
    },
    OrphanEffectContract {
        view: String,
        location: EffectRefinementLocationV1,
    },
    AmbiguousWriteSite {
        view: String,
        location: EffectRefinementLocationV1,
        matches: usize,
    },
    DuplicateEffectContract {
        view: String,
        write: EffectRefinementLocationV1,
        first: EffectRefinementLocationV1,
        second: EffectRefinementLocationV1,
    },
    UnmodeledWrite {
        view: String,
        witness: EffectRefinementWitnessV1,
    },
    UnmodeledWriteSite {
        view: String,
        location: EffectRefinementLocationV1,
    },
    ReferenceProofIncomplete {
        obligation: [u64; 4],
        location: EffectRefinementLocationV1,
        reason: &'static str,
    },
    ReferenceProofRejected {
        obligation: [u64; 4],
        location: EffectRefinementLocationV1,
        reason: &'static str,
    },
    UnresolvedExpression {
        view: String,
        location: EffectRefinementLocationV1,
        component: &'static str,
        value: String,
    },
    DomainMismatch {
        view: String,
        location: EffectRefinementLocationV1,
        actual: String,
        expected: String,
        witness: Option<EffectRefinementWitnessV1>,
    },
    PreconditionMismatch {
        view: String,
        location: EffectRefinementLocationV1,
        actual: String,
        expected: String,
        witness: Option<EffectRefinementWitnessV1>,
    },
    ValueMismatch {
        view: String,
        location: EffectRefinementLocationV1,
        actual: String,
        expected: String,
        witness: Option<EffectRefinementWitnessV1>,
    },
    TraceIncomplete {
        detail: String,
    },
}

impl PlironEffectRefinementFindingV1 {
    pub const fn status(&self) -> KernelCheckStatusV1 {
        match self {
            Self::OrphanEffectContract { .. }
            | Self::AmbiguousWriteSite { .. }
            | Self::DuplicateEffectContract { .. }
            | Self::ReferenceProofRejected { .. }
            | Self::DomainMismatch { .. }
            | Self::PreconditionMismatch { .. }
            | Self::ValueMismatch { .. }
            | Self::UnmodeledWriteSite { .. }
            | Self::OwnershipRejected { .. } => KernelCheckStatusV1::Rejected,
            Self::ResourceLimitExceeded { .. }
            | Self::MissingOwnershipContract { .. }
            | Self::DynamicOwnershipIncomplete { .. }
            | Self::OwnershipIncomplete { .. }
            | Self::UnmodeledWrite { .. }
            | Self::ReferenceProofIncomplete { .. }
            | Self::UnresolvedExpression { .. }
            | Self::TraceIncomplete { .. } => KernelCheckStatusV1::Incomplete,
        }
    }
}

impl fmt::Display for PlironEffectRefinementFindingV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResourceLimitExceeded { actual, limit } => write!(
                f,
                "error[FE2O3-EFFECT-009]: function has {actual} effect-refinement contracts, exceeding limit {limit}"
            ),
            Self::MissingOwnershipContract { view, location } => write!(
                f,
                "error[FE2O3-EFFECT-002]: effect refinement for {view} at block {} op {} is incomplete because the view has no exact hierarchy ownership contract",
                location.block, location.operation
            ),
            Self::DynamicOwnershipIncomplete {
                view,
                dimension,
                detail,
            } => write!(
                f,
                "error[FE2O3-EFFECT-003]: dynamic ownership of {view}{} is incomplete: {detail}",
                dimension
                    .map(|d| format!(" dimension {d}"))
                    .unwrap_or_default()
            ),
            Self::OwnershipIncomplete { detail } => write!(
                f,
                "error[FE2O3-EFFECT-003]: effect refinement is incomplete because hierarchy ownership is unproved: {detail}"
            ),
            Self::OwnershipRejected { detail } => write!(
                f,
                "error[FE2O3-EFFECT-004]: effect refinement rejected because hierarchy ownership is invalid: {detail}"
            ),
            Self::OrphanEffectContract { view, location } => write!(
                f,
                "error[FE2O3-EFFECT-005]: effect contract for {view} at block {} op {} does not identify a real write with the same view and indices",
                location.block, location.operation
            ),
            Self::AmbiguousWriteSite {
                view,
                location,
                matches,
            } => write!(
                f,
                "error[FE2O3-EFFECT-005]: effect contract for {view} at block {} op {} matches {matches} writes; one exact write site is required",
                location.block, location.operation
            ),
            Self::DuplicateEffectContract {
                view,
                write,
                first,
                second,
            } => write!(
                f,
                "error[FE2O3-EFFECT-005]: {view} write at block {} op {} has duplicate effect contracts at block {} op {} and block {} op {}",
                write.block,
                write.operation,
                first.block,
                first.operation,
                second.block,
                second.operation
            ),
            Self::UnmodeledWrite { view, witness } => write!(
                f,
                "error[FE2O3-EFFECT-006]: logical write {view}{:?} is not modeled by the sequential reference; invocation {:?} (workgroup {}, subgroup {}, lane {}) writes at block {} op {}",
                witness.coordinate,
                witness.invocation,
                witness.workgroup,
                witness.subgroup,
                witness.lane,
                witness.location.block,
                witness.location.operation
            ),
            Self::UnmodeledWriteSite { view, location } => write!(
                f,
                "error[FE2O3-EFFECT-008]: global write to {view} at block {} op {} has no exact effect-refinement contract; every observable global write must participate in the reference-effect bijection",
                location.block, location.operation,
            ),
            Self::ReferenceProofIncomplete {
                obligation,
                location,
                reason,
            } => write!(
                f,
                "error[FE2O3-EFFECT-007]: MIR effect proof {} is incomplete at block {} op {}: {reason}",
                proof_identity(*obligation),
                location.block,
                location.operation
            ),
            Self::ReferenceProofRejected {
                obligation,
                location,
                reason,
            } => write!(
                f,
                "error[FE2O3-EFFECT-007]: MIR effect proof {} is invalid at block {} op {}: {reason}",
                proof_identity(*obligation),
                location.block,
                location.operation
            ),
            Self::UnresolvedExpression {
                view,
                location,
                component,
                value,
            } => write!(
                f,
                "error[FE2O3-EFFECT-008]: cannot normalize {component} expression {value} for {view} at block {} op {}",
                location.block, location.operation
            ),
            Self::DomainMismatch {
                view,
                location,
                actual,
                expected,
                witness,
            } => mismatch(
                f,
                "domain",
                view,
                *location,
                actual,
                expected,
                witness.as_ref(),
            ),
            Self::PreconditionMismatch {
                view,
                location,
                actual,
                expected,
                witness,
            } => mismatch(
                f,
                "precondition",
                view,
                *location,
                actual,
                expected,
                witness.as_ref(),
            ),
            Self::ValueMismatch {
                view,
                location,
                actual,
                expected,
                witness,
            } => mismatch(
                f,
                "value",
                view,
                *location,
                actual,
                expected,
                witness.as_ref(),
            ),
            Self::TraceIncomplete { detail } => write!(
                f,
                "error[FE2O3-EFFECT-003]: guarded GPU effect tracing is incomplete: {detail}"
            ),
        }
    }
}

fn mismatch(
    f: &mut fmt::Formatter<'_>,
    component: &str,
    view: &str,
    location: EffectRefinementLocationV1,
    actual: &str,
    expected: &str,
    witness: Option<&EffectRefinementWitnessV1>,
) -> fmt::Result {
    write!(
        f,
        "error[FE2O3-EFFECT-001]: {component} mismatch for {view} at block {} op {}; GPU `{actual}` is not equivalent to sequential reference `{expected}`",
        location.block, location.operation
    )?;
    if let Some(witness) = witness {
        write!(
            f,
            "; counterexample coordinate {:?}, invocation {:?} (workgroup {}, subgroup {}, lane {})",
            witness.coordinate,
            witness.invocation,
            witness.workgroup,
            witness.subgroup,
            witness.lane
        )?;
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironEffectRefinementReportV1 {
    findings: Vec<PlironEffectRefinementFindingV1>,
    contracts: usize,
    proved_contracts: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironEffectRefinementCheckErrorV1 {
    report: PlironEffectRefinementReportV1,
}

impl PlironEffectRefinementCheckErrorV1 {
    pub const fn report(&self) -> &PlironEffectRefinementReportV1 {
        &self.report
    }
}

impl fmt::Display for PlironEffectRefinementCheckErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, finding) in self.report.findings.iter().enumerate() {
            if index != 0 {
                formatter.write_str("\n")?;
            }
            finding.fmt(formatter)?;
        }
        Ok(())
    }
}

impl std::error::Error for PlironEffectRefinementCheckErrorV1 {}

impl PlironEffectRefinementReportV1 {
    #[cfg(test)]
    pub(super) fn set_validation_findings_for_test_v1(&mut self, nonempty: bool) {
        self.findings.reserve_exact(1);
        if nonempty {
            self.findings
                .push(PlironEffectRefinementFindingV1::ResourceLimitExceeded {
                    actual: 2,
                    limit: 1,
                });
        }
    }

    pub(super) fn has_empty_validation_findings_v1(&self) -> bool {
        self.findings.is_empty() && self.findings.capacity() == 0
    }

    pub(super) fn try_clone_validation_payload_v1(
        &self,
    ) -> Result<Self, super::pliron_resource_envelope::ProductionAnalysisResourceLimitV1> {
        if !self.has_empty_validation_findings_v1() {
            return Err(super::pliron_report_payload_receipt::payload_limit_v1(
                "report payload shape changed",
            ));
        }
        Ok(Self {
            findings: Vec::new(),
            contracts: self.contracts,
            proved_contracts: self.proved_contracts,
        })
    }

    pub fn status(&self) -> KernelCheckStatusV1 {
        self.findings
            .iter()
            .fold(KernelCheckStatusV1::Clean, |status, finding| {
                status.join(finding.status())
            })
    }
    pub fn findings(&self) -> &[PlironEffectRefinementFindingV1] {
        &self.findings
    }
    pub fn is_clean(&self) -> bool {
        self.status() == KernelCheckStatusV1::Clean
    }
    pub const fn contract_count(&self) -> usize {
        self.contracts
    }
    pub const fn proved_contract_count(&self) -> usize {
        self.proved_contracts
    }
    pub fn all_declared_effects_are_proved(&self) -> bool {
        self.is_clean() && self.contracts != 0 && self.contracts == self.proved_contracts
    }
    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone)]
struct EffectContractV1 {
    location: EffectRefinementLocationV1,
    obligation: [u64; 4],
    view: Value,
    view_name: String,
    indices: Vec<Value>,
    coordinates: Vec<(Value, Value)>,
    expressions: [Value; 6],
}

#[derive(Clone)]
struct WriteSiteV1 {
    location: EffectRefinementLocationV1,
    view: Value,
    indices: Vec<Value>,
}

pub(crate) fn clean_effect_refinement_report_v1() -> PlironEffectRefinementReportV1 {
    PlironEffectRefinementReportV1 {
        findings: Vec::new(),
        contracts: 0,
        proved_contracts: 0,
    }
}

fn project_hierarchy_finding_v1(
    finding: &HierarchicalOwnershipFindingV1,
    dynamic: Option<(String, usize)>,
) -> PlironEffectRefinementFindingV1 {
    let detail = || bounded_effect_diagnostic_v1(finding);
    match (finding, dynamic) {
        (HierarchicalOwnershipFindingV1::DynamicExtentIncomplete { view, dimension }, _) => {
            PlironEffectRefinementFindingV1::DynamicOwnershipIncomplete {
                view: bounded_effect_diagnostic_v1(view),
                dimension: Some(*dimension),
                detail: detail(),
            }
        }
        (_, Some((view, dimension))) if finding.status() != KernelCheckStatusV1::Rejected => {
            PlironEffectRefinementFindingV1::DynamicOwnershipIncomplete {
                view: bounded_effect_owned_diagnostic_v1(view),
                dimension: Some(dimension),
                detail: detail(),
            }
        }
        _ if finding.status() == KernelCheckStatusV1::Rejected => {
            PlironEffectRefinementFindingV1::OwnershipRejected { detail: detail() }
        }
        _ => PlironEffectRefinementFindingV1::OwnershipIncomplete { detail: detail() },
    }
}

#[cfg(test)]
pub(crate) fn run_pliron_effect_refinement_check_v1(
    context: &Context,
    function: &FuncOp,
) -> PlironEffectRefinementReportV1 {
    let mut analyses = PlironAnalysisManagerV1::new(function);
    run_pliron_effect_refinement_with_analyses_v1(context, function, &mut analyses)
}

#[cfg(test)]
pub(crate) fn run_pliron_effect_refinement_with_analyses_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
) -> PlironEffectRefinementReportV1 {
    run_pliron_effect_refinement_with_observation_v1(context, function, analyses, None)
}

pub(crate) fn run_pliron_effect_refinement_with_observation_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
    observer: EffectObserverV1<'_, '_, '_>,
) -> PlironEffectRefinementReportV1 {
    let body = run_effect_core_v1(
        context,
        function,
        analyses,
        &mut OrdinaryEffectModeV1,
        observer,
    );
    PlironEffectRefinementReportV1 {
        findings: body.findings,
        contracts: body.contracts,
        proved_contracts: body.proved_contracts,
    }
}

pub(super) mod conditional_v1;

#[cfg(all(test, feature = "internal-proof-staging"))]
pub(crate) mod source259_effect_tests {
    pub(crate) use super::conditional_v1::*;
}

include!("pliron_effect_refinement/execution_v1.rs");

type CollectedProofObligationV1 = (
    [u64; 4],
    Option<[u64; 4]>,
    Option<[u64; 4]>,
    Option<PropertyAttr>,
);

type CollectedEvidenceReferenceV1 = (
    Option<[u64; 4]>,
    Option<[u64; 4]>,
    Option<PropertyAttr>,
    Option<EvidenceStatusAttr>,
    Option<CoveredBoundaryAttr>,
);

type CollectedV1 = (
    Vec<EffectContractV1>,
    Vec<WriteSiteV1>,
    HashSet<Value>,
    Vec<CollectedProofObligationV1>,
    Vec<CollectedEvidenceReferenceV1>,
);

fn collect(
    context: &Context,
    inventory: &crate::production_analysis::pliron_function_inventory::BoundedPlironFunctionInventoryV1,
) -> CollectedV1 {
    let mut contracts = Vec::new();
    let mut writes = Vec::new();
    let mut ownership = HashSet::new();
    let mut obligations = Vec::new();
    let mut evidence = Vec::new();
    for site in inventory.operations() {
        let block_index = site.block();
        let operation_index = site.operation();
        let operation = Operation::get_op_dyn(site.pointer(), context);
        let location = EffectRefinementLocationV1 {
            block: block_index,
            operation: operation_index,
        };
        if let Some(contract) = operation.downcast_ref::<RequireEffectRefinementOp>() {
            let view = contract.view(context);
            contracts.push(EffectContractV1 {
                location,
                obligation: contract.obligation_id(context).unwrap_or([0; 4]),
                view,
                view_name: bounded_effect_diagnostic_v1(view.unique_name(context)),
                indices: contract.indices(context),
                coordinates: contract
                    .gpu_coordinates(context)
                    .into_iter()
                    .zip(contract.reference_coordinates(context))
                    .collect(),
                expressions: [
                    contract.gpu_domain(context),
                    contract.reference_domain(context),
                    contract.gpu_precondition(context),
                    contract.reference_precondition(context),
                    contract.gpu_value(context),
                    contract.reference_value(context),
                ],
            });
        } else if let Some(access) = operation.downcast_ref::<RankedAccessOp>() {
            if access
                .kind(context)
                .is_some_and(|kind| kind.writes_memory())
                && access
                    .view(context)
                    .defining_op()
                    .map(|definition| Operation::get_op_dyn(definition, context))
                    .and_then(|definition| definition.downcast_ref::<RankedViewOp>().copied())
                    .and_then(|view| view.memory_space(context))
                    .is_none_or(|space| space == MemorySpaceAttr::Global)
            {
                writes.push(WriteSiteV1 {
                    location,
                    view: access.view(context),
                    indices: access.indices(context),
                });
            }
        } else if let Some(contract) = operation.downcast_ref::<OwnershipContractOp>() {
            ownership.insert(contract.view(context));
        } else if let Some(obligation) = operation.downcast_ref::<ObligationOp>() {
            obligations.push((
                obligation.obligation_id(context).unwrap_or([0; 4]),
                obligation.subject_id(context),
                obligation.model_id(context),
                obligation.property(context),
            ));
        } else if let Some(record) = operation.downcast_ref::<EvidenceRefOp>() {
            evidence.push((
                record.evidence_id(context),
                record.obligation_id(context),
                record.property(context),
                record.status(context),
                record.covered_boundary(context),
            ));
        }
    }
    (contracts, writes, ownership, obligations, evidence)
}

fn validate_proof(
    contract: &EffectContractV1,
    obligations: &[CollectedProofObligationV1],
    evidence: &[CollectedEvidenceReferenceV1],
    findings: &mut Vec<PlironEffectRefinementFindingV1>,
) -> bool {
    let matching = obligations
        .iter()
        .filter(|record| record.0 == contract.obligation)
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        findings.push(if matching.is_empty() {
            PlironEffectRefinementFindingV1::ReferenceProofIncomplete {
                obligation: contract.obligation,
                location: contract.location,
                reason: "the exact MIR proof obligation is missing",
            }
        } else {
            PlironEffectRefinementFindingV1::ReferenceProofRejected {
                obligation: contract.obligation,
                location: contract.location,
                reason: "the MIR proof obligation identity is duplicated",
            }
        });
        return false;
    }
    let obligation = matching[0];
    if obligation.1.is_none()
        || obligation.2.is_none()
        || obligation.3 != Some(PropertyAttr::FunctionalRefinement)
    {
        findings.push(PlironEffectRefinementFindingV1::ReferenceProofRejected { obligation: contract.obligation, location: contract.location, reason: "the obligation lacks exact subject/model identities or FunctionalRefinement property" });
        return false;
    }
    let matching = evidence
        .iter()
        .filter(|record| record.1 == Some(contract.obligation))
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        findings.push(if matching.is_empty() {
            PlironEffectRefinementFindingV1::ReferenceProofIncomplete {
                obligation: contract.obligation,
                location: contract.location,
                reason: "the exact MIR evidence record is missing",
            }
        } else {
            PlironEffectRefinementFindingV1::ReferenceProofRejected {
                obligation: contract.obligation,
                location: contract.location,
                reason: "more than one evidence record claims the obligation",
            }
        });
        return false;
    }
    let record = matching[0];
    if record.0.is_none() || record.2 != Some(PropertyAttr::FunctionalRefinement) {
        findings.push(PlironEffectRefinementFindingV1::ReferenceProofRejected {
            obligation: contract.obligation,
            location: contract.location,
            reason: "the evidence identity or property is invalid",
        });
        return false;
    }
    if record.3 != Some(EvidenceStatusAttr::Checked) || record.4 != Some(CoveredBoundaryAttr::Mir) {
        findings.push(PlironEffectRefinementFindingV1::ReferenceProofIncomplete {
            obligation: contract.obligation,
            location: contract.location,
            reason: "effect refinement V1 requires policy-checked staging at the exact MIR boundary",
        });
        return false;
    }
    true
}

fn report(
    contracts: usize,
    proved_contracts: usize,
    findings: Vec<PlironEffectRefinementFindingV1>,
) -> EffectBodyV1 {
    EffectBodyV1 {
        findings,
        contracts,
        proved_contracts,
    }
}
fn one(contracts: usize, finding: PlironEffectRefinementFindingV1) -> EffectBodyV1 {
    report(contracts, 0, vec![finding])
}
fn proof_identity(words: [u64; 4]) -> String {
    format!(
        "{:016x}{:016x}{:016x}{:016x}",
        words[0], words[1], words[2], words[3]
    )
}

include!("pliron_effect_refinement/resource_tests.rs");
