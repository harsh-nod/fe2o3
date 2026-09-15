//! Closed KernelContext address transparency, never context issuance.

use fe2o3_mir_model::semantic_mir_v1::{
    SemanticCallableDeclV1, SemanticCompilerIntrinsicOperationV1 as I, SemanticDirectCallV1,
    SemanticExecutionCapabilityOperationV1 as E, SemanticFunctionAbiV1, SemanticMutabilityV1,
    SemanticOperandV1, SemanticPointerKindV1, SemanticPointerMetadataV1,
    SemanticSourceArgumentOwnershipV1, SemanticTypeDeclV1, SemanticTypeIdV1, SemanticTypeShapeV1,
};

#[derive(Clone, Copy)]
pub(super) struct KernelContextBorrowV1<'a> {
    reference: SemanticTypeIdV1,
    owned: SemanticTypeIdV1,
    abi: &'a SemanticFunctionAbiV1,
}

impl<'a> KernelContextBorrowV1<'a> {
    /// Callers must select `callable` using the actual call's callee ID. The
    /// admitted source roster authenticates the callable; this adds no issuer.
    pub(super) fn for_callable(
        types: &[SemanticTypeDeclV1],
        callable: &'a SemanticCallableDeclV1,
    ) -> Option<Self> {
        let SemanticCallableDeclV1::CompilerIntrinsic {
            binding, operation, ..
        } = callable
        else {
            return None;
        };
        let abi = binding.abi();
        let &reference = abi.source_input_types().first()?;
        if abi.source_argument_ownership().first()
            != Some(&SemanticSourceArgumentOwnershipV1::SharedBorrow)
            || abi.c_variadic()
            || !abi.hidden_arguments().is_empty()
        {
            return None;
        }
        let SemanticTypeShapeV1::Pointer(pointer) = types.get(reference.index() as usize)?.shape()
        else {
            return None;
        };
        if pointer.kind() != SemanticPointerKindV1::Reference
            || pointer.mutability() != SemanticMutabilityV1::Immutable
            || pointer.metadata() != SemanticPointerMetadataV1::None
        {
            return None;
        }
        let owned = pointer.pointee();
        if owned == reference || types.get(owned.index() as usize).is_none() {
            return None;
        }
        match operation {
            I::ExecutionCapability { contract } => {
                let context = match contract.operation() {
                    E::NumericalPolicyIssue { context, .. }
                    | E::PrivateMemoryAllocate { context, .. } => context,
                    _ => return None,
                };
                if binding.identity() != contract.source_identity()
                    || context != reference
                    || !abi
                        .source_input_types()
                        .iter()
                        .copied()
                        .eq(contract.signature().arguments())
                    || abi.source_output_type() != contract.signature().output()
                {
                    return None;
                }
            }
            I::CapabilityGlobalBindReadOnly {
                context,
                physical,
                view,
                source_identity,
                ..
            }
            | I::CapabilityGlobalBindExclusiveReadWrite {
                context,
                physical,
                view,
                source_identity,
                ..
            }
            | I::CapabilityGlobalBindDisjointWrite {
                context,
                physical,
                view,
                source_identity,
                ..
            } if *context == owned
                && binding.identity() == *source_identity
                && abi.source_input_types() == [reference, *physical]
                && abi.source_output_type() == *view => {}
            _ => return None,
        }
        Some(Self {
            reference,
            owned,
            abi,
        })
    }

    pub(super) const fn reference_pair(self) -> (SemanticTypeIdV1, SemanticTypeIdV1) {
        (self.reference, self.owned)
    }

    pub(super) fn accepts(
        self,
        call: &SemanticDirectCallV1,
        argument: usize,
        source_type: SemanticTypeIdV1,
    ) -> bool {
        argument == 0
            && source_type == self.owned
            && call.variadic_argument_abis().is_empty()
            && call.arguments().iter().map(SemanticOperandV1::ty).eq(self
                .abi
                .source_input_types()
                .iter()
                .copied())
            && call.destination().is_some_and(|destination| {
                destination.place().projections().is_empty()
                    && destination.place().ty() == self.abi.source_output_type()
            })
            && matches!(call.arguments().first(),
                Some(SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place))
                    if place.projections().is_empty() && place.ty() == self.reference)
    }
}

#[cfg(test)]
#[path = "kernel_context_borrows_v1/tests.rs"]
mod tests;
