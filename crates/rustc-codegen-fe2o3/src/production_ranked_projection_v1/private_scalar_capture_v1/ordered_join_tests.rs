use super::*;

// Independent previous join recipe. Kept small and limited to this lattice
// operation; it is not a second analysis or source-admission implementation.
fn old_join(left: &Flow, right: &Flow, budget: &mut Budget) -> Result<Flow> {
    let nodes = left.nodes().checked_add(right.nodes()).ok_or(())?;
    budget.charge(nodes)?;
    let references = left.reference_nodes(budget)? + right.reference_nodes(budget)?;
    budget.check_storage(&[nodes, nodes, nodes, references, references])?;
    let mut joined = Flow {
        values: Values::default(),
        escaped: left.escaped.union(&right.escaped).copied().collect(),
        dead: left.dead.union(&right.dead).copied().collect(),
    };
    let mut killed = BTreeSet::new();
    for (&local, value) in left.values.iter() {
        if let Some(other) = right.values.get_metered(local, budget)? {
            joined.values.insert_metered(
                local,
                value.join(other, &mut joined.escaped, budget)?,
                budget,
            )?;
        } else {
            killed.insert(local);
            value.targets(&mut joined.escaped, budget)?;
        }
    }
    for (&local, value) in right.values.iter() {
        if left.values.get_metered(local, budget)?.is_none() {
            killed.insert(local);
            value.targets(&mut joined.escaped, budget)?;
        }
    }
    budget.charge(joined.escaped.len() + joined.dead.len() + joined.values.len())?;
    killed.extend(joined.escaped.iter().copied());
    killed.extend(joined.dead.iter().copied());
    for local in &joined.dead {
        joined.values.remove_metered(*local, budget)?;
    }
    joined.values.invalidate(&killed, budget)?;
    Ok(joined)
}

fn reference(local: u32, site: usize, mutable: bool) -> Value {
    Value::Reference(Reference {
        target: local,
        mutable,
        borrow: Site {
            block: site,
            statement: site + 1,
        },
    })
}

fn value(seed: usize) -> Value {
    match seed % 8 {
        0 => Value::Opaque,
        1 => reference((seed % 7) as u32, seed % 3, false),
        2 => reference((seed % 7) as u32, seed % 3, true),
        3 => Value::Fields(vec![
            Some(reference(2, seed % 2, false)),
            None,
            Some(Value::Opaque),
        ]),
        4 => Value::Fields(vec![
            None,
            Some(Value::Fields(vec![Some(reference(3, 0, true))])),
        ]),
        _ => {
            let possible = (seed % 3 + 1) as u8;
            Value::Variants {
                ty: SemanticTypeIdV1::from_index((seed % 2) as u32),
                possible,
                fields: (0..2)
                    .map(|v| {
                        (possible & (1 << v) != 0)
                            .then(|| Value::Fields(vec![Some(reference(v, seed % 2, false)), None]))
                    })
                    .collect(),
            }
        }
    }
}

fn flow(seed: usize) -> Flow {
    let values = (0..12)
        .filter(|i| (seed >> (i % 6)) & 1 == 0)
        .map(|i| ((i * 2) as u32, value(seed + i)))
        .collect::<BTreeMap<_, _>>()
        .into();
    Flow {
        values,
        escaped: (0..6)
            .filter(|i| (seed >> i) & 1 != 0)
            .map(|i| i as u32)
            .collect(),
        dead: (0..6)
            .filter(|i| (seed >> (i + 2)) & 1 != 0)
            .map(|i| i as u32 * 2)
            .collect(),
    }
}

#[test]
fn private_ordered_join_matches_old_meet_cache_and_poisoning() {
    for seed in 0..4096 {
        let left = flow(seed);
        let right = flow(seed.rotate_left(3) ^ 97);
        let originals = (left.clone(), right.clone());
        let mut old = Budget::new(MAX_WORK);
        let expected = old_join(&left, &right, &mut old).unwrap();
        let mut new = Budget::new(MAX_WORK);
        let actual = left.join(&right, &mut new).unwrap();
        assert_eq!(actual, expected, "seed={seed}");
        assert_eq!(actual.nodes(), expected.nodes());
        assert_eq!((left, right), originals);
        assert!(
            new.remaining >= old.remaining,
            "removed searches cannot add work"
        );
    }
}

