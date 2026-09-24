// Exact one-component return transport for generated typed-u32 helpers.
// This is not generic aggregate support and never changes the ordinary scalar ABI.
pub(super) fn singleton_u32_type(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Option<SemanticTypeIdV1> {
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticBackendReprV1, SemanticBackendScalarV1, SemanticFieldsShapeV1,
        SemanticRustTypeKindV1, SemanticRustcVariantsV1, SemanticTypeLayoutDetailsV1,
    };
    let tuple = types.get(ty.index() as usize)?;
    let SemanticTypeShapeV1::Tuple(fields) = tuple.shape() else {
        return None;
    };
    let [component] = fields.fields() else {
        return None;
    };
    let word = types.get(component.index() as usize)?;
    if !matches!(
        word.shape(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32
        })
    ) || word.rust_type_kind() != SemanticRustTypeKindV1::Ordinary
        || tuple.rust_type_kind() != SemanticRustTypeKindV1::Ordinary
    {
        return None;
    }
    let layout = tuple.layout();
    let word_layout = word.layout();
    if [layout, word_layout].into_iter().any(|l| {
        l.size_bytes() != Some(4)
            || l.rustc_size_bytes() != 4
            || l.alignment_bytes() != 4
            || l.unadjusted_abi_alignment_bytes() != 4
            || l.is_uninhabited()
            || l.largest_niche().is_some()
            || !matches!(l.variants(), SemanticRustcVariantsV1::Single { index: 0 })
    }) || layout.backend_repr() != word_layout.backend_repr()
        || !matches!(
            word_layout.backend_repr(),
            SemanticBackendReprV1::Scalar(SemanticBackendScalarV1::Initialized { .. })
        )
        || !matches!(word_layout.fields(), SemanticFieldsShapeV1::Primitive)
        || !matches!(word_layout.details(), SemanticTypeLayoutDetailsV1::None)
        || !matches!(layout.fields(), SemanticFieldsShapeV1::Arbitrary {
            source_order_offsets_bytes, memory_order_source_indices
        } if source_order_offsets_bytes.as_ref() == [0]
            && memory_order_source_indices.as_ref() == [0])
        || !matches!(layout.details(), SemanticTypeLayoutDetailsV1::Aggregate(a)
            if a.field_offsets() == [0] && a.padding().is_empty())
    {
        return None;
    }
    Some(*component)
}

pub(super) fn result_scalar(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Result<Scalar, Error> {
    scalar(types, singleton_u32_type(types, ty).unwrap_or(ty))
}

fn singleton_field(
    types: &[SemanticTypeDeclV1],
    local_ty: SemanticTypeIdV1,
    place: &SemanticPlaceV1,
) -> bool {
    let Some(component) = singleton_u32_type(types, local_ty) else {
        return false;
    };
    matches!(place.projections(), [p]
        if matches!(p.kind(), SemanticProjectionKindV1::Field(0))
        && p.result_type() == component && place.ty() == component)
}

impl Frame<'_, '_, '_> {
    fn singleton_operand(
        &mut self,
        operand: &SemanticOperandV1,
        expected: SemanticTypeIdV1,
    ) -> Result<Value, Error> {
        self.meter.work(6)?;
        let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand else {
            return Err("helper singleton has no aggregate constant transport");
        };
        let index = place.local().index() as usize;
        if place.ty() != expected
            || !place.projections().is_empty()
            || self
                .function
                .locals()
                .get(index)
                .map(SemanticLocalDeclV1::ty)
                != Some(expected)
            || self.live.get(index).copied() != Some(true)
            || singleton_u32_type(self.types, expected).is_none()
        {
            return Err("helper singleton transport is mistyped or outside lifetime");
        }
        let Some(Slot::SingletonU32(value)) = self.locals.get(index).copied() else {
            return Err("helper singleton is uninitialized");
        };
        if matches!(operand, SemanticOperandV1::Move(_)) {
            self.locals[index] = Slot::Uninitialized;
        }
        Ok(value)
    }

    fn singleton_rvalue(
        &mut self,
        value: &SemanticRvalueV1,
        component: SemanticTypeIdV1,
    ) -> Result<Slot, Error> {
        self.meter.work(6)?;
        let result = match value.kind() {
            SemanticRvalueKindV1::Aggregate(aggregate)
                if aggregate.kind() == &SemanticAggregateKindV1::Tuple =>
            {
                let [operand] = aggregate.operands() else {
                    return Err("helper singleton aggregate has wrong arity");
                };
                if operand.ty() != component {
                    return Err("helper singleton aggregate changes component type");
                }
                self.operand(operand)?
            }
            SemanticRvalueKindV1::Use(operand) => {
                self.singleton_operand(operand, value.result_type())?
            }
            _ => return Err("helper singleton has no admitted aggregate producer"),
        };
        if self.output.scalar(result)? != result_scalar(self.types, value.result_type())? {
            return Err("helper singleton component has wrong scalar type");
        }
        Ok(Slot::SingletonU32(result))
    }
}
