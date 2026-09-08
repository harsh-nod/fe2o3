// Expected-negative R60 mutation: substituted custody is restored as reusable.
use vstd::prelude::*;
verus! {
pub struct EntryV1 { pub identity: nat, pub quarantined: bool }
pub open spec fn mutated_restore_v1(entry: EntryV1, _observed_identity: nat) -> EntryV1 {
    EntryV1 { quarantined: false, ..entry }
}
pub proof fn mutated_substituted_restore_quarantines_v1(entry: EntryV1, substituted: nat)
    requires substituted != entry.identity,
    ensures mutated_restore_v1(entry, substituted).quarantined,
{}
}
