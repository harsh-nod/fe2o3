use fe2o3_device::KernelContext;

fn require_sync<T: Sync>() {}

fn main() {
    require_sync::<KernelContext<'static>>();
}
