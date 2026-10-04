#[test]
fn retained_engine_all_source_producers_match_unchanged_whole_oracle_data_rows_and_work() {
    for (name, function, types) in corpus() {
        for cap in [0, 1, 4, 64] {
            let _context = (name, cap);
            run(&function, &types, cap, ROWS);
        }
    }
}
#[test]
fn retained_engine_every_work_cut_matches_original_partial_data_rows_and_first_denial() {
    for (name, function, types) in corpus().into_iter().filter(|(name, _, _)| {
        matches!(
            *name,
            "direct" | "tuple" | "self-assignment" | "two-blocks" | "assert-bounds"
        )
    }) {
        let needed = run(&function, &types, 64, ROWS).0;
        for limit in 0..needed {
            let mut engine = RetainedSharedEngineV1::new();
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(FLOOR).unwrap();
            let mut owned = 0;
            engine
                .prepare_observer_into(&function, &types, SCRATCH, ROWS, &mut budget, &mut owned)
                .unwrap();
            let result = engine.analyze_into(&function, &types, 64, &mut budget, &mut owned);
            assert!(
                matches!(
                    result,
                    Err(RetainedAliasErrorV1::Resource(Resource::Work(_)))
                ),
                "{name} {limit}"
            );
            assert_eq!(engine.phase, EnginePhase::Terminal);
            if limit >= 64 {
                let expected = oracle(&function, &types, 64, ROWS, limit - 64);
                assert!(expected.outcome.is_err());
                assert_eq!(budget.work(), expected.work + 64, "{name} {limit}");
                assert_eq!(
                    budget.failed_work(),
                    expected.denied.map(|actual| actual + 64)
                );
                assert_eq!(engine.aliases.observer.test_rows(), expected.rows);
                if !expected.blocks.is_empty() {
                    assert_eq!(engine_data(&engine), expected.blocks, "{name} {limit}");
                }
            }
            assert_eq!(budget.storage(), FLOOR + owned);
            let before = (
                engine_data(&engine),
                engine.aliases.observer.test_rows(),
                budget.work(),
                budget.storage(),
                owned,
            );
            assert!(
                engine
                    .analyze_into(&function, &types, 64, &mut budget, &mut owned)
                    .is_err()
            );
            assert!(
                engine
                    .prepare_observer_into(&function, &types, 0, 0, &mut budget, &mut owned)
                    .is_err()
            );
            assert_eq!(
                before,
                (
                    engine_data(&engine),
                    engine.aliases.observer.test_rows(),
                    budget.work(),
                    budget.storage(),
                    owned
                )
            );
            drop(engine);
            budget.release_storage(owned).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
    }
}
fn pending(
    engine: &RetainedSharedEngineV1<'_>,
) -> (
    AliasRows,
    Option<AliasRows>,
    Vec<u32>,
    Option<AliasRows>,
    Option<(usize, Vec<u32>, bool)>,
    AliasRows,
    AliasRows,
    Vec<u32>,
) {
    let owner = &engine.aliases;
    (
        aliases(&owner.rhs),
        owner.installed.as_ref().map(|v| aliases(v)),
        owner.replacement.clone(),
        owner.drain.as_ref().map(|v| aliases(v.as_slice())),
        owner
            .current
            .as_ref()
            .map(|a| (a.candidate, a.fields.clone(), a.live)),
        aliases(&owner.output),
        aliases(&owner.selected),
        owner.fields.clone(),
    )
}
#[test]
fn retained_engine_every_storage_cut_keeps_all_partial_owners_and_coupled_credits() {
    let mut rhs = false;
    let mut installed = false;
    let mut replacement = false;
    let mut selected = false;
    let mut current = false;
    let mut retired = false;
    // Fresh destinations in two-blocks never debit storage while RHS is live.
    // A two-reference aggregate retains its first real RHS alias while reading
    // the second operand; add_live admits active-map storage at that point.
    let mut cases: Vec<_> = corpus()
        .into_iter()
        .filter(|(name, _, _)| matches!(*name, "two-blocks" | "self-assignment"))
        .collect();
    cases.push((
        "two-alias-aggregate",
        function(vec![
            shared(),
            assign(
                place(6, 3),
                SemanticRvalueKindV1::aggregate(
                    SemanticAggregateKindV1::Tuple,
                    vec![
                        SemanticOperandV1::Copy(place(2, 1)),
                        SemanticOperandV1::Copy(place(2, 1)),
                    ],
                )
                .unwrap(),
            ),
            assign_operand(
                place(5, 0),
                SemanticOperandV1::Copy(projected(
                    6,
                    &[
                        (SemanticProjectionKindV1::Field(0), 1),
                        (SemanticProjectionKindV1::Dereference, 0),
                    ],
                )),
            ),
            assign_operand(
                place(5, 0),
                SemanticOperandV1::Copy(projected(
                    6,
                    &[
                        (SemanticProjectionKindV1::Field(1), 1),
                        (SemanticProjectionKindV1::Dereference, 0),
                    ],
                )),
            ),
            dead(6),
        ]),
        types(),
    ));
    for (name, function, types) in cases {
        let required = run(&function, &types, 64, ROWS).1;
        for limit in FLOOR..required {
            let mut engine = RetainedSharedEngineV1::new();
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, limit);
            budget.reserve_storage(FLOOR).unwrap();
            let mut owned = 0;
            let result = engine
                .prepare_observer_into(&function, &types, SCRATCH, ROWS, &mut budget, &mut owned)
                .and_then(|()| engine.analyze_into(&function, &types, 64, &mut budget, &mut owned));
            assert!(
                matches!(
                    result,
                    Err(RetainedAliasErrorV1::Resource(Resource::Storage(_)))
                ),
                "{name} {limit}"
            );
            rhs |= !engine.aliases.rhs.is_empty();
            installed |= engine.aliases.installed.is_some();
            replacement |= engine.aliases.replacement.capacity() != 0;
            selected |= !engine.aliases.selected.is_empty();
            current |= engine.aliases.current.is_some();
            retired |= !engine.retired.is_empty();
            assert_eq!(budget.storage(), FLOOR + owned);
            let before = (
                engine_data(&engine),
                pending(&engine),
                engine.aliases.observer.test_rows(),
                budget.work(),
                budget.storage(),
                owned,
            );
            assert!(
                engine
                    .analyze_into(&function, &types, 64, &mut budget, &mut owned)
                    .is_err()
            );
            assert_eq!(
                before,
                (
                    engine_data(&engine),
                    pending(&engine),
                    engine.aliases.observer.test_rows(),
                    budget.work(),
                    budget.storage(),
                    owned
                )
            );
            drop(engine);
            budget.release_storage(owned).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
    }
    assert!(rhs, "no storage denial retained the source-produced RHS");
    assert!(
        installed,
        "no storage denial retained the removed installed holder"
    );
    assert!(
        replacement,
        "no storage denial retained replacement-field capacity"
    );
    assert!(selected, "no storage denial retained selected aliases");
    assert!(current, "no storage denial retained the current alias");
    assert!(retired, "no storage denial retained prior-block DATA");
}
#[test]
fn retained_engine_exhausted_fallback_preserves_previous_accepted_set_and_data() {
    let function = two_blocks();
    let types = types();
    let mut saw = false;
    for cap in 1..16 {
        let mut engine = RetainedSharedEngineV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let mut owned = 0;
        engine
            .prepare_observer_into(&function, &types, SCRATCH, ROWS, &mut budget, &mut owned)
            .unwrap();
        engine
            .analyze_into(&function, &types, cap, &mut budget, &mut owned)
            .unwrap();
        let expected = whole_original(&function, &types, cap, ROWS, LIMIT);
        assert_eq!(
            engine
                .accepted_for(&function, &types, &budget, &owned)
                .unwrap(),
            expected.0.as_ref().unwrap()
        );
        if engine.exhausted && !engine.accepted.is_empty() {
            saw = true;
            assert!(engine.empty_result.is_empty());
            assert_eq!(engine.retired.len(), 1);
            assert!(engine.aliases.data.exhausted);
        }
    }
    assert!(saw);
}
#[test]
fn retained_engine_zero_observer_cap_keeps_original_error_and_real_candidate_side_effect() {
    let function = function(vec![shared(), read(2)]);
    let types = types();
    let mut engine = RetainedSharedEngineV1::new();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    engine
        .prepare_observer_into(&function, &types, 0, 0, &mut budget, &mut owned)
        .unwrap();
    assert!(matches!(
        engine.analyze_into(&function, &types, 64, &mut budget, &mut owned),
        Err(RetainedAliasErrorV1::Original(Error::ResourceOverflow))
    ));
    assert!(engine.aliases.failure.is_none());
    assert!(engine.aliases.data.candidates.iter().any(|c| c.read));
    assert!(engine.aliases.current.is_some());
    assert!(engine.aliases.observer.test_rows().is_empty());
    let before = (
        engine_data(&engine),
        pending(&engine),
        budget.work(),
        budget.storage(),
        owned,
    );
    assert!(
        engine
            .analyze_into(&function, &types, 64, &mut budget, &mut owned)
            .is_err()
    );
    assert_eq!(
        before,
        (
            engine_data(&engine),
            pending(&engine),
            budget.work(),
            budget.storage(),
            owned
        )
    );
}
#[test]
fn retained_engine_missing_preparation_and_wrong_source_refuse_without_semantic_work() {
    let function = function(vec![shared(), read(2)]);
    let types = types();
    let other = function.clone();
    for prepared in [false, true] {
        let mut engine = RetainedSharedEngineV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        if prepared {
            engine
                .prepare_observer_into(&function, &types, 0, ROWS, &mut budget, &mut owned)
                .unwrap();
        }
        let before = (budget.work(), budget.storage(), owned);
        assert!(
            engine
                .analyze_into(&other, &types, 64, &mut budget, &mut owned)
                .is_err()
        );
        assert_eq!(before, (budget.work(), budget.storage(), owned));
        assert!(engine.aliases.data.candidates.is_empty());
    }
}
#[test]
fn retained_engine_completed_view_binds_exact_source_pair_and_held_floor() {
    let function = function(vec![shared(), read(2)]);
    let types = types();
    let other = function.clone();
    let mut engine = RetainedSharedEngineV1::new();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    engine
        .prepare_observer_into(&function, &types, 0, ROWS, &mut budget, &mut owned)
        .unwrap();
    engine
        .analyze_into(&function, &types, 64, &mut budget, &mut owned)
        .unwrap();
    assert!(
        engine
            .accepted_for(&function, &types, &budget, &owned)
            .is_ok()
    );
    assert!(
        engine
            .accepted_for(&other, &types, &budget, &owned)
            .is_err()
    );
    let other_types = types.clone();
    assert!(
        engine
            .accepted_for(&function, &other_types, &budget, &owned)
            .is_err()
    );
    let other_owned = owned;
    assert!(
        engine
            .accepted_for(&function, &types, &budget, &other_owned)
            .is_err()
    );
    budget.reserve_storage(7).unwrap();
    owned += 7;
    assert!(
        engine
            .accepted_for(&function, &types, &budget, &owned)
            .is_ok()
    );
    budget.release_storage(8).unwrap();
    owned -= 8;
    assert!(
        engine
            .accepted_for(&function, &types, &budget, &owned)
            .is_err()
    );
}
#[test]
fn retained_engine_ignored_original_error_cannot_restart_or_replace_any_pending_arena() {
    let function = function(vec![shared(), read(2)]);
    let types = types();
    let mut owner = RetainedAliasStateV1::new();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    owner
        .prepare_observer_into(&function, &types, 0, 0, &mut budget, &mut owned)
        .unwrap();
    let mut session = RetainedAliasSessionV1::begin(
        &mut owner,
        &function,
        &types,
        64,
        256,
        &mut budget,
        &mut owned,
    )
    .unwrap();
    session
        .statement(
            SemanticTransparentBorrowSiteV1 {
                block: 0,
                statement: 0,
            },
            function.blocks()[0].statements()[0].kind(),
        )
        .unwrap();
    assert!(
        session
            .statement(
                SemanticTransparentBorrowSiteV1 {
                    block: 0,
                    statement: 1
                },
                function.blocks()[0].statements()[1].kind()
            )
            .is_err()
    );
    let before = (
        data_rows(&session.owner.data),
        session.owner.current.as_ref().map(|a| a.fields.clone()),
        session.budget.work(),
        session.budget.storage(),
        *session.owned,
    );
    assert!(
        session
            .close_block(function.blocks()[0].terminator().kind())
            .is_err()
    );
    assert!(
        session
            .statement(
                SemanticTransparentBorrowSiteV1 {
                    block: 0,
                    statement: 0
                },
                function.blocks()[0].statements()[0].kind()
            )
            .is_err()
    );
    assert!(session.discard_rhs(true).is_err());
    assert!(session.expiry_drop_local(2).is_err());
    assert_eq!(
        before,
        (
            data_rows(&session.owner.data),
            session.owner.current.as_ref().map(|a| a.fields.clone()),
            session.budget.work(),
            session.budget.storage(),
            *session.owned
        )
    );
}
#[test]
fn retained_engine_new_statement_and_close_do_not_overwrite_unconsumed_rhs() {
    let function = function(vec![
        shared(),
        assign_operand(place(3, 1), SemanticOperandV1::Copy(place(2, 1))),
    ]);
    let types = types();
    for close in [false, true] {
        let mut owner = RetainedAliasStateV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        owner
            .prepare_observer_into(&function, &types, 0, ROWS, &mut budget, &mut owned)
            .unwrap();
        let mut session = RetainedAliasSessionV1::begin(
            &mut owner,
            &function,
            &types,
            64,
            256,
            &mut budget,
            &mut owned,
        )
        .unwrap();
        session
            .statement(
                SemanticTransparentBorrowSiteV1 {
                    block: 0,
                    statement: 0,
                },
                function.blocks()[0].statements()[0].kind(),
            )
            .unwrap();
        let SemanticStatementKindV1::Assign(assignment) =
            function.blocks()[0].statements()[1].kind()
        else {
            unreachable!()
        };
        session.rvalue(assignment.value()).unwrap();
        assert!(!session.owner.rhs.is_empty());
        let before = (
            aliases(&session.owner.rhs),
            data_rows(&session.owner.data),
            session.budget.work(),
            session.budget.storage(),
            *session.owned,
        );
        let result = if close {
            session.close_block(function.blocks()[0].terminator().kind())
        } else {
            session.statement(
                SemanticTransparentBorrowSiteV1 {
                    block: 0,
                    statement: 1,
                },
                function.blocks()[0].statements()[1].kind(),
            )
        };
        assert!(result.is_err());
        assert_eq!(
            before,
            (
                aliases(&session.owner.rhs),
                data_rows(&session.owner.data),
                session.budget.work(),
                session.budget.storage(),
                *session.owned
            )
        );
    }
}
#[test]
fn retained_engine_caller_unwind_keeps_source_created_rhs_until_drop_before_refund() {
    let function = function(vec![
        shared(),
        assign_operand(place(3, 1), SemanticOperandV1::Copy(place(2, 1))),
    ]);
    let types = types();
    let mut owner = RetainedAliasStateV1::new();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    owner
        .prepare_observer_into(&function, &types, 0, ROWS, &mut budget, &mut owned)
        .unwrap();
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut session = RetainedAliasSessionV1::begin(
            &mut owner,
            &function,
            &types,
            64,
            256,
            &mut budget,
            &mut owned,
        )
        .unwrap();
        session
            .statement(
                SemanticTransparentBorrowSiteV1 {
                    block: 0,
                    statement: 0,
                },
                function.blocks()[0].statements()[0].kind(),
            )
            .unwrap();
        let SemanticStatementKindV1::Assign(assignment) =
            function.blocks()[0].statements()[1].kind()
        else {
            unreachable!()
        };
        session.rvalue(assignment.value()).unwrap();
        panic!("controlled caller unwind");
    }));
    assert!(caught.is_err());
    assert_eq!(owner.phase, Phase::Terminal);
    assert!(!owner.rhs.is_empty());
    assert!(!owner.data.candidates.is_empty());
    assert_eq!(budget.storage(), FLOOR + owned);
    drop(owner);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}
