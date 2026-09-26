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
