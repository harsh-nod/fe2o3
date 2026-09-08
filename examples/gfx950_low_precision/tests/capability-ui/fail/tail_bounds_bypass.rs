#![forbid(unsafe_code)]

use fe2o3_device::{Global, ReadOnly};

fn unchecked_tail<Brand>(input: &Global<'_, u8, ReadOnly, Brand>, index: usize) -> u8 {
    input.load_unchecked(index)
}
