// Expected-negative R60 mutation: physical retirement releases predecessor retain.
use vstd::prelude::*;
verus! {
pub struct EntryV1 { pub retired: bool, pub predecessor_retain: bool }
pub open spec fn mutated_retire_v1(_entry: EntryV1) -> EntryV1 {
    EntryV1 { retired: true, predecessor_retain: false }
}
pub proof fn mutated_predecessor_retain_survives_retirement_v1(entry: EntryV1)
    requires entry.predecessor_retain,
    ensures mutated_retire_v1(entry).predecessor_retain,
{}
}
