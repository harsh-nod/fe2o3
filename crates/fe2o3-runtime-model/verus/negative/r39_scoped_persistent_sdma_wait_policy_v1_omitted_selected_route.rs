// Expected-negative R39 mutation: one allowlisted directional window keeps the
// default profile.
use vstd::prelude::*;

verus! {
#[derive(PartialEq, Eq)] pub enum ProfileV1 { Default, Scoped(nat) }
pub open spec fn mutated_directional_window_profile_v1() -> ProfileV1 { ProfileV1::Default }

pub proof fn mutated_directional_window_is_scoped_v1()
    ensures mutated_directional_window_profile_v1() == ProfileV1::Scoped(50_000),
{}
}
