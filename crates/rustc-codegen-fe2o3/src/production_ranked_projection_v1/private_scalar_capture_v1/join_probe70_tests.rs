use super::*;

fn scalar(count: u32) -> Flow {
    Flow {
        values: (0..count)
            .map(|key| (key, Value::Opaque))
            .collect::<BTreeMap<_, _>>()
            .into(),
        ..Flow::default()
    }
}

fn reference(target: u32) -> Value {
    Value::Reference(Reference {
        target,
        mutable: false,
        borrow: Site {
            block: 2,
            statement: 3,
        },
    })
}

fn measured(left: &Flow, right: &Flow) -> (bool, usize) {
    let mut budget = Budget::new(MAX_WORK);
    let answer = can_reuse(left, right, &mut budget).unwrap();
    (answer, MAX_WORK - budget.remaining)
}

fn agrees_with_original(left: &Flow, right: &Flow) {
    let expected = left.join(right, &mut Budget::new(MAX_WORK)).unwrap();
    assert_eq!(measured(left, right).0, left == right && &expected == left);
    let mut budget = Budget::new(MAX_WORK);
    let a = RetainedFlow::new(left.clone(), &mut budget).unwrap();
    let b = RetainedFlow::new(right.clone(), &mut budget).unwrap();
    let result = a.join(&b, &mut budget).unwrap();
    assert_eq!(result.flow(), &expected);
    assert_eq!(a.flow(), left);
    assert_eq!(b.flow(), right);
}

#[test]
fn join_probe70_distinct_payloads_compare_and_normalize_each_scalar_once() {
    for count in [0, 1, 8, 64, 1024] {
        let left = scalar(count);
        let right = left.clone();
        assert_eq!(measured(&left, &right), (true, 1 + 4 * count as usize));
        agrees_with_original(&left, &right);
    }
}

#[test]
fn join_probe70_rc_identity_skips_equality_but_spends_full_normalization_work() {
    for count in [0, 1, 8, 64, 1024] {
        let mut budget = Budget::new(MAX_WORK);
        let left = RetainedFlow::new(scalar(count), &mut budget).unwrap();
        let right = left.share(&mut budget).unwrap();
        let live = budget.storage.get();
        let before = budget.remaining;
        let result = left.join(&right, &mut budget).unwrap();
        // Pointer check, every key/value in normalization, then one handle.
        assert_eq!(before - budget.remaining, 2 + 2 * count as usize);
        assert!(Rc::ptr_eq(&left.payload, &result.payload));
        assert_eq!(budget.storage.get(), live + HANDLE_NODES);
    }
}

#[test]
fn join_probe70_first_difference_does_not_scan_an_unrelated_large_tail() {
    for count in [16, 256, 2048] {
        let left = scalar(count);
        let mut right = left.clone();
        right
            .values
            .insert(0, Value::Fields(vec![Some(Value::Opaque); 16]));
        assert_eq!(measured(&left, &right), (false, 5));
        // Normalizing the whole meet is still mandatory on probe failure.
        agrees_with_original(&left, &right);
        let mut wrong_key = left.clone();
        wrong_key.values.remove(&0);
        wrong_key.values.insert(count, Value::Opaque);
        assert_eq!(measured(&left, &wrong_key), (false, 3));
    }
}

