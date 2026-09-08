use fe2o3_device::prelude::*;

enum Brand {}

fn require_send<T: Send>() {}

fn main() {
    require_send::<PrivateMemoryView<'static, u32, ExclusiveReadWrite, Brand>>();
}
