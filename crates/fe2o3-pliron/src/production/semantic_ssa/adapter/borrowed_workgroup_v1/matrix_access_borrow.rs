//! Exact source-reference consumption, not a Matrix or Subgroup issuer proof.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticDirectCallV1, SemanticFunctionAbiV1, SemanticMutabilityV1, SemanticScalarTypeV1,
    SemanticPointerKindV1, SemanticPointerMetadataV1, SemanticSourceArgumentOwnershipV1,
    SemanticTypeLayoutDetailsV1, SemanticTypeShapeV1,
};

#[derive(Clone, Copy)]
pub(super) struct MatrixAccessBorrow<'a> {
    pairs: [(SemanticTypeIdV1, SemanticTypeIdV1); 2],
    lane: SemanticTypeIdV1,
    abi: &'a SemanticFunctionAbiV1,
}

impl<'a> MatrixAccessBorrow<'a> {
    pub(super) fn for_callable(
        types: &[SemanticTypeDeclV1],
        callable: &'a SemanticCallableDeclV1,
    ) -> Option<Self> {
        let SemanticCallableDeclV1::CompilerIntrinsic {
            binding,
            operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
            ..
        } = callable else { return None };
        let E::MatrixAccess { subgroup, epoch, matrix, width, .. } = contract.operation()
        else { return None };
        let abi = binding.abi();
        if binding.identity() != contract.source_identity()
            || width != 64
            || abi.c_variadic() || !abi.hidden_arguments().is_empty()
            || abi.source_input_types() != [subgroup, epoch]
            || abi.source_output_type() != matrix
            || abi.source_argument_ownership() != [SemanticSourceArgumentOwnershipV1::SharedBorrow; 2]
            || !contract.signature().arguments().eq([subgroup, epoch])
            || contract.signature().output() != matrix
            || contract.workgroup_brand().is_none() || contract.epoch_before().is_none()
            || contract.epoch_after().is_some()
            || !zst(types, matrix)
        { return None }
        let owned = shared_pointee(types, subgroup)?;
        let epoch_owned = shared_pointee(types, epoch)?;
        let [lane, brand, marker] = fields(types, owned, &[0, 4, 4])? else { return None };
        let [rank, width_marker, lane_brand, lane_marker] = fields(types, *lane, &[0, 4, 4, 4])?
        else { return None };
        if !matches!(types.get(rank.index() as usize)?.shape(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed: false, bits: 32 }))
            || ![epoch_owned, *brand, *marker, *width_marker, *lane_brand, *lane_marker]
                .into_iter().all(|ty| zst(types, ty))
        { return None }
        Some(Self { pairs: [(subgroup, owned), (epoch, epoch_owned)], lane: *lane, abi })
    }

    pub(super) fn pairs(self) -> [(SemanticTypeIdV1, SemanticTypeIdV1); 2] { self.pairs }
    pub(super) fn lane(self) -> SemanticTypeIdV1 { self.lane }

    pub(super) fn accepts(self, call: &SemanticDirectCallV1, argument: usize, owned: SemanticTypeIdV1) -> bool {
        self.pairs.get(argument).is_some_and(|(_, expected)| *expected == owned)
            && call.variadic_argument_abis().is_empty()
            && call.arguments().len() == 2
            && call.arguments().iter().zip(self.pairs).all(|(operand, (reference, _))| {
                matches!(operand, SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p)
                    if p.projections().is_empty() && p.ty() == reference)
            })
            && call.destination().is_some_and(|d| d.place().projections().is_empty()
                && d.place().ty() == self.abi.source_output_type())
    }
}

pub(super) fn shared_pointee(types: &[SemanticTypeDeclV1], reference: SemanticTypeIdV1) -> Option<SemanticTypeIdV1> {
    let SemanticTypeShapeV1::Pointer(p) = types.get(reference.index() as usize)?.shape() else { return None };
    (p.kind() == SemanticPointerKindV1::Reference && p.mutability() == SemanticMutabilityV1::Immutable
        && p.metadata() == SemanticPointerMetadataV1::None && p.address_space() == 0
        && p.pointer_width_bits() == 64 && p.pointee() != reference
        && types.get(p.pointee().index() as usize).is_some()).then_some(p.pointee())
}

fn zst(types: &[SemanticTypeDeclV1], id: SemanticTypeIdV1) -> bool {
    types.get(id.index() as usize).is_some_and(|ty| matches!(ty.shape(), SemanticTypeShapeV1::Aggregate(_))
        && ty.layout().size_bytes() == Some(0) && ty.layout().alignment_bytes() == 1
        && !ty.layout().is_uninhabited())
}

fn fields<'a>(types: &'a [SemanticTypeDeclV1], id: SemanticTypeIdV1, offsets: &[u64]) -> Option<&'a [SemanticTypeIdV1]> {
    let ty = types.get(id.index() as usize)?;
    let SemanticTypeShapeV1::Aggregate(fields) = ty.shape() else { return None };
    let SemanticTypeLayoutDetailsV1::Aggregate(layout) = ty.layout().details() else { return None };
    (ty.layout().size_bytes() == Some(4) && ty.layout().alignment_bytes() == 4
        && !ty.layout().is_uninhabited() && layout.field_offsets() == offsets).then_some(fields.fields())
}
