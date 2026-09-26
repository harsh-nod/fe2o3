use super::*;
use ResourceKindV1 as K;

fn capacity(bytes: u64, records: u64) -> ResourceVectorV1 {
    ResourceVectorV1::ZERO
        .with(K::ControlResidentBytes, 1 << 20)
        .with(K::ResidentHostAllocationBytes, bytes)
        .with(K::AllocationRecords, records)
}

fn charge(bytes: u64) -> ResourceVectorV1 {
    ResourceVectorV1::ZERO
        .with(K::ResidentHostAllocationBytes, bytes)
        .with(K::AllocationRecords, 1)
}

fn domain(account: &ResourceCreditAccountV1) -> &DomainAccount {
    let AccountHandle::Domain(domain) = &account.0 else {
        panic!("domain")
    };
    domain
}

fn snapshot(
    account: &ResourceCreditAccountV1,
) -> (ResourceCreditUsageV1, u64, u64, Vec<usize>, Vec<usize>) {
    let domain = domain(account);
    let state = domain.root.lock();
    (
        state.usage(domain.key),
        state.next_node,
        state.next_owner,
        state.free_nodes.clone(),
        state.free_records.clone(),
    )
}

#[test]
fn class_domains_preserve_default_depth_and_bound_explicit_profile() {
    for classes in [false, true] {
        let root = if classes {
            ResourceCreditAccountV1::new_root_with_class_domains_v1(capacity(20, 8), 8, 8)
        } else {
            ResourceCreditAccountV1::new_root(capacity(20, 8), 8, 8)
        }
        .unwrap();
        let device = root.new_child(capacity(20, 8), 8).unwrap();
        let session = device.new_child(capacity(20, 8), 8).unwrap();
        let leaf = if classes {
            session.new_child(capacity(20, 8), 8).unwrap()
        } else {
            session.clone()
        };
        let before = snapshot(&root);
        assert!(matches!(
            leaf.new_child(capacity(20, 8), 8),
            Err(ResourceCreditErrorV1::DomainDepth)
        ));
        assert_eq!(snapshot(&root), before);
        assert!(!root.usage().poisoned);
    }
}

#[test]
fn class_domains_scalar_and_batch_rejections_are_atomic_at_all_four_ancestors() {
    for limiting in 0..4 {
        for records in [false, true] {
            for batch in [false, true] {
                let cap = |level| {
                    capacity(
                        if !records && level == limiting {
                            5
                        } else {
                            100
                        },
                        if records && limiting == 0 && level == 0 {
                            1
                        } else {
                            100
                        },
                    )
                };
                let root =
                    ResourceCreditAccountV1::new_root_with_class_domains_v1(cap(0), 8, 8).unwrap();
                let mut chain = vec![root];
                for level in 1..4 {
                    let child = chain
                        .last()
                        .unwrap()
                        .new_child(cap(level), if records && level == limiting { 1 } else { 8 })
                        .unwrap();
                    chain.push(child);
                }
                let first = records.then(|| chain[3].reserve(charge(1)).unwrap().retain());
                let before = chain.iter().map(snapshot).collect::<Vec<_>>();
                let expected = if records && limiting != 0 {
                    ResourceCreditErrorV1::RecordCapacity
                } else {
                    ResourceCreditErrorV1::Capacity
                };
                let result = if batch {
                    chain[3].reserve_batch(&[charge(3), charge(3)]).map(drop)
                } else {
                    chain[3].reserve(charge(6)).map(drop)
                };
                assert_eq!(result, Err(expected));
                assert_eq!(chain.iter().map(snapshot).collect::<Vec<_>>(), before);
                if let Some(first) = first {
                    first.release_after_disposal().unwrap();
                }
            }
        }
    }
}

