use vstd::prelude::*;

verus! {
proof fn rejected() { verus_builtin::assert_(true); }
pub uninterp spec fn exp_real_v1(value: real) -> real;
} // verus!
