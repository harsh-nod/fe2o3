use fe2o3_device::StaticView;

fn require_send<T: Send>() {}

fn main() {
    require_send::<StaticView<'static, u32, 4>>();
}
