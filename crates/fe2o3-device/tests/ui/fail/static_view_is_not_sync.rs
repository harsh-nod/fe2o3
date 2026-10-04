use fe2o3_device::StaticView;

fn require_sync<T: Sync>() {}

fn main() {
    require_sync::<StaticView<'static, u32, 4>>();
}
