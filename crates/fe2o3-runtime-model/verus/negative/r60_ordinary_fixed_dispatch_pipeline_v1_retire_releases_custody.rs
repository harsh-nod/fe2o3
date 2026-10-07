// Expected-negative R60 mutation: physical retirement releases logical custody.
use vstd::prelude::*;
verus! {
pub struct EntryV1 { pub physically_retired: bool, pub owns_custody: bool }
pub open spec fn mutated_retire_v1(entry: EntryV1) -> EntryV1 {
    EntryV1 { physically_retired: true, owns_custody: false }
}
pub proof fn mutated_retirement_retains_custody_v1(entry: EntryV1)
    ensures mutated_retire_v1(entry).owns_custody,
{}
}
