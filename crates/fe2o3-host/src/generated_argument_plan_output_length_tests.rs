fn packed_slice_binding_fixture(
    field: AbiField,
    pointer_width: PointerWidth,
    base_address: u64,
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
            GeneratedArgumentValueV1::Slice {
                address: base_address,
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
fn packed_output_slice_bindings_preserve_all_u64_bits() {
    // These private numeric fixtures test decoding, not allocation validity.
    for access in [Access::WriteOnly, Access::ReadWrite] {
        for (base_address, length) in [
            (0, 0),
            (1, 1),
            (0x1020_3040_5060_7080, 0x0102_0304_0506_0708),
            (u64::MAX, 0),
            (u64::MAX, u64::MAX),
        ] {
            let (plan, packed) = packed_slice_binding_fixture(
                slice_with_access("output", 8, access, 1),
                PointerWidth::Bits64,
                base_address,
                length,
            );
            let original_bytes = packed.bytes().to_vec();
            let binding = plan.packed_output_slice_binding_v1(&packed, 0).unwrap();
            assert_eq!(
                (
                    binding.base_address,
                    binding.length,
                    binding.element_size,
                    binding.element_alignment,
                ),
                (base_address, length, 4, 4)
            );
            assert_eq!(&packed.bytes()[..8], &[0; 8]);
            assert_eq!(&packed.bytes()[8..16], &base_address.to_le_bytes());
            assert_eq!(&packed.bytes()[16..24], &length.to_le_bytes());
            assert_eq!(packed.bytes(), original_bytes.as_slice());
            let retained_plan = plan.clone();
            drop(plan);
            assert_eq!(
                retained_plan.packed_output_slice_binding_v1(&packed, 0),
                Ok(binding)
            );
        }
    }
}

#[test]
fn packed_output_slice_binding_preserves_address_free_placeholder() {
    let (plan, _) = packed_slice_binding_fixture(
        slice_with_access("output", 0, Access::WriteOnly, 1),
        PointerWidth::Bits64,
        0,
        0,
    );
    let input = plan
        .bind_input(
            0,
            GeneratedArgumentValueV1::AddressFreeSlice {
                length: u64::MAX,
                pointer_width: PointerWidth::Bits64,
                address_space: AddressSpace::Global,
                access: Access::WriteOnly,
            },
        )
        .unwrap();
    let packed = plan.pack([input]).unwrap();
    let binding = plan.packed_output_slice_binding_v1(&packed, 0).unwrap();
    assert_eq!((binding.base_address, binding.length), (0, u64::MAX));
}

#[test]
fn packed_output_slice_binding_uses_field_element_layout() {
    for (element_size, element_alignment) in [(1, 1), (4, 4), (24, 8)] {
        let field = AbiField::new(
            Name::new("output").unwrap(),
            0,
            16,
            8,
            AbiKind::Slice {
                element_size,
                element_alignment,
            },
            Mutability::Mutable,
            Access::WriteOnly,
            AddressSpace::Global,
            identity(1),
            ArgumentOwnership::UniqueBorrow,
            AliasClass::Exclusive,
        )
        .unwrap();
        let (plan, packed) = packed_slice_binding_fixture(field, PointerWidth::Bits64, 0x1000, 17);
        let binding = plan.packed_output_slice_binding_v1(&packed, 0).unwrap();
        assert_eq!(binding.element_size, element_size);
        assert_eq!(binding.element_alignment, element_alignment);
    }
}

#[test]
fn packed_output_slice_bindings_keep_distinct_abi_arguments() {
    let fields = vec![
        slice_with_access("first", 0, Access::WriteOnly, 1),
        slice_with_access("second", 16, Access::WriteOnly, 2),
    ];
    let manifest = layout(fields.clone(), 32, 8);
    let plan = validate(&manifest, &generated(fields, 32, 8)).unwrap();
    let inputs = [(1, 0x3000, 65), (0, 0x2000, 64)].map(|(argument_index, address, length)| {
        plan.bind_input(
            argument_index,
            GeneratedArgumentValueV1::Slice {
                address,
                length,
                pointer_width: PointerWidth::Bits64,
                address_space: AddressSpace::Global,
                access: Access::WriteOnly,
            },
        )
        .unwrap()
    });
    let packed = plan.pack(inputs).unwrap();
    for (argument_index, base_address, length) in [(0, 0x2000, 64), (1, 0x3000, 65)] {
        let binding = plan
            .packed_output_slice_binding_v1(&packed, argument_index)
            .unwrap();
        assert_eq!(
            (binding.base_address, binding.length),
            (base_address, length)
        );
        let mut reordered = plan.clone();
        reordered.components.reverse();
        assert_eq!(
            reordered.packed_output_slice_binding_v1(&packed, argument_index),
            Ok(binding)
        );
    }

    // Keep a unique component per argument while swapping its ABI field ordinal.
    for (first, second, kind) in [
        (0, 2, GeneratedPackingComponentKindV1::SlicePointer),
        (1, 3, GeneratedPackingComponentKindV1::SliceLength),
    ] {
        let mut swapped = plan.clone();
        swapped.components[first].argument_index = 1;
        swapped.components[second].argument_index = 0;
        for argument_index in [0, 1] {
            assert_eq!(
                swapped.packed_output_slice_binding_v1(&packed, argument_index),
                Err(GeneratedArgumentPackError::PhysicalComponentMismatch {
                    argument_index,
                    component: kind,
                })
            );
        }
    }
}

#[test]
fn packed_output_slice_binding_rejects_missing_and_scalar_arguments() {
    let field = canonical_scalar::<u64>("count");
    let manifest = layout(vec![field.clone()], 8, 8);
    let plan = validate(&manifest, &generated(vec![field], 8, 8)).unwrap();
    let packed = plan.pack([plan.scalar_u64(0, 17).unwrap()]).unwrap();
    assert_eq!(
        plan.packed_output_slice_binding_v1(&packed, 0),
        Err(GeneratedArgumentPackError::FieldMismatch {
            argument_index: 0,
            property: GeneratedArgumentFieldProperty::Kind,
        })
    );
    for argument_index in [1, usize::MAX] {
        assert_eq!(
            plan.packed_output_slice_binding_v1(&packed, argument_index),
            Err(GeneratedArgumentPackError::ArgumentIndexOutOfBounds {
                argument_index,
                argument_count: 1,
            })
        );
    }
}

#[test]
fn packed_output_slice_binding_requires_exclusive_global_output() {
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
        let (plan, packed) = packed_slice_binding_fixture(field, PointerWidth::Bits64, 0x1000, 17);
        assert_eq!(
            plan.packed_output_slice_binding_v1(&packed, 0),
            Err(GeneratedArgumentPackError::FieldMismatch {
                argument_index: 0,
                property
            })
        );
    }
}

#[test]
fn packed_output_slice_binding_rejects_narrow_pointer_width() {
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
    let (plan, packed) = packed_slice_binding_fixture(field, PointerWidth::Bits32, 0x1000, 17);
    assert_eq!(
        plan.packed_output_slice_binding_v1(&packed, 0),
        Err(GeneratedArgumentPackError::PointerWidthMismatch {
            argument_index: 0,
            expected: PointerWidth::Bits64,
            provided: PointerWidth::Bits32,
        })
    );
}

#[test]
fn packed_output_slice_binding_requires_both_exact_components() {
    let (plan, packed) = packed_slice_binding_fixture(
        slice_with_access("output", 0, Access::WriteOnly, 1),
        PointerWidth::Bits64,
        0x1000,
        17,
    );
    for component_index in [0, 1] {
        let component = plan.components[component_index];
        let mismatch = Err(GeneratedArgumentPackError::PhysicalComponentMismatch {
            argument_index: 0,
            component: component.kind,
        });
        let mut missing = plan.clone();
        missing.components = vec![plan.components[1 - component_index]].into_boxed_slice();
        assert_eq!(missing.packed_output_slice_binding_v1(&packed, 0), mismatch);
        let mut duplicate = plan.clone();
        duplicate.components =
            vec![plan.components[0], plan.components[1], component].into_boxed_slice();
        assert_eq!(
            duplicate.packed_output_slice_binding_v1(&packed, 0),
            mismatch
        );
        let mut wrong_ordinal = plan.clone();
        wrong_ordinal.components[component_index].argument_index = 1;
        assert_eq!(
            wrong_ordinal.packed_output_slice_binding_v1(&packed, 0),
            mismatch
        );
        let mut wrong_kind = plan.clone();
        wrong_kind.components[component_index].kind = GeneratedPackingComponentKindV1::Scalar;
        assert_eq!(
            wrong_kind.packed_output_slice_binding_v1(&packed, 0),
            mismatch
        );
        for (offset, size, alignment) in [
            (component.offset ^ 8, 8, 8),
            (16, 8, 8),
            (u64::MAX, 8, 8),
            (component.offset, 0, 8),
            (component.offset, 4, 8),
            (component.offset, u64::MAX, 8),
            (component.offset, 8, 0),
            (component.offset, 8, 1),
            (component.offset, 8, 4),
            (component.offset, 8, 16),
        ] {
            let mut malformed = plan.clone();
            malformed.components[component_index].offset = offset;
            malformed.components[component_index].size = size;
            malformed.components[component_index].alignment = alignment;
            assert_eq!(
                malformed.packed_output_slice_binding_v1(&packed, 0),
                mismatch
            );
        }
    }
    let mut swapped_kinds = plan.clone();
    swapped_kinds.components[0].kind = GeneratedPackingComponentKindV1::SliceLength;
    swapped_kinds.components[1].kind = GeneratedPackingComponentKindV1::SlicePointer;
    assert_eq!(
        swapped_kinds.packed_output_slice_binding_v1(&packed, 0),
        Err(GeneratedArgumentPackError::PhysicalComponentMismatch {
            argument_index: 0,
            component: GeneratedPackingComponentKindV1::SlicePointer,
        })
    );
}

#[test]
fn packed_output_slice_binding_checks_buffer_bounds() {
    let (plan, mut packed) = packed_slice_binding_fixture(
        slice_with_access("output", 0, Access::WriteOnly, 1),
        PointerWidth::Bits64,
        0x1000,
        17,
    );
    for byte_length in [0, 8, 15, 17] {
        packed.bytes = vec![0; byte_length].into_boxed_slice();
        assert_eq!(
            plan.packed_output_slice_binding_v1(&packed, 0),
            Err(GeneratedArgumentPackError::SourcePlanMismatch { argument_index: 0 })
        );
    }
    packed.bytes = vec![0; 16].into_boxed_slice();
    for alignment in [0, 4, 16] {
        packed.alignment = alignment;
        assert_eq!(
            plan.packed_output_slice_binding_v1(&packed, 0),
            Err(GeneratedArgumentPackError::SourcePlanMismatch { argument_index: 0 })
        );
    }
    packed.alignment = 8;
    for (field_offset, kind) in [
        (8, GeneratedPackingComponentKindV1::SliceLength),
        (16, GeneratedPackingComponentKindV1::SlicePointer),
    ] {
        let mut malformed = plan.clone();
        malformed.fields[0] = slice_with_access("output", field_offset, Access::WriteOnly, 1);
        malformed.components[0].offset = field_offset;
        malformed.components[1].offset = field_offset + 8;
        assert_eq!(
            malformed.packed_output_slice_binding_v1(&packed, 0),
            Err(GeneratedArgumentPackError::ComponentOutOfBounds {
                argument_index: 0,
                component: kind,
                offset: 16,
                size: 8,
                kernarg_size: 16,
            })
        );
    }
}

#[test]
fn packed_output_slice_binding_rejects_plan_and_kernel_substitution() {
    let field = slice_with_access("output", 0, Access::WriteOnly, 1);
    let (plan, packed) =
        packed_slice_binding_fixture(field.clone(), PointerWidth::Bits64, 0x1000, 17);
    let (other, other_packed) =
        packed_slice_binding_fixture(field.clone(), PointerWidth::Bits64, 0x2000, 23);
    assert_eq!(
        other.packed_output_slice_binding_v1(&packed, 0),
        Err(GeneratedArgumentPackError::SourcePlanMismatch { argument_index: 0 })
    );
    assert_eq!(
        plan.packed_output_slice_binding_v1(&other_packed, 0),
        Err(GeneratedArgumentPackError::SourcePlanMismatch { argument_index: 0 })
    );
    let foreign = validate_argument_packing(
        KernelId::from_bytes([10; 32]),
        &layout(vec![field.clone()], 16, 8),
        &generated(vec![field], 16, 8),
    )
    .unwrap();
    assert_eq!(
        foreign.packed_output_slice_binding_v1(&packed, 0),
        Err(GeneratedArgumentPackError::SourceKernelMismatch { argument_index: 0 })
    );
    let foreign_input = foreign
        .bind_input(
            0,
            GeneratedArgumentValueV1::Slice {
                address: 0x3000,
                length: 29,
                pointer_width: PointerWidth::Bits64,
                address_space: AddressSpace::Global,
                access: Access::WriteOnly,
            },
        )
        .unwrap();
    let foreign_packed = foreign.pack([foreign_input]).unwrap();
    assert_eq!(
        plan.packed_output_slice_binding_v1(&foreign_packed, 0),
        Err(GeneratedArgumentPackError::SourceKernelMismatch { argument_index: 0 })
    );

    let different_field = slice_with_access("different", 8, Access::WriteOnly, 2);
    let (different, different_packed) =
        packed_slice_binding_fixture(different_field, PointerWidth::Bits64, 0x4000, 31);
    assert_eq!(
        plan.packed_output_slice_binding_v1(&different_packed, 0),
        Err(GeneratedArgumentPackError::SourcePlanMismatch { argument_index: 0 })
    );
    assert_eq!(
        different.packed_output_slice_binding_v1(&packed, 0),
        Err(GeneratedArgumentPackError::SourcePlanMismatch { argument_index: 0 })
    );
}

#[test]
fn packed_output_slice_binding_retains_exact_plan_owner() {
    let (plan, packed) = packed_slice_binding_fixture(
        slice_with_access("output", 0, Access::WriteOnly, 1),
        PointerWidth::Bits64,
        0x1000,
        17,
    );
    let seal = std::sync::Arc::downgrade(&plan.seal);
    let retained_plan = plan.clone();
    drop(plan);
    let binding = retained_plan
        .packed_output_slice_binding_v1(&packed, 0)
        .unwrap();
    drop(retained_plan);
    assert_eq!((binding.base_address, binding.length), (0x1000, 17));
    assert!(seal.upgrade().is_some());
    drop(packed);
    assert!(seal.upgrade().is_none());
}
