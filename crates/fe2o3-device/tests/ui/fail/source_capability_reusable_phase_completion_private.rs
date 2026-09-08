use core::marker::PhantomData;

use fe2o3_device::ReusablePhaseCompletion;

fn reject() {
    let _ = ReusablePhaseCompletion::<'static, 'static, ()> {
        _phase: PhantomData,
        _workgroup: PhantomData,
        _not_send_sync: PhantomData,
    };
}

fn main() {}