#[test]
fn private_ordered_join_identical_inputs_still_poison_dead_and_escaped_references() {
    let left = Flow {
        values: BTreeMap::from([
            (0, Value::Opaque),
            (1, reference(0, 0, false)),
            (2, Value::Fields(vec![Some(reference(3, 0, false))])),
            (3, Value::Opaque),
        ])
        .into(),
        dead: BTreeSet::from([0]),
        escaped: BTreeSet::from([3]),
    };
    let result = left.join(&left, &mut Budget::new(MAX_WORK)).unwrap();
    assert!(!result.values.contains_key(&0));
    assert_eq!(result.values[&1], Value::Opaque);
    assert_eq!(result.values[&2], Value::Fields(vec![Some(Value::Opaque)]));
    assert_ne!(result, left, "equal input is not an identity shortcut");
}

#[test]
fn private_ordered_join_work_and_storage_boundaries_keep_inputs_immutable() {
    let (left, right) = (flow(16), flow(69));
    let before = (left.clone(), right.clone());
    let mut measured = Budget::new(MAX_WORK);
    let expected = left.join(&right, &mut measured).unwrap();
    let used = MAX_WORK - measured.remaining;
    for allowance in 0..=used {
        let mut budget = Budget::new(allowance);
        let result = left.join(&right, &mut budget);
        assert_eq!(result.is_ok(), allowance == used);
        if let Ok(result) = result {
            assert_eq!(result, expected);
        }
        assert_eq!((&left, &right), (&before.0, &before.1));
        assert!(budget.remaining <= allowance);
    }
    let nodes = left.nodes() + right.nodes();
    let references = left.reference_nodes(&mut Budget::new(MAX_WORK)).unwrap()
        + right.reference_nodes(&mut Budget::new(MAX_WORK)).unwrap();
    let scratch = 3 * nodes + 2 * references;
    for short in [false, true] {
        let mut budget = Budget::new(MAX_WORK);
        let held = budget
            .reserve(MAX_STORAGE - scratch + usize::from(short))
            .unwrap();
        assert_eq!(left.join(&right, &mut budget).is_err(), short);
        assert_eq!(
            budget.storage.get(),
            MAX_STORAGE - scratch + usize::from(short)
        );
        drop(held);
        assert_eq!(budget.storage.get(), 0);
    }
}

#[test]
fn private_ordered_join_unique_entry_rejects_collision_and_exhaustion_without_mutation() {
    for seed in [0, 3, 6] {
        let original: Values = BTreeMap::from([(1, value(seed))]).into();
        for local in [1, 2] {
            let incoming = value(seed + 1);
            let key = key_work(original.len() + 1);
            let total = key + 1 + incoming.nodes();
            for allowance in 0..=total {
                let mut values = original.clone();
                let mut budget = Budget::new(allowance);
                let result = values.insert_new_metered(local, incoming.clone(), &mut budget);
                assert_eq!(result.is_ok(), local == 2 && allowance == total);
                if result.is_err() {
                    assert_eq!(values, original);
                }
                assert_eq!(
                    values.nodes(),
                    values.values().map(|v| 1 + v.nodes()).sum::<usize>()
                );
            }
        }
    }
}

#[test]
fn private_ordered_join_reduces_real_key_searches_without_changing_retained_size() {
    for count in [16, 64, 256, 512] {
        let left = Flow {
            values: (0..count)
                .map(|local| (local, Value::Opaque))
                .collect::<BTreeMap<_, _>>()
                .into(),
            ..Flow::default()
        };
        let mut old = Budget::new(MAX_WORK);
        let expected = old_join(&left, &left, &mut old).unwrap();
        let mut new = Budget::new(MAX_WORK);
        let actual = left.join(&left, &mut new).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(actual.nodes(), expected.nodes());
        assert!(new.remaining > old.remaining);
        println!(
            "ordered_join keys={count} old_work={} new_work={} retained_nodes={}",
            MAX_WORK - old.remaining,
            MAX_WORK - new.remaining,
            actual.nodes()
        );
    }
}
