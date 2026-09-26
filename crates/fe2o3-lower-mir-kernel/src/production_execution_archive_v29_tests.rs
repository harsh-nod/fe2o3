fn archive_key(local: u32) -> SsaValueV1 {
    SsaValueV1::BlockArgument {
        block: SsaBlockIdV1::new(3),
        variable: fe2o3_mir_model::SsaVariableIdV1::new(local),
    }
}

fn archive_object_pointer_v29(id: u32, schema: u32, access: AccessMode) -> SemanticValueBindingV1 {
    SemanticValueBindingV1::Value {
        id: ValueId(id),
        ty: Type::pointer(Type::StorageObject(fe2o3_kernel_ir::StorageLayoutIdV1(schema)), AddressSpace::Private, access),
    }
}

fn archive_object_place_v29(field: bool, dereference: bool, suffix: bool) -> SemanticPlaceV1 {
    let ty = SemanticTypeIdV1::from_index(1);
    let mut projections = Vec::new();
    if field { projections.push(SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), ty).unwrap()); }
    if dereference { projections.push(SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty).unwrap()); }
    if suffix { projections.push(SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), ty).unwrap()); }
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0), projections, ty).unwrap()
}

#[test]
fn archive_object_pointer_holder_identity_covers_whole_dereferenced_and_nested_values() {
    for access in [AccessMode::ReadOnly, AccessMode::ReadWrite] {
        for field in [false, true] {
            for (dereference, suffix) in [(false, false), (true, false), (true, true)] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
                let mut budget = ArgumentBudgetV1::new(&mut work, 31);
                budget.reserve_storage(31).unwrap();
                let value = archive_object_pointer_v29(61, 7, access);
                let held = if field { SemanticValueBindingV1::Aggregate(vec![value]) } else { value };
                let archive = SemanticSsaBindingsV1::from([(archive_key(0), held.clone())]);
                let locals = [Some(held)];
                check_execution_archive_v29(&locals, &archive, &archive_object_place_v29(field, dereference, suffix), archive_key(0), &mut budget).unwrap();
                assert_eq!(budget.storage(), 31, "archive identity has no retained allocation or permission grant");
            }
        }
    }
}

#[test]
fn archive_object_pointer_refuses_changed_value_schema_access_and_missing_origin() {
    for dereference in [false, true] {
        for mutation in 0..10 {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
            let mut budget = ArgumentBudgetV1::new(&mut work, 19);
            budget.reserve_storage(19).unwrap();
            let original = archive_object_pointer_v29(61, 7, AccessMode::ReadWrite);
            let mut archive = SemanticSsaBindingsV1::from([(archive_key(0), original.clone())]);
            let mut locals = [Some(original)];
            match mutation {
                0 => locals[0] = Some(archive_object_pointer_v29(62, 7, AccessMode::ReadWrite)),
                1 => locals[0] = Some(archive_object_pointer_v29(61, 8, AccessMode::ReadWrite)),
                2 => locals[0] = Some(archive_object_pointer_v29(61, 7, AccessMode::ReadOnly)),
                3 => locals[0] = Some(SemanticValueBindingV1::Value { id: ValueId(61), ty: Type::pointer(
                    Type::StorageObject(fe2o3_kernel_ir::StorageLayoutIdV1(7)), AddressSpace::Generic, AccessMode::ReadWrite) }),
                4 => locals[0] = Some(SemanticValueBindingV1::Value { id: ValueId(61), ty: Type::Scalar(ScalarType::U32) }),
                5 => { archive.clear(); },
                6 => locals[0] = None,
                7 => { let value = archive.remove(&archive_key(0)).unwrap(); archive.insert(archive_key(1), value); },
                8 => locals[0] = Some(SemanticValueBindingV1::Unit),
                9 => locals[0] = Some(SemanticValueBindingV1::MovedExecution),
                _ => unreachable!(),
            }
            assert!(check_execution_archive_v29(&locals, &archive, &archive_object_place_v29(false, dereference, false), archive_key(0), &mut budget).is_err(),
                "dereference {dereference}, mutation {mutation}");
            assert_eq!(budget.storage(), 19);
        }
    }
}

