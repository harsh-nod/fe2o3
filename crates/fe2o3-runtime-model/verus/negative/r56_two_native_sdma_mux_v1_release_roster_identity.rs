// Expected-negative R56 mutation: completion release trusts cardinality while
// ignoring exact ticket-roster identity.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_release_admits_v1(expected: Seq<nat>, observed: Seq<nat>) -> bool {
    expected.len() == observed.len()
}
pub proof fn mutated_release_roster_identity_is_rejected_v1()
    ensures !mutated_release_admits_v1(seq![1nat, 2nat], seq![1nat, 3nat]),
{}
}
fn main() {}
