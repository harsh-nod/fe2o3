#[test]
fn address_free_empty_slices_encode_element_alignment_at_each_pointer_width() {
    for pointer_width in [PointerWidth::Bits32, PointerWidth::Bits64] {
        let width = pointer_width.bytes();
        let field = AbiField::new(
            Name::new("input").unwrap(),
            0,
            width * 2,
            width as u32,
            AbiKind::Slice {
                element_size: 8,
                element_alignment: 8,
            },
            Mutability::Immutable,
            Access::ReadOnly,
            AddressSpace::Global,
            u64::shared_slice_type_identity_v1(pointer_width),
            ArgumentOwnership::SharedBorrow,
            AliasClass::SharedReadOnly,
        )
        .unwrap();
        let manifest =
            layout_with_width(vec![field.clone()], width * 2, width as u32, pointer_width);
        let generated = generated_with_width(vec![field], width * 2, width as u32, pointer_width);
        let plan = validate(&manifest, &generated).unwrap();
        for length in [0, 1, 7] {
            let input = plan
                .bind_input(
                    0,
                    GeneratedArgumentValueV1::AddressFreeSlice {
                        length,
                        pointer_width,
                        address_space: AddressSpace::Global,
                        access: Access::ReadOnly,
                    },
                )
                .unwrap();
            let packed = plan.pack([input]).unwrap();
            let expected_address = if length == 0 { 8_u64 } else { 0 };
            assert_eq!(
                &packed.bytes()[..width as usize],
                &expected_address.to_le_bytes()[..width as usize]
            );
            assert_eq!(
                &packed.bytes()[width as usize..],
                &length.to_le_bytes()[..width as usize]
            );
        }
    }
}
