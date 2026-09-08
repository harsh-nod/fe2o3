use fe2o3_device::capability_memory::{Global, ReadOnly};

fn main() {
    let values = [1.0_f32];
    let _forged: Global<'_, f32, ReadOnly> = Global::new(&values);
}
