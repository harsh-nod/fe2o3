use core::marker::PhantomData;
use fe2o3_device::StaticViewMut;

fn forge<'a>(ptr: *mut u32) -> StaticViewMut<'a, u32, 4> {
    StaticViewMut::<u32, 4> {
        ptr,
        _borrow: PhantomData,
        _not_send_sync: PhantomData,
    }
}

fn main() {}