#[test]
fn retained_engine_original_liveness_original_error_keeps_partial_liveness_and_terminal_state() {
    let function = function(vec![assign_operand(place(99, 0), constant(0))]);
    let types = types();
    let mut engine = RetainedSharedEngineV1::new();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    engine
        .prepare_observer_into(&function, &types, 0, ROWS, &mut budget, &mut owned)
        .unwrap();
    assert!(matches!(
        engine.analyze_into(&function, &types, 64, &mut budget, &mut owned),
        Err(RetainedAliasErrorV1::Original(Error::ReplayMismatch))
    ));
    assert_eq!(engine.phase, EnginePhase::Terminal);
    assert_eq!(engine.aliases.phase, Phase::Terminal);
    assert!(engine.aliases.failure.is_none());
    assert!(
        engine
            .liveness
            .completed_for(&function, &budget, &owned)
            .is_err()
    );
    assert_eq!(budget.storage(), owned);
}
#[test]
fn retained_engine_source_order_keeps_expiry_before_fallback_and_filter_after_close() {
    fn compact(value: &str) -> String {
        let mut value: String = value.chars().filter(|c| !c.is_whitespace()).collect();
        while value.contains(",)") {
            value = value.replace(",)", ")");
        }
        value
    }
    let source = compact(include_str!(
        "adapter_shared_primitive_engine_retained_v1.rs"
    ));
    let order = [
        "session.statement(site,statement.kind())?;",
        "schedule.completed_retained(&mutsession,site,statement.kind())?;",
        "ifsession.owner.data.exhausted",
        "session.close_block(block.terminator().kind())?;",
        "let teardown = session.owner.data.alias_words.checked_mul(4)",
        "session.expiry_work(tree_work)?;",
        "self.accepted.insert(site);",
    ];
    let mut offset = 0;
    for marker in order {
        let marker = compact(marker);
        let next = source[offset..].find(&marker).expect(&marker);
        offset += next + marker.len();
    }
    assert!(source.contains("std::mem::swap(&mutself.owner.rhs,&mutself.owner.selected)"));
    assert!(!source.contains("self.accepted.clear()"));
    assert!(!source.contains("has_candidates("));
    assert!(!source.contains("scan_size("));
}

