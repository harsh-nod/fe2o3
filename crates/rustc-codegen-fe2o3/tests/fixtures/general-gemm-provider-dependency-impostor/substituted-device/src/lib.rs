#![no_std]

use core::marker::PhantomData;

pub struct DisjointSlice<T, IndexSpace = ()> {
    _marker: PhantomData<fn() -> (T, IndexSpace)>,
}
