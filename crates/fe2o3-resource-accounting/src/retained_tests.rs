use super::*;
use ResourceKindV1 as K;

pub(crate) const KINDS: [K; 19] = [
    K::LogicalPayloadBytes,
    K::RequestedAllocationBytes,
    K::ResidentHostAllocationBytes,
    K::ResidentDeviceAllocationBytes,
    K::ExecutableHostImageBytes,
    K::ExecutableDeviceBytes,
    K::ControlResidentBytes,
    K::QueueResidentBytes,
    K::SignalResidentBytes,
    K::KernargResidentBytes,
    K::QueueSlots,
    K::SignalSlots,
    K::KernargSlots,
    K::OperationSlots,
    K::ReplyBytes,
    K::ReplyCells,
    K::TerminalRecordBytes,
    K::QuarantineBookkeepingBytes,
    K::AllocationRecords,
];

fn charge() -> ResourceVectorV1 {
    KINDS
        .iter()
        .enumerate()
        .fold(ResourceVectorV1::ZERO, |v, (i, &k)| v.with(k, i as u64 + 1))
}

fn account() -> ResourceCreditAccountV1 {
    ResourceCreditAccountV1::new(charge(), 2).unwrap()
}

fn snapshot(account: &ResourceCreditAccountV1) -> String {
    let state = account.independent().state.lock().unwrap();
    format!(
        "{:?}",
        (
            state.used,
            state
                .records
                .iter()
                .map(|r| r.map(|r| (r.owner, r.charge, r.phase)))
                .collect::<Vec<_>>(),
            &state.free,
            state.next_owner,
            state.reserved,
            state.retained,
            state.quarantined,
            state.poisoned,
            state.quarantine_anchor.is_some(),
            Arc::strong_count(account.independent()),
        )
    )
}

#[test]
fn retained_record_predicate_exact_owner_and_all_phases_without_mutation() {
    for actual in [0, 1, 2, u64::MAX] {
        for expected in [0, 1, 2, u64::MAX] {
            for phase in [
                Phase::Reserved,
                Phase::Retained,
                Phase::Quarantined,
                Phase::Vacant,
            ] {
                let record = Record {
                    owner: actual,
                    charge: charge(),
                    phase,
                };
                let before = (record.owner, record.charge, record.phase);
                assert_eq!(
                    record.matches_retained_charge(expected, record.charge),
                    expected != 0 && actual == expected && phase == Phase::Retained
                );
                assert_eq!((record.owner, record.charge, record.phase), before);
            }
        }
    }
}

#[test]
fn retained_record_predicate_checks_every_vector_coordinate_and_zero_charge() {
    assert_eq!(KINDS.len(), fe2o3_runtime_model::R67_RESOURCE_DIMENSIONS_V1);
    for charge in [
        ResourceVectorV1::ZERO,
        charge(),
        KINDS.iter().fold(ResourceVectorV1::ZERO, |vector, &kind| {
            vector.with(kind, u64::MAX)
        }),
    ] {
        let record = Record {
            owner: u64::MAX,
            charge,
            phase: Phase::Retained,
        };
        assert!(record.matches_retained_charge(u64::MAX, charge));
        for kind in KINDS {
            let changed = charge.with(kind, charge.get(kind) ^ 1);
            assert!(!record.matches_retained_charge(u64::MAX, changed));
            let changed_record = Record {
                charge: changed,
                ..record
            };
            assert!(!changed_record.matches_retained_charge(u64::MAX, charge));
            assert_eq!(record.charge, charge);
        }
    }
}

