extern crate self as core;

pub mod marker {
    pub use std::marker::{Copy, Send, Sync};
}

pub mod mem {
    pub const fn size_of<T>() -> usize {
        0
    }
}

pub mod option {
    pub use std::option::Option;
}

#[macro_export]
macro_rules! assert {
    ($($tokens:tt)*) => {};
}

#[macro_export]
macro_rules! panic {
    ($($tokens:tt)*) => {
        loop {}
    };
}

use fe2o3_core::DeviceCopy;

#[derive(Clone, Copy, DeviceCopy)]
#[repr(C)]
struct Padded {
    byte: u8,
    word: u32,
}

fn main() {}
