use fe2o3_amd_target::AmdTargetId;
use fe2o3_tiled_gemm_v1::contract::{TARGET_V1, TILE_K_V1, TILE_M_V1, TILE_N_V1, WAVE_LANES_V1};
use fe2o3_tiled_gemm_v1::{
    AdmittedTargetV1, EDGE_CASES_V1, ExpectedDecisionV1, LaunchDecisionV1, PlanErrorV1,
    ShapeErrorV1, ShapeV1, TileOriginV1, admit_target_v1, exact_target_v1, plan_v1,
};

fn admitted_target() -> AdmittedTargetV1 {
    admit_target_v1(AmdTargetId::parse(TARGET_V1).unwrap()).unwrap()
}

#[test]
fn exact_profile_is_gfx942_wave64_m16n16k16() {
    assert_eq!(TARGET_V1, "gfx942:xnack-");
    assert_eq!((TILE_M_V1, TILE_N_V1, TILE_K_V1), (16, 16, 16));
    assert_eq!(WAVE_LANES_V1, 64);
}

#[test]
fn target_admission_is_typed_exact_and_fails_closed() {
    let exact = exact_target_v1();
    assert_eq!(exact, AmdTargetId::parse(TARGET_V1).unwrap());
    assert_eq!(exact.to_string(), TARGET_V1);

    let admitted = admit_target_v1(exact).unwrap();
    assert_eq!(admitted.target_id(), exact);

    for different in [
        "gfx942",
        "gfx942:xnack+",
        "gfx942:sramecc-:xnack-",
        "gfx942:sramecc+:xnack-",
        "gfx950:xnack-",
    ] {
        let candidate = AmdTargetId::parse(different).unwrap();
        let error = admit_target_v1(candidate).unwrap_err();
        assert_eq!(error.candidate(), candidate);
        assert_eq!(error.required(), exact);
        assert!(error.to_string().contains(TARGET_V1));
        assert!(error.to_string().contains(different));
    }
}

