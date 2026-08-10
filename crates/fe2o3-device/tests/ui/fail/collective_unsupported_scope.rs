use fe2o3_device::{ActiveLaneGroup, Gfx942Collectives, Grid};

fn reject_grid(context: &Gfx942Collectives, grid: &Grid<'_>) {
    let _ = unsafe { grid.reduce_sum(context, 1_u32) };
}

fn reject_active(context: &Gfx942Collectives, active: &ActiveLaneGroup<'_>) {
    let _ = unsafe { active.reduce_sum(context, 1_u32) };
}

fn main() {}
