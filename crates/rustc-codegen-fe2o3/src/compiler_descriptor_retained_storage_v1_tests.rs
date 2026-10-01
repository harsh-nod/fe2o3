//! Synthetic typed-data controls, not authenticated compiler/launch owners.
use super::*;
use crate::collector::TypedArgumentListV1;
use fe2o3_artifacts::{
    BlockSize, Dimensions, LaunchContract, PointerWidth, RustLayoutEvidenceV1,
    RustPhysicalComponentKindV1, RustPhysicalComponentV1, RustScalarElementTypeV1,
    RustSourceTypeShapeV1, RustTypeEvidenceV1, RustcAbiClassV1,
};
use fe2o3_kernel_descriptor::{AccessMode, ScalarTypeV1};
use fe2o3_kernel_ir::LogicalStorageLimitsV1;
use fe2o3_mir_model::semantic_mir_v1::{SemanticLayoutIdentityV1, SemanticTypeIdentityV1};
use reserved_fe2o3_symbols::KernelBindingIdV1;
use std::mem::size_of;

fn counter(bytes: Option<usize>, items: usize) -> LogicalStorageCounterV1 {
    LogicalStorageCounterV1::new(LogicalStorageLimitsV1 {
        max_bytes: bytes,
        max_items: items,
    })
}
fn text(capacity: usize) -> String {
    let mut value = String::with_capacity(capacity);
    value.push_str("same");
    value
}
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
    let bytes = components.capacity() * size_of::<RustPhysicalComponentV1>();
    let value = RustLayoutEvidenceV1::new(
        RustTypeEvidenceV1::new(RustSourceTypeShapeV1::scalar(RustScalarElementTypeV1::U32)),
        RustcAbiClassV1::Scalar,
        PointerWidth::Bits64,
        4,
        4,
        components,
    )
    .unwrap();
    (value, bytes)
}
fn fixture(with_layout: bool, with_launch: bool) -> (Vec<TypedDescriptorRootV1>, usize, usize) {
    let logical_name = text(23);
    let export_name = text(31);
    let mut expected = logical_name.capacity() + export_name.capacity();
    let mut arguments = Vec::with_capacity(5);
    for i in 0..2 {
        let name = text(11 + i * 8);
        expected += name.capacity();
        let component = if with_layout && i == 0 {
            let (value, bytes) = layout();
            expected += bytes;
            Some(value)
        } else {
            None
        };
        arguments.push(TypedDescriptorArgumentV1 {
            name,
            kind: DescriptorArgumentKindV1::Scalar(ScalarTypeV1::U32),
            access: AccessMode::ByValue,
            offset: (i * 4) as u32,
            layout: component,
            source_size: 4,
            source_alignment: 4,
            rustc_abi_class: RustcAbiClassV1::Scalar,
            semantic_type_identity: SemanticTypeIdentityV1::from_sha256([1; 32]),
            semantic_layout_identity: SemanticLayoutIdentityV1::from_sha256([2; 32]),
        });
    }
    expected += arguments.capacity() * size_of::<TypedDescriptorArgumentV1>();
    let source_launch = with_launch.then(|| {
        LaunchContract::new(1, BlockSize::Any, Dimensions::new(64, 1, 1).unwrap(), 0, 0).unwrap()
    });
    // Existing typed-data fixture construction only. No actual rustc source,
    // AuthenticatedProductionBindings, validation or launch authority is claimed.
    let root = TypedDescriptorRootV1 {
        logical_name,
        export_name,
        kernel_binding: KernelBindingIdV1::from_bytes([3; 32]),
        arguments: TypedArgumentListV1::new(arguments).unwrap(),
        explicit_argument_bytes: 8,
        kernarg_alignment_bytes: 4,
        source_launch,
    };
    let mut roots = Vec::with_capacity(4);
    roots.push(root);
    expected += roots.capacity() * size_of::<TypedDescriptorRootV1>();
    let items = 2 + 4 + 2 * 2 + usize::from(with_layout) + usize::from(with_launch);
    (roots, expected, items)
}

#[test]
fn complete_nonempty_branch_uses_independent_capacity_formula_and_exact_limits() {
    let (roots, expected, items) = fixture(true, true);
    let before = roots.clone();
    let pointer = roots.as_ptr();
    let canonical = roots[0].arguments.as_slice()[0]
        .layout
        .as_ref()
        .unwrap()
        .canonical_bytes();
    let mut c = counter(Some(expected), items);
    TypedDescriptorRootV1::charge_roster_retained_heap_v1(&roots, &mut c).unwrap();
    assert_eq!((c.bytes(), c.items()), (expected, items));
    assert_eq!(roots, before);
    assert_eq!(roots.as_ptr(), pointer);
    assert_eq!(
        roots[0].arguments.as_slice()[0]
            .layout
            .as_ref()
            .unwrap()
            .canonical_bytes(),
        canonical
    );
}

