// Expected-negative R60 mutation: physical retirement publishes the profile.
use vstd::prelude::*;
verus! {
pub struct EntryV1 { pub retired: bool, pub profile_visible: bool }
pub open spec fn mutated_retire_v1(_entry: EntryV1) -> EntryV1 {
    EntryV1 { retired: true, profile_visible: true }
}
pub proof fn mutated_retired_profile_is_hidden_v1(entry: EntryV1)
    ensures !mutated_retire_v1(entry).profile_visible,
{}
}
