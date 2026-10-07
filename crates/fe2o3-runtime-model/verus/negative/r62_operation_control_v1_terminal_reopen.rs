// Expected-negative R62 host control mutation: terminal_reopen.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_terminal_v1(p: nat) -> nat { if p == 4 { 2 } else { p } }
pub proof fn mutated_terminal_reopen_v1()
    ensures mutated_terminal_v1(4) == 4,
{}
}
