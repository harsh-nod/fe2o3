use core::marker::PhantomData;
use fe2o3_device::Gfx942StaticLdsU32x256;

fn main() {
    let _ = Gfx942StaticLdsU32x256 {
        _private: (),
        _not_send_sync: PhantomData,
    };
}
