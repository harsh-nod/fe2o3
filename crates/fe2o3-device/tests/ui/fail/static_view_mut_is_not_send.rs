use fe2o3_device::StaticViewMut;

fn require_send<T: Send>() {}

fn main() {
    require_send::<StaticViewMut<'static, u32, 4>>();
}
