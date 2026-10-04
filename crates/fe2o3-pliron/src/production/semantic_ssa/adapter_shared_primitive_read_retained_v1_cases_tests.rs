#[test]
fn retained_read_all_paths_and_operands_match_original_selected_data_rows_and_work() {
    for case in cases() {
        for operation in operations() {
            let operand = operand_for(operation, &case.place);
            for observed in [false, true] {
                let expected = original(operation, &case, &operand, LIMIT, ROW_CAP, CAP, observed);
                let (mut owner, prefix) = seeded(false, false, 0);
                owner.site = observed.then_some(current_site());
                let mut work = Work::new(LIMIT);
                let mut budget = Budget::new(&mut work, LIMIT);
                budget.reserve_storage(FLOOR + prefix).unwrap();
                let mut owned = prefix;
                owner
                    .prepare_observer_into(
                        &case.function,
                        &case.types,
                        SCRATCH,
                        ROW_CAP,
                        &mut budget,
                        &mut owned,
                    )
                    .unwrap();
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
                let result = operate(&mut session, operation, &case.place, &operand)
                    .map(|()| aliases(&session.owner.selected))
                    .map_err(|e| format!("{e:?}"));
                assert_eq!(result, expected.0, "{} {operation:?} {observed}", case.name);
                session.finish().unwrap();
                assert_eq!(data_rows(&owner.data), expected.1);
                assert_eq!(owner.observer.test_rows(), expected.2);
                assert_eq!(budget.work(), expected.3 + 32);
                assert!(owner.current.is_none() && owner.drain.is_none());
                assert!(owner.output.is_empty() && owner.fields.is_empty());
                assert!(
                    owner
                        .postflight_for(&case.function, &case.types, &budget, &owned)
                        .is_ok()
                );
                assert!(
                    owner
                        .observer
                        .postflight_for(
                            &case.function,
                            &case.types,
                            &budget,
                            &owned,
                            &owner.failure
                        )
                        .is_ok()
                );
                assert_eq!(budget.storage(), FLOOR + owned);
                drop(owner);
                budget.release_storage(owned).unwrap();
                assert_eq!(budget.storage(), FLOOR);
            }
        }
    }
}
#[test]
fn retained_read_every_work_cut_matches_original_partial_state_rows_and_first_denial() {
    let mut saw_selected = false;
    let mut saw_output = false;
    let mut saw_current = false;
    for case in cases().into_iter().filter(|case| {
        matches!(
            case.name,
            "prefix" | "deref" | "missing-field" | "whole" | "nonmatching"
        )
    }) {
        for operation in operations() {
            let operand = operand_for(operation, &case.place);
            let needed = measure(operation, &case, &operand).0;
            for limit in 0..needed {
                let (mut owner, prefix) = seeded(false, false, 0);
                owner.site = Some(current_site());
                let mut work = Work::new(limit);
                let mut budget = Budget::new(&mut work, LIMIT);
                budget.reserve_storage(FLOOR + prefix).unwrap();
                let mut owned = prefix;
                owner
                    .prepare_observer_into(
                        &case.function,
                        &case.types,
                        SCRATCH,
                        ROW_CAP,
                        &mut budget,
                        &mut owned,
                    )
                    .unwrap();
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
                        let error = operate(&mut session, operation, &case.place, &operand)
                            .err()
                            .unwrap();
                        Err(session.map_error(error))
                    }
                };
                let RetainedAliasErrorV1::Resource(Resource::Work(first)) = result.err().unwrap()
                else {
                    panic!("wrong refusal");
                };
                assert_eq!(owner.failure, Some(Resource::Work(first)));
                if limit >= 32 {
                    let expected =
                        original(operation, &case, &operand, limit - 32, ROW_CAP, CAP, true);
                    assert!(expected.0.is_err());
                    assert_eq!(
                        data_rows(&owner.data),
                        expected.1,
                        "{} {operation:?} {limit}",
                        case.name
                    );
                    assert_eq!(owner.observer.test_rows(), expected.2);
                    assert_eq!(budget.work(), expected.3 + 32);
                    assert_eq!(first.actual(), expected.4.unwrap() + 32);
                } else {
                    assert_eq!(budget.work(), 0);
                }
                saw_selected |= !owner.selected.is_empty();
                saw_output |= !owner.output.is_empty();
                saw_current |= owner.current.is_some();
                let payload = parked(&owner);
                let rows = owner.observer.test_rows();
                assert!(
                    owner
                        .postflight_for(&case.function, &case.types, &budget, &owned)
                        .is_err()
                );
                assert_eq!(parked(&owner), payload);
                assert_eq!(owner.observer.test_rows(), rows);
                assert_eq!(budget.storage(), FLOOR + owned);
                drop(owner);
                budget.release_storage(owned).unwrap();
                assert_eq!(budget.storage(), FLOOR);
            }
        }
    }
    assert!(saw_selected && saw_output && saw_current);
}
#[test]
fn retained_read_every_storage_cut_keeps_fields_selected_retained_and_current_owners() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "prefix")
        .unwrap();
    let operation = Operation::Read(false);
    let operand = operand_for(operation, &case.place);
    let (_, needed, prefix) = measure(operation, &case, &operand);
    let mut saw_fields = false;
    let mut saw_selected = false;
    let mut saw_output = false;
    let mut saw_current = false;
    let mut saw_reinsert = false;
    for limit in FLOOR + prefix..needed {
        let (mut owner, _) = seeded(false, false, 0);
        owner.site = Some(current_site());
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(FLOOR + prefix).unwrap();
        let mut owned = prefix;
        let result: RetainedResult<()> = owner
            .prepare_observer_into(
                &case.function,
                &case.types,
                SCRATCH,
                ROW_CAP,
                &mut budget,
                &mut owned,
            )
            .and_then(|()| {
                match RetainedAliasSessionV1::begin(
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
                        let error = session.read_place(&case.place, false).err().unwrap();
                        Err(session.map_error(error))
                    }
                }
            });
        assert!(matches!(
            result,
            Err(RetainedAliasErrorV1::Resource(Resource::Storage(_)))
        ));
        assert_eq!(budget.storage(), FLOOR + owned);
        assert_eq!(owner.phase, Phase::Terminal);
        saw_fields |= !owner.fields.is_empty();
        saw_selected |= !owner.selected.is_empty();
        saw_output |= !owner.output.is_empty();
        saw_current |= owner.current.is_some();
        if owner.current.is_none() && owner.drain.is_none() && !owner.output.is_empty() {
            saw_reinsert = true;
            assert_eq!(
                aliases(&owner.output),
                vec![
                    (0, vec![0], true),
                    (1, vec![1, 1], true),
                    (2, vec![1, 1, 0], true)
                ]
            );
            assert_eq!(
                aliases(&owner.selected),
                vec![(1, vec![1], true), (2, vec![1, 0], true)]
            );
        }
        if let Some(current) = &owner.current {
            let expected = [vec![0], vec![1, 1], vec![1, 1, 0]];
            assert_eq!(current.fields, expected[current.candidate]);
            for alias in owner.drain.as_ref().unwrap().as_slice() {
                assert_eq!(alias.fields, expected[alias.candidate]);
            }
            for alias in &owner.selected {
                assert_eq!(alias.fields, expected[alias.candidate][1..]);
            }
        }
        drop(owner);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
    assert!(saw_fields && saw_selected && saw_output && saw_current && saw_reinsert);
}
#[test]
fn retained_read_zero_observer_cap_preserves_original_read_side_effect_before_refusal() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "deref")
        .unwrap();
    let operation = Operation::Read(false);
    let operand = operand_for(operation, &case.place);
    let expected = original(operation, &case, &operand, LIMIT, 0, CAP, true);
    let (mut owner, prefix) = seeded(false, false, 0);
    owner.site = Some(current_site());
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(prefix).unwrap();
    let mut owned = prefix;
    owner
        .prepare_observer_into(
            &case.function,
            &case.types,
            SCRATCH,
            0,
            &mut budget,
            &mut owned,
        )
        .unwrap();
    let capacity = owner.observer.test_capacity();
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
        let error = session.read_place(&case.place, false).err().unwrap();
        assert_eq!(format!("{error:?}"), expected.0.err().unwrap());
        assert!(matches!(
            session.map_error(error),
            RetainedAliasErrorV1::Original(_)
        ));
    }
    assert_eq!(data_rows(&owner.data), expected.1);
    assert_eq!(owner.observer.test_rows(), expected.2);
    assert_eq!(budget.work(), expected.3 + 32);
    assert!(owner.failure.is_none());
    assert!(owner.data.candidates[1].read);
    assert_eq!(owner.current.as_ref().unwrap().fields, vec![1, 1]);
    assert_eq!(aliases(&owner.output), vec![(0, vec![0], true)]);
    assert_eq!(
        aliases(owner.drain.as_ref().unwrap().as_slice()),
        vec![(2, vec![1, 1, 0], true)]
    );
    assert_eq!(owner.observer.test_capacity(), capacity);
    drop(owner);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_read_expansion_fallback_preserves_original_data_and_empty_selected_owner() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "prefix")
        .unwrap();
    for operation in [
        Operation::Read(false),
        Operation::Read(true),
        Operation::Scalar(false),
    ] {
        let operand = operand_for(operation, &case.place);
        let expected = original(operation, &case, &operand, LIMIT, ROW_CAP, 9, true);
        let (mut owner, prefix) = seeded(false, false, 0);
        owner.site = Some(current_site());
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(prefix).unwrap();
        let mut owned = prefix;
        owner
            .prepare_observer_into(
                &case.function,
                &case.types,
                SCRATCH,
                ROW_CAP,
                &mut budget,
                &mut owned,
            )
            .unwrap();
        let mut session = RetainedAliasSessionV1::begin(
            &mut owner,
            &case.function,
            &case.types,
            9,
            TREE,
            &mut budget,
            &mut owned,
        )
        .unwrap();
        operate(&mut session, operation, &case.place, &operand).unwrap();
        session.finish().unwrap();
        assert_eq!(data_rows(&owner.data), expected.1);
        assert_eq!(budget.work(), expected.3 + 32);
        assert!(owner.data.exhausted);
        assert!(owner.selected.is_empty());
        assert!(owner.failure.is_none());
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn retained_read_terminal_guards_preserve_every_payload_and_observer_after_either_error() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "deref")
        .unwrap();
    let constant = operand_for(Operation::Constant, &case.place);
    for resource in [false, true] {
        let (mut owner, prefix) = seeded(false, false, 0);
        owner.site = Some(current_site());
        let mut work = Work::new(if resource { 33 } else { LIMIT });
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(prefix).unwrap();
        let mut owned = prefix;
        owner
            .prepare_observer_into(
                &case.function,
                &case.types,
                SCRATCH,
                0,
                &mut budget,
                &mut owned,
            )
            .unwrap();
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
        assert!(session.read_place(&case.place, false).is_err());
        let before = (
            parked(session.owner),
            data_rows(&session.owner.data),
            session.owner.observer.test_rows(),
            session.owner.observer.test_capacity(),
            session.owner.failure,
            snapshot(session.budget, session.owned),
        );
        assert!(session.read_place(&case.place, false).is_err());
        assert!(session.operand(&constant).is_err());
        assert!(session.scalar_operand(&constant).is_err());
        assert!(session.read_alias_capacity(ReadOutput::Selected).is_err());
        assert!(session.read_field_capacity(1).is_err());
        let after = (
            parked(session.owner),
            data_rows(&session.owner.data),
            session.owner.observer.test_rows(),
            session.owner.observer.test_capacity(),
            session.owner.failure,
            snapshot(session.budget, session.owned),
        );
        assert!(before == after);
        assert!(session.finish().is_err());
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn retained_read_empty_selected_and_field_allocations_survive_entry_and_work_refusal() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "prefix")
        .unwrap();
    for limit in [0, 32, 33] {
        let (mut owner, mut prefix) = seeded(false, false, 0);
        owner.selected = Vec::with_capacity(3);
        owner.fields = Vec::with_capacity(5);
        prefix += owner.selected.capacity() * size_of::<Alias>()
            + owner.fields.capacity() * size_of::<u32>();
        let payload = (
            owner.selected.as_ptr(),
            owner.selected.capacity(),
            owner.fields.as_ptr(),
            owner.fields.capacity(),
        );
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(prefix).unwrap();
        let mut owned = prefix;
        owner
            .prepare_observer_into(
                &case.function,
                &case.types,
                SCRATCH,
                ROW_CAP,
                &mut budget,
                &mut owned,
            )
            .unwrap();
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
                assert!(session.read_place(&case.place, false).is_err());
            }
        }
        assert!(owner.selected.is_empty() && owner.fields.is_empty());
        assert_eq!(
            payload,
            (
                owner.selected.as_ptr(),
                owner.selected.capacity(),
                owner.fields.as_ptr(),
                owner.fields.capacity()
            )
        );
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn retained_read_pending_selected_input_is_not_overwritten_by_another_operand() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "prefix")
        .unwrap();
    let constant = operand_for(Operation::Constant, &case.place);
    for scalar in [false, true] {
        let (mut owner, prefix) = seeded(false, false, 0);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(prefix).unwrap();
        let mut owned = prefix;
        owner
            .prepare_observer_into(
                &case.function,
                &case.types,
                SCRATCH,
                ROW_CAP,
                &mut budget,
                &mut owned,
            )
            .unwrap();
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
        session.read_place(&case.place, false).unwrap();
        let selected = aliases(&session.owner.selected);
        assert!(!selected.is_empty());
        let before = snapshot(session.budget, session.owned);
        if scalar {
            assert!(session.scalar_operand(&constant).is_err());
        } else {
            assert!(session.operand(&constant).is_err());
        }
        assert_eq!(aliases(&session.owner.selected), selected);
        assert!(before == snapshot(session.budget, session.owned));
        drop(session);
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn retained_read_observer_preparation_is_one_shot_and_keeps_existing_alias_payloads() {
    let case = cases().remove(0);
    let (mut owner, prefix) = seeded(false, false, 0);
    let before = data_rows(&owner.data);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(prefix).unwrap();
    let mut owned = prefix;
    owner
        .prepare_observer_into(
            &case.function,
            &case.types,
            SCRATCH,
            ROW_CAP,
            &mut budget,
            &mut owned,
        )
        .unwrap();
    let capacity = owner.observer.test_capacity();
    let checkpoint = snapshot(&budget, &owned);
    assert!(
        owner
            .prepare_observer_into(&case.function, &case.types, 0, 0, &mut budget, &mut owned)
            .is_err()
    );
    assert_eq!(owner.phase, Phase::Terminal);
    assert_eq!(data_rows(&owner.data), before);
    assert_eq!(owner.observer.test_capacity(), capacity);
    assert!(checkpoint == snapshot(&budget, &owned));
    drop(owner);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_read_caller_unwind_retains_selected_fields_and_real_observer_rows() {
    for name in ["prefix", "deref"] {
        let case = cases().into_iter().find(|case| case.name == name).unwrap();
        let (mut owner, prefix) = seeded(false, false, 0);
        owner.site = Some(current_site());
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(prefix).unwrap();
        let mut owned = prefix;
        owner
            .prepare_observer_into(
                &case.function,
                &case.types,
                SCRATCH,
                ROW_CAP,
                &mut budget,
                &mut owned,
            )
            .unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
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
            session.read_place(&case.place, false).unwrap();
            panic!("inert outer caller unwind");
        }));
        assert!(result.is_err());
        assert_eq!(owner.phase, Phase::Terminal);
        if name == "prefix" {
            assert_eq!(
                aliases(&owner.selected),
                vec![(1, vec![1], true), (2, vec![1, 0], true)]
            );
        } else {
            assert_eq!(owner.observer.test_rows().len(), 1);
        }
        assert_eq!(budget.storage(), owned);
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn retained_read_source_order_matches_real_selected_and_observer_construction_points() {
    let source: String = include_str!("adapter_shared_primitive_read_retained_v1.rs")
        .split_whitespace()
        .collect::<String>()
        .replace(",)", ")");
    let read = source
        .split("fnread_place")
        .nth(1)
        .unwrap()
        .split("fnoperand")
        .next()
        .unwrap();
    let reserve = read.find("self.reserve(1+length)?").unwrap();
    let work = read
        .find("alias_work(self.budget,&mutself.owner.failure,length)?")
        .unwrap();
    let live = read.find("self.add_live(index)?").unwrap();
    let fields = read.find("self.read_field_capacity(length)?").unwrap();
    let copy = read.find("self.owner.fields.extend_from_slice").unwrap();
    let selected = read
        .find("self.read_alias_capacity(ReadOutput::Selected)?")
        .unwrap();
    let push = read.find("self.owner.selected.push").unwrap();
    let consume = read.find("self.deactivate(Slot::Current)?").unwrap();
    assert!(
        reserve < work
            && work < live
            && live < fields
            && fields < copy
            && copy < selected
            && selected < push
            && push < consume
    );
    assert!(
        read.find("candidate.read=true").unwrap()
            < read.find("self.owner.observer.record").unwrap()
    );
    let scalar = source
        .split("fnscalar_operand")
        .nth(1)
        .unwrap()
        .split("fnread_alias_capacity")
        .next()
        .unwrap();
    assert!(
        scalar.find("self.owner.drain=Some").unwrap()
            < scalar.find("self.discard_pending(true)").unwrap()
    );
    assert!(!source.contains("Budget::new("));
    assert!(!source.contains("release_storage("));
}

#[test]
fn retained_read_prepared_observer_binding_is_checked_before_session_work() {
    let case = cases().remove(0);
    let other_function = case.function.clone();
    let other_types = case.types.clone();
    for changed_types in [false, true] {
        let (mut owner, prefix) = seeded(false, false, 0);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(prefix).unwrap();
        let mut owned = prefix;
        owner
            .prepare_observer_into(
                &case.function,
                &case.types,
                SCRATCH,
                ROW_CAP,
                &mut budget,
                &mut owned,
            )
            .unwrap();
        let before = (
            data_rows(&owner.data),
            owner.observer.test_rows(),
            owner.observer.test_capacity(),
            snapshot(&budget, &owned),
        );
        let function = if changed_types {
            &case.function
        } else {
            &other_function
        };
        let types = if changed_types {
            &other_types[..]
        } else {
            &case.types[..]
        };
        assert!(
            RetainedAliasSessionV1::begin(
                &mut owner,
                function,
                types,
                CAP,
                TREE,
                &mut budget,
                &mut owned
            )
            .is_err()
        );
        let after = (
            data_rows(&owner.data),
            owner.observer.test_rows(),
            owner.observer.test_capacity(),
            snapshot(&budget, &owned),
        );
        assert!(before == after);
        assert_eq!(budget.work(), 0);
        assert_eq!(owner.phase, Phase::Terminal);
        assert_eq!(owner.failure, Some(Resource::Accounting));
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
}
