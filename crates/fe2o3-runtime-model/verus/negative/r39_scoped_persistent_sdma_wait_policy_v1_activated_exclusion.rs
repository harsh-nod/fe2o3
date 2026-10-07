// Expected-negative R39 mutation: ordinary batch/striped waits are activated
// for the persistent elapsed-spin profile.
use vstd::prelude::*;

verus! {
#[derive(PartialEq, Eq)] pub enum ProfileV1 { Default, Scoped(nat) }
pub open spec fn mutated_ordinary_batch_profile_v1() -> ProfileV1 { ProfileV1::Scoped(50_000) }

pub proof fn mutated_ordinary_batch_remains_default_v1()
    ensures mutated_ordinary_batch_profile_v1() == ProfileV1::Default,
{}
}