#[test]
fn independent_retained_observation_exact_raw_state_and_unchanged_records() {
    for actual_owner in [0, 1, 2, u64::MAX] {
        for owner in [0, 1, 2, u64::MAX] {
            for phase in [
                Phase::Reserved,
                Phase::Retained,
                Phase::Quarantined,
                Phase::Vacant,
            ] {
                let records = [
                    None,
                    Some(Record {
                        owner: actual_owner,
                        charge: charge(),
                        phase,
                    }),
                    None,
                ];
                let before = records.map(|record| record.map(|r| (r.owner, r.charge, r.phase)));
                for poisoned in [false, true] {
                    for slot in [0, 1, 2, 3, usize::MAX] {
                        assert_eq!(
                            independent_retained_observation_v1(
                                &records,
                                poisoned,
                                slot,
                                owner,
                                charge()
                            ),
                            !poisoned
                                && slot == 1
                                && owner != 0
                                && owner == actual_owner
                                && phase == Phase::Retained,
                        );
                        assert_eq!(
                            records.map(|record| record.map(|r| (r.owner, r.charge, r.phase))),
                            before
                        );
                    }
                    assert!(!independent_retained_observation_v1(
                        &[],
                        poisoned,
                        0,
                        owner,
                        charge()
                    ));
                    assert!(!independent_retained_observation_v1(
                        &[],
                        poisoned,
                        usize::MAX,
                        owner,
                        charge()
                    ));
                }
            }
        }
    }
}

#[test]
fn independent_retained_observation_selects_exact_record_and_complete_vector() {
    for expected in [
        ResourceVectorV1::ZERO,
        charge(),
        KINDS
            .iter()
            .fold(ResourceVectorV1::ZERO, |v, &k| v.with(k, u64::MAX)),
    ] {
        let records = [
            Some(Record {
                owner: 3,
                charge: expected,
                phase: Phase::Retained,
            }),
            Some(Record {
                owner: 7,
                charge: expected,
                phase: Phase::Retained,
            }),
            Some(Record {
                owner: 7,
                charge: expected,
                phase: Phase::Quarantined,
            }),
        ];
        assert!(independent_retained_observation_v1(
            &records, false, 1, 7, expected
        ));
        assert!(!independent_retained_observation_v1(
            &records, false, 0, 7, expected
        ));
        assert!(!independent_retained_observation_v1(
            &records, false, 2, 7, expected
        ));
        for kind in KINDS {
            let changed = expected.with(kind, expected.get(kind) ^ 1);
            assert!(!independent_retained_observation_v1(
                &records, false, 1, 7, changed
            ));
            let mut changed_records = records;
            changed_records[1].as_mut().unwrap().charge = changed;
            assert!(!independent_retained_observation_v1(
                &changed_records,
                false,
                1,
                7,
                expected
            ));
        }
        assert_eq!(records[1].unwrap().charge, expected);
    }
}

#[test]
fn independent_retained_observation_rechecks_locked_fields_without_touching_token() {
    let a = account();
    let credits = a.reserve(charge()).unwrap().retain();
    let token = credits.token.as_ref().unwrap();
    let token_address = token as *const _;
    let (slot, owner) = (token.slot, token.owner);
    let original = a.independent().state.lock().unwrap().records[slot];
    for mode in 0..5 {
        {
            let mut state = a.independent().state.lock().unwrap();
            state.poisoned = mode == 1;
            state.records[slot] = if mode == 2 { None } else { original };
            if mode == 3 {
                state.records[slot].as_mut().unwrap().phase = Phase::Quarantined;
            }
        }
        let before = snapshot(&a);
        for _ in 0..4 {
            assert_eq!(
                a.matches_retained_charge_v1(&credits, charge()),
                mode == 0 || mode == 4
            );
            let observed = credits.token.as_ref().unwrap();
            assert_eq!(observed as *const _, token_address);
            assert_eq!((observed.slot, observed.owner), (slot, owner));
        }
        assert_eq!(snapshot(&a), before);
    }
    credits.release_after_disposal().unwrap();
}

