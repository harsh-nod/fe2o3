//! Generic declared semantic-equivalence verification for PLIRON SSA.

use std::{collections::HashMap, fmt};

use dialect_kernel::{
    RequireEquivalentOp, SemanticBinaryKindAttr, SemanticBinaryOp, SemanticConstantOp,
    SemanticSymbolOp,
};
use pliron::{
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp},
    common_traits::Named,
    context::Context,
    linked_list::ContainsLinkedList,
    operation::Operation,
};

use crate::pliron_analysis_manager::PlironAnalysisManagerV1;
use crate::pliron_ranked_bounds::run_pliron_ranked_bounds_check_with_analyses_v1;
use crate::{KernelCheckPassKindV1, KernelCheckStatusV1};

pub const MAX_PLIRON_SEMANTIC_NODES_V1: usize = 65_536;
pub const MAX_PLIRON_SEMANTIC_FINDINGS_V1: usize = 4_096;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum SemanticNodeV1 {
    Symbol(u32),
    Constant(u64),
    Binary(SemanticBinaryKindAttr, usize, usize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlironSemanticRefinementFindingV1 {
    BoundsPrerequisiteRejected,
    UnresolvedExpression {
        block: usize,
        operation: usize,
        value: String,
    },
    ExpressionMismatch {
        block: usize,
        operation: usize,
        actual: String,
        expected: String,
    },
    ResourceLimitExceeded,
}

impl PlironSemanticRefinementFindingV1 {
    pub const fn status(&self) -> KernelCheckStatusV1 {
        match self {
            Self::ExpressionMismatch { .. } => KernelCheckStatusV1::Rejected,
            Self::BoundsPrerequisiteRejected
            | Self::UnresolvedExpression { .. }
            | Self::ResourceLimitExceeded => KernelCheckStatusV1::Incomplete,
        }
    }
}

impl fmt::Display for PlironSemanticRefinementFindingV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BoundsPrerequisiteRejected => formatter.write_str(
                "error[FE2O3-SEMANTIC-000]: bounds prerequisite rejected before declared semantic refinement",
            ),
            Self::UnresolvedExpression {
                block,
                operation,
                value,
            } => write!(
                formatter,
                "error[FE2O3-SEMANTIC-002]: cannot resolve declared semantic expression {value} at block {block} op {operation}",
            ),
            Self::ExpressionMismatch {
                block,
                operation,
                actual,
                expected,
            } => write!(
                formatter,
                "error[FE2O3-SEMANTIC-001]: declared semantic refinement failed at block {block} op {operation}; actual expression `{actual}` is not equivalent to required expression `{expected}`; help: preserve the frontend-declared target-neutral semantic formula",
            ),
            Self::ResourceLimitExceeded => formatter.write_str(
                "error[FE2O3-SEMANTIC-002]: semantic refinement analysis resource limit exceeded",
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironSemanticRefinementReportV1 {
    findings: Vec<PlironSemanticRefinementFindingV1>,
}

impl PlironSemanticRefinementReportV1 {
    pub const fn pass(&self) -> KernelCheckPassKindV1 {
        KernelCheckPassKindV1::SemanticRefinement
    }

    pub fn status(&self) -> KernelCheckStatusV1 {
        self.findings
            .iter()
            .fold(KernelCheckStatusV1::Clean, |status, finding| {
                status.join(finding.status())
            })
    }

    pub fn findings(&self) -> &[PlironSemanticRefinementFindingV1] {
        &self.findings
    }

    pub fn is_clean(&self) -> bool {
        self.status() == KernelCheckStatusV1::Clean
    }

    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }

    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironSemanticRefinementCheckErrorV1 {
    report: PlironSemanticRefinementReportV1,
}

impl PlironSemanticRefinementCheckErrorV1 {
    pub fn report(&self) -> &PlironSemanticRefinementReportV1 {
        &self.report
    }
}

impl fmt::Display for PlironSemanticRefinementCheckErrorV1 {
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

impl std::error::Error for PlironSemanticRefinementCheckErrorV1 {}

pub fn run_pliron_semantic_refinement_check_v1(
    context: &Context,
    function: &FuncOp,
) -> PlironSemanticRefinementReportV1 {
    let mut analyses = PlironAnalysisManagerV1::new(function);
    if !run_pliron_ranked_bounds_check_with_analyses_v1(context, function, &mut analyses).is_clean()
    {
        return one(PlironSemanticRefinementFindingV1::BoundsPrerequisiteRejected);
    }
    run_pliron_semantic_refinement_check_after_bounds_v1(context, function)
}

pub(crate) fn run_pliron_semantic_refinement_check_after_bounds_v1(
    context: &Context,
    function: &FuncOp,
) -> PlironSemanticRefinementReportV1 {
    let mut definitions = Vec::new();
    let mut requirements = Vec::new();
    for (block_index, block) in function
        .get_region(context)
        .deref(context)
        .iter(context)
        .enumerate()
    {
        for (operation_index, operation) in block.deref(context).iter(context).enumerate() {
            let operation = Operation::get_op_dyn(operation, context);
            if operation.downcast_ref::<SemanticSymbolOp>().is_some()
                || operation.downcast_ref::<SemanticConstantOp>().is_some()
                || operation.downcast_ref::<SemanticBinaryOp>().is_some()
            {
                definitions.push(operation.get_operation());
            } else if let Some(requirement) = operation.downcast_ref::<RequireEquivalentOp>() {
                requirements.push((
                    block_index,
                    operation_index,
                    requirement.actual(context),
                    requirement.expected(context),
                ));
            }
        }
    }
    if requirements.is_empty() {
        return PlironSemanticRefinementReportV1 { findings: vec![] };
    }
    if definitions.len() > MAX_PLIRON_SEMANTIC_NODES_V1 {
        return one(PlironSemanticRefinementFindingV1::ResourceLimitExceeded);
    }

    let mut nodes = Vec::new();
    let mut interned = HashMap::new();
    let mut facts = HashMap::new();
    for _ in 0..=definitions.len() {
        let mut changed = false;
        for definition in &definitions {
            let operation = Operation::get_op_dyn(*definition, context);
            let node = if let Some(symbol) = operation.downcast_ref::<SemanticSymbolOp>() {
                symbol.symbol(context).map(SemanticNodeV1::Symbol)
            } else if let Some(constant) = operation.downcast_ref::<SemanticConstantOp>() {
                constant.value(context).map(SemanticNodeV1::Constant)
            } else if let Some(binary) = operation.downcast_ref::<SemanticBinaryOp>() {
                let lhs = facts.get(&binary.lhs(context)).copied();
                let rhs = facts.get(&binary.rhs(context)).copied();
                match (binary.kind(context), lhs, rhs) {
                    (Some(kind), Some(lhs), Some(rhs)) => {
                        let (lhs, rhs) = if lhs <= rhs { (lhs, rhs) } else { (rhs, lhs) };
                        Some(SemanticNodeV1::Binary(kind, lhs, rhs))
                    }
                    _ => None,
                }
            } else {
                None
            };
            let Some(node) = node else { continue };
            let identity = if let Some(identity) = interned.get(&node).copied() {
                identity
            } else {
                if nodes.len() == MAX_PLIRON_SEMANTIC_NODES_V1 {
                    return one(PlironSemanticRefinementFindingV1::ResourceLimitExceeded);
                }
                let identity = nodes.len();
                nodes.push(node.clone());
                interned.insert(node, identity);
                identity
            };
            let result = definition.deref(context).get_result(0);
            if facts.insert(result, identity) != Some(identity) {
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    let mut findings = Vec::new();
    for (block, operation, actual, expected) in requirements {
        let Some(actual_node) = facts.get(&actual).copied() else {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::UnresolvedExpression {
                    block,
                    operation,
                    value: actual.unique_name(context).to_string(),
                },
            );
            continue;
        };
        let Some(expected_node) = facts.get(&expected).copied() else {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::UnresolvedExpression {
                    block,
                    operation,
                    value: expected.unique_name(context).to_string(),
                },
            );
            continue;
        };
        if actual_node != expected_node {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::ExpressionMismatch {
                    block,
                    operation,
                    actual: describe(actual_node, &nodes),
                    expected: describe(expected_node, &nodes),
                },
            );
        }
    }
    PlironSemanticRefinementReportV1 { findings }
}

pub(crate) fn require_pliron_semantic_refinement_with_analyses_v1(
    context: &Context,
    function: &FuncOp,
    _analyses: &mut PlironAnalysisManagerV1,
) -> Result<PlironSemanticRefinementReportV1, PlironSemanticRefinementCheckErrorV1> {
    let report = run_pliron_semantic_refinement_check_after_bounds_v1(context, function);
    if report.is_clean() {
        Ok(report)
    } else {
        Err(PlironSemanticRefinementCheckErrorV1 { report })
    }
}

pub fn require_pliron_semantic_refinement_before_lowering_v1(
    context: &Context,
    function: &FuncOp,
) -> Result<PlironSemanticRefinementReportV1, PlironSemanticRefinementCheckErrorV1> {
    let report = run_pliron_semantic_refinement_check_v1(context, function);
    if report.is_clean() {
        Ok(report)
    } else {
        Err(PlironSemanticRefinementCheckErrorV1 { report })
    }
}

fn describe(identity: usize, nodes: &[SemanticNodeV1]) -> String {
    match &nodes[identity] {
        SemanticNodeV1::Symbol(symbol) => format!("s{symbol}"),
        SemanticNodeV1::Constant(value) => format!("c0x{value:016x}"),
        SemanticNodeV1::Binary(kind, lhs, rhs) => {
            let operator = match kind {
                SemanticBinaryKindAttr::Add => "+",
                SemanticBinaryKindAttr::Multiply => "*",
            };
            format!(
                "({} {operator} {})",
                describe(*lhs, nodes),
                describe(*rhs, nodes)
            )
        }
    }
}

fn push(
    findings: &mut Vec<PlironSemanticRefinementFindingV1>,
    finding: PlironSemanticRefinementFindingV1,
) {
    if findings.len() < MAX_PLIRON_SEMANTIC_FINDINGS_V1 {
        findings.push(finding);
    } else if !matches!(
        findings.last(),
        Some(PlironSemanticRefinementFindingV1::ResourceLimitExceeded)
    ) {
        findings.push(PlironSemanticRefinementFindingV1::ResourceLimitExceeded);
    }
}

fn one(finding: PlironSemanticRefinementFindingV1) -> PlironSemanticRefinementReportV1 {
    PlironSemanticRefinementReportV1 {
        findings: vec![finding],
    }
}

#[cfg(test)]
mod status_tests {
    use super::*;

    fn mismatch() -> PlironSemanticRefinementFindingV1 {
        PlironSemanticRefinementFindingV1::ExpressionMismatch {
            block: 0,
            operation: 0,
            actual: "s0".to_owned(),
            expected: "s1".to_owned(),
        }
    }

    #[test]
    fn every_semantic_finding_has_the_shared_status() {
        let incomplete = [
            PlironSemanticRefinementFindingV1::BoundsPrerequisiteRejected,
            PlironSemanticRefinementFindingV1::UnresolvedExpression {
                block: 0,
                operation: 0,
                value: "v0".to_owned(),
            },
            PlironSemanticRefinementFindingV1::ResourceLimitExceeded,
        ];
        for finding in incomplete {
            assert_eq!(finding.status(), KernelCheckStatusV1::Incomplete);
        }
        assert_eq!(mismatch().status(), KernelCheckStatusV1::Rejected);
    }

    #[test]
    fn rejected_semantic_finding_dominates_an_incomplete_finding() {
        let report = PlironSemanticRefinementReportV1 {
            findings: vec![
                PlironSemanticRefinementFindingV1::UnresolvedExpression {
                    block: 0,
                    operation: 0,
                    value: "v0".to_owned(),
                },
                mismatch(),
            ],
        };
        assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
        assert!(!report.is_clean());
        assert_eq!(
            PlironSemanticRefinementReportV1 { findings: vec![] }.status(),
            KernelCheckStatusV1::Clean
        );
    }
}
