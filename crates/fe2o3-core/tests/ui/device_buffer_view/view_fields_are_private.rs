use fe2o3_core::{DeviceBuffer, DeviceBufferView};

fn forge(buffer: &DeviceBuffer<u32>) -> DeviceBufferView<'_, u32> {
    DeviceBufferView {
        buffer,
        ptr: core::ptr::null_mut(),
        len: 0,
    }
}

fn main() {}
