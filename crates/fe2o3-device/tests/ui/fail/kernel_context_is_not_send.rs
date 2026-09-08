use fe2o3_device::KernelContext;

fn require_send<T: Send>() {}

fn main() {
    require_send::<KernelContext<'static>>();
}
