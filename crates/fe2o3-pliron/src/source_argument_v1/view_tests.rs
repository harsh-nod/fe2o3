use super::*;

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);

#[test]
fn atomic_argument_child_preserves_wide_source_indices() {
    // Test the production selector directly; do not pretend to visit billions of nodes.
    let shape = SemanticTypeShapeV1::Array {
        element: UNIT,
        length: u64::MAX,
    };
    for index in [u64::from(u32::MAX), u64::from(u32::MAX) + 1, u64::MAX - 1] {
        assert_eq!(
            atomic_argument_child_v1(&shape, index).unwrap(),
            Some((UNIT, ProductionArgumentProjectionV1::ArrayIndex(index)))
        );
    }
    assert_eq!(atomic_argument_child_v1(&shape, u64::MAX).unwrap(), None);
}
