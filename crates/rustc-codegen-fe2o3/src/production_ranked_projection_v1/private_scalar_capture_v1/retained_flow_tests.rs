use super::*;

fn reference(target: u32) -> Value {
    Value::Reference(Reference {
        target,
        mutable: false,
        borrow: Site {
            block: 0,
            statement: 1,
        },
    })
}

fn flow() -> Flow {
    Flow {
        values: BTreeMap::from([
            (0, Value::Opaque),
            (1, reference(0)),
            (2, Value::Fields(vec![Some(reference(0))])),
        ])
        .into(),
        ..Flow::default()
    }
}

#[test]
fn retained_flow_payload_once_handles_each_and_final_drop_are_conserved() {
    let mut budget = Budget::new(MAX_WORK);
    let original = flow();
    let payload_nodes = original.nodes() + PAYLOAD_OVERHEAD;
    let source = RetainedFlow::new(original, &mut budget).unwrap();
    assert_eq!(budget.storage.get(), payload_nodes + HANDLE_NODES);
    let left = source.share(&mut budget).unwrap();
    let right = source.share(&mut budget).unwrap();
    assert_eq!(budget.storage.get(), payload_nodes + 3 * HANDLE_NODES);
    assert!(Rc::ptr_eq(&source.payload, &left.payload));
    assert!(Rc::ptr_eq(&source.payload, &right.payload));
    drop(source);
    drop(left);
    assert_eq!(budget.storage.get(), payload_nodes + HANDLE_NODES);
    drop(right);
    assert_eq!(budget.storage.get(), 0);
}

#[test]
fn retained_flow_reproduces_route42_census_without_a_full_successor_copy() {
    // This is only the measured node/ledger census, not reconstructed source
    // facts and not evidence that Route reaches its complete fixed point.
    let census = Flow {
        values: (0..628)
            .map(|local| (local, Value::Opaque))
            .collect::<BTreeMap<_, _>>()
            .into(),
        ..Flow::default()
    };
    assert_eq!(census.nodes(), 1257);
    let mut old = Budget::new(MAX_WORK);
    let _old_retained = old.reserve(130_054).unwrap();
    assert!(census.clone_retained(&mut old).is_err());
    assert_eq!(
        old.failure(),
        Some(ResourceFailure::Storage {
            operation: StorageOperation::Check,
            retained: 130_054,
            replaced: 0,
            requested: Some(1257),
            attempted: Some(131_311),
            limit: 131_072,
        })
    );

    let mut budget = Budget::new(MAX_WORK);
    let rest = budget.reserve(130_054 - census.nodes()).unwrap();
    let source = RetainedFlow::new(census, &mut budget).unwrap();
    assert_eq!(budget.storage.get(), 130_054 + PAYLOAD_OVERHEAD + HANDLE_NODES);
    let before_work = budget.remaining;
    let next = source.share(&mut budget).unwrap();
    assert_eq!(budget.storage.get(), 130_054 + PAYLOAD_OVERHEAD + 2 * HANDLE_NODES);
    assert_eq!(before_work - budget.remaining, 1);
    assert_eq!(next.flow(), source.flow());
    // Sharing cannot grant a free mutable copy at the same tight ceiling.
    assert!(next.working(&mut budget).is_err());
    drop(next);
    drop(source);
    drop(rest);
    assert_eq!(budget.storage.get(), 0);
}

#[test]
fn retained_flow_handle_limit_and_failed_factory_release_all_reservations() {
    let mut budget = Budget::new(MAX_WORK);
    let original = flow();
    let nodes = original.nodes();
    let mut rest = budget
        .reserve(MAX_STORAGE - nodes - PAYLOAD_OVERHEAD - HANDLE_NODES)
        .unwrap();
    let source = RetainedFlow::new(original, &mut budget).unwrap();
    assert_eq!(budget.storage.get(), MAX_STORAGE);
    assert!(source.share(&mut budget).is_err());
    assert_eq!(Rc::strong_count(&source.payload), 1);
    assert_eq!(budget.storage.get(), MAX_STORAGE);
    rest.resize(rest.nodes - HANDLE_NODES).unwrap();
    let shared = source.share(&mut budget).unwrap();
    assert_eq!(budget.storage.get(), MAX_STORAGE);
    drop(shared);
    drop(source);
    drop(rest);
    assert_eq!(budget.storage.get(), 0);

    let held = budget
        .reserve(MAX_STORAGE - nodes - PAYLOAD_OVERHEAD)
        .unwrap();
    let before = budget.storage.get();
    assert!(RetainedFlow::new(flow(), &mut budget).is_err());
    assert_eq!(budget.storage.get(), before);
    drop(held);
    assert_eq!(budget.storage.get(), 0);
}

