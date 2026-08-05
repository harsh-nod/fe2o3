use fe2o3_artifacts::{
    AbiField, AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership,
    DeclaredRustLayoutIdentity, DeclaredRustTypeIdentity, DigestBytes, HostLaunchAbi,
    HostLaunchAbiError, Mutability, Name, PointerWidth, ScalarType, TypeIdentity,
};

#[derive(Clone, Copy)]
enum ReferenceKind {
    Pointer,
    Slice,
}

fn name(value: &str) -> Name {
    Name::new(value).unwrap()
}

fn type_identity() -> TypeIdentity {
    TypeIdentity::new(
        DeclaredRustTypeIdentity::from_untrusted_bytes(DigestBytes::from_bytes([0x31; 32])),
        DeclaredRustLayoutIdentity::from_untrusted_bytes(DigestBytes::from_bytes([0x32; 32])),
    )
}

fn scalar_layout(scalar: ScalarType, size: u64, alignment: u32) -> AbiLayout {
    let field = AbiField::new(
        name("value"),
        0,
        size,
        alignment,
        AbiKind::Scalar(scalar),
        Mutability::Immutable,
        Access::ByValue,
        AddressSpace::Value,
        type_identity(),
        ArgumentOwnership::ByValue,
        AliasClass::Value,
    )
    .unwrap();
    AbiLayout::new(size, alignment, PointerWidth::Bits64, vec![field]).unwrap()
}

#[allow(clippy::too_many_arguments)]
fn reference_layout(
    kind: ReferenceKind,
    pointer_width: PointerWidth,
    address_space: AddressSpace,
    mutability: Mutability,
    access: Access,
    ownership: ArgumentOwnership,
    alias_class: AliasClass,
) -> AbiLayout {
    let pointer_bytes = pointer_width.bytes();
    let (kind, size) = match kind {
        ReferenceKind::Pointer => (
            AbiKind::Pointer {
                pointee_size: 4,
                pointee_alignment: 4,
            },
            pointer_bytes,
        ),
        ReferenceKind::Slice => (
            AbiKind::Slice {
                element_size: 4,
                element_alignment: 4,
            },
            pointer_bytes * 2,
        ),
    };
    let alignment = u32::try_from(pointer_bytes).unwrap();
    let field = AbiField::new(
        name("reference"),
        0,
        size,
        alignment,
        kind,
        mutability,
        access,
        address_space,
        type_identity(),
        ownership,
        alias_class,
    )
    .unwrap();
    AbiLayout::new(size, alignment, pointer_width, vec![field]).unwrap()
}

#[test]
fn accepts_empty_layout_and_every_scalar_kind() {
    let empty = AbiLayout::new(0, 1, PointerWidth::Bits64, vec![]).unwrap();
    assert_eq!(HostLaunchAbi::validate(&empty).unwrap().layout(), &empty);

    for (scalar, size, alignment) in [
        (ScalarType::I8, 1, 1),
        (ScalarType::U8, 1, 1),
        (ScalarType::I16, 2, 2),
        (ScalarType::U16, 2, 2),
        (ScalarType::I32, 4, 4),
        (ScalarType::U32, 4, 4),
        (ScalarType::I64, 8, 8),
        (ScalarType::U64, 8, 8),
        (ScalarType::F16, 2, 2),
        (ScalarType::F32, 4, 4),
        (ScalarType::F64, 8, 8),
    ] {
        let layout = scalar_layout(scalar, size, alignment);
        let validated = HostLaunchAbi::validate(&layout).unwrap();
        assert!(std::ptr::eq(validated.layout(), &layout));
    }
}

#[test]
fn accepts_supported_reference_kind_space_and_borrow_combinations() {
    for kind in [ReferenceKind::Pointer, ReferenceKind::Slice] {
        for address_space in [AddressSpace::Global, AddressSpace::Generic] {
            let shared = reference_layout(
                kind,
                PointerWidth::Bits64,
                address_space,
                Mutability::Immutable,
                Access::ReadOnly,
                ArgumentOwnership::SharedBorrow,
                AliasClass::SharedReadOnly,
            );
            assert!(HostLaunchAbi::validate(&shared).is_ok());

            for access in [Access::ReadOnly, Access::WriteOnly, Access::ReadWrite] {
                let unique = reference_layout(
                    kind,
                    PointerWidth::Bits64,
                    address_space,
                    Mutability::Mutable,
                    access,
                    ArgumentOwnership::UniqueBorrow,
                    AliasClass::Exclusive,
                );
                assert!(HostLaunchAbi::validate(&unique).is_ok());
            }
        }
    }
}

