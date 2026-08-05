use fe2o3_artifacts::{
    AbiField, AbiKind, AbiLayout, Access, AddressSpace, MAX_ABI_BYTES, MAX_ABI_FIELDS, Mutability,
    Name, PointerWidth, ScalarType, ValidationError,
};

fn name(value: &str) -> Name {
    Name::new(value).unwrap()
}

fn scalar(field_name: &str, offset: u64) -> AbiField {
    AbiField::new(
        name(field_name),
        offset,
        4,
        4,
        AbiKind::Scalar(ScalarType::U32),
        Mutability::Immutable,
        Access::ByValue,
        AddressSpace::Value,
    )
    .unwrap()
}

fn fields() -> Vec<AbiField> {
    vec![
        scalar("n", 0),
        AbiField::new(
            name("input"),
            8,
            8,
            8,
            AbiKind::Pointer {
                pointee_size: 4,
                pointee_alignment: 4,
            },
            Mutability::Immutable,
            Access::ReadOnly,
            AddressSpace::Global,
        )
        .unwrap(),
        AbiField::new(
            name("output"),
            16,
            16,
            8,
            AbiKind::Slice {
                element_size: 4,
                element_alignment: 4,
            },
            Mutability::Mutable,
            Access::ReadWrite,
            AddressSpace::Global,
        )
        .unwrap(),
    ]
}

#[test]
fn abi_fields_enforce_kind_access_and_address_space_rules() {
    let layout = AbiLayout::new(32, 8, PointerWidth::Bits64, fields()).unwrap();
    assert_eq!(layout.fields().len(), 3);
    assert_eq!(layout.fields()[2].size(), 16);
    assert!(matches!(layout.fields()[2].kind(), AbiKind::Slice { .. }));

    assert!(matches!(
        AbiField::new(
            name("bad_align"),
            0,
            4,
            3,
            AbiKind::Scalar(ScalarType::U32),
            Mutability::Immutable,
            Access::ByValue,
            AddressSpace::Value,
        ),
        Err(ValidationError::InvalidAlignment { .. })
    ));
    assert!(matches!(
        AbiField::new(
            name("immutable_write"),
            0,
            8,
            8,
            AbiKind::Pointer {
                pointee_size: 4,
                pointee_alignment: 4,
            },
            Mutability::Immutable,
            Access::ReadWrite,
            AddressSpace::Global,
        ),
        Err(ValidationError::InvalidAccess(_))
    ));
    assert!(matches!(
        AbiField::new(
            name("constant_write"),
            0,
            8,
            8,
            AbiKind::Pointer {
                pointee_size: 4,
                pointee_alignment: 4,
            },
            Mutability::Mutable,
            Access::WriteOnly,
            AddressSpace::Constant,
        ),
        Err(ValidationError::InvalidAccess(_))
    ));
    assert!(
        AbiField::new(
            name("zst_pointer"),
            0,
            8,
            8,
            AbiKind::Pointer {
                pointee_size: 0,
                pointee_alignment: 1,
            },
            Mutability::Immutable,
            Access::ReadOnly,
            AddressSpace::Global,
        )
        .is_ok()
    );
    assert!(matches!(
        AbiField::new(
            name("malformed_pointer"),
            0,
            1,
            1,
            AbiKind::Pointer {
                pointee_size: 4,
                pointee_alignment: 4,
            },
            Mutability::Immutable,
            Access::ReadOnly,
            AddressSpace::Global,
        ),
        Err(ValidationError::InvalidLayout(_))
    ));
    assert!(matches!(
        AbiField::new(
            name("malformed_slice"),
            0,
            8,
            8,
            AbiKind::Slice {
                element_size: 4,
                element_alignment: 4,
            },
            Mutability::Immutable,
            Access::ReadOnly,
            AddressSpace::Global,
        ),
        Err(ValidationError::InvalidLayout(_))
    ));
}

#[test]
fn abi_layout_rejects_overlap_overflow_duplicates_and_wrong_pointer_width() {
    assert!(matches!(
        AbiField::new(
            name("overflow"),
            u64::MAX - 3,
            4,
            4,
            AbiKind::Scalar(ScalarType::U32),
            Mutability::Immutable,
            Access::ByValue,
            AddressSpace::Value,
        ),
        Err(ValidationError::Overflow("ABI field end"))
    ));
    assert!(matches!(
        AbiLayout::new(
            8,
            4,
            PointerWidth::Bits64,
            vec![scalar("a", 0), scalar("b", 0)],
        ),
        Err(ValidationError::InvalidLayout(_))
    ));
    assert!(matches!(
        AbiLayout::new(
            8,
            4,
            PointerWidth::Bits64,
            vec![scalar("same", 0), scalar("same", 4)],
        ),
        Err(ValidationError::Duplicate { field: "ABI name" })
    ));
    assert!(matches!(
        AbiLayout::new(32, 8, PointerWidth::Bits32, fields()),
        Err(ValidationError::InvalidLayout(_))
    ));
    assert!(matches!(
        AbiLayout::new(
            8,
            4,
            PointerWidth::Bits64,
            vec![scalar("field", 0); MAX_ABI_FIELDS + 1],
        ),
        Err(ValidationError::TooMany {
            field: "ABI fields",
            ..
        })
    ));
    assert!(
        AbiLayout::new(
            MAX_ABI_BYTES,
            4,
            PointerWidth::Bits64,
            vec![scalar("field", 0)],
        )
        .is_ok()
    );
    assert!(matches!(
        AbiLayout::new(
            MAX_ABI_BYTES + 4,
            4,
            PointerWidth::Bits64,
            vec![scalar("field", 0)],
        ),
        Err(ValidationError::InvalidLayout(_))
    ));
}
