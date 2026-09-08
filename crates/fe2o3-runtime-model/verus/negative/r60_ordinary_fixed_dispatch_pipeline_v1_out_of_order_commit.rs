// Expected-negative R60 mutation: a retired suffix bypasses the frontier.
use vstd::prelude::*;
verus! {
pub struct EntryV1 { pub retired: bool, pub committed: bool }
pub open spec fn mutated_commit_v1(entry: EntryV1) -> EntryV1 {
    if entry.retired { EntryV1 { committed: true, ..entry } } else { entry }
}
pub proof fn mutated_nonfrontier_retirement_stays_hidden_v1(entry: EntryV1)
    requires entry.retired, !entry.committed,
    ensures mutated_commit_v1(entry) == entry,
{}
}
