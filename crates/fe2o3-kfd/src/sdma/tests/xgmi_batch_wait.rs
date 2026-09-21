use super::*;
use crate::shared_memory::PreparationMemoryFixtureV1;

type MappingPair = (Gfx942DeviceMemoryIdentityV1, Gfx942DeviceMemoryIdentityV1);

pub(super) fn fixture() -> (
    PreparationMemoryFixtureV1,
    Gfx942SdmaQueueOwnerV1,
    Vec<Gfx942SdmaCopyTicketV1>,
    Vec<MappingPair>,
) {
    let mut memory = PreparationMemoryFixtureV1::new(true);
    let completion_cpu = memory
        .allocate::<HostVisibleCoherentGttV1>(GFX942_SDMA_RING_BYTES_V1 as usize)
        .unwrap();
    let mut completions = memory.map(completion_cpu).unwrap();
    memory.enable_sdma_mapped_bytes_v1();
    SdmaSingleMemoryV1::overwrite_mapped_host_visible_subrange_in_current_scope(
        &mut memory,
        &mut completions,
        0,
        &[0; 16],
    )
    .unwrap();

    let mut owner = compute_coexistence_owner_for_test(GFX942_SDMA_D2H_ENGINE_INDEX_V1);
    owner.completions = Some(completions);
    let mut tickets = Vec::new();
    let mut identities = Vec::new();
    for (slot, generation) in [(0_usize, 3_u32), (1, 5)] {
        let source = crate::shared_memory::xgmi_mapping_for_sdma_test(100 + 2 * slot as u64);
        let destination = crate::shared_memory::xgmi_mapping_for_sdma_test(101 + 2 * slot as u64);
        identities.push((
            source.lease().storage_identity(),
            destination.lease().storage_identity(),
        ));
        owner.generations[slot] = generation;
        owner.xgmi_records[slot] = Some(XgmiSdmaCopyRecordV1 {
            generation,
            completion_value: generation,
            source,
            destination,
            copy_bytes: 4096,
        });
        tickets.push(Gfx942SdmaCopyTicketV1 {
            owner: owner.owner,
            queue_id: owner.queue_id,
            slot: slot as u16,
            generation,
        });
    }
    (memory, owner, tickets, identities)
}

pub(super) fn write_completion(
    memory: &mut PreparationMemoryFixtureV1,
    owner: &mut Gfx942SdmaQueueOwnerV1,
    slot: usize,
    value: i64,
) {
    SdmaSingleMemoryV1::overwrite_mapped_host_visible_subrange_in_current_scope(
        memory,
        owner.completions.as_mut().unwrap(),
        (slot * core::mem::size_of::<i64>()) as u64,
        &value.to_le_bytes(),
    )
    .unwrap();
}

pub(super) fn retained_identities(owner: &Gfx942SdmaQueueOwnerV1) -> Vec<MappingPair> {
    owner
        .xgmi_records
        .iter()
        .filter_map(Option::as_ref)
        .map(|record| {
            (
                record.source.lease().storage_identity(),
                record.destination.lease().storage_identity(),
            )
        })
        .collect()
}

pub(super) fn completed_identities(completed: Vec<Gfx942XgmiCompletedCopyV1>) -> Vec<MappingPair> {
    completed
        .into_iter()
        .map(|completed| {
            assert_eq!(completed.copy_bytes(), 4096);
            let (source, destination) = completed.into_mappings();
            (
                source.lease().storage_identity(),
                destination.lease().storage_identity(),
            )
        })
        .collect()
}

fn assert_contract(
    result: Result<Vec<Gfx942XgmiCompletedCopyV1>, Gfx942SdmaErrorV1>,
    expected: &'static str,
) {
    match result {
        Err(Gfx942SdmaErrorV1::Contract(actual)) => assert_eq!(actual, expected),
        Ok(_) => panic!("expected {expected:?} contract failure, got completion"),
        Err(error) => panic!("expected {expected:?} contract failure, got {error:?}"),
    }
}

#[test]
fn expired_absolute_deadline_completes_an_all_ready_roster() {
    let (mut memory, mut owner, mut tickets, mut identities) = fixture();
    write_completion(&mut memory, &mut owner, 0, i64::from(tickets[0].generation));
    write_completion(&mut memory, &mut owner, 1, i64::from(tickets[1].generation));
    tickets.reverse();
    identities.reverse();

    let completed = owner
        .wait_many_xgmi_for_in_current_scope(
            &mut memory,
            &tickets,
            XgmiBatchDeadlineV1::Absolute(Instant::now()),
        )
        .unwrap();

    assert_eq!(completed_identities(completed), identities);
    assert!(owner.xgmi_records.iter().all(Option::is_none));
}

