//! Closed workload-neutral contracts for bounded collective semantics.

use std::{error::Error, fmt};

use pliron::{
    builtin::{
        ATTR_KEY_DEBUG_INFO,
        op_interfaces::{NOpdsInterface, NRegionsInterface, NResultsInterface},
    },
    common_traits::Verify,
    context::Context,
    derive::{pliron_attr, pliron_op},
    op::Op,
    operation::Operation,
    result::Result as PlironResult,
    value::Value,
    verify_err,
};

use crate::{SemanticExpressionCommitmentAttr, SemanticTypedExpressionRootOp, ranked_view_type};

/// Hard limit on a declared finite semantic domain or execution bound.
pub const MAX_COLLECTIVE_SEMANTIC_STEPS_V1: u64 = 1 << 24;

#[pliron_attr(
    name = "kernel.semantic_domain_bound",
    format = "$0",
    verifier = "succ"
)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SemanticDomainBoundAttr(pub u64);

#[pliron_attr(name = "kernel.semantic_step_bound", format = "$0", verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SemanticStepBoundAttr(pub u64);

#[pliron_attr(name = "kernel.semantic_evaluation_order", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SemanticEvaluationOrderAttr {
    Ascending,
    Descending,
    Lexicographic,
    Explicit,
}

/// Only exact operator-congruence policies are admitted by this V1 dialect.
#[pliron_attr(name = "kernel.semantic_numerical_policy", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SemanticNumericalPolicyAttr {
    ExactBitVectorOperatorCongruence,
    ExactIeeeNearestTiesToEvenPreserveBits,
}

/// Coverage theorem that must independently hold for the contract's output.
#[pliron_attr(name = "kernel.semantic_coverage_binding", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SemanticCoverageBindingAttr {
    TotalView,
    CollectiveContributions,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollectiveSemanticContractError {
    MalformedOperation,
    ForeignTypedRootOperand { operand: usize },
    ZeroOrExcessiveDomainBound,
    InvalidTerminationBound,
}

impl fmt::Display for CollectiveSemanticContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedOperation => formatter.write_str(
                "collective semantic operation has a malformed closed payload",
            ),
            Self::ForeignTypedRootOperand { operand } => write!(
                formatter,
                "collective semantic operand {operand} is not defined by kernel.semantic_typed_root",
            ),
            Self::ZeroOrExcessiveDomainBound => write!(
                formatter,
                "collective semantic domain bound must be in 1..={MAX_COLLECTIVE_SEMANTIC_STEPS_V1}",
            ),
            Self::InvalidTerminationBound => formatter.write_str(
                "collective semantic step bound must be nonzero and no greater than the domain bound",
            ),
        }
    }
}

impl Error for CollectiveSemanticContractError {}

#[pliron_op(
    name = "kernel.require_finite_fold",
    format,
    interfaces = [NOpdsInterface<5>, NResultsInterface<0>, NRegionsInterface<0>],
    attributes = (
        kernel_fold_contract_identity: SemanticExpressionCommitmentAttr,
        kernel_fold_domain_identity: SemanticExpressionCommitmentAttr,
        kernel_fold_domain_bound: SemanticDomainBoundAttr,
        kernel_fold_step_bound: SemanticStepBoundAttr,
        kernel_fold_evaluation_order: SemanticEvaluationOrderAttr,
        kernel_fold_numerical_policy: SemanticNumericalPolicyAttr,
        kernel_fold_coverage_binding: SemanticCoverageBindingAttr
    )
)]
pub struct RequireFiniteFoldOp;

