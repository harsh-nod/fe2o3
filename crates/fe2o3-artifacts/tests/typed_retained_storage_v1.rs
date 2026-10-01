//! Public API controls using original inert data constructors, not compiler authority.
use fe2o3_artifacts::{
    BlockSize, Dimensions, LaunchContract, PointerWidth, RustLayoutEvidenceV1,
    RustPhysicalComponentKindV1, RustPhysicalComponentV1, RustScalarElementTypeV1,
    RustSourceTypeShapeV1, RustTypeEvidenceV1, RustcAbiClassV1,
};
use std::mem::size_of;

fn layout() -> (RustLayoutEvidenceV1, usize) {
    let mut components = Vec::with_capacity(65);
    components.push(
        RustPhysicalComponentV1::new(
            0,
            4,
            4,
            RustPhysicalComponentKindV1::Scalar {
                scalar: RustScalarElementTypeV1::U32,
            },
        )
        .unwrap(),
    );
    let capacity = components.capacity();
    let value = RustLayoutEvidenceV1::new(
        RustTypeEvidenceV1::new(RustSourceTypeShapeV1::scalar(RustScalarElementTypeV1::U32)),
        RustcAbiClassV1::Scalar,
        PointerWidth::Bits64,
        4,
        4,
        components,
    )
    .unwrap();
    (value, capacity)
}

#[test]
fn original_layout_preserves_spare_capacity_and_canonical_bytes() {
    let (value, capacity) = layout();
    let before = value.canonical_bytes();
    let mut extents = Vec::new();
    value
        .visit_retained_heap_storage_v1(|n, w| {
            extents.push((n, w));
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(extents, [(capacity, size_of::<RustPhysicalComponentV1>())]);
    assert!(capacity > value.components().len());
    assert_eq!(value.canonical_bytes(), before);
}

#[test]
fn layout_first_callback_refusal_is_returned_without_retry() {
    let (value, _) = layout();
    let mut calls = 0;
    assert_eq!(
        value.visit_retained_heap_storage_v1(|_, _| {
            calls += 1;
            Err::<(), _>("stop")
        }),
        Err("stop")
    );
    assert_eq!(calls, 1);
}

#[test]
fn all_original_launch_shapes_have_one_zero_heap_visit_and_refuse() {
    let dimensions = Dimensions::new(8, 1, 1).unwrap();
    for block in [
        BlockSize::Any,
        BlockSize::Exact(dimensions),
        BlockSize::AtMost(dimensions),
    ] {
        let value = LaunchContract::new(1, block, dimensions, 0, 8).unwrap();
        let before = value.clone();
        let mut extents = Vec::new();
        value
            .visit_retained_heap_storage_v1(|n, w| {
                extents.push((n, w));
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(extents, [(0, 1)]);
        assert_eq!(value, before);
        let mut calls = 0;
        assert_eq!(
            value.visit_retained_heap_storage_v1(|_, _| {
                calls += 1;
                Err::<(), _>(7)
            }),
            Err(7)
        );
        assert_eq!(calls, 1);
    }
}
