use super::*;
use ResourceKindV1 as K;

fn bytes(n: u64) -> ResourceVectorV1 {
    ResourceVectorV1::ZERO.with(K::RequestedAllocationBytes, n)
}

fn root(n: u64, domains: usize, records: usize) -> ResourceCreditAccountV1 {
    ResourceCreditAccountV1::new_root(
        bytes(n).with(
            K::ControlResidentBytes,
            resource_domain_bootstrap_bytes_v1(domains, records).unwrap(),
        ),
        domains,
        records,
    )
    .unwrap()
}

fn domain(account: &ResourceCreditAccountV1) -> &DomainAccount {
    let AccountHandle::Domain(domain) = &account.0 else {
        panic!("expected domain")
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
        state.next_owner,
        state.next_node,
        state.free_records.clone(),
        state.free_nodes.clone(),
    )
}

#[test]
fn domain_bootstrap_is_charged_before_admission_and_never_refunded_by_slot_reuse() {
    for (nodes, records) in [(0, 1), (1, 0), (usize::MAX, 1), (1, usize::MAX)] {
        assert!(resource_domain_bootstrap_bytes_v1(nodes, records).is_err());
    }
    let baseline = resource_domain_bootstrap_bytes_v1(3, 4).unwrap();
    assert!(matches!(
        ResourceCreditAccountV1::new_root(
            ResourceVectorV1::ZERO.with(K::ControlResidentBytes, baseline - 1),
            3,
            4
        ),
        Err(ResourceCreditErrorV1::Capacity)
    ));
    let root = root(8, 3, 4);
    let before = root.usage();
    assert_eq!(
        before.used,
        ResourceVectorV1::ZERO.with(K::ControlResidentBytes, baseline)
    );
    for _ in 0..8 {
        let child = root.new_child(bytes(8), 4).unwrap();
        let credit = child.reserve(bytes(8)).unwrap().retain();
        assert_eq!(root.usage().used.get(K::RequestedAllocationBytes), 8);
        credit.release_after_disposal().unwrap();
        drop(child);
        assert_eq!(root.usage(), before);
    }
}

#[test]
fn domain_siblings_share_ancestors_but_not_leaf_identity() {
    let root = root(10, 5, 4);
    let device = root.new_child(bytes(8), 4).unwrap();
    let a = device.new_child(bytes(8), 2).unwrap();
    let b = device.new_child(bytes(8), 2).unwrap();
    let other = root.new_child(bytes(10), 4).unwrap();
    assert!(a.shares_ledger_with(&a.clone()));
    assert!(!a.shares_ledger_with(&b));
    assert!(a.shares_root_with(&b));
    assert!(!a.shares_root_with(&ResourceCreditAccountV1::new(bytes(10), 4).unwrap()));
    let baseline = root.usage();
    let first = a.reserve(bytes(6)).unwrap().retain();
    let snapshots = [snapshot(&root), snapshot(&device), snapshot(&b)];
    assert!(matches!(
        b.reserve(bytes(3)),
        Err(ResourceCreditErrorV1::Capacity)
    ));
    assert_eq!(
        [snapshot(&root), snapshot(&device), snapshot(&b)],
        snapshots
    );
    let last = other.reserve(bytes(4)).unwrap();
    assert!(matches!(
        b.reserve(bytes(1)),
        Err(ResourceCreditErrorV1::Capacity)
    ));
    drop(last);
    first.release_after_disposal().unwrap();
    assert_eq!(root.usage(), baseline);
    assert_eq!(device.usage().used, ResourceVectorV1::ZERO);
}

#[test]
fn domain_batch_failure_is_atomic_at_each_ancestor_and_members_dispose_independently() {
    for limiting in 0..3 {
        let root = root(if limiting == 0 { 8 } else { 20 }, 3, 5);
        let parent = root
            .new_child(bytes(if limiting == 1 { 8 } else { 20 }), 5)
            .unwrap();
        let leaf = parent
            .new_child(bytes(if limiting == 2 { 8 } else { 20 }), 5)
            .unwrap();
        let before = [snapshot(&root), snapshot(&parent), snapshot(&leaf)];
        assert!(matches!(
            leaf.reserve_batch(&[bytes(4), bytes(5)]),
            Err(ResourceCreditErrorV1::Capacity)
        ));
        assert_eq!(
            [snapshot(&root), snapshot(&parent), snapshot(&leaf)],
            before
        );
        let mut members = leaf
            .reserve_batch(&[bytes(3), bytes(5)])
            .unwrap()
            .into_vec()
            .into_iter();
        let a = members.next().unwrap().retain();
        let b = members.next().unwrap().retain();
        a.release_after_disposal().unwrap();
        for account in [&root, &parent, &leaf] {
            assert_eq!(account.usage().used.get(K::RequestedAllocationBytes), 5);
            assert_eq!(account.usage().retained_records, 1);
        }
        b.release_after_rejection().unwrap();
        assert_eq!(root.usage(), before[0].0);
    }
}

