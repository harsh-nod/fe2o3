use core::marker::PhantomData;
use fe2o3_device::GridLeader;

fn main() {
    let _ = GridLeader {
        _private: (),
        _not_send_sync: PhantomData,
    };
}
