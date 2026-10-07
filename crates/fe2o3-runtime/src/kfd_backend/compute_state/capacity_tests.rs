use super::super::tests::pipelined_active_for_test_v1 as active;
use super::*;
use fe2o3_resource_accounting::{
    ResourceKindV1, ResourceVectorV1, host_metadata_table_payload_bytes_v1,
};

fn payload() -> u64 {
    host_metadata_table_payload_bytes_v1::<RuntimeComputePipelineSlotV1>(1024).unwrap()
}

#[test]
fn ordered_publication_stage_retry_preserves_epochs_and_burns_generations() {
    for older in [false, true] {
        let mut pipeline = RuntimeComputePipelineV1::vacant();
        if older {
            pipeline.insert_published(active(2)).unwrap();
        }
        let epoch = pipeline.next_logical_epoch;
        let frontier = pipeline.commit_frontier;
        let mut previous = None;
        for _ in 0..128 {
            let identity = pipeline.stage_publication_v1(active(3)).unwrap();
            assert_eq!(Some(identity.logical_epoch), epoch);
            assert_eq!(pipeline.len(), usize::from(older) + 1);
            assert!(pipeline.contains(3));
            assert_eq!(pipeline.iter().count(), pipeline.len());
            assert!(pipeline.take_commit_frontier().is_none());
            assert!(pipeline.checked_frontier_v1().is_err());
            assert!(pipeline.stage_publication_v1(active(4)).is_err());
            if let Some(stale) = previous {
                assert!(pipeline.entry_v1(stale).is_none());
                assert!(pipeline.confirm_publication_v1(stale).is_err());
                assert!(pipeline.withdraw_publication_v1(stale).is_none());
                assert!(identity.slot_generation > stale.slot_generation);
            }
            assert_eq!(pipeline.withdraw_publication_v1(identity).unwrap().id, 3);
            assert!(pipeline.entry_v1(identity).is_none());
            assert_eq!(pipeline.next_logical_epoch, epoch);
            assert_eq!(pipeline.commit_frontier, frontier);
            assert_eq!(pipeline.len(), usize::from(older));
            previous = Some(identity);
        }
        let confirmed = pipeline.stage_publication_v1(active(3)).unwrap();
        pipeline.confirm_publication_v1(confirmed).unwrap();
        assert!(pipeline.confirm_publication_v1(confirmed).is_err());
        assert!(pipeline.withdraw_publication_v1(confirmed).is_none());
        let next = pipeline.insert_published(active(4)).unwrap();
        assert_eq!(next.logical_epoch, confirmed.logical_epoch + 1);
        if older {
            assert_eq!(pipeline.take_commit_frontier().unwrap().1.id, 2);
        }
        assert_eq!(pipeline.take_commit_frontier().unwrap().1.id, 3);
        assert_eq!(pipeline.take_commit_frontier().unwrap().1.id, 4);
        assert!(pipeline.is_empty());
    }
}

#[test]
fn ordered_publication_stage_refuses_corrupt_or_quarantined_identity() {
    for case in 0..10 {
        let mut pipeline = RuntimeComputePipelineV1::vacant();
        pipeline.insert_published(active(2)).unwrap();
        let identity = pipeline.stage_publication_v1(active(3)).unwrap();
        match case {
            0 => pipeline.staged = None,
            1 => pipeline.next_logical_epoch = None,
            2 => pipeline.live = 0,
            3 => pipeline.live += 1,
            4 => pipeline.slots[identity.slot as usize].generation += 1,
            5 => pipeline.entry_mut_v1(identity).unwrap().active.id = 4,
            6 => {
                pipeline.entry_mut_v1(identity).unwrap().phase =
                    RuntimeComputePipelinePhaseV1::Published
            }
            7 => pipeline.quarantine_all(),
            8 => pipeline.commit_frontier = None,
            9 => pipeline.next_logical_epoch = Some(identity.logical_epoch + 1),
            _ => unreachable!(),
        }
        let before = (
            pipeline.live,
            pipeline.next_logical_epoch,
            pipeline.commit_frontier,
            pipeline.staged,
        );
        assert!(pipeline.confirm_publication_v1(identity).is_err());
        assert!(pipeline.withdraw_publication_v1(identity).is_none());
        assert_eq!(
            (
                pipeline.live,
                pipeline.next_logical_epoch,
                pipeline.commit_frontier,
                pipeline.staged
            ),
            before
        );
        assert!(pipeline.slots[0].entry.is_some());
        assert!(pipeline.slots[identity.slot as usize].entry.is_some());
    }
}

