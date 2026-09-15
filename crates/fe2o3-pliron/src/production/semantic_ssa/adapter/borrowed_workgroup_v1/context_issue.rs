//! Address transparency for the exact mutable KernelContext issuance receiver.

use fe2o3_mir_model::semantic_mir_v1::{
    SemanticCallableDeclV1, SemanticCompilerIntrinsicOperationV1, SemanticDirectCallV1,
    SemanticExecutionCapabilityOperationV1, SemanticFunctionAbiV1, SemanticMutabilityV1,
    SemanticOperandV1, SemanticPointerKindV1, SemanticPointerMetadataV1,
    SemanticSourceArgumentOwnershipV1, SemanticTypeDeclV1, SemanticTypeIdV1, SemanticTypeShapeV1,
};

#[derive(Clone, Copy)]
pub(super) struct WorkgroupContextBorrowV1<'a> {
    reference: SemanticTypeIdV1,
    owned: SemanticTypeIdV1,
    abi: &'a SemanticFunctionAbiV1,
}

impl<'a> WorkgroupContextBorrowV1<'a> {
    pub(super) fn for_callable(
        types: &[SemanticTypeDeclV1],
        callable: &'a SemanticCallableDeclV1,
    ) -> Option<Self> {
        let SemanticCallableDeclV1::CompilerIntrinsic {
            binding,
            operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
            ..
        } = callable
        else {
            return None;
        };
        let SemanticExecutionCapabilityOperationV1::WorkgroupDerive {
            context: reference,
            workgroup,
        } = contract.operation()
        else {
            return None;
        };
        let abi = binding.abi();
        if binding.identity() != contract.source_identity()
            || abi.c_variadic()
            || !abi.hidden_arguments().is_empty()
            || abi.source_input_types() != [reference]
            || abi.source_output_type() != workgroup
            || abi.source_argument_ownership() != [SemanticSourceArgumentOwnershipV1::UniqueBorrow]
            || !contract.signature().arguments().eq([reference])
            || contract.signature().output() != workgroup
            || contract.epoch_before().is_none()
            || contract.workgroup_brand().is_none()
            || contract.epoch_after().is_some()
        {
            return None;
        }
        let SemanticTypeShapeV1::Pointer(pointer) = types.get(reference.index() as usize)?.shape()
        else {
            return None;
        };
        let owned = pointer.pointee();
        if pointer.kind() != SemanticPointerKindV1::Reference
            || pointer.mutability() != SemanticMutabilityV1::Mutable
            || pointer.metadata() != SemanticPointerMetadataV1::None
            || owned == reference
            || types.get(owned.index() as usize).is_none()
            || types.get(workgroup.index() as usize).is_none()
        {
            return None;
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
            && matches!(call.arguments(),
                [SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)]
                    if place.projections().is_empty() && place.ty() == self.reference)
            && call.destination().is_some_and(|destination| {
                destination.place().projections().is_empty()
                    && destination.place().ty() == self.abi.source_output_type()
            })
    }
}
