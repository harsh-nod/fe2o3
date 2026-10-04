use vstd::prelude::*;

verus! {
pub struct Holder;
impl Holder {
    pub uninterp spec fn exp_real_v1(value: real) -> real;
}
} // verus!