#[test]
fn ordered_publication_stage_exhaustion_never_wraps_or_consumes_an_epoch_on_retry() {
    let mut pipeline = RuntimeComputePipelineV1::vacant();
    assert!(pipeline.stage_publication_v1(active(0)).is_err());
    pipeline.next_logical_epoch = Some(0);
    assert!(pipeline.stage_publication_v1(active(2)).is_err());
    pipeline.next_logical_epoch = Some(u64::MAX);
    let first = pipeline.stage_publication_v1(active(2)).unwrap();
    pipeline.withdraw_publication_v1(first).unwrap();
    assert_eq!(pipeline.next_logical_epoch, Some(u64::MAX));
    let last = pipeline.stage_publication_v1(active(2)).unwrap();
    assert_eq!(last.logical_epoch, u64::MAX);
    pipeline.confirm_publication_v1(last).unwrap();
    assert_eq!(pipeline.next_logical_epoch, None);
    assert!(pipeline.stage_publication_v1(active(3)).is_err());
    assert_eq!(pipeline.take_commit_frontier().unwrap().1.id, 2);
    assert!(!pipeline.has_successor_capacity());
    let mut slots = RuntimeComputePipelineV1::vacant();
    slots.exhaust_vacant_identities_for_test_v1();
    assert!(slots.stage_publication_v1(active(2)).is_err());
    assert_eq!(slots.next_logical_epoch, Some(1));
    assert!(slots.is_empty());
}

#[test]
fn ordered_publication_stage_metadata_does_not_allocate() {
    for publish in [false, true] {
        let mut pipeline = RuntimeComputePipelineV1::vacant();
        pipeline.insert_published(active(2)).unwrap();
        let owner = active(3);
        let ((), allocations) = super::super::drain_capture::tests::counted(|| {
            let identity = pipeline.stage_publication_v1(owner).unwrap();
            if publish {
                pipeline.confirm_publication_v1(identity).unwrap();
            } else {
                assert_eq!(pipeline.withdraw_publication_v1(identity).unwrap().id, 3);
            }
        });
        assert_eq!(allocations, 0);
        assert_eq!(pipeline.len(), 1 + usize::from(publish));
    }
}

#[test]
fn materialized_completion_pipeline_promotion_checks_occupancy_and_identity() {
    for case in 0..8 {
        let mut pipeline = RuntimeComputePipelineV1::vacant();
        let first = pipeline.insert_published(active(2)).unwrap();
        let second = pipeline.insert_published(active(3)).unwrap();
        assert_eq!(
            pipeline.checked_frontier_v1().unwrap().unwrap().identity,
            first
        );
        match case {
            0 => {
                pipeline.live = 0;
                pipeline.commit_frontier = None;
            }
            1 => pipeline.live = 1,
            2 => pipeline.slots[0].generation += 1,
            3 => pipeline.slots[0].entry.as_mut().unwrap().identity.slot = 1,
            4 => pipeline.slots[0].entry.as_mut().unwrap().active.id = 4,
            5 => {
                pipeline.slots[1]
                    .entry
                    .as_mut()
                    .unwrap()
                    .identity
                    .logical_epoch = first.logical_epoch
            }
            6 => {
                pipeline.slots[1]
                    .entry
                    .as_mut()
                    .unwrap()
                    .identity
                    .logical_epoch += 1
            }
            7 => pipeline.commit_frontier = None,
            _ => unreachable!(),
        }
        assert!(pipeline.checked_frontier_v1().is_err());
        assert!(pipeline.slots[0].entry.is_some());
        assert!(pipeline.slots[1].entry.is_some());
        if case != 2 && case != 3 && case != 4 {
            assert!(pipeline.entry_mut_v1(first).is_some());
        }
        if case != 5 && case != 6 {
            assert!(pipeline.entry_mut_v1(second).is_some());
        }
    }
}

fn account(bytes: u64) -> ResourceCreditAccountV1 {
    ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, bytes),
        4,
    )
    .unwrap()
}

fn scaled(account: &ResourceCreditAccountV1) -> RuntimeComputePipelineV1 {
    RuntimeComputePipelineV1::try_vacant(
        Gfx942FixedDispatchCapacityProfileV1::Qualification1024,
        Some(account),
    )
    .unwrap()
}

