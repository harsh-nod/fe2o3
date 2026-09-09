// Expected-negative R61 policy mutation: duplicate_reply.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_resolve_v1(resolved: bool, count: nat) -> nat { count + 1 }
pub proof fn mutated_reply_resolves_once_v1(count: nat)
    ensures mutated_resolve_v1(true, count) == count,
{}
}
