use super::*;
use fe2o3_kernel_ir::StorageLayoutLimitsV1;

const NARROW: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 7,
    edges: 11,
    containment_depth: 3,
    object_bytes: 127,
};
const EXPLICIT: ProductionSemanticKirLimitsV1 = ProductionSemanticKirLimitsV1::new(2, 3, 4)
    .with_storage_layout_limits(NARROW)
    .with_argument_correspondence_limits(13, 17);

#[test]
fn existing_const_constructors_retain_one_fixed_layout_policy() {
    let expected = StorageLayoutLimitsV1 {
        rows: 1 << 20,
        edges: 1 << 22,
        containment_depth: 256,
        object_bytes: (1_u64 << 61) - 1,
    };
    for limits in [
        ProductionSemanticKirLimitsV1::default(),
        ProductionSemanticKirLimitsV1::new(0, 0, 0),
        ProductionSemanticKirLimitsV1::new_with_max_operations(1, 2, 3, 4),
    ] {
        assert_eq!(limits.storage_layout_limits(), expected);
    }
    let target = fe2o3_mir_model::semantic_mir_v1::SemanticTargetDataLayoutV1::gfx942(
        fe2o3_mir_model::semantic_mir_v1::SemanticLayoutIdentityV1::from_sha256([250; 32]),
    );
    assert_eq!(expected.object_bytes + 1, target.object_size_bound_bytes());
}

#[test]
fn caller_policy_survives_other_builders_and_copy_without_widening() {
    assert_eq!(EXPLICIT.storage_layout_limits(), NARROW);
    assert_eq!(
        (
            EXPLICIT.max_functions,
            EXPLICIT.max_blocks,
            EXPLICIT.max_statements
        ),
        (2, 3, 4)
    );
    assert_eq!(
        (
            EXPLICIT.max_argument_correspondence_work,
            EXPLICIT.max_argument_correspondence_storage
        ),
        (13, 17)
    );
    let copied = EXPLICIT;
    assert_eq!(copied, EXPLICIT);
    let reverse = ProductionSemanticKirLimitsV1::new(2, 3, 4)
        .with_argument_correspondence_limits(13, 17)
        .with_storage_layout_limits(NARROW);
    assert_eq!(reverse, EXPLICIT);
}

#[test]
fn zero_and_narrow_object_limits_are_not_inferred_from_candidate_rows() {
    let zero = StorageLayoutLimitsV1 {
        rows: 0,
        edges: 0,
        containment_depth: 0,
        object_bytes: 0,
    };
    assert_eq!(
        ProductionSemanticKirLimitsV1::default()
            .with_storage_layout_limits(zero)
            .storage_layout_limits(),
        zero
    );
    assert_eq!(EXPLICIT.storage_layout_limits().object_bytes, 127);
}
