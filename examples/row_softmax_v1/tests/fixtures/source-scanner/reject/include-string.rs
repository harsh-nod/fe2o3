use vstd::prelude::*;

verus! {
pub const HIDDEN: &str = include_str!("hidden.rs");
pub uninterp spec fn exp_real_v1(value: real) -> real;
} // verus!
