use super::super::tests::pipelined_active_for_test_v1 as active;
use super::*;
use fe2o3_resource_accounting::{
    ResourceKindV1, ResourceVectorV1, host_metadata_table_payload_bytes_v1,
};

fn payload() -> u64 {
    host_metadata_table_payload_bytes_v1::<RuntimeComputePipelineSlotV1>(1024).unwrap()
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
