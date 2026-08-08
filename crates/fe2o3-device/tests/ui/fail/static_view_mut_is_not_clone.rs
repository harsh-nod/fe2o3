use fe2o3_device::StaticViewMut;

fn duplicate(view: StaticViewMut<'_, u32, 4>) {
    let _ = view.clone();
}

fn main() {}