#[test]
fn archive_object_pointer_does_not_admit_generic_scalar_or_writeonly_pointer_shapes() {
    for (pointee, address_space, access) in [
        (Type::StorageObject(fe2o3_kernel_ir::StorageLayoutIdV1(7)), AddressSpace::Generic, AccessMode::ReadWrite),
        (Type::StorageObject(fe2o3_kernel_ir::StorageLayoutIdV1(7)), AddressSpace::Global, AccessMode::ReadWrite),
        (Type::StorageObject(fe2o3_kernel_ir::StorageLayoutIdV1(7)), AddressSpace::Private, AccessMode::WriteOnly),
        (Type::Scalar(ScalarType::U32), AddressSpace::Private, AccessMode::ReadWrite),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        let value = SemanticValueBindingV1::Value { id: ValueId(61), ty: Type::pointer(pointee, address_space, access) };
        let archive = SemanticSsaBindingsV1::from([(archive_key(0), value.clone())]);
        assert!(check_execution_archive_v29(&[Some(value)], &archive, &archive_object_place_v29(false, true, false), archive_key(0), &mut budget).is_err());
    }
}

#[test]
fn archive_object_pointer_identity_has_independent_exact_work_and_no_storage() {
    // One-row archive lookup (2), one projection (1), fixed holder checks (6),
    // and both two-node structural pointer types (4).
    let required = 2 + 1 + 6 + 4;
    for limit in [required, required - 1] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, 23);
        budget.reserve_storage(23).unwrap();
        let value = archive_object_pointer_v29(61, 7, AccessMode::ReadWrite);
        let archive = SemanticSsaBindingsV1::from([(archive_key(0), value.clone())]);
        let result = check_execution_archive_v29(&[Some(value)], &archive, &archive_object_place_v29(false, true, false), archive_key(0), &mut budget);
        if limit == required { result.unwrap(); assert_eq!(budget.work(), required); }
        else { assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(_))))); }
        assert_eq!(budget.storage(), 23);
    }
}

#[test]
fn archive_storage_exact_and_one_short_are_independent_of_capacity() {
    use std::mem::size_of;
    // Empty map: two conservative split nodes, each with 32 tuple slots.
    // Slots hold pointers, and the one actual binding has one owned box.
    // Its pointer type adds one separate boxed Type, not another binding table.
    let nodes = 2 * 32 * size_of::<(SsaValueV1, Box<SemanticValueBindingV1>, usize)>();
    let backing = nodes + size_of::<SemanticValueBindingV1>();
    let storage = backing + size_of::<Type>();
    // Duplicate lookup + split-path lookup, then binding, pointer and scalar.
    let work_needed = 2 * (2 * 16) + 3;
    for (work_limit, storage_limit, denial) in [
        (work_needed, storage, None),
        (work_needed - 1, storage, Some("work")),
        (work_needed, storage - 1, Some("storage")),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, 19 + storage_limit);
        budget.reserve_storage(19).unwrap();
        let input = SemanticValueBindingV1::Value {
            id: ValueId(801),
            ty: Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            ),
        };
        let mut map = SemanticSsaBindingsV1::default();
        let mut credit = None;
        let result =
            archive_owned_binding_v29(&mut map, &mut credit, archive_key(2), &input, &mut budget);
        match denial {
            None => {
                result.unwrap();
                assert_eq!(map.len(), 1);
                assert!(
                    matches!(&map[&archive_key(2)], SemanticValueBindingV1::Value { id: ValueId(801), ty } if *ty == Type::pointer(Type::Scalar(ScalarType::U32), AddressSpace::Private, AccessMode::ReadWrite))
                );
                assert_eq!(credit.unwrap().bytes, storage);
                assert_eq!(budget.storage(), 19 + storage);
                assert_eq!(budget.work(), work_needed);
            }
            Some("work") => {
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Work(_)
                        )
                    )
                ));
                assert!(map.is_empty());
                assert_eq!(budget.storage(), 19 + backing);
                assert_eq!(credit.unwrap().bytes, backing);
            }
            Some("storage") => {
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Storage(_)
                        )
                    )
                ));
                assert!(map.is_empty());
                assert_eq!(budget.storage(), 19 + backing);
                assert_eq!(credit.unwrap().bytes, backing);
            }
            _ => unreachable!(),
        }
        // The retained allowance cannot be refunded until the actual map dies.
        drop(map);
        budget.release_storage(credit.unwrap().bytes).unwrap();
        assert_eq!(budget.storage(), 19);
    }
}

#[test]
fn archive_box_and_map_nodes_are_prepaid_before_clone_or_publication() {
    use std::mem::size_of;
    let nodes = 2 * 32 * size_of::<(SsaValueV1, Box<SemanticValueBindingV1>, usize)>();
    let backing = nodes + size_of::<SemanticValueBindingV1>();
    for limit in [0, nodes, backing - 1] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 19 + limit);
        budget.reserve_storage(19).unwrap();
        let mut bindings = SemanticSsaBindingsV1::default();
        let mut credit = None;
        let result = archive_owned_binding_v29(
            &mut bindings,
            &mut credit,
            archive_key(0),
            &SemanticValueBindingV1::Unit,
            &mut budget,
        );
        assert!(matches!(
            result,
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(_)
            ))
        ));
        assert!(bindings.is_empty());
        assert!(credit.is_none());
        assert_eq!(budget.storage(), 19);
        assert_eq!(budget.work(), 2 * (2 * 16));
    }
}