#[test]
fn scaled_pipeline_rejects_unaccounted_and_insufficient_payload_credit() {
    assert!(matches!(
        RuntimeComputePipelineV1::try_vacant(
            Gfx942FixedDispatchCapacityProfileV1::Qualification1024,
            None,
        ),
        Err(ResourceCreditErrorV1::Capacity)
    ));
    let account = account(payload() - 1);
    let before = account.usage();
    assert!(matches!(
        RuntimeComputePipelineV1::try_vacant(
            Gfx942FixedDispatchCapacityProfileV1::Qualification1024,
            Some(&account),
        ),
        Err(ResourceCreditErrorV1::Capacity)
    ));
    assert_eq!(account.usage(), before);
    let default = RuntimeComputePipelineV1::vacant();
    assert_eq!(default.slots.len(), 64);
}

#[test]
fn scaled_pipeline_high_slot_retirement_preserves_contiguous_commit_and_aba() {
    let account = account(payload());
    let mut pipeline = scaled(&account);
    // Burning one vacant slot forces use of index 1023 without changing the
    // 1023-successor bound (the active frontier is owned separately).
    pipeline.slots[0].generation = u64::MAX;
    for id in 2..=1024 {
        let identity = pipeline.insert_published(active(id)).unwrap();
        assert_eq!(u64::from(identity.slot), id - 1);
    }
    assert_eq!(pipeline.len(), 1023);
    assert_eq!(
        pipeline.insert_published(active(1025)).unwrap_err().id,
        1025
    );
    let stale = pipeline.identity_for_submission_v1(2).unwrap();
    let (high, owner) = pipeline.take_physical_owner(1024).unwrap();
    assert_eq!(high.slot, 1023);
    pipeline
        .restore(
            high,
            RuntimeComputePipelinePhaseV1::PhysicallyRetired,
            owner,
        )
        .unwrap();
    for expected in 2..=1024 {
        let (phase, owner) = pipeline.take_commit_frontier().unwrap();
        assert_eq!(owner.id, expected);
        assert_eq!(
            phase,
            if expected == 1024 {
                RuntimeComputePipelinePhaseV1::PhysicallyRetired
            } else {
                RuntimeComputePipelinePhaseV1::Published
            }
        );
    }
    assert!(pipeline.is_empty());
    assert_eq!(
        account
            .usage()
            .used
            .get(ResourceKindV1::ControlResidentBytes),
        payload()
    );
    let replacement = pipeline.insert_published(active(1025)).unwrap();
    assert_eq!(replacement.slot, stale.slot);
    assert_eq!(replacement.slot_generation, stale.slot_generation + 1);
    let (fresh, owner) = pipeline.take_physical_owner(1025).unwrap();
    assert_eq!(fresh, replacement);
    assert!(
        pipeline
            .restore(stale, RuntimeComputePipelinePhaseV1::Published, active(2))
            .is_err()
    );
    pipeline
        .restore(fresh, RuntimeComputePipelinePhaseV1::Published, owner)
        .unwrap();
    assert_eq!(pipeline.take_commit_frontier().unwrap().1.id, 1025);
    drop(pipeline);
    assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
}

#[test]
fn both_scaled_lanes_share_payload_debits_through_owner_swaps_and_disposal() {
    let account = account(2 * payload());
    let mut primary = scaled(&account);
    let mut auxiliary = scaled(&account);
    primary.insert_published(active(2)).unwrap();
    auxiliary.insert_published(active(3)).unwrap();
    let usage = account.usage();
    assert_eq!(
        usage.used.get(ResourceKindV1::ControlResidentBytes),
        2 * payload()
    );
    assert_eq!(usage.retained_records, 2);
    assert!(matches!(
        RuntimeComputePipelineV1::try_vacant(
            Gfx942FixedDispatchCapacityProfileV1::Qualification1024,
            Some(&account),
        ),
        Err(ResourceCreditErrorV1::Capacity)
    ));
    core::mem::swap(&mut primary, &mut auxiliary);
    assert_eq!(primary.take_commit_frontier().unwrap().1.id, 3);
    assert_eq!(auxiliary.take_commit_frontier().unwrap().1.id, 2);
    assert_eq!(account.usage(), usage);
    drop(primary);
    assert_eq!(
        account
            .usage()
            .used
            .get(ResourceKindV1::ControlResidentBytes),
        payload()
    );
    drop(auxiliary);
    assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
}
