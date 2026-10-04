//! CPU arithmetic model tests only.
//!
//! These tests provide no compiler lowering, codegen, Verus, or hardware
//! evidence for typed groups or synchronization.

use super::{
    ActiveLaneGroup, Grid, Group, GroupMemoryOrdering, GroupMemorySpace, GroupScope, SubgroupTile,
    SynchronizationContract, TYPED_GROUP_CONTRACT_VERSION_V1, UnsupportedSynchronization,
    ValidWave64TileWidth, Wave64TileWidth, Workgroup, WorkgroupSynchronization,
};
use crate::thread::{GridSize, Invocation3D, WorkgroupId, WorkgroupSize, WorkitemId};
use crate::wave::{Wave64, WaveLane};
use std::vec;

fn invocation(
    local: [u32; 3],
    workgroup: [u32; 3],
    workgroup_size: [u32; 3],
    grid_size: [u32; 3],
) -> Invocation3D {
    let workgroup_size =
        WorkgroupSize::new(workgroup_size[0], workgroup_size[1], workgroup_size[2]).unwrap();
    let grid_size = GridSize::new(grid_size[0], grid_size[1], grid_size[2]).unwrap();
    Invocation3D::from_model_snapshot(
        WorkitemId::new(local[0], local[1], local[2]),
        WorkgroupId::new(workgroup[0], workgroup[1], workgroup[2]),
        workgroup_size,
        grid_size,
    )
    .unwrap()
}

fn wave64_lane(lane: u32) -> WaveLane<Wave64> {
    WaveLane::from_model_snapshot(lane).unwrap()
}

fn inspect_group(group: &impl Group, expected_size: u64, expected_rank: u64) {
    assert_eq!(group.size(), expected_size);
    assert_eq!(group.thread_rank(), expected_rank);
    assert!(group.thread_rank() < group.size());
    assert!(group.has_valid_rank());
}

