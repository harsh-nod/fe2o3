use fe2o3_device::{
    GlobalGridSize, GlobalWorkitemId, GridSize, Invocation3D, Wave32, Wave64, WaveLane, WaveWidth,
    WorkgroupId, WorkgroupSize, WorkitemId,
};

fn coordinate_data_is_copy(
    workitem: WorkitemId,
    workgroup: WorkgroupId,
    workgroup_size: WorkgroupSize,
    grid_size: GridSize,
) {
    let _ = (workitem, workitem.x(), workitem.y(), workitem.z());
    let _ = (workgroup, workgroup.x(), workgroup.y(), workgroup.z());
    let _ = (
        workgroup_size,
        workgroup_size.contains(workitem),
        workgroup_size.volume(),
    );
    let _ = (grid_size, grid_size.contains(workgroup), grid_size.volume());
}

fn inspect_invocation(invocation: &Invocation3D) {
    let global: GlobalWorkitemId = invocation.global_workitem_id();
    let extent: GlobalGridSize = invocation.global_grid_size();
    let _ = (
        invocation.workitem_id(),
        invocation.workgroup_id(),
        invocation.workgroup_size(),
        invocation.grid_size(),
        global.linear(extent),
        extent.contains(global),
        extent.volume(),
    );
}

fn inspect_lane<Width: WaveWidth>(lane: &WaveLane<Width>) {
    let _ = (lane.get(), lane.width(), lane.is_first(), lane.is_last());
}

unsafe fn compiler_boundary(
    workitem: WorkitemId,
    workgroup: WorkgroupId,
    workgroup_size: WorkgroupSize,
    grid_size: GridSize,
    lane32: u32,
    lane64: u32,
) {
    let invocation = unsafe {
        Invocation3D::from_raw_parts(workitem, workgroup, workgroup_size, grid_size)
    };
    let wave32 = unsafe { WaveLane::<Wave32>::from_raw(lane32) };
    let wave64 = unsafe { WaveLane::<Wave64>::from_raw(lane64) };
    let _ = (invocation, wave32, wave64);
}

fn main() {}
