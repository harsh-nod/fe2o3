fn run_original_objects_v40(
    explicit: bool,
    work: usize,
    storage: usize,
    examine: impl FnOnce(
        &InvocationPlan<'_, '_>,
        &SourceSlots<'_, '_>,
        &mut Writer<'_, '_>,
    ) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    let execute = |plan: &mut InvocationPlan<'_, '_>, out: &mut Writer<'_, '_>| {
        let source = plan.source(out)?;
        let owner = source.canonical(out.budget)?;
        let (inventory, receipt) =
            super::super::super::super::Inventory::derive_v18(owner, out.budget)?;
        out.budget.reserve_storage(receipt.retained_storage())?;
        let result =
            source.with_ranked_correspondence_v18(&inventory, out.budget, |relation, budget| {
                let mut writer = Writer::new(budget)?;
                let slots = SourceSlots::derive(plan, relation, &mut writer)?;
                examine(plan, &slots, &mut writer)
            });
        drop(inventory);
        if result.is_ok() {
            out.budget.release_storage(receipt.retained_storage())?;
        }
        result
    };
    super::super::super::invocations::tests::run_original_object_variant_v41(
        explicit, work, storage, execute,
    )
}

#[test]
fn original_object_byte_events_follow_exact_storage_live_read_write_and_dead() {
    run_original_objects_v40(true, LIMIT, LIMIT, |plan, slots, out| {
        let original = plan.source(out)?.source_semantic(out.budget)?;
        for root in 0..2 {
            let instance = plan.instance(root, 0, out)?;
            let function = &original.functions()[instance.function.index() as usize];
            let statements = function.blocks()[0].statements();
            assert_eq!(statements.len(), 6);
            assert!(matches!(statements[0].kind(), Statement::StorageLive(local) if local.index() == 4));
            assert!(matches!(statements[1].kind(), Statement::Store(_)));
            assert!(matches!(statements[2].kind(), Statement::Assign(assignment)
                if matches!(assignment.value().kind(), Rvalue::AddressOf { .. })));
            assert!(matches!(statements[3].kind(), Statement::Assign(assignment)
                if matches!(assignment.value().kind(), Rvalue::Load(_))));
            assert!(matches!(statements[4].kind(), Statement::StorageDead(local) if local.index() == 5));
            assert!(matches!(statements[5].kind(), Statement::StorageDead(local) if local.index() == 4));
            let body = SourceByteBody::derive(plan, slots, root, 0, out)?;
            let local = instance.locals.start + 4;
            let scalar = ScalarV30::Integer { signed: false, width: 32 };
            assert!(matches!(body.event_at(0, 0, out)?, Event::ObjectLive {
                local: found, activation: 1, ..
            } if found == local));
            assert_eq!(body.event_at(0, 5, out)?, Event::ObjectDead { local });
            assert!(matches!(body.event_at(0, 2, out)?, Event::Address {
                destination, access: Access { address: Address::Object { local: found, offset: 0 }, bytes: 4, alignment: 4, .. },
            } if destination == instance.locals.start + 5 && found == local));
            assert_eq!(body.event_at(0, 4, out)?, Event::Scalar);
            assert!(matches!(body.event_at(0, 1, out)?, Event::Transfer {
                destination: Destination::Memory(Access {
                    address: Address::Object { local: found, offset: 0 },
                    bytes: 4, alignment: 4, ..
                }),
                value: Value::Local { local: input, moved: false }, scalar: found_scalar,
            } if found == local && input == instance.locals.start + 1 && found_scalar == scalar));
            assert!(matches!(body.event_at(0, 3, out)?, Event::Transfer {
                destination: Destination::Local(destination),
                value: Value::Read { access: Access {
                    address: Address::Object { local: found, offset: 0 },
                    bytes: 4, alignment: 4, ..
                }, moved: false }, scalar: found_scalar,
            } if found == local && destination == instance.locals.start + 1 && found_scalar == scalar));
            body.emit(out)?;
        }
        assert_eq!(out.text.matches("InvocationSourceByteEventV36::ObjectLive {").count(), 2);
        assert_eq!(out.text.matches("InvocationSourceByteEventV36::ObjectDead {").count(), 2);
        assert_eq!(out.text.matches("InvocationSourceByteBaseV36::ObjectLocal(").count(), 6);
        Ok(())
    })
    .0
    .unwrap();
}

