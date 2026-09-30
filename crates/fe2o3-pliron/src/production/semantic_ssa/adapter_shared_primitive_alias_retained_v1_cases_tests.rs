#[test]
fn retained_alias_primitives_match_all_original_state_fields_and_work() {
    let function = function(vec![]);
    let types = types();
    for operation in all_operations() {
        let expected = original(operation, &function, &types, LIMIT);
        let (mut owner, prefix) = seeded(operation);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR + prefix).unwrap();
        let mut owned = prefix;
        let mut session = RetainedAliasSessionV1::begin(
            &mut owner,
            &function,
            &types,
            CAP,
            TREE,
            &mut budget,
            &mut owned,
        )
        .unwrap();
        let actual = operate(&mut session, operation).map_err(|error| format!("{error:?}"));
        assert_eq!(actual, expected.0, "{operation:?}");
        session.finish().unwrap();
        assert_eq!(data_rows(&owner.data), expected.1, "{operation:?}");
        assert_eq!(budget.work(), expected.2 + 32, "{operation:?}");
        assert!(
            owner
                .postflight_for(&function, &types, &budget, &owned)
                .is_ok()
        );
        assert_eq!(
            owned,
            prefix
                + frame().unwrap()
                + if matches!(operation, Operation::AddLive) {
                    active_mutation_frame().unwrap()
                } else if matches!(operation, Operation::EndLocal) {
                    size_of::<(u32, Vec<Alias>)>()
                } else {
                    0
                }
        );
        assert_eq!(budget.storage(), FLOOR + owned);
        if matches!(operation, Operation::Release) {
            assert!(owner.current.is_none());
        }
        if matches!(operation, Operation::Discard | Operation::Poison) {
            assert!(owner.drain.is_none() && owner.current.is_none());
        }
        if matches!(operation, Operation::EndLocal) {
            assert!(owner.removed.is_none());
        }
        drop(owner);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}
