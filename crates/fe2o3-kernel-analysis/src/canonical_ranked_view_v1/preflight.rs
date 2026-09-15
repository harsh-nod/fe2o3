//! Inert write inspection. This type deliberately has no materialize or
//! blueprint-extraction API; absence of write contracts remains terminal.

use super::*;
use dialect_kernel::{
    MAX_SEMANTIC_TYPED_EXPRESSION_DEPTH_V1, MAX_SEMANTIC_TYPED_EXPRESSION_NODES_V1,
    SemanticExceptionalValueAttr, SemanticIeeeRoundingAttr, SemanticNumericalContractV1,
    SemanticNumericalPolicyAttr, SemanticTypedExpressionV1,
};

#[derive(Debug)]
pub struct CanonicalRankedViewPreflightV1 {
    canonical_bytes: Box<[u8]>,
    function: Function,
    plan: CanonicalRankedViewBlueprintV1,
    write_rhs: Box<[SemanticTypedExpressionV1]>,
}

impl CanonicalRankedViewPreflightV1 {
    pub fn function(&self) -> &Function {
        &self.function
    }
    pub fn canonical_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV13 {
        &self.plan.canonical
    }
    pub fn final_epoch(&self) -> u64 {
        self.plan.epoch
    }
    pub fn writes(&self) -> &[CanonicalRankedWriteBindingV1] {
        &self.plan.writes
    }
    pub fn write_rhs(&self, ordinal: usize) -> Option<&SemanticTypedExpressionV1> {
        self.write_rhs.get(ordinal)
    }

    /// The actual canonical entry parameter, not its physical ordinal and not
    /// a parameter from another source/final function with coincident SSA IDs.
    pub fn write_parameter(&self, ordinal: usize) -> Option<ValueId> {
        let write = self.writes().get(ordinal)?;
        self.function
            .body
            .as_ref()?
            .parameters
            .get(write.parameter)
            .copied()
    }

    /// Slice ABI lengths remain dynamic. Source ownership and launch extents
    /// cannot specialize this shape without an independent extent binding.
    pub fn write_view_shape(&self, ordinal: usize) -> Option<&[u64]> {
        let write = self.writes().get(ordinal)?;
        matches!(
            self.function.signature.parameters.get(write.parameter)?,
            Type::Slice(_)
        )
        .then_some(&[DYNAMIC_EXTENT][..])
    }

    pub fn numerical_contract(&self) -> SemanticNumericalContractV1 {
        f32_numerical_contract()
    }

    pub fn require_exact(
        &self,
        canonical: &VerifiedCanonicalKernelIrV13,
        epoch: u64,
        function: &FunctionId,
    ) -> Result<(), CanonicalRankedViewErrorV1> {
        if canonical.canonical_bytes() != self.canonical_bytes.as_ref()
            || canonical.identity() != &self.plan.canonical
            || epoch != self.plan.epoch
            || function != &self.plan.function
        {
            return Err(CanonicalRankedViewErrorV1::CanonicalSubjectChanged);
        }
        Ok(())
    }

    pub const fn grants_graph_verification_authority(&self) -> bool {
        false
    }
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

pub fn preflight_canonical_ranked_view_v1(
    canonical: &VerifiedCanonicalKernelIrV13,
    final_epoch: u64,
    function_id: &FunctionId,
) -> Result<CanonicalRankedViewPreflightV1, CanonicalRankedViewErrorV1> {
    let plan = plan_canonical_ranked_view(canonical, final_epoch, function_id)?;
    let module = decode_module_v13(canonical.canonical_bytes())
        .map_err(|_| CanonicalRankedViewErrorV1::CanonicalDecode)?;
    let function = module
        .functions
        .into_iter()
        .find(|f| &f.id == function_id)
        .ok_or(CanonicalRankedViewErrorV1::MissingFunction)?;
    // Expand only actual write producers and charge before every recursive
    // copy. Sharing in the SSA DAG must not cause unbounded tree expansion.
    let definitions = plan
        .blocks
        .iter()
        .flat_map(|b| &b.operations)
        .filter_map(|op| match op {
            PlannedOp::F32 { result, .. }
            | PlannedOp::F32Parameter { result, .. }
            | PlannedOp::F32Binary { result, .. } => Some((result.0, op)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let mut budget = MAX_CANONICAL_RANKED_SOURCE_VALUES_V1;
    let write_rhs = plan
        .writes
        .iter()
        .map(|write| {
            let mut root_budget = MAX_SEMANTIC_TYPED_EXPRESSION_NODES_V1;
            let expression = expand_rhs(
                write.rhs_node,
                &definitions,
                1,
                &mut root_budget,
                &mut budget,
            )?;
            expression.validate().map_err(|_| {
                unsupported(
                    Some(write.source_block),
                    Some(write.source_operation),
                    "invalid typed write RHS",
                )
            })?;
            f32_numerical_contract()
                .validate(&expression)
                .map_err(|_| {
                    unsupported(
                        Some(write.source_block),
                        Some(write.source_operation),
                        "invalid numerical write RHS",
                    )
                })?;
            Ok(expression)
        })
        .collect::<Result<Vec<_>, CanonicalRankedViewErrorV1>>()?
        .into_boxed_slice();
    Ok(CanonicalRankedViewPreflightV1 {
        canonical_bytes: canonical.canonical_bytes().into(),
        function,
        plan,
        write_rhs,
    })
}

fn f32_numerical_contract() -> SemanticNumericalContractV1 {
    SemanticNumericalContractV1 {
        policy: SemanticNumericalPolicyAttr::ExactIeeeNearestTiesToEvenPreserveBits,
        rounding: SemanticIeeeRoundingAttr::NearestTiesToEven,
        exceptional_values: SemanticExceptionalValueAttr::PreserveExactBits,
    }
}

fn expand_rhs(
    node: Node,
    definitions: &BTreeMap<usize, &PlannedOp>,
    depth: usize,
    root_budget: &mut usize,
    budget: &mut usize,
) -> Result<SemanticTypedExpressionV1, CanonicalRankedViewErrorV1> {
    if depth > MAX_SEMANTIC_TYPED_EXPRESSION_DEPTH_V1 || *root_budget == 0 || *budget == 0 {
        return Err(unsupported(None, None, "typed write RHS expansion limit"));
    }
    *root_budget -= 1;
    *budget -= 1;
    let scalar = SemanticTypedScalarV1::new(SemanticScalarKindAttr::Float, 32)
        .ok_or_else(|| unsupported(None, None, "f32 scalar"))?;
    Ok(match definitions.get(&node.0).copied() {
        Some(PlannedOp::F32 { bits, .. }) => SemanticTypedExpressionV1::Constant {
            scalar,
            bits: u64::from(*bits),
        },
        Some(PlannedOp::F32Parameter { parameter, .. }) => SemanticTypedExpressionV1::Symbol {
            symbol: *parameter,
            scalar,
        },
        Some(PlannedOp::F32Binary { kind, lhs, rhs, .. }) => SemanticTypedExpressionV1::Binary {
            operation: *kind,
            scalar,
            overflow: SemanticOverflowAttr::Wrapping,
            lhs: Box::new(expand_rhs(
                *lhs,
                definitions,
                depth + 1,
                root_budget,
                budget,
            )?),
            rhs: Box::new(expand_rhs(
                *rhs,
                definitions,
                depth + 1,
                root_budget,
                budget,
            )?),
        },
        _ => return Err(unsupported(None, None, "unbound typed write RHS")),
    })
}
