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
    assert!(references.plan.descriptor_root.is_some());
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

fn slice_reborrow_kernel_abi_probe_v29(
    write: bool,
    fault: DescriptorFault,
    allowance: Option<(usize, usize)>,
) -> (
    Result<(), EntranceError>,
    usize,
    usize,
    Option<usize>,
    Option<usize>,
) {
    let _observers = DescriptorObservers::install(fault);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let (result, used, peak, denied_storage) = {
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = with_pending_api_owner_v18(
            ModuleFixture::Ordinary,
            false,
            &mut budget,
            || slice_reborrow_source_owner_v29(write),
            |owner, launch, input, _, budget| {
                let fixture = kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(&owner);
                let roots = fixture.roots();
                ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
                    owner,
                    launch,
                    input,
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    ProductionSemanticKirLimitsV1::default(),
                    budget,
                )
                .unwrap()
            },
        );
        let retained = prepared.adopted_storage();
        budget
            .reserve_storage(budget.peak_storage() + 1 - budget.storage())
            .unwrap();
        if let Some((left_work, left_storage)) = allowance {
            budget
                .charge_work(MODULE_LIMIT - budget.work() - left_work)
                .unwrap();
            budget
                .reserve_storage(MODULE_LIMIT - budget.storage() - left_storage)
                .unwrap();
        }
        let entry = budget.storage();
        let before = budget.work();
        let result = prepared.with_checked_source_v18(&mut budget, |view, budget| {
            let semantic = view.source_semantic(budget)?;
            let original = &semantic.functions()[0];
            for &ty in &original.abi().source_input_types()[..2] {
                let SemanticTypeShapeV1::Pointer(pointer) =
                    semantic.types()[ty.index() as usize].shape()
                else {
                    panic!("source slice");
                };
                assert_eq!(pointer.address_space(), 0);
            }
            let canonical = view.canonical(budget)?;
            let (_, root) = view.root(0, budget)?;
            for ty in &canonical.module().functions[root].signature.parameters[..2] {
                assert!(matches!(ty, Type::Slice(slice)
                    if slice.address_space == AddressSpace::Global));
            }
            let identity = *view.owner.pending_identity();
            let floor = budget.storage();
            view.owner.replay_with_budget(budget)?;
            assert_eq!(view.owner.pending_identity(), &identity);
            assert_eq!(budget.storage(), floor);
            Ok(())
        });
        assert_eq!(budget.storage(), entry - retained);
        let used = budget.work() - before;
        let peak = budget.peak_storage() - entry;
        let denied_storage = budget.failed_storage();
        budget.release_storage(budget.storage() - MODULE_FLOOR);
        assert_eq!(budget.storage(), MODULE_FLOOR);
        DESCRIPTOR_RUN_COMPLETED.set(true);
        (result, used, peak, denied_storage)
    };
    (result, used, peak, work.failed_work(), denied_storage)
}

#[test]
fn source_same_type_slice_reborrow_consumes_original_use_before_definition_and_replays() {
    for write in [false, true] {
        slice_reborrow_kernel_abi_probe_v29(write, DescriptorFault::SliceReborrowSourceAudit, None)
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
        let run = |allowance| {
            slice_reborrow_kernel_abi_probe_v29(write, DescriptorFault::None, allowance)
        };
        let (result, work, peak, denied_work, denied_storage) = run(None);
        result.unwrap();
        descriptor_resource_assertions_completed(true);
        assert!(work > 0 && peak > 0);
        assert_eq!((denied_work, denied_storage), (None, None));
        let exact = run(Some((work, peak)));
        exact.0.unwrap();
        assert_eq!(
            (exact.1, exact.2, exact.3, exact.4),
            (work, peak, None, None)
        );
        descriptor_resource_assertions_completed(true);
        let short_work = run(Some((work - 1, peak)));
        descriptor_resource_assertions_completed(false);
        assert!(matches!(
            entrance_resource(short_work.0.unwrap_err()),
            ArgumentResourceV1::Work(_)
        ));
        assert!(short_work.3.is_some());
        let short_storage = run(Some((work, peak - 1)));
        descriptor_resource_assertions_completed(false);
        assert!(matches!(
            entrance_resource(short_storage.0.unwrap_err()),
            ArgumentResourceV1::Storage(_)
        ));
        assert!(short_storage.4.is_some());
    }
}

#[test]
fn source_same_type_slice_reborrow_generic_profile_preserves_exact_representation_refusal() {
    // Unprofiled AS0 descriptors are Generic. The existing same-type slice
    // reborrow lowering expects Global; source admission alone is not lowering.
    for write in [false, true] {
        let result = run_descriptor_owner_module(
            slice_reborrow_source_owner_v29(write),
            DescriptorCase {
                write,
                ..DescriptorCase::READ
            },
            DescriptorFault::None,
            MODULE_LIMIT,
            MODULE_LIMIT,
        );
        assert!(matches!(
            result.0,
            Err(ScopedModuleErrorV29::Source(
                ProductionSemanticKirErrorV1::CorrespondenceMismatch
            ))
        ));
        descriptor_resource_assertions_completed(false);
        assert_eq!(DESCRIPTOR_TAMPERED.get(), 0);
    }
}