#[test]
fn universal_group_contract_is_versioned_and_scoped() {
    assert_eq!(TYPED_GROUP_CONTRACT_VERSION_V1, 1);
    assert_eq!(<Grid<'_> as Group>::SCOPE, GroupScope::Grid);
    assert_eq!(<Workgroup<'_> as Group>::SCOPE, GroupScope::Workgroup);
    assert_eq!(<SubgroupTile<'_, 64> as Group>::SCOPE, GroupScope::Subgroup);
    assert_eq!(<ActiveLaneGroup<'_> as Group>::SCOPE, GroupScope::Subgroup);
}

fn enumerated_volume(shape: [u32; 3]) -> u64 {
    let mut count = 0;
    for _z in 0..shape[2] {
        for _y in 0..shape[1] {
            for _x in 0..shape[0] {
                count += 1;
            }
        }
    }
    count
}

#[test]
fn property_workgroup_rank_matches_an_enumeration_oracle() {
    for size_x in 1..=8 {
        for size_y in 1..=5 {
            for size_z in 1..=3 {
                let shape = [size_x, size_y, size_z];
                let size = enumerated_volume(shape);
                let mut expected_rank = 0;
                let mut seen = vec![false; size as usize];
                for z in 0..size_z {
                    for y in 0..size_y {
                        for x in 0..size_x {
                            let invocation = invocation([x, y, z], [1, 1, 1], shape, [2, 2, 2]);
                            let group = Workgroup::from_invocation_snapshot(&invocation).unwrap();
                            inspect_group(&group, size, expected_rank);
                            assert!(!seen[expected_rank as usize]);
                            seen[expected_rank as usize] = true;
                            expected_rank += 1;
                        }
                    }
                }
                assert_eq!(expected_rank, size);
                assert!(seen.into_iter().all(core::convert::identity));
            }
        }
    }
}

#[test]
fn property_grid_rank_matches_a_global_coordinate_enumeration_oracle() {
    let workgroup_shapes = [[1, 1, 1], [8, 4, 2], [64, 1, 1], [3, 5, 2]];
    let grid_shapes = [[1, 1, 1], [2, 3, 4], [5, 2, 3]];

    for workgroup_size in workgroup_shapes {
        for grid_size in grid_shapes {
            let extent = [
                workgroup_size[0] * grid_size[0],
                workgroup_size[1] * grid_size[1],
                workgroup_size[2] * grid_size[2],
            ];
            let size = enumerated_volume(extent);
            let mut expected_rank = 0;
            let mut seen = vec![false; size as usize];
            for global_z in 0..extent[2] {
                for global_y in 0..extent[1] {
                    for global_x in 0..extent[0] {
                        let local = [
                            global_x % workgroup_size[0],
                            global_y % workgroup_size[1],
                            global_z % workgroup_size[2],
                        ];
                        let workgroup = [
                            global_x / workgroup_size[0],
                            global_y / workgroup_size[1],
                            global_z / workgroup_size[2],
                        ];
                        let invocation = invocation(local, workgroup, workgroup_size, grid_size);
                        let group = Grid::from_invocation_snapshot(&invocation).unwrap();
                        inspect_group(&group, size, expected_rank);
                        assert!(!seen[expected_rank as usize]);
                        seen[expected_rank as usize] = true;
                        expected_rank += 1;
                    }
                }
            }
            assert_eq!(expected_rank, size);
            assert!(seen.into_iter().all(core::convert::identity));
        }
    }
}

#[test]
fn fixed_rank_table_is_independent_of_the_implementation_formulas() {
    let workgroup_cases = [
        ([0, 0, 0], [4, 3, 2], 24, 0),
        ([2, 1, 1], [4, 3, 2], 24, 18),
        ([3, 2, 1], [4, 3, 2], 24, 23),
    ];
    for (local, shape, expected_size, expected_rank) in workgroup_cases {
        let invocation = invocation(local, [0, 0, 0], shape, [1, 1, 1]);
        let group = Workgroup::from_invocation_snapshot(&invocation).unwrap();
        inspect_group(&group, expected_size, expected_rank);
    }

    let grid_cases = [
        ([0, 0, 0], [0, 0, 0], 0),
        ([2, 1, 0], [1, 0, 1], 110),
        ([3, 2, 1], [1, 1, 1], 191),
    ];
    for (local, workgroup, expected_rank) in grid_cases {
        let invocation = invocation(local, workgroup, [4, 3, 2], [2, 2, 2]);
        let group = Grid::from_invocation_snapshot(&invocation).unwrap();
        inspect_group(&group, 192, expected_rank);
    }

    let lane = wave64_lane(37);
    let tile: SubgroupTile<'_, 8> = SubgroupTile::from_wave64_snapshot(&lane);
    inspect_group(&tile, 8, 5);
    assert_eq!(tile.tile_index(), 4);

    let lane = wave64_lane(63);
    let active =
        ActiveLaneGroup::from_model_snapshot(&lane, (1_u64 << 63) | (1 << 17) | 1).unwrap();
    inspect_group(&active, 3, 2);
}

fn enumerated_tile_position(lane: u32, width: u32) -> (u32, u32) {
    let mut tile_index = 0;
    let mut tile_start = 0;
    while lane >= tile_start + width {
        tile_start += width;
        tile_index += 1;
    }
    (tile_index, lane - tile_start)
}

fn verify_tile_width<const N: u32>()
where
    Wave64TileWidth<N>: ValidWave64TileWidth,
{
    for lane in 0..64 {
        let lane_snapshot = wave64_lane(lane);
        let group: SubgroupTile<'_, N> = SubgroupTile::from_wave64_snapshot(&lane_snapshot);
        let (expected_tile, expected_rank) = enumerated_tile_position(lane, N);
        inspect_group(&group, u64::from(N), u64::from(expected_rank));
        assert_eq!(group.tile_index(), expected_tile);
    }
}

#[test]
fn property_every_supported_wave64_tile_width_matches_enumeration() {
    verify_tile_width::<1>();
    verify_tile_width::<2>();
    verify_tile_width::<4>();
    verify_tile_width::<8>();
    verify_tile_width::<16>();
    verify_tile_width::<32>();
    verify_tile_width::<64>();
}

fn enumerated_active_size_and_rank(mask: u64, lane: u32) -> (u64, u64) {
    let mut size = 0;
    let mut rank = 0;
    for candidate in 0..64 {
        if mask & (1_u64 << candidate) != 0 {
            size += 1;
            if candidate < lane {
                rank += 1;
            }
        }
    }
    (size, rank)
}

#[test]
fn property_active_lane_rank_matches_bit_enumeration() {
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    for lane in 0..64 {
        for sample in 0..256 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let mask = match sample {
                0 => u64::MAX,
                1 => 1_u64 << lane,
                2 => 0xaaaa_aaaa_aaaa_aaaa | (1_u64 << lane),
                3 => 0x5555_5555_5555_5555 | (1_u64 << lane),
                _ => state | (1_u64 << lane),
            };
            let lane_snapshot = wave64_lane(lane);
            let group = ActiveLaneGroup::from_model_snapshot(&lane_snapshot, mask).unwrap();
            let (expected_size, expected_rank) = enumerated_active_size_and_rank(mask, lane);
            inspect_group(&group, expected_size, expected_rank);
            assert_eq!(group.caller_asserted_mask(), mask);
        }
    }
}

#[test]
fn active_lane_group_rejects_a_mask_that_excludes_the_lane_snapshot() {
    for lane in 0..64 {
        let lane_snapshot = wave64_lane(lane);
        let group = ActiveLaneGroup::from_model_snapshot(&lane_snapshot, !(1_u64 << lane));
        assert!(group.is_none());
    }
}

#[test]
fn group_synchronization_contracts_are_exact_and_fail_closed() {
    const {
        assert!(!UnsupportedSynchronization::SUPPORTED);
        assert!(!UnsupportedSynchronization::REQUIRES_UNIFORM_CONVERGENCE);
        assert!(WorkgroupSynchronization::SUPPORTED);
        assert!(WorkgroupSynchronization::REQUIRES_UNIFORM_CONVERGENCE);
    }
    assert_eq!(UnsupportedSynchronization::EXECUTION_SCOPE, None);
    assert_eq!(UnsupportedSynchronization::MEMORY_SCOPE, None);
    assert_eq!(UnsupportedSynchronization::ORDERING, None);
    assert_eq!(UnsupportedSynchronization::ADDRESS_SPACES, &[]);

    assert_eq!(
        WorkgroupSynchronization::EXECUTION_SCOPE,
        Some(GroupScope::Workgroup)
    );
    assert_eq!(
        WorkgroupSynchronization::MEMORY_SCOPE,
        Some(GroupScope::Workgroup)
    );
    assert_eq!(
        WorkgroupSynchronization::ORDERING,
        Some(GroupMemoryOrdering::AcquireRelease)
    );
    assert_eq!(
        WorkgroupSynchronization::ADDRESS_SPACES,
        &[GroupMemorySpace::Global, GroupMemorySpace::Workgroup]
    );
}

#[test]
fn oversized_groups_fail_closed_instead_of_truncating() {
    assert!(WorkgroupSize::new(u32::MAX, u32::MAX, u32::MAX).is_none());

    let invocation = invocation(
        [0, 0, 0],
        [0, 0, 0],
        [u32::MAX, u32::MAX, 1],
        [u32::MAX, u32::MAX, 1],
    );
    assert!(Workgroup::from_invocation_snapshot(&invocation).is_some());
    assert!(Grid::from_invocation_snapshot(&invocation).is_none());
}

#[test]
fn workgroup_synchronization_panics_closed_on_the_host() {
    let invocation = invocation([0, 0, 0], [0, 0, 0], [1, 1, 1], [1, 1, 1]);
    let group = Workgroup::from_invocation_snapshot(&invocation).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        group.synchronize();
    }));
    assert!(result.is_err());
}