#[test]
fn retained_flow_rejects_foreign_ledger_and_work_exhaustion() {
    let mut budget = Budget::new(MAX_WORK);
    let mut foreign = Budget::new(MAX_WORK);
    let source = RetainedFlow::new(flow(), &mut budget).unwrap();
    assert!(source.share(&mut foreign).is_err());
    assert!(source.working(&mut foreign).is_err());
    let other = RetainedFlow::new(flow(), &mut foreign).unwrap();
    assert!(source.join(&other, &mut budget).is_err());
    assert!(source.join(&other, &mut foreign).is_err());
    let before = budget.storage.get();
    budget.remaining = 0;
    assert!(source.share(&mut budget).is_err());
    assert!(source.working(&mut budget).is_err());
    assert_eq!(budget.storage.get(), before);
    assert_eq!(Rc::strong_count(&source.payload), 1);
}

#[test]
fn retained_flow_mutation_and_call_result_working_copies_are_independent() {
    let mut budget = Budget::new(MAX_WORK);
    let original = flow();
    let source = RetainedFlow::new(original.clone(), &mut budget).unwrap();
    let sibling = source.share(&mut budget).unwrap();
    let mut working = sibling.working(&mut budget).unwrap();
    working.kill(0, &mut budget).unwrap();
    assert_eq!(source.flow(), &original);
    assert_eq!(sibling.flow(), &original);
    assert!(!working.values[&1].contains_references());
    assert!(!working.values[&2].contains_references());
    working.assign(7, Some(Value::Opaque), &mut budget).unwrap();
    let changed = RetainedFlow::new(working, &mut budget).unwrap();
    assert!(source.flow().values.get(&7).is_none());
    assert_eq!(changed.flow().values.get(&7), Some(&Value::Opaque));
    assert!(!Rc::ptr_eq(&source.payload, &changed.payload));
}

#[test]
fn retained_flow_join_uses_the_original_kills_escapes_and_must_meet() {
    for change in 0..5 {
        let left = flow();
        let mut right = left.clone();
        let mut reference_budget = Budget::new(MAX_WORK);
        match change {
            0 => {}
            1 => right.kill(0, &mut reference_budget).unwrap(),
            2 => right.escape(2, &mut reference_budget).unwrap(),
            3 => {
                right.dead.insert(0);
                right.kill(0, &mut reference_budget).unwrap();
            }
            4 => right
                .assign(1, Some(Value::Opaque), &mut reference_budget)
                .unwrap(),
            _ => unreachable!(),
        }
        let expected = left.join(&right, &mut reference_budget).unwrap();
        let mut budget = Budget::new(MAX_WORK);
        let left = RetainedFlow::new(left, &mut budget).unwrap();
        let right = RetainedFlow::new(right, &mut budget).unwrap();
        let joined = left.join(&right, &mut budget).unwrap();
        assert_eq!(joined.flow(), &expected, "mutation {change}");
        assert_eq!(Rc::ptr_eq(&joined.payload, &left.payload), change == 0);
        assert!(!Rc::ptr_eq(&joined.payload, &right.payload));
    }
}

#[test]
fn retained_flow_publication_moves_only_unique_storage_and_clones_shared_storage() {
    let mut budget = Budget::new(MAX_WORK);
    let original = flow();
    let source = RetainedFlow::new(original.clone(), &mut budget).unwrap();
    let sibling = source.share(&mut budget).unwrap();
    let nodes = source.flow().nodes();
    let before_work = budget.remaining;
    let mut first = source.into_working(&mut budget).unwrap();
    assert_eq!(before_work - budget.remaining, 1 + 2 * nodes);
    assert_eq!(
        budget.storage.get(),
        nodes + PAYLOAD_OVERHEAD + HANDLE_NODES
    );
    first.kill(0, &mut budget).unwrap();
    assert_eq!(sibling.flow(), &original);
    drop(first);
    let before_work = budget.remaining;
    let last = sibling.into_working(&mut budget).unwrap();
    assert_eq!(before_work - budget.remaining, 1);
    assert_eq!(budget.storage.get(), 0);
    assert_eq!(last, original);
}

#[test]
fn retained_flow_failed_shared_publication_keeps_other_entry_reserved() {
    let mut budget = Budget::new(MAX_WORK);
    let source = RetainedFlow::new(flow(), &mut budget).unwrap();
    let sibling = source.share(&mut budget).unwrap();
    let held = budget.reserve(MAX_STORAGE - budget.storage.get()).unwrap();
    assert!(source.into_working(&mut budget).is_err());
    assert_eq!(Rc::strong_count(&sibling.payload), 1);
    assert_eq!(budget.storage.get(), MAX_STORAGE - HANDLE_NODES);
    drop(sibling);
    drop(held);
    assert_eq!(budget.storage.get(), 0);
}
