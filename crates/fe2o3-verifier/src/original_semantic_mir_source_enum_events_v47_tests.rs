use super::super::super::slots::{SourceTagFixtureV40 as Fixture, source_tag_fixture_v40};
use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

#[path = "original_semantic_mir_source_enum_calls_v50_tests.rs"]
mod calls;

#[path = "original_semantic_mir_source_enum_spill_lifetimes_v50_tests.rs"]
mod spill_lifetimes;

const LIMIT: usize = 256 * 1024 * 1024;

fn logical_fixture(
    types: &mut Vec<SemanticTypeDeclV1>,
    functions: &mut Vec<SemanticFunctionDeclV1>,
    fixture: Fixture,
) {
    source_tag_fixture_v40(types, functions, fixture);
    let function = functions.last_mut().unwrap();
    let mut removed = 0;
    let blocks = function
        .blocks()
        .iter()
        .map(|block| {
            let statements = block.statements().iter().filter(|statement| {
            let remove = matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment)
                if matches!(assignment.value().kind(), SemanticRvalueKindV1::AddressOf { place, .. }
                    if place.local().index() == 4));
            removed += usize::from(remove);
            !remove
        }).cloned().collect();
            SemanticBasicBlockV1::new(
                block.identity(),
                block.source(),
                statements,
                block.terminator().clone(),
            )
            .unwrap()
        })
        .collect();
    assert_eq!(removed, 1);
    *function = SemanticFunctionDeclV1::new(
        function.identity(),
        function.role(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        function.source(),
        function.abi().clone(),
        function.locals().to_vec(),
        function.entry(),
        blocks,
    )
    .unwrap();
}

fn joined_fixture(
    types: &mut Vec<SemanticTypeDeclV1>,
    functions: &mut Vec<SemanticFunctionDeclV1>,
    fixture: Fixture,
) {
    assert!(matches!(fixture, Fixture::Direct | Fixture::SignedDirect));
    logical_fixture(types, functions, fixture);
    let function = functions.last_mut().unwrap();
    assert_eq!(function.blocks().len(), 1);
    let original = &function.blocks()[0];
    let source = original.source();
    let constructor = original
        .statements()
        .iter()
        .position(|statement| {
            matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment)
            if assignment.destination().local().index() == 4
                && matches!(assignment.value().kind(), SemanticRvalueKindV1::Aggregate(_)))
        })
        .expect("one original enum constructor");
    let SemanticStatementKindV1::Assign(assignment) = original.statements()[constructor].kind()
    else {
        unreachable!()
    };
    let enumeration = assignment.destination().ty();
    let word = function.locals()[1].ty();
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    let branch = |variant, operand| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(4, enumeration),
                SemanticRvalueV1::new(
                    enumeration,
                    SemanticRvalueKindV1::Aggregate(
                        SemanticAggregateRvalueV1::new(
                            SemanticAggregateKindV1::EnumVariant(variant),
                            vec![SemanticOperandV1::Copy(place(operand, word))],
                        )
                        .unwrap(),
                    ),
                ),
            )),
        )
    };
    let block = |identity, statements, terminator| {
        SemanticBasicBlockV1::new(
            identity,
            source,
            statements,
            SemanticTerminatorV1::new(source, terminator),
        )
        .unwrap()
    };
    let blocks = vec![
        block(
            original.identity(),
            original.statements()[..constructor].to_vec(),
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: SemanticOperandV1::Copy(place(1, word)),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        edge(SemanticEdgeRoleV1::SwitchValue, 1),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                )
                .unwrap(),
            },
        ),
        block(
            SemanticBlockIdentityV1::from_sha256([243; 32]),
            vec![branch(0, 1)],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
        ),
        block(
            SemanticBlockIdentityV1::from_sha256([244; 32]),
            vec![branch(1, 2)],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
        ),
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([245; 32]),
            source,
            original.statements()[constructor + 1..].to_vec(),
            original.terminator().clone(),
        )
        .unwrap(),
    ];
    assert!(
        blocks
            .windows(2)
            .all(|pair| pair[0].identity() < pair[1].identity())
    );
    *function = SemanticFunctionDeclV1::new(
        function.identity(),
        function.role(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        function.source(),
        function.abi().clone(),
        function.locals().to_vec(),
        function.entry(),
        blocks,
    )
    .unwrap();
}

