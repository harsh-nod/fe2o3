use vstd::prelude::*;

verus! {
#[verifier::opaque]
pub uninterp spec fn exp_real_v1(value: real) -> real;
} // verus!
