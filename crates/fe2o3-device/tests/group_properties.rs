use fe2o3_device::{
    ActiveLaneGroup, Gfx942SubgroupWidth, Grid, GridSize, Group, GroupMemoryOrdering,
    GroupMemorySpace, GroupScope, Invocation3D, SubgroupTile, SynchronizationContract,
    UnsupportedSynchronization, ValidGfx942SubgroupWidth, Wave64, WaveLane, Workgroup, WorkgroupId,
    WorkgroupSize, WorkgroupSynchronization, WorkitemId,
};

fn invocation(
    local: [u32; 3],
    workgroup: [u32; 3],
    workgroup_size: [u32; 3],
    grid_size: [u32; 3],
) -> Invocation3D {
    let workgroup_size =
        WorkgroupSize::new(workgroup_size[0], workgroup_size[1], workgroup_size[2]).unwrap();
    let grid_size = GridSize::new(grid_size[0], grid_size[1], grid_size[2]).unwrap();
    // SAFETY: The test supplies the checked current-invocation coordinates to
    // exercise the public capability boundary without bypassing it.
    unsafe {
        Invocation3D::from_raw_parts(
            WorkitemId::new(local[0], local[1], local[2]),
            WorkgroupId::new(workgroup[0], workgroup[1], workgroup[2]),
            workgroup_size,
            grid_size,
        )
        .unwrap()
    }
}

fn wave64_lane(lane: u32) -> WaveLane<Wave64> {
    // SAFETY: Each test lane is the modeled current lane and the modeled target
    // is gfx942 wave64.
    unsafe { WaveLane::from_raw(lane).unwrap() }
}

fn inspect_group(group: &impl Group, expected_size: u64, expected_rank: u64) {
    assert_eq!(group.size(), expected_size);
    assert_eq!(group.thread_rank(), expected_rank);
    assert!(group.thread_rank() < group.size());
}

#[test]
fn property_workgroup_rank_is_row_major_and_bijective() {
    for size_x in 1..=8 {
        for size_y in 1..=5 {
            for size_z in 1..=3 {
                let size = u64::from(size_x) * u64::from(size_y) * u64::from(size_z);
                let mut seen = vec![false; size as usize];
                for z in 0..size_z {
                    for y in 0..size_y {
                        for x in 0..size_x {
                            let invocation = invocation(
                                [x, y, z],
                                [1, 1, 1],
                                [size_x, size_y, size_z],
                                [2, 2, 2],
                            );
                            let group = Workgroup::from_invocation(&invocation).unwrap();
                            let rank = (u64::from(z) * u64::from(size_y) + u64::from(y))
                                * u64::from(size_x)
                                + u64::from(x);
                            inspect_group(&group, size, rank);
                            assert!(!seen[rank as usize]);
                            seen[rank as usize] = true;
                        }
                    }
                }
                assert!(seen.into_iter().all(core::convert::identity));
            }
        }
    }
}

#[test]
fn property_grid_rank_is_row_major_and_bijective() {
    let workgroup_shapes = [[1, 1, 1], [8, 4, 2], [64, 1, 1], [3, 5, 2]];
    let grid_shapes = [[1, 1, 1], [2, 3, 4], [5, 2, 3]];

    for workgroup_size in workgroup_shapes {
        for grid_size in grid_shapes {
            let extent = [
                u64::from(workgroup_size[0]) * u64::from(grid_size[0]),
                u64::from(workgroup_size[1]) * u64::from(grid_size[1]),
                u64::from(workgroup_size[2]) * u64::from(grid_size[2]),
            ];
            let size = extent[0] * extent[1] * extent[2];
            let mut seen = vec![false; size as usize];
            for workgroup_z in 0..grid_size[2] {
                for workgroup_y in 0..grid_size[1] {
                    for workgroup_x in 0..grid_size[0] {
                        for local_z in 0..workgroup_size[2] {
                            for local_y in 0..workgroup_size[1] {
                                for local_x in 0..workgroup_size[0] {
                                    let invocation = invocation(
                                        [local_x, local_y, local_z],
                                        [workgroup_x, workgroup_y, workgroup_z],
                                        workgroup_size,
                                        grid_size,
                                    );
                                    let group = Grid::from_invocation(&invocation).unwrap();
                                    let global = [
                                        u64::from(workgroup_x) * u64::from(workgroup_size[0])
                                            + u64::from(local_x),
                                        u64::from(workgroup_y) * u64::from(workgroup_size[1])
                                            + u64::from(local_y),
                                        u64::from(workgroup_z) * u64::from(workgroup_size[2])
                                            + u64::from(local_z),
                                    ];
                                    let rank =
                                        (global[2] * extent[1] + global[1]) * extent[0] + global[0];
                                    inspect_group(&group, size, rank);
                                    assert!(!seen[rank as usize]);
                                    seen[rank as usize] = true;
                                }
                            }
                        }
                    }
                }
            }
            assert!(seen.into_iter().all(core::convert::identity));
        }
    }
}

