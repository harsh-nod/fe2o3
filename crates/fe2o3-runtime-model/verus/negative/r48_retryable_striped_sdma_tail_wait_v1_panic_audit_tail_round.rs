// Expected-negative R48 mutation: audit-prefix panic omits the completed tail round.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_panic_rounds_v1(rounds_before: nat) -> nat { rounds_before }
pub proof fn audit_panic_includes_completed_tail_round_v1(rounds_before: nat)
    ensures mutated_panic_rounds_v1(rounds_before) == rounds_before + 1,
{}
}
