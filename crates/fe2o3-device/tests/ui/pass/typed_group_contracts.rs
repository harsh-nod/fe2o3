use fe2o3_device::{
    ActiveLaneGroup, Grid, GridSize, Group, Invocation3D, SubgroupTile,
    SynchronizationContract, ValidWave64TileWidth, Wave64, Wave64TileWidth, WaveLane, Workgroup,
    WorkgroupId, WorkgroupSize, WorkgroupSynchronization, WorkitemId,
};

fn inspect(group: &impl Group) -> (u64, u64) {
    (group.size(), group.thread_rank())
}

fn inspect_supported_width<const N: u32>(lane: &WaveLane<Wave64>)
where
    Wave64TileWidth<N>: ValidWave64TileWidth,
{
    let tile: SubgroupTile<'_, N> = SubgroupTile::from_wave64_snapshot(lane);
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

unsafe fn caller_asserted_snapshot_boundary(
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
    let grid = Grid::from_invocation_snapshot(&invocation).unwrap();
    let workgroup = Workgroup::from_invocation_snapshot(&invocation).unwrap();
    let _ = (inspect(&grid), inspect(&workgroup));

    let lane_snapshot = unsafe { WaveLane::<Wave64>::from_raw(lane).unwrap() };
    inspect_supported_width::<1>(&lane_snapshot);
    inspect_supported_width::<2>(&lane_snapshot);
    inspect_supported_width::<4>(&lane_snapshot);
    inspect_supported_width::<8>(&lane_snapshot);
    inspect_supported_width::<16>(&lane_snapshot);
    inspect_supported_width::<32>(&lane_snapshot);
    inspect_supported_width::<64>(&lane_snapshot);

    let active = unsafe {
        ActiveLaneGroup::from_caller_asserted_snapshot(&lane_snapshot, active_mask)
    };
    let _ = active.map(|group| (inspect(&group), group.caller_asserted_mask()));
}

unsafe fn caller_proven_uniform_synchronization(group: &Workgroup<'_>) {
    unsafe { group.synchronize() };
}

fn main() {
    let _ = workgroup_policy_is_typed;
    let _ = caller_asserted_snapshot_boundary;
    let _ = caller_proven_uniform_synchronization;
}
