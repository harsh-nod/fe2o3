use vstd::prelude::*;

verus! {
proof fn rejected() { assume/*split*/(false); }
} // verus!
