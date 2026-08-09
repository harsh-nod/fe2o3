use fe2o3_device::StaticView;

fn escape(parent: &[u32]) -> StaticView<'static, u32, 4> {
    StaticView::from_shared_slice(parent, 0).unwrap()
}

fn main() {}
