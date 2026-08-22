use gpu_device::kernel;

#[kernel]
pub unsafe fn unsafe_signature(value: u32) -> u32 {
    value
}

#[kernel]
pub fn unsafe_block(value: u32) -> u32 {
    unsafe { value }
}

fn main() {}