#[test]
fn domain_checks_every_resource_coordinate_at_each_ancestor() {
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
        for limiting in 0..3 {
            let baseline = resource_domain_bootstrap_bytes_v1(3, 4).unwrap();
            let root_capacity =
                ResourceVectorV1::ZERO.with(kind, if limiting == 0 { 8 } else { 20 });
            let root_capacity = root_capacity.with(
                K::ControlResidentBytes,
                baseline + root_capacity.get(K::ControlResidentBytes),
            );
            let root = ResourceCreditAccountV1::new_root(root_capacity, 3, 4).unwrap();
            let parent = root
                .new_child(
                    ResourceVectorV1::ZERO.with(kind, if limiting == 1 { 8 } else { 20 }),
                    4,
                )
                .unwrap();
            let leaf = parent
                .new_child(
                    ResourceVectorV1::ZERO.with(kind, if limiting == 2 { 8 } else { 20 }),
                    4,
                )
                .unwrap();
            let before = [snapshot(&root), snapshot(&parent), snapshot(&leaf)];
            assert!(matches!(
                leaf.reserve_batch(&[
                    ResourceVectorV1::ZERO.with(kind, 4),
                    ResourceVectorV1::ZERO.with(kind, 5)
                ]),
                Err(ResourceCreditErrorV1::Capacity)
            ));
            assert_eq!(
                [snapshot(&root), snapshot(&parent), snapshot(&leaf)],
                before
            );
        }
    }
}

#[test]
fn domain_records_generations_and_depth_reject_before_mutation() {
    let root = root(100, 3, 4);
    let parent = root.new_child(bytes(100), 1).unwrap();
    let leaf = parent.new_child(bytes(100), 4).unwrap();
    let before = snapshot(&root);
    assert!(matches!(
        leaf.new_child(bytes(1), 1),
        Err(ResourceCreditErrorV1::DomainDepth)
    ));
    assert!(matches!(
        root.new_child(bytes(1), 1),
        Err(ResourceCreditErrorV1::DomainCapacity)
    ));
    assert!(matches!(
        leaf.reserve_batch(&[bytes(1), bytes(1)]),
        Err(ResourceCreditErrorV1::RecordCapacity)
    ));
    assert_eq!(snapshot(&root), before);
    domain(&root).root.lock().next_owner = u64::MAX;
    let before = snapshot(&root);
    assert!(matches!(
        leaf.reserve(bytes(1)),
        Err(ResourceCreditErrorV1::GenerationExhausted)
    ));
    assert_eq!(snapshot(&root), before);
    drop(leaf);
    domain(&root).root.lock().next_node = u64::MAX;
    let before = snapshot(&root);
    assert!(matches!(
        parent.new_child(bytes(1), 1),
        Err(ResourceCreditErrorV1::GenerationExhausted)
    ));
    assert_eq!(snapshot(&root), before);
}

#[test]
fn domain_tokens_keep_retired_parents_live_and_clean_reuse_changes_generation() {
    let root = root(8, 3, 4);
    let parent = root.new_child(bytes(8), 4).unwrap();
    let leaf = parent.new_child(bytes(8), 4).unwrap();
    let old_key = domain(&parent).key;
    let credit = leaf.reserve(bytes(8)).unwrap().retain();
    drop(parent);
    drop(leaf);
    assert!(matches!(
        root.new_child(bytes(8), 4),
        Err(ResourceCreditErrorV1::DomainCapacity)
    ));
    credit.release_after_disposal().unwrap();
    let replacement = root.new_child(bytes(8), 4).unwrap();
    assert_eq!(domain(&replacement).key.slot, old_key.slot);
    assert_ne!(domain(&replacement).key.generation, old_key.generation);
    assert!(domain(&root).root.lock().node(old_key).is_err());
}

#[test]
fn domain_quarantine_survives_all_external_handles_and_cannot_reuse_its_node() {
    let root = root(8, 2, 1);
    let leaf = root.new_child(bytes(8), 1).unwrap();
    let weak = Arc::downgrade(&domain(&root).root);
    let retained = leaf.reserve(bytes(8)).unwrap().retain();
    drop(leaf);
    drop(retained);
    assert_eq!(root.usage().quarantined_records, 1);
    assert_eq!(root.usage().used.get(K::RequestedAllocationBytes), 8);
    assert!(matches!(
        root.reserve(bytes(1)),
        Err(ResourceCreditErrorV1::RecordCapacity)
    ));
    assert!(matches!(
        root.new_child(bytes(8), 1),
        Err(ResourceCreditErrorV1::DomainCapacity)
    ));
    drop(root);
    let anchored = weak.upgrade().expect("root survives quarantined owners");
    assert_eq!(anchored.lock().usage(ROOT).quarantined_records, 1);
    // CPU-only credit fixture: no resource or native custody was created.
    anchored.lock().quarantine_anchor = None;
}

