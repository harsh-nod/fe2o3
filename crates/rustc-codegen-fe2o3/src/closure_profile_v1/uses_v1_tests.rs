use super::*;

fn environment(local: usize, call_kind: ClosureCallKindV1) -> ClosureEnvironmentV1 {
    ClosureEnvironmentV1 {
        local,
        origin: ClosureOriginV1::DeviceInternal,
        call_kind,
        definition_hash: [0; 16],
        size_bytes: 0,
        alignment_bytes: 1,
        captures: Vec::new(),
    }
}

fn operand(local: usize) -> Operand<'static> {
    Operand::Move(Place::from(Local::from_usize(local)))
}

fn call(block: usize, closure_local: usize) -> StaticClosureCallV1 {
    StaticClosureCallV1 {
        block,
        closure_local,
        call_kind: ClosureCallKindV1::FnOnce,
        argument_count: 0,
        target_definition_hash: [0; 16],
    }
}

#[test]
fn flattened_aliases_resolve_without_a_depth_limit() {
    let root = Local::from_usize(1);
    let roots = BTreeSet::from([root]);
    let aliases = (2..34)
        .map(|index| (Local::from_usize(index), root))
        .collect();
    let mut work = SourceClosureWorkV1::default();
    assert_eq!(
        resolve_alias_root(root, &roots, &aliases, &mut work).unwrap(),
        Some(root)
    );
    for index in 2..34 {
        assert_eq!(
            resolve_alias_root(Local::from_usize(index), &roots, &aliases, &mut work).unwrap(),
            Some(root),
        );
    }
    let chained = BTreeMap::from([
        (Local::from_usize(2), Local::from_usize(3)),
        (Local::from_usize(3), root),
    ]);
    assert_eq!(
        resolve_alias_root(Local::from_usize(2), &roots, &chained, &mut work).unwrap(),
        None,
    );
}

#[test]
fn forwarding_requires_the_exact_block_and_source_ordinal() {
    let environments = [environment(1, ClosureCallKindV1::Fn)];
    let roots = BTreeSet::from([Local::from_usize(1)]);
    let aliases = BTreeMap::from([(Local::from_usize(2), Local::from_usize(1))]);
    let forwarding = BTreeMap::from([(7, BTreeSet::from([0, 2]))]);
    let mut work = SourceClosureWorkV1::default();
    let mut scanner = ClosureUseScannerV1::new(
        ClosureUsesV1 {
            environments: &environments,
            closure_locals: &roots,
            aliases: &aliases,
            forwarding: &forwarding,
        },
        &mut work,
    )
    .unwrap();

    assert!(scanner.record_forward(6, 0, &operand(1)).is_err());
    assert!(scanner.record_forward(7, 1, &operand(1)).is_err());
    assert!(scanner.record_forward(7, 0, &operand(3)).is_err());
    scanner.record_forward(7, 0, &operand(1)).unwrap();
    scanner.record_forward(7, 2, &operand(2)).unwrap();
    let (calls, forwards) = scanner.finish().unwrap();
    assert!(calls.is_empty());
    assert_eq!(forwards.len(), 2);
    assert_eq!(
        (
            forwards[0].block,
            forwards[0].argument,
            forwards[0].closure_local
        ),
        (7, 0, 1)
    );
    assert_eq!(
        (
            forwards[1].block,
            forwards[1].argument,
            forwards[1].closure_local
        ),
        (7, 2, 1)
    );
}

#[test]
fn alias_assignment_requires_the_recorded_root() {
    let environments = [
        environment(1, ClosureCallKindV1::Fn),
        environment(2, ClosureCallKindV1::Fn),
    ];
    let roots = BTreeSet::from([Local::from_usize(1), Local::from_usize(2)]);
    let aliases = BTreeMap::from([(Local::from_usize(3), Local::from_usize(1))]);
    let forwarding = BTreeMap::new();
    let mut work = SourceClosureWorkV1::default();
    let mut scanner = ClosureUseScannerV1::new(
        ClosureUsesV1 {
            environments: &environments,
            closure_locals: &roots,
            aliases: &aliases,
            forwarding: &forwarding,
        },
        &mut work,
    )
    .unwrap();
    let destination = Place::from(Local::from_usize(3));
    assert!(
        scanner
            .allowed_closure_assignment(destination, &Rvalue::Use(operand(1)))
            .unwrap()
    );
    assert!(
        !scanner
            .allowed_closure_assignment(destination, &Rvalue::Use(operand(2)))
            .unwrap()
    );
    assert!(
        !scanner
            .allowed_closure_assignment(destination, &Rvalue::Use(operand(3)))
            .unwrap()
    );
    assert!(scanner.operand_mentions_closure(&operand(3)).unwrap());
}