#[test]
fn each_optional_layout_and_launch_branch_is_complete() {
    for layout in [false, true] {
        for launch in [false, true] {
            let (roots, bytes, items) = fixture(layout, launch);
            let mut c = counter(Some(bytes), items);
            TypedDescriptorRootV1::charge_roster_retained_heap_v1(&roots, &mut c).unwrap();
            assert_eq!((c.bytes(), c.items()), (bytes, items));
        }
    }
}

#[test]
fn empty_and_spare_root_capacity_are_not_length_or_header_substitutes() {
    for roots in [Vec::new(), Vec::with_capacity(7)] {
        let expected = roots.capacity() * size_of::<TypedDescriptorRootV1>();
        let mut c = counter(Some(expected), 2);
        TypedDescriptorRootV1::charge_roster_retained_heap_v1(&roots, &mut c).unwrap();
        assert_eq!((c.bytes(), c.items()), (expected, 2));
    }
}

#[test]
fn one_short_bytes_and_items_refuse_without_inventing_a_full_result() {
    let (roots, bytes, items) = fixture(true, true);
    let mut short_bytes = counter(Some(bytes - 1), items);
    assert_eq!(
        TypedDescriptorRootV1::charge_roster_retained_heap_v1(&roots, &mut short_bytes),
        Err(LogicalStorageErrorV1::ByteLimit)
    );
    assert!(short_bytes.bytes() < bytes);
    let mut short_items = counter(Some(bytes), items - 1);
    assert_eq!(
        TypedDescriptorRootV1::charge_roster_retained_heap_v1(&roots, &mut short_items),
        Err(LogicalStorageErrorV1::ItemLimit)
    );
    assert_eq!(short_items.items(), items - 1);
}

#[test]
fn branch_and_roster_refusals_precede_nested_payloads() {
    let (roots, _, _) = fixture(true, true);
    let mut c = counter(None, 0);
    assert_eq!(
        TypedDescriptorRootV1::charge_roster_retained_heap_v1(&roots, &mut c),
        Err(LogicalStorageErrorV1::ItemLimit)
    );
    assert_eq!((c.bytes(), c.items()), (0, 0));
    let mut c = counter(Some(0), usize::MAX);
    assert_eq!(
        TypedDescriptorRootV1::charge_roster_retained_heap_v1(&roots, &mut c),
        Err(LogicalStorageErrorV1::ByteLimit)
    );
    assert_eq!((c.bytes(), c.items()), (0, 1));
}

#[test]
fn existing_shared_counter_overflow_and_extent_overflow_are_atomic() {
    let (roots, _, _) = fixture(true, true);
    let mut c = counter(None, usize::MAX);
    c.charge(usize::MAX, 0).unwrap();
    assert_eq!(
        TypedDescriptorRootV1::charge_roster_retained_heap_v1(&roots, &mut c),
        Err(LogicalStorageErrorV1::Arithmetic)
    );
    assert_eq!((c.bytes(), c.items()), (usize::MAX, 1));
    let mut c = counter(None, usize::MAX);
    assert_eq!(
        extent(&mut c, usize::MAX, 2),
        Err(LogicalStorageErrorV1::Arithmetic)
    );
    assert_eq!((c.bytes(), c.items()), (0, 0));
    c.charge(0, usize::MAX).unwrap();
    assert_eq!(
        TypedDescriptorRootV1::charge_roster_retained_heap_v1(&roots, &mut c),
        Err(LogicalStorageErrorV1::Arithmetic)
    );
    assert_eq!((c.bytes(), c.items()), (0, usize::MAX));
}

#[test]
fn all_current_argument_kind_variants_are_fixed_payloads() {
    let kinds = [
        DescriptorArgumentKindV1::SharedSlice(ScalarTypeV1::U32),
        DescriptorArgumentKindV1::DisjointSlice(ScalarTypeV1::U32),
        DescriptorArgumentKindV1::GlobalMutPointer(ScalarTypeV1::U32),
        DescriptorArgumentKindV1::Scalar(ScalarTypeV1::U32),
        DescriptorArgumentKindV1::CompilerLaidOutByValue,
        DescriptorArgumentKindV1::CompilerLaidOutUsize,
        DescriptorArgumentKindV1::CompilerLaidOutIsize,
    ];
    for kind in kinds {
        let argument = TypedDescriptorArgumentV1 {
            name: text(13),
            kind,
            access: AccessMode::ByValue,
            offset: 0,
            layout: None,
            source_size: 4,
            source_alignment: 4,
            rustc_abi_class: RustcAbiClassV1::Scalar,
            semantic_type_identity: SemanticTypeIdentityV1::from_sha256([1; 32]),
            semantic_layout_identity: SemanticLayoutIdentityV1::from_sha256([2; 32]),
        };
        let mut c = counter(Some(argument.name.capacity()), 1);
        charge_argument(&argument, &mut c).unwrap();
        assert_eq!((c.bytes(), c.items()), (argument.name.capacity(), 1));
    }
}
