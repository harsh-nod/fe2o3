use super::*;

fn reference(target: u32, salt: usize) -> Value {
    Value::Reference(Reference {
        target,
        mutable: salt & 1 != 0,
        borrow: Site {
            block: salt % 3,
            statement: salt % 5,
        },
    })
}

fn scalar_flow(count: u32) -> Flow {
    Flow {
        values: (0..count)
            .map(|local| (local, Value::Opaque))
            .collect::<BTreeMap<_, _>>()
            .into(),
        ..Flow::default()
    }
}

fn aliases() -> Flow {
    Flow {
        values: BTreeMap::from([
            (0, Value::Opaque),
            (1, reference(0, 0)),
            (2, Value::Fields(vec![Some(reference(3, 1)), None])),
            (3, Value::Opaque),
            (
                4,
                Value::Variants {
                    ty: SemanticTypeIdV1::from_index(0),
                    possible: 1,
                    fields: vec![Some(Value::Fields(vec![Some(reference(0, 2))])), None],
                },
            ),
        ])
        .into(),
        dead: BTreeSet::from([7, 11]),
        escaped: BTreeSet::from([8, 12]),
    }
}

#[test]
fn unchanged_private_join_reuses_only_equal_normalized_values_and_keeps_owner() {
    let original = aliases();
    let expected = original
        .join(&original, &mut Budget::new(MAX_WORK))
        .unwrap();
    assert_eq!(expected, original);
    let mut budget = Budget::new(MAX_WORK);
    let left = RetainedFlow::new(original.clone(), &mut budget).unwrap();
    let right = RetainedFlow::new(original, &mut budget).unwrap();
    let before = budget.storage.get();
    let joined = left.join(&right, &mut budget).unwrap();
    assert_eq!(joined.flow(), &expected);
    assert!(Rc::ptr_eq(&joined.payload, &left.payload));
    assert!(!Rc::ptr_eq(&joined.payload, &right.payload));
    assert_eq!(budget.storage.get(), before + HANDLE_NODES);
    let left_payload = joined.flow().nodes() + PAYLOAD_OVERHEAD;
    drop(left);
    drop(right);
    assert_eq!(budget.storage.get(), left_payload + HANDLE_NODES);
    drop(joined);
    assert_eq!(budget.storage.get(), 0);
}

#[test]
fn unchanged_private_join_equal_dead_escaped_and_inactive_targets_require_original_meet() {
    for mutation in 0..5 {
        let mut original = aliases();
        match mutation {
            0 => {
                original.dead.insert(0);
            }
            1 => {
                original.escaped.insert(3);
            }
            2 => {
                original.dead.insert(2);
            }
            3 => original.values.edit(4, |value| {
                let Value::Variants { fields, .. } = value else {
                    panic!()
                };
                // Even inactive slots are traversed by final invalidation.
                fields[1] = Some(reference(7, 0));
            }),
            4 => original.values.edit(4, |value| {
                let Value::Variants { fields, .. } = value else {
                    panic!()
                };
                fields[0] = Some(Value::Fields(vec![Some(Value::Fields(vec![Some(
                    reference(8, 1),
                )]))]));
            }),
            _ => unreachable!(),
        }
        assert!(!can_reuse(&original, &original, &mut Budget::new(MAX_WORK)).unwrap());
        let expected = original
            .join(&original, &mut Budget::new(MAX_WORK))
            .unwrap();
        assert_ne!(
            expected, original,
            "mutation {mutation} must not bypass normalization"
        );
        let mut budget = Budget::new(MAX_WORK);
        let left = RetainedFlow::new(original.clone(), &mut budget).unwrap();
        let right = left.share(&mut budget).unwrap();
        assert!(Rc::ptr_eq(&left.payload, &right.payload));
        let joined = left.join(&right, &mut budget).unwrap();
        assert_eq!(joined.flow(), &expected, "mutation {mutation}");
        assert!(!Rc::ptr_eq(&joined.payload, &left.payload));
        assert_eq!(left.flow(), &original);
        assert_eq!(right.flow(), &original);
    }
}