#[test]
fn growing_boxed_archives_preserve_exact_definitions_and_independent_debits() {
    use std::mem::size_of;
    for count in [32u32, 64, 128] {
        let node = 32 * size_of::<(SsaValueV1, Box<SemanticValueBindingV1>, usize)>();
        let mut storage = 0;
        let mut required_work = 0;
        for previous in 0..count {
            let levels = previous.checked_ilog2().unwrap_or(0) as usize + 2;
            storage += levels * node + size_of::<SemanticValueBindingV1>();
            required_work += 2 * levels * 16 + 2;
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(required_work);
        let mut budget = ArgumentBudgetV1::new(&mut work, 19 + storage);
        budget.reserve_storage(19).unwrap();
        let mut bindings = SemanticSsaBindingsV1::default();
        let mut credit = None;
        for local in (0..count).rev() {
            archive_owned_binding_v29(
                &mut bindings,
                &mut credit,
                archive_key(local),
                &SemanticValueBindingV1::Value {
                    id: ValueId(local + 300),
                    ty: Type::Scalar(ScalarType::U32),
                },
                &mut budget,
            ).unwrap();
        }
        assert_eq!(bindings.len(), count as usize);
        assert_eq!(budget.work(), required_work);
        assert_eq!(budget.storage(), 19 + storage);
        assert_eq!(credit.unwrap().bytes, storage);
        for local in 0..count {
            assert!(matches!(
                bindings.get(&archive_key(local)),
                Some(SemanticValueBindingV1::Value {
                    id, ty: Type::Scalar(ScalarType::U32),
                }) if *id == ValueId(local + 300)
            ));
        }
        drop(bindings);
        budget.release_storage(credit.unwrap().bytes).unwrap();
        assert_eq!(budget.storage(), 19);
    }
}

#[test]
fn archive_duplicate_slot_ledger_and_lost_credit_refuse_without_publication() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 100_000);
    let mut map = SemanticSsaBindingsV1::default();
    let mut credit = None;
    archive_owned_binding_v29(
        &mut map,
        &mut credit,
        archive_key(1),
        &SemanticValueBindingV1::Unit,
        &mut budget,
    )
    .unwrap();
    let bytes = credit.unwrap().bytes;
    assert!(
        archive_owned_binding_v29(
            &mut map,
            &mut credit,
            archive_key(1),
            &SemanticValueBindingV1::Unit,
            &mut budget
        )
        .is_err()
    );
    assert_eq!(map.len(), 1);
    assert_eq!(budget.storage(), bytes);
    let original = credit.unwrap();
    let mut wrong_slot = original;
    wrong_slot.slot ^= 1;
    assert!(wrong_slot.check(&budget).is_err());
    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(100_000);
    let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, 100_000);
    foreign.reserve_storage(bytes).unwrap();
    assert!(original.check(&foreign).is_err());
    assert_eq!(foreign.storage(), bytes);
    budget.release_storage(1).unwrap();
    assert!(original.check(&budget).is_err());
    budget.reserve_storage(1).unwrap();
    original.check(&budget).unwrap();
    drop(map);
    budget.release_storage(bytes).unwrap();
}

