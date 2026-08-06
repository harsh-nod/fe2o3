#![no_std]

#[inline(never)]
pub fn same_name<T: Copy>(value: T) -> T {
    value
}
