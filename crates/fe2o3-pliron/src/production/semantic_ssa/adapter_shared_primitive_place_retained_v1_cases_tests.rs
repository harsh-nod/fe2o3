#[test]
fn retained_place_all_paths_match_complete_original_data_and_work() {
    for case in cases() {
        for operation in [Operation::Path, Operation::Deinitialize, Operation::Write] {
            let expected = original(operation, &case, LIMIT, false, 0);
            let (mut owner, prefix) = seeded(false, false, 0);
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(FLOOR + prefix).unwrap();
            let mut owned = prefix;
            let mut session = RetainedAliasSessionV1::begin(
                &mut owner,
                &case.function,
                &case.types,
                CAP,
                TREE,
                &mut budget,
                &mut owned,
            )
            .unwrap();
            let actual =
                operate(&mut session, operation, &case.place).map_err(|e| format!("{e:?}"));
            assert_eq!(actual, expected.0, "{} {operation:?}", case.name);
            session.finish().unwrap();
            assert_eq!(
                data_rows(&owner.data),
                expected.1,
                "{} {operation:?}",
                case.name
            );
            assert_eq!(budget.work(), expected.2 + 32);
            assert!(std::ptr::eq(owner.place.unwrap(), &case.place));
            assert!(
                owner
                    .postflight_for(&case.function, &case.types, &budget, &owned)
                    .is_ok()
            );
            assert_eq!(budget.storage(), FLOOR + owned);
            assert!(owner.removed.is_none() && owner.current.is_none() && owner.drain.is_none());
            assert!(owner.output.is_empty());
            drop(owner);
            budget.release_storage(owned).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
    }
}
#[test]
fn retained_place_every_work_cut_matches_original_partial_data_and_first_denial() {
    let mut saw_current = false;
    let mut saw_drain = false;
    let mut saw_output = false;
    let mut saw_removed = false;
    for case in cases() {
        for operation in [Operation::Path, Operation::Deinitialize, Operation::Write] {
            let needed = measure(operation, &case, false, false).0;
            for limit in 0..needed {
                let (mut owner, prefix) = seeded(false, false, 0);
                let mut work = Work::new(limit);
                let mut budget = Budget::new(&mut work, LIMIT);
                budget.reserve_storage(FLOOR + prefix).unwrap();
                let mut owned = prefix;
                let result: RetainedResult<()> = match RetainedAliasSessionV1::begin(
                    &mut owner,
                    &case.function,
                    &case.types,
                    CAP,
                    TREE,
                    &mut budget,
                    &mut owned,
                ) {
                    Err(error) => Err(error),
                    Ok(mut session) => {
                        let error = operate(&mut session, operation, &case.place).err().unwrap();
                        Err(session.map_error(error))
                    }
                };
                let RetainedAliasErrorV1::Resource(Resource::Work(first)) = result.err().unwrap()
                else {
                    panic!("wrong refusal");
                };
                assert_eq!(owner.failure, Some(Resource::Work(first)));
                assert_eq!(owner.phase, Phase::Terminal);
                assert_eq!(budget.storage(), FLOOR + owned);
                if limit >= 32 {
                    let expected = original(operation, &case, limit - 32, false, 0);
                    assert!(expected.0.is_err());
                    assert_eq!(
                        data_rows(&owner.data),
                        expected.1,
                        "{} {operation:?} {limit}",
                        case.name
                    );
                    assert_eq!(budget.work(), expected.2 + 32);
                    assert_eq!(first.actual(), expected.3.unwrap() + 32);
                    assert!(std::ptr::eq(owner.place.unwrap(), &case.place));
                } else {
                    assert_eq!(owned, prefix);
                    assert_eq!(budget.work(), 0);
                    assert!(owner.place.is_none());
                }
                saw_current |= owner.current.is_some();
                saw_drain |= owner
                    .drain
                    .as_ref()
                    .is_some_and(|d| !d.as_slice().is_empty());
                saw_output |= !owner.output.is_empty();
                saw_removed |= owner.removed.is_some();
                let parked = parked_rows(&owner);
                assert!(
                    owner
                        .postflight_for(&case.function, &case.types, &budget, &owned)
                        .is_err()
                );
                assert_eq!(parked, parked_rows(&owner));
                drop(owner);
                budget.release_storage(owned).unwrap();
                assert_eq!(budget.storage(), FLOOR);
                assert_eq!(budget.failed_work(), Some(first.actual()));
            }
        }
    }
    assert!(saw_current && saw_drain && saw_output && saw_removed);
}
#[test]
fn retained_place_every_storage_cut_retains_current_output_and_reinsert_buffers() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "prefix")
        .unwrap();
    let mut saw_current = false;
    let mut saw_output_reinsert = false;
    let mut saw_removed_reinsert = false;
    for operation in [Operation::Deinitialize, Operation::Write] {
        let (_, needed, prefix) = measure(operation, &case, false, false);
        for limit in FLOOR + prefix..needed {
            let (mut owner, actual_prefix) = seeded(false, false, 0);
            assert_eq!(actual_prefix, prefix);
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, limit);
            budget.reserve_storage(FLOOR + prefix).unwrap();
            let mut owned = prefix;
            let result: RetainedResult<()> = match RetainedAliasSessionV1::begin(
                &mut owner,
                &case.function,
                &case.types,
                CAP,
                TREE,
                &mut budget,
                &mut owned,
            ) {
                Err(error) => Err(error),
                Ok(mut session) => {
                    let error = operate(&mut session, operation, &case.place).err().unwrap();
                    Err(session.map_error(error))
                }
            };
            let RetainedAliasErrorV1::Resource(Resource::Storage(first)) = result.err().unwrap()
            else {
                panic!("wrong refusal");
            };
            assert_eq!(owner.failure, Some(Resource::Storage(first)));
            assert_eq!(budget.storage(), FLOOR + owned);
            assert_eq!(budget.failed_storage(), Some(first.actual()));
            if owned > prefix {
                assert!(std::ptr::eq(owner.place.unwrap(), &case.place));
                if let Some(current) = &owner.current {
                    saw_current = true;
                    assert_eq!(current.candidate, 0);
                    assert_eq!(current.fields, vec![0]);
                    assert_eq!(owner.drain.as_ref().unwrap().as_slice().len(), 2);
                    assert!(owner.output.is_empty());
                } else if let Some(held) = &owner.removed {
                    saw_removed_reinsert = true;
                    assert_eq!(held.cursor, 3);
                    assert_eq!(
                        aliases(&held.aliases),
                        vec![
                            (0, vec![0], true),
                            (1, vec![1, 1], false),
                            (2, vec![1, 1, 0], false)
                        ]
                    );
                } else {
                    saw_output_reinsert = true;
                    assert_eq!(aliases(&owner.output), vec![(0, vec![0], true)]);
                    assert!(owner.drain.is_none());
                }
                assert!(!owner.data.holders.contains_key(&7));
            }
            let peak = budget.peak_storage();
            drop(owner);
            budget.release_storage(owned).unwrap();
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.peak_storage(), peak);
        }
    }
    assert!(saw_current && saw_output_reinsert && saw_removed_reinsert);
}
#[test]
fn retained_place_write_parks_exact_current_unvisited_and_retained_aliases() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "prefix")
        .unwrap();
    let needed = measure(Operation::Write, &case, false, false).0;
    let mut seen = [false; 3];
    for limit in 32..needed {
        let (mut owner, prefix) = seeded(false, false, 0);
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(prefix).unwrap();
        let mut owned = prefix;
        {
            let mut session = RetainedAliasSessionV1::begin(
                &mut owner,
                &case.function,
                &case.types,
                CAP,
                TREE,
                &mut budget,
                &mut owned,
            )
            .unwrap();
            assert!(session.write_place(&case.place).is_err());
        }
        if let Some(current) = &owner.current {
            seen[current.candidate] = true;
            let expected = [vec![0], vec![1, 1], vec![1, 1, 0]];
            assert_eq!(current.fields, expected[current.candidate]);
            let drain = owner.drain.as_ref().unwrap().as_slice();
            assert_eq!(drain.len(), 2 - current.candidate);
            for alias in drain {
                assert_eq!(alias.fields, expected[alias.candidate]);
            }
            if current.candidate > 0 {
                assert_eq!(aliases(&owner.output), vec![(0, vec![0], true)]);
            } else {
                assert!(owner.output.is_empty());
            }
        }
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
    assert!(seen.into_iter().all(|value| value));
}
#[test]
fn retained_place_deinitialize_cursors_and_all_removed_fields_survive() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "prefix")
        .unwrap();
    let needed = measure(Operation::Deinitialize, &case, false, false).0;
    let mut seen = [false; 4];
    for limit in 32..needed {
        let (mut owner, prefix) = seeded(false, false, 0);
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(prefix).unwrap();
        let mut owned = prefix;
        {
            let mut session = RetainedAliasSessionV1::begin(
                &mut owner,
                &case.function,
                &case.types,
                CAP,
                TREE,
                &mut budget,
                &mut owned,
            )
            .unwrap();
            assert!(session.deinitialize(&case.place).is_err());
        }
        if let Some(held) = &owner.removed {
            seen[held.cursor] = true;
            assert_eq!(held.local, 7);
            assert_eq!(held.aliases.len(), 3);
            assert_eq!(held.aliases[0].fields, vec![0]);
            assert_eq!(held.aliases[1].fields, vec![1, 1]);
            assert_eq!(held.aliases[2].fields, vec![1, 1, 0]);
            assert!(!owner.data.holders.contains_key(&7));
        }
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
    assert!(seen.into_iter().all(|value| value));
}
#[test]
fn retained_place_empty_holders_follow_distinct_original_reinsert_rules() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "prefix")
        .unwrap();
    for operation in [Operation::Deinitialize, Operation::Write] {
        let expected = original(operation, &case, LIMIT, true, 0);
        let (mut owner, prefix) = seeded(true, false, 0);
        let pointer = owner.data.holders[&7].as_ptr();
        let capacity = owner.data.holders[&7].capacity();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(prefix).unwrap();
        let mut owned = prefix;
        let mut session = RetainedAliasSessionV1::begin(
            &mut owner,
            &case.function,
            &case.types,
            CAP,
            TREE,
            &mut budget,
            &mut owned,
        )
        .unwrap();
        operate(&mut session, operation, &case.place).unwrap();
        session.finish().unwrap();
        assert_eq!(data_rows(&owner.data), expected.1);
        assert_eq!(budget.work(), expected.2 + 32);
        if matches!(operation, Operation::Deinitialize) {
            assert!(owner.data.holders[&7].is_empty());
            assert_eq!(owner.data.holders[&7].as_ptr(), pointer);
            assert_eq!(owner.data.holders[&7].capacity(), capacity);
            assert_eq!(
                owned,
                prefix + total_frame() + size_of::<(u32, Vec<Alias>)>()
            );
        } else {
            assert!(!owner.data.holders.contains_key(&7));
            assert_eq!(owned, prefix + total_frame());
        }
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn retained_place_empty_allocated_output_survives_none_and_early_refusals() {
    for case in cases().into_iter().filter(|case| {
        original(Operation::Write, case, LIMIT, false, 0)
            .0
            .unwrap()
            .is_none()
    }) {
        for limit in [0, 32, 32 + TREE, LIMIT] {
            let (mut owner, prefix) = seeded(false, true, 0);
            let pointer = owner.output.as_ptr();
            let capacity = owner.output.capacity();
            assert!(capacity > 0);
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(prefix).unwrap();
            let mut owned = prefix;
            match RetainedAliasSessionV1::begin(
                &mut owner,
                &case.function,
                &case.types,
                CAP,
                TREE,
                &mut budget,
                &mut owned,
            ) {
                Err(_) => {}
                Ok(mut session) => {
                    if session.write_place(&case.place).is_ok() {
                        session.finish().unwrap();
                    }
                }
            }
            assert!(owner.output.is_empty());
            assert_eq!(owner.output.as_ptr(), pointer);
            assert_eq!(owner.output.capacity(), capacity);
            drop(owner);
            budget.release_storage(owned).unwrap();
        }
    }
}
#[test]
fn retained_place_empty_output_capacity_is_attached_before_population() {
    let case = cases().remove(0);
    let (mut owner, prefix) = seeded(false, false, 0);
    let mut work = Work::new(33);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(prefix).unwrap();
    let mut owned = prefix;
    {
        let mut session = RetainedAliasSessionV1::begin(
            &mut owner,
            &case.function,
            &case.types,
            CAP,
            TREE,
            &mut budget,
            &mut owned,
        )
        .unwrap();
        session.output_capacity().unwrap();
        assert!(session.owner.output.is_empty());
        assert!(session.owner.output.capacity() >= 1);
        let pointer = session.owner.output.as_ptr();
        let capacity = session.owner.output.capacity();
        let counter = *session.owned;
        assert!(session.tree().is_err());
        assert!(session.write_place(&case.place).is_err());
        assert!(session.output_capacity().is_err());
        assert_eq!(session.owner.output.as_ptr(), pointer);
        assert_eq!(session.owner.output.capacity(), capacity);
        assert_eq!(*session.owned, counter);
    }
    assert!(owner.output.is_empty() && owner.output.capacity() >= 1);
    drop(owner);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_place_original_errors_preserve_mutations_and_parked_fields() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "prefix")
        .unwrap();
    for (operation, malformed) in [
        (Operation::Deinitialize, 1),
        (Operation::Write, 1),
        (Operation::Write, 2),
    ] {
        let expected = original(operation, &case, LIMIT, false, malformed);
        let (mut owner, prefix) = seeded(false, false, malformed);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(prefix).unwrap();
        let mut owned = prefix;
        {
            let mut session = RetainedAliasSessionV1::begin(
                &mut owner,
                &case.function,
                &case.types,
                CAP,
                TREE,
                &mut budget,
                &mut owned,
            )
            .unwrap();
            let error = operate(&mut session, operation, &case.place).err().unwrap();
            assert_eq!(format!("{error:?}"), expected.0.err().unwrap());
            assert!(matches!(
                session.map_error(error),
                RetainedAliasErrorV1::Original(_)
            ));
        }
        assert_eq!(data_rows(&owner.data), expected.1);
        assert_eq!(budget.work(), expected.2 + 32);
        assert!(owner.failure.is_none());
        assert_eq!(owner.phase, Phase::Terminal);
        if matches!(operation, Operation::Write) {
            assert_eq!(owner.current.as_ref().unwrap().fields, vec![1, 1]);
            assert_eq!(aliases(&owner.output), vec![(0, vec![0], true)]);
            assert_eq!(
                aliases(owner.drain.as_ref().unwrap().as_slice()),
                vec![(2, vec![1, 1, 0], true)]
            );
        } else {
            assert_eq!(owner.removed.as_ref().unwrap().cursor, 1);
        }
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn retained_place_terminal_guards_preserve_inputs_outputs_and_first_failure() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "prefix")
        .unwrap();
    for resource in [false, true] {
        let (mut owner, prefix) = seeded(false, true, if resource { 0 } else { 1 });
        let mut work = Work::new(if resource { 33 } else { LIMIT });
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(prefix).unwrap();
        let mut owned = prefix;
        let mut session = RetainedAliasSessionV1::begin(
            &mut owner,
            &case.function,
            &case.types,
            CAP,
            TREE,
            &mut budget,
            &mut owned,
        )
        .unwrap();
        if resource {
            assert!(session.tree().is_err());
        } else {
            assert!(session.deinitialize(&case.place).is_err());
        }
        let before = (
            data_rows(&session.owner.data),
            parked_rows(session.owner),
            path_rows(session.owner.path),
            session.owner.place.map(|p| p as *const _),
            session.owner.output.as_ptr(),
            session.owner.output.capacity(),
            session.owner.failure,
            snapshot(session.budget, session.owned),
        );
        assert!(session.path(&case.place).is_err());
        assert!(
            session
                .matches(Slot::Current, case.place.projections())
                .is_err()
        );
        assert!(session.deinitialize(&case.place).is_err());
        assert!(session.write_place(&case.place).is_err());
        assert!(session.output_capacity().is_err());
        let after = (
            data_rows(&session.owner.data),
            parked_rows(session.owner),
            path_rows(session.owner.path),
            session.owner.place.map(|p| p as *const _),
            session.owner.output.as_ptr(),
            session.owner.output.capacity(),
            session.owner.failure,
            snapshot(session.budget, session.owned),
        );
        assert!(before == after); // Snapshot's ledger identity intentionally has no Debug.
        assert!(session.finish().is_err());
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn retained_place_incompatible_slots_fail_without_overwriting_payloads() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "prefix")
        .unwrap();
    for operation in [Operation::Deinitialize, Operation::Write] {
        for slot in 0..4 {
            let (mut owner, prefix) = seeded(false, false, 0);
            let aliases = owner.data.holders.remove(&7).unwrap();
            match slot {
                0 => {
                    owner.removed = Some(Held {
                        local: 7,
                        aliases,
                        cursor: 0,
                    })
                }
                1 => owner.drain = Some(aliases.into_iter()),
                2 => {
                    let mut values = aliases;
                    owner.current = Some(values.remove(0));
                    owner.data.holders.insert(7, values);
                }
                _ => owner.output = aliases,
            }
            let before = parked_rows(&owner);
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(prefix).unwrap();
            let mut owned = prefix;
            let mut session = RetainedAliasSessionV1::begin(
                &mut owner,
                &case.function,
                &case.types,
                CAP,
                TREE,
                &mut budget,
                &mut owned,
            )
            .unwrap();
            let entry = snapshot(session.budget, session.owned);
            assert!(operate(&mut session, operation, &case.place).is_err());
            assert_eq!(before, parked_rows(session.owner));
            assert!(entry == snapshot(session.budget, session.owned));
            assert!(session.owner.place.is_none());
            drop(session);
            drop(owner);
            budget.release_storage(owned).unwrap();
        }
    }
}
#[test]
fn retained_place_path_and_projection_borrows_bind_exact_input_lifetime() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "deref")
        .unwrap();
    let other = case.place.clone();
    let (mut owner, prefix) = seeded(false, false, 0);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(prefix).unwrap();
    let mut owned = prefix;
    let mut session = RetainedAliasSessionV1::begin(
        &mut owner,
        &case.function,
        &case.types,
        CAP,
        TREE,
        &mut budget,
        &mut owned,
    )
    .unwrap();
    let path = session.path(&case.place).unwrap().unwrap();
    assert_eq!(path.fields.as_ptr(), case.place.projections().as_ptr());
    assert_eq!(path.fields.len(), 2);
    assert_ne!(path.fields.as_ptr(), other.projections().as_ptr());
    session.finish().unwrap();
    assert!(std::ptr::eq(owner.place.unwrap(), &case.place));
    assert_eq!(
        owner.path.unwrap().fields.as_ptr(),
        case.place.projections().as_ptr()
    );
    // This is lifetime/pointer custody only, not proof of membership in function.
    drop(owner);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_place_match_slot_preserves_prefix_semantics_and_work() {
    for case in cases() {
        for index in 0..3 {
            for current in [false, true] {
                let (mut owner, prefix) = seeded(false, false, 0);
                let aliases = owner.data.holders.remove(&7).unwrap();
                let fields = aliases[index].fields.clone();
                if current {
                    let mut values = aliases;
                    owner.current = Some(values.remove(index));
                    owner.data.holders.insert(7, values);
                } else {
                    owner.removed = Some(Held {
                        local: 7,
                        aliases,
                        cursor: 0,
                    });
                }
                let mut work = Work::new(LIMIT);
                let mut budget = Budget::new(&mut work, LIMIT);
                budget.reserve_storage(prefix).unwrap();
                let mut owned = prefix;
                let mut session = RetainedAliasSessionV1::begin(
                    &mut owner,
                    &case.function,
                    &case.types,
                    CAP,
                    TREE,
                    &mut budget,
                    &mut owned,
                )
                .unwrap();
                let prefix_fields = case.place.projections();
                let value = session
                    .matches(
                        if current {
                            Slot::Current
                        } else {
                            Slot::Held(index)
                        },
                        prefix_fields,
                    )
                    .unwrap();
                let mut old_work = Work::new(LIMIT);
                let mut old_budget = Budget::new(&mut old_work, LIMIT);
                let mut meter = OriginalMeter(&mut old_budget);
                let mut observer = NoReads;
                let mut old = Analysis {
                    function: &case.function,
                    types: &case.types,
                    meter: &mut meter,
                    observer: &mut observer,
                    site: None,
                    candidates: vec![],
                    holders: BTreeMap::new(),
                    active: BTreeMap::new(),
                    alias_words: 0,
                    cap: CAP,
                    tree_work: TREE,
                    exhausted: false,
                };
                assert_eq!(value, old.matches(&fields, prefix_fields).unwrap());
                assert_eq!(session.budget.work(), old_budget.work() + 32);
                session.finish().unwrap();
                drop(owner);
                budget.release_storage(owned).unwrap();
            }
        }
    }
}
#[test]
fn retained_place_source_capacity_and_custody_order_is_explicit_without_route_activation() {
    fn compact(value: &str) -> String {
        value.split_whitespace().collect()
    }
    let source = compact(include_str!(
        "adapter_shared_primitive_place_retained_v1.rs"
    ));
    let owner = compact(include_str!(
        "adapter_shared_primitive_alias_retained_v1.rs"
    ));
    let parent = include_str!("adapter_shared_primitive_v29.rs");
    assert!(source.contains("place:&'aSemanticPlaceV1"));
    assert!(
        source.contains(
            "self.owner.drain=self.owner.data.holders.remove(&local).map(Vec::into_iter)"
        )
    );
    let capacity = source
        .split("fnoutput_capacity")
        .nth(1)
        .unwrap()
        .split("pub(super)fnframe")
        .next()
        .unwrap();
    let debit = capacity.find("self.reserve_storage(growth)").unwrap();
    let allocate = capacity
        .find("self.owner.output.try_reserve_exact(1)")
        .unwrap();
    let observed = capacity
        .find("letexcess=self.owner.output.capacity()")
        .unwrap();
    let excess = capacity.find("self.reserve_storage(excess)").unwrap();
    assert!(debit < allocate && allocate < observed && observed < excess);
    assert!(!capacity.contains("mem::take"));
    assert!(owner.contains("output:Vec::new(),place:None,path:None"));
    let ordinary = parent
        .split("// Inert retained alias-state primitives")
        .next()
        .unwrap();
    assert!(!ordinary.contains("RetainedAliasSessionV1"));
    assert!(!ordinary.contains("place_ops"));
}

