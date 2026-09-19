use fe2o3_device::{DisjointSlice, kernel, thread};

// Test symbols share the exact-CSE ABI/SIM contract, never a compiler selector.
macro_rules! define {
    ($ty:ident) => {
        #[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
        pub fn dominance_and(mut output: DisjointSlice<$ty>, lhs: $ty, rhs: $ty, choose: u32) {
            if let Some(element) = output.get_mut(thread::index_1d()) {
                *element = lhs & rhs;
                if choose != 0 {
                    let quotient = choose / choose;
                    if quotient != 0 {
                        *element = rhs & lhs;
                    }
                }
            }
        }
        #[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
        pub fn dominance_or(mut output: DisjointSlice<$ty>, lhs: $ty, rhs: $ty, choose: u32) {
            if let Some(element) = output.get_mut(thread::index_1d()) {
                *element = lhs | rhs;
                if choose != 0 {
                    let quotient = choose / choose;
                    if quotient != 0 {
                        *element = rhs | lhs;
                    }
                }
            }
        }
        #[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
        pub fn dominance_xor(mut output: DisjointSlice<$ty>, lhs: $ty, rhs: $ty, choose: u32) {
            if let Some(element) = output.get_mut(thread::index_1d()) {
                *element = lhs ^ rhs;
                if choose != 0 {
                    let quotient = choose / choose;
                    if quotient != 0 {
                        *element = rhs ^ lhs;
                    }
                }
            }
        }
    };
}

#[cfg(feature = "commutative-cse-i8")]
define!(i8);
#[cfg(feature = "commutative-cse-u8")]
define!(u8);
#[cfg(feature = "commutative-cse-i16")]
define!(i16);
#[cfg(feature = "commutative-cse-u16")]
define!(u16);
#[cfg(feature = "commutative-cse-i32")]
define!(i32);
#[cfg(feature = "commutative-cse-u32")]
define!(u32);
#[cfg(feature = "commutative-cse-i64")]
define!(i64);
#[cfg(feature = "commutative-cse-u64")]
define!(u64);
