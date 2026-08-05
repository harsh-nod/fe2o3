use fe2o3_core::DeviceCopy;

#[derive(DeviceCopy)]
#[repr(C)]
struct NotCopy {
    value: u32,
}

fn main() {}