#[test]
fn retained_place_all_output_growth_cuts_keep_prior_current_and_unvisited_payloads() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "nonmatching")
        .unwrap();
    let (_, needed, prefix) = measure(Operation::Write, &case, false, false);
    let mut seen = [false; 3];
    let mut saw_reinsert = false;
    for limit in FLOOR + prefix + total_frame()..needed {
        let (mut owner, _) = seeded(false, false, 0);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(FLOOR + prefix).unwrap();
        let mut owned = prefix;
        {
            let mut session = RetainedAliasSessionV1::begin(
                &mut owner,
                &case.function,
                &case.types,
                CAP,
                TREE,
                &mut budget,
                &mut owned,
            )
            .unwrap();
            assert!(session.write_place(&case.place).is_err());
        }
        assert_eq!(owner.phase, Phase::Terminal);
        assert!(!owner.data.holders.contains_key(&7));
        if let Some(current) = &owner.current {
            seen[current.candidate] = true;
            assert_eq!(owner.output.len(), current.candidate);
            assert_eq!(
                owner.drain.as_ref().unwrap().as_slice().len(),
                2 - current.candidate
            );
            let expected = [vec![0], vec![1, 1], vec![1, 1, 0]];
            assert_eq!(current.fields, expected[current.candidate]);
            for alias in &owner.output {
                assert_eq!(alias.fields, expected[alias.candidate]);
            }
            for alias in owner.drain.as_ref().unwrap().as_slice() {
                assert_eq!(alias.fields, expected[alias.candidate]);
            }
        } else {
            saw_reinsert = true;
            assert_eq!(owner.output.len(), 3);
            assert!(owner.drain.is_none());
        }
        assert_eq!(budget.storage(), FLOOR + owned);
        drop(owner);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
    // Exact reserve may grant extra capacity; assert a first growth denial and
    // final reinsert denial, and independently validate every observed growth.
    assert!(seen[0] && saw_reinsert);
}
#[test]
fn retained_place_empty_allocated_output_survives_storage_entry_refusal() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "prefix")
        .unwrap();
    let (mut owner, prefix) = seeded(false, true, 0);
    let pointer = owner.output.as_ptr();
    let capacity = owner.output.capacity();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, prefix + total_frame() - 1);
    budget.reserve_storage(prefix).unwrap();
    let mut owned = prefix;
    assert!(
        RetainedAliasSessionV1::begin(
            &mut owner,
            &case.function,
            &case.types,
            CAP,
            TREE,
            &mut budget,
            &mut owned
        )
        .is_err()
    );
    assert!(owner.output.is_empty() && capacity > 0);
    assert_eq!(owner.output.as_ptr(), pointer);
    assert_eq!(owner.output.capacity(), capacity);
    assert_eq!(owned, prefix);
    assert!(matches!(owner.failure, Some(Resource::Storage(_))));
    drop(owner);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_place_empty_removed_holder_survives_final_tree_and_reinsert_refusals() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "prefix")
        .unwrap();
    let full = original(Operation::Deinitialize, &case, LIMIT, true, 0).2;
    for storage in [false, true] {
        let (mut owner, prefix) = seeded(true, false, 0);
        let pointer = owner.data.holders[&7].as_ptr();
        let capacity = owner.data.holders[&7].capacity();
        let mut work = Work::new(if storage { LIMIT } else { 32 + full - 1 });
        let mut budget = Budget::new(
            &mut work,
            if storage {
                prefix + total_frame()
            } else {
                LIMIT
            },
        );
        budget.reserve_storage(prefix).unwrap();
        let mut owned = prefix;
        {
            let mut session = RetainedAliasSessionV1::begin(
                &mut owner,
                &case.function,
                &case.types,
                CAP,
                TREE,
                &mut budget,
                &mut owned,
            )
            .unwrap();
            assert!(session.deinitialize(&case.place).is_err());
        }
        let held = owner.removed.as_ref().unwrap();
        assert_eq!(held.local, 7);
        assert_eq!(held.cursor, 0);
        assert!(held.aliases.is_empty() && capacity > 0);
        assert_eq!(held.aliases.as_ptr(), pointer);
        assert_eq!(held.aliases.capacity(), capacity);
        assert!(!owner.data.holders.contains_key(&7));
        assert_eq!(owned, prefix + total_frame());
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
}
