//! Ordinary Rust inputs; names and features are test enumeration only.
use fe2o3_device::{DisjointSlice, kernel, thread};

macro_rules! cast_kernel {
    ($input:ident, $output:ident) => {
        #[cfg(feature = "numeric-cast-retained")]
        #[inline(never)]
        fn retained_conversion(value: $input) -> $output {
            value as $output
        }

        #[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
        pub fn numeric_cast(mut output: DisjointSlice<$output>, value: $input) {
            if let Some(element) = output.get_mut(thread::index_1d()) {
                #[cfg(feature = "numeric-cast-retained")]
                let result = retained_conversion(value);
                #[cfg(not(feature = "numeric-cast-retained"))]
                let result = value as $output;
                *element = result;
            }
        }
    };
}

macro_rules! direction {
    ($integer:ident) => {
        #[cfg(feature = "numeric-cast-to-int")]
        cast_kernel!(f32, $integer);
        #[cfg(not(feature = "numeric-cast-to-int"))]
        cast_kernel!($integer, f32);
    };
}

#[cfg(feature = "numeric-cast-i8")]
direction!(i8);
#[cfg(feature = "numeric-cast-u8")]
direction!(u8);
#[cfg(feature = "numeric-cast-i16")]
direction!(i16);
#[cfg(feature = "numeric-cast-u16")]
direction!(u16);
#[cfg(feature = "numeric-cast-i32")]
direction!(i32);
#[cfg(feature = "numeric-cast-u32")]
direction!(u32);
#[cfg(feature = "numeric-cast-i64")]
direction!(i64);
#[cfg(feature = "numeric-cast-u64")]
direction!(u64);
