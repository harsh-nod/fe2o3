use vstd::prelude::*;

macro_rules! generate_proof { () => { proof fn generated() { } } }
verus! {
pub uninterp spec fn exp_real_v1(value: real) -> real;
} // verus!
