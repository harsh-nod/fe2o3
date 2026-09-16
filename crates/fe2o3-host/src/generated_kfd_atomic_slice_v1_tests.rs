use super::*;
use fe2o3_artifacts::{
    AbiField, AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership, Mutability,
    Name, PointerWidth,
};
use std::sync::atomic::{AtomicU32, Ordering};

fn field() -> AbiField {
    AbiField::new(
        Name::new("channels").unwrap(),
        0,
        16,
        8,
        AbiKind::Slice {
            element_size: 4,
            element_alignment: 4,
        },
        Mutability::Immutable,
        Access::ReadWrite,
        AddressSpace::Global,
        GeneratedKfdAtomicSliceU32::type_identity_v1(PointerWidth::Bits64),
        ArgumentOwnership::SharedBorrow,
        AliasClass::SharedAtomic,
    )
    .unwrap()
}

fn plan_for_field(field: AbiField) -> GeneratedArgumentPackingPlanV1 {
    let manifest = AbiLayout::new(16, 8, PointerWidth::Bits64, vec![field.clone()]).unwrap();
    let generated =
        CompilerGeneratedArgumentLayoutV1::new(16, 8, PointerWidth::Bits64, vec![field]).unwrap();
    crate::generated_argument_plan::validate_argument_packing(
        KernelId::from_bytes([42; 32]),
        &manifest,
        &generated,
    )
    .unwrap()
}

