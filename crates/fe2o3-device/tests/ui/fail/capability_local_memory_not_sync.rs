use fe2o3_device::prelude::*;

enum Brand {}

fn require_sync<T: Sync>() {}

fn main() {
    require_sync::<PrivateMemoryView<'static, u32, ExclusiveReadWrite, Brand>>();
}