#[test]
fn retained_alias_every_work_cut_matches_original_prefix_and_preserves_state() {
    let function = function(vec![]);
    let types = types();
    let mut saw_current = false;
    let mut saw_remaining = false;
    let mut saw_removed = false;
    for operation in all_operations() {
        let (needed, _, _) = measure(operation, &function, &types);
        for limit in 0..needed {
            let (mut owner, prefix) = seeded(operation);
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(FLOOR + prefix).unwrap();
            let mut owned = prefix;
            let result: RetainedResult<()> = match RetainedAliasSessionV1::begin(
                &mut owner,
                &function,
                &types,
                CAP,
                TREE,
                &mut budget,
                &mut owned,
            ) {
                Err(error) => Err(error),
                Ok(mut session) => {
                    let error = operate(&mut session, operation).err().unwrap();
                    Err(session.map_error(error))
                }
            };
            let RetainedAliasErrorV1::Resource(Resource::Work(first)) = result.err().unwrap()
            else {
                panic!("wrong refusal");
            };
            assert_eq!(owner.failure, Some(Resource::Work(first)));
            assert_eq!(budget.failed_work(), Some(first.actual()));
            assert_eq!(budget.storage(), FLOOR + owned);
            assert_eq!(owner.phase, Phase::Terminal);
            if limit >= 32 {
                let expected = original(operation, &function, &types, limit - 32);
                assert!(expected.0.is_err());
                assert_eq!(
                    data_rows(&owner.data),
                    expected.1,
                    "{operation:?} limit {limit}"
                );
                assert_eq!(budget.work(), expected.2 + 32);
                assert_eq!(first.actual(), expected.3.unwrap() + 32);
            } else {
                assert_eq!(owned, prefix);
                assert_eq!(budget.work(), 0);
            }
            saw_current |= owner
                .current
                .as_ref()
                .is_some_and(|alias| !alias.fields.is_empty());
            saw_remaining |= owner
                .drain
                .as_ref()
                .is_some_and(|drain| !drain.as_slice().is_empty());
            saw_removed |= owner
                .removed
                .as_ref()
                .is_some_and(|held| !held.aliases.is_empty());
            let parked = parked_rows(&owner);
            assert!(
                owner
                    .postflight_for(&function, &types, &budget, &owned)
                    .is_err()
            );
            assert_eq!(parked, parked_rows(&owner));
            drop(owner);
            budget.release_storage(owned).unwrap();
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.failed_work(), Some(first.actual()));
        }
    }
    assert!(saw_current && saw_remaining && saw_removed);
}
#[test]
fn retained_alias_discard_keeps_current_and_unvisited_fields_at_each_refusal() {
    let function = function(vec![]);
    let types = types();
    let needed = measure(Operation::Poison, &function, &types).0;
    let mut saw_first = false;
    let mut saw_second = false;
    for limit in 32..needed {
        let (mut owner, prefix) = seeded(Operation::Poison);
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(prefix).unwrap();
        let mut owned = prefix;
        {
            let mut session = RetainedAliasSessionV1::begin(
                &mut owner,
                &function,
                &types,
                CAP,
                TREE,
                &mut budget,
                &mut owned,
            )
            .unwrap();
            assert!(session.discard_pending(true).is_err());
        }
        let current = owner.current.as_ref().unwrap();
        let remaining = owner.drain.as_ref().unwrap().as_slice();
        assert_eq!(
            current.fields,
            if current.candidate == 0 {
                vec![1]
            } else {
                vec![1, 1]
            }
        );
        if current.candidate == 0 {
            saw_first = true;
            assert_eq!(aliases(remaining), vec![(1, vec![1, 1], true)]);
        } else {
            saw_second = true;
            assert!(remaining.is_empty());
        }
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
    assert!(saw_first && saw_second);
}
#[test]
fn retained_alias_end_local_retains_removed_vector_and_cursor_on_error() {
    let function = function(vec![]);
    let types = types();
    let needed = measure(Operation::EndLocal, &function, &types).0;
    let mut cursors = [false; 3];
    for limit in 32..needed {
        let (mut owner, prefix) = seeded(Operation::EndLocal);
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(prefix).unwrap();
        let mut owned = prefix;
        {
            let mut session = RetainedAliasSessionV1::begin(
                &mut owner,
                &function,
                &types,
                CAP,
                TREE,
                &mut budget,
                &mut owned,
            )
            .unwrap();
            assert!(session.end_local(2).is_err());
        }
        if let Some(held) = &owner.removed {
            cursors[held.cursor] = true;
            assert_eq!(held.local, 2);
            assert_eq!(held.aliases.len(), 2);
            assert_eq!(held.aliases[0].fields, vec![1]);
            assert_eq!(held.aliases[1].fields, vec![1, 1]);
            assert!(!owner.data.holders.contains_key(&2));
        }
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
    assert!(cursors.into_iter().all(|seen| seen));
}
#[test]
fn retained_alias_every_storage_cut_preserves_seed_and_removed_reinsert_owners() {
    let function = function(vec![]);
    let types = types();
    for operation in [Operation::AddLive, Operation::EndLocal, Operation::Discard] {
        let (_, needed, prefix) = measure(operation, &function, &types);
        for limit in FLOOR + prefix..needed {
            let (mut owner, actual_prefix) = seeded(operation);
            assert_eq!(actual_prefix, prefix);
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, limit);
            budget.reserve_storage(FLOOR + prefix).unwrap();
            let mut owned = prefix;
            let result: RetainedResult<()> = match RetainedAliasSessionV1::begin(
                &mut owner,
                &function,
                &types,
                CAP,
                TREE,
                &mut budget,
                &mut owned,
            ) {
                Err(error) => Err(error),
                Ok(mut session) => {
                    let error = operate(&mut session, operation).err().unwrap();
                    Err(session.map_error(error))
                }
            };
            let RetainedAliasErrorV1::Resource(Resource::Storage(first)) = result.err().unwrap()
            else {
                panic!("wrong refusal");
            };
            assert_eq!(owner.failure, Some(Resource::Storage(first)));
            assert_eq!(budget.failed_storage(), Some(first.actual()));
            assert_eq!(budget.storage(), FLOOR + owned);
            if matches!(operation, Operation::EndLocal) && owned > prefix {
                let held = owner.removed.as_ref().unwrap();
                assert_eq!(held.cursor, 2);
                assert_eq!(
                    aliases(&held.aliases),
                    vec![(0, vec![1], false), (1, vec![1, 1], false)]
                );
            }
            let peak = budget.peak_storage();
            drop(owner);
            budget.release_storage(owned).unwrap();
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.peak_storage(), peak);
        }
    }
}
#[test]
fn retained_alias_malformed_active_set_keeps_original_error_and_current_payload() {
    let function = function(vec![]);
    let types = types();
    for operation in [Operation::MalformedActive, Operation::MalformedWords] {
        let expected = original(operation, &function, &types, LIMIT);
        let (mut owner, prefix) = seeded(operation);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(prefix).unwrap();
        let mut owned = prefix;
        {
            let mut session = RetainedAliasSessionV1::begin(
                &mut owner,
                &function,
                &types,
                CAP,
                TREE,
                &mut budget,
                &mut owned,
            )
            .unwrap();
            let error = session.release_current().err().unwrap();
            assert_eq!(format!("{error:?}"), expected.0.err().unwrap());
            assert!(matches!(
                session.map_error(error),
                RetainedAliasErrorV1::Original(_)
            ));
        }
        assert_eq!(data_rows(&owner.data), expected.1);
        assert_eq!(budget.work(), expected.2 + 32);
        assert!(owner.failure.is_none());
        assert_eq!(owner.current.as_ref().unwrap().fields, vec![1]);
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn retained_alias_exact_source_counter_and_ledger_bindings_are_checked() {
    let function = function(vec![]);
    let other_function = function.clone();
    let types = types();
    let other_types = types.clone();
    let mut owner = RetainedAliasStateV1::new();
    let mut work = Work::new(LIMIT);
    let mut other_work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut other = Budget::new(&mut other_work, LIMIT);
    let mut owned = 0;
    RetainedAliasSessionV1::begin(
        &mut owner,
        &function,
        &types,
        CAP,
        TREE,
        &mut budget,
        &mut owned,
    )
    .unwrap()
    .finish()
    .unwrap();
    assert!(
        owner
            .postflight_for(&other_function, &types, &budget, &owned)
            .is_err()
    );
    assert!(
        owner
            .postflight_for(&function, &other_types, &budget, &owned)
            .is_err()
    );
    let counterfeit = owned;
    assert!(
        owner
            .postflight_for(&function, &types, &budget, &counterfeit)
            .is_err()
    );
    other.charge_work(budget.work()).unwrap();
    other.reserve_storage(budget.storage()).unwrap();
    std::mem::swap(&mut budget, &mut other);
    assert!(
        owner
            .postflight_for(&function, &types, &budget, &owned)
            .is_err()
    );
    std::mem::swap(&mut budget, &mut other);
    assert!(
        owner
            .postflight_for(&function, &types, &budget, &owned)
            .is_ok()
    );
    drop(owner);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_alias_held_floor_and_sticky_denial_cannot_be_hidden() {
    let function = function(vec![]);
    let types = types();
    for storage in [false, true] {
        let mut owner = RetainedAliasStateV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        RetainedAliasSessionV1::begin(
            &mut owner,
            &function,
            &types,
            CAP,
            TREE,
            &mut budget,
            &mut owned,
        )
        .unwrap()
        .finish()
        .unwrap();
        budget.release_storage(1).unwrap();
        owned -= 1;
        assert!(
            owner
                .postflight_for(&function, &types, &budget, &owned)
                .is_err()
        );
        budget.reserve_storage(1).unwrap();
        owned += 1;
        assert!(
            owner
                .postflight_for(&function, &types, &budget, &owned)
                .is_ok()
        );
        budget.reserve_storage(7).unwrap();
        owned += 7;
        assert!(
            owner
                .postflight_for(&function, &types, &budget, &owned)
                .is_ok()
        );
        if storage {
            assert!(budget.reserve_storage(LIMIT + 1).is_err());
        } else {
            assert!(budget.charge_work(LIMIT + 1).is_err());
        }
        let denied = (budget.failed_work(), budget.failed_storage());
        assert!(
            owner
                .postflight_for(&function, &types, &budget, &owned)
                .is_err()
        );
        drop(owner);
        budget.release_storage(owned).unwrap();
        assert_eq!((budget.failed_work(), budget.failed_storage()), denied);
    }
}
#[test]
fn retained_alias_one_shot_and_existing_entry_denials_preserve_inputs() {
    let function = function(vec![]);
    let types = types();
    for denial in [0, 1, 2] {
        let (mut owner, prefix) = seeded(Operation::Poison);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(prefix).unwrap();
        let mut owned = prefix;
        if denial == 1 {
            assert!(budget.charge_work(LIMIT + 1).is_err());
        }
        if denial == 2 {
            assert!(budget.reserve_storage(LIMIT + 1).is_err());
        }
        let before = parked_rows(&owner);
        if denial == 0 {
            RetainedAliasSessionV1::begin(
                &mut owner,
                &function,
                &types,
                CAP,
                TREE,
                &mut budget,
                &mut owned,
            )
            .unwrap()
            .finish()
            .unwrap();
        }
        let checkpoint = (
            budget.work(),
            budget.storage(),
            owned,
            budget.failed_work(),
            budget.failed_storage(),
        );
        assert!(
            RetainedAliasSessionV1::begin(
                &mut owner,
                &function,
                &types,
                CAP,
                TREE,
                &mut budget,
                &mut owned
            )
            .is_err()
        );
        assert_eq!(
            checkpoint,
            (
                budget.work(),
                budget.storage(),
                owned,
                budget.failed_work(),
                budget.failed_storage()
            )
        );
        assert_eq!(parked_rows(&owner), before);
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn retained_alias_caller_error_and_unwind_drop_session_not_payloads() {
    let function = function(vec![]);
    let types = types();
    for unwind in [false, true] {
        let (mut owner, prefix) = seeded(Operation::Poison);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR + prefix).unwrap();
        let mut owned = prefix;
        let before = parked_rows(&owner);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _session = RetainedAliasSessionV1::begin(
                &mut owner,
                &function,
                &types,
                CAP,
                TREE,
                &mut budget,
                &mut owned,
            )
            .unwrap();
            if unwind {
                panic!("inert caller unwind");
            }
            Err::<(), &'static str>("inert caller error")
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(matches!(result, Ok(Err("inert caller error"))));
        }
        assert_eq!(owner.phase, Phase::Terminal);
        assert_eq!(parked_rows(&owner), before);
        assert_eq!(budget.storage(), FLOOR + owned);
        let peak = budget.peak_storage();
        drop(owner);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.peak_storage(), peak);
    }
}
#[test]
fn retained_alias_cap_fallback_is_original_data_not_resource_failure() {
    let function = function(vec![]);
    let types = types();
    let (mut owner, prefix) = seeded(Operation::Exhaust);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(prefix).unwrap();
    let mut owned = prefix;
    let mut session = RetainedAliasSessionV1::begin(
        &mut owner,
        &function,
        &types,
        CAP,
        TREE,
        &mut budget,
        &mut owned,
    )
    .unwrap();
    assert!(!session.reserve(CAP + 1).unwrap());
    session.finish().unwrap();
    assert!(owner.data.exhausted);
    assert_eq!(owner.data.alias_words, 5);
    assert!(owner.failure.is_none());
    assert_eq!(
        (budget.failed_work(), budget.failed_storage()),
        (None, None)
    );
    drop(owner);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_alias_empty_production_owner_has_no_synthetic_creation_route() {
    let function = function(vec![]);
    let types = types();
    let mut owner = RetainedAliasStateV1::new();
    assert!(
        owner.data.candidates.is_empty()
            && owner.data.holders.is_empty()
            && owner.data.active.is_empty()
    );
    assert!(owner.current.is_none() && owner.drain.is_none() && owner.removed.is_none());
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut session = RetainedAliasSessionV1::begin(
        &mut owner,
        &function,
        &types,
        CAP,
        TREE,
        &mut budget,
        &mut owned,
    )
    .unwrap();
    session.end_local(2).unwrap();
    session.finish().unwrap();
    assert!(
        owner.data.candidates.is_empty()
            && owner.data.holders.is_empty()
            && owner.data.active.is_empty()
    );
    assert_eq!(owned, frame().unwrap());
    drop(owner);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_alias_source_order_retains_consumed_inputs_before_fallibility() {
    let text = include_str!("adapter_shared_primitive_alias_retained_v1.rs");
    let discard =
        &text[text.find("fn discard_pending(").unwrap()..text.find("fn source_write(").unwrap()];
    let attached = discard.find("self.owner.current = next;").unwrap();
    assert!(attached < discard.find("alias_work(").unwrap());
    let end = &text[text.find("fn end_local(").unwrap()..text.find("impl Drop for").unwrap()];
    assert!(end.find("self.owner.removed =").unwrap() < end.find("self.deactivate(").unwrap());
    assert!(
        end.find("self.reserve_storage(").unwrap() < end.find("self.owner.removed.take()").unwrap()
    );
    assert!(!text.contains("release_storage("));
    assert!(!text.contains("Budget::new("));
    let parent = include_str!("adapter_shared_primitive_v29.rs");
    assert!(parent.contains("pub(in super::super) fn analyze_observed("));
    assert!(parent.contains("let liveness = liveness::Schedule::new(function, meter)?;"));
}

#[test]
fn retained_alias_first_resource_error_survives_later_denial_and_counter_overflow() {
    let function = function(vec![]);
    let types = types();
    for arithmetic in [false, true] {
        let (mut owner, prefix) = seeded(Operation::Poison);
        let mut work = Work::new(32);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(prefix).unwrap();
        let mut owned = prefix;
        let error;
        {
            let mut session = RetainedAliasSessionV1::begin(
                &mut owner,
                &function,
                &types,
                CAP,
                TREE,
                &mut budget,
                &mut owned,
            )
            .unwrap();
            if arithmetic {
                let paid = *session.owned;
                *session.owned = usize::MAX;
                assert_eq!(session.reserve_storage(1), Err(Resource::Arithmetic));
                *session.owned = paid;
                error = Resource::Arithmetic;
                assert_eq!(session.accept_work(1), Err(error));
            } else {
                let first = session.accept_work(1).err().unwrap();
                error = first;
                assert_eq!(session.reserve_storage(LIMIT + 1), Err(first));
            }
            assert_eq!(session.owner.failure, Some(error));
        }
        assert_eq!(owner.failure, Some(error));
        assert!(
            owner
                .drain
                .as_ref()
                .is_some_and(|drain| drain.as_slice().len() == 2)
        );
        assert_eq!(budget.failed_work(), Some(33));
        if !arithmetic {
            assert!(budget.failed_storage().is_some());
        }
        assert!(
            owner
                .postflight_for(&function, &types, &budget, &owned)
                .is_err()
        );
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
}

#[test]
fn retained_alias_ignored_original_refusal_cannot_finish_as_complete() {
    let function = function(vec![]);
    let types = types();
    let (mut owner, prefix) = seeded(Operation::MalformedActive);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(prefix).unwrap();
    let mut owned = prefix;
    let mut session = RetainedAliasSessionV1::begin(
        &mut owner,
        &function,
        &types,
        CAP,
        TREE,
        &mut budget,
        &mut owned,
    )
    .unwrap();
    assert!(session.release_current().is_err());
    assert!(session.finish().is_err());
    assert_eq!(owner.phase, Phase::Terminal);
    assert_eq!(owner.current.as_ref().unwrap().fields, vec![1]);
    assert!(
        owner
            .postflight_for(&function, &types, &budget, &owned)
            .is_err()
    );
    drop(owner);
    budget.release_storage(owned).unwrap();
}

#[test]
fn retained_alias_terminal_session_rejects_every_primitive_without_work_or_payload_change() {
    let function = function(vec![]);
    let types = types();
    for resource in [false, true] {
        let (mut owner, prefix) = seeded(if resource {
            Operation::Release
        } else {
            Operation::MalformedWords
        });
        // The refused tree charge leaves one accepted-work unit available.
        // Without an entry guard, deactivate/release could still consume it.
        let mut work = Work::new(if resource { 33 } else { LIMIT });
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(prefix).unwrap();
        let mut owned = prefix;
        let mut session = RetainedAliasSessionV1::begin(
            &mut owner,
            &function,
            &types,
            CAP,
            TREE,
            &mut budget,
            &mut owned,
        )
        .unwrap();
        if resource {
            assert!(session.tree().is_err());
            assert_eq!(session.budget.work(), 32);
            assert!(matches!(session.owner.failure, Some(Resource::Work(_))));
        } else {
            assert!(session.release_current().is_err());
            assert!(session.owner.failure.is_none());
        }
        assert_eq!(session.owner.phase, Phase::Terminal);
        let first_error = session.owner.failure;
        let first_denials = (
            session.budget.failed_work(),
            session.budget.failed_storage(),
        );
        let data = data_rows(&session.owner.data);
        let parked = parked_rows(session.owner);
        let current = session.owner.current.as_ref().unwrap();
        let field_owner = (current.fields.as_ptr() as usize, current.fields.capacity());
        let ledger = snapshot(session.budget, session.owned);
        for operation in all_operations() {
            assert!(operate(&mut session, operation).is_err(), "{operation:?}");
            assert_eq!(data_rows(&session.owner.data), data, "{operation:?}");
            assert_eq!(parked_rows(session.owner), parked, "{operation:?}");
            let current = session.owner.current.as_ref().unwrap();
            assert_eq!(
                (current.fields.as_ptr() as usize, current.fields.capacity()),
                field_owner
            );
            assert!(snapshot(session.budget, session.owned) == ledger);
            assert_eq!(
                (
                    session.budget.failed_work(),
                    session.budget.failed_storage()
                ),
                first_denials
            );
            assert_eq!(session.owner.phase, Phase::Terminal);
            assert_eq!(session.owner.failure, first_error);
        }
        assert!(session.finish().is_err());
        assert_eq!(owner.current.as_ref().unwrap().fields, vec![1]);
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
}
