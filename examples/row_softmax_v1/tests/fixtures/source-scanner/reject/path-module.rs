use vstd::prelude::*;

#[path = "hidden.rs"]
mod hidden;
verus! {
pub uninterp spec fn exp_real_v1(value: real) -> real;
} // verus!
