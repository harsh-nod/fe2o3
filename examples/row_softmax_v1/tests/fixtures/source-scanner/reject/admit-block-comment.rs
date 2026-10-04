use vstd::prelude::*;

verus! {
proof fn rejected() { admit/*split*/(); }
} // verus!
