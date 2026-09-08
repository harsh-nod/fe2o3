#![allow(deprecated)]

use fe2o3_device::capability_memory::{Private, PrivateMemoryView, ReadOnly};

enum BrandA {}
enum BrandB {}

fn alias_requires_brand_b(_: Private<'_, u32, ReadOnly, BrandB>) {}

fn alias_does_not_erase_brand(view: PrivateMemoryView<'_, u32, ReadOnly, BrandA>) {
    alias_requires_brand_b(view);
}

fn main() {}
