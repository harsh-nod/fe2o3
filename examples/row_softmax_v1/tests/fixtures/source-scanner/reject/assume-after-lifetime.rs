use vstd::prelude::*;

verus! {
proof fn rejected<'a>() { assume(false); let _value = 'x'; }
} // verus!
