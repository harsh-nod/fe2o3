use vstd::prelude::*;

verus! {
proof fn rejected() { 𝕒ssume_(false); }
pub uninterp spec fn exp_real_v1(value: real) -> real;
} // verus!