#[test]
fn retained_charge_exact_independent_account_vector_and_unchanged_queries() {
    let a = account();
    let cloned = a.clone();
    let b = account();
    let credits = a.reserve(charge()).unwrap().retain();
    let foreign = b.reserve(charge()).unwrap().retain();
    assert_eq!(
        credits.token.as_ref().unwrap().slot,
        foreign.token.as_ref().unwrap().slot
    );
    assert_eq!(
        credits.token.as_ref().unwrap().owner,
        foreign.token.as_ref().unwrap().owner
    );
    let before = snapshot(&a);
    for _ in 0..16 {
        assert!(a.matches_retained_charge_v1(&credits, charge()));
        assert!(cloned.matches_retained_charge_v1(&credits, charge()));
        assert!(!b.matches_retained_charge_v1(&credits, charge()));
        assert!(!a.matches_retained_charge_v1(&foreign, charge()));
        for kind in KINDS {
            assert!(
                !a.matches_retained_charge_v1(
                    &credits,
                    charge().with(kind, charge().get(kind) - 1)
                )
            );
            assert!(
                !a.matches_retained_charge_v1(
                    &credits,
                    charge().with(kind, charge().get(kind) + 1)
                )
            );
        }
    }
    assert_eq!(snapshot(&a), before);
    credits.release_after_disposal().unwrap();
    foreign.release_after_disposal().unwrap();
    assert_eq!(a.usage().used, ResourceVectorV1::ZERO);
}

#[test]
fn retained_charge_rejects_invalid_token_slot_owner_phase_and_empty_record() {
    for mode in 0..10 {
        let a = account();
        let mut credits = a.reserve(charge()).unwrap().retain();
        let token = credits.token.as_mut().unwrap();
        let (slot, owner) = (token.slot, token.owner);
        let original = a.independent().state.lock().unwrap().records[slot];
        match mode {
            0 => token.slot = usize::MAX,
            1 => token.slot = 1,
            2 => token.owner = 0,
            3 => token.owner += 1,
            4 => a.independent().state.lock().unwrap().records[slot] = None,
            5..=7 => {
                a.independent().state.lock().unwrap().records[slot]
                    .as_mut()
                    .unwrap()
                    .phase = [Phase::Reserved, Phase::Vacant, Phase::Quarantined][mode - 5]
            }
            8 => a.independent().state.lock().unwrap().poisoned = true,
            9 => {
                token.owner = 0;
                a.independent().state.lock().unwrap().records[slot]
                    .as_mut()
                    .unwrap()
                    .owner = 0;
            }
            _ => unreachable!(),
        }
        let before = snapshot(&a);
        assert!(
            !a.matches_retained_charge_v1(&credits, charge()),
            "mode={mode}"
        );
        assert_eq!(snapshot(&a), before);
        let token = credits.token.as_mut().unwrap();
        token.slot = slot;
        token.owner = owner;
        {
            let mut state = a.independent().state.lock().unwrap();
            state.records[slot] = original;
            state.poisoned = false;
        }
        let token = credits.token.take();
        assert!(!a.matches_retained_charge_v1(&credits, charge()));
        credits.token = token;
        assert!(a.matches_retained_charge_v1(&credits, charge()));
        credits.release_after_disposal().unwrap();
    }
}

#[test]
fn retained_charge_independent_mutex_poison_is_not_recovered_or_anchored() {
    let a = account();
    let credits = a.reserve(charge()).unwrap().retain();
    let before = snapshot(&a);
    assert!(
        std::panic::catch_unwind(|| {
            let _guard = a.independent().state.lock().unwrap();
            panic!("poison CPU fixture");
        })
        .is_err()
    );
    assert!(!a.matches_retained_charge_v1(&credits, charge()));
    match a.independent().state.lock() {
        Err(error) => {
            let state = error.into_inner();
            assert!(!state.poisoned);
            assert!(state.quarantine_anchor.is_none());
        }
        Ok(_) => panic!("query cleared mutex poison"),
    }
    a.independent().state.clear_poison();
    assert_eq!(snapshot(&a), before);
    credits.release_after_disposal().unwrap();
}