impl RequireFiniteFoldOp {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        context: &mut Context,
        view: Value,
        actual: Value,
        expected: Value,
        identity: Value,
        operator: Value,
        contract_identity: SemanticExpressionCommitmentAttr,
        domain_identity: SemanticExpressionCommitmentAttr,
        domain_bound: u64,
        step_bound: u64,
        order: SemanticEvaluationOrderAttr,
        numerical_policy: SemanticNumericalPolicyAttr,
        coverage: SemanticCoverageBindingAttr,
    ) -> Self {
        let op = Self::from_operation(Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![],
            vec![view, actual, expected, identity, operator],
            vec![],
            0,
        ));
        op.set_attr_kernel_fold_contract_identity(context, contract_identity);
        op.set_attr_kernel_fold_domain_identity(context, domain_identity);
        op.set_attr_kernel_fold_domain_bound(context, SemanticDomainBoundAttr(domain_bound));
        op.set_attr_kernel_fold_step_bound(context, SemanticStepBoundAttr(step_bound));
        op.set_attr_kernel_fold_evaluation_order(context, order);
        op.set_attr_kernel_fold_numerical_policy(context, numerical_policy);
        op.set_attr_kernel_fold_coverage_binding(context, coverage);
        op
    }

    pub fn view(&self, context: &Context) -> Value {
        operand(self, context, 0)
    }
    pub fn actual(&self, context: &Context) -> Value {
        operand(self, context, 1)
    }
    pub fn expected(&self, context: &Context) -> Value {
        operand(self, context, 2)
    }
    pub fn identity(&self, context: &Context) -> Value {
        operand(self, context, 3)
    }
    pub fn operator(&self, context: &Context) -> Value {
        operand(self, context, 4)
    }
    pub fn domain_bound(&self, context: &Context) -> Option<u64> {
        self.get_attr_kernel_fold_domain_bound(context)
            .map(|value| value.0)
    }
    pub fn step_bound(&self, context: &Context) -> Option<u64> {
        self.get_attr_kernel_fold_step_bound(context)
            .map(|value| value.0)
    }
    pub fn coverage(&self, context: &Context) -> Option<SemanticCoverageBindingAttr> {
        self.get_attr_kernel_fold_coverage_binding(context)
            .map(|value| *value)
    }
    pub fn numerical_policy(&self, context: &Context) -> Option<SemanticNumericalPolicyAttr> {
        self.get_attr_kernel_fold_numerical_policy(context)
            .map(|value| *value)
    }
}

impl Verify for RequireFiniteFoldOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        verify_contract(self, context, 5, FOLD_ATTRIBUTES)?;
        verify_common_operands(self, context, &[1, 2, 3, 4])?;
        verify_bounds(
            self,
            context,
            self.domain_bound(context),
            self.step_bound(context),
        )
    }
}

#[pliron_op(
    name = "kernel.require_finite_recurrence",
    format,
    interfaces = [NOpdsInterface<5>, NResultsInterface<0>, NRegionsInterface<0>],
    attributes = (
        kernel_recurrence_contract_identity: SemanticExpressionCommitmentAttr,
        kernel_recurrence_domain_identity: SemanticExpressionCommitmentAttr,
        kernel_recurrence_domain_bound: SemanticDomainBoundAttr,
        kernel_recurrence_step_bound: SemanticStepBoundAttr,
        kernel_recurrence_evaluation_order: SemanticEvaluationOrderAttr,
        kernel_recurrence_numerical_policy: SemanticNumericalPolicyAttr,
        kernel_recurrence_coverage_binding: SemanticCoverageBindingAttr
    )
)]
pub struct RequireFiniteRecurrenceOp;

impl RequireFiniteRecurrenceOp {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        context: &mut Context,
        view: Value,
        actual: Value,
        expected: Value,
        initial: Value,
        transition: Value,
        contract_identity: SemanticExpressionCommitmentAttr,
        domain_identity: SemanticExpressionCommitmentAttr,
        domain_bound: u64,
        step_bound: u64,
        order: SemanticEvaluationOrderAttr,
        numerical_policy: SemanticNumericalPolicyAttr,
        coverage: SemanticCoverageBindingAttr,
    ) -> Self {
        let op = Self::from_operation(Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![],
            vec![view, actual, expected, initial, transition],
            vec![],
            0,
        ));
        op.set_attr_kernel_recurrence_contract_identity(context, contract_identity);
        op.set_attr_kernel_recurrence_domain_identity(context, domain_identity);
        op.set_attr_kernel_recurrence_domain_bound(context, SemanticDomainBoundAttr(domain_bound));
        op.set_attr_kernel_recurrence_step_bound(context, SemanticStepBoundAttr(step_bound));
        op.set_attr_kernel_recurrence_evaluation_order(context, order);
        op.set_attr_kernel_recurrence_numerical_policy(context, numerical_policy);
        op.set_attr_kernel_recurrence_coverage_binding(context, coverage);
        op
    }

    pub fn view(&self, context: &Context) -> Value {
        operand(self, context, 0)
    }
    pub fn actual(&self, context: &Context) -> Value {
        operand(self, context, 1)
    }
    pub fn expected(&self, context: &Context) -> Value {
        operand(self, context, 2)
    }
    pub fn initial(&self, context: &Context) -> Value {
        operand(self, context, 3)
    }
    pub fn transition(&self, context: &Context) -> Value {
        operand(self, context, 4)
    }
    pub fn domain_bound(&self, context: &Context) -> Option<u64> {
        self.get_attr_kernel_recurrence_domain_bound(context)
            .map(|value| value.0)
    }
    pub fn step_bound(&self, context: &Context) -> Option<u64> {
        self.get_attr_kernel_recurrence_step_bound(context)
            .map(|value| value.0)
    }
    pub fn coverage(&self, context: &Context) -> Option<SemanticCoverageBindingAttr> {
        self.get_attr_kernel_recurrence_coverage_binding(context)
            .map(|value| *value)
    }
    pub fn numerical_policy(&self, context: &Context) -> Option<SemanticNumericalPolicyAttr> {
        self.get_attr_kernel_recurrence_numerical_policy(context)
            .map(|value| *value)
    }
}

impl Verify for RequireFiniteRecurrenceOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        verify_contract(self, context, 5, RECURRENCE_ATTRIBUTES)?;
        verify_common_operands(self, context, &[1, 2, 3, 4])?;
        verify_bounds(
            self,
            context,
            self.domain_bound(context),
            self.step_bound(context),
        )
    }
}

#[pliron_op(
    name = "kernel.require_permutation_gather",
    format,
    interfaces = [NOpdsInterface<5>, NResultsInterface<0>, NRegionsInterface<0>],
    attributes = (
        kernel_permutation_contract_identity: SemanticExpressionCommitmentAttr,
        kernel_permutation_source_domain_identity: SemanticExpressionCommitmentAttr,
        kernel_permutation_target_domain_identity: SemanticExpressionCommitmentAttr,
        kernel_permutation_domain_bound: SemanticDomainBoundAttr,
        kernel_permutation_step_bound: SemanticStepBoundAttr,
        kernel_permutation_evaluation_order: SemanticEvaluationOrderAttr,
        kernel_permutation_numerical_policy: SemanticNumericalPolicyAttr,
        kernel_permutation_coverage_binding: SemanticCoverageBindingAttr
    )
)]
pub struct RequirePermutationGatherOp;

impl RequirePermutationGatherOp {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        context: &mut Context,
        view: Value,
        actual: Value,
        expected: Value,
        mapping: Value,
        inverse: Value,
        contract_identity: SemanticExpressionCommitmentAttr,
        source_domain_identity: SemanticExpressionCommitmentAttr,
        target_domain_identity: SemanticExpressionCommitmentAttr,
        domain_bound: u64,
        step_bound: u64,
        order: SemanticEvaluationOrderAttr,
        numerical_policy: SemanticNumericalPolicyAttr,
        coverage: SemanticCoverageBindingAttr,
    ) -> Self {
        let op = Self::from_operation(Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![],
            vec![view, actual, expected, mapping, inverse],
            vec![],
            0,
        ));
        op.set_attr_kernel_permutation_contract_identity(context, contract_identity);
        op.set_attr_kernel_permutation_source_domain_identity(context, source_domain_identity);
        op.set_attr_kernel_permutation_target_domain_identity(context, target_domain_identity);
        op.set_attr_kernel_permutation_domain_bound(context, SemanticDomainBoundAttr(domain_bound));
        op.set_attr_kernel_permutation_step_bound(context, SemanticStepBoundAttr(step_bound));
        op.set_attr_kernel_permutation_evaluation_order(context, order);
        op.set_attr_kernel_permutation_numerical_policy(context, numerical_policy);
        op.set_attr_kernel_permutation_coverage_binding(context, coverage);
        op
    }

    pub fn view(&self, context: &Context) -> Value {
        operand(self, context, 0)
    }
    pub fn actual(&self, context: &Context) -> Value {
        operand(self, context, 1)
    }
    pub fn expected(&self, context: &Context) -> Value {
        operand(self, context, 2)
    }
    pub fn mapping(&self, context: &Context) -> Value {
        operand(self, context, 3)
    }
    pub fn inverse(&self, context: &Context) -> Value {
        operand(self, context, 4)
    }
    pub fn domain_bound(&self, context: &Context) -> Option<u64> {
        self.get_attr_kernel_permutation_domain_bound(context)
            .map(|value| value.0)
    }
    pub fn step_bound(&self, context: &Context) -> Option<u64> {
        self.get_attr_kernel_permutation_step_bound(context)
            .map(|value| value.0)
    }
    pub fn coverage(&self, context: &Context) -> Option<SemanticCoverageBindingAttr> {
        self.get_attr_kernel_permutation_coverage_binding(context)
            .map(|value| *value)
    }
    pub fn numerical_policy(&self, context: &Context) -> Option<SemanticNumericalPolicyAttr> {
        self.get_attr_kernel_permutation_numerical_policy(context)
            .map(|value| *value)
    }
}