#[test]
fn expired_absolute_deadline_retains_a_ready_prefix_then_retry_completes() {
    let (mut memory, mut owner, tickets, identities) = fixture();
    write_completion(&mut memory, &mut owner, 0, i64::from(tickets[0].generation));

    let result = owner.wait_many_xgmi_for_in_current_scope(
        &mut memory,
        &tickets,
        XgmiBatchDeadlineV1::Absolute(Instant::now()),
    );
    assert!(matches!(result, Err(Gfx942SdmaErrorV1::Timeout)));
    assert_eq!(retained_identities(&owner), identities);

    write_completion(&mut memory, &mut owner, 1, i64::from(tickets[1].generation));
    let completed = owner
        .wait_many_xgmi_for_in_current_scope(
            &mut memory,
            &tickets,
            XgmiBatchDeadlineV1::Absolute(Instant::now()),
        )
        .unwrap();
    assert_eq!(completed_identities(completed), identities);
    assert!(owner.xgmi_records.iter().all(Option::is_none));
}

#[test]
fn relative_zero_deadline_keeps_the_existing_one_scan_semantics() {
    let (mut memory, mut owner, tickets, identities) = fixture();
    write_completion(&mut memory, &mut owner, 0, i64::from(tickets[0].generation));

    let result = owner.wait_many_xgmi_for_in_current_scope(
        &mut memory,
        &tickets,
        XgmiBatchDeadlineV1::Relative(Duration::ZERO),
    );
    assert!(matches!(result, Err(Gfx942SdmaErrorV1::Timeout)));
    assert_eq!(retained_identities(&owner), identities);

    write_completion(&mut memory, &mut owner, 1, i64::from(tickets[1].generation));
    let completed = owner
        .wait_many_xgmi_for_in_current_scope(
            &mut memory,
            &tickets,
            XgmiBatchDeadlineV1::Relative(Duration::ZERO),
        )
        .unwrap();
    assert_eq!(completed_identities(completed), identities);
}

#[test]
fn malformed_ticket_rosters_are_rejected_without_consuming_records() {
    let (mut memory, mut owner, tickets, identities) = fixture();
    let deadline = || XgmiBatchDeadlineV1::Absolute(Instant::now());

    assert_contract(
        owner.wait_many_xgmi_for_in_current_scope(&mut memory, &[], deadline()),
        "XGMI SDMA wait batch size",
    );

    let oversized = vec![tickets[0]; GFX942_SDMA_MAX_IN_FLIGHT_V1 + 1];
    assert_contract(
        owner.wait_many_xgmi_for_in_current_scope(&mut memory, &oversized, deadline()),
        "XGMI SDMA wait batch size",
    );
    assert_contract(
        owner.wait_many_xgmi_for_in_current_scope(
            &mut memory,
            &[tickets[0], tickets[0]],
            deadline(),
        ),
        "duplicate XGMI SDMA wait ticket",
    );

    let mut foreign = tickets[0];
    foreign.owner = queue_key(8, 11, 13);
    assert_contract(
        owner.wait_many_xgmi_for_in_current_scope(&mut memory, &[foreign], deadline()),
        "XGMI SDMA ticket queue occurrence",
    );
    let mut stale = tickets[0];
    stale.generation += 1;
    assert_contract(
        owner.wait_many_xgmi_for_in_current_scope(&mut memory, &[stale], deadline()),
        "XGMI SDMA ticket generation",
    );

    assert_eq!(retained_identities(&owner), identities);
    assert!(!owner.is_poisoned());
}

#[test]
fn unexpected_completion_poisons_the_owner_and_retains_every_mapping() {
    let (mut memory, mut owner, tickets, identities) = fixture();
    write_completion(&mut memory, &mut owner, 0, i64::from(tickets[0].generation));
    write_completion(
        &mut memory,
        &mut owner,
        1,
        i64::from(tickets[1].generation) + 1,
    );

    assert_contract(
        owner.wait_many_xgmi_for_in_current_scope(
            &mut memory,
            &tickets,
            XgmiBatchDeadlineV1::Absolute(Instant::now()),
        ),
        "unexpected XGMI SDMA batch completion value",
    );
    assert!(owner.is_poisoned());
    assert_eq!(retained_identities(&owner), identities);
}

#[test]
fn completion_observation_panic_preserves_every_mapping_and_payload() {
    let (mut memory, mut owner, tickets, identities) = fixture();
    memory.sdma_mapping_panic_v1(owner.completions.as_ref().unwrap(), "observe_i64_acquire");

    let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = owner.wait_many_xgmi_for_in_current_scope(
            &mut memory,
            &tickets,
            XgmiBatchDeadlineV1::Absolute(Instant::now()),
        );
    }))
    .unwrap_err();

    assert_eq!(
        payload.downcast_ref::<(&'static str, &'static str)>(),
        Some(&("N1 mapped panic", "observe_i64_acquire"))
    );
    assert_eq!(retained_identities(&owner), identities);
}