fn run(
    fixture: Fixture,
    work: usize,
    storage: usize,
    examine: impl Fn(&InvocationPlan<'_, '_>, &SourceSlots<'_, '_>, &mut Writer<'_, '_>) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    super::super::super::super::invocations::tests::run_source_transform(
        work,
        storage,
        |types, functions| logical_fixture(types, functions, fixture),
        |plan, out| {
            super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                examine(plan, slots, out)
            })
        },
    )
}

#[test]
fn original_logical_enums_derive_from_promoted_original_locals_without_object_backing() {
    for fixture in [
        Fixture::Direct,
        Fixture::SignedDirect,
        Fixture::SharedNull,
        Fixture::MutableNull,
    ] {
        let reached = std::cell::Cell::new(false);
        run(fixture, LIMIT, LIMIT, |plan, slots, out| {
            reached.set(true);
            let semantic = slots.correspondence(out)?.source(out.budget)?.source_semantic(out.budget)?;
            let mut counts = [[0usize; 3]; 2];
            for root in 0..2 {
                for instance in 0..plan.root(root, out)?.instances.len() {
                    let row = plan.instance(root, instance, out)?;
                    if !row.active { continue }
                    let function = &semantic.functions()[row.function.index() as usize];
                    let body = SourceByteBody::derive(plan, slots, root, instance, out)?;
                    for (block_ordinal, block) in function.blocks().iter().enumerate() {
                        for (statement_ordinal, statement) in block.statements().iter().enumerate() {
                            match body.event_at(block_ordinal, statement_ordinal, out)? {
                                Event::LogicalEnum(LogicalEvent::Construct { destination, ty, variant, count, .. }) => {
                                    let Statement::Assign(assignment) = statement.kind() else { panic!("original constructor") };
                                    let Rvalue::Aggregate(aggregate) = assignment.value().kind() else { panic!("original aggregate") };
                                    assert_eq!(destination, row.locals.start + 4);
                                    assert_eq!(ty, assignment.destination().ty());
                                    assert_eq!(variant, 0);
                                    assert_eq!(count, aggregate.operands().len());
                                    assert!(!slots.has_original_object(root, instance, 4, out)?);
                                    assert!(slots.legacy_descriptor_by_source(root, instance, 4, out)?.is_none());
                                    counts[root][0] += 1;
                                }
                                Event::LogicalEnum(LogicalEvent::Discriminant { destination, input, .. }) => {
                                    assert!(matches!(statement.kind(), Statement::Assign(a) if matches!(a.value().kind(), Rvalue::Discriminant(_))));
                                    assert_eq!(destination, row.locals.start + 5);
                                    assert_eq!(input, row.locals.start + 4);
                                    counts[root][1] += 1;
                                }
                                Event::LogicalEnum(LogicalEvent::Reset { .. }) => counts[root][2] += 1,
                                Event::EnumConstruct(_) | Event::Discriminant(_) => panic!("promoted enum cannot acquire source object storage"),
                                _ => {}
                            }
                        }
                    }
                    body.emit(out)?;
                }
            }
            assert_eq!(counts, [[2, 2, 4], [2, 2, 4]]);
            assert!(out.text.contains("InvocationSourceLogicalEnumEventV47::Construct"));
            assert!(out.text.contains("InvocationSourceLogicalEnumEventV47::Discriminant"));
            Ok(())
        }).0.unwrap();
        assert!(reached.get(), "genuine owner callback must run");
    }
}