#[test]
fn assertion_diagnostics_cannot_use_closures_or_aliases() {
    let environments = [environment(1, ClosureCallKindV1::Fn)];
    let roots = BTreeSet::from([Local::from_usize(1)]);
    let aliases = BTreeMap::from([(Local::from_usize(2), Local::from_usize(1))]);
    let forwarding = BTreeMap::new();
    let mut work = SourceClosureWorkV1::default();
    let mut scanner = ClosureUseScannerV1::new(
        ClosureUsesV1 {
            environments: &environments,
            closure_locals: &roots,
            aliases: &aliases,
            forwarding: &forwarding,
        },
        &mut work,
    )
    .unwrap();
    for (len, index) in [(1, 3), (3, 1), (2, 3), (3, 2)] {
        assert!(
            scanner
                .assert_message_mentions_closure(&AssertKind::BoundsCheck {
                    len: operand(len),
                    index: operand(index),
                })
                .unwrap()
        );
    }
    assert!(
        !scanner
            .assert_message_mentions_closure(&AssertKind::BoundsCheck {
                len: operand(3),
                index: operand(3),
            })
            .unwrap()
    );
    assert!(
        scanner
            .assert_message_mentions_closure(&AssertKind::DivisionByZero(operand(2)))
            .unwrap()
    );
}

#[test]
fn fn_once_counts_invocations_and_forwards_together() {
    let environments = [environment(1, ClosureCallKindV1::FnOnce)];
    let roots = BTreeSet::from([Local::from_usize(1)]);
    let aliases = BTreeMap::new();
    let forwarding = BTreeMap::from([(0, BTreeSet::from([0, 1]))]);
    for (invocations, forwards, accepted) in [
        (0, 0, false),
        (1, 0, true),
        (0, 1, true),
        (1, 1, false),
        (0, 2, false),
    ] {
        let mut work = SourceClosureWorkV1::default();
        let mut scanner = ClosureUseScannerV1::new(
            ClosureUsesV1 {
                environments: &environments,
                closure_locals: &roots,
                aliases: &aliases,
                forwarding: &forwarding,
            },
            &mut work,
        )
        .unwrap();
        for block in 0..invocations {
            scanner.record_call(call(block, 1)).unwrap();
        }
        for argument in 0..forwards {
            scanner.record_forward(0, argument, &operand(1)).unwrap();
        }
        assert_eq!(scanner.finish().is_ok(), accepted);
    }
}

#[test]
fn unused_fn_and_fn_mut_are_rejected() {
    let roots = BTreeSet::from([Local::from_usize(1)]);
    let aliases = BTreeMap::new();
    let forwarding = BTreeMap::new();
    for kind in [ClosureCallKindV1::Fn, ClosureCallKindV1::FnMut] {
        let environments = [environment(1, kind)];
        let mut work = SourceClosureWorkV1::default();
        let scanner = ClosureUseScannerV1::new(
            ClosureUsesV1 {
                environments: &environments,
                closure_locals: &roots,
                aliases: &aliases,
                forwarding: &forwarding,
            },
            &mut work,
        )
        .unwrap();
        assert!(scanner.finish().is_err());
    }
}

#[test]
fn static_use_limit_includes_both_record_kinds() {
    let environments = [environment(1, ClosureCallKindV1::Fn)];
    let roots = BTreeSet::from([Local::from_usize(1)]);
    let aliases = BTreeMap::new();
    let forwarding = (0..=MAX_STATIC_CALLS)
        .map(|block| (block, BTreeSet::from([0])))
        .collect();
    let mut work = SourceClosureWorkV1::default();
    let mut scanner = ClosureUseScannerV1::new(
        ClosureUsesV1 {
            environments: &environments,
            closure_locals: &roots,
            aliases: &aliases,
            forwarding: &forwarding,
        },
        &mut work,
    )
    .unwrap();
    for block in 0..MAX_STATIC_CALLS {
        if block % 2 == 0 {
            scanner.record_call(call(block, 1)).unwrap();
        } else {
            scanner.record_forward(block, 0, &operand(1)).unwrap();
        }
    }
    assert!(
        scanner
            .record_forward(MAX_STATIC_CALLS, 0, &operand(1))
            .is_err()
    );
    assert!(scanner.record_call(call(MAX_STATIC_CALLS, 1)).is_err());
    let (calls, forwards) = scanner.finish().unwrap();
    assert_eq!(calls.len() + forwards.len(), MAX_STATIC_CALLS);
}

#[test]
fn exhausted_shared_work_rejects_lookups_and_scans() {
    let roots = BTreeSet::from([Local::from_usize(1)]);
    let aliases = BTreeMap::new();
    let mut work = SourceClosureWorkV1::default();
    assert!(work.charge(usize::MAX).is_err());
    assert!(resolve_alias_root(Local::from_usize(1), &roots, &aliases, &mut work).is_err());
    assert!(operand_local(&operand(1), &mut work).is_err());
}