impl Verify for RequirePermutationGatherOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        verify_contract(self, context, 5, PERMUTATION_ATTRIBUTES)?;
        verify_common_operands(self, context, &[1, 2, 3, 4])?;
        verify_bounds(
            self,
            context,
            self.domain_bound(context),
            self.step_bound(context),
        )
    }
}

const FOLD_ATTRIBUTES: &[&str] = &[
    "kernel_fold_contract_identity",
    "kernel_fold_domain_identity",
    "kernel_fold_domain_bound",
    "kernel_fold_step_bound",
    "kernel_fold_evaluation_order",
    "kernel_fold_numerical_policy",
    "kernel_fold_coverage_binding",
];

const RECURRENCE_ATTRIBUTES: &[&str] = &[
    "kernel_recurrence_contract_identity",
    "kernel_recurrence_domain_identity",
    "kernel_recurrence_domain_bound",
    "kernel_recurrence_step_bound",
    "kernel_recurrence_evaluation_order",
    "kernel_recurrence_numerical_policy",
    "kernel_recurrence_coverage_binding",
];

const PERMUTATION_ATTRIBUTES: &[&str] = &[
    "kernel_permutation_contract_identity",
    "kernel_permutation_source_domain_identity",
    "kernel_permutation_target_domain_identity",
    "kernel_permutation_domain_bound",
    "kernel_permutation_step_bound",
    "kernel_permutation_evaluation_order",
    "kernel_permutation_numerical_policy",
    "kernel_permutation_coverage_binding",
];

fn operand(operation: &dyn Op, context: &Context, index: usize) -> Value {
    operation.get_operation().deref(context).get_operand(index)
}

fn verify_contract(
    operation: &dyn Op,
    context: &Context,
    operand_count: usize,
    allowed_attributes: &[&str],
) -> PlironResult<()> {
    let raw = operation.get_operation().deref(context);
    let attributes_are_closed = raw
        .attributes
        .0
        .keys()
        .all(|key| key == &*ATTR_KEY_DEBUG_INFO || allowed_attributes.contains(&key.as_ref()));
    if raw.get_num_operands() != operand_count
        || raw.get_num_results() != 0
        || raw.get_num_successors() != 0
        || raw.num_regions() != 0
        || raw
            .attributes
            .0
            .keys()
            .filter(|key| *key != &*ATTR_KEY_DEBUG_INFO)
            .count()
            != allowed_attributes.len()
        || !attributes_are_closed
        || ranked_view_type(raw.get_operand(0), context).is_none()
    {
        return verify_err!(
            operation.loc(context),
            CollectiveSemanticContractError::MalformedOperation
        );
    }
    Ok(())
}

fn verify_common_operands(
    operation: &dyn Op,
    context: &Context,
    commitment_operands: &[usize],
) -> PlironResult<()> {
    for operand_index in commitment_operands {
        let value = operand(operation, context, *operand_index);
        let is_typed_root = value.defining_op().is_some_and(|definition| {
            Operation::get_op_dyn(definition, context)
                .downcast_ref::<SemanticTypedExpressionRootOp>()
                .is_some()
        });
        if !is_typed_root {
            return verify_err!(
                operation.loc(context),
                CollectiveSemanticContractError::ForeignTypedRootOperand {
                    operand: *operand_index,
                }
            );
        }
    }
    Ok(())
}

fn verify_bounds(
    operation: &dyn Op,
    context: &Context,
    domain_bound: Option<u64>,
    step_bound: Option<u64>,
) -> PlironResult<()> {
    let Some(domain_bound) = domain_bound else {
        return verify_err!(
            operation.loc(context),
            CollectiveSemanticContractError::MalformedOperation
        );
    };
    if domain_bound == 0 || domain_bound > MAX_COLLECTIVE_SEMANTIC_STEPS_V1 {
        return verify_err!(
            operation.loc(context),
            CollectiveSemanticContractError::ZeroOrExcessiveDomainBound
        );
    }
    let Some(step_bound) = step_bound else {
        return verify_err!(
            operation.loc(context),
            CollectiveSemanticContractError::MalformedOperation
        );
    };
    if step_bound == 0 || step_bound > domain_bound {
        return verify_err!(
            operation.loc(context),
            CollectiveSemanticContractError::InvalidTerminationBound
        );
    }
    Ok(())
}
