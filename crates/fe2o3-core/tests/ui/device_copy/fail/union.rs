use fe2o3_core::DeviceCopy;

#[derive(Clone, Copy, DeviceCopy)]
#[repr(C)]
union NotAStruct {
    integer: u32,
    float: f32,
}

fn main() {}
