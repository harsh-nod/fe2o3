use fe2o3_core::DeviceCopy;

#[derive(Clone, Copy, DeviceCopy)]
#[repr(C)]
enum NotAStruct {
    Zero,
    One,
}

fn main() {}
