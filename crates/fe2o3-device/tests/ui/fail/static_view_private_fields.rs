use core::marker::PhantomData;
use fe2o3_contracts::StaticViewContractV1;
use fe2o3_device::{Index1D, StaticViewMut};

fn forge<'a>(ptr: *mut u32, contract: StaticViewContractV1) -> StaticViewMut<'a, u32, 4> {
    StaticViewMut::<u32, 4, Index1D> {
        ptr,
        contract,
        _borrow: PhantomData,
        _index_space: PhantomData,
    }
}

fn main() {}
