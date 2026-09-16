pub(crate) fn atomic_slice_u32_type_identity_v1(pointer_width: PointerWidth) -> TypeIdentity {
    canonical_slice_layout_with_source_v1(
        RustScalarElementTypeV1::U32,
        pointer_width,
        RustSourceTypeShapeV1::SharedAtomicSliceU32,
        false,
    )
    .type_identity()
}

impl GeneratedArgumentPackingPlanV1 {
    pub(crate) fn bind_generated_address_free_atomic_slice_u32_v1<'allocation>(
        &self,
        argument_index: usize,
        length: usize,
        _borrow: GeneratedArgumentBorrowV1<'allocation>,
    ) -> Result<GeneratedArgumentInputV1<'allocation>, GeneratedArgumentPackError> {
        if self.pointer_width != PointerWidth::Bits64 {
            return Err(GeneratedArgumentPackError::PointerWidthMismatch {
                argument_index,
                expected: PointerWidth::Bits64,
                provided: self.pointer_width,
            });
        }
        let width = self.pointer_width.bytes();
        validate_generated_field_v1(
            self,
            argument_index,
            GeneratedFieldExpectationV1 {
                kind: AbiKind::Slice {
                    element_size: 4,
                    element_alignment: 4,
                },
                size: width * 2,
                alignment: u32::try_from(width).expect("pointer width fits u32"),
                type_identity: atomic_slice_u32_type_identity_v1(self.pointer_width),
                mutability: Mutability::Immutable,
                access: Access::ReadWrite,
                address_space: AddressSpace::Global,
                ownership: ArgumentOwnership::SharedBorrow,
                alias_class: AliasClass::SharedAtomic,
            },
        )?;
        if length
            .checked_mul(4)
            .is_none_or(|bytes| bytes > isize::MAX as usize)
        {
            return Err(GeneratedArgumentPackError::SliceByteExtentOverflow {
                argument_index,
                length,
                element_size: 4,
            });
        }
        let length = u64::try_from(length).map_err(|_| {
            GeneratedArgumentPackError::IntegerWidthOverflow {
                argument_index,
                component: GeneratedPackingComponentKindV1::SliceLength,
                value: u64::MAX,
                pointer_width: self.pointer_width,
            }
        })?;
        self.bind_input(
            argument_index,
            GeneratedArgumentValueV1::AddressFreeSlice {
                length,
                pointer_width: self.pointer_width,
                address_space: AddressSpace::Global,
                access: Access::ReadWrite,
            },
        )
    }
}
