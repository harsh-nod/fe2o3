//! Ordinary safe Rust inputs for integer-identity qualification.
#![allow(
    clippy::identity_op,
    reason = "the source identities are the test inputs"
)]
use fe2o3_device::{DisjointSlice, kernel, thread};

macro_rules! identity_root {
    ($ty:ident, $root:ident, $helper:ident, $value:ident, $expression:expr) => {
        #[cfg(feature = "integer-identity-retained")]
        #[inline(never)]
        fn $helper($value: $ty) -> $ty {
            $expression
        }

        #[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
        pub fn $root(mut output: DisjointSlice<$ty>, $value: $ty) {
            if let Some(element) = output.get_mut(thread::index_1d()) {
                #[cfg(feature = "integer-identity-retained")]
                let result = $helper($value);
                #[cfg(not(feature = "integer-identity-retained"))]
                let result = $expression;
                *element = result;
            }
        }
    };
}

macro_rules! identity_roots {
    ($ty:ident) => {
        identity_root!($ty, identity_xor_zero, xor_zero, value, value ^ 0);
        identity_root!($ty, identity_or_zero, or_zero, value, value | 0);
        identity_root!($ty, identity_and_ones, and_ones, value, value & !0);
        identity_root!($ty, identity_control, control, value, value ^ 3);
        identity_root!($ty, identity_noop, noop, value, value);
    };
}

#[cfg(feature = "integer-identity-i8")]
identity_roots!(i8);
#[cfg(feature = "integer-identity-u8")]
identity_roots!(u8);
#[cfg(feature = "integer-identity-i16")]
identity_roots!(i16);
#[cfg(feature = "integer-identity-u16")]
identity_roots!(u16);
#[cfg(feature = "integer-identity-i32")]
identity_roots!(i32);
#[cfg(feature = "integer-identity-u32")]
identity_roots!(u32);
#[cfg(feature = "integer-identity-i64")]
identity_roots!(i64);
#[cfg(feature = "integer-identity-u64")]
identity_roots!(u64);
