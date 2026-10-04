use vstd::prelude::*;

verus! {
#[verifier::assume_termination]
pub proof fn rejected() { }
pub uninterp spec fn exp_real_v1(value: real) -> real;
} // verus!
