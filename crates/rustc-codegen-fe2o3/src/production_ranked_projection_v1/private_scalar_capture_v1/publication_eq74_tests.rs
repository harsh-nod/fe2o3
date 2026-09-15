use super::*;

fn flow() -> Flow {
    let reference = Value::Reference(Reference {
        target: 0,
        mutable: false,
        borrow: Site {
            block: 0,
            statement: 1,
        },
    });
    Flow {
        values: BTreeMap::from([
            (0, Value::Opaque),
            (1, reference.clone()),
            (2, Value::Fields(vec![Some(reference), None])),
        ])
        .into(),
        ..Flow::default()
    }
}

#[test]
fn publication_eq74_shared_payload_costs_one_without_storage_or_work_refund() {
    let mut budget = Budget::new(MAX_WORK);
    let source = RetainedFlow::new(flow(), &mut budget).unwrap();
    let other = source.share(&mut budget).unwrap();
    let retained = budget.storage.get();
    budget.remaining = 1;
    assert!(source.publication_eq(&other, &mut budget).unwrap());
    assert_eq!(budget.remaining, 0);
    assert!(source.publication_eq(&other, &mut budget).is_err());
    assert_eq!(budget.storage.get(), retained);
    drop(source);
    drop(other);
    assert_eq!(budget.storage.get(), 0);
    assert_eq!(budget.remaining, 0);
}

#[test]
fn publication_eq74_distinct_equal_payload_keeps_full_structural_precharge() {
    let expected = flow();
    let cost = 1 + 2 * expected.nodes();
    for available in [0, 1, cost - 1, cost] {
        let mut budget = Budget::new(MAX_WORK);
        let left = RetainedFlow::new(expected.clone(), &mut budget).unwrap();
        let right = RetainedFlow::new(expected.clone(), &mut budget).unwrap();
        assert!(!Rc::ptr_eq(&left.payload, &right.payload));
        let retained = budget.storage.get();
        budget.remaining = available;
        let result = left.publication_eq(&right, &mut budget);
        if available == cost {
            assert_eq!(result, Ok(true));
            assert_eq!(budget.remaining, 0);
        } else {
            assert!(result.is_err());
        }
        assert_eq!(budget.storage.get(), retained);
        assert_eq!(left.flow(), &expected);
        assert_eq!(right.flow(), &expected);
    }
}

#[test]
fn publication_eq74_reference_metadata_changes_remain_unequal() {
    for change in 0..3 {
        let mut changed = flow();
        let value = Value::Reference(Reference {
            target: if change == 0 { 2 } else { 0 },
            mutable: change == 1,
            borrow: Site {
                block: 0,
                statement: if change == 2 { 2 } else { 1 },
            },
        });
        let mut budget = Budget::new(MAX_WORK);
        changed
            .values
            .insert_metered(1, value, &mut budget)
            .unwrap();
        let left = RetainedFlow::new(flow(), &mut budget).unwrap();
        let right = RetainedFlow::new(changed, &mut budget).unwrap();
        let before = budget.remaining;
        assert!(!left.publication_eq(&right, &mut budget).unwrap());
        assert_eq!(
            before - budget.remaining,
            1 + left.flow().nodes() + right.flow().nodes()
        );
    }
}

#[test]
fn publication_eq74_rejects_foreign_ledgers_without_reservations() {
    let mut a = Budget::new(MAX_WORK);
    let mut b = Budget::new(MAX_WORK);
    let left = RetainedFlow::new(flow(), &mut a).unwrap();
    let right = RetainedFlow::new(flow(), &mut b).unwrap();
    let before = (a.remaining, b.remaining, a.storage.get(), b.storage.get());
    assert!(left.publication_eq(&right, &mut a).is_err());
    assert!(left.publication_eq(&right, &mut b).is_err());
    assert_eq!(
        before,
        (a.remaining, b.remaining, a.storage.get(), b.storage.get())
    );
}

#[test]
fn publication_eq74_never_replaces_dead_or_escaped_normalization() {
    for dead in [false, true] {
        let mut poisoned = flow();
        if dead {
            poisoned.dead.insert(0);
        } else {
            poisoned.escaped.insert(0);
        }
        let mut oracle = Budget::new(MAX_WORK);
        let expected = poisoned.join(&poisoned, &mut oracle).unwrap();
        assert_ne!(expected, poisoned);
        let mut budget = Budget::new(MAX_WORK);
        let old = RetainedFlow::new(poisoned, &mut budget).unwrap();
        let incoming = old.share(&mut budget).unwrap();
        let merged = old.join(&incoming, &mut budget).unwrap();
        assert_eq!(merged.flow(), &expected);
        assert!(!Rc::ptr_eq(&old.payload, &merged.payload));
        assert!(!old.publication_eq(&merged, &mut budget).unwrap());
    }
}

#[test]
fn publication_eq74_mutable_working_copy_cannot_change_retained_equality() {
    let mut budget = Budget::new(MAX_WORK);
    let old = RetainedFlow::new(flow(), &mut budget).unwrap();
    let sibling = old.share(&mut budget).unwrap();
    let mut working = old.working(&mut budget).unwrap();
    working.kill(0, &mut budget).unwrap();
    let changed = RetainedFlow::new(working, &mut budget).unwrap();
    assert!(old.publication_eq(&sibling, &mut budget).unwrap());
    assert!(!old.publication_eq(&changed, &mut budget).unwrap());
    assert_eq!(old.flow(), &flow());
}