fn verify_tile_width<const N: u32>()
where
    Gfx942SubgroupWidth<N>: ValidGfx942SubgroupWidth,
{
    for lane in 0..64 {
        let group: SubgroupTile<N> = wave64_lane(lane).into_subgroup_tile();
        inspect_group(&group, u64::from(N), u64::from(lane % N));
        assert_eq!(group.tile_index(), lane / N);
    }
}

#[test]
fn property_every_supported_gfx942_tile_width_partitions_wave64() {
    verify_tile_width::<1>();
    verify_tile_width::<2>();
    verify_tile_width::<4>();
    verify_tile_width::<8>();
    verify_tile_width::<16>();
    verify_tile_width::<32>();
    verify_tile_width::<64>();
}

fn active_group(lane: u32, mask: u64) -> Option<ActiveLaneGroup> {
    // SAFETY: The generated mask is the modeled EXEC mask at the modeled
    // current lane's convergent source point.
    unsafe { wave64_lane(lane).into_active_lane_group(mask) }
}

#[test]
fn property_active_lane_rank_counts_only_lower_active_lanes() {
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
            let group = active_group(lane, mask).unwrap();
            let lower_lanes = (1_u64 << lane) - 1;
            inspect_group(
                &group,
                u64::from(mask.count_ones()),
                u64::from((mask & lower_lanes).count_ones()),
            );
            assert_eq!(group.member_mask(), mask);
        }
    }
}

#[test]
fn active_lane_group_rejects_a_mask_that_excludes_the_current_lane() {
    for lane in 0..64 {
        assert!(active_group(lane, !(1_u64 << lane)).is_none());
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
        &[GroupMemorySpace::Workgroup]
    );
}

#[test]
fn oversized_groups_fail_closed_instead_of_truncating() {
    let oversized_workgroup = invocation(
        [0, 0, 0],
        [0, 0, 0],
        [u32::MAX, u32::MAX, u32::MAX],
        [1, 1, 1],
    );
    assert!(Workgroup::from_invocation(&oversized_workgroup).is_none());
    assert!(Grid::from_invocation(&oversized_workgroup).is_none());

    let invocation = invocation(
        [0, 0, 0],
        [0, 0, 0],
        [u32::MAX, u32::MAX, 1],
        [u32::MAX, u32::MAX, 1],
    );
    assert!(Workgroup::from_invocation(&invocation).is_some());
    assert!(Grid::from_invocation(&invocation).is_none());
}

#[test]
fn workgroup_synchronization_panics_closed_on_the_host() {
    let invocation = invocation([0, 0, 0], [0, 0, 0], [1, 1, 1], [1, 1, 1]);
    let group = Workgroup::from_invocation(&invocation).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // SAFETY: This one-work-item modeled workgroup reaches this exact
        // dynamic barrier once. The host barrier still must fail closed.
        unsafe { group.assume_uniform() }.synchronize();
    }));
    assert!(result.is_err());
}