#[test]
fn generated_atomic_slice_packing_is_address_free_readwrite_and_exact_u32() {
    let plan = plan_for_field(field());
    let mut values = [AtomicU32::new(0x0102_0304), AtomicU32::new(u32::MAX)];
    let slice = GeneratedKfdAtomicSliceU32::new(&mut values);
    assert_eq!(slice.len(), 2);
    assert!(!slice.is_empty());
    let binding = slice.bind_argument(&plan, 0).unwrap();
    let mut packed =
        GeneratedKfdArgumentBinding::from_compiler_generated_parts(vec![], vec![binding])
            .pack(&plan)
            .unwrap();
    assert_eq!(
        packed.buffers()[0].access(),
        Gfx942RuntimeBufferAccessV1::ReadWrite
    );
    assert_eq!(
        packed.buffers()[0].bytes(),
        &[4, 3, 2, 1, 255, 255, 255, 255]
    );
    assert_eq!(
        packed.explicit_kernarg(),
        &[0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(packed.pointer_fixups().len(), 1);
    let writeback = packed.completion.buffers[0].writeback.take().unwrap();
    writeback.apply(&[7, 0, 0, 0, 9, 0, 0, 0]);
    drop(packed);
    assert_eq!(values[0].load(Ordering::Relaxed), 7);
    assert_eq!(values[1].load(Ordering::Relaxed), 9);
}

#[test]
fn generated_atomic_slice_rejects_readonly_disjoint_and_extent_substitution() {
    for mutable in [false, true] {
        let ordinary = AbiField::new(
            Name::new("values").unwrap(),
            0,
            16,
            8,
            AbiKind::Slice {
                element_size: 4,
                element_alignment: 4,
            },
            if mutable {
                Mutability::Mutable
            } else {
                Mutability::Immutable
            },
            if mutable {
                Access::ReadWrite
            } else {
                Access::ReadOnly
            },
            AddressSpace::Global,
            if mutable {
                u32::disjoint_slice_type_identity_v1(PointerWidth::Bits64)
            } else {
                u32::shared_slice_type_identity_v1(PointerWidth::Bits64)
            },
            if mutable {
                ArgumentOwnership::UniqueBorrow
            } else {
                ArgumentOwnership::SharedBorrow
            },
            if mutable {
                AliasClass::Exclusive
            } else {
                AliasClass::SharedReadOnly
            },
        )
        .unwrap();
        let plan = plan_for_field(ordinary);
        let mut values = [AtomicU32::new(3)];
        assert!(
            GeneratedKfdAtomicSliceU32::new(&mut values)
                .bind_argument(&plan, 0)
                .is_err()
        );
    }
    let plan = plan_for_field(field());
    assert!(
        GeneratedKfdReadSlice::new(&[1_u32])
            .bind_argument(&plan, 0)
            .is_err()
    );
    assert!(
        GeneratedKfdReadWriteSlice::new(&mut [1_u32])
            .bind_argument(&plan, 0)
            .is_err()
    );
    assert!(matches!(
        plan.bind_generated_address_free_atomic_slice_u32_v1(
            0,
            usize::MAX,
            GeneratedArgumentBorrowV1::new()
        ),
        Err(GeneratedArgumentPackError::SliceByteExtentOverflow { .. })
    ));
    assert!(
        plan.bind_generated_address_free_atomic_slice_u32_v1(
            1,
            1,
            GeneratedArgumentBorrowV1::new()
        )
        .is_err()
    );
    assert_ne!(
        GeneratedKfdAtomicSliceU32::type_identity_v1(PointerWidth::Bits64),
        u32::shared_slice_type_identity_v1(PointerWidth::Bits64)
    );
    assert_ne!(
        GeneratedKfdAtomicSliceU32::type_identity_v1(PointerWidth::Bits64),
        u32::disjoint_slice_type_identity_v1(PointerWidth::Bits64)
    );
}

#[test]
fn generated_atomic_slice_empty_and_dropped_bindings_preserve_host_storage() {
    let plan = plan_for_field(field());
    let mut empty = [];
    let slice = GeneratedKfdAtomicSliceU32::new(&mut empty);
    assert!(slice.is_empty());
    let binding = slice.bind_argument(&plan, 0).unwrap();
    let packed = GeneratedKfdArgumentBinding::from_compiler_generated_parts(vec![], vec![binding])
        .pack(&plan)
        .unwrap();
    assert_eq!(packed.explicit_kernarg(), &[0; 16]);
    assert!(packed.buffers().is_empty());
    assert!(packed.pointer_fixups().is_empty());
    let mut values = [AtomicU32::new(17)];
    let binding = GeneratedKfdAtomicSliceU32::new(&mut values)
        .bind_argument(&plan, 0)
        .unwrap();
    drop(binding);
    assert_eq!(values[0].load(Ordering::Relaxed), 17);
}

fn descriptor(atomic: bool) -> fe2o3_kernel_descriptor::KernelDescriptorV1 {
    use fe2o3_kernel_descriptor::*;
    let source = SourceTypeRecordV1::new(if atomic {
        SourceTypeDescriptorV1::shared_atomic_slice_u32()
    } else {
        SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::U32)
    });
    let layout = DeviceLayoutRecordV1::new(if atomic {
        DeviceLayoutDescriptorV1::shared_atomic_slice_u32()
    } else {
        DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::U32)
    });
    let name = |value| ValidName::new(value).unwrap();
    let argument = if atomic {
        LogicalArgumentV1::shared_atomic_slice_u32(0, name("channels"), &source, &layout, 0)
            .unwrap()
    } else {
        LogicalArgumentV1::shared_slice(0, name("values"), &source, &layout, 0).unwrap()
    };
    let evidence = BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes([1; 32]),
        EvidenceDigest::from_sha256_bytes([2; 32]),
    );
    KernelDescriptorV1::new(
        fe2o3_kernel_descriptor::KernelId::from_bytes([42; 32]),
        name("atomic_test"),
        name("atomic_test"),
        name("atomic_test.kd"),
        evidence,
        evidence,
        vec![CapabilityV1::Atomics],
        KernelAbiLayoutV1::new(16, 16, 8).unwrap(),
        LaunchConstraintsV1::new(
            1,
            BlockSizeV1::Any,
            DimensionsV1::new(256, 1, 1).unwrap(),
            128,
            0,
            0,
        )
        .unwrap(),
        vec![argument],
    )
    .unwrap()
}

#[test]
fn generated_atomic_slice_v1_descriptor_does_not_grant_runtime_coherence_authority() {
    assert!(matches!(
        reject_unjoined_atomic_runtime_v1(&descriptor(true)),
        Err(GeneratedKfdPrepareError::AtomicRuntimeContractUnavailable)
    ));
    assert!(reject_unjoined_atomic_runtime_v1(&descriptor(false)).is_ok());
}
