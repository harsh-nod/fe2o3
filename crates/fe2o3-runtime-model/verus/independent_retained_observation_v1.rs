// Actual independent-account post-lock observation over borrowed records.
// Token presence, account-kind dispatch, Arc identity and raw Mutex acquisition
// remain in the unchanged native caller. This does not prove their semantics,
// cross-call freshness, conservation, custody, Context or native authority.
// Pinned vstd slice/array/equality specifications and Rust derive/compiler
// lowering retain the explicit boundary of the included record root.
include!("retained_credit_record_v1.rs");
include!("../../fe2o3-resource-accounting/src/independent_retained_observation_body.rs");

verus! {

spec fn independent_observation_matches(records: Seq<Option<Record>>, poisoned: bool,
    slot: usize, owner: u64, expected: R67ResourceVectorV1) -> bool
{
    &&& !poisoned
    &&& slot < records.len()
    &&& records[slot as int].is_some()
    &&& owner != 0
    &&& records[slot as int]->Some_0.owner == owner
    &&& records[slot as int]->Some_0.phase == Phase::Retained
    &&& records[slot as int]->Some_0.charge.counts@ == expected.counts@
}

fn independent_retained_observation_v1(records: &[Option<Record>], poisoned: bool,
    slot: usize, owner: u64, expected: R67ResourceVectorV1) -> (out: bool)
    ensures out == independent_observation_matches(records@, poisoned, slot, owner, expected),
{
    independent_retained_observation_body_v1!(records, poisoned, slot, owner, expected)
}

}
