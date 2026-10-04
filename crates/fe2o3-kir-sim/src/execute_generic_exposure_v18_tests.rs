//! Concrete backing and exposed SSA types have independent checks.
use super::*;

fn pointer(space: AddressSpace, access: AccessMode) -> PointerValue {
    PointerValue {
        exposed_generic: false,
        allocation: 7,
        byte_offset: 4,
        element: ScalarType::U32,
        address_space: space,
        access,
        lower_bound: 4,
        upper_bound: 12,
        abi_argument_ordinal: 3,
    }
}

fn make_allocation(space: AddressSpace, access: AccessMode) -> Allocation {
    Allocation {
        address_space: space,
        access,
        alignment: 4,
        bytes: vec![0; 16],
        initialized: vec![true; 16],
        workgroup_published: vec![true; 16],
        workgroup_writer: vec![0; 16],
        observation_descriptor: Vec::new(),
    }
}

fn exposed(pointer: &PointerValue) -> PointerValue {
    let value = expose(
        &RuntimeValue::Pointer(pointer.clone()),
        CastKind::PointerToGeneric,
        &Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Generic,
            pointer.access,
        ),
        SimulationTargetV1::amdgpu_64(),
    )
    .unwrap();
    let RuntimeValue::Pointer(pointer) = value else {
        panic!("pointer exposure")
    };
    pointer
}

#[test]
fn generic_exposure_keeps_exact_concrete_backing_and_permissions() {
    for space in [
        AddressSpace::Global,
        AddressSpace::Private,
        AddressSpace::Workgroup,
        AddressSpace::Constant,
    ] {
        for access in [
            AccessMode::ReadOnly,
            AccessMode::WriteOnly,
            AccessMode::ReadWrite,
        ] {
            if space == AddressSpace::Constant && access != AccessMode::ReadOnly {
                continue;
            }
            let original = pointer(space, access);
            let actual = exposed(&original);
            let mut expected = original.clone();
            expected.exposed_generic = true;
            assert_eq!(actual, expected);
            assert_eq!(actual.logical_address_space(), AddressSpace::Generic);
            assert_eq!(actual.address_space, space);
            assert!(!original.exposed_generic);
            assert_eq!(
                runtime_type(&RuntimeValue::Pointer(actual)),
                Type::pointer(Type::Scalar(ScalarType::U32), AddressSpace::Generic, access)
            );
        }
    }
}

#[test]
fn generic_slice_exposure_preserves_extent_and_debug_logical_space() {
    let original = SliceValue {
        exposed_generic: false,
        allocation: 7,
        elements: 2,
        element: ScalarType::U32,
        address_space: AddressSpace::Global,
        access: AccessMode::ReadOnly,
        byte_offset: 4,
        byte_len: 8,
        abi_argument_ordinal: 3,
    };
    let actual = expose(
        &RuntimeValue::Slice(original.clone()),
        CastKind::SliceToGeneric,
        &Type::slice(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Generic,
            AccessMode::ReadOnly,
        ),
        SimulationTargetV1::amdgpu_64(),
    )
    .unwrap();
    let mut expected = original;
    expected.exposed_generic = true;
    assert_eq!(actual, RuntimeValue::Slice(expected));
    assert!(matches!(
        debug_value(&actual),
        Some(SimulationDebugValueV1::Slice {
            allocation: 7,
            address_space: AddressSpace::Generic,
            byte_offset: 4,
            byte_len: 8,
            ..
        })
    ));
}

#[test]
fn generic_exposure_refuses_type_permission_direction_and_reexposure_changes() {
    for access in [
        AccessMode::ReadOnly,
        AccessMode::WriteOnly,
        AccessMode::ReadWrite,
    ] {
        let value = RuntimeValue::Pointer(pointer(AddressSpace::Global, access));
        for changed in [
            AccessMode::ReadOnly,
            AccessMode::WriteOnly,
            AccessMode::ReadWrite,
        ] {
            if changed == access {
                continue;
            }
            assert!(matches!(
                expose(
                    &value,
                    CastKind::PointerToGeneric,
                    &Type::pointer(
                        Type::Scalar(ScalarType::U32),
                        AddressSpace::Generic,
                        changed
                    ),
                    SimulationTargetV1::amdgpu_64()
                ),
                Err(SimulationExecutionErrorKindV1::RuntimeType { .. })
            ));
        }
        for ty in [
            Type::pointer(Type::Scalar(ScalarType::I32), AddressSpace::Generic, access),
            Type::pointer(Type::Scalar(ScalarType::U32), AddressSpace::Private, access),
            Type::slice(Type::Scalar(ScalarType::U32), AddressSpace::Generic, access),
        ] {
            assert!(matches!(
                expose(
                    &value,
                    CastKind::PointerToGeneric,
                    &ty,
                    SimulationTargetV1::amdgpu_64()
                ),
                Err(SimulationExecutionErrorKindV1::RuntimeType { .. })
            ));
        }
        let value = RuntimeValue::Pointer(exposed(&pointer(AddressSpace::Global, access)));
        assert!(matches!(
            expose(
                &value,
                CastKind::PointerToGeneric,
                &runtime_type(&value),
                SimulationTargetV1::amdgpu_64()
            ),
            Err(SimulationExecutionErrorKindV1::RuntimeType { .. })
        ));
    }
}

