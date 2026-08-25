//! Functional effect refinement between sequential references and GPU writes.
//!
//! The checker is workload neutral. It correlates inert proof contracts with
//! real guarded writes, consumes hierarchy ownership, and compares canonical
//! domain, precondition, and value expressions for every logical output.

use std::{
    collections::{HashMap, HashSet},
    fmt,
};

use dialect_kernel::{
    DYNAMIC_EXTENT, MemorySpaceAttr, OwnershipContractOp, RankedAccessOp, RankedViewOp,
    ranked_view_type,
};
use dialect_proof::{
    CoveredBoundaryAttr, EvidenceRefOp, EvidenceStatusAttr, ObligationOp, PropertyAttr,
    RequireEffectRefinementOp,
};
use pliron::{
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp},
    common_traits::Named,
    context::Context,
    linked_list::ContainsLinkedList,
    operation::Operation,
    value::Value,
};

use crate::KernelCheckStatusV1;
use crate::pliron_analysis_manager::PlironAnalysisManagerV1;
use crate::pliron_hierarchical_ownership::{
    HierarchicalOwnershipFindingV1, run_pliron_hierarchical_ownership_check_with_analyses_v1,
};
use crate::pliron_invocation_trace::PlironTraceLocationV1;
use crate::pliron_semantic_refinement::SemanticExpressionTableV1;

pub const MAX_EFFECT_REFINEMENT_CONTRACTS_V1: usize = 4_096;

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

pub fn run_pliron_effect_refinement_check_v1(
    context: &Context,
    function: &FuncOp,
) -> PlironEffectRefinementReportV1 {
    let mut analyses = PlironAnalysisManagerV1::new(function);
    run_pliron_effect_refinement_with_analyses_v1(context, function, &mut analyses)
}

pub fn require_pliron_effect_refinement_before_lowering_v1(
    context: &Context,
    function: &FuncOp,
) -> Result<PlironEffectRefinementReportV1, PlironEffectRefinementCheckErrorV1> {
    let report = run_pliron_effect_refinement_check_v1(context, function);
    if report.is_clean() {
        Ok(report)
    } else {
        Err(PlironEffectRefinementCheckErrorV1 { report })
    }
}

