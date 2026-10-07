use super::*;
use fe2o3_resource_accounting::{ResourceKindV1, ResourceVectorV1};

fn account(bytes: usize) -> ResourceCreditAccountV1 {
    ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, bytes as u64),
        1,
    )
    .unwrap()
}

fn funded() -> (ResourceCreditAccountV1, Observations) {
    let account = account(SLOTS * size_of::<RuntimeGfx942ArenaMemberObservationV1>());
    let observations = Observations::new(&account).unwrap();
    (account, observations)
}

#[test]
fn observations_require_original_same_member_publication() {
    let (_account, mut observations) = funded();
    for event in [Event::Pending, Event::Completed] {
        assert!(observations.record(0, event).is_err());
        assert_eq!(
            observations.summary,
            RuntimeGfx942ArenaObservationV1::default()
        );
    }
    observations.record(1, Event::Published).unwrap();
    assert!(observations.record(0, Event::Pending).is_err());
    assert!(observations.record(1, Event::Published).is_err());
    assert!(observations.record(SLOTS, Event::Published).is_err());
    assert_eq!(observations.summary.events, 1);
}

#[test]
fn pending_then_later_completion_is_not_out_of_order_witness() {
    let (_account, mut observations) = funded();
    observations.record(0, Event::Published).unwrap();
    observations.record(1, Event::Published).unwrap();
    observations.record(0, Event::Pending).unwrap();
    observations.record(1, Event::Completed).unwrap();
    assert!(observations.summary.out_of_order.is_none());
    observations.record(0, Event::Completed).unwrap();
    assert!(observations.summary.out_of_order.is_none());
    assert!(observations.record(0, Event::Pending).is_err());
}

#[test]
fn later_ready_before_subsequent_earlier_pending_has_exact_sequences() {
    let (_account, mut observations) = funded();
    observations.record(0, Event::Published).unwrap();
    observations.record(17, Event::Published).unwrap();
    observations.record(17, Event::Completed).unwrap();
    observations.record(0, Event::Pending).unwrap();
    assert_eq!(
        observations.summary.out_of_order,
        Some(RuntimeGfx942ArenaOutOfOrderObservationV1 {
            earlier_member: 0,
            later_member: 17,
            earlier_published: 1,
            later_published: 2,
            later_completed: 3,
            earlier_pending: 4,
        })
    );
    assert_eq!(
        observations
            .summary
            .maximum_published_without_observed_completion,
        2
    );
    assert_eq!(observations.summary.pending_observations, 1);
}

#[test]
fn earlier_completion_then_later_pending_is_not_out_of_order() {
    let (_account, mut observations) = funded();
    observations.record(0, Event::Published).unwrap();
    observations.record(1, Event::Published).unwrap();
    observations.record(0, Event::Completed).unwrap();
    observations.record(1, Event::Pending).unwrap();
    assert!(observations.summary.out_of_order.is_none());
}

#[test]
fn earlier_index_published_late_is_not_a_reordered_publication_witness() {
    let (_account, mut observations) = funded();
    observations.record(1, Event::Published).unwrap();
    observations.record(1, Event::Completed).unwrap();
    observations.record(0, Event::Published).unwrap();
    observations.record(0, Event::Pending).unwrap();
    assert!(observations.summary.out_of_order.is_none());
}

#[test]
fn finite_sequence_overflow_refuses_without_event_or_member_mutation() {
    let (_account, mut observations) = funded();
    observations.summary.events = u64::MAX;
    assert!(observations.record(0, Event::Published).is_err());
    assert_eq!(
        observations.members[0],
        RuntimeGfx942ArenaMemberObservationV1::default()
    );
    assert_eq!(observations.summary.publications, 0);
}

#[test]
fn observation_table_exact_debit_and_one_short_refusal() {
    let bytes = SLOTS * size_of::<RuntimeGfx942ArenaMemberObservationV1>();
    let short = account(bytes - 1);
    assert!(Observations::new(&short).is_err());
    assert_eq!(short.usage().used, ResourceVectorV1::ZERO);
    let (account, observations) = funded();
    assert_eq!(account.usage().retained_records, 1);
    assert_eq!(
        account.usage().used,
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, bytes as u64)
    );
    drop(observations);
    assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
    assert_eq!(account.usage().retained_records, 0);
}

#[test]
fn full_roster_observed_publication_is_not_hardware_overlap_claim() {
    let (_account, mut observations) = funded();
    for index in 0..SLOTS {
        observations.record(index, Event::Published).unwrap();
    }
    for index in 0..SLOTS {
        observations.record(index, Event::Completed).unwrap();
    }
    assert_eq!(observations.summary.publications, SLOTS);
    assert_eq!(observations.summary.completions, SLOTS);
    assert_eq!(
        observations
            .summary
            .maximum_published_without_observed_completion,
        SLOTS
    );
    assert!(observations.summary.out_of_order.is_none());
}
