use fe2o3_device::StaticViewMut;

fn require_sync<T: Sync>() {}

fn main() {
    require_sync::<StaticViewMut<'static, u32, 4>>();
}