pub(crate) fn run_pliron_effect_refinement_with_analyses_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
) -> PlironEffectRefinementReportV1 {
    let (contracts, writes, ownership_views, obligations, evidence) = collect(context, function);
    if contracts.is_empty() {
        return clean_effect_refinement_report_v1();
    }
    if contracts.len() > MAX_EFFECT_REFINEMENT_CONTRACTS_V1 {
        return one(
            contracts.len(),
            PlironEffectRefinementFindingV1::ResourceLimitExceeded {
                actual: contracts.len(),
                limit: MAX_EFFECT_REFINEMENT_CONTRACTS_V1,
            },
        );
    }
    let mut findings = Vec::new();
    for contract in &contracts {
        if !ownership_views.contains(&contract.view) {
            findings.push(PlironEffectRefinementFindingV1::MissingOwnershipContract {
                view: contract.view_name.clone(),
                location: contract.location,
            });
        }
    }
    if !findings.is_empty() {
        return report(contracts.len(), 0, findings);
    }

    let hierarchy =
        run_pliron_hierarchical_ownership_check_with_analyses_v1(context, function, analyses);
    if !hierarchy.is_clean() {
        let finding = hierarchy
            .findings()
            .first()
            .expect("non-clean hierarchy has finding");
        let dynamic = contracts.iter().find_map(|contract| {
            ranked_view_type(contract.view, context).and_then(|view| {
                view.deref(context)
                    .shape()
                    .iter()
                    .position(|extent| *extent == DYNAMIC_EXTENT)
                    .map(|dimension| (contract.view_name.clone(), dimension))
            })
        });
        let effect_finding = match (finding, dynamic) {
            (HierarchicalOwnershipFindingV1::DynamicExtentIncomplete { view, dimension }, _) => {
                PlironEffectRefinementFindingV1::DynamicOwnershipIncomplete {
                    view: view.clone(),
                    dimension: Some(*dimension),
                    detail: finding.to_string(),
                }
            }
            (_, Some((view, dimension))) if finding.status() != KernelCheckStatusV1::Rejected => {
                PlironEffectRefinementFindingV1::DynamicOwnershipIncomplete {
                    view,
                    dimension: Some(dimension),
                    detail: finding.to_string(),
                }
            }
            _ if finding.status() == KernelCheckStatusV1::Rejected => {
                PlironEffectRefinementFindingV1::OwnershipRejected {
                    detail: finding.to_string(),
                }
            }
            _ => PlironEffectRefinementFindingV1::OwnershipIncomplete {
                detail: finding.to_string(),
            },
        };
        return one(contracts.len(), effect_finding);
    }

    let mut writes_by_signature =
        HashMap::<(usize, Value, Vec<Value>), Vec<EffectRefinementLocationV1>>::new();
    for write in &writes {
        writes_by_signature
            .entry((write.location.block, write.view, write.indices.clone()))
            .or_default()
            .push(write.location);
    }
    let mut by_write = HashMap::<EffectRefinementLocationV1, usize>::new();
    let mut write_by_contract = vec![None; contracts.len()];
    for (contract_index, contract) in contracts.iter().enumerate() {
        let signature = (
            contract.location.block,
            contract.view,
            contract.indices.clone(),
        );
        let matching = writes_by_signature
            .get(&signature)
            .map(Vec::as_slice)
            .unwrap_or_default();
        if matching.is_empty() {
            findings.push(PlironEffectRefinementFindingV1::OrphanEffectContract {
                view: contract.view_name.clone(),
                location: contract.location,
            });
            continue;
        }
        if matching.len() != 1 {
            findings.push(PlironEffectRefinementFindingV1::AmbiguousWriteSite {
                view: contract.view_name.clone(),
                location: contract.location,
                matches: matching.len(),
            });
            continue;
        }
        let write = matching[0];
        write_by_contract[contract_index] = Some(write);
        if let Some(first_index) = by_write.insert(write, contract_index) {
            findings.push(PlironEffectRefinementFindingV1::DuplicateEffectContract {
                view: contract.view_name.clone(),
                write,
                first: contracts[first_index].location,
                second: contract.location,
            });
        }
    }
    if !findings.is_empty() {
        return report(contracts.len(), 0, findings);
    }

    for write in &writes {
        if !by_write.contains_key(&write.location) {
            findings.push(PlironEffectRefinementFindingV1::UnmodeledWriteSite {
                view: write.view.unique_name(context).to_string(),
                location: write.location,
            });
        }
    }
    if !findings.is_empty() {
        return report(contracts.len(), 0, findings);
    }

    let expressions = match SemanticExpressionTableV1::from_function(context, function) {
        Ok(expressions) => expressions,
        Err(_) => {
            return one(
                contracts.len(),
                PlironEffectRefinementFindingV1::ResourceLimitExceeded {
                    actual: contracts.len(),
                    limit: MAX_EFFECT_REFINEMENT_CONTRACTS_V1,
                },
            );
        }
    };
    let mut proved = 0;
    for (index, contract) in contracts.iter().enumerate() {
        if !validate_proof(contract, &obligations, &evidence, &mut findings) {
            continue;
        }
        let _write = write_by_contract[index].expect("correlated contract has write");
        let witness = None;
        let mut pairs = contract
            .coordinates
            .iter()
            .map(|(actual, expected)| ("coordinate", *actual, *expected))
            .collect::<Vec<_>>();
        pairs.extend([
            ("domain", contract.expressions[0], contract.expressions[1]),
            (
                "precondition",
                contract.expressions[2],
                contract.expressions[3],
            ),
            ("value", contract.expressions[4], contract.expressions[5]),
        ]);
        let mut valid = true;
        for (component, actual, expected) in pairs {
            let Some(actual_description) = expressions.describe_value(actual) else {
                findings.push(PlironEffectRefinementFindingV1::UnresolvedExpression {
                    view: contract.view_name.clone(),
                    location: contract.location,
                    component,
                    value: actual.unique_name(context).to_string(),
                });
                valid = false;
                continue;
            };
            let Some(expected_description) = expressions.describe_value(expected) else {
                findings.push(PlironEffectRefinementFindingV1::UnresolvedExpression {
                    view: contract.view_name.clone(),
                    location: contract.location,
                    component,
                    value: expected.unique_name(context).to_string(),
                });
                valid = false;
                continue;
            };
            if expressions.equivalent(actual, expected) != Some(true) {
                let finding = match component {
                    "domain" => PlironEffectRefinementFindingV1::DomainMismatch {
                        view: contract.view_name.clone(),
                        location: contract.location,
                        actual: actual_description,
                        expected: expected_description,
                        witness: witness.clone(),
                    },
                    "precondition" => PlironEffectRefinementFindingV1::PreconditionMismatch {
                        view: contract.view_name.clone(),
                        location: contract.location,
                        actual: actual_description,
                        expected: expected_description,
                        witness: witness.clone(),
                    },
                    _ => PlironEffectRefinementFindingV1::ValueMismatch {
                        view: contract.view_name.clone(),
                        location: contract.location,
                        actual: actual_description,
                        expected: expected_description,
                        witness: witness.clone(),
                    },
                };
                findings.push(finding);
                valid = false;
            }
        }
        if valid {
            proved += 1;
        }
    }
    report(contracts.len(), proved, findings)
}