#[test]
fn domain_concurrent_siblings_cannot_overbook_the_last_parent_credit() {
    let root = root(8, 3, 2);
    let a = root.new_child(bytes(8), 2).unwrap();
    let b = root.new_child(bytes(8), 2).unwrap();
    let barrier = std::sync::Barrier::new(2);
    let wins = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for account in [&a, &b] {
            scope.spawn(|| {
                let reservation = account.reserve(bytes(8));
                if reservation.is_ok() {
                    wins.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
                barrier.wait();
                assert_eq!(root.usage().used.get(K::RequestedAllocationBytes), 8);
                barrier.wait();
                drop(reservation);
            });
        }
    });
    assert_eq!(wins.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(root.usage().used.get(K::RequestedAllocationBytes), 0);
}

#[test]
fn domain_wrong_leaf_and_stale_owner_poison_without_refund() {
    for stale_owner in [false, true] {
        let root = root(8, 3, 2);
        let a = root.new_child(bytes(8), 2).unwrap();
        let b = root.new_child(bytes(8), 2).unwrap();
        let retained = a.reserve(bytes(8)).unwrap().retain();
        let token = retained.token.as_ref().unwrap();
        let before = root.usage();
        let result = if stale_owner {
            a.transition(token.slot, token.owner + 1, Action::ReleaseDisposed)
        } else {
            b.transition(token.slot, token.owner, Action::ReleaseDisposed)
        };
        assert_eq!(result, Err(ResourceCreditErrorV1::Invariant));
        assert_eq!(root.usage().used, before.used);
        assert!(a.usage().poisoned && b.usage().poisoned);
        drop(retained);
        // CPU-only fixture: release its intentional fail-closed anchor.
        domain(&root).root.lock().quarantine_anchor = None;
    }
}

#[test]
fn domain_corrupt_child_preflight_poisons_without_issuing_a_domain() {
    for corrupt_path in [false, true] {
        let root = root(8, 4, 2);
        let parent = root.new_child(bytes(8), 2).unwrap();
        let mut state = domain(&root).root.lock();
        let node = state.node_mut(domain(&parent).key).unwrap();
        if corrupt_path {
            node.parent = Some(node.key);
        } else {
            node.children = usize::MAX;
        }
        drop(state);
        let before = snapshot(&root);
        assert!(matches!(
            parent.new_child(bytes(8), 2),
            Err(ResourceCreditErrorV1::Invariant)
        ));
        let after = snapshot(&root);
        assert!(after.0.poisoned);
        assert_eq!(after.0.used, before.0.used);
        assert_eq!(
            (after.1, after.2, after.3, after.4),
            (before.1, before.2, before.3, before.4)
        );
        assert!(matches!(
            root.reserve(bytes(1)),
            Err(ResourceCreditErrorV1::Invariant)
        ));
        domain(&root).root.lock().quarantine_anchor = None;
    }
}

#[test]
fn domain_foreign_roots_with_equal_slots_owners_and_limits_remain_independent() {
    let a = root(8, 2, 2);
    let b = root(8, 2, 2);
    let leaf_a = a.new_child(bytes(8), 2).unwrap();
    let leaf_b = b.new_child(bytes(8), 2).unwrap();
    assert_eq!(domain(&leaf_a).key, domain(&leaf_b).key);
    assert!(!leaf_a.shares_root_with(&leaf_b));
    assert!(!leaf_a.shares_ledger_with(&leaf_b));
    let ta = leaf_a.reserve(bytes(8)).unwrap().retain();
    let tb = leaf_b.reserve(bytes(8)).unwrap().retain();
    assert_eq!(
        ta.token.as_ref().unwrap().slot,
        tb.token.as_ref().unwrap().slot
    );
    assert_eq!(
        ta.token.as_ref().unwrap().owner,
        tb.token.as_ref().unwrap().owner
    );
    let before = b.usage();
    ta.release_after_disposal().unwrap();
    assert_eq!(b.usage(), before);
    assert_eq!(a.usage().used.get(K::RequestedAllocationBytes), 0);
    tb.release_after_disposal().unwrap();
}