fn joined_program(
    fixture: Fixture,
    work: usize,
    storage: usize,
) -> (Result<()>, usize, usize, usize) {
    super::super::super::super::invocations::tests::run_source_transform(
        work,
        storage,
        |types, functions| joined_fixture(types, functions, fixture),
        |plan, out| {
            super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                let relation = slots.correspondence(out)?;
                let inventory = relation.inventory(out.budget)?;
                let original = relation.source(out.budget)?.source_semantic(out.budget)?;
                let mut spills = [0usize; 2];
                for root in 0..2 {
                    let count = relation.enum_spill_count_v48(root, out.budget)?;
                    assert_eq!(
                        count, 4,
                        "two helpers each retain the two original variant fields"
                    );
                    for ordinal in 0..count {
                        let receipt = relation.enum_spill_v48(root, ordinal, out.budget)?;
                        let origin = receipt.origin(out.budget)?;
                        let (definition, operation) = receipt.allocation(out.budget)?;
                        let row = plan.instance(root, origin.instance as usize, out)?;
                        assert!(row.active);
                        assert_eq!(origin.local, 4);
                        assert_eq!(origin.variant, (ordinal % 2) as u32);
                        assert_eq!((origin.field, origin.component), (0, 0));
                        assert_eq!(
                            origin.source_type,
                            original.functions()[row.function.index() as usize].locals()[4].ty()
                        );
                        assert!(!slots.has_original_object(
                            root,
                            origin.instance as usize,
                            4,
                            out
                        )?);
                        assert!(
                            slots
                                .legacy_descriptor_by_source(
                                    root,
                                    origin.instance as usize,
                                    4,
                                    out
                                )?
                                .is_none()
                        );
                        let coordinate = inventory.operations()[operation].coordinate;
                        let super::super::super::slots::AllocationOrigin::CompilerSpill(spill) =
                            slots.allocation_origin(coordinate, out)?
                        else {
                            panic!("compiler storage cannot acquire an original frame descriptor")
                        };
                        assert_eq!(spill.definition, definition);
                        assert_eq!(spill.operation, coordinate);
                        assert_eq!(spill.origin, origin);
                        spills[root] += 1;
                    }
                }
                assert_eq!(spills, [4, 4]);
                let mut program = super::super::super::source_function::SourceByteProgram::derive(
                    plan, slots, out,
                )?;
                let paired = super::super::super::paired::PairedInvocations::derive(
                    plan,
                    &program,
                    fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                    out,
                )?;
                let bindings =
                    super::super::super::byte_bindings::SourceByteBindings::derive(slots, out)?;
                slots.emit(out)?;
                program.emit(out)?;
                bindings.emit(out)?;
                let start = out.text.len();
                paired.emit(out)?;
                let generated = &out.text[start..];
                assert!(
                    generated.contains("match invocation_source_enum_local_v47(source,"),
                    "paired cuts must consume a real enum carrier"
                );
                assert!(
                    generated.contains("match invocation_enum_spill_read_v49(target,"),
                    "join payloads must be checked through their authenticated compiler storage"
                );
                assert!(generated.contains("value.fields.contains_key(0)"));
                assert!(generated.contains("value.variant == 0"));
                assert!(generated.contains("value.variant == 1"));
                assert!(!generated.contains("assume("));
                Ok(())
            })
        },
    )
}

#[test]
fn original_promoted_enum_joins_bind_original_tags_and_compiler_spills_on_both_roots() {
    for fixture in [Fixture::Direct, Fixture::SignedDirect] {
        joined_program(fixture, LIMIT, LIMIT).0.unwrap();
    }
}

#[test]
fn original_promoted_enum_complete_join_has_exact_and_one_short_resource_boundaries() {
    let measured = joined_program(Fixture::Direct, LIMIT, LIMIT);
    measured.0.unwrap();
    let exact = joined_program(Fixture::Direct, measured.1, measured.3);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.3), (measured.1, measured.3));
    for (work, storage, work_short) in [
        (measured.1 - 1, measured.3, true),
        (measured.1, measured.3 - 1, false),
    ] {
        let error = joined_program(Fixture::Direct, work, storage)
            .0
            .unwrap_err();
        let mut chain: &(dyn std::error::Error + 'static) = &error;
        let resource = loop {
            if let Some(resource) = chain.downcast_ref::<Resource>() {
                break resource;
            }
            chain = chain
                .source()
                .unwrap_or_else(|| panic!("missing resource: {error:?}"));
        };
        match resource {
            Resource::Work(limit) if work_short => {
                assert_eq!(limit.limit(), work);
                assert_eq!(limit.actual(), measured.1);
            }
            Resource::Storage(limit) if !work_short => {
                assert_eq!(limit.limit(), storage);
                assert_eq!(limit.actual(), measured.3);
            }
            other => panic!("wrong resource: {other:?}"),
        }
    }
}

