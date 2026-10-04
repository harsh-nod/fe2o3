use fe2o3_device::{DisjointSlice, kernel, thread};

macro_rules! define {
    ($ty:ident) => {
        #[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
        pub fn dominance_and(mut output: DisjointSlice<$ty>, lhs: $ty, rhs: $ty, choose: u32) {
            if let Some(element) = output.get_mut(thread::index_1d()) {
                let anchor = lhs & rhs;
                *element = anchor;
                if choose != 0 {
                    *element = lhs & rhs;
                }
            }
        }
        #[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
        pub fn dominance_or(mut output: DisjointSlice<$ty>, lhs: $ty, rhs: $ty, choose: u32) {
            if let Some(element) = output.get_mut(thread::index_1d()) {
                let anchor = lhs | rhs;
                *element = anchor;
                if choose != 0 {
                    *element = lhs | rhs;
                }
            }
        }
        #[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
        pub fn dominance_xor(mut output: DisjointSlice<$ty>, lhs: $ty, rhs: $ty, choose: u32) {
            if let Some(element) = output.get_mut(thread::index_1d()) {
                let anchor = lhs ^ rhs;
                *element = anchor;
                if choose != 0 {
                    *element = lhs ^ rhs;
                }
            }
        }
    };
}

#[cfg(feature = "dominance-cse-i8")]
define!(i8);
#[cfg(feature = "dominance-cse-u8")]
define!(u8);
#[cfg(feature = "dominance-cse-i16")]
define!(i16);
#[cfg(feature = "dominance-cse-u16")]
define!(u16);
#[cfg(feature = "dominance-cse-i32")]
define!(i32);
#[cfg(feature = "dominance-cse-u32")]
define!(u32);
#[cfg(feature = "dominance-cse-i64")]
define!(i64);
#[cfg(feature = "dominance-cse-u64")]
define!(u64);
