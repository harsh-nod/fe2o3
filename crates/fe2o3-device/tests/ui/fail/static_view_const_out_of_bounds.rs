use fe2o3_device::{StaticIndex, StaticView};

fn out_of_bounds(view: &StaticView<'_, u32, 4>) {
    let _ = view.at_const(StaticIndex::<4, 4>::CHECKED);
}

fn main() {}
