//! Generic declared semantic-equivalence verification for PLIRON SSA.

use std::{
    collections::{HashMap, HashSet},
    fmt,
};

use dialect_kernel::{
    OwnershipContractOp, OwnershipCoverageAttr, RequireEquivalentOp, RequireFiniteFoldOp,
    RequireFiniteRecurrenceOp, RequirePermutationGatherOp, SemanticBinaryKindAttr,
    SemanticBinaryOp, SemanticConstantOp, SemanticCoverageBindingAttr,
    SemanticExpressionCommitmentOp, SemanticNumericalContractV1, SemanticNumericalPolicyAttr,
    SemanticSymbolOp, SemanticTypedBinaryOp, SemanticTypedCastOp, SemanticTypedCompareOp,
    SemanticTypedConstantOp, SemanticTypedExpressionRootOp, SemanticTypedExpressionV1,
    SemanticTypedScalarV1, SemanticTypedSelectOp, SemanticTypedSymbolOp, SemanticTypedUnaryOp,
    TensorResultComponentOp,
};
use dialect_proof::{
    CoveredBoundaryAttr, EvidenceRefOp, EvidenceStatusAttr, ObligationOp, PropertyAttr,
    RequireEffectRefinementOp, RequireNumericalRefinementOp, RequireRefinementOp,
    RequireTensorRefinementOp,
};
use pliron::{
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp},
    common_traits::Named,
    context::Context,
    linked_list::ContainsLinkedList,
    operation::Operation,
};

use crate::pliron_analysis_manager::PlironAnalysisManagerV1;
use crate::pliron_effect_refinement::{
    PlironEffectRefinementReportV1, clean_effect_refinement_report_v1,
    run_pliron_effect_refinement_with_analyses_v1,
};
use crate::pliron_ranked_bounds::run_pliron_ranked_bounds_check_with_analyses_v1;
use crate::{KernelCheckPassKindV1, KernelCheckStatusV1};