#[test]
fn generic_access_requires_exposure_in_both_directions() {
    let original = pointer(AddressSpace::Global, AccessMode::ReadWrite);
    let generic = exposed(&original);
    let allocation = make_allocation(AddressSpace::Global, AccessMode::ReadWrite);
    for write in [false, true] {
        validate_access(
            &allocation,
            &original,
            MemoryAccess::new(AddressSpace::Global, 4),
            4,
            write,
        )
        .unwrap();
        validate_access(
            &allocation,
            &generic,
            MemoryAccess::new(AddressSpace::Generic, 4),
            4,
            write,
        )
        .unwrap();
        assert_eq!(
            validate_access(
                &allocation,
                &original,
                MemoryAccess::new(AddressSpace::Generic, 4),
                4,
                write
            ),
            Err(SimulationExecutionErrorKindV1::AddressSpaceMismatch)
        );
        assert_eq!(
            validate_access(
                &allocation,
                &generic,
                MemoryAccess::new(AddressSpace::Global, 4),
                4,
                write
            ),
            Err(SimulationExecutionErrorKindV1::AddressSpaceMismatch)
        );
    }
}

#[test]
fn generic_access_does_not_expand_permissions_alignment_or_view_bounds() {
    let access = MemoryAccess::new(AddressSpace::Generic, 4);
    let original = pointer(AddressSpace::Global, AccessMode::ReadWrite);
    let mut generic = exposed(&original);
    let allocation = make_allocation(AddressSpace::Global, AccessMode::ReadWrite);
    generic.access = AccessMode::ReadOnly;
    assert_eq!(
        validate_access(&allocation, &generic, access, 4, true),
        Err(SimulationExecutionErrorKindV1::ReadOnlyWrite)
    );
    generic.access = AccessMode::WriteOnly;
    assert_eq!(
        validate_access(&allocation, &generic, access, 4, false),
        Err(SimulationExecutionErrorKindV1::WriteOnlyRead)
    );
    generic.access = AccessMode::ReadWrite;
    generic.byte_offset = 5;
    assert!(matches!(
        validate_access(&allocation, &generic, access, 4, false),
        Err(SimulationExecutionErrorKindV1::MisalignedAccess { .. })
    ));
    generic.byte_offset = 12;
    assert!(validate_access(&allocation, &generic, access, 4, false).is_err());
    generic.byte_offset = 0;
    assert!(validate_access(&allocation, &generic, access, 4, false).is_err());
    let constant = exposed(&pointer(AddressSpace::Constant, AccessMode::ReadOnly));
    let backing = make_allocation(AddressSpace::Constant, AccessMode::ReadOnly);
    assert_eq!(
        validate_access(&backing, &constant, access, 4, true),
        Err(SimulationExecutionErrorKindV1::ReadOnlyWrite)
    );
}

#[test]
fn generic_exposure_does_not_forge_allocation_identity_or_lifetime() {
    let generic = exposed(&pointer(AddressSpace::Private, AccessMode::ReadOnly));
    let mut memory = Memory {
        allocations: HashMap::from([(
            7,
            make_allocation(AddressSpace::Private, AccessMode::ReadOnly),
        )]),
        argument_allocations: vec![],
        shared_allocations: HashMap::new(),
        next_allocation: 8,
        allocations_created: 1,
        live_bytes: 16,
        reuse: None,
    };
    assert!(memory.allocation(&generic).is_ok());
    memory.allocations.get_mut(&7).unwrap().address_space = AddressSpace::Global;
    assert!(matches!(
        memory.allocation(&generic),
        Err(SimulationExecutionErrorKindV1::AddressSpaceMismatch)
    ));
    memory.allocations.remove(&7);
    assert!(matches!(
        memory.allocation(&generic),
        Err(SimulationExecutionErrorKindV1::DanglingPointer { allocation: 7 })
    ));
    assert!(matches!(
        debug_value(&RuntimeValue::Pointer(generic)),
        Some(SimulationDebugValueV1::Pointer {
            allocation: 7,
            address_space: AddressSpace::Generic,
            lower_bound: 4,
            upper_bound: 12,
            ..
        })
    ));
}

#[test]
fn generic_exposure_admission_keeps_numeric_and_slice_source_boundaries() {
    let target = SimulationTargetV1::amdgpu_64();
    for kind in [CastKind::PointerToGeneric, CastKind::SliceToGeneric] {
        assert!(!crate::preflight::supported_cast(
            kind,
            ScalarType::U32,
            ScalarType::U32,
            target
        ));
        for space in [
            AddressSpace::Global,
            AddressSpace::Private,
            AddressSpace::Workgroup,
            AddressSpace::Constant,
            AddressSpace::Generic,
        ] {
            for access in [
                AccessMode::ReadOnly,
                AccessMode::WriteOnly,
                AccessMode::ReadWrite,
            ] {
                let expected = space != AddressSpace::Generic
                    && (space != AddressSpace::Constant || access == AccessMode::ReadOnly)
                    && (kind == CastKind::PointerToGeneric
                        || matches!(space, AddressSpace::Global | AddressSpace::Constant));
                let ty = |space| {
                    if kind == CastKind::PointerToGeneric {
                        Type::pointer(Type::Scalar(ScalarType::U32), space, access)
                    } else {
                        Type::slice(Type::Scalar(ScalarType::U32), space, access)
                    }
                };
                assert_eq!(
                    crate::generic_exposure_v18::supports_cast(
                        kind,
                        &ty(space),
                        &ty(AddressSpace::Generic),
                        target,
                    ),
                    expected
                );
                let original = if kind == CastKind::PointerToGeneric {
                    RuntimeValue::Pointer(pointer(space, access))
                } else {
                    RuntimeValue::Slice(SliceValue {
                        exposed_generic: false,
                        allocation: 7,
                        elements: 2,
                        element: ScalarType::U32,
                        address_space: space,
                        access,
                        byte_offset: 4,
                        byte_len: 8,
                        abi_argument_ordinal: 3,
                    })
                };
                assert_eq!(
                    expose(&original, kind, &ty(AddressSpace::Generic), target).is_ok(),
                    expected
                );
            }
        }
    }
}
