use fe2o3_device::{InitialEpoch, WorkgroupCapability};

enum KernelA {}
enum KernelB {}

fn needs_a(_: &WorkgroupCapability<'_, KernelA, InitialEpoch>) {}

fn substitute(group: &WorkgroupCapability<'_, KernelB, InitialEpoch>) {
    needs_a(group);
}

fn main() {}
