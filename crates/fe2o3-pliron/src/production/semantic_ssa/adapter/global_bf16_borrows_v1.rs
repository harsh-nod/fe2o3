//! Closed address transparency only. The lowerer's live resolver proves loans.

use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAggregateKindV1, SemanticDirectCallV1, SemanticGlobalBf16MatrixLoadV1,
    SemanticMutabilityV1, SemanticPointerKindV1, SemanticPointerMetadataV1,
    SemanticSourceArgumentOwnershipV1, semantic_global_bf16_matrix_layout_matches_v1,
};

#[derive(Clone, Copy)]
pub(super) struct GlobalBf16BorrowV1 {
    contract: SemanticGlobalBf16MatrixLoadV1,
    references: [SemanticTypeIdV1; 3],
}

impl GlobalBf16BorrowV1 {
    pub(super) fn for_callable(
        types: &[SemanticTypeDeclV1],
        callable: &SemanticCallableDeclV1,
    ) -> Option<Self> {
        let SemanticCallableDeclV1::CompilerIntrinsic {
            binding,
            operation: SemanticCompilerIntrinsicOperationV1::GlobalBf16MatrixLoad { contract },
            ..
        } = callable
        else {
            return None;
        };
        let t = contract.types();
        let abi = binding.abi();
        if binding.identity() != contract.source_identity()
            || !semantic_global_bf16_matrix_layout_matches_v1(types, t)
            || abi.source_output_type() != t.fragment
            || abi.source_input_types().len() != 4
            || abi.source_input_types()[2..] != [t.index; 2]
            || abi.c_variadic()
            || abi.source_argument_ownership()
                != [
                    SemanticSourceArgumentOwnershipV1::SharedBorrow,
                    SemanticSourceArgumentOwnershipV1::SharedBorrow,
                    SemanticSourceArgumentOwnershipV1::ByValue,
                    SemanticSourceArgumentOwnershipV1::ByValue,
                ]
        {
            return None;
        }
        let SemanticTypeShapeV1::Aggregate(fields) = types[t.matrix.index() as usize].shape()
        else {
            return None;
        };
        let references = [
            abi.source_input_types()[0],
            abi.source_input_types()[1],
            fields.fields()[0],
        ];
        let fact = Self {
            contract: *contract,
            references,
        };
        if fact.pairs().into_iter().any(|(reference, owned)| !matches!(
            types.get(reference.index() as usize).map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Pointer(pointer)) if pointer.kind() == SemanticPointerKindV1::Reference
                && pointer.mutability() == SemanticMutabilityV1::Immutable && pointer.pointee() == owned
                && pointer.metadata() == SemanticPointerMetadataV1::None && pointer.address_space() == 0
                && pointer.pointer_width_bits() == 64
        )) { return None; }
        Some(fact)
    }

    pub(super) fn pairs(self) -> [(SemanticTypeIdV1, SemanticTypeIdV1); 3] {
        let t = self.contract.types();
        [
            (self.references[0], t.matrix),
            (self.references[1], t.lane),
            (self.references[2], t.global),
        ]
    }

    pub(super) fn accepts(
        self,
        call: &SemanticDirectCallV1,
        argument: usize,
        owned: SemanticTypeIdV1,
    ) -> bool {
        let t = self.contract.types();
        argument < 2
            && owned == [t.matrix, t.lane][argument]
            && call.variadic_argument_abis().is_empty()
            && call.arguments().iter().map(SemanticOperandV1::ty).eq([
                self.references[0],
                self.references[1],
                t.index,
                t.index,
            ])
            && call.destination().is_some_and(|destination| {
                destination.place().projections().is_empty()
                    && destination.place().ty() == t.fragment
            })
    }

    pub(super) fn captured_reference(
        self,
        statement: &SemanticStatementKindV1,
    ) -> Option<SemanticLocalIdV1> {
        let SemanticStatementKindV1::Assign(assignment) = statement else {
            return None;
        };
        if !assignment.destination().projections().is_empty()
            || assignment.destination().ty() != self.contract.types().matrix
            || assignment.value().result_type() != self.contract.types().matrix
        {
            return None;
        }
        let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
            return None;
        };
        if aggregate.kind() != &SemanticAggregateKindV1::Aggregate
            || aggregate.operands().len() != 8
        {
            return None;
        }
        let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) =
            &aggregate.operands()[0]
        else {
            return None;
        };
        if !place.projections().is_empty() || place.ty() != self.references[2] {
            return None;
        }
        Some(place.local())
    }

    pub(super) fn metadata_reference(
        self,
        statement: &SemanticStatementKindV1,
    ) -> Option<SemanticLocalIdV1> {
        let SemanticStatementKindV1::Assign(assignment) = statement else {
            return None;
        };
        if !assignment.destination().projections().is_empty() {
            return None;
        }
        let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) = assignment.value().kind()
        else {
            return None;
        };
        let [deref, field] = place.projections() else {
            return None;
        };
        if deref.kind() != SemanticProjectionKindV1::Dereference
            || deref.result_type() != self.contract.types().global
            || field.kind() != SemanticProjectionKindV1::Field(0)
        {
            return None;
        }
        Some(place.local())
    }
}