fn generated(seed: usize) -> Flow {
    let mut entries = BTreeMap::new();
    for index in 0..12 {
        if (seed >> (index % 7)) & 1 != 0 {
            continue;
        }
        let target = ((seed + index) % 9) as u32;
        let value = match (seed + index) % 5 {
            0 => Value::Opaque,
            1 => reference(target, seed + index),
            2 => Value::Fields(vec![
                None,
                Some(reference(target, seed)),
                Some(Value::Opaque),
            ]),
            3 => Value::Fields(vec![Some(Value::Fields(vec![Some(reference(
                target, seed,
            ))]))]),
            _ => {
                let possible = ((seed % 3) + 1) as u8;
                Value::Variants {
                    ty: SemanticTypeIdV1::from_index((seed % 3) as u32),
                    possible,
                    fields: (0..2)
                        .map(|i| {
                            (possible & (1 << i) != 0).then(|| {
                                Value::Fields(vec![Some(reference(target, seed + i)), None])
                            })
                        })
                        .collect(),
                }
            }
        };
        entries.insert(index as u32, value);
    }
    Flow {
        values: entries.into(),
        dead: (0..9)
            .filter(|i| seed & (1 << i) != 0)
            .map(|i| i as u32)
            .collect(),
        escaped: (0..9)
            .filter(|i| seed.rotate_left(2) & (1 << i) != 0)
            .map(|i| i as u32)
            .collect(),
    }
}

#[test]
fn unchanged_private_join_differential_preserves_complete_lattice_and_input_mutations() {
    let mut hits = 0;
    for seed in 0..1024usize {
        let raw = generated(seed);
        let normalized = raw.join(&raw, &mut Budget::new(MAX_WORK)).unwrap();
        for (left, right) in [
            (raw.clone(), raw.clone()),
            (raw, generated(seed.rotate_left(3) ^ 73)),
            (normalized.clone(), normalized),
        ] {
            let before = (left.clone(), right.clone());
            let expected = left.join(&right, &mut Budget::new(MAX_WORK)).unwrap();
            let mut budget = Budget::new(MAX_WORK);
            let a = RetainedFlow::new(left, &mut budget).unwrap();
            let b = RetainedFlow::new(right, &mut budget).unwrap();
            let joined = a.join(&b, &mut budget).unwrap();
            assert_eq!(joined.flow(), &expected, "seed {seed}");
            assert_eq!((a.flow(), b.flow()), (&before.0, &before.1));
            if Rc::ptr_eq(&joined.payload, &a.payload) {
                assert_eq!(a.flow(), b.flow());
                assert_eq!(a.flow(), &expected);
                hits += 1;
            }
            assert_eq!(
                joined.flow().values.nodes(),
                joined
                    .flow()
                    .values
                    .values()
                    .map(|value| 1 + value.nodes())
                    .sum::<usize>()
            );
            drop(joined);
            drop(a);
            drop(b);
            assert_eq!(budget.storage.get(), 0);
        }
    }
    assert!(
        hits >= 1024,
        "normalized equal corpus must exercise real reuse"
    );
}

#[test]
fn unchanged_private_join_work_prefixes_keep_inputs_and_do_not_refund_probe_or_fallback() {
    for rejected_normalization in [false, true] {
        let mut original = aliases();
        if rejected_normalization {
            original.escaped.insert(0);
        }
        let expected = original
            .join(&original, &mut Budget::new(MAX_WORK))
            .unwrap();
        let mut measured = Budget::new(MAX_WORK);
        let left = RetainedFlow::new(original.clone(), &mut measured).unwrap();
        let right = left.share(&mut measured).unwrap();
        let before = measured.remaining;
        let result = left.join(&right, &mut measured).unwrap();
        let used = before - measured.remaining;
        drop(result);
        for allowance in 0..=used {
            let mut budget = Budget::new(MAX_WORK);
            let a = RetainedFlow::new(original.clone(), &mut budget).unwrap();
            let b = a.share(&mut budget).unwrap();
            let live = budget.storage.get();
            budget.remaining = allowance;
            let result = a.join(&b, &mut budget);
            assert_eq!(result.is_ok(), allowance == used);
            assert_eq!(a.flow(), &original);
            assert_eq!(b.flow(), &original);
            assert!(budget.remaining <= allowance);
            if let Ok(joined) = result {
                assert_eq!(joined.flow(), &expected);
                assert_eq!(budget.remaining, 0);
            } else {
                assert!(budget.work_exhausted);
                assert_eq!(Rc::strong_count(&a.payload), 2);
            }
            assert_eq!(budget.storage.get(), live);
        }
    }
}

