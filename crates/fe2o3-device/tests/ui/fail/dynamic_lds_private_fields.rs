use core::marker::PhantomData;
use core::ptr::NonNull;
use fe2o3_device::{DynamicLds, LdsUninitialized};

fn main() {
    let _ = DynamicLds::<u32, LdsUninitialized> {
        ptr: NonNull::dangling(),
        len: 0,
        byte_len: 0,
        _borrow: PhantomData,
        _state: PhantomData,
        _not_send_sync: PhantomData,
    };
}