#[test]
fn join_probe70_enum_shape_reference_site_and_poison_sets_remain_exact() {
    let mut left = scalar(4);
    left.values.insert(
        1,
        Value::Variants {
            ty: SemanticTypeIdV1::from_index(7),
            possible: 1,
            fields: vec![Some(Value::Fields(vec![Some(reference(0)), None])), None],
        },
    );
    left.dead.insert(8);
    left.escaped.insert(9);
    for mutation in 0..12 {
        let mut right = left.clone();
        match mutation {
            0 => {}
            1 => {
                right.dead.clear();
                right.dead.insert(9);
            }
            2 => {
                right.escaped.clear();
                right.escaped.insert(8);
            }
            3 => {
                right.dead.insert(0);
            }
            4 => {
                right.escaped.insert(0);
            }
            _ => right.values.edit(1, |value| {
                let Value::Variants {
                    ty,
                    possible,
                    fields,
                } = value
                else {
                    panic!()
                };
                match mutation {
                    5 => *ty = SemanticTypeIdV1::from_index(8),
                    6 => *possible = 3,
                    7 => fields.push(None),
                    8 => fields[0] = None,
                    9 => fields[1] = Some(reference(8)),
                    10 => fields[0] = Some(reference(9)),
                    11 => {
                        fields[0] = Some(Value::Reference(Reference {
                            target: 0,
                            mutable: true,
                            borrow: Site {
                                block: 4,
                                statement: 5,
                            },
                        }))
                    }
                    _ => unreachable!(),
                }
            }),
        }
        agrees_with_original(&left, &right);
        agrees_with_original(&right, &right);
        let mut budget = Budget::new(MAX_WORK);
        let a = RetainedFlow::new(right.clone(), &mut budget).unwrap();
        let b = a.share(&mut budget).unwrap();
        assert_eq!(
            a.join(&b, &mut budget).unwrap().flow(),
            &right.join(&right, &mut Budget::new(MAX_WORK)).unwrap()
        );
    }
}

#[test]
fn join_probe70_distinct_work_prefix_failure_keeps_both_owners_and_no_refund() {
    for mismatch in [false, true] {
        let left = scalar(8);
        let mut right = left.clone();
        if mismatch {
            right.values.insert(0, reference(7));
        }
        let expected = left.join(&right, &mut Budget::new(MAX_WORK)).unwrap();
        let mut full = Budget::new(MAX_WORK);
        let a = RetainedFlow::new(left.clone(), &mut full).unwrap();
        let b = RetainedFlow::new(right.clone(), &mut full).unwrap();
        let before = full.remaining;
        let done = a.join(&b, &mut full).unwrap();
        let used = before - full.remaining;
        drop(done);
        for available in 0..=used {
            let mut budget = Budget::new(MAX_WORK);
            let a = RetainedFlow::new(left.clone(), &mut budget).unwrap();
            let b = RetainedFlow::new(right.clone(), &mut budget).unwrap();
            let live = budget.storage.get();
            budget.remaining = available;
            let result = a.join(&b, &mut budget);
            assert_eq!(result.is_ok(), available == used);
            assert!(budget.remaining <= available);
            if let Ok(result) = result {
                assert_eq!(result.flow(), &expected);
                assert_eq!(budget.remaining, 0);
            } else {
                assert!(budget.work_exhausted);
                assert_eq!(Rc::strong_count(&a.payload), 1);
                assert_eq!(Rc::strong_count(&b.payload), 1);
            }
            assert_eq!(budget.storage.get(), live);
            assert_eq!(a.flow(), &left);
            assert_eq!(b.flow(), &right);
        }
    }
}

#[test]
fn join_probe70_working_mutation_cannot_inherit_identity_or_normalization() {
    let mut original = scalar(3);
    original.values.insert(1, reference(0));
    let mut budget = Budget::new(MAX_WORK);
    let a = RetainedFlow::new(original.clone(), &mut budget).unwrap();
    let kept = a.share(&mut budget).unwrap();
    let mut changed = kept.into_working(&mut budget).unwrap();
    changed.escaped.insert(0);
    let b = RetainedFlow::new(changed.clone(), &mut budget).unwrap();
    assert!(!Rc::ptr_eq(&a.payload, &b.payload));
    let same = b.share(&mut budget).unwrap();
    let result = b.join(&same, &mut budget).unwrap();
    assert!(!Rc::ptr_eq(&b.payload, &result.payload));
    assert_eq!(
        result.flow(),
        &changed.join(&changed, &mut Budget::new(MAX_WORK)).unwrap()
    );
    assert_eq!(a.flow(), &original);
}
