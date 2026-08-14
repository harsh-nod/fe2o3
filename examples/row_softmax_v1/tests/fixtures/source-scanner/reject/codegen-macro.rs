use vstd::prelude::*;

verus! {
generate_proof!();
pub uninterp spec fn exp_real_v1(value: real) -> real;
} // verus!
