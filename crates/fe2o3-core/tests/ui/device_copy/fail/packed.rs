use fe2o3_core::DeviceCopy;

#[derive(Clone, Copy, DeviceCopy)]
#[repr(C, packed)]
struct Packed {
    byte: u8,
    word: u32,
}

fn main() {}
