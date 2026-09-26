use super::retirement_tests::{bytes, chain, domain, snapshot};
use super::*;

#[test]
fn retained_charge_domain_requires_exact_leaf_root_and_all_coordinates() {
    for depth in [3, 4] {
        let chain = chain(depth);
        let leaf = chain.last().unwrap();
        let sibling = chain[depth - 2].new_child(bytes(100), 8).unwrap();
        let independent = ResourceCreditAccountV1::new(bytes(100), 8).unwrap();
        let other_chain = super::retirement_tests::chain(depth);
        let other = other_chain.last().unwrap();
        let cloned = leaf.clone();
        let credits = leaf.reserve(bytes(8)).unwrap().retain();
        let second = leaf.reserve(bytes(8)).unwrap().retain();
        let foreign = other.reserve(bytes(8)).unwrap().retain();
        assert_eq!(domain(leaf).key, domain(other).key);
        assert_eq!(
            credits.token.as_ref().unwrap().slot,
            foreign.token.as_ref().unwrap().slot
        );
        assert_eq!(
            credits.token.as_ref().unwrap().owner,
            foreign.token.as_ref().unwrap().owner
        );
        let independent_credit = independent.reserve(bytes(8)).unwrap().retain();
        let root = &domain(leaf).root;
        let before = snapshot(root);
        let references = Arc::strong_count(root);
        for _ in 0..16 {
            for credit in [&credits, &second] {
                assert!(leaf.matches_retained_charge_v1(credit, bytes(8)));
                assert!(cloned.matches_retained_charge_v1(credit, bytes(8)));
                for wrong in [&sibling, &chain[0], &chain[depth - 2], &independent, other] {
                    assert!(!wrong.matches_retained_charge_v1(credit, bytes(8)));
                }
                for kind in crate::retained_tests::KINDS {
                    assert!(!leaf.matches_retained_charge_v1(
                        credit,
                        bytes(8).with(kind, bytes(8).get(kind) + 1)
                    ));
                }
            }
        }
        assert_eq!(snapshot(root), before);
        assert_eq!(Arc::strong_count(root), references);
        assert!(!root.state.lock().unwrap().poisoned);
        assert!(root.state.lock().unwrap().quarantine_anchor.is_none());
        assert!(!leaf.matches_retained_charge_v1(&foreign, bytes(8)));
        assert!(!leaf.matches_retained_charge_v1(&independent_credit, bytes(8)));
        independent_credit.release_after_disposal().unwrap();
        foreign.release_after_disposal().unwrap();
        credits.release_after_disposal().unwrap();
        second.release_after_disposal().unwrap();
    }
}

#[test]
fn retained_charge_domain_rejects_record_and_token_corruption_without_effects() {
    for mode in 0..11 {
        let chain = chain(4);
        let leaf = chain.last().unwrap();
        let root = &domain(leaf).root;
        let mut credits = leaf.reserve(bytes(8)).unwrap().retain();
        let token = credits.token.as_mut().unwrap();
        let (slot, owner) = (token.slot, token.owner);
        let original = root.state.lock().unwrap().records[slot];
        match mode {
            0 => token.slot = usize::MAX,
            1 => token.slot = 7,
            2 => token.owner = 0,
            3 => token.owner += 1,
            4 => root.state.lock().unwrap().records[slot] = None,
            5..=7 => {
                root.state.lock().unwrap().records[slot]
                    .as_mut()
                    .unwrap()
                    .credit
                    .phase = [Phase::Reserved, Phase::Vacant, Phase::Quarantined][mode - 5]
            }
            8 => root.state.lock().unwrap().poisoned = true,
            10 => {
                token.owner = 0;
                root.state.lock().unwrap().records[slot]
                    .as_mut()
                    .unwrap()
                    .credit
                    .owner = 0;
            }
            9 => {
                root.state.lock().unwrap().records[slot]
                    .as_mut()
                    .unwrap()
                    .leaf = ROOT
            }
            _ => unreachable!(),
        }
        let before = snapshot(root);
        assert!(
            !leaf.matches_retained_charge_v1(&credits, bytes(8)),
            "mode={mode}"
        );
        assert_eq!(snapshot(root), before);
        assert_eq!(root.state.lock().unwrap().poisoned, mode == 8);
        assert!(root.state.lock().unwrap().quarantine_anchor.is_none());
        let token = credits.token.as_mut().unwrap();
        token.slot = slot;
        token.owner = owner;
        {
            let mut state = root.state.lock().unwrap();
            state.records[slot] = original;
            state.poisoned = false;
        }
        credits.release_after_disposal().unwrap();
    }
}

#[test]
fn retained_charge_domain_rejects_malformed_selected_ancestry_without_effects() {
    for mode in 0..8 {
        let chain = chain(4);
        let leaf = chain.last().unwrap();
        let root = &domain(leaf).root;
        let leaf_key = domain(leaf).key;
        let ancestor = domain(&chain[1]).key;
        let credits = leaf.reserve(bytes(8)).unwrap().retain();
        let saved = {
            let mut state = root.state.lock().unwrap();
            let key = if mode < 3 { leaf_key } else { ancestor };
            let saved = state.nodes[key.slot].take().unwrap();
            if mode != 2 {
                let mut node = Node { ..saved };
                match mode {
                    0 | 3 => node.key.generation += 1,
                    1 => node.key.slot = usize::MAX,
                    4 => node.parent = Some(leaf_key),
                    5 => node.parent = None,
                    6 => state.max_depth = 3,
                    7 => state.max_depth = 2,
                    _ => unreachable!(),
                }
                state.nodes[key.slot] = Some(node);
            }
            (key, saved)
        };
        let before = snapshot(root);
        assert!(
            !leaf.matches_retained_charge_v1(&credits, bytes(8)),
            "mode={mode}"
        );
        assert_eq!(snapshot(root), before);
        assert!(!root.state.lock().unwrap().poisoned);
        assert!(root.state.lock().unwrap().quarantine_anchor.is_none());
        {
            let mut state = root.state.lock().unwrap();
            state.nodes[saved.0.slot] = Some(saved.1);
            state.max_depth = 4;
        }
        credits.release_after_disposal().unwrap();
    }
}

#[test]
fn retained_charge_domain_mutex_poison_is_not_recovered_or_anchored() {
    let chain = chain(4);
    let leaf = chain.last().unwrap();
    let root = &domain(leaf).root;
    let credits = leaf.reserve(bytes(8)).unwrap().retain();
    let before = snapshot(root);
    assert!(
        std::panic::catch_unwind(|| {
            let _guard = root.state.lock().unwrap();
            panic!("poison CPU fixture");
        })
        .is_err()
    );
    assert!(!leaf.matches_retained_charge_v1(&credits, bytes(8)));
    match root.state.lock() {
        Err(error) => {
            let state = error.into_inner();
            assert!(!state.poisoned);
            assert!(state.quarantine_anchor.is_none());
        }
        Ok(_) => panic!("query cleared mutex poison"),
    }
    root.state.clear_poison();
    assert_eq!(snapshot(root), before);
    credits.release_after_disposal().unwrap();
}
