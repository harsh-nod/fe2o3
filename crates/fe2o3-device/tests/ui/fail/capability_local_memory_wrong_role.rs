use fe2o3_device::prelude::*;

enum Brand {}

fn read_write_only(_: &PrivateMemoryView<'_, u32, ExclusiveReadWrite, Brand>) {}

fn wrong_role(read_only: &PrivateMemoryView<'_, u32, ReadOnly, Brand>) {
    read_write_only(read_only);
}

fn main() {}
