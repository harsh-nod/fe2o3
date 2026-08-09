use fe2o3_device::{
    Gfx942SubgroupWidth, Grid, GridSize, Group, Invocation3D, SynchronizationContract,
    ValidGfx942SubgroupWidth, Wave64, WaveLane, Workgroup, WorkgroupId, WorkgroupSize,
    WorkgroupSynchronization, WorkitemId,
};

fn inspect(group: &impl Group) -> (u64, u64) {
    (group.size(), group.thread_rank())
}

fn inspect_supported_width<const N: u32>(lane: WaveLane<Wave64>)
where
    Gfx942SubgroupWidth<N>: ValidGfx942SubgroupWidth,
{
    let tile = lane.into_subgroup_tile::<N>();
    let _ = (inspect(&tile), tile.tile_index());
}

fn workgroup_policy_is_typed(group: &Workgroup<'_>)
where
    WorkgroupSynchronization: SynchronizationContract,
{
    fn require_contract<Contract: SynchronizationContract>() {}
    require_contract::<<Workgroup<'_> as Group>::Synchronization>();
    let _ = group;
}

unsafe fn compiler_capability_boundary(
    workitem: WorkitemId,
    workgroup_id: WorkgroupId,
    workgroup_size: WorkgroupSize,
    grid_size: GridSize,
    lane: u32,
    active_mask: u64,
) {
    let invocation = unsafe {
        Invocation3D::from_raw_parts(workitem, workgroup_id, workgroup_size, grid_size).unwrap()
    };
    let grid = Grid::from_invocation(&invocation).unwrap();
    let workgroup = Workgroup::from_invocation(&invocation).unwrap();
    let _ = (inspect(&grid), inspect(&workgroup));

    let convergence = unsafe { workgroup.assume_uniform() };
    drop(convergence);

    inspect_supported_width::<1>(unsafe { WaveLane::from_raw(lane).unwrap() });
    inspect_supported_width::<2>(unsafe { WaveLane::from_raw(lane).unwrap() });
    inspect_supported_width::<4>(unsafe { WaveLane::from_raw(lane).unwrap() });
    inspect_supported_width::<8>(unsafe { WaveLane::from_raw(lane).unwrap() });
    inspect_supported_width::<16>(unsafe { WaveLane::from_raw(lane).unwrap() });
    inspect_supported_width::<32>(unsafe { WaveLane::from_raw(lane).unwrap() });
    inspect_supported_width::<64>(unsafe { WaveLane::from_raw(lane).unwrap() });

    let active = unsafe {
        WaveLane::<Wave64>::from_raw(lane)
            .unwrap()
            .into_active_lane_group(active_mask)
    };
    let _ = active.map(|group| (inspect(&group), group.member_mask()));
}

fn main() {
    let _ = workgroup_policy_is_typed;
}
