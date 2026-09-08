use fe2o3_device::prelude::*;

enum BrandA {}
enum BrandB {}

fn brand_b_only(_: &PrivateMemoryView<'_, u32, ReadOnly, BrandB>) {}

fn wrong_brand(view: &PrivateMemoryView<'_, u32, ReadOnly, BrandA>) {
    brand_b_only(view);
}

fn main() {}
