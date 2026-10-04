use super::*;

#[test]
fn block_prepass_denial_precedes_traversal_and_does_not_consume_elisions() {
    let (types, function, callables) = repeated_elisions(true, true, true);
    let transparent = transparent_borrow_sites_v1(&function, &callables);
    let prepared = prepare_semantic_ssa_adapter_with_observer_v1(
        &function,
        Some(&types),
        &callables,
        &transparent,
        &mut Trace::default(),
    )
    .unwrap();
    let mut denied = Trace {
        reject_hook: Some(RejectedHook::BlockPass),
        ..Trace::default()
    };
    let mut output = CountOutput::default();
    assert_eq!(
        prepared.emit_blocks(&mut output, &mut denied),
        Err(EmissionError::Observer(HOOK_DENIED))
    );
    assert_eq!(denied.block_passes, [2]);
    assert!(denied.visits.is_empty());
    assert!(denied.elision_lookups.is_empty());
    assert!(denied.elided.is_empty());
    assert!(denied.events.is_empty());
    assert!(denied.successors.is_empty());
    assert!(denied.blocks.is_empty());
    assert_eq!(output.counts.tuple(), (0, 0, 0, 0, 0));

    let mut accepted = Trace::default();
    prepared.emit_blocks(&mut output, &mut accepted).unwrap();
    assert_eq!(accepted.block_passes, [2]);
    assert_eq!(output.counts.tuple(), (9, 15, 9, 3, 0));
    assert_eq!(
        accepted.elided,
        [
            Site::Statement {
                block: 2,
                statement: 1
            },
            Site::Statement {
                block: 2,
                statement: 3
            },
        ]
    );

    let mut denied_real = Trace {
        reject_hook: Some(RejectedHook::BlockPass),
        ..Trace::default()
    };
    assert!(matches!(
        prepared.into_entries(&mut denied_real),
        Err(HOOK_DENIED)
    ));
    assert_eq!(denied_real.block_passes, [2]);
    assert!(denied_real.visits.is_empty());
    assert!(denied_real.events.is_empty());
    assert!(denied_real.entries.is_empty());
}

#[test]
fn entry_prepass_denial_precedes_local_visits_and_preserves_implicit_cursor() {
    for (role, expected) in [
        (SemanticLocalRoleV1::Argument(0), Entry::Argument(0)),
        (
            SemanticLocalRoleV1::RustCallTupleField {
                argument: 1,
                field: 2,
            },
            Entry::RustCallTupleField {
                argument: 1,
                field: 2,
            },
        ),
    ] {
        let (types, function, callables) = two_implicit_scopes();
        let mut locals = function.locals().to_vec();
        locals[3] = test_local(194, 2, role);
        let function = replace_source_parts(&function, locals, function.blocks().to_vec());
        let transparent = transparent_borrow_sites_v1(&function, &callables);
        let entries = prepare_semantic_ssa_adapter_with_observer_v1(
            &function,
            Some(&types),
            &callables,
            &transparent,
            &mut Trace::default(),
        )
        .unwrap()
        .into_entries(&mut Trace::default())
        .unwrap();
        let mut denied = Trace {
            reject_hook: Some(RejectedHook::EntryPass),
            ..Trace::default()
        };
        let mut output = CountOutput::default();
        assert_eq!(
            entries.emit_entries(&mut output, &mut denied),
            Err(EmissionError::Observer(HOOK_DENIED))
        );
        assert_eq!(denied.entry_passes, [(8, 2)]);
        assert!(denied.visits.is_empty());
        assert!(denied.entries.is_empty());
        assert!(denied.input.is_empty());
        assert_eq!(output.counts.tuple(), (0, 0, 0, 0, 0));

        let mut accepted = Trace::default();
        entries.emit_entries(&mut output, &mut accepted).unwrap();
        assert_eq!(accepted.entry_passes, [(8, 2)]);
        assert_eq!(output.counts.tuple(), (0, 0, 0, 0, 3));
        assert_eq!(
            accepted.entries,
            [
                (0, variable(1), Entry::ImplicitCapability),
                (1, variable(3), expected),
                (2, variable(6), Entry::ImplicitCapability),
            ]
        );

        let mut denied_real = Trace {
            reject_hook: Some(RejectedHook::EntryPass),
            ..Trace::default()
        };
        assert!(matches!(entries.finish(&mut denied_real), Err(HOOK_DENIED)));
        assert_eq!(denied_real.entry_passes, [(8, 2)]);
        assert!(denied_real.visits.is_empty());
        assert!(denied_real.entries.is_empty());
        assert!(denied_real.input.is_empty());
    }
}
