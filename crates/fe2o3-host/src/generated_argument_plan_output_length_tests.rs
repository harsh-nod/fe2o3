fn packed_slice_length_fixture(
    field: AbiField,
    pointer_width: PointerWidth,
    length: u64,
) -> (
    super::GeneratedArgumentPackingPlanV1,
    super::GeneratedPackedArgumentsV1<'static>,
) {
    let size = field.offset() + field.size();
    let alignment = field.alignment();
    let address_space = field.address_space();
    let access = field.access();
    let manifest = layout_with_width(vec![field.clone()], size, alignment, pointer_width);
    let generated = generated_with_width(vec![field], size, alignment, pointer_width);
    let plan = validate(&manifest, &generated).unwrap();
    let input = plan
        .bind_input(
            0,
            GeneratedArgumentValueV1::AddressFreeSlice {
                length,
                pointer_width,
                address_space,
                access,
            },
        )
        .unwrap();
    let packed = plan.pack([input]).unwrap();
    (plan, packed)
}

#[test]
fn packed_output_slice_lengths_preserve_all_u64_bits() {
    for access in [Access::WriteOnly, Access::ReadWrite] {
        for length in [0, 1, 0x0102_0304_0506_0708, u64::MAX] {
            let (plan, packed) = packed_slice_length_fixture(
                slice_with_access("output", 8, access, 1),
                PointerWidth::Bits64,
                length,
            );
            assert_eq!(plan.packed_output_slice_length_v1(&packed, 0), Ok(length));
            assert_eq!(
                plan.clone().packed_output_slice_length_v1(&packed, 0),
                Ok(length)
            );
        }
    }
}

#[test]
fn packed_output_slice_lengths_keep_distinct_source_arguments() {
    let fields = vec![
        slice_with_access("first", 0, Access::WriteOnly, 1),
        slice_with_access("second", 16, Access::WriteOnly, 2),
    ];
    let manifest = layout(fields.clone(), 32, 8);
    let plan = validate(&manifest, &generated(fields, 32, 8)).unwrap();
    let inputs = [(1, 65), (0, 64)].map(|(argument_index, length)| {
        plan.bind_input(
            argument_index,
            GeneratedArgumentValueV1::AddressFreeSlice {
                length,
                pointer_width: PointerWidth::Bits64,
                address_space: AddressSpace::Global,
                access: Access::WriteOnly,
            },
        )
        .unwrap()
    });
    let packed = plan.pack(inputs).unwrap();
    assert_eq!(plan.packed_output_slice_length_v1(&packed, 0), Ok(64));
    assert_eq!(plan.packed_output_slice_length_v1(&packed, 1), Ok(65));

    // Keep one length per argument while swapping their source ordinals.
    let mut swapped = plan.clone();
    swapped.components[1].argument_index = 1;
    swapped.components[3].argument_index = 0;
    for argument_index in [0, 1] {
        assert_eq!(
            swapped.packed_output_slice_length_v1(&packed, argument_index),
            Err(GeneratedArgumentPackError::PhysicalComponentMismatch {
                argument_index,
                component: GeneratedPackingComponentKindV1::SliceLength,
            })
        );
    }
}

#[test]
fn packed_output_slice_length_rejects_missing_and_scalar_arguments() {
    let field = canonical_scalar::<u64>("count");
    let manifest = layout(vec![field.clone()], 8, 8);
    let plan = validate(&manifest, &generated(vec![field], 8, 8)).unwrap();
    let packed = plan.pack([plan.scalar_u64(0, 17).unwrap()]).unwrap();
    assert_eq!(
        plan.packed_output_slice_length_v1(&packed, 0),
        Err(GeneratedArgumentPackError::FieldMismatch {
            argument_index: 0,
            property: GeneratedArgumentFieldProperty::Kind,
        })
    );
    for argument_index in [1, usize::MAX] {
        assert_eq!(
            plan.packed_output_slice_length_v1(&packed, argument_index),
            Err(GeneratedArgumentPackError::ArgumentIndexOutOfBounds {
                argument_index,
                argument_count: 1,
            })
        );
    }
}

#[test]
fn packed_output_slice_length_requires_exclusive_global_output() {
    for (mutability, access, space, ownership, alias, property) in [
        (
            Mutability::Mutable,
            Access::ReadOnly,
            AddressSpace::Global,
            ArgumentOwnership::UniqueBorrow,
            AliasClass::Exclusive,
            GeneratedArgumentFieldProperty::Access,
        ),
        (
            Mutability::Mutable,
            Access::ReadWrite,
            AddressSpace::Workgroup,
            ArgumentOwnership::UniqueBorrow,
            AliasClass::Exclusive,
            GeneratedArgumentFieldProperty::AddressSpace,
        ),
        (
            Mutability::Mutable,
            Access::ReadWrite,
            AddressSpace::Global,
            ArgumentOwnership::RawPointer,
            AliasClass::Unrestricted,
            GeneratedArgumentFieldProperty::Ownership,
        ),
        (
            Mutability::Immutable,
            Access::ReadWrite,
            AddressSpace::Global,
            ArgumentOwnership::SharedBorrow,
            AliasClass::SharedAtomic,
            GeneratedArgumentFieldProperty::Mutability,
        ),
    ] {
        let field = AbiField::new(
            Name::new("output").unwrap(),
            0,
            16,
            8,
            AbiKind::Slice {
                element_size: 4,
                element_alignment: 4,
            },
            mutability,
            access,
            space,
            identity(1),
            ownership,
            alias,
        )
        .unwrap();
        let (plan, packed) = packed_slice_length_fixture(field, PointerWidth::Bits64, 17);
        assert_eq!(
            plan.packed_output_slice_length_v1(&packed, 0),
            Err(GeneratedArgumentPackError::FieldMismatch {
                argument_index: 0,
                property
            })
        );
    }
}