#[test]
fn original_compiler_spill_queries_have_exact_work_and_sticky_one_short_refusal() {
    for field_query in [false, true] {
        // Exercise the owner query, the first raw lookup debit, and one-short.
        let cost = 1 + 2 + 4 + 1 + usize::from(!field_query);
        for remaining in [0, 3, cost - 1] {
            let ready = std::cell::Cell::new(false);
            let attacked = std::cell::Cell::new(false);
            let result = super::super::super::super::invocations::tests::run_source_transform(
                LIMIT,
                LIMIT,
                |types, functions| joined_fixture(types, functions, Fixture::Direct),
                |plan, out| {
                    super::super::super::source_function::tests::with_slots(
                        plan,
                        out,
                        |slots, out| {
                            let relation = slots.correspondence(out)?;
                            let mut frames = 0;
                            for root in 0..2 {
                                assert_eq!(relation.enum_spill_count_v48(root, out.budget)?, 4);
                                relation.visit_allocation_frames_v32(
                                    root,
                                    out.budget,
                                    |_, _| {
                                        frames += 1;
                                        Ok(())
                                    },
                                )?;
                            }
                            assert_eq!(
                                frames, 0,
                                "compiler slots never become source frame descriptors"
                            );
                            let receipt = relation.enum_spill_v48(0, 0, out.budget)?;
                            let origin = receipt.origin(out.budget)?;
                            let inventory = relation.inventory(out.budget)?;
                            let operation = inventory
                                .operations()
                                .iter()
                                .find(|row| {
                                    matches!(
                                        row.operation.kind,
                                        fe2o3_kernel_ir::OperationKind::Alloca { .. }
                                    )
                                })
                                .expect("genuine compiler allocations")
                                .coordinate;
                            let key = [
                                0,
                                origin.instance,
                                origin.local as usize,
                                origin.variant as usize,
                                origin.field as usize,
                                origin.component,
                            ];
                            let query =
                                |out: &mut Writer<'_, '_>| -> Result<()> {
                                    if field_query {
                                        assert_eq!(
                                            slots
                                                .compiler_spill_field(key, out)?
                                                .expect("exact nominal field")
                                                .origin,
                                            origin
                                        );
                                    } else {
                                        assert!(matches!(
                                slots.allocation_origin(operation, out)?,
                                super::super::super::slots::AllocationOrigin::CompilerSpill(_)
                            ));
                                    }
                                    Ok(())
                                };
                            // The source owner query costs one and consistency costs two.
                            // The first of eight rows uses four comparisons plus equality;
                            // allocation also probes the empty original-frame index once.
                            let before = (
                                out.budget.work(),
                                out.budget.storage(),
                                out.budget.peak_storage(),
                            );
                            query(out)?;
                            assert_eq!(out.budget.work() - before.0, cost);
                            assert_eq!(
                                (out.budget.storage(), out.budget.peak_storage()),
                                (before.1, before.2)
                            );
                            ready.set(true);
                            out.budget
                                .charge_work(LIMIT - out.budget.work() - remaining)?;
                            let error = query(out).expect_err("underfunded query must refuse");
                            let mut chain: &(dyn std::error::Error + 'static) = &error;
                            let resource = loop {
                                if let Some(resource) = chain.downcast_ref::<Resource>() {
                                    break *resource;
                                }
                                chain = chain
                                    .source()
                                    .unwrap_or_else(|| panic!("missing resource: {error:?}"));
                            };
                            assert!(
                                matches!(resource, Resource::Work(limit) if limit.limit() == LIMIT && limit.actual() == LIMIT + 1)
                            );
                            let after = (
                                out.budget.work(),
                                out.budget.storage(),
                                out.budget.peak_storage(),
                            );
                            assert!(
                                matches!(query(out), Err(Error::Source(fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Resource(repeated))) if repeated == resource)
                            );
                            assert_eq!(
                                (
                                    out.budget.work(),
                                    out.budget.storage(),
                                    out.budget.peak_storage()
                                ),
                                after
                            );
                            attacked.set(true);
                            Err(error)
                        },
                    )
                },
            );
            assert!(ready.get(), "positive query did not run: {:?}", result.0);
            assert!(
                attacked.get(),
                "intended query attack did not run: {:?}",
                result.0
            );
            assert!(
                matches!(result.0, Err(Error::Source(fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Resource(Resource::Work(limit)))) if limit.limit() == LIMIT && limit.actual() == LIMIT + 1)
            );
        }
    }
}

#[test]
fn original_logical_enum_event_headers_have_an_independent_fixed_envelope() {
    type Local = (usize, TypeId);
    #[allow(dead_code)]
    enum EventFields {
        Construct {
            destination: usize,
            ty: TypeId,
            variant: u32,
            first: usize,
            count: usize,
        },
        Transfer {
            destination: usize,
            input: usize,
            ty: TypeId,
            moved: bool,
        },
        Extract {
            destination: usize,
            input: usize,
            ty: TypeId,
            variant: u32,
            field: u32,
            moved: bool,
        },
        Discriminant {
            destination: usize,
            input: usize,
            ty: TypeId,
            bits: u32,
        },
        FieldDeinitialize {
            input: usize,
            ty: TypeId,
            variant: u32,
            field: u32,
        },
        Reset {
            local: usize,
        },
    }
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    assert_eq!(size_of::<EnumLocal>(), size_of::<Local>());
    let expected = h::<EventFields>()
        + h::<Option<EventFields>>()
        + h::<Local>()
        + h::<Option<Local>>()
        + h::<(u32, u32, EnumFieldV47)>()
        + 10 * size_of::<usize>()
        + 8 * size_of::<&()>();
    assert_eq!(headers(), expected);
}

#[test]
fn original_logical_enum_runtime_keeps_tag_definedness_move_order_and_escape_census() {
    let runtime = include_str!("original_semantic_mir_source_enum_values_v47.vrs");
    assert!(
        runtime.contains("InvocationSourceEnumV47 { fields: value.fields.remove(field), ..value }")
    );
    assert!(runtime.contains("!moved && !invocation_source_enum_type_copyable_v47(source_type)"));
    let transfer = runtime
        .split("spec fn invocation_source_enum_transfer_v47")
        .nth(1)
        .unwrap()
        .split("spec fn invocation_source_logical_enum_step_v47")
        .next()
        .unwrap();
    assert!(
        transfer.find("let consumed = if moved").unwrap()
            < transfer
                .find("invocation_source_enum_install_v47(consumed")
                .unwrap()
    );
    let logical = include_str!("original_semantic_mir_source_logical_locals_v38.vrs");
    assert!(logical.contains("enums: logical.enums.remove(local)"));
    let frames = include_str!("original_semantic_mir_invocation_source_frames_v36.rs");
    assert!(frames.contains(
        "invocation_source_value_escapes_frame_v36(logical.enums[local].fields[field], frame)"
    ));
    for effects in [
        include_str!("original_semantic_mir_invocation_effects_v36.rs"),
        include_str!("original_semantic_mir_observed_effects_v39.vrs"),
    ] {
        assert!(effects.contains("Some(InvocationSourceByteEventV36::LogicalEnum(event))"));
        assert!(
            effects.contains("invocation_source_logical_enum_step_v47(observation.before, event")
        );
    }
}

#[test]
fn original_logical_enum_payloads_participate_in_ordinary_lifetime_end_after_local_clear() {
    let source = include_str!("original_semantic_mir_invocation_source_bytes_v36.rs");
    let end = source
        .split("spec fn invocation_source_byte_end_v36")
        .nth(1)
        .unwrap()
        .split("spec fn invocation_source_object_end_v40")
        .next()
        .unwrap();
    let clear = end
        .find("let cleared = invocation_source_byte_put_local_v36")
        .unwrap();
    let logical = end
        .find("!invocation_source_enums_name_allocation_v49(cleared.logical, pointer.allocation)")
        .unwrap();
    let release = end
        .find("byte_end_lifetime_v30(cleared.machine.memory, pointer.allocation)")
        .unwrap();
    assert!(clear < logical && logical < release);
    assert!(end.contains(
        "!invocation_memory_names_allocation_v37(cleared.machine.memory, pointer.allocation)"
    ));
    assert!(end.contains(
        "!invocation_value_names_allocation_v36(cleared.machine.values[i], pointer.allocation)"
    ));
    let runtime = include_str!("original_semantic_mir_source_enum_values_v47.vrs");
    assert!(runtime.contains("logical.enums[local].fields.contains_key(field)"));
    assert!(runtime.contains(
        "invocation_value_names_allocation_v36(logical.enums[local].fields[field], allocation)"
    ));
    assert!(runtime.contains("proof fn invocation_source_enum_reference_blocks_lifetime_end_v49"));
    assert!(
        runtime.contains("proof fn invocation_source_enum_clear_drops_only_its_own_provenance_v49")
    );
}
