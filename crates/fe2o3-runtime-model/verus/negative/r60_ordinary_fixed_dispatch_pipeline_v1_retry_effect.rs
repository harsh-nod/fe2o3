// Expected-negative R60 mutation: retryable publication records a native effect.
use vstd::prelude::*;
verus! {
pub struct EntryV1 { pub effects: nat, pub recipe: nat }
pub open spec fn mutated_retry_v1(entry: EntryV1) -> EntryV1 {
    EntryV1 { effects: entry.effects + 1, ..entry }
}
pub proof fn mutated_retry_is_exactly_no_effect_v1(entry: EntryV1)
    ensures mutated_retry_v1(entry) == entry,
{}
}
