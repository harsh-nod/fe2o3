// Expected-negative R35 mutation: a failed quarantine transition discards the
// still-live Prepared authority by labeling it Quarantined.
use vstd::prelude::*;

verus! {
#[derive(PartialEq, Eq)] pub enum AuthorityStateV1 { Prepared, Quarantined }
pub struct StateV1 { pub quarantine_succeeded: bool, pub authority: AuthorityStateV1 }

pub open spec fn mutated_failed_quarantine_v1() -> StateV1 {
    StateV1 { quarantine_succeeded: false, authority: AuthorityStateV1::Quarantined }
}

pub proof fn mutated_failed_quarantine_preserves_prepared_v1()
    ensures mutated_failed_quarantine_v1().authority == AuthorityStateV1::Prepared,
{}
}