pub const MAX_PLIRON_SEMANTIC_NODES_V1: usize = 65_536;
pub const MAX_PLIRON_SEMANTIC_FINDINGS_V1: usize = 4_096;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum SemanticNodeV1 {
    Symbol(u32),
    Constant(u64),
    Binary(SemanticBinaryKindAttr, usize, usize),
    /// Opaque equality is sound only for byte-identical retained commitments.
    Commitment([u64; 4]),
    TypedExpression(SemanticTypedExpressionV1),
    TypedRoot(SemanticTypedExpressionV1, SemanticNumericalContractV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SemanticExpressionBuildErrorV1 {
    ResourceLimit,
    InvalidTypedExpression(&'static str),
}

/// Shared canonical expression table used by scalar and effect refinement.
pub(crate) struct SemanticExpressionTableV1 {
    nodes: Vec<SemanticNodeV1>,
    facts: HashMap<pliron::value::Value, usize>,
    typed_root_commitments: Vec<[u64; 4]>,
}

impl SemanticExpressionTableV1 {
    pub(crate) fn from_function(
        context: &Context,
        function: &FuncOp,
    ) -> Result<Self, SemanticExpressionBuildErrorV1> {
        let definitions = function
            .get_region(context)
            .deref(context)
            .iter(context)
            .flat_map(|block| block.deref(context).iter(context))
            .filter(|operation| {
                let operation = Operation::get_op_dyn(*operation, context);
                operation.downcast_ref::<SemanticSymbolOp>().is_some()
                    || operation.downcast_ref::<SemanticConstantOp>().is_some()
                    || operation.downcast_ref::<SemanticBinaryOp>().is_some()
                    || operation
                        .downcast_ref::<SemanticExpressionCommitmentOp>()
                        .is_some()
                    || is_typed_semantic_definition(&*operation)
            })
            .collect::<Vec<_>>();
        Self::build(context, &definitions)
    }

    fn build(
        context: &Context,
        definitions: &[pliron::context::Ptr<Operation>],
    ) -> Result<Self, SemanticExpressionBuildErrorV1> {
        if definitions.len() > MAX_PLIRON_SEMANTIC_NODES_V1 {
            return Err(SemanticExpressionBuildErrorV1::ResourceLimit);
        }
        let mut nodes = Vec::new();
        let mut interned = HashMap::new();
        let mut facts = HashMap::new();
        for _ in 0..=definitions.len() {
            let mut changed = false;
            for definition in definitions {
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
                } else if let Some(commitment) =
                    operation.downcast_ref::<SemanticExpressionCommitmentOp>()
                {
                    commitment.identity(context).map(SemanticNodeV1::Commitment)
                } else if let Some(symbol) = operation.downcast_ref::<SemanticTypedSymbolOp>() {
                    match (symbol.symbol(context), symbol.scalar(context)) {
                        (Some(symbol), Some(scalar)) => Some(SemanticNodeV1::TypedExpression(
                            SemanticTypedExpressionV1::Symbol { symbol, scalar },
                        )),
                        _ => None,
                    }
                } else if let Some(constant) = operation.downcast_ref::<SemanticTypedConstantOp>() {
                    match (constant.bits(context), constant.scalar(context)) {
                        (Some(bits), Some(scalar)) => Some(SemanticNodeV1::TypedExpression(
                            SemanticTypedExpressionV1::Constant { scalar, bits },
                        )),
                        _ => None,
                    }
                } else if let Some(unary) = operation.downcast_ref::<SemanticTypedUnaryOp>() {
                    let operand =
                        typed_expression(&nodes, facts.get(&unary.operand(context)).copied());
                    match (unary.kind(context), unary.scalar(context), operand) {
                        (Some(operation), Some(scalar), Some(operand)) => Some(
                            SemanticNodeV1::TypedExpression(SemanticTypedExpressionV1::Unary {
                                operation,
                                scalar,
                                operand: Box::new(operand),
                            }),
                        ),
                        _ => None,
                    }
                } else if let Some(binary) = operation.downcast_ref::<SemanticTypedBinaryOp>() {
                    let lhs = typed_expression(&nodes, facts.get(&binary.lhs(context)).copied());
                    let rhs = typed_expression(&nodes, facts.get(&binary.rhs(context)).copied());
                    match (
                        binary.kind(context),
                        binary.scalar(context),
                        binary.overflow(context),
                        lhs,
                        rhs,
                    ) {
                        (Some(operation), Some(scalar), Some(overflow), Some(lhs), Some(rhs)) => {
                            Some(SemanticNodeV1::TypedExpression(
                                SemanticTypedExpressionV1::Binary {
                                    operation,
                                    scalar,
                                    overflow,
                                    lhs: Box::new(lhs),
                                    rhs: Box::new(rhs),
                                },
                            ))
                        }
                        _ => None,
                    }
                } else if let Some(compare) = operation.downcast_ref::<SemanticTypedCompareOp>() {
                    let lhs = typed_expression(&nodes, facts.get(&compare.lhs(context)).copied());
                    let rhs = typed_expression(&nodes, facts.get(&compare.rhs(context)).copied());
                    match (
                        compare.kind(context),
                        compare.operand_scalar(context),
                        lhs,
                        rhs,
                    ) {
                        (Some(operation), Some(operand_scalar), Some(lhs), Some(rhs)) => Some(
                            SemanticNodeV1::TypedExpression(SemanticTypedExpressionV1::Compare {
                                operation,
                                operand_scalar,
                                lhs: Box::new(lhs),
                                rhs: Box::new(rhs),
                            }),
                        ),
                        _ => None,
                    }
                } else if let Some(select) = operation.downcast_ref::<SemanticTypedSelectOp>() {
                    let condition =
                        typed_expression(&nodes, facts.get(&select.condition(context)).copied());
                    let when_true =
                        typed_expression(&nodes, facts.get(&select.when_true(context)).copied());
                    let when_false =
                        typed_expression(&nodes, facts.get(&select.when_false(context)).copied());
                    match (select.scalar(context), condition, when_true, when_false) {
                        (Some(scalar), Some(condition), Some(when_true), Some(when_false)) => Some(
                            SemanticNodeV1::TypedExpression(SemanticTypedExpressionV1::Select {
                                scalar,
                                condition: Box::new(condition),
                                when_true: Box::new(when_true),
                                when_false: Box::new(when_false),
                            }),
                        ),
                        _ => None,
                    }
                } else if let Some(cast) = operation.downcast_ref::<SemanticTypedCastOp>() {
                    let operand =
                        typed_expression(&nodes, facts.get(&cast.operand(context)).copied());
                    match (
                        cast.kind(context),
                        cast.source(context),
                        cast.target(context),
                        operand,
                    ) {
                        (Some(kind), Some(source), Some(target), Some(operand)) => Some(
                            SemanticNodeV1::TypedExpression(SemanticTypedExpressionV1::Cast {
                                kind,
                                source,
                                target,
                                operand: Box::new(operand),
                            }),
                        ),
                        _ => None,
                    }
                } else if let Some(root) = operation.downcast_ref::<SemanticTypedExpressionRootOp>()
                {
                    let expression =
                        typed_expression(&nodes, facts.get(&root.expression(context)).copied());
                    match (
                        expression,
                        root.policy(context),
                        root.rounding(context),
                        root.exceptional_values(context),
                        root.commitment(context),
                    ) {
                        (
                            Some(expression),
                            Some(policy),
                            Some(rounding),
                            Some(exceptional_values),
                            Some(commitment),
                        ) => {
                            let contract = SemanticNumericalContractV1 {
                                policy,
                                rounding,
                                exceptional_values,
                            };
                            validate_typed_root(&expression, contract, commitment)?;
                            Some(SemanticNodeV1::TypedRoot(expression, contract))
                        }
                        _ => None,
                    }
                } else {
                    None
                };
                let Some(node) = node else { continue };
                if let SemanticNodeV1::TypedExpression(expression) = &node {
                    expression.validate().map_err(|error| {
                        SemanticExpressionBuildErrorV1::InvalidTypedExpression(match error {
                            dialect_kernel::SemanticTypedExpressionErrorV1::ResourceLimit => {
                                "typed semantic expression exceeds its node or depth bound"
                            }
                            dialect_kernel::SemanticTypedExpressionErrorV1::TypeMismatch => {
                                "typed semantic expression has an invalid type or operator"
                            }
                            dialect_kernel::SemanticTypedExpressionErrorV1::ConstantOutOfRange => {
                                "typed semantic constant exceeds its scalar width"
                            }
                            dialect_kernel::SemanticTypedExpressionErrorV1::UnsupportedNumericalPolicy => {
                                "typed semantic numerical policy is unsupported"
                            }
                            dialect_kernel::SemanticTypedExpressionErrorV1::IncompleteDomain => {
                                "typed semantic operation definedness is incomplete"
                            }
                        })
                    })?;
                }
                let identity = if let Some(identity) = interned.get(&node).copied() {
                    identity
                } else {
                    if nodes.len() == MAX_PLIRON_SEMANTIC_NODES_V1 {
                        return Err(SemanticExpressionBuildErrorV1::ResourceLimit);
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
        let mut typed_root_commitments = Vec::new();
        for definition in definitions {
            let operation = Operation::get_op_dyn(*definition, context);
            if is_typed_semantic_definition(&*operation)
                && !facts.contains_key(&definition.deref(context).get_result(0))
            {
                return Err(SemanticExpressionBuildErrorV1::InvalidTypedExpression(
                    "typed semantic SSA node cannot be reconstructed",
                ));
            }
            if let Some(root) = operation.downcast_ref::<SemanticTypedExpressionRootOp>() {
                if !facts.contains_key(&root.result(context)) {
                    return Err(SemanticExpressionBuildErrorV1::InvalidTypedExpression(
                        "typed semantic SSA root cannot be reconstructed",
                    ));
                }
                typed_root_commitments.push(root.commitment(context).ok_or(
                    SemanticExpressionBuildErrorV1::InvalidTypedExpression(
                        "typed semantic root lacks its commitment",
                    ),
                )?);
            }
        }
        Ok(Self {
            nodes,
            facts,
            typed_root_commitments,
        })
    }

    pub(crate) fn identity(&self, value: pliron::value::Value) -> Option<usize> {
        self.facts.get(&value).copied()
    }

    pub(crate) fn equivalent(
        &self,
        actual: pliron::value::Value,
        expected: pliron::value::Value,
    ) -> Option<bool> {
        Some(self.identity(actual)? == self.identity(expected)?)
    }

    pub(crate) fn typed_root_commitments(&self) -> &[[u64; 4]] {
        &self.typed_root_commitments
    }

    fn typed_root_fact(&self, value: pliron::value::Value) -> Option<TypedRootFactV1> {
        match self.nodes.get(self.identity(value)?)? {
            SemanticNodeV1::TypedRoot(expression, contract) => Some(TypedRootFactV1 {
                scalar: expression.scalar(),
                contract: *contract,
            }),
            _ => None,
        }
    }

    pub(crate) fn describe_value(&self, value: pliron::value::Value) -> Option<String> {
        self.identity(value).map(|identity| self.describe(identity))
    }

    fn describe(&self, identity: usize) -> String {
        match &self.nodes[identity] {
            SemanticNodeV1::Symbol(symbol) => format!("s{symbol}"),
            SemanticNodeV1::Constant(value) => format!("c0x{value:016x}"),
            SemanticNodeV1::Binary(kind, lhs, rhs) => {
                let operator = match kind {
                    SemanticBinaryKindAttr::Add => "+",
                    SemanticBinaryKindAttr::Multiply => "*",
                };
                format!(
                    "({} {operator} {})",
                    self.describe(*lhs),
                    self.describe(*rhs)
                )
            }
            SemanticNodeV1::Commitment(identity) => format!(
                "typed-commitment:{:016x}{:016x}{:016x}{:016x}",
                identity[0], identity[1], identity[2], identity[3]
            ),
            SemanticNodeV1::TypedExpression(expression) => {
                format!("typed-node:{expression:?}")
            }
            SemanticNodeV1::TypedRoot(expression, contract) => {
                format!("typed-root:{contract:?}:{expression:?}")
            }
        }
    }
}

fn is_typed_semantic_definition(operation: &dyn pliron::op::Op) -> bool {
    operation.downcast_ref::<SemanticTypedSymbolOp>().is_some()
        || operation
            .downcast_ref::<SemanticTypedConstantOp>()
            .is_some()
        || operation.downcast_ref::<SemanticTypedUnaryOp>().is_some()
        || operation.downcast_ref::<SemanticTypedBinaryOp>().is_some()
        || operation.downcast_ref::<SemanticTypedCompareOp>().is_some()
        || operation.downcast_ref::<SemanticTypedSelectOp>().is_some()
        || operation.downcast_ref::<SemanticTypedCastOp>().is_some()
        || operation
            .downcast_ref::<SemanticTypedExpressionRootOp>()
            .is_some()
}

fn typed_expression(
    nodes: &[SemanticNodeV1],
    identity: Option<usize>,
) -> Option<SemanticTypedExpressionV1> {
    match nodes.get(identity?)? {
        SemanticNodeV1::TypedExpression(expression) => Some(expression.clone()),
        _ => None,
    }
}

fn validate_typed_root(
    expression: &SemanticTypedExpressionV1,
    contract: SemanticNumericalContractV1,
    commitment: [u64; 4],
) -> Result<(), SemanticExpressionBuildErrorV1> {
    expression.validate().map_err(|error| {
        SemanticExpressionBuildErrorV1::InvalidTypedExpression(match error {
            dialect_kernel::SemanticTypedExpressionErrorV1::ResourceLimit => {
                "typed semantic expression exceeds its node or depth bound"
            }
            dialect_kernel::SemanticTypedExpressionErrorV1::TypeMismatch => {
                "typed semantic expression has an invalid type or operator"
            }
            dialect_kernel::SemanticTypedExpressionErrorV1::ConstantOutOfRange => {
                "typed semantic constant exceeds its scalar width"
            }
            dialect_kernel::SemanticTypedExpressionErrorV1::UnsupportedNumericalPolicy => {
                "typed semantic numerical policy is unsupported"
            }
            dialect_kernel::SemanticTypedExpressionErrorV1::IncompleteDomain => {
                "typed semantic operation definedness is incomplete"
            }
        })
    })?;
    expression.validate_static_domains().map_err(|_| {
        SemanticExpressionBuildErrorV1::InvalidTypedExpression(
            "typed semantic operation definedness is incomplete",
        )
    })?;
    contract.validate(expression).map_err(|_| {
        SemanticExpressionBuildErrorV1::InvalidTypedExpression(
            "typed semantic numerical policy does not match the expression",
        )
    })?;
    if digest_words(expression.canonical_transcript_sha256(contract)) != commitment {
        return Err(SemanticExpressionBuildErrorV1::InvalidTypedExpression(
            "typed semantic commitment does not match the reconstructed expression and policy",
        ));
    }
    Ok(())
}

fn digest_words(digest: [u8; 32]) -> [u64; 4] {
    std::array::from_fn(|index| {
        u64::from_le_bytes(
            digest[index * 8..(index + 1) * 8]
                .try_into()
                .expect("digest word has fixed width"),
        )
    })
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
    ReferenceContractIncomplete {
        block: usize,
        operation: usize,
        obligation: [u64; 4],
        reason: &'static str,
    },
    ReferenceContractRejected {
        block: usize,
        operation: usize,
        obligation: [u64; 4],
        reason: &'static str,
    },
    CollectiveContractIncomplete {
        block: usize,
        operation: usize,
        reason: &'static str,
    },
    CollectiveContractRejected {
        block: usize,
        operation: usize,
        reason: &'static str,
    },
    TypedExpressionRejected {
        reason: &'static str,
    },
    ResourceLimitExceeded,
}

impl PlironSemanticRefinementFindingV1 {
    pub const fn status(&self) -> KernelCheckStatusV1 {
        match self {
            Self::ExpressionMismatch { .. }
            | Self::ReferenceContractRejected { .. }
            | Self::CollectiveContractRejected { .. }
            | Self::TypedExpressionRejected { .. } => KernelCheckStatusV1::Rejected,
            Self::BoundsPrerequisiteRejected
            | Self::UnresolvedExpression { .. }
            | Self::ReferenceContractIncomplete { .. }
            | Self::CollectiveContractIncomplete { .. }
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
            Self::ReferenceContractIncomplete {
                block,
                operation,
                obligation,
                reason,
            } => write!(
                formatter,
                "error[FE2O3-SEMANTIC-003]: functional-reference obligation {} is incomplete at block {block} op {operation}: {reason}",
                proof_identity(*obligation),
            ),
            Self::ReferenceContractRejected {
                block,
                operation,
                obligation,
                reason,
            } => write!(
                formatter,
                "error[FE2O3-SEMANTIC-004]: functional-reference obligation {} is invalid at block {block} op {operation}: {reason}",
                proof_identity(*obligation),
            ),
            Self::CollectiveContractIncomplete {
                block,
                operation,
                reason,
            } => write!(
                formatter,
                "error[FE2O3-SEMANTIC-005]: finite collective contract is incomplete at block {block} op {operation}: {reason}",
            ),
            Self::CollectiveContractRejected {
                block,
                operation,
                reason,
            } => write!(
                formatter,
                "error[FE2O3-SEMANTIC-006]: finite collective contract is invalid at block {block} op {operation}: {reason}",
            ),
            Self::TypedExpressionRejected { reason } => write!(
                formatter,
                "error[FE2O3-SEMANTIC-007]: typed semantic expression payload is invalid: {reason}",
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
    reference_obligations: usize,
    proved_reference_obligations: usize,
    numerical_obligations: usize,
    proved_numerical_obligations: usize,
    collective_contracts: usize,
    proved_collective_contracts: usize,
    typed_root_commitments: Vec<[u64; 4]>,
    effect_refinement: PlironEffectRefinementReportV1,
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
            .join(self.effect_refinement.status())
    }

    pub fn findings(&self) -> &[PlironSemanticRefinementFindingV1] {
        &self.findings
    }

    pub fn is_clean(&self) -> bool {
        self.status() == KernelCheckStatusV1::Clean
    }

    /// Number of target-neutral expression equalities explicitly bound to a
    /// functional-reference proof obligation.
    pub const fn reference_obligation_count(&self) -> usize {
        self.reference_obligations
    }

    /// Number of reference-bound equalities with one exact proved source
    /// evidence record and an equal target-neutral expression.
    pub const fn proved_reference_obligation_count(&self) -> usize {
        self.proved_reference_obligations
    }

    /// Whether at least one reference obligation was declared and every such
    /// obligation was discharged by this pass.
    pub fn all_reference_obligations_are_proved(&self) -> bool {
        self.is_clean()
            && self.reference_obligations != 0
            && self.reference_obligations == self.proved_reference_obligations
    }

    /// Number of authenticated finite-error relations in the live graph.
    pub const fn numerical_obligation_count(&self) -> usize {
        self.numerical_obligations
    }

    /// Number whose typed roots, finite bounds, and exact evidence join passed.
    pub const fn proved_numerical_obligation_count(&self) -> usize {
        self.proved_numerical_obligations
    }

    pub fn all_numerical_obligations_are_proved(&self) -> bool {
        self.is_clean()
            && self.numerical_obligations != 0
            && self.numerical_obligations == self.proved_numerical_obligations
    }

    /// Number of closed finite fold, recurrence, and permutation contracts.
    pub const fn collective_contract_count(&self) -> usize {
        self.collective_contracts
    }

    /// Number independently joined to one proved MIR functional-refinement obligation.
    pub const fn proved_collective_contract_count(&self) -> usize {
        self.proved_collective_contracts
    }

    pub fn all_collective_contracts_are_proved(&self) -> bool {
        self.is_clean()
            && self.collective_contracts != 0
            && self.collective_contracts == self.proved_collective_contracts
    }

    /// Canonical typed-root commitments reconstructed and verified by the
    /// mandatory semantic pass, in function order.
    pub fn typed_root_commitments(&self) -> &[[u64; 4]] {
        &self.typed_root_commitments
    }

    pub const fn effect_refinement(&self) -> &PlironEffectRefinementReportV1 {
        &self.effect_refinement
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
        let mut wrote = false;
        for (index, finding) in self.report.findings.iter().enumerate() {
            if index != 0 {
                formatter.write_str("\n")?;
            }
            finding.fmt(formatter)?;
            wrote = true;
        }
        for finding in self.report.effect_refinement.findings() {
            if wrote {
                formatter.write_str("\n")?;
            }
            finding.fmt(formatter)?;
            wrote = true;
        }
        Ok(())
    }
}

impl std::error::Error for PlironSemanticRefinementCheckErrorV1 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TypedRootFactV1 {
    scalar: SemanticTypedScalarV1,
    contract: SemanticNumericalContractV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CollectiveContractKindV1 {
    Fold,
    Recurrence,
    Permutation,
}

#[derive(Clone, Copy, Debug)]
struct CollectiveContractV1 {
    kind: CollectiveContractKindV1,
    block: usize,
    operation: usize,
    view: pliron::value::Value,
    actual: pliron::value::Value,
    expected: pliron::value::Value,
    coverage: Option<SemanticCoverageBindingAttr>,
    numerical_policy: Option<SemanticNumericalPolicyAttr>,
    witness0: pliron::value::Value,
    witness1: pliron::value::Value,
}

pub fn run_pliron_semantic_refinement_check_v1(
    context: &Context,
    function: &FuncOp,
) -> PlironSemanticRefinementReportV1 {
    let mut analyses = PlironAnalysisManagerV1::new(function);
    if !run_pliron_ranked_bounds_check_with_analyses_v1(context, function, &mut analyses).is_clean()
    {
        return one(PlironSemanticRefinementFindingV1::BoundsPrerequisiteRejected);
    }
    run_pliron_semantic_refinement_check_after_bounds_v1(context, function, &mut analyses)
}

pub(crate) fn run_pliron_semantic_refinement_check_after_bounds_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
) -> PlironSemanticRefinementReportV1 {
    let mut definitions = Vec::new();
    let mut requirements = Vec::new();
    let mut reference_requirements = Vec::new();
    let mut numerical_requirements = Vec::new();
    let mut tensor_requirements = Vec::new();
    let mut tensor_components = HashMap::new();
    let mut effect_requirement_ids = HashSet::new();
    let mut obligations = Vec::new();
    let mut evidence = Vec::new();
    let mut ownership_contracts = Vec::new();
    let mut collective_contracts = Vec::new();
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
                || operation
                    .downcast_ref::<SemanticExpressionCommitmentOp>()
                    .is_some()
                || is_typed_semantic_definition(&*operation)
            {
                definitions.push(operation.get_operation());
            } else if let Some(component) = operation.downcast_ref::<TensorResultComponentOp>() {
                tensor_components.insert(
                    component.result(context),
                    (
                        component.result_root(context),
                        component.component(context),
                        component.scalar(context),
                    ),
                );
            } else if let Some(requirement) = operation.downcast_ref::<RequireEquivalentOp>() {
                requirements.push((
                    block_index,
                    operation_index,
                    requirement.actual(context),
                    requirement.expected(context),
                ));
            } else if let Some(requirement) = operation.downcast_ref::<RequireRefinementOp>() {
                reference_requirements.push((
                    block_index,
                    operation_index,
                    requirement.obligation_id(context),
                    requirement.actual(context),
                    requirement.expected(context),
                ));
            } else if let Some(requirement) = operation.downcast_ref::<RequireTensorRefinementOp>()
            {
                tensor_requirements.push((
                    block_index,
                    operation_index,
                    requirement.obligation_id(context),
                    requirement.result_root(context),
                    requirement.view(context),
                    requirement.actual(context),
                    requirement.reference(context),
                    requirement.components(context),
                ));
            } else if let Some(requirement) = operation.downcast_ref::<RequireEffectRefinementOp>()
            {
                effect_requirement_ids.insert(requirement.obligation_id(context).unwrap_or([0; 4]));
            } else if let Some(requirement) =
                operation.downcast_ref::<RequireNumericalRefinementOp>()
            {
                numerical_requirements.push((
                    block_index,
                    operation_index,
                    requirement.obligation_id(context),
                    operation.get_operation().deref(context).get_operand(0),
                    operation.get_operation().deref(context).get_operand(1),
                    operation.get_operation().deref(context).get_operand(2),
                    operation.get_operation().deref(context).get_operand(3),
                    requirement.absolute_error_f64_bits(context),
                    requirement.relative_error_f64_bits(context),
                ));
            } else if let Some(obligation) = operation.downcast_ref::<ObligationOp>() {
                obligations.push((
                    block_index,
                    operation_index,
                    obligation.obligation_id(context),
                    obligation.subject_id(context),
                    obligation.model_id(context),
                    obligation.property(context),
                ));
            } else if let Some(record) = operation.downcast_ref::<EvidenceRefOp>() {
                evidence.push((
                    block_index,
                    operation_index,
                    record.evidence_id(context),
                    record.obligation_id(context),
                    record.property(context),
                    record.status(context),
                    record.covered_boundary(context),
                ));
            } else if let Some(ownership) = operation.downcast_ref::<OwnershipContractOp>() {
                ownership_contracts.push((ownership.view(context), ownership.coverage(context)));
            } else if let Some(contract) = operation.downcast_ref::<RequireFiniteFoldOp>() {
                collective_contracts.push(CollectiveContractV1 {
                    kind: CollectiveContractKindV1::Fold,
                    block: block_index,
                    operation: operation_index,
                    view: contract.view(context),
                    actual: contract.actual(context),
                    expected: contract.expected(context),
                    coverage: contract.coverage(context),
                    numerical_policy: contract.numerical_policy(context),
                    witness0: contract.identity(context),
                    witness1: contract.operator(context),
                });
            } else if let Some(contract) = operation.downcast_ref::<RequireFiniteRecurrenceOp>() {
                collective_contracts.push(CollectiveContractV1 {
                    kind: CollectiveContractKindV1::Recurrence,
                    block: block_index,
                    operation: operation_index,
                    view: contract.view(context),
                    actual: contract.actual(context),
                    expected: contract.expected(context),
                    coverage: contract.coverage(context),
                    numerical_policy: contract.numerical_policy(context),
                    witness0: contract.initial(context),
                    witness1: contract.transition(context),
                });
            } else if let Some(contract) = operation.downcast_ref::<RequirePermutationGatherOp>() {
                collective_contracts.push(CollectiveContractV1 {
                    kind: CollectiveContractKindV1::Permutation,
                    block: block_index,
                    operation: operation_index,
                    view: contract.view(context),
                    actual: contract.actual(context),
                    expected: contract.expected(context),
                    coverage: contract.coverage(context),
                    numerical_policy: contract.numerical_policy(context),
                    witness0: contract.mapping(context),
                    witness1: contract.inverse(context),
                });
            }
        }
    }
    if definitions.len() > MAX_PLIRON_SEMANTIC_NODES_V1 {
        return one(PlironSemanticRefinementFindingV1::ResourceLimitExceeded);
    }

    let reference_count = reference_requirements.len() + tensor_requirements.len();
    let numerical_count = numerical_requirements.len();
    let authenticated_requirements = reference_requirements
        .iter()
        .map(|&(block, operation, identity, actual, expected)| {
            (block, operation, identity, actual, expected)
        })
        .chain(numerical_requirements.iter().map(
            |&(block, operation, identity, actual, reference, ..)| {
                (block, operation, identity, actual, reference)
            },
        ))
        .chain(tensor_requirements.iter().map(
            |&(block, operation, identity, _, _, actual, reference, _)| {
                (block, operation, identity, actual, reference)
            },
        ))
        .collect::<Vec<_>>();
    let mut findings = Vec::new();
    let mut contract_valid = HashSet::new();
    let mut used_obligations = HashSet::new();
    for (block, operation, identity, _, _) in &authenticated_requirements {
        let identity = identity.unwrap_or([0; 4]);
        let matching = obligations
            .iter()
            .filter(|record| record.2 == Some(identity))
            .collect::<Vec<_>>();
        if matching.len() != 1 {
            push(
                &mut findings,
                if matching.is_empty() {
                    PlironSemanticRefinementFindingV1::ReferenceContractIncomplete {
                        block: *block,
                        operation: *operation,
                        obligation: identity,
                        reason: "the exact proof.obligation record is missing",
                    }
                } else {
                    PlironSemanticRefinementFindingV1::ReferenceContractRejected {
                        block: *block,
                        operation: *operation,
                        obligation: identity,
                        reason: "the proof obligation identity is duplicated",
                    }
                },
            );
            continue;
        }
        let obligation = matching[0];
        used_obligations.insert(identity);
        if obligation.3.is_none() || obligation.4.is_none() {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::ReferenceContractRejected {
                    block: *block,
                    operation: *operation,
                    obligation: identity,
                    reason: "the obligation lacks an exact subject or reference-model identity",
                },
            );
            continue;
        }
        if obligation.5 != Some(PropertyAttr::FunctionalRefinement) {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::ReferenceContractRejected {
                    block: *block,
                    operation: *operation,
                    obligation: identity,
                    reason: "the referenced obligation is not FunctionalRefinement",
                },
            );
            continue;
        }
        let matching_evidence = evidence
            .iter()
            .filter(|record| record.3 == Some(identity))
            .collect::<Vec<_>>();
        if matching_evidence.len() != 1 {
            push(
                &mut findings,
                if matching_evidence.is_empty() {
                    PlironSemanticRefinementFindingV1::ReferenceContractIncomplete {
                        block: *block,
                        operation: *operation,
                        obligation: identity,
                        reason: "the exact proof.evidence_ref record is missing",
                    }
                } else {
                    PlironSemanticRefinementFindingV1::ReferenceContractRejected {
                        block: *block,
                        operation: *operation,
                        obligation: identity,
                        reason: "more than one evidence record claims the obligation",
                    }
                },
            );
            continue;
        }
        let record = matching_evidence[0];
        if record.2.is_none() || record.4 != Some(PropertyAttr::FunctionalRefinement) {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::ReferenceContractRejected {
                    block: *block,
                    operation: *operation,
                    obligation: identity,
                    reason: "the evidence identity or property does not match functional refinement",
                },
            );
            continue;
        }
        if record.5 != Some(EvidenceStatusAttr::Proved) {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::ReferenceContractIncomplete {
                    block: *block,
                    operation: *operation,
                    obligation: identity,
                    reason: "functional refinement requires exact Proved evidence",
                },
            );
            continue;
        }
        if record.6 != Some(CoveredBoundaryAttr::Mir) {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::ReferenceContractIncomplete {
                    block: *block,
                    operation: *operation,
                    obligation: identity,
                    reason: "the Verus reference evidence must cover the exact MIR boundary",
                },
            );
            continue;
        }
        contract_valid.insert((*block, *operation));
    }
    for (block, operation, identity, _, _, property) in &obligations {
        if *property == Some(PropertyAttr::FunctionalRefinement)
            && identity.is_none_or(|identity| {
                !used_obligations.contains(&identity) && !effect_requirement_ids.contains(&identity)
            })
        {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::ReferenceContractIncomplete {
                    block: *block,
                    operation: *operation,
                    obligation: identity.unwrap_or([0; 4]),
                    reason: "functional-refinement proof obligation has no semantic equality",
                },
            );
        }
    }

    let expressions = match SemanticExpressionTableV1::build(context, &definitions) {
        Ok(expressions) => expressions,
        Err(SemanticExpressionBuildErrorV1::ResourceLimit) => {
            return one(PlironSemanticRefinementFindingV1::ResourceLimitExceeded);
        }
        Err(SemanticExpressionBuildErrorV1::InvalidTypedExpression(reason)) => {
            return one(PlironSemanticRefinementFindingV1::TypedExpressionRejected { reason });
        }
    };

    for (block, operation, actual, expected) in requirements {
        let Some(actual_node) = expressions.identity(actual) else {
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
        let Some(expected_node) = expressions.identity(expected) else {
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
                    actual: expressions.describe(actual_node),
                    expected: expressions.describe(expected_node),
                },
            );
        }
    }
    let mut proved_reference_obligations = 0;
    let mut proved_reference_pairs = HashSet::new();
    for (block, operation, _, result_root, _, actual, reference, components) in tensor_requirements
    {
        let actual_fact = expressions.typed_root_fact(actual);
        let reference_fact = expressions.typed_root_fact(reference);
        let valid_aggregate = actual_fact
            .zip(reference_fact)
            .is_some_and(|(actual, reference)| actual == reference);
        let root_component_count = result_root.map_or(0, |root| {
            tensor_components
                .values()
                .filter(|(candidate, ..)| *candidate == Some(root))
                .count()
        });
        let mut seen = HashSet::new();
        let valid_components = result_root.is_some()
            && !components.is_empty()
            && root_component_count == components.len()
            && components
                .iter()
                .enumerate()
                .all(|(ordinal, (gpu, sequential))| {
                    seen.insert(*gpu)
                        && tensor_components.get(gpu).is_some_and(
                            |(component_root, component_ordinal, scalar)| {
                                *component_root == result_root
                                    && *component_ordinal == u32::try_from(ordinal).ok()
                                    && actual_fact.is_some_and(|fact| *scalar == Some(fact.scalar))
                            },
                        )
                        && reference_fact
                            .zip(expressions.typed_root_fact(*sequential))
                            .is_some_and(|(aggregate, component)| aggregate == component)
                });
        if !valid_aggregate || !valid_components {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::ReferenceContractRejected {
                    block,
                    operation,
                    obligation: [0; 4],
                    reason: "tensor refinement lacks an exact result-root/component SSA mapping or compatible typed aggregate roots",
                },
            );
        } else if contract_valid.contains(&(block, operation)) {
            proved_reference_obligations += 1;
            proved_reference_pairs.insert((actual, reference));
        }
    }
    for (block, operation, _, actual, expected) in reference_requirements {
        let Some(actual_node) = expressions.identity(actual) else {
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
        let Some(expected_node) = expressions.identity(expected) else {
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
                    actual: expressions.describe(actual_node),
                    expected: expressions.describe(expected_node),
                },
            );
        } else if contract_valid.contains(&(block, operation)) {
            proved_reference_obligations += 1;
            proved_reference_pairs.insert((actual, expected));
        }
    }
    let mut proved_numerical_obligations = 0;
    for (
        block,
        operation,
        _,
        actual,
        reference,
        domain,
        precondition,
        absolute_error,
        relative_error,
    ) in numerical_requirements
    {
        let Some(actual_fact) = expressions.typed_root_fact(actual) else {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::ReferenceContractIncomplete {
                    block,
                    operation,
                    obligation: [0; 4],
                    reason: "the numerical actual value is not a reconstructed typed root",
                },
            );
            continue;
        };
        let Some(reference_fact) = expressions.typed_root_fact(reference) else {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::ReferenceContractIncomplete {
                    block,
                    operation,
                    obligation: [0; 4],
                    reason: "the numerical reference value is not a reconstructed typed root",
                },
            );
            continue;
        };
        let domain_fact = expressions.typed_root_fact(domain);
        let precondition_fact = expressions.typed_root_fact(precondition);
        if actual_fact != reference_fact || !actual_fact.scalar.is_float() {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::ReferenceContractRejected {
                    block,
                    operation,
                    obligation: [0; 4],
                    reason: "numerical actual and reference roots must share one floating scalar contract",
                },
            );
            continue;
        }
        if domain_fact.is_none_or(|fact| !fact.scalar.is_bool())
            || precondition_fact.is_none_or(|fact| !fact.scalar.is_bool())
        {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::ReferenceContractRejected {
                    block,
                    operation,
                    obligation: [0; 4],
                    reason: "numerical domain and precondition must be typed Boolean roots",
                },
            );
            continue;
        }
        let bounds = absolute_error
            .zip(relative_error)
            .map(|(absolute, relative)| (f64::from_bits(absolute), f64::from_bits(relative)));
        if bounds.is_none_or(|(absolute, relative)| {
            !absolute.is_finite()
                || !relative.is_finite()
                || absolute < 0.0
                || relative < 0.0
                || (absolute == 0.0 && relative == 0.0)
        }) {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::ReferenceContractRejected {
                    block,
                    operation,
                    obligation: [0; 4],
                    reason: "numerical refinement requires finite nonnegative nonzero bounds",
                },
            );
            continue;
        }
        if contract_valid.contains(&(block, operation)) {
            proved_numerical_obligations += 1;
        }
    }
    let collective_count = collective_contracts.len();
    let mut proved_collective_contracts = 0;
    let mut used_reference_pairs = HashSet::new();
    for collective in collective_contracts {
        let block = collective.block;
        let operation = collective.operation;
        let view = collective.view;
        let actual = collective.actual;
        let expected = collective.expected;
        let coverage = collective.coverage;
        let Some(actual_fact) = expressions.typed_root_fact(actual) else {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::CollectiveContractIncomplete {
                    block,
                    operation,
                    reason: "the actual value is not a reconstructed typed semantic root",
                },
            );
            continue;
        };
        let Some(expected_fact) = expressions.typed_root_fact(expected) else {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::CollectiveContractIncomplete {
                    block,
                    operation,
                    reason: "the expected value is not a reconstructed typed semantic root",
                },
            );
            continue;
        };
        let Some(witness0_fact) = expressions.typed_root_fact(collective.witness0) else {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::CollectiveContractIncomplete {
                    block,
                    operation,
                    reason: "the first collective witness is not a reconstructed typed semantic root",
                },
            );
            continue;
        };
        let Some(witness1_fact) = expressions.typed_root_fact(collective.witness1) else {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::CollectiveContractIncomplete {
                    block,
                    operation,
                    reason: "the second collective witness is not a reconstructed typed semantic root",
                },
            );
            continue;
        };
        let Some(numerical_policy) = collective.numerical_policy else {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::CollectiveContractRejected {
                    block,
                    operation,
                    reason: "the declared collective numerical policy is absent",
                },
            );
            continue;
        };
        if actual_fact != expected_fact {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::CollectiveContractRejected {
                    block,
                    operation,
                    reason: "actual and expected roots do not share one scalar and numerical contract",
                },
            );
            continue;
        }
        if actual_fact.contract.policy != numerical_policy {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::CollectiveContractRejected {
                    block,
                    operation,
                    reason: "the declared collective policy does not match the actual and expected typed roots",
                },
            );
            continue;
        }
        match collective.kind {
            CollectiveContractKindV1::Fold | CollectiveContractKindV1::Recurrence => {
                if witness0_fact != actual_fact || witness1_fact != actual_fact {
                    push(
                        &mut findings,
                        PlironSemanticRefinementFindingV1::CollectiveContractRejected {
                            block,
                            operation,
                            reason: "a fold or recurrence witness scalar or numerical contract does not match its result",
                        },
                    );
                    continue;
                }
            }
            CollectiveContractKindV1::Permutation => {
                if witness0_fact != witness1_fact
                    || !witness0_fact.scalar.is_integer()
                    || witness0_fact.contract.policy
                        != SemanticNumericalPolicyAttr::ExactBitVectorOperatorCongruence
                {
                    push(
                        &mut findings,
                        PlironSemanticRefinementFindingV1::CollectiveContractRejected {
                            block,
                            operation,
                            reason: "permutation mapping and inverse must share one integer bitvector contract",
                        },
                    );
                    continue;
                }
            }
        }
        let required_coverage = match coverage {
            Some(SemanticCoverageBindingAttr::TotalView) => OwnershipCoverageAttr::TotalView,
            Some(SemanticCoverageBindingAttr::CollectiveContributions) => {
                OwnershipCoverageAttr::CollectiveContributions
            }
            None => {
                push(
                    &mut findings,
                    PlironSemanticRefinementFindingV1::CollectiveContractRejected {
                        block,
                        operation,
                        reason: "the coverage binding is absent",
                    },
                );
                continue;
            }
        };
        let matching_coverage = ownership_contracts
            .iter()
            .filter(|(candidate_view, _)| *candidate_view == view)
            .collect::<Vec<_>>();
        if matching_coverage.len() != 1 {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::CollectiveContractIncomplete {
                    block,
                    operation,
                    reason: "exactly one independently verified ownership contract is required for the output view",
                },
            );
            continue;
        }
        if matching_coverage[0].1 != Some(required_coverage) {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::CollectiveContractRejected {
                    block,
                    operation,
                    reason: "the output ownership theorem does not match the declared coverage binding",
                },
            );
            continue;
        }
        if !proved_reference_pairs.contains(&(actual, expected)) {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::CollectiveContractIncomplete {
                    block,
                    operation,
                    reason: "coverage never proves a final value; an independently proved MIR functional-refinement equality is required",
                },
            );
            continue;
        }
        if !used_reference_pairs.insert((actual, expected)) {
            push(
                &mut findings,
                PlironSemanticRefinementFindingV1::CollectiveContractRejected {
                    block,
                    operation,
                    reason: "one scalar proof equality cannot discharge multiple finite collective contracts",
                },
            );
            continue;
        }
        proved_collective_contracts += 1;
    }
    let effect_refinement =
        run_pliron_effect_refinement_with_analyses_v1(context, function, analyses);
    let typed_root_commitments = expressions.typed_root_commitments().to_vec();
    PlironSemanticRefinementReportV1 {
        findings,
        reference_obligations: reference_count,
        proved_reference_obligations,
        numerical_obligations: numerical_count,
        proved_numerical_obligations,
        collective_contracts: collective_count,
        proved_collective_contracts,
        typed_root_commitments,
        effect_refinement,
    }
}