#[test]
fn class_domains_check_every_resource_coordinate_at_all_four_ancestors() {
    let kinds = [
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
    for kind in kinds {
        for limiting in 0..4 {
            let baseline = resource_domain_bootstrap_bytes_v1(4, 4).unwrap();
            let capacity = |level| {
                let capacity =
                    ResourceVectorV1::ZERO.with(kind, if level == limiting { 8 } else { 20 });
                if level == 0 {
                    capacity.with(
                        K::ControlResidentBytes,
                        baseline + capacity.get(K::ControlResidentBytes),
                    )
                } else {
                    capacity
                }
            };
            let root =
                ResourceCreditAccountV1::new_root_with_class_domains_v1(capacity(0), 4, 4).unwrap();
            let mut chain = vec![root];
            for level in 1..4 {
                chain.push(chain.last().unwrap().new_child(capacity(level), 4).unwrap());
            }
            let before = chain.iter().map(snapshot).collect::<Vec<_>>();
            assert!(matches!(
                chain[3].reserve_batch(&[
                    ResourceVectorV1::ZERO.with(kind, 4),
                    ResourceVectorV1::ZERO.with(kind, 5),
                ]),
                Err(ResourceCreditErrorV1::Capacity)
            ));
            assert_eq!(chain.iter().map(snapshot).collect::<Vec<_>>(), before);
        }
    }
}

#[test]
fn class_domains_owner_and_node_generations_never_wrap_or_partially_debit() {
    let root =
        ResourceCreditAccountV1::new_root_with_class_domains_v1(capacity(20, 8), 4, 8).unwrap();
    let device = root.new_child(capacity(20, 8), 8).unwrap();
    let session = device.new_child(capacity(20, 8), 8).unwrap();
    let leaf = session.new_child(capacity(20, 8), 8).unwrap();
    for (generation, batch) in [(u64::MAX, false), (u64::MAX - 1, true)] {
        domain(&root).root.lock().next_owner = generation;
        let accounts = [&root, &device, &session, &leaf];
        let before = accounts.map(snapshot);
        let result = if batch {
            leaf.reserve_batch(&[charge(1), charge(1)]).map(drop)
        } else {
            leaf.reserve(charge(1)).map(drop)
        };
        assert_eq!(result, Err(ResourceCreditErrorV1::GenerationExhausted));
        assert_eq!(accounts.map(snapshot), before);
    }
    drop(leaf);
    domain(&root).root.lock().next_node = u64::MAX;
    let accounts = [&root, &device, &session];
    let before = accounts.map(snapshot);
    assert!(matches!(
        session.new_child(capacity(20, 8), 8),
        Err(ResourceCreditErrorV1::GenerationExhausted)
    ));
    assert_eq!(accounts.map(snapshot), before);
}

#[test]
fn class_domains_batch_lifetime_reaps_all_ancestors_only_after_final_disposal() {
    let root =
        ResourceCreditAccountV1::new_root_with_class_domains_v1(capacity(20, 8), 8, 8).unwrap();
    let baseline = root.usage();
    let device = root.new_child(capacity(20, 8), 8).unwrap();
    let session = device.new_child(capacity(20, 8), 8).unwrap();
    let leaf = session.new_child(capacity(20, 8), 8).unwrap();
    let old_key = domain(&leaf).key;
    let members = leaf
        .reserve_batch(&[charge(3), charge(5)])
        .unwrap()
        .into_vec();
    let mut credits = members.into_iter().map(ResourceReservationV1::retain);
    let first = credits.next().unwrap();
    let second = credits.next().unwrap();
    drop(leaf);
    drop(session);
    drop(device);
    first.release_after_disposal().unwrap();
    assert_eq!(root.usage().used.get(K::ResidentHostAllocationBytes), 5);
    assert_eq!(domain(&root).root.lock().nodes.iter().flatten().count(), 4);
    second.release_after_disposal().unwrap();
    assert_eq!(root.usage(), baseline);
    assert_eq!(domain(&root).root.lock().nodes.iter().flatten().count(), 1);
    let replacement = root.new_child(capacity(20, 8), 8).unwrap();
    assert_ne!(domain(&replacement).key.generation, old_key.generation);
}

#[test]
fn class_domains_siblings_serialize_last_session_credit_and_preserve_quarantine() {
    let root =
        ResourceCreditAccountV1::new_root_with_class_domains_v1(capacity(20, 8), 8, 8).unwrap();
    let device = root.new_child(capacity(20, 8), 8).unwrap();
    let session = device.new_child(capacity(20, 1), 8).unwrap();
    let a = session.new_child(capacity(20, 8), 8).unwrap();
    let b = session.new_child(capacity(20, 8), 8).unwrap();
    assert!(a.shares_root_with(&b));
    assert!(!a.shares_ledger_with(&b));
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let workers = [a, b].map(|leaf| {
        let barrier = barrier.clone();
        std::thread::spawn(move || {
            barrier.wait();
            let result = leaf.reserve(charge(1));
            barrier.wait();
            if let Ok(reservation) = result {
                reservation.retain().quarantine();
                true
            } else {
                false
            }
        })
    });
    assert_eq!(
        workers
            .into_iter()
            .map(|worker| usize::from(worker.join().unwrap()))
            .sum::<usize>(),
        1
    );
    for account in [&root, &device, &session] {
        assert_eq!(account.usage().quarantined_records, 1);
        assert_eq!(account.usage().used.get(K::AllocationRecords), 1);
    }
}

#[test]
fn class_domains_corrupt_cycle_and_profile_overrun_poison_without_refund() {
    for cycle in [false, true] {
        let root =
            ResourceCreditAccountV1::new_root_with_class_domains_v1(capacity(20, 8), 8, 8).unwrap();
        let device = root.new_child(capacity(20, 8), 8).unwrap();
        let session = device.new_child(capacity(20, 8), 8).unwrap();
        let leaf = session.new_child(capacity(20, 8), 8).unwrap();
        let retained = leaf.reserve(charge(1)).unwrap().retain();
        let before = root.usage().used;
        {
            let mut state = domain(&root).root.lock();
            if cycle {
                state.node_mut(domain(&leaf).key).unwrap().parent = Some(domain(&leaf).key);
            } else {
                state.max_depth = MAX_RESOURCE_DOMAIN_DEPTH_V1;
            }
        }
        assert!(matches!(
            leaf.reserve(charge(1)),
            Err(ResourceCreditErrorV1::Invariant)
        ));
        assert!(root.usage().poisoned);
        drop(retained);
        assert_eq!(root.usage().used, before);
    }
}
