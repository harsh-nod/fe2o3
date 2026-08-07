use fe2o3_core::{DeviceBufferView, GpuContext};
use std::sync::Arc;

fn forge(context: &Arc<GpuContext>) -> DeviceBufferView<'_, u32> {
    DeviceBufferView::from_raw(context, core::ptr::null_mut(), 0)
}

fn main() {}