#[test]
fn rejects_every_non_host_reference_address_space() {
    for kind in [ReferenceKind::Pointer, ReferenceKind::Slice] {
        for address_space in [
            AddressSpace::Constant,
            AddressSpace::Workgroup,
            AddressSpace::Private,
        ] {
            let layout = reference_layout(
                kind,
                PointerWidth::Bits64,
                address_space,
                Mutability::Immutable,
                Access::ReadOnly,
                ArgumentOwnership::SharedBorrow,
                AliasClass::SharedReadOnly,
            );
            assert_eq!(
                HostLaunchAbi::validate(&layout),
                Err(HostLaunchAbiError::UnsupportedAddressSpace {
                    field_index: 0,
                    address_space,
                })
            );
        }
    }
}

#[test]
fn rejects_raw_and_atomic_reference_contracts() {
    for kind in [ReferenceKind::Pointer, ReferenceKind::Slice] {
        for address_space in [AddressSpace::Global, AddressSpace::Generic] {
            for (mutability, access) in [
                (Mutability::Immutable, Access::ReadOnly),
                (Mutability::Mutable, Access::ReadOnly),
                (Mutability::Mutable, Access::WriteOnly),
                (Mutability::Mutable, Access::ReadWrite),
            ] {
                let raw = reference_layout(
                    kind,
                    PointerWidth::Bits64,
                    address_space,
                    mutability,
                    access,
                    ArgumentOwnership::RawPointer,
                    AliasClass::Unrestricted,
                );
                assert!(matches!(
                    HostLaunchAbi::validate(&raw),
                    Err(HostLaunchAbiError::UnsupportedReferenceContract {
                        field_index: 0,
                        ownership: ArgumentOwnership::RawPointer,
                        alias_class: AliasClass::Unrestricted,
                        ..
                    })
                ));
            }

            for access in [Access::ReadOnly, Access::WriteOnly, Access::ReadWrite] {
                let atomic = reference_layout(
                    kind,
                    PointerWidth::Bits64,
                    address_space,
                    Mutability::Immutable,
                    access,
                    ArgumentOwnership::SharedBorrow,
                    AliasClass::SharedAtomic,
                );
                assert!(matches!(
                    HostLaunchAbi::validate(&atomic),
                    Err(HostLaunchAbiError::UnsupportedReferenceContract {
                        field_index: 0,
                        ownership: ArgumentOwnership::SharedBorrow,
                        alias_class: AliasClass::SharedAtomic,
                        ..
                    })
                ));
            }
        }
    }
}

#[test]
fn rejects_32_bit_layouts_before_classifying_fields() {
    let scalar = AbiField::new(
        name("value"),
        0,
        4,
        4,
        AbiKind::Scalar(ScalarType::U32),
        Mutability::Immutable,
        Access::ByValue,
        AddressSpace::Value,
        type_identity(),
        ArgumentOwnership::ByValue,
        AliasClass::Value,
    )
    .unwrap();
    let scalar_layout = AbiLayout::new(4, 4, PointerWidth::Bits32, vec![scalar]).unwrap();
    assert_eq!(
        HostLaunchAbi::validate(&scalar_layout),
        Err(HostLaunchAbiError::UnsupportedPointerWidth(
            PointerWidth::Bits32
        ))
    );

    let reference = reference_layout(
        ReferenceKind::Slice,
        PointerWidth::Bits32,
        AddressSpace::Global,
        Mutability::Immutable,
        Access::ReadOnly,
        ArgumentOwnership::SharedBorrow,
        AliasClass::SharedReadOnly,
    );
    assert_eq!(
        HostLaunchAbi::validate(&reference),
        Err(HostLaunchAbiError::UnsupportedPointerWidth(
            PointerWidth::Bits32
        ))
    );
}
