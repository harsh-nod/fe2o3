use fe2o3_core::DeviceBufferViewMut;

fn rejected(view: DeviceBufferViewMut<'_, u32>) {
    let _duplicate = view.clone();
}

fn main() {}
