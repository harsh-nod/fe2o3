// Expected-negative R48 mutation: a non-system+snoop fence header is accepted.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_header_v1() -> nat { 0x0053_0004 }
pub proof fn fence_header_is_exact_v1()
    ensures mutated_header_v1() == 0x0053_0005,
{}
}