#[test]
fn unchanged_private_join_exact_handle_boundary_and_foreign_owner_fail_before_publication() {
    for short in [false, true] {
        let mut budget = Budget::new(MAX_WORK);
        let a = RetainedFlow::new(scalar_flow(64), &mut budget).unwrap();
        let b = a.share(&mut budget).unwrap();
        let live = budget.storage.get();
        let held = budget
            .reserve(MAX_STORAGE - live - HANDLE_NODES + usize::from(short))
            .unwrap();
        let before = budget.storage.get();
        let result = a.join(&b, &mut budget);
        assert_eq!(result.is_err(), short);
        if let Ok(joined) = result {
            assert!(Rc::ptr_eq(&a.payload, &joined.payload));
            assert_eq!(budget.storage.get(), MAX_STORAGE);
        } else {
            assert!(!budget.work_exhausted);
            assert_eq!(Rc::strong_count(&a.payload), 2);
        }
        assert_eq!(budget.storage.get(), before);
        drop(held);
        assert_eq!(budget.storage.get(), live);
    }
    let mut owner = Budget::new(MAX_WORK);
    let mut foreign = Budget::new(MAX_WORK);
    let a = RetainedFlow::new(scalar_flow(16), &mut owner).unwrap();
    let b = RetainedFlow::new(scalar_flow(16), &mut foreign).unwrap();
    let work = (owner.remaining, foreign.remaining);
    assert!(a.join(&b, &mut owner).is_err());
    assert!(a.join(&a, &mut foreign).is_err());
    assert_eq!((owner.remaining, foreign.remaining), work);
}

#[test]
fn unchanged_private_join_retains_real_spare_capacity_without_payload_discount() {
    let mut fields = Vec::with_capacity(64);
    fields.push(Some(Value::Opaque));
    let left = Flow {
        values: BTreeMap::from([(0, Value::Fields(fields))]).into(),
        ..Flow::default()
    };
    let right = left.clone();
    assert_eq!(left, right);
    assert!(left.nodes() > right.nodes());
    let expected = left.join(&right, &mut Budget::new(MAX_WORK)).unwrap();
    let mut budget = Budget::new(MAX_WORK);
    let a = RetainedFlow::new(left, &mut budget).unwrap();
    let b = RetainedFlow::new(right, &mut budget).unwrap();
    let live = budget.storage.get();
    let joined = a.join(&b, &mut budget).unwrap();
    assert_eq!(joined.flow(), &expected);
    assert!(joined.flow().nodes() > expected.nodes());
    assert_eq!(budget.storage.get(), live + HANDLE_NODES);
    assert!(Rc::ptr_eq(&a.payload, &joined.payload));
    let actual_payload = a.flow().nodes() + PAYLOAD_OVERHEAD;
    drop(a);
    drop(b);
    assert_eq!(budget.storage.get(), actual_payload + HANDLE_NODES);
    drop(joined);
    assert_eq!(budget.storage.get(), 0);
}

#[test]
fn unchanged_private_join_removes_rebuild_work_and_never_confuses_unequal_metadata() {
    for count in [16, 64, 256, 512] {
        let original = scalar_flow(count);
        let mut baseline = Budget::new(MAX_WORK);
        let _old = original.join(&original, &mut baseline).unwrap();
        let old_work = MAX_WORK - baseline.remaining;
        let mut budget = Budget::new(MAX_WORK);
        let a = RetainedFlow::new(original.clone(), &mut budget).unwrap();
        let b = RetainedFlow::new(original, &mut budget).unwrap();
        let before = budget.remaining;
        let result = a.join(&b, &mut budget).unwrap();
        let new_work = before - budget.remaining;
        assert!(new_work < old_work);
        assert!(Rc::ptr_eq(&a.payload, &result.payload));
        println!("equal_join68 values={count} old_work={old_work} new_work={new_work}");
    }
    let left = aliases();
    for mutation in 0..5 {
        let mut right = left.clone();
        match mutation {
            0 => right.values.edit(1, |value| *value = reference(0, 1)),
            1 => right.values.edit(1, |value| *value = reference(0, 2)),
            2 => right.values.edit(4, |value| {
                let Value::Variants { ty, .. } = value else {
                    panic!()
                };
                *ty = SemanticTypeIdV1::from_index(1);
            }),
            3 => right.values.edit(4, |value| {
                let Value::Variants { possible, .. } = value else {
                    panic!()
                };
                *possible = 3;
            }),
            4 => {
                right.dead.remove(&7);
                right.dead.insert(9);
            }
            _ => unreachable!(),
        }
        assert!(!can_reuse(&left, &right, &mut Budget::new(MAX_WORK)).unwrap());
        let expected = left.join(&right, &mut Budget::new(MAX_WORK)).unwrap();
        let mut budget = Budget::new(MAX_WORK);
        let a = RetainedFlow::new(left.clone(), &mut budget).unwrap();
        let b = RetainedFlow::new(right, &mut budget).unwrap();
        let joined = a.join(&b, &mut budget).unwrap();
        assert!(!Rc::ptr_eq(&a.payload, &joined.payload));
        assert_eq!(joined.flow(), &expected);
    }
}