#[test]
fn archive_definition_capture_requires_the_consumed_original_event() {
    let completed = std::cell::Cell::new(false);
    captured_execution(Flow::Linear, |instances, budget| {
        let instance = instances.calls(instances.root()).unwrap()[1]
            .child()
            .unwrap();
        with_execution_availability_v29(instances, instance, budget, |mut cursor, budget| {
            let block = SemanticBlockIdV1::from_index(0);
            cursor.begin_block(block, budget)?;
            let event = cursor
                .occurrences
                .events()
                .iter()
                .find(|event| {
                    event.role() == ExecutionEventV29::DestinationDefine
                        && event.resolved().is_some()
                })
                .unwrap();
            let site = event.site();
            let Some(SsaResolvedEventV1::Define { variable, value }) = event.resolved() else {
                panic!("not an original definition")
            };
            let mut map = SemanticSsaBindingsV1::default();
            let mut credit = None;
            let storage = budget.storage();
            let coordinate = ExecutionArchiveDefinitionSiteV29::Definition {
                site,
                local: variable.get(),
            };
            assert!(
                archive_scoped_binding_v29(
                    &cursor,
                    &mut map,
                    &mut credit,
                    value,
                    &SemanticValueBindingV1::Unit,
                    coordinate,
                    budget
                )
                .is_err()
            );
            assert!(map.is_empty());
            assert!(credit.is_none());
            assert_eq!(budget.storage(), storage);
            // Consume the real Move before the original destination definition.
            let source = place(1, CONTEXT);
            cursor.use_place(
                site,
                ExecutionOperandV29::RvalueOperand(0),
                &source,
                true,
                budget,
            )?;
            cursor.define(
                site,
                SemanticLocalIdV1::from_index(variable.get()),
                value,
                budget,
            )?;
            archive_scoped_binding_v29(
                &cursor,
                &mut map,
                &mut credit,
                value,
                &SemanticValueBindingV1::Unit,
                coordinate,
                budget,
            )?;
            assert_eq!(map.len(), 1);
            // This is a locator test: Unit was deliberately not certified as the
            // source Context's value. The final expression checker is separate.
            assert!(
                cursor
                    .check_archive_definition_v29(
                        value,
                        ExecutionArchiveDefinitionSiteV29::Definition {
                            site,
                            local: variable.get() + 1
                        },
                        budget
                    )
                    .is_err()
            );
            assert!(
                cursor
                    .check_archive_definition_v29(
                        value,
                        ExecutionArchiveDefinitionSiteV29::Definition {
                            site: execution_site_v29(SemanticBlockIdV1::from_index(0), Some(99)),
                            local: variable.get(),
                        },
                        budget,
                    )
                    .is_err()
            );
            drop(map);
            budget.release_storage(credit.unwrap().bytes)?;
            assert_eq!(budget.storage(), storage);
            completed.set(true);
            Ok(())
        })
        .unwrap();
    });
    assert!(completed.get());
}

#[test]
fn actual_archive_frame_exact_and_one_short_resources_complete_the_observer() {
    let observed = std::cell::Cell::new(0);
    let measured = lower_cfg_fixture_with_limits(
        Shape::Mixed,
        (10_000_000, 10_000_000),
        |_| {},
        |_| {},
        |_, _, result| {
            let result = result.unwrap();
            let archive = result.execution_observation.as_ref().unwrap();
            assert!(!archive.bindings.is_empty());
            assert!(archive.credit.bytes > std::mem::size_of::<ExecutionArchiveV29>());
            observed.set(observed.get() + 1);
        },
    )
    .unwrap();
    assert_eq!(observed.get(), 1);
    let exact = lower_cfg_fixture_with_limits(
        Shape::Mixed,
        measured,
        |_| {},
        |_| {},
        |_, _, result| {
            assert!(result.unwrap().execution_observation.is_some());
            observed.set(observed.get() + 1);
        },
    )
    .unwrap();
    assert_eq!(exact, measured);
    assert_eq!(observed.get(), 2);
    for storage_short in [false, true] {
        let checked = std::cell::Cell::new(false);
        let limits = if storage_short {
            (measured.0, measured.1 - 1)
        } else {
            (measured.0 - 1, measured.1)
        };
        let _ = lower_cfg_fixture_with_limits(
            Shape::Mixed,
            limits,
            |_| {},
            |_| {},
            |_, _, result| {
                assert!(
                    matches!(
                        &result,
                        Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(_))) if storage_short
                    ) || matches!(
                        &result,
                        Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(_))) if !storage_short
                    )
                );
                checked.set(true);
            },
        );
        // This must be outside the helpers' catch boundaries, after all inner assertions.
        assert!(
            checked.get(),
            "the original frame resource assertion did not complete"
        );
    }
}

#[test]
fn actual_frame_moves_one_archive_and_keeps_existing_observation_fields() {
    let completed = std::cell::Cell::new(false);
    lower_cfg_fixture(
        Shape::Mixed,
        |_| {},
        |owner, _, result| {
            let result = result.unwrap();
            let archive = result.execution_observation.as_ref().unwrap();
            assert_eq!(
                archive.subject.source.semantic,
                *owner.source_semantic_sha256()
            );
            assert_eq!(archive.subject.source.ssa, owner.identity());
            assert_eq!(
                archive.plan,
                owner.plan_for_function(ROOT).unwrap().plan().identity()
            );
            assert!(!archive.bindings.is_empty());
            assert!(!archive.locals.is_empty());
            assert!(archive.credit.bytes >= std::mem::size_of::<ExecutionArchiveV29>());
            let key = SsaValueV1::BlockArgument {
                block: SsaBlockIdV1::new(3),
                variable: fe2o3_mir_model::SsaVariableIdV1::new(4),
            };
            assert!(archive.bindings.contains_key(&key));
            completed.set(true);
        },
    );
    assert!(completed.get());
}

