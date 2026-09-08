#![forbid(unsafe_code)]

use fe2o3_device::{Global, PrivateMemoryView, ReadOnly};

fn require_global<Brand>(_: &Global<'_, u8, ReadOnly, Brand>) {}

fn substitute_private<Brand>(input: &PrivateMemoryView<'_, u8, ReadOnly, Brand>) {
    require_global(input);
}
