use fe2o3_core::{DeviceBuffer, DeviceBufferViewMut};

fn rejected(buffer: &mut DeviceBuffer<u32>) -> DeviceBufferViewMut<'static, u32> {
    let (left, _right) = buffer.split_at_mut(1).unwrap();
    left
}

fn main() {}