fn emit_original_objects_v40(
    explicit: bool,
    plan: &InvocationPlan<'_, '_>,
    slots: &SourceSlots<'_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    slots.emit(out)?;
    let mut program = super::super::source_function::SourceByteProgram::derive(plan, slots, out)?;
    program.emit(out)?;
    assert_eq!(
        out.text
            .matches("fn invocation_source_object_binding_v40(")
            .count(),
        1
    );
    assert_eq!(out.text.matches("objects: Map::empty()").count(), 2);
    assert_eq!(
        out.text
            .matches("InvocationSourceByteBaseV36::ObjectLocal(")
            .count(),
        6
    );
    assert_eq!(
        out.text
            .matches("source.objects.contains_key(local)")
            .count(),
        6
    );
    if explicit {
        assert_eq!(
            out.text
                .matches("InvocationSourceByteEventV36::ObjectLive {")
                .count(),
            2
        );
        assert_eq!(
            out.text
                .matches("InvocationSourceByteEventV36::ObjectDead {")
                .count(),
            2
        );
    } else {
        assert_eq!(
            out.text
                .matches("let entered = invocation_source_object_activate_v40(")
                .count(),
            2
        );
        assert!(
            !out.text
                .contains("InvocationSourceByteEventV36::ObjectLive {")
        );
    }
    for root in 0..2 {
        let local = plan.instance(root, 0, out)?.locals.start + 4;
        let entry = slots.object_activation(root, 0, 4, 0, out)?.unwrap();
        let enter = out
            .text
            .split_once(&format!(
                "open spec fn invocation_source_enter_{root}_0_v36("
            ))
            .unwrap()
            .1
            .split_once("\n}\n")
            .unwrap()
            .0;
        assert_eq!(
            enter
                .matches("let entered = invocation_source_object_activate_v40(")
                .count(),
            usize::from(!explicit)
        );
        assert_eq!(enter.contains(&format!(
            "let entered = invocation_source_object_activate_v40(entered, {}, invocation_source_slot_{}_v36(), {local}, 0int, {root}, 0);",
            entry.descriptor, entry.descriptor)), !explicit);
        let restart = if explicit {
            Some(slots.object_activation(root, 0, 4, 1, out)?.unwrap())
        } else {
            None
        };
        let events = out
            .text
            .split_once(&format!(
                "open spec fn invocation_source_byte_event_{root}_0_v36("
            ))
            .unwrap()
            .1
            .split_once("\n}\n")
            .unwrap()
            .0;
        assert_eq!(
            events
                .matches("InvocationSourceByteEventV36::ObjectLive {")
                .count(),
            usize::from(explicit)
        );
        assert_eq!(
            events
                .matches("InvocationSourceByteEventV36::ObjectDead {")
                .count(),
            usize::from(explicit)
        );
        if let Some(restart) = restart {
            assert!(events.contains(&format!(
                "ObjectLive {{ descriptor: {}int, slot: invocation_source_slot_{}_v36(), local: {local}int, activation: 1int }}",
                restart.descriptor, restart.descriptor)));
            assert!(events.contains(&format!("ObjectDead {{ local: {local}int }}")));
        }
    }
    assert!(!out.text.contains("byte_target_view_contracts"));
    Ok(())
}

#[test]
fn original_object_lifetimes_are_consumed_by_complete_source_program_entry_and_statements() {
    for explicit in [false, true] {
        run_original_objects_v40(explicit, LIMIT, LIMIT, |plan, slots, out| {
            emit_original_objects_v40(explicit, plan, slots, out)
        })
        .0
        .unwrap();
    }
}

#[test]
fn original_object_dead_only_marker_does_not_activate_catalogued_entry_storage() {
    super::super::super::invocations::tests::run_original_object_dead_only_variant_v42(
        LIMIT, LIMIT, |plan, out| {
            super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                let original = plan.source(out)?.source_semantic(out.budget)?;
                let mut program = super::super::source_function::SourceByteProgram::derive(plan, slots, out)?;
                program.emit(out)?;
                for root in 0..2 {
                    let instance = plan.instance(root, 0, out)?;
                    let function = &original.functions()[instance.function.index() as usize];
                    assert!(!function.blocks().iter().flat_map(|block| block.statements()).any(|statement|
                        matches!(statement.kind(), Statement::StorageLive(local) if local.index() == 4)));
                    assert!(matches!(function.blocks()[0].statements()[4].kind(),
                        Statement::StorageDead(local) if local.index() == 4));
                    assert!(slots.object_activation(root, 0, 4, 0, out)?.is_some(),
                        "a catalogued Entry identity is not permission to activate it");
                    let body = SourceByteBody::derive(plan, slots, root, 0, out)?;
                    let local = instance.locals.start + 4;
                    assert_eq!(body.event_at(0, 4, out)?, Event::ObjectDead { local });
                    assert!(matches!(body.event_at(0, 2, out)?, Event::Transfer {
                        value: Value::Read { access: Access { address: Address::Object { local: found, .. }, .. }, .. }, ..
                    } if found == local));
                    let enter = out.text.split_once(&format!("open spec fn invocation_source_enter_{root}_0_v36("))
                        .unwrap().1.split_once("\n}\n").unwrap().0;
                    assert!(!enter.contains("invocation_source_object_activate_v40("));
                    assert!(!enter.contains("invocation_source_byte_activate_v36("));
                }
                let object_address = SOURCE_BYTES_V36.split_once("open spec fn invocation_source_byte_address_v36(")
                    .unwrap().1.split_once("InvocationSourceByteBaseV36::ObjectLocal(local) =>")
                    .unwrap().1.split_once("InvocationSourceByteBaseV36::Slot").unwrap().0;
                assert!(object_address.contains("if source.objects.contains_key(local)"));
                assert!(object_address.contains("} else { None }"));
                Ok(())
            })
        },
    ).0.unwrap();
}

