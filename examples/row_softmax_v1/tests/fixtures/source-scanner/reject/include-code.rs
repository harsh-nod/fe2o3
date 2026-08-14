use vstd::prelude::*;

verus! {
include!("hidden.rs");
pub uninterp spec fn exp_real_v1(value: real) -> real;
} // verus!
