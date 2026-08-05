use fe2o3_core::DeviceCopy;

#[derive(Clone, Copy, DeviceCopy)]
#[repr(C)]
struct TrailingPadding {
    word: u32,
    byte: u8,
}

fn main() {}