#[test]
fn original_object_lifetime_source_program_has_exact_and_one_short_resources() {
    let execute = |work, storage| {
        run_original_objects_v40(true, work, storage, |plan, slots, out| {
            emit_original_objects_v40(true, plan, slots, out)
        })
    };
    let measured = execute(LIMIT, LIMIT);
    measured.0.unwrap();
    let exact = execute(measured.1, measured.3);
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3),
        (measured.1, measured.2, measured.3)
    );
    assert!(matches!(execute(measured.1 - 1, measured.3).0,
        Err(Error::Source(fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Resource(Resource::Work(error))))
            if error.actual() == measured.1 && error.limit() == measured.1 - 1));
    assert!(matches!(execute(measured.1, measured.3 - 1).0,
        Err(Error::Source(fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Resource(Resource::Storage(error))))
            if error.actual() == measured.3 && error.limit() == measured.3 - 1));
}

#[test]
fn original_object_runtime_authenticates_bindings_and_restarts_dynamic_generations() {
    let wf = SOURCE_BYTES_V36
        .split_once("open spec fn invocation_source_byte_state_well_formed_v36(")
        .unwrap()
        .1
        .split_once("\n}\n")
        .unwrap()
        .0;
    assert!(wf.contains("invocation_source_object_binding_v40(local, source.objects[local])"));
    assert!(wf.contains("source.slots.contains_key(source.objects[local].descriptor)"));
    assert!(wf.contains("source.objects[left].descriptor != source.objects[right].descriptor"));
    let activate = SOURCE_BYTES_V36
        .split_once("open spec fn invocation_source_object_activate_v40(")
        .unwrap()
        .1
        .split_once("\n}\n")
        .unwrap()
        .0;
    let authenticated = activate
        .find("invocation_source_object_binding_v40(local, object)")
        .unwrap();
    let ended = activate
        .find("invocation_source_object_end_v40(source, local, root, instance)")
        .unwrap();
    let fresh = activate
        .find(
            "invocation_source_byte_activate_v36(before, descriptor, slot, local, root, instance)",
        )
        .unwrap();
    let inserted = activate
        .find("objects: active.objects.insert(local, object)")
        .unwrap();
    assert!(authenticated < ended && ended < fresh && fresh < inserted);
    let end = SOURCE_BYTES_V36
        .split_once("open spec fn invocation_source_object_end_v40(")
        .unwrap()
        .1
        .split_once("\n}\n")
        .unwrap()
        .0;
    assert!(
        end.find("objects: source.objects.remove(local)").unwrap()
            < end
                .find("invocation_source_byte_end_v36(removed, object.descriptor")
                .unwrap()
    );
    let allocate = SOURCE_BYTES_V36
        .split_once("open spec fn invocation_source_byte_activate_v36(")
        .unwrap()
        .1
        .split_once("\n}\n")
        .unwrap()
        .0;
    assert!(
        allocate
            .contains("let generation = private_generation_v30(source.machine.generations, site)")
    );
    assert!(
        allocate.contains("generations: source.machine.generations.insert(site, generation + 1)")
    );
    assert!(!allocate.contains("generation: slot.source_generation"));
    let frames = super::super::source_frames::SOURCE_FRAMES_V36;
    assert!(frames.contains("objects: Map::new("));
    assert!(frames.contains("|local: int| source.objects.contains_key(local)"));
    assert!(frames.contains(
        "!byte_allocation_in_frame_v30(source.slots[source.objects[local].descriptor].allocation, frame)"
    ));
    let laws = include_str!("original_semantic_mir_source_object_lifetime_laws_v40.vrs");
    assert_eq!(laws.matches("proof fn original_object_").count(), 5);
    assert!(!laws.contains("assume("));
    assert!(!laws.contains("external_body"));
}
