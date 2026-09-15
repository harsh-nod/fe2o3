use super::*;

fn budget() -> Budget {
    Budget::new(MAX_WORK)
}

fn reference(target: u32) -> Value {
    Value::Reference(Reference {
        target,
        mutable: false,
        borrow: Site {
            block: 1,
            statement: 2,
        },
    })
}

fn tree(target: u32) -> Value {
    let mut fields = Vec::with_capacity(9);
    fields.extend([Some(reference(target)), None, Some(Value::Opaque)]);
    let mut variants = Vec::with_capacity(4);
    variants.extend([Some(Value::Fields(vec![])), Some(Value::Fields(fields))]);
    Value::Variants {
        ty: SemanticTypeIdV1::from_index(7),
        possible: 3,
        fields: variants,
    }
}

fn exact(values: &Values) {
    assert_eq!(
        values.nodes(),
        values
            .entries
            .values()
            .map(|value| 1 + value.nodes())
            .sum::<usize>()
    );
}

fn exact_flow(flow: &Flow) {
    exact(&flow.values);
    assert_eq!(
        flow.nodes(),
        1 + flow.dead.len()
            + flow.escaped.len()
            + flow
                .values
                .values()
                .map(|value| 1 + value.nodes())
                .sum::<usize>()
    );
}

#[test]
fn value_map_cache_tracks_replacement_removal_and_missing_keys() {
    let mut values = Values::default();
    for (local, value) in [
        (3, tree(0)),
        (5, reference(1)),
        (3, Value::Opaque),
        (5, tree(2)),
    ] {
        values.insert_metered(local, value, &mut budget()).unwrap();
        exact(&values);
    }
    for local in [0, 3, 3, 5] {
        values.remove_metered(local, &mut budget()).unwrap();
        exact(&values);
    }
    assert_eq!(values.nodes(), 0);
}

#[test]
fn value_map_cache_recomputes_clone_capacity_without_affecting_fact_equality() {
    let mut values = Values::default();
    values.insert_metered(3, tree(0), &mut budget()).unwrap();
    let copied = values.clone();
    exact(&values);
    exact(&copied);
    assert_eq!(values, copied);
    assert!(copied.nodes() < values.nodes());
    let flow = Flow {
        values,
        ..Flow::default()
    };
    let cloned = flow.clone_retained(&mut budget()).unwrap();
    assert_eq!(flow, cloned);
    exact_flow(&cloned);
}

#[test]
fn value_map_invalidation_and_failed_invalidation_preserve_exact_size() {
    for limit in 0..30 {
        let mut values = Values::default();
        values.insert_metered(0, tree(7), &mut budget()).unwrap();
        values
            .insert_metered(1, reference(7), &mut budget())
            .unwrap();
        let before = values.nodes();
        let _ = values.invalidate(&BTreeSet::from([7]), &mut Budget::new(limit));
        exact(&values);
        assert_eq!(values.nodes(), before);
    }
}

#[test]
fn value_map_rejected_accounting_keeps_the_map_and_cache_consistent() {
    let mut values = Values::default();
    values.insert_metered(1, tree(7), &mut budget()).unwrap();
    let before = values.clone();
    let nodes = values.nodes();
    assert!(
        values
            .insert_metered(1, Value::Opaque, &mut Budget::new(0))
            .is_err()
    );
    assert_eq!(values, before);
    assert_eq!(values.nodes(), nodes);
    assert!(values.remove_metered(1, &mut Budget::new(0)).is_err());
    assert_eq!(values, before);
    exact(&values);
}

#[test]
fn value_map_cache_is_accounted_under_the_existing_storage_ceiling() {
    let flow = Flow::default();
    assert_eq!(flow.nodes(), 1);
    let mut budget = budget();
    let mut held = budget.reserve(MAX_STORAGE - 1).unwrap();
    assert!(flow.clone_retained(&mut budget).is_ok());
    held.resize(MAX_STORAGE).unwrap();
    assert!(flow.clone_retained(&mut budget).is_err());
}

#[test]
fn value_map_all_flow_mutations_and_conditional_joins_keep_exact_accounting() {
    let mut flow = Flow::default();
    flow.assign(0, Some(Value::Opaque), &mut budget()).unwrap();
    flow.assign(1, Some(tree(0)), &mut budget()).unwrap();
    exact_flow(&flow);
    for kind in 0..5 {
        let mut changed = flow.clone();
        match kind {
            0 => changed.kill(0, &mut budget()).unwrap(),
            1 => changed
                .assign(0, Some(Value::Opaque), &mut budget())
                .unwrap(),
            2 => changed.escape(1, &mut budget()).unwrap(),
            3 => changed.forget_references(&mut budget()).unwrap(),
            4 => {
                changed.dead.insert(0);
            }
            _ => unreachable!(),
        }
        exact_flow(&changed);
        let joined = flow.join(&changed, &mut budget()).unwrap();
        let reversed = changed.join(&flow, &mut budget()).unwrap();
        exact_flow(&joined);
        exact_flow(&reversed);
        assert_eq!(joined, reversed);
    }
}

