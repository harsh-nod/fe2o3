#[test]
fn atomic_slice_descriptor_requires_exact_readwrite_u32_global_slice() {
    use fe2o3_kernel_ir::{
        AccessMode as KAccess, AddressSpace as KSpace, ScalarType as KScalar, Type,
    };
    let kind = DescriptorArgumentKindV1::SharedAtomicSliceU32;
    let slice = |scalar, space, access| Type::slice(Type::Scalar(scalar), space, access);
    let exact = slice(KScalar::U32, KSpace::Global, KAccess::ReadWrite);
    assert!(production_descriptor_argument_matches_kernel_type_v1(
        kind,
        AccessMode::ReadWrite,
        &exact
    ));
    for (access, ty) in [
        (AccessMode::ReadOnly, exact.clone()),
        (AccessMode::WriteOnly, exact),
        (
            AccessMode::ReadWrite,
            slice(KScalar::U32, KSpace::Global, KAccess::ReadOnly),
        ),
        (
            AccessMode::ReadWrite,
            slice(KScalar::U32, KSpace::Global, KAccess::WriteOnly),
        ),
        (
            AccessMode::ReadWrite,
            slice(KScalar::I32, KSpace::Global, KAccess::ReadWrite),
        ),
        (
            AccessMode::ReadWrite,
            slice(KScalar::U64, KSpace::Global, KAccess::ReadWrite),
        ),
        (
            AccessMode::ReadWrite,
            slice(KScalar::U32, KSpace::Workgroup, KAccess::ReadWrite),
        ),
        (
            AccessMode::ReadWrite,
            Type::pointer(
                Type::Scalar(KScalar::U32),
                KSpace::Global,
                KAccess::ReadWrite,
            ),
        ),
    ] {
        assert!(!production_descriptor_argument_matches_kernel_type_v1(
            kind, access, &ty
        ));
    }
    let (source, layout) = descriptor_records(kind);
    assert!(source.descriptor().is_shared_atomic_slice_u32());
    assert_eq!(layout.descriptor().size_bytes(), 16);
    assert_eq!(layout.descriptor().alignment_bytes(), 8);
    assert_eq!(
        descriptor_argument_kind(GeneralTypedArgumentKindV3::SharedAtomicSliceU32),
        kind
    );
}

#[test]
fn atomic_slice_descriptor_does_not_discharge_aliases_from_shared_rust_ownership() {
    let mut atomic = descriptor_argument(
        0,
        DescriptorArgumentKindV1::SharedSlice(ScalarTypeV1::U32),
        0,
    );
    atomic.kind = DescriptorArgumentKindV1::SharedAtomicSliceU32;
    atomic.access = AccessMode::ReadWrite;
    let other = atomic.clone();
    assert!(!rust_ownership_discharges_runtime_alias_v1(
        0,
        1,
        &[atomic.clone(), other]
    ));
    let ordinary = descriptor_argument(
        1,
        DescriptorArgumentKindV1::SharedSlice(ScalarTypeV1::U32),
        16,
    );
    assert!(!rust_ownership_discharges_runtime_alias_v1(
        0,
        1,
        &[atomic.clone(), ordinary]
    ));
    let exclusive = descriptor_argument(
        1,
        DescriptorArgumentKindV1::DisjointSlice(ScalarTypeV1::U32),
        16,
    );
    assert!(rust_ownership_discharges_runtime_alias_v1(
        0,
        1,
        &[atomic, exclusive]
    ));
}
