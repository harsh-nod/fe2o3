use fe2o3_device::{NumericalPolicyCapability, StrictIeee};

fn assert_send<T: Send>() {}
fn assert_sync<T: Sync>() {}
fn assert_copy<T: Copy>() {}
fn assert_clone<T: Clone>() {}

fn reject() {
    assert_send::<NumericalPolicyCapability<(), StrictIeee>>();
    assert_sync::<NumericalPolicyCapability<(), StrictIeee>>();
    assert_copy::<NumericalPolicyCapability<(), StrictIeee>>();
    assert_clone::<NumericalPolicyCapability<(), StrictIeee>>();
}

fn main() {}
