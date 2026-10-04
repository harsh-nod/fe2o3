use vstd::prelude::*;

verus! {
pub assume_specification[ core::mem::size_of::<u64> ]() -> (value: usize);
pub uninterp spec fn exp_real_v1(value: real) -> real;
} // verus!
