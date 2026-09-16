impl<'a> SemanticFunctionLoweringV1<'a> {
    #[allow(clippy::too_many_arguments)]
    fn prepare_atomic_load_store_v1(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        place: &SemanticPlaceV1,
        value: Option<&SemanticOperandV1>,
        volatility: SemanticVolatilityV1,
        atomic_access: SemanticAtomicAccessV1,
        operations: &mut Vec<Operation>,
    ) -> Result<(Atomic, Type), ProductionSemanticKirErrorV1> {
        let store = value.is_some();
        let invalid = || {
            unsupported(
                0,
                Some(block.index()),
                statement,
                "semantic atomic load/store has no exact scalar memory contract",
            )
        };
        let valid_ordering = matches!(
            atomic_access.ordering(),
            SemanticAtomicOrderingV1::Relaxed | SemanticAtomicOrderingV1::SequentiallyConsistent
        ) || matches!(
            (store, atomic_access.ordering()),
            (false, SemanticAtomicOrderingV1::Acquire) | (true, SemanticAtomicOrderingV1::Release)
        );
        if volatility != SemanticVolatilityV1::NonVolatile || !valid_ordering {
            return Err(invalid());
        }
        let address = if place.projections().is_empty()
            && self
                .retained_local_slots
                .contains_key(&place.local().index())
        {
            self.retained_local_pointer_binding_v1(
                place.local(),
                AccessMode::ReadWrite,
                operations,
            )?
        } else {
            self.resolve_place(block, statement, place, operations)?
        };
        let (pointer, pointer_ty) = address.value().map_err(|_| invalid())?;
        let Type::Pointer(pointer_ty) = pointer_ty else {
            return Err(invalid());
        };
        let pointee = (*pointer_ty.pointee).clone();
        if !matches!(
            pointee.as_scalar(),
            Some(ScalarType::I32 | ScalarType::U32 | ScalarType::I64 | ScalarType::U64)
        ) || (store && pointer_ty.access != AccessMode::ReadWrite)
            || lower_memory_element_type(self.types, place.ty())? != pointee
        {
            return Err(invalid());
        }
        let value = match value {
            Some(value) => {
                let binding = self.lower_operand(block, statement, value, operations)?;
                let (id, ty) = binding.value().map_err(|_| invalid())?;
                if ty != pointee {
                    return Err(invalid());
                }
                Some(id)
            }
            None => None,
        };
        let scope = lower_atomic_scope(atomic_access.scope()).ok_or_else(invalid)?;
        let access = memory_access_for_type(self.types, place.ty(), pointer_ty.address_space)?;
        Ok((
            Atomic {
                kind: if store {
                    AtomicKind::Store
                } else {
                    AtomicKind::Load
                },
                pointer,
                value,
                compare: None,
                access,
                scope,
                ordering: lower_atomic_ordering(atomic_access.ordering()),
                failure_ordering: None,
            },
            pointee,
        ))
    }
}

fn pointer_access_restriction_v1(kind: SemanticCastKindV1, from: &Type, to: &Type) -> bool {
    matches!((kind, from, to), (SemanticCastKindV1::Pointer, Type::Pointer(from), Type::Pointer(to))
        if from.access == AccessMode::ReadWrite && to.access == AccessMode::ReadOnly
            && from.address_space == to.address_space && from.pointee == to.pointee)
}

#[cfg(test)]
mod atomic_pointer_restriction_tests {
    use super::*;

    #[test]
    fn pointer_restriction_cannot_widen_access_or_change_identity() {
        let pointer = |element, space, access| Type::pointer(Type::Scalar(element), space, access);
        let writable = pointer(ScalarType::U32, AddressSpace::Global, AccessMode::ReadWrite);
        let read_only = pointer(ScalarType::U32, AddressSpace::Global, AccessMode::ReadOnly);
        assert!(pointer_access_restriction_v1(
            SemanticCastKindV1::Pointer,
            &writable,
            &read_only
        ));
        assert!(!pointer_access_restriction_v1(
            SemanticCastKindV1::Pointer,
            &read_only,
            &writable
        ));
        assert!(!pointer_access_restriction_v1(
            SemanticCastKindV1::Transmute,
            &writable,
            &read_only
        ));
        for target in [
            pointer(ScalarType::U64, AddressSpace::Global, AccessMode::ReadOnly),
            pointer(
                ScalarType::U32,
                AddressSpace::Workgroup,
                AccessMode::ReadOnly,
            ),
            pointer(ScalarType::U32, AddressSpace::Global, AccessMode::WriteOnly),
        ] {
            assert!(!pointer_access_restriction_v1(
                SemanticCastKindV1::Pointer,
                &writable,
                &target
            ));
        }
    }
}
