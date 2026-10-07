// Expected-negative R60 mutation: repeating host commit duplicates effects.
use vstd::prelude::*;
verus! {
pub struct EntryV1 { pub effects: nat }
pub open spec fn mutated_commit_v1(entry: EntryV1) -> EntryV1 {
    EntryV1 { effects: entry.effects + 1 }
}
pub proof fn mutated_commit_is_exactly_once_v1(entry: EntryV1)
    ensures mutated_commit_v1(mutated_commit_v1(entry)) == mutated_commit_v1(entry),
{}
}
