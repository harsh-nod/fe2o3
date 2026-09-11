    #[test]
    fn tiled_geometry_requires_finite_equal_products() {
        for ([lanes, rows, columns, elements], expected) in [
            ([4, 2, 4, 2], true),
            ([8, 2, 4, 1], true),
            ([64, 16, 16, 4], true),
            ([u64::MAX, 1, u64::MAX, 1], true),
            ([1, u64::MAX, 1, u64::MAX], true),
            ([u64::MAX, u64::MAX, 1, 1], true),
            ([(1_u64 << 63) - 1, 2, (1_u64 << 63) - 1, 2], true),
            ([1_u64 << 63, 2, 1_u64 << 63, 2], false),
            ([1_u64 << 63, 4, 1_u64 << 62, 2], false),
            ([u64::MAX, 2, u64::MAX, 2], false),
            ([2, u64::MAX, 2, u64::MAX], false),
            ([u64::MAX, 1, u64::MAX, 2], false),
            ([1, u64::MAX, 2, 1], false),
            ([4, 3, 4, 2], false),
            ([3, 2, 2, 1], false),
            ([0, 2, 4, 2], false),
            ([4, 0, 4, 2], false),
            ([4, 2, 0, 2], false),
            ([4, 2, 4, 0], false),
        ] {
            assert_eq!(
                tiled_2d_geometry_valid(lanes, rows, columns, elements),
                expected,
                "geometry {:?}",
                [lanes, rows, columns, elements],
            );
        }
    }

mod semantic_ssa_transport_tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticAtomicAccessV1, SemanticBorrowKindV1, SemanticMemoryStoreV1,
        SemanticPointerTypeV1,
    };

    include!("semantic_ssa_capability_01_tests.rs");
    include!("semantic_ssa_transport_01_tests.rs");
    include!("semantic_ssa_enum_01_tests.rs");
}
