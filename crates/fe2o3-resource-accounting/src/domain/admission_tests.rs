use super::retirement_tests::{bytes, chain, domain, snapshot};
use super::*;

#[test]
fn domain_planner_competing_failures_preserve_exact_state_and_error_order() {
    use ResourceCreditErrorV1 as E;
    for mode in 0..12 {
        let chain = chain(4);
        let root = domain(&chain[0]).root.clone();
        let leaf = domain(&chain[3]);
        let parent = domain(&chain[2]).key;
        let distant = domain(&chain[1]).key;
        let count = if mode == 0 { 9 } else { 2 };
        let charges = vec![bytes(1); count];
        let mut output = (0..count)
            .map(|_| ResourceReservationV1 { token: None })
            .collect::<Vec<_>>();
        if mode == 11 {
            output[0] = chain[3].reserve(bytes(1)).unwrap();
        }
        let expected = {
            let mut state = root.lock();
            match mode {
                0 => {
                    state.next_owner = 0;
                    E::RecordCapacity
                }
                1 => {
                    state.next_owner = 0;
                    state.node_mut(leaf.key).unwrap().counts = [usize::MAX, 1, 0];
                    E::Invariant
                }
                2 => {
                    state.next_owner = u64::MAX;
                    state.node_mut(leaf.key).unwrap().counts = [usize::MAX, 1, 0];
                    E::Invariant
                }
                3 => {
                    state.next_owner = u64::MAX;
                    let node = state.node_mut(leaf.key).unwrap();
                    node.record_limit = 1;
                    node.used = bytes(u64::MAX);
                    node.capacity = bytes(0);
                    E::RecordCapacity
                }
                4 => {
                    state.next_owner = u64::MAX;
                    state.node_mut(leaf.key).unwrap().capacity = bytes(0);
                    E::GenerationExhausted
                }
                5 => {
                    state.node_mut(leaf.key).unwrap().capacity = bytes(0);
                    state.node_mut(parent).unwrap().counts = [usize::MAX, 1, 0];
                    E::Capacity
                }
                6 => {
                    let node = state.node_mut(parent).unwrap();
                    node.counts = [usize::MAX, 1, 0];
                    node.capacity = bytes(0);
                    E::Invariant
                }
                7 => {
                    let node = state.node_mut(parent).unwrap();
                    node.record_limit = 1;
                    node.capacity = bytes(0);
                    E::RecordCapacity
                }
                8 => {
                    state.node_mut(parent).unwrap().capacity = bytes(0);
                    state.node_mut(distant).unwrap().counts = [usize::MAX, 1, 0];
                    E::Capacity
                }
                9 => {
                    state.node_mut(ROOT).unwrap().counts[0] = 1;
                    state.node_mut(leaf.key).unwrap().capacity = bytes(0);
                    E::Invariant
                }
                10 => {
                    state.node_mut(leaf.key).unwrap().parent = Some(Key {
                        slot: parent.slot,
                        generation: u64::MAX,
                    });
                    E::Invariant
                }
                11 => {
                    state.node_mut(leaf.key).unwrap().capacity = bytes(0);
                    E::Invariant
                }
                _ => unreachable!(),
            }
        };
        let tokens = output
            .iter()
            .map(|r| r.token.as_ref().map(|t| (t.slot, t.owner)))
            .collect::<Vec<_>>();
        let before = snapshot(&root);
        assert_eq!(
            leaf.reserve_into(&charges, &mut output),
            Err(expected),
            "mode={mode}"
        );
        assert_eq!(snapshot(&root), before, "mode={mode}");
        assert_eq!(root.lock().poisoned, expected == E::Invariant);
        assert_eq!(
            output
                .iter()
                .map(|r| r.token.as_ref().map(|t| (t.slot, t.owner)))
                .collect::<Vec<_>>(),
            tokens
        );
        // Fault-injected CPU ledger only. No native resource or callback exists.
        drop(output);
        root.lock().poisoned = true;
        root.lock().quarantine_anchor = None;
    }
}

#[test]
fn domain_planner_commits_original_members_once_and_frames_siblings() {
    for depth in 1..=4 {
        let chain = chain(depth);
        let root = domain(&chain[0]).root.clone();
        let sibling = chain[0].new_child(bytes(100), 8).unwrap();
        let sibling_before = sibling.usage();
        let baseline = chain.iter().map(|a| a.usage()).collect::<Vec<_>>();
        let leaf = domain(chain.last().unwrap());
        let mut output = core::array::from_fn::<_, 3, _>(|_| ResourceReservationV1 { token: None });
        let charges = [bytes(2), bytes(3), bytes(5)];
        leaf.reserve_into(&charges, &mut output).unwrap();
        {
            let state = root.lock();
            assert_eq!(state.next_owner, 4);
            assert_eq!(state.free_records, vec![7, 6, 5, 4, 3]);
            for (i, &charge) in charges.iter().enumerate() {
                let token = output[i].token.as_ref().unwrap();
                assert_eq!((token.slot, token.owner), (i, i as u64 + 1));
                let record = state.records[i].unwrap();
                assert_eq!(record.leaf, leaf.key);
                assert_eq!(record.credit.charge, charge);
                assert_eq!(record.credit.phase, Phase::Reserved);
            }
        }
        for (account, before) in chain.iter().zip(&baseline) {
            assert_eq!(
                account.usage().used,
                r67_resource_reserve_v1(before.used, bytes(10), before.capacity).unwrap()
            );
            assert_eq!(account.usage().reserved_records, 3);
        }
        assert_eq!(sibling.usage(), sibling_before);
        let [a, b, c] = output;
        let a = a.retain();
        let b = b.retain();
        let c = c.retain();
        b.release_after_disposal().unwrap();
        c.release_after_disposal().unwrap();
        for account in &chain {
            assert_eq!(
                account
                    .usage()
                    .used
                    .get(ResourceKindV1::RequestedAllocationBytes),
                2
            );
        }
        a.release_after_disposal().unwrap();
        for (account, before) in chain.iter().zip(&baseline) {
            assert_eq!(account.usage(), *before);
        }
        assert_eq!(sibling.usage(), sibling_before);
    }
}
