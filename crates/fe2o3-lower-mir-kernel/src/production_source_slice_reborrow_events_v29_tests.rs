fn slice_reborrow_source_owner_v29(write: bool) -> ProductionSemanticSsaOwnerV1 {
    let base = descriptor_source_owner(DescriptorCase {
        write,
        ..DescriptorCase::READ
    });
    let source = base.source_semantic();
    let original = &source.functions()[0];
    let pointer = original.locals()[1].ty();
    let SemanticTypeShapeV1::Pointer(pointer_shape) =
        source.types()[pointer.index() as usize].shape()
    else {
        panic!("the original source parameter must be a slice reference");
    };
    assert_eq!(
        pointer_shape.metadata(),
        SemanticPointerMetadataV1::SliceLength
    );
    let slice = pointer_shape.pointee();
    let word = original.locals()[3].ty();
    let mut locals = original.locals().to_vec();
    let borrowed = u32::try_from(locals.len()).unwrap();
    locals.push(local(229, pointer, SemanticLocalRoleV1::Temporary));
    locals.push(local(230, word, SemanticLocalRoleV1::Temporary));
    let dereference = |holder| {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(holder),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, slice).unwrap()],
            slice,
        )
        .unwrap()
    };
    let mut first = vec![
        assign(
            place(borrowed, pointer),
            SemanticRvalueKindV1::Borrow {
                kind: if write {
                    SemanticBorrowKindV1::Mutable
                } else {
                    SemanticBorrowKindV1::Shared
                },
                place: dereference(1),
            },
        ),
        assign(
            place(borrowed + 1, word),
            SemanticRvalueKindV1::Length(dereference(borrowed)),
        ),
    ];
    first.extend_from_slice(original.blocks()[0].statements());
    let mut blocks = original.blocks().to_vec();
    blocks[0] = SemanticBasicBlockV1::new(
        blocks[0].identity(),
        blocks[0].source(),
        first,
        blocks[0].terminator().clone(),
    )
    .unwrap();
    let root = function(
        231,
        SemanticFunctionRoleV1::KernelRoot,
        original.abi().clone(),
        locals,
        blocks,
    )
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        source.types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![root],
        source.callables().to_vec(),
        source.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn assert_slice_reborrow_event_refusal_v29<T>(result: Result<T, ProductionSemanticKirErrorV1>) {
    assert!(matches!(
        result,
        Err(ProductionSemanticKirErrorV1::Unsupported {
            function: 0,
            block: None,
            statement: None,
            detail: "execution availability differs from its source SSA instance",
        })
    ));
}

fn audit_slice_reborrow_source_events_v29(
    references: &SourceReferenceEmissionV29<'_, '_>,
    instances: &ExecutionInstancesV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) {
    let floor = budget.storage();
    with_source_reference_availability_v29(
        instances,
        instances.root(),
        Some(references),
        budget,
        |mut cursor, budget| {
            let block = SemanticBlockIdV1::from_index(0);
            let site = execution_site_v29(block, Some(0));
            let function = cursor.function;
            let SemanticStatementKindV1::Assign(assignment) =
                function.blocks()[0].statements()[0].kind()
            else {
                panic!("missing original slice reborrow assignment");
            };
            let SemanticRvalueKindV1::Borrow { place, .. } = assignment.value().kind() else {
                panic!("missing original slice reborrow source");
            };
            let destination = assignment.destination().local();
            assert_eq!(place.local().index(), 1);
            assert_eq!(cursor.cfg.nominal_locals[1], 0);
            assert!(cursor.cfg.reference_locals[1]);
            assert!(cursor.cfg.reference_locals[destination.index() as usize]);
            cursor.begin_block(block, budget)?;
            let source_index = cursor.event(
                site,
                ExecutionOperandV29::RvaluePlace,
                ExecutionEventV29::BaseUse,
                budget,
            )?;
            let destination_index = cursor.event(
                site,
                ExecutionOperandV29::Destination,
                ExecutionEventV29::DestinationDefine,
                budget,
            )?;
            assert_eq!(
                &cursor.events.required[..2],
                &[source_index, destination_index],
                "the original source use must precede destination definition"
            );
            let Some(SsaResolvedEventV1::Use {
                value: original, ..
            }) = cursor.occurrences.events()[source_index].resolved()
            else {
                panic!("source reborrow use lacks its original SSA definition");
            };
            let Some(SsaResolvedEventV1::Define { value: defined, .. }) =
                cursor.occurrences.events()[destination_index].resolved()
            else {
                panic!("source reborrow destination lacks its original SSA definition");
            };
            let pending = cursor.events.pending.clone();
            // The actual emitter must claim the use; a valid destination cannot skip it.
            assert_slice_reborrow_event_refusal_v29(cursor.define(
                site,
                destination,
                defined,
                budget,
            ));
            assert_eq!(cursor.events.pending, pending);
            assert!(!cursor.claimed[source_index] && !cursor.claimed[destination_index]);
            assert_eq!(
                cursor.use_place(site, ExecutionOperandV29::RvaluePlace, place, false, budget)?,
                original
            );
            cursor.check_claimed_original_use_v29(
                site,
                ExecutionOperandV29::RvaluePlace,
                place,
                original,
                budget,
            )?;
            let cloned = place.clone();
            assert_slice_reborrow_event_refusal_v29(cursor.check_claimed_original_use_v29(
                site,
                ExecutionOperandV29::RvaluePlace,
                &cloned,
                original,
                budget,
            ));
            assert_slice_reborrow_event_refusal_v29(cursor.use_place(
                site,
                ExecutionOperandV29::RvaluePlace,
                place,
                false,
                budget,
            ));
            cursor.define(site, destination, defined, budget)?;
            assert!(cursor.claimed[source_index] && cursor.claimed[destination_index]);
            assert_eq!(cursor.current[destination.index() as usize], Some(defined));
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn source_same_type_slice_reborrow_consumes_original_use_before_definition_and_replays() {
    for write in [false, true] {
        let case = DescriptorCase {
            write,
            ..DescriptorCase::READ
        };
        run_descriptor_owner_module(
            slice_reborrow_source_owner_v29(write),
            case,
            DescriptorFault::SliceReborrowSourceAudit,
            MODULE_LIMIT,
            MODULE_LIMIT,
        )
        .0
        .unwrap_or_else(|error| panic!("write={write}: {error:?}"));
        descriptor_resource_assertions_completed(true);
        assert!(DESCRIPTOR_EMITTED.get() > 0);
        assert_eq!(DESCRIPTOR_TAMPERED.get(), 0);
    }
}

#[test]
fn source_same_type_slice_reborrow_has_exact_and_one_short_source_resources() {
    for write in [false, true] {
        let case = DescriptorCase {
            write,
            ..DescriptorCase::READ
        };
        let run = |work, storage| {
            run_descriptor_owner_module(
                slice_reborrow_source_owner_v29(write),
                case,
                DescriptorFault::None,
                work,
                storage,
            )
        };
        let (result, work, peak) = run(MODULE_LIMIT, MODULE_LIMIT);
        result.unwrap();
        descriptor_resource_assertions_completed(true);
        assert!(work > 0 && peak > MODULE_FLOOR);
        run(work, peak).0.unwrap();
        descriptor_resource_assertions_completed(true);
        let work_error = run(work - 1, peak).0.unwrap_err();
        descriptor_resource_assertions_completed(false);
        descriptor_resource_error(work_error, true);
        let storage_error = run(work, peak - 1).0.unwrap_err();
        descriptor_resource_assertions_completed(false);
        descriptor_resource_error(storage_error, false);
    }
}
