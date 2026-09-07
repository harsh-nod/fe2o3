// Expected-negative R48 mutation: two published requests share one signal identity.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_signals_are_distinct_v1(_left: nat, _right: nat) -> bool { true }
pub proof fn equal_signals_are_rejected_v1(signal: nat)
    ensures !mutated_signals_are_distinct_v1(signal, signal),
{}
}
