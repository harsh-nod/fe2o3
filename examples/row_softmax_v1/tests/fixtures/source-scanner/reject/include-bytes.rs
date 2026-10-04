use vstd::prelude::*;

verus! {
pub const HIDDEN: &[u8] = include_bytes!("hidden.rs");
pub uninterp spec fn exp_real_v1(value: real) -> real;
} // verus!