type CollectedV1 = (
    Vec<EffectContractV1>,
    Vec<WriteSiteV1>,
    HashSet<Value>,
    Vec<(
        [u64; 4],
        Option<[u64; 4]>,
        Option<[u64; 4]>,
        Option<PropertyAttr>,
    )>,
    Vec<(
        Option<[u64; 4]>,
        Option<[u64; 4]>,
        Option<PropertyAttr>,
        Option<EvidenceStatusAttr>,
        Option<CoveredBoundaryAttr>,
    )>,
);

fn collect(context: &Context, function: &FuncOp) -> CollectedV1 {
    let mut contracts = Vec::new();
    let mut writes = Vec::new();
    let mut ownership = HashSet::new();
    let mut obligations = Vec::new();
    let mut evidence = Vec::new();
    for (block_index, block) in function
        .get_region(context)
        .deref(context)
        .iter(context)
        .enumerate()
    {
        for (operation_index, raw) in block.deref(context).iter(context).enumerate() {
            let operation = Operation::get_op_dyn(raw, context);
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
                    view_name: view.unique_name(context).to_string(),
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
    }
    (contracts, writes, ownership, obligations, evidence)
}

fn validate_proof(
    contract: &EffectContractV1,
    obligations: &[(
        [u64; 4],
        Option<[u64; 4]>,
        Option<[u64; 4]>,
        Option<PropertyAttr>,
    )],
    evidence: &[(
        Option<[u64; 4]>,
        Option<[u64; 4]>,
        Option<PropertyAttr>,
        Option<EvidenceStatusAttr>,
        Option<CoveredBoundaryAttr>,
    )],
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
    if record.3 != Some(EvidenceStatusAttr::Proved) || record.4 != Some(CoveredBoundaryAttr::Mir) {
        findings.push(PlironEffectRefinementFindingV1::ReferenceProofIncomplete {
            obligation: contract.obligation,
            location: contract.location,
            reason: "effect refinement V1 requires Proved evidence at the exact MIR boundary",
        });
        return false;
    }
    true
}

fn report(
    contracts: usize,
    proved_contracts: usize,
    findings: Vec<PlironEffectRefinementFindingV1>,
) -> PlironEffectRefinementReportV1 {
    PlironEffectRefinementReportV1 {
        findings,
        contracts,
        proved_contracts,
    }
}
fn one(
    contracts: usize,
    finding: PlironEffectRefinementFindingV1,
) -> PlironEffectRefinementReportV1 {
    report(contracts, 0, vec![finding])
}
fn proof_identity(words: [u64; 4]) -> String {
    format!(
        "{:016x}{:016x}{:016x}{:016x}",
        words[0], words[1], words[2], words[3]
    )
}
