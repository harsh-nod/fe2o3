use fe2o3_device::{StaticIndex, StaticView};

fn mutate(view: &mut StaticView<'_, u32, 4>) {
    let _ = view.at_const_mut(StaticIndex::<4, 0>::CHECKED);
}

fn main() {}
