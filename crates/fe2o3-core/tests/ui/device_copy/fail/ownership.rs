use fe2o3_core::DeviceCopy;

#[derive(Clone, DeviceCopy)]
#[repr(C)]
struct OwnsMemory {
    value: String,
}

fn main() {}