#[test]
fn value_map_logical_key_debit_tracks_cardinality_not_std_fanout() {
    for (entries, expected) in [(0, 1), (1, 1), (2, 2), (3, 2), (4, 3), (7, 3), (8, 4)] {
        assert_eq!(key_work(entries), expected);
    }
    for bit in 1..usize::BITS {
        let power = 1usize << bit;
        assert_eq!(key_work(power - 1), bit as usize);
        assert_eq!(key_work(power), bit as usize + 1);
    }
    assert_eq!(key_work(usize::MAX), usize::BITS as usize);
}

fn keyed_values() -> Values {
    BTreeMap::from([(0, Value::Opaque), (1, tree(0)), (2, Value::Opaque)]).into()
}

#[test]
fn value_map_lookup_and_copy_charge_hit_and_miss_before_returning_values() {
    let flow = Flow {
        values: keyed_values(),
        ..Flow::default()
    };
    let key_debit = flow.values.key_work();
    for local in [0, 1, 9] {
        let value = flow.values.get(&local);
        let mut short = Budget::new(key_debit - 1);
        assert!(flow.values.get_metered(local, &mut short).is_err());
        assert!(short.work_exhausted);
        assert_eq!(short.remaining, key_debit - 1);
        let mut exact = Budget::new(key_debit);
        assert_eq!(flow.values.get_metered(local, &mut exact).unwrap(), value);
        assert_eq!(exact.remaining, 0);
        let debit = key_debit + 1 + 2 * value.map_or(0, Value::nodes);
        assert!(flow.copy_value(local, &mut Budget::new(debit - 1)).is_err());
        let mut exact = Budget::new(debit);
        assert_eq!(flow.copy_value(local, &mut exact).unwrap().as_ref(), value);
        assert_eq!(exact.remaining, 0);
        exact_flow(&flow);
    }
}

#[test]
fn value_map_insert_prepays_both_key_operations_and_subtree_accounting() {
    for local in [0, 1, 9] {
        let original = keyed_values();
        let incoming = tree(2);
        let tree_debit =
            1 + incoming.nodes() + original.get(&local).map_or(0, |value| 1 + value.nodes());
        let key_debit = key_work(original.len()) + key_work(original.len() + 1);
        let total = key_debit + tree_debit;
        for allowance in [0, key_debit - 1, key_debit, total - 1, total] {
            let mut values = keyed_values();
            let mut budget = Budget::new(allowance);
            let outcome = values.insert_metered(local, tree(2), &mut budget);
            assert_eq!(outcome.is_ok(), allowance == total);
            if allowance == total {
                assert_eq!(budget.remaining, 0);
                assert_eq!(values.get(&local), Some(&incoming));
            } else {
                assert!(budget.work_exhausted);
                assert_eq!(values, original);
                assert_eq!(values.nodes(), original.nodes());
            }
            exact(&values);
        }
    }
}

#[test]
fn value_map_remove_prepays_both_key_operations_on_hit_and_miss() {
    for local in [0, 1, 9] {
        let original = keyed_values();
        let key_debit = 2 * key_work(original.len());
        let total = key_debit + 1 + original.get(&local).map_or(0, |value| 1 + value.nodes());
        for allowance in [0, key_debit - 1, key_debit, total - 1, total] {
            let mut values = keyed_values();
            let mut budget = Budget::new(allowance);
            let outcome = values.remove_metered(local, &mut budget);
            assert_eq!(outcome.is_ok(), allowance == total);
            if allowance == total {
                assert_eq!(budget.remaining, 0);
                assert!(!values.contains_key(&local));
            } else {
                assert!(budget.work_exhausted);
                assert_eq!(values, original);
                assert_eq!(values.nodes(), original.nodes());
            }
            exact(&values);
        }
    }
}

#[test]
fn value_map_empty_and_first_key_operations_keep_exact_debits() {
    let mut values = Values::default();
    assert!(values.get_metered(0, &mut Budget::new(0)).is_err());
    let mut lookup = Budget::new(1);
    assert!(values.get_metered(0, &mut lookup).unwrap().is_none());
    assert_eq!(lookup.remaining, 0);
    assert!(values.remove_metered(0, &mut Budget::new(2)).is_err());
    let mut remove = Budget::new(3);
    values.remove_metered(0, &mut remove).unwrap();
    assert_eq!(remove.remaining, 0);
    assert!(
        values
            .insert_metered(0, Value::Opaque, &mut Budget::new(3))
            .is_err()
    );
    assert!(values.is_empty());
    let mut insert = Budget::new(4);
    values
        .insert_metered(0, Value::Opaque, &mut insert)
        .unwrap();
    assert_eq!(insert.remaining, 0);
    assert_eq!(values.nodes(), 2);
    let mut remove = Budget::new(5);
    values.remove_metered(0, &mut remove).unwrap();
    assert_eq!(remove.remaining, 0);
    assert!(values.is_empty());
    exact(&values);
}
