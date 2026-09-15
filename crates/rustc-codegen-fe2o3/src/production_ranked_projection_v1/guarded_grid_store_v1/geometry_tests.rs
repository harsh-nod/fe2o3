use super::*;
use fe2o3_artifacts::{Dimensions, ValidationError};

fn exact(rank: u8, block: [u32; 3], grid: [u32; 3]) -> LaunchContract {
    LaunchContract::new(
        rank,
        BlockSize::Exact(Dimensions::new(block[0], block[1], block[2]).unwrap()),
        Dimensions::new(grid[0], grid[1], grid[2]).unwrap(),
        0,
        0,
    )
    .unwrap()
}

pub(super) fn rejected_launches() -> [LaunchContract; 6] {
    [
        exact(2, [32, 2, 1], [2, 3, 1]),
        exact(3, [16, 2, 2], [2, 3, 4]),
        // Rank is required, not inferred from numerically singleton Y/Z.
        exact(2, [64, 1, 1], [2, 1, 1]),
        exact(1, [64, 1, 1], [u32::MAX, 1, 1]),
        LaunchContract::new(1, BlockSize::Any, Dimensions::new(2, 1, 1).unwrap(), 0, 0).unwrap(),
        LaunchContract::new(
            1,
            BlockSize::AtMost(Dimensions::new(64, 1, 1).unwrap()),
            Dimensions::new(2, 1, 1).unwrap(),
            0,
            0,
        )
        .unwrap(),
    ]
}

#[test]
fn guarded_grid_geometry_rejects_rank_two_three_dynamic_and_inexact() {
    for launch in rejected_launches() {
        assert_eq!(bounded_linear_launch_extent_v1(&launch), None, "{launch:?}");
    }
}

#[test]
fn guarded_grid_geometry_rank_one_cannot_hide_live_y_or_z() {
    for axes in [[2, 2, 1], [2, 1, 2]] {
        let dimensions = Dimensions::new(axes[0], axes[1], axes[2]).unwrap();
        assert_eq!(
            LaunchContract::new(
                1,
                BlockSize::Exact(dimensions),
                Dimensions::new(2, 1, 1).unwrap(),
                0,
                0
            ),
            Err(ValidationError::InvalidDimension { field: "block" })
        );
        assert_eq!(
            LaunchContract::new(
                1,
                BlockSize::Exact(Dimensions::new(64, 1, 1).unwrap()),
                dimensions,
                0,
                0
            ),
            Err(ValidationError::InvalidDimension { field: "grid" })
        );
    }
}

#[test]
fn guarded_grid_geometry_exact_rank_one_uses_complete_global_x_extent() {
    for block in [1, 2, 64, 1024] {
        for grid in [1, 2, 17, u32::MAX - 1] {
            let launch = exact(1, [block, 1, 1], [grid, 1, 1]);
            assert_eq!(
                bounded_linear_launch_extent_v1(&launch),
                Some(u64::from(block) * u64::from(grid))
            );
            assert_eq!((launch.max_grid().y(), launch.max_grid().z()), (1, 1));
        }
    }
}

#[test]
fn guarded_grid_geometry_x_zero_is_not_a_multidimensional_singleton() {
    // Counterexample motivating the rejection, not an alternative proof engine.
    // Original GlobalWorkitemId::linear computes ((z * Y) + y) * X + x.
    let launch = exact(2, [2, 2, 1], [1, 1, 1]);
    let coordinate = [0_u64, 1, 0];
    let linear = ((coordinate[2] * 2) + coordinate[1]) * 2 + coordinate[0];
    assert_eq!(coordinate[0], 0);
    assert_eq!(linear, 2);
    assert_eq!(bounded_linear_launch_extent_v1(&launch), None);
}