#[test]
fn checked_shape_binds_row_major_extents_and_indices() {
    let shape = ShapeV1::checked(32, 48, 16).unwrap();
    assert_eq!(shape.dimensions(), [32, 48, 16]);
    assert_eq!((shape.m(), shape.n(), shape.k()), (32, 48, 16));
    assert!(!shape.is_empty_output());
    assert_eq!(shape.a_elements(), 512);
    assert_eq!(shape.b_elements(), 768);
    assert_eq!(shape.c_elements(), 1_536);
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
fn extreme_empty_outputs_precede_unused_extent_arithmetic() {
    for &(m, n, k) in &[
        (0, u32::MAX, u32::MAX),
        (u32::MAX, 0, u32::MAX),
        (0, 0, u32::MAX),
    ] {
        let shape = ShapeV1::checked(m, n, k).unwrap();
        assert_eq!(shape.dimensions(), [m, n, k]);
        assert!(shape.is_empty_output());
        assert_eq!(shape.a_elements(), 0);
        assert_eq!(shape.b_elements(), 0);
        assert_eq!(shape.c_elements(), 0);
        assert_eq!(shape.a_index(0, 0), None);
        assert_eq!(shape.b_index(0, 0), None);
        assert_eq!(shape.c_index(0, 0), None);
        assert_eq!(
            plan_v1(admitted_target(), shape),
            Ok(LaunchDecisionV1::NoDispatchEmptyOutput)
        );
    }
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
        let actual = plan_v1(admitted_target(), shape);
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
    let LaunchDecisionV1::Dispatch(geometry) = plan_v1(admitted_target(), shape).unwrap() else {
        panic!("exact tiled shape did not dispatch");
    };
    assert_eq!(geometry.target(), admitted_target());
    assert_eq!(geometry.block_counts(), [3, 2, 1]);
    assert_eq!(geometry.workgroup_dimensions(), [64, 1, 1]);
    assert_eq!(geometry.aql_grid_work_items(), [192, 2, 1]);
    assert_eq!(geometry.tile_rows(), 2);
    assert_eq!(geometry.tile_columns(), 3);
    assert_eq!(geometry.reduction_tiles(), 2);
    assert_eq!(geometry.total_work_items(), 384);
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
fn representative_shapes_pin_blocks_workgroups_and_aql_work_items() {
    let cases = [
        ((16, 16, 16), [1, 1, 1], [64, 1, 1], 1),
        ((16, 48, 16), [3, 1, 1], [192, 1, 1], 1),
        ((32, 48, 32), [3, 2, 1], [192, 2, 1], 2),
    ];

    for ((m, n, k), expected_blocks, expected_aql, expected_reduction_tiles) in cases {
        let shape = ShapeV1::checked(m, n, k).unwrap();
        let LaunchDecisionV1::Dispatch(geometry) = plan_v1(admitted_target(), shape).unwrap()
        else {
            panic!("exact tiled shape ({m},{n},{k}) did not dispatch");
        };

        assert_eq!(geometry.block_counts(), expected_blocks);
        assert_eq!(geometry.workgroup_dimensions(), [64, 1, 1]);
        assert_eq!(geometry.aql_grid_work_items(), expected_aql);
        assert_eq!(geometry.reduction_tiles(), expected_reduction_tiles);
        assert_eq!(
            geometry.total_work_items(),
            expected_aql.into_iter().map(u64::from).product::<u64>()
        );
    }
}

#[test]
fn geometry_derivation_is_exhaustive_over_small_exact_tiles() {
    for tile_rows in 1..=8 {
        for tile_columns in 1..=8 {
            for reduction_tiles in 1..=4 {
                let m = tile_rows * TILE_M_V1;
                let n = tile_columns * TILE_N_V1;
                let k = reduction_tiles * TILE_K_V1;
                let shape = ShapeV1::checked(m, n, k).unwrap();
                let LaunchDecisionV1::Dispatch(geometry) =
                    plan_v1(admitted_target(), shape).unwrap()
                else {
                    panic!("exact tiled shape ({m},{n},{k}) did not dispatch");
                };

                let blocks = geometry.block_counts();
                let workgroup = geometry.workgroup_dimensions();
                let aql = geometry.aql_grid_work_items();
                assert_eq!(blocks, [tile_columns, tile_rows, 1]);
                assert_eq!(workgroup, [WAVE_LANES_V1, 1, 1]);
                assert_eq!(
                    aql,
                    [
                        blocks[0] * workgroup[0],
                        blocks[1] * workgroup[1],
                        blocks[2] * workgroup[2],
                    ]
                );
                assert_eq!(geometry.tile_columns(), blocks[0]);
                assert_eq!(geometry.tile_rows(), blocks[1]);
                assert_eq!(geometry.reduction_tiles(), reduction_tiles);
            }
        }
    }
}

#[test]
fn representative_shapes_reject_common_geometry_mutations() {
    let cases = [(16, 16, 16), (16, 48, 16), (32, 48, 32)];
    let mut caught_blocks_as_aql = false;
    let mut caught_single_block_x = false;
    let mut caught_wave_factor_on_y = false;
    let mut caught_swapped_block_axes = false;

    for (m, n, k) in cases {
        let shape = ShapeV1::checked(m, n, k).unwrap();
        let LaunchDecisionV1::Dispatch(geometry) = plan_v1(admitted_target(), shape).unwrap()
        else {
            panic!("exact tiled shape ({m},{n},{k}) did not dispatch");
        };
        let blocks = geometry.block_counts();
        let actual = geometry.aql_grid_work_items();

        caught_blocks_as_aql |= actual != blocks;
        caught_single_block_x |= actual != [WAVE_LANES_V1, blocks[1], blocks[2]];
        caught_wave_factor_on_y |= actual != [blocks[0], blocks[1] * WAVE_LANES_V1, blocks[2]];
        caught_swapped_block_axes |= actual != [blocks[1] * WAVE_LANES_V1, blocks[0], blocks[2]];
    }

    assert!(caught_blocks_as_aql);
    assert!(caught_single_block_x);
    assert!(caught_wave_factor_on_y);
    assert!(caught_swapped_block_axes);
}

#[test]
fn zero_dimension_precedence_never_constructs_a_launch() {
    for &(m, n, k) in &[(0, 1, 1), (1, 0, 1), (0, 17, 3), (31, 0, 17)] {
        let shape = ShapeV1::checked(m, n, k).unwrap();
        assert_eq!(
            plan_v1(admitted_target(), shape),
            Ok(LaunchDecisionV1::NoDispatchEmptyOutput)
        );
    }
}

#[test]
fn nonempty_zero_k_is_positive_zero_host_fill_only_after_mn_validation() {
    let shape = ShapeV1::checked(16, 32, 0).unwrap();
    assert_eq!(
        plan_v1(admitted_target(), shape),
        Ok(LaunchDecisionV1::HostFillPositiveZero {
            output_elements: 512
        })
    );
    assert_eq!(
        plan_v1(admitted_target(), ShapeV1::checked(17, 16, 0).unwrap()),
        Err(PlanErrorV1::MNotMultipleOf16(17))
    );
    assert_eq!(
        plan_v1(admitted_target(), ShapeV1::checked(16, 17, 0).unwrap()),
        Err(PlanErrorV1::NNotMultipleOf16(17))
    );
}

#[test]
fn every_small_positive_shape_is_admitted_exactly_when_fully_tiled() {
    for m in 1..=48 {
        for n in 1..=48 {
            for k in 1..=48 {
                let shape = ShapeV1::checked(m, n, k).unwrap();
                let result = plan_v1(admitted_target(), shape);
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
fn checked_geometry_rejects_unrepresentable_aql_grid_x() {
    let shape = ShapeV1::checked(16, 0xffff_fff0, 16).unwrap();
    assert_eq!(
        plan_v1(admitted_target(), shape),
        Err(PlanErrorV1::AqlGridXOverflow)
    );
}

#[test]
fn checked_geometry_covers_the_exact_aql_grid_x_boundary() {
    let largest_n = 0x3fff_fff0;
    let accepted = ShapeV1::checked(16, largest_n, 16).unwrap();
    let LaunchDecisionV1::Dispatch(geometry) = plan_v1(admitted_target(), accepted).unwrap() else {
        panic!("largest representable x-grid did not dispatch");
    };
    assert_eq!(geometry.block_counts(), [0x03ff_ffff, 1, 1]);
    assert_eq!(geometry.workgroup_dimensions(), [64, 1, 1]);
    assert_eq!(geometry.aql_grid_work_items(), [0xffff_ffc0, 1, 1]);
    assert_eq!(geometry.tile_columns(), 0x03ff_ffff);
    assert_eq!(geometry.total_work_items(), u64::from(0xffff_ffc0_u32));
    assert_eq!(
        geometry.tile_origin(0x03ff_fffe, 0),
        Some(TileOriginV1 {
            row: 0,
            column: 0x3fff_ffe0,
        })
    );

    let first_rejected = ShapeV1::checked(16, 0x4000_0000, 16).unwrap();
    assert_eq!(
        plan_v1(admitted_target(), first_rejected),
        Err(PlanErrorV1::AqlGridXOverflow)
    );
}
