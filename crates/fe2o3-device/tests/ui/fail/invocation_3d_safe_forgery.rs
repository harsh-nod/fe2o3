use fe2o3_device::{GridSize, Invocation3D, WorkgroupId, WorkgroupSize, WorkitemId};

fn main() {
    let workgroup_size = WorkgroupSize::new(64, 1, 1).unwrap();
    let grid_size = GridSize::new(1, 1, 1).unwrap();
    let _ = Invocation3D::from_raw_parts(
        WorkitemId::new(0, 0, 0),
        WorkgroupId::new(0, 0, 0),
        workgroup_size,
        grid_size,
    );
}
