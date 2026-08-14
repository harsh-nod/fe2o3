use fe2o3_tiled_gemm_v1::contract::{TARGET_V1, TILE_K_V1, TILE_M_V1, TILE_N_V1, WAVE_LANES_V1};
use fe2o3_tiled_gemm_v1::{
    EDGE_CASES_V1, ExpectedDecisionV1, LaunchDecisionV1, PlanErrorV1, ShapeErrorV1, ShapeV1,
    TileOriginV1, plan_v1,
};

#[test]
fn exact_profile_is_gfx942_wave64_m16n16k16() {
    assert_eq!(TARGET_V1, "gfx942:xnack-");
    assert_eq!((TILE_M_V1, TILE_N_V1, TILE_K_V1), (16, 16, 16));
    assert_eq!(WAVE_LANES_V1, 64);
}

#[test]
fn checked_shape_binds_row_major_extents_and_indices() {
    let shape = ShapeV1::checked(32, 48, 16).unwrap();
    assert_eq!(shape.dimensions(), [32, 48, 16]);
    assert_eq!(shape.a_elements, 512);
    assert_eq!(shape.b_elements, 768);
    assert_eq!(shape.c_elements, 1_536);
    assert_eq!(shape.a_index(31, 15), Some(511));
    assert_eq!(shape.b_index(15, 47), Some(767));
    assert_eq!(shape.c_index(31, 47), Some(1_535));
    assert_eq!(shape.a_index(32, 0), None);
    assert_eq!(shape.a_index(0, 16), None);
    assert_eq!(shape.b_index(16, 0), None);
    assert_eq!(shape.b_index(0, 48), None);
    assert_eq!(shape.c_index(32, 0), None);
    assert_eq!(shape.c_index(0, 48), None);
}

#[test]
fn shape_byte_accounting_fails_before_impossible_host_extents() {
    assert_eq!(
        ShapeV1::checked(u32::MAX, 1, u32::MAX),
        Err(ShapeErrorV1::ByteCountOverflow("A"))
    );
    assert_eq!(
        ShapeV1::checked(1, u32::MAX, u32::MAX),
        Err(ShapeErrorV1::ByteCountOverflow("B"))
    );
    assert_eq!(
        ShapeV1::checked(u32::MAX, u32::MAX, 1),
        Err(ShapeErrorV1::ByteCountOverflow("C"))
    );
}

#[test]
fn required_edge_matrix_has_the_declared_outcome() {
    for case in EDGE_CASES_V1 {
        let shape = ShapeV1::checked(case.m, case.n, case.k).unwrap();
        let actual = plan_v1(shape);
        match case.expected {
            ExpectedDecisionV1::NoDispatch => assert_eq!(
                actual,
                Ok(LaunchDecisionV1::NoDispatchEmptyOutput),
                "{}",
                case.name
            ),
            ExpectedDecisionV1::HostFill => assert!(
                matches!(actual, Ok(LaunchDecisionV1::HostFillPositiveZero { .. })),
                "{}: {actual:?}",
                case.name
            ),
            ExpectedDecisionV1::Dispatch => assert!(
                matches!(actual, Ok(LaunchDecisionV1::Dispatch(_))),
                "{}: {actual:?}",
                case.name
            ),
            ExpectedDecisionV1::Reject(error) => {
                assert_eq!(actual, Err(error), "{}", case.name)
            }
        }
    }
}

#[test]
fn exact_tiles_map_to_one_wave64_workgroup_each() {
    let shape = ShapeV1::checked(32, 48, 32).unwrap();
    let LaunchDecisionV1::Dispatch(geometry) = plan_v1(shape).unwrap() else {
        panic!("exact tiled shape did not dispatch");
    };
    assert_eq!(geometry.workgroup, [64, 1, 1]);
    assert_eq!(geometry.grid, [192, 2, 1]);
    assert_eq!(geometry.tile_rows, 2);
    assert_eq!(geometry.tile_columns, 3);
    assert_eq!(geometry.reduction_tiles, 2);
    assert_eq!(geometry.total_work_items, 384);
    assert_eq!(
        geometry.tile_origin(0, 0),
        Some(TileOriginV1 { row: 0, column: 0 })
    );
    assert_eq!(
        geometry.tile_origin(2, 1),
        Some(TileOriginV1 {
            row: 16,
            column: 32
        })
    );
    assert_eq!(geometry.tile_origin(3, 0), None);
    assert_eq!(geometry.tile_origin(0, 2), None);
}

#[test]
fn zero_dimension_precedence_never_constructs_a_launch() {
    for &(m, n, k) in &[(0, 1, 1), (1, 0, 1), (0, 17, 3), (31, 0, 17)] {
        let shape = ShapeV1::checked(m, n, k).unwrap();
        assert_eq!(plan_v1(shape), Ok(LaunchDecisionV1::NoDispatchEmptyOutput));
    }
}

#[test]
fn nonempty_zero_k_is_positive_zero_host_fill_only_after_mn_validation() {
    let shape = ShapeV1::checked(16, 32, 0).unwrap();
    assert_eq!(
        plan_v1(shape),
        Ok(LaunchDecisionV1::HostFillPositiveZero {
            output_elements: 512
        })
    );
    assert_eq!(
        plan_v1(ShapeV1::checked(17, 16, 0).unwrap()),
        Err(PlanErrorV1::MNotMultipleOf16(17))
    );
    assert_eq!(
        plan_v1(ShapeV1::checked(16, 17, 0).unwrap()),
        Err(PlanErrorV1::NNotMultipleOf16(17))
    );
}

#[test]
fn every_small_positive_shape_is_admitted_exactly_when_fully_tiled() {
    for m in 1..=48 {
        for n in 1..=48 {
            for k in 1..=48 {
                let shape = ShapeV1::checked(m, n, k).unwrap();
                let result = plan_v1(shape);
                let fully_tiled = m % 16 == 0 && n % 16 == 0 && k % 16 == 0;
                assert_eq!(result.is_ok(), fully_tiled, "shape=({m},{n},{k})");
                assert_eq!(
                    matches!(result, Ok(LaunchDecisionV1::Dispatch(_))),
                    fully_tiled,
                    "shape=({m},{n},{k})"
                );
            }
        }
    }
}

#[test]
fn checked_geometry_rejects_unrepresentable_grid_x() {
    let shape = ShapeV1::checked(16, 0xffff_fff0, 16).unwrap();
    assert_eq!(plan_v1(shape), Err(PlanErrorV1::GridXOverflow));
}
