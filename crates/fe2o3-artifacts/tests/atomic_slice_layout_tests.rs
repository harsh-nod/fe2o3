use fe2o3_artifacts::{
    PointerWidth, RustLayoutEvidenceV1, RustPhysicalComponentKindV1, RustPhysicalComponentV1,
    RustPointerMutabilityV1, RustScalarElementTypeV1, RustSourceTypeShapeV1, RustTypeEvidenceV1,
    RustcAbiClassV1,
};

fn components(
    pointee: RustScalarElementTypeV1,
    mutability: RustPointerMutabilityV1,
) -> Vec<RustPhysicalComponentV1> {
    vec![
        RustPhysicalComponentV1::new(
            0,
            8,
            8,
            RustPhysicalComponentKindV1::Pointer {
                mutability,
                pointee,
            },
        )
        .unwrap(),
        RustPhysicalComponentV1::new(8, 8, 8, RustPhysicalComponentKindV1::Usize).unwrap(),
    ]
}

#[test]
fn shared_atomic_u32_identity_is_not_readonly_or_disjoint_storage() {
    let atomic = RustLayoutEvidenceV1::new(
        RustTypeEvidenceV1::new(RustSourceTypeShapeV1::SharedAtomicSliceU32),
        RustcAbiClassV1::ScalarPair,
        PointerWidth::Bits64,
        16,
        8,
        components(RustScalarElementTypeV1::U32, RustPointerMutabilityV1::Const),
    )
    .unwrap();
    let readonly = RustLayoutEvidenceV1::new(
        RustTypeEvidenceV1::new(RustSourceTypeShapeV1::shared_slice(
            RustScalarElementTypeV1::U32,
        )),
        RustcAbiClassV1::ScalarPair,
        PointerWidth::Bits64,
        16,
        8,
        components(RustScalarElementTypeV1::U32, RustPointerMutabilityV1::Const),
    )
    .unwrap();
    assert_ne!(atomic.type_identity(), readonly.type_identity());
    assert_eq!(
        RustSourceTypeShapeV1::SharedAtomicSliceU32.element(),
        RustScalarElementTypeV1::U32
    );
    for (element, mutability) in [
        (RustScalarElementTypeV1::U64, RustPointerMutabilityV1::Const),
        (RustScalarElementTypeV1::U32, RustPointerMutabilityV1::Mut),
    ] {
        assert!(
            RustLayoutEvidenceV1::new(
                RustTypeEvidenceV1::new(RustSourceTypeShapeV1::SharedAtomicSliceU32),
                RustcAbiClassV1::ScalarPair,
                PointerWidth::Bits64,
                16,
                8,
                components(element, mutability),
            )
            .is_err()
        );
    }
}