pub(crate) fn require_pliron_semantic_refinement_with_analyses_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
) -> Result<PlironSemanticRefinementReportV1, PlironSemanticRefinementCheckErrorV1> {
    let report = run_pliron_semantic_refinement_check_after_bounds_v1(context, function, analyses);
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
        reference_obligations: 0,
        proved_reference_obligations: 0,
        numerical_obligations: 0,
        proved_numerical_obligations: 0,
        collective_contracts: 0,
        proved_collective_contracts: 0,
        typed_root_commitments: Vec::new(),
        effect_refinement: clean_effect_refinement_report_v1(),
    }
}

fn proof_identity(words: [u64; 4]) -> String {
    format!(
        "{:016x}{:016x}{:016x}{:016x}",
        words[0], words[1], words[2], words[3]
    )
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
            reference_obligations: 1,
            proved_reference_obligations: 0,
            numerical_obligations: 0,
            proved_numerical_obligations: 0,
            collective_contracts: 0,
            proved_collective_contracts: 0,
            typed_root_commitments: Vec::new(),
            effect_refinement: clean_effect_refinement_report_v1(),
        };
        assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
        assert!(!report.is_clean());
        assert_eq!(
            PlironSemanticRefinementReportV1 {
                findings: vec![],
                reference_obligations: 0,
                proved_reference_obligations: 0,
                numerical_obligations: 0,
                proved_numerical_obligations: 0,
                collective_contracts: 0,
                proved_collective_contracts: 0,
                typed_root_commitments: Vec::new(),
                effect_refinement: clean_effect_refinement_report_v1(),
            }
            .status(),
            KernelCheckStatusV1::Clean
        );
    }
}