#[test]
fn domain_token_alone_preserves_root_and_final_disposal_releases_it() {
    let root = root(8, 2, 1);
    let leaf = root.new_child(bytes(8), 1).unwrap();
    let weak = Arc::downgrade(&domain(&root).root);
    let token = leaf.reserve(bytes(8)).unwrap().retain();
    drop(leaf);
    drop(root);
    assert_eq!(
        weak.upgrade().unwrap().lock().usage(ROOT).retained_records,
        1
    );
    token.release_after_disposal().unwrap();
    assert!(weak.upgrade().is_none());
}

#[test]
fn domain_corrupt_global_record_totals_poison_before_any_new_member() {
    let root = root(8, 2, 2);
    let leaf = root.new_child(bytes(8), 2).unwrap();
    domain(&root).root.lock().node_mut(ROOT).unwrap().counts[0] = 1;
    let before = snapshot(&root);
    assert!(matches!(
        leaf.reserve_batch(&[bytes(1)]),
        Err(ResourceCreditErrorV1::Invariant)
    ));
    let after = snapshot(&root);
    assert!(after.0.poisoned);
    assert_eq!(after.0.used, before.0.used);
    assert_eq!((after.1, after.2, after.3), (before.1, before.2, before.3));
    domain(&root).root.lock().quarantine_anchor = None;
}

#[test]
fn domain_rooted_host_tables_keep_baseline_and_exact_leaf_identity() {
    let baseline = resource_domain_bootstrap_bytes_v1(3, 2).unwrap();
    let root = ResourceCreditAccountV1::new_root(
        ResourceVectorV1::ZERO.with(K::ControlResidentBytes, baseline + 16),
        3,
        2,
    )
    .unwrap();
    let a = root
        .new_child(ResourceVectorV1::ZERO.with(K::ControlResidentBytes, 16), 2)
        .unwrap();
    let b = root
        .new_child(ResourceVectorV1::ZERO.with(K::ControlResidentBytes, 16), 2)
        .unwrap();
    let table_a = HostMetadataTableV1::try_new(1, Some(&a), || 7u64).unwrap();
    let table_b = HostMetadataTableV1::try_new(1, Some(&b), || 7u64).unwrap();
    assert_ne!(table_a, table_b);
    assert!(table_a.try_clone().is_err());
    drop(table_b);
    let clone = table_a.try_clone().unwrap();
    assert_eq!(table_a, clone);
    drop(a);
    drop(b);
    drop(table_a);
    assert_eq!(root.usage().used.get(K::ControlResidentBytes), baseline + 8);
    drop(clone);
    assert_eq!(root.usage().used.get(K::ControlResidentBytes), baseline);
}

#[test]
fn domain_recovered_table_account_outlives_payload_and_preserves_poisoned_clone_behavior() {
    assert!(core::mem::size_of::<HostMetadataTableV1<u64>>() <= 64);
    let baseline = resource_domain_bootstrap_bytes_v1(2, 2).unwrap();
    let capacity = ResourceVectorV1::ZERO.with(K::ControlResidentBytes, 16);
    let root = ResourceCreditAccountV1::new_root(
        capacity.with(K::ControlResidentBytes, baseline + 16),
        2,
        2,
    )
    .unwrap();
    let leaf = root.new_child(capacity, 2).unwrap();
    let table = HostMetadataTableV1::try_new(1, Some(&leaf), || 7u64).unwrap();
    let recovered = table.account().unwrap();
    assert!(recovered.shares_ledger_with(&leaf));
    drop(leaf);
    drop(table);
    assert!(matches!(
        root.new_child(capacity, 2),
        Err(ResourceCreditErrorV1::DomainCapacity)
    ));
    let table = HostMetadataTableV1::try_new(1, Some(&recovered), || 9u64).unwrap();
    let root_inner = &domain(&root).root;
    root_inner.poison(&mut root_inner.lock());
    assert!(matches!(
        table.try_clone(),
        Err(ResourceCreditErrorV1::Invariant)
    ));
    assert!(table.account().unwrap().shares_ledger_with(&recovered));
    drop(table);
    root_inner.lock().quarantine_anchor = None;
}

#[test]
fn domain_two_token_only_records_prevent_reap_until_last_disposal() {
    let root = root(8, 2, 2);
    let leaf = root.new_child(bytes(8), 2).unwrap();
    let a = leaf.reserve(bytes(3)).unwrap().retain();
    let b = leaf.reserve(bytes(5)).unwrap().retain();
    drop(leaf);
    a.release_after_disposal().unwrap();
    assert!(matches!(
        root.new_child(bytes(8), 2),
        Err(ResourceCreditErrorV1::DomainCapacity)
    ));
    assert_eq!(root.usage().used.get(K::RequestedAllocationBytes), 5);
    b.release_after_disposal().unwrap();
    assert!(root.new_child(bytes(8), 2).is_ok());
}
