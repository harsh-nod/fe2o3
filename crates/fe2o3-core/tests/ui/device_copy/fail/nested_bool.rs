use fe2o3_core::DeviceCopy;

#[derive(Clone, Copy, DeviceCopy)]
#[repr(C)]
struct NestedInvalid {
    values: [u32; 2],
    invalid: bool,
}

fn assert_device_copy<T: DeviceCopy>() {}

fn main() {
    assert_device_copy::<NestedInvalid>();
}