#[test]
fn retained_engine_expiry_original_error_terminalizes_even_when_the_caller_ignores_it() {
    let function = function(vec![statement(SemanticStatementKindV1::Nop)]);
    let types = types();
    let invalid = SemanticStatementKindV1::StorageDead(local(99));
    let mut owner = RetainedAliasStateV1::new();
    let mut live = liveness::RetainedSharedLivenessV1::new();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    owner
        .prepare_observer_into(&function, &types, 0, ROWS, &mut budget, &mut owned)
        .unwrap();
    live.prepare_into(&function, &mut budget, &mut owned)
        .unwrap();
    let mut session = RetainedAliasSessionV1::begin(
        &mut owner,
        &function,
        &types,
        64,
        256,
        &mut budget,
        &mut owned,
    )
    .unwrap();
    let schedule = live
        .completed_for(&function, session.budget, session.owned)
        .unwrap();
    assert!(matches!(
        schedule.completed_retained(
            &mut session,
            SemanticTransparentBorrowSiteV1 {
                block: 0,
                statement: 0
            },
            &invalid
        ),
        Err(Error::ReplayMismatch)
    ));
    assert_eq!(session.owner.phase, Phase::Terminal);
    let before = (
        session.budget.work(),
        session.budget.storage(),
        *session.owned,
        data_rows(&session.owner.data),
    );
    assert!(
        session
            .statement(
                SemanticTransparentBorrowSiteV1 {
                    block: 0,
                    statement: 0
                },
                function.blocks()[0].statements()[0].kind()
            )
            .is_err()
    );
    assert_eq!(
        before,
        (
            session.budget.work(),
            session.budget.storage(),
            *session.owned,
            data_rows(&session.owner.data)
        )
    );
}
#[test]
fn retained_engine_empty_allocated_producer_owners_survive_entry_and_work_refusal() {
    let function = function(vec![statement(SemanticStatementKindV1::Nop)]);
    let types = types();
    for incompatible_installed in [false, true] {
        for close in [false, true] {
            // Inert prepaid empty-capacity fixture only; no admitted source facts.
            let mut owner = RetainedAliasStateV1::new();
            owner.rhs = Vec::with_capacity(3);
            owner.replacement = Vec::with_capacity(5);
            if incompatible_installed {
                owner.installed = Some(Vec::with_capacity(7));
            }
            let prefix = owner.rhs.capacity() * size_of::<Alias>()
                + owner.replacement.capacity() * size_of::<u32>()
                + owner
                    .installed
                    .as_ref()
                    .map_or(0, |v| v.capacity() * size_of::<Alias>());
            let mut work = Work::new(32);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(FLOOR + prefix).unwrap();
            let mut owned = prefix;
            owner
                .prepare_observer_into(&function, &types, 0, ROWS, &mut budget, &mut owned)
                .unwrap();
            let pointers = (
                owner.rhs.as_ptr(),
                owner.rhs.capacity(),
                owner.replacement.as_ptr(),
                owner.replacement.capacity(),
                owner.installed.as_ref().map(|v| (v.as_ptr(), v.capacity())),
            );
            {
                let mut session = RetainedAliasSessionV1::begin(
                    &mut owner,
                    &function,
                    &types,
                    64,
                    256,
                    &mut budget,
                    &mut owned,
                )
                .unwrap();
                let result = if close {
                    session.close_block(function.blocks()[0].terminator().kind())
                } else {
                    session.statement(
                        SemanticTransparentBorrowSiteV1 {
                            block: 0,
                            statement: 0,
                        },
                        function.blocks()[0].statements()[0].kind(),
                    )
                };
                assert!(result.is_err());
                assert_eq!(session.budget.work(), 32);
            }
            assert_eq!(
                pointers,
                (
                    owner.rhs.as_ptr(),
                    owner.rhs.capacity(),
                    owner.replacement.as_ptr(),
                    owner.replacement.capacity(),
                    owner.installed.as_ref().map(|v| (v.as_ptr(), v.capacity()))
                )
            );
            assert_eq!(budget.storage(), FLOOR + owned);
            drop(owner);
            budget.release_storage(owned).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
    }
}
