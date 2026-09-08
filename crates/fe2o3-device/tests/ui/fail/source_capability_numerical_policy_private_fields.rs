use core::marker::PhantomData;

use fe2o3_device::{NumericalPolicyCapability, StrictIeee};

fn reject() {
    let _ = NumericalPolicyCapability::<(), StrictIeee> {
        _kernel: PhantomData,
        _policy: PhantomData,
        _not_send_sync: PhantomData,
    };
}

fn main() {}