#[test]
fn archive_roster_requires_original_instance_even_for_same_function_siblings() {
    let completed = std::cell::Cell::new(false);
    captured_execution(Flow::Linear, |instances, budget| {
        let calls = instances.calls(instances.root()).unwrap();
        let first = calls[1].child().unwrap();
        let second = calls[3].child().unwrap();
        assert_ne!(first, second);
        assert_eq!(
            instances.instance(first).unwrap().function(),
            instances.instance(second).unwrap().function()
        );
        with_execution_availability_v29(instances, first, budget, |cursor, budget| {
            let floor = budget.storage();
            let header = std::mem::size_of::<ExecutionArchiveV29>();
            budget.reserve_storage(header)?;
            let mut credit = ExecutionArchiveCreditV29::new(budget)?;
            credit.bytes = header;
            // A custody-only empty-map candidate: it certifies no source value.
            let mut archive = ExecutionArchiveV29 {
                subject: ScopedInitializationSubjectV29::from_cursor(&cursor),
                plan: cursor.ssa.plan().identity(),
                credit,
                bindings: SemanticSsaBindingsV1::default(),
                locals: Vec::new(),
                retained_seeds: Vec::new(),
            };
            assert_eq!(
                check_execution_archive_instance_v29(
                    instances,
                    first,
                    Some(first),
                    Some(&archive),
                    budget
                )?,
                header
            );
            assert!(
                check_execution_archive_instance_v29(instances, first, Some(first), None, budget)
                    .is_err()
            );
            assert!(
                check_execution_archive_instance_v29(
                    instances,
                    first,
                    Some(second),
                    Some(&archive),
                    budget
                )
                .is_err()
            );
            assert!(
                check_execution_archive_instance_v29(
                    instances,
                    second,
                    Some(second),
                    Some(&archive),
                    budget
                )
                .is_err()
            );
            let saved = archive.subject.source;
            archive.subject.source.semantic[0] ^= 1;
            assert!(
                archive
                    .check_original_v29(instances, first, budget)
                    .is_err()
            );
            archive.subject.source = saved;
            archive.credit.slot ^= 1;
            assert!(
                archive
                    .check_original_v29(instances, first, budget)
                    .is_err()
            );
            archive.credit = credit;
            assert_eq!(budget.storage(), floor + header);
            drop(archive);
            budget.release_storage(header)?;
            assert_eq!(budget.storage(), floor);
            completed.set(true);
            Ok(())
        })
        .unwrap();
    });
    assert!(completed.get());
}

#[test]
fn archive_exact_refund_uses_original_source_plan_and_preserves_its_floor() {
    let completed = std::cell::Cell::new(false);
    captured_execution(Flow::Linear, |instances, budget| {
        with_source_reference_plan_v29(instances, budget, |plan, budget| {
            let floor = budget.storage();
            let mut bindings = SemanticSsaBindingsV1::default();
            let mut credit = None;
            archive_owned_binding_v29(
                &mut bindings,
                &mut credit,
                archive_key(1),
                &SemanticValueBindingV1::Unit,
                budget,
            )?;
            let owned = credit.unwrap();
            let required = budget.storage();
            assert!(budget.permits_prepared_input_refund_v1(
                Some(plan),
                owned.slot,
                owned.ledger,
                required,
                owned.bytes
            ));
            assert!(!budget.permits_prepared_input_refund_v1(
                Some(plan),
                owned.slot ^ 1,
                owned.ledger,
                required,
                owned.bytes
            ));
            let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(100_000);
            let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, required);
            foreign.reserve_storage(required)?;
            assert!(!foreign.permits_prepared_input_refund_v1(
                Some(plan),
                owned.slot,
                owned.ledger,
                required,
                owned.bytes
            ));
            assert_eq!(foreign.storage(), required);
            budget.release_storage(1)?;
            assert!(!budget.permits_prepared_input_refund_v1(
                Some(plan),
                owned.slot,
                owned.ledger,
                required,
                owned.bytes
            ));
            budget.reserve_storage(1)?;
            drop(bindings);
            budget.release_emission_service_storage_v1(
                Some(plan),
                owned.slot,
                owned.ledger,
                required,
                owned.bytes,
            )?;
            assert_eq!(budget.storage(), floor);
            completed.set(true);
            Ok(())
        })
        .unwrap();
    });
    assert!(completed.get());
}
