// Expected-negative R74 mutation: recovered failure can reopen for publication.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_open_v1(open: bool, recovered: bool) -> bool { !open }
pub proof fn mutated_reopen_recovery_v1()
    ensures !mutated_open_v1(false, true),
{}
}