#[test]
fn packed_output_slice_length_rejects_narrow_pointer_width() {
    let field = AbiField::new(
        Name::new("output").unwrap(),
        0,
        8,
        4,
        AbiKind::Slice {
            element_size: 4,
            element_alignment: 4,
        },
        Mutability::Mutable,
        Access::ReadWrite,
        AddressSpace::Global,
        identity(1),
        ArgumentOwnership::UniqueBorrow,
        AliasClass::Exclusive,
    )
    .unwrap();
    let (plan, packed) = packed_slice_length_fixture(field, PointerWidth::Bits32, 17);
    assert_eq!(
        plan.packed_output_slice_length_v1(&packed, 0),
        Err(GeneratedArgumentPackError::PointerWidthMismatch {
            argument_index: 0,
            expected: PointerWidth::Bits64,
            provided: PointerWidth::Bits32,
        })
    );
}

#[test]
fn packed_output_slice_length_requires_one_exact_length_component() {
    let (plan, packed) = packed_slice_length_fixture(
        slice_with_access("output", 0, Access::WriteOnly, 1),
        PointerWidth::Bits64,
        17,
    );
    let mismatch = Err(GeneratedArgumentPackError::PhysicalComponentMismatch {
        argument_index: 0,
        component: GeneratedPackingComponentKindV1::SliceLength,
    });
    let mut missing = plan.clone();
    missing.components = vec![plan.components[0]].into_boxed_slice();
    assert_eq!(missing.packed_output_slice_length_v1(&packed, 0), mismatch);
    let mut duplicate = plan.clone();
    duplicate.components =
        vec![plan.components[0], plan.components[1], plan.components[1]].into_boxed_slice();
    assert_eq!(
        duplicate.packed_output_slice_length_v1(&packed, 0),
        mismatch
    );
    let mut swapped = plan.clone();
    swapped.components[1].argument_index = 1;
    assert_eq!(swapped.packed_output_slice_length_v1(&packed, 0), mismatch);
    for (offset, size, alignment) in [
        (0, 8, 8),
        (16, 8, 8),
        (u64::MAX, 8, 8),
        (8, 4, 8),
        (8, 8, 0),
    ] {
        let mut malformed = plan.clone();
        malformed.components[1].offset = offset;
        malformed.components[1].size = size;
        malformed.components[1].alignment = alignment;
        assert_eq!(
            malformed.packed_output_slice_length_v1(&packed, 0),
            mismatch
        );
    }
}

#[test]
fn packed_output_slice_length_checks_buffer_bounds() {
    let (mut plan, mut packed) = packed_slice_length_fixture(
        slice_with_access("output", 0, Access::WriteOnly, 1),
        PointerWidth::Bits64,
        17,
    );
    packed.bytes = packed.bytes[..15].to_vec().into_boxed_slice();
    assert_eq!(
        plan.packed_output_slice_length_v1(&packed, 0),
        Err(GeneratedArgumentPackError::SourcePlanMismatch { argument_index: 0 })
    );
    packed.bytes = vec![0; 16].into_boxed_slice();
    plan.fields[0] = slice_with_access("output", 16, Access::WriteOnly, 1);
    plan.components[0].offset = 16;
    plan.components[1].offset = 24;
    assert_eq!(
        plan.packed_output_slice_length_v1(&packed, 0),
        Err(GeneratedArgumentPackError::ComponentOutOfBounds {
            argument_index: 0,
            component: GeneratedPackingComponentKindV1::SliceLength,
            offset: 24,
            size: 8,
            kernarg_size: 16,
        })
    );
}

#[test]
fn packed_output_slice_length_rejects_plan_substitution() {
    let field = slice_with_access("output", 0, Access::WriteOnly, 1);
    let (plan, packed) = packed_slice_length_fixture(field.clone(), PointerWidth::Bits64, 17);
    let (other, other_packed) =
        packed_slice_length_fixture(field.clone(), PointerWidth::Bits64, 23);
    assert_eq!(
        other.packed_output_slice_length_v1(&packed, 0),
        Err(GeneratedArgumentPackError::SourcePlanMismatch { argument_index: 0 })
    );
    assert_eq!(
        plan.packed_output_slice_length_v1(&other_packed, 0),
        Err(GeneratedArgumentPackError::SourcePlanMismatch { argument_index: 0 })
    );
    let foreign = validate_argument_packing(
        KernelId::from_bytes([10; 32]),
        &layout(vec![field.clone()], 16, 8),
        &generated(vec![field], 16, 8),
    )
    .unwrap();
    assert_eq!(
        foreign.packed_output_slice_length_v1(&packed, 0),
        Err(GeneratedArgumentPackError::SourceKernelMismatch { argument_index: 0 })
    );
}
