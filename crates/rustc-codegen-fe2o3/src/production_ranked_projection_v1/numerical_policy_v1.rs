//! Numerical issuance is logical transport, not a ranked memory effect.
//!
//! Check its exact source/root contract and dominating shared context borrow.
//! The unchanged semantic module (including ordinary policy-bind aggregates)
//! still goes through SSA and canonical lowering. This projection neither
//! erases policy operands nor grants a numerical refinement certificate.

use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticExecutionCapabilityContractV1, SemanticExecutionCapabilityOperationV1,
};

mod legacy_math;

fn issuance(
    callable: &SemanticCallableDeclV1,
) -> Option<(
    SemanticFunctionIdentityV1,
    SemanticExecutionCapabilityContractV1,
)> {
    match callable {
        SemanticCallableDeclV1::CompilerIntrinsic {
            binding,
            operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
            ..
        } if matches!(
            contract.operation(),
            SemanticExecutionCapabilityOperationV1::NumericalPolicyIssue { .. }
        ) =>
        {
            Some((binding.identity(), *contract))
        }
        _ => None,
    }
}

fn context_pointee(
    types: &[SemanticTypeDeclV1],
    reference: SemanticTypeIdV1,
) -> Option<SemanticTypeIdV1> {
    let SemanticTypeShapeV1::Pointer(pointer) = types.get(reference.index() as usize)?.shape()
    else {
        return None;
    };
    (pointer.kind() == SemanticPointerKindV1::Reference
        && pointer.mutability() == SemanticMutabilityV1::Immutable
        && pointer.metadata() == SemanticPointerMetadataV1::None)
        .then_some(pointer.pointee())
}

fn incomplete(detail: &'static str) -> ProductionRankedProjectionErrorV1 {
    ProductionRankedProjectionErrorV1::Incomplete(detail)
}

pub(super) fn validate_roots(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    root: SemanticFunctionIdV1,
    kernel_binding: SemanticKernelBindingIdentityV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let calls = || {
        function.blocks().iter().filter_map(|block| {
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                return None;
            };
            Some((call, callables.get(call.callee().index() as usize)?))
        })
    };
    if !calls().any(|(_, callable)| issuance(callable).is_some()) {
        return Ok(());
    }
    let policies = calls()
        .filter_map(|(_, callable)| {
            let (_, contract) = issuance(callable)?;
            let SemanticExecutionCapabilityOperationV1::NumericalPolicyIssue { capability, .. } =
                contract.operation()
            else {
                return None;
            };
            Some(capability)
        })
        .collect::<Vec<_>>();
    for (_, callable) in calls() {
        if let SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::MathF32 { context, .. },
            ..
        } = callable
            && !legacy_math::policy_free_context(types, *context, &policies)
        {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "policy-bound FP consumer requires a policy-bearing semantic operation and numerical refinement",
            ));
        }
    }
    let mut issued_context = None;
    for (call, callable) in calls() {
        if let SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::KernelContextIssue { context },
            ..
        } = callable
            && (issued_context.replace(*context).is_some()
                || !call.arguments().is_empty()
                || !call.destination().is_some_and(|destination| {
                    destination.place().projections().is_empty()
                        && destination.place().ty() == *context
                }))
        {
            return Err(incomplete(
                "numerical-policy root has duplicate or malformed KernelContext issuance",
            ));
        }
    }
    let mut provenance = None;
    for (call, callable) in calls() {
        let Some((source, contract)) = issuance(callable) else {
            continue;
        };
        let SemanticExecutionCapabilityOperationV1::NumericalPolicyIssue {
            context,
            capability,
            policy,
        } = contract.operation()
        else {
            unreachable!("issuance filters the operation")
        };
        let pointee = context_pointee(types, context).ok_or_else(|| {
            incomplete("numerical-policy source receiver is not an exact typed shared reference")
        })?;
        if issued_context.is_none() {
            return Err(incomplete(
                "numerical-policy expanded body has no retained KernelContext issuer",
            ));
        }
        if issued_context != Some(pointee) {
            return Err(incomplete(
                "numerical-policy receiver pointee differs from the retained root KernelContext issuer",
            ));
        }
        if contract.source_identity() != source {
            return Err(incomplete(
                "numerical-policy source identity differs from its callable binding",
            ));
        }
        if contract.provenance().root() != root
            || contract.provenance().kernel_binding() != kernel_binding
            || provenance.is_some_and(|previous| previous != contract.provenance())
        {
            return Err(incomplete(
                "numerical-policy source provenance differs from the selected kernel root",
            ));
        }
        if [context, pointee, capability].iter().any(|ty| {
            types
                .get(ty.index() as usize)
                .is_none_or(|ty| ty.identity() == policy)
        }) {
            return Err(incomplete(
                "numerical-policy marker aliases its context or capability type",
            ));
        }
        if !contract.signature().arguments().eq([context])
            || contract.signature().output() != capability
            || !matches!(call.arguments(), [receiver] if receiver.ty() == context)
            || !call.destination().is_some_and(|destination| {
                destination.place().projections().is_empty()
                    && destination.place().ty() == capability
            })
            || matches!(call.unwind(), SemanticUnwindActionV1::Cleanup(_))
        {
            return Err(incomplete(
                "numerical-policy call signature, receiver, destination or unwind differs from its exact source contract",
            ));
        }
        provenance = Some(contract.provenance());
    }
    // Other root-bound operations must agree on every provenance axis, not
    // merely on a layout-equivalent context or the root function number.
    for (_, callable) in calls() {
        let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = callable else {
            continue;
        };
        let other = match operation {
            SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract } => {
                Some(contract.provenance())
            }
            SemanticCompilerIntrinsicOperationV1::CapabilityGlobalBindReadOnly {
                provenance,
                ..
            }
            | SemanticCompilerIntrinsicOperationV1::CapabilityGlobalBindExclusiveReadWrite {
                provenance,
                ..
            }
            | SemanticCompilerIntrinsicOperationV1::CapabilityGlobalBindDisjointWrite {
                provenance,
                ..
            } => Some(*provenance),
            _ => None,
        };
        if other.is_some_and(|other| Some(other) != provenance) {
            return Err(incomplete(
                "numerical-policy and another root-bound operation disagree on source provenance",
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_receiver(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    call: &SemanticDirectCallV1,
    state: &ProjectedCapabilityStateV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let Some((_, contract)) = callables
        .get(call.callee().index() as usize)
        .and_then(issuance)
    else {
        return Ok(());
    };
    let SemanticExecutionCapabilityOperationV1::NumericalPolicyIssue { context, .. } =
        contract.operation()
    else {
        unreachable!("issuance filters the operation")
    };
    let pointee = context_pointee(types, context).ok_or_else(|| {
        incomplete("numerical-policy source receiver is not an exact typed shared reference")
    })?;
    if !matches!(call.arguments(), [receiver]
    if receiver.ty() == context
        && capability_known_origin_v1(state, receiver)
            == Some(ProjectedCapabilityOriginV1::KernelContext {
                context: pointee,
                borrow: context_borrow_v1::Borrow::Shared,
            }))
    {
        return Err(incomplete(
            "numerical-policy receiver lacks the dominating retained shared KernelContext origin",
        ));
    }
    Ok(())
}
