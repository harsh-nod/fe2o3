use super::*;
use crate::{
    AccessMode, Atomic, AtomicKind, BasicBlock, BlockId, Function, MemoryAccess, MemoryOrdering,
    ScalarType, Signature, Terminator, Type, ValueDef, ValueId,
};

fn atomic() -> Atomic {
    Atomic {
        kind: AtomicKind::Load,
        pointer: ValueId(7),
        value: None,
        compare: None,
        access: MemoryAccess::new(AddressSpace::Global, 8),
        scope: SynchronizationScope::Device,
        ordering: MemoryOrdering::Acquire,
        failure_ordering: None,
    }
}

#[test]
fn descriptor_atomic_classifier_preserves_exact_fixed_width_rule() {
    for scalar in [
        ScalarType::Bool,
        ScalarType::I8,
        ScalarType::U8,
        ScalarType::I16,
        ScalarType::U16,
        ScalarType::I32,
        ScalarType::U32,
        ScalarType::I64,
        ScalarType::U64,
        ScalarType::I128,
        ScalarType::U128,
        ScalarType::Index,
        ScalarType::F16,
        ScalarType::Bf16,
        ScalarType::F32,
        ScalarType::F64,
    ] {
        let ty = Type::pointer(
            Type::Scalar(scalar),
            AddressSpace::Global,
            AccessMode::ReadWrite,
        );
        let expected = scalar
            .bit_width()
            .filter(|width| matches!(width, 8 | 16 | 32 | 64))
            .map(|width_bits| TargetCapability::Atomic {
                width_bits,
                address_space: AddressSpace::Global,
                max_scope: SynchronizationScope::Device,
            });
        assert_eq!(atomic_pointer_capability_v1(&atomic(), &ty), expected);
        assert_eq!(
            atomic_pointer_capability_v1(&atomic(), &Type::Scalar(scalar)),
            None
        );
    }
    let nested = Type::pointer(
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    assert_eq!(atomic_pointer_capability_v1(&atomic(), &nested), None);
}

#[test]
fn descriptor_atomic_classifier_retains_access_space_and_scope_not_pointer_space() {
    let ty = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    for space in [
        AddressSpace::Private,
        AddressSpace::Global,
        AddressSpace::Workgroup,
    ] {
        for scope in [
            SynchronizationScope::Invocation,
            SynchronizationScope::Workgroup,
            SynchronizationScope::Device,
        ] {
            let mut operation = atomic();
            operation.access.address_space = space;
            operation.scope = scope;
            assert_eq!(
                atomic_pointer_capability_v1(&operation, &ty),
                Some(TargetCapability::Atomic {
                    width_bits: 32,
                    address_space: space,
                    max_scope: scope,
                })
            );
        }
    }
}

#[test]
fn descriptor_borrowed_public_visitor_matches_owned_operation_and_function_closure() {
    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let mut block = BasicBlock::new(BlockId(4));
    block.operations = vec![
        Operation::new(
            vec![ValueDef::new(ValueId(9), Type::Scalar(ScalarType::U32))],
            OperationKind::Atomic(atomic()),
        ),
        Operation::effect_free(
            ValueDef::new(
                ValueId(10),
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Workgroup,
                    AccessMode::ReadWrite,
                ),
            ),
            OperationKind::Alloca {
                element: Type::Scalar(ScalarType::U32),
                count: None,
                address_space: AddressSpace::Workgroup,
                alignment: 4,
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let function = Function::internal_helper(
        "atomic",
        Signature::new(vec![pointer.clone()], vec![]),
        vec![ValueId(7)],
        vec![block],
    );
    let mut observed = std::collections::BTreeSet::new();
    for operation in &function.body.as_ref().unwrap().blocks[0].operations {
        let mut borrowed = std::collections::BTreeSet::new();
        operation
            .try_visit_required_capabilities_v1(|cap| {
                borrowed.insert(cap.into_owned());
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(borrowed, operation.required_capabilities());
        observed.extend(borrowed);
        if let OperationKind::Atomic(atomic) = &operation.kind {
            observed.extend(atomic_pointer_capability_v1(atomic, &pointer));
        }
    }
    assert_eq!(observed, function.derived_capabilities());
    assert!(observed.contains(&TargetCapability::Atomic {
        width_bits: 32,
        address_space: AddressSpace::Global,
        max_scope: SynchronizationScope::Device
    }));
    assert!(observed.contains(&TargetCapability::WorkgroupMemory));
}

#[test]
fn descriptor_borrowed_name_extents_and_exact_matching_are_allocation_free_views() {
    for (view, expected) in [
        (TargetCapabilityNameRefV1::Text("00abff"), "00abff"),
        (
            TargetCapabilityNameRefV1::LowerHex(&[0, 0xab, 0xff]),
            "00abff",
        ),
    ] {
        assert_eq!(view.visible_len(), Some(expected.len()));
        assert!(view.matches(expected));
        for wrong in ["00ABFF", "00abf", "00abff0", "000000"] {
            assert!(!view.matches(wrong));
        }
    }
    let cap = TargetCapability::Extension {
        namespace: "n".into(),
        name: "00abff".into(),
    };
    assert_eq!(
        format!("{:?}", TargetCapabilityRefV1::from_owned(&cap)),
        format!("{cap:?}")
    );
}
