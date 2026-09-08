use fe2o3_device::prelude::*;

enum Brand {}

fn require_clone<T: Clone>() {}

fn main() {
    require_clone::<PrivateMemoryView<'static, u32, ExclusiveReadWrite, Brand>>();
}
