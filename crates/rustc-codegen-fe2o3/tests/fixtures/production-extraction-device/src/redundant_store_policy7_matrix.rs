#![allow(unused_assignments)]
use fe2o3_device::{DisjointSlice, kernel, thread};

macro_rules! instantiate {
    ($ty:ident) => {
        #[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
        pub fn private_store_policy7_matrix(mut output: DisjointSlice<$ty>, input: $ty) {
            if let Some(element) = output.get_mut(thread::index_1d()) {
                let mut local = input;
                local = input;
                let reference = &mut local;
                *element = input;
                *element = *reference;
            }
        }
    };
}

#[cfg(feature = "redundant-store-policy7-i8")]
instantiate!(i8);
#[cfg(feature = "redundant-store-policy7-u8")]
instantiate!(u8);
#[cfg(feature = "redundant-store-policy7-i16")]
instantiate!(i16);
#[cfg(feature = "redundant-store-policy7-u16")]
instantiate!(u16);
#[cfg(feature = "redundant-store-policy7-i32")]
instantiate!(i32);
#[cfg(feature = "redundant-store-policy7-u32")]
instantiate!(u32);
#[cfg(feature = "redundant-store-policy7-i64")]
instantiate!(i64);
#[cfg(feature = "redundant-store-policy7-u64")]
instantiate!(u64);
