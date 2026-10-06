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
            let source = instances.owner().source_semantic();
            let representation = if references.plan.descriptor_root.is_some() {
                AddressSpace::Global
            } else {
                AddressSpace::Generic
            };
            let result_type = assignment.destination().ty();
            let original_site = SourceReferenceSiteV29 {
                instance: instances.root(),
                block,
                statement: Some(0),
            };
            assert!(!references.plan.loans.iter().any(|loan| loan.site == original_site),
                "same-type slice transport must not invent a local referent loan");
            let key = source_reference_selector_site_v29(instances.root(), site, place, 0);
            if references.plan.descriptor_root.is_some() {
                let retained = references.plan.descriptor_values.get(&key)
                    .expect("C1 must retain the exact original reborrow holder read");
                assert_eq!(retained.ty, result_type);
                let descriptor = references.plan.descriptor_sets[retained.descriptor
                    .expect("the original kernel ABI supplies this descriptor")];
                assert_eq!((descriptor.representation, descriptor.has_unknown, descriptor.count),
                    (AddressSpace::Global, false, 1));
                assert_eq!(references.plan.descriptor_origins[descriptor.first],
                    SourceDescriptorOriginV29 { original_argument: 0, data_component: 0, length_component: 1 });
            } else {
                assert!(references.plan.descriptor_values.get(&key).is_none(),
                    "an ordinary Generic carrier must not acquire a kernel-ABI origin");
            }
            with_canonical_call_scratch_v1(budget, |budget| {
                let legacy = ExecutionCfgV29::new(
                    source.types(), function, cursor.ssa, &cursor.occurrences, budget,
                )?;
                assert!(!legacy.reference_locals[1]);
                assert!(!legacy.reference_locals[destination.index() as usize]);
                drop(legacy);
                let query = |source_place: &SemanticPlaceV1, types: &[SemanticTypeDeclV1], budget: &mut ArgumentBudgetV1<'_>| {
                    source_slice_reborrow_representation_v29(
                        &cursor, function, types, source.callables(), site, result_type, source_place, budget,
                    )
                };
                assert_eq!(query(place, source.types(), budget)?, representation);
                let copied = place.clone();
                for refusal in [
                    query(&copied, source.types(), budget),
                    query(place, &source.types().to_vec(), budget),
                ] {
                    assert!(matches!(refusal,
                        Err(ProductionSemanticKirErrorV1::Unsupported {
                            function: 0, block: None, statement: None,
                            detail: "source runtime slice descriptor/index/extent correspondence differs",
                        })
                    ));
                }
                assert_eq!(query(place, source.types(), budget)?, representation);
                Ok(())
            })?;
            assert_eq!(cursor.events.pending, pending);
            assert!(!cursor.claimed[source_index] && !cursor.claimed[destination_index]);
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

fn prepare_slice_reborrow_profile_v29(
    write: bool,
    kernel_abi: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<ProductionPreparedSourceV18, EntranceError> {
    with_pending_api_owner_v18(
        ModuleFixture::Ordinary,
        false,
        budget,
        || slice_reborrow_source_owner_v29(write),
        |owner, launch, input, _, budget| {
            assert_eq!(
                owner.source_semantic().functions()[0]
                    .abi()
                    .source_argument_ownership()[0],
                if write {
                    SemanticSourceArgumentOwnershipV1::UniqueBorrow
                } else {
                    SemanticSourceArgumentOwnershipV1::SharedBorrow
                }
            );
            if kernel_abi {
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
            } else {
                ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
                    owner,
                    launch,
                    input,
                    ProductionSemanticKirLimitsV1::default(),
                    budget,
                )
            }
        },
    )
}

fn slice_reborrow_profile_probe_v29(
    write: bool,
    kernel_abi: bool,
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
        let prepared = prepare_slice_reborrow_profile_v29(write, kernel_abi, &mut budget)
            .unwrap_or_else(|error| panic!("write={write}, kernel_abi={kernel_abi}: {error:?}"));
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
            let expected_space = if kernel_abi {
                AddressSpace::Global
            } else {
                AddressSpace::Generic
            };
            let actual = &canonical.module().functions[root];
            for (position, ty) in actual.signature.parameters[..2].iter().enumerate() {
                assert!(matches!(ty, Type::Slice(slice)
                    if slice.address_space == expected_space));
                let mut expected = lower_parameter_type(
                    semantic.types(),
                    semantic.callables(),
                    original.abi().source_input_types()[position],
                )
                .unwrap();
                let Type::Slice(slice) = &mut expected else {
                    panic!("original slice");
                };
                slice.address_space = expected_space;
                let id = actual.body.as_ref().unwrap().parameters[position];
                let binding = SemanticValueBindingV1::Value { id, ty: ty.clone() };
                check_source_slice_reborrow_binding_v29(&expected, &binding).unwrap();
                for fault in 0..3 {
                    let mut forged = ty.clone();
                    let Type::Slice(slice) = &mut forged else {
                        unreachable!()
                    };
                    match fault {
                        0 => {
                            slice.address_space = if kernel_abi {
                                AddressSpace::Generic
                            } else {
                                AddressSpace::Global
                            }
                        }
                        1 => {
                            slice.access = match slice.access {
                                AccessMode::ReadOnly => AccessMode::ReadWrite,
                                AccessMode::ReadWrite | AccessMode::WriteOnly => {
                                    AccessMode::ReadOnly
                                }
                            }
                        }
                        2 => *slice.element = Type::Scalar(ScalarType::U64),
                        _ => unreachable!(),
                    }
                    assert_ne!(&forged, ty);
                    assert!(matches!(
                        check_source_slice_reborrow_binding_v29(
                            &expected,
                            &SemanticValueBindingV1::Value { id, ty: forged }
                        ),
                        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                    ));
                }
                check_source_slice_reborrow_binding_v29(&expected, &binding).unwrap();
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
        let denied_work = budget.failed_work();
        budget
            .release_storage(budget.storage().checked_sub(MODULE_FLOOR).unwrap())
            .unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
        assert_eq!(
            (budget.failed_work(), budget.failed_storage()),
            (denied_work, denied_storage)
        );
        DESCRIPTOR_RUN_COMPLETED.set(true);
        (result, used, peak, denied_storage)
    };
    (result, used, peak, work.failed_work(), denied_storage)
}

#[test]
fn source_same_type_slice_reborrow_consumes_original_use_before_definition_and_replays() {
    for (write, kernel_abi) in [(false, true), (false, false), (true, false)] {
        slice_reborrow_profile_probe_v29(
            write,
            kernel_abi,
            DescriptorFault::SliceReborrowSourceAudit,
            None,
        )
        .0
        .unwrap_or_else(|error| panic!("write={write}, kernel_abi={kernel_abi}: {error:?}"));
        descriptor_resource_assertions_completed(true);
        assert!(DESCRIPTOR_EMITTED.get() > 0);
        assert_eq!(DESCRIPTOR_TAMPERED.get(), 0);
    }
}

#[test]
fn source_same_type_slice_reborrow_has_exact_and_one_short_source_resources() {
    for (write, kernel_abi) in [(false, true), (false, false), (true, false)] {
        let run = |allowance| {
            slice_reborrow_profile_probe_v29(write, kernel_abi, DescriptorFault::None, allowance)
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
            ArgumentResourceV1::Work(error)
                if Some(error.actual()) == short_work.3 && error.limit() == MODULE_LIMIT
        ));
        assert!(short_work.3.is_some());
        let short_storage = run(Some((work, peak - 1)));
        descriptor_resource_assertions_completed(false);
        assert!(matches!(
            entrance_resource(short_storage.0.unwrap_err()),
            ArgumentResourceV1::Storage(error)
                if Some(error.actual()) == short_storage.4 && error.limit() == MODULE_LIMIT
        ));
        assert!(short_storage.4.is_some());
    }
}

#[test]
fn source_same_type_slice_reborrow_kernel_profile_rejects_unowned_mutable_descriptor() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let error = prepare_slice_reborrow_profile_v29(true, true, &mut budget)
        .err()
        .expect("UniqueBorrow cannot become an ExclusiveOwner kernel descriptor");
    assert!(
        matches!(
            &error,
            EntranceError::Source(ProductionPendingScopedSourceErrorV29::Source(
                ProductionSemanticKirErrorV1::Unsupported {
                    function: 0,
                    block: None,
                    statement: None,
                    detail: "kernel argument ABI profile differs from the complete original descriptor/source contract",
                }
            ))
        ),
        "{error:?}"
    );
    assert_eq!(budget.storage(), MODULE_FLOOR);
    assert_eq!(
        (budget.failed_work(), budget.failed_storage()),
        (None, None)
    );
}

#[test]
fn source_same_type_slice_reborrow_generic_profile_preserves_exact_representation_refusal() {
    // The shared type checker sees actual source-bound physical parameters,
    // refuses each changed representation/access/element, then accepts restore.
    for write in [false, true] {
        slice_reborrow_profile_probe_v29(write, false, DescriptorFault::None, None)
            .0
            .unwrap();
        descriptor_resource_assertions_completed(true);
        assert!(DESCRIPTOR_EMITTED.get() > 0);
        assert_eq!(DESCRIPTOR_TAMPERED.get(), 0);
    }
}

#[test]
fn source_same_type_slice_reborrow_generic_profile_consumes_original_use_and_replays() {
    for write in [false, true] {
        slice_reborrow_profile_probe_v29(
            write,
            false,
            DescriptorFault::SliceReborrowSourceAudit,
            None,
        )
        .0
        .unwrap_or_else(|error| panic!("write={write}: {error:?}"));
        descriptor_resource_assertions_completed(true);
        assert!(DESCRIPTOR_EMITTED.get() > 0);
        assert_eq!(DESCRIPTOR_TAMPERED.get(), 0);
    }
}

#[test]
fn source_same_type_slice_reborrow_representation_query_has_independent_fixed_headers() {
    use std::mem::size_of;
    let expected = size_of::<AddressSpace>()
        + 2 * size_of::<Result<AddressSpace, ProductionSemanticKirErrorV1>>()
        + size_of::<Type>()
        + 2 * size_of::<Result<Type, ProductionSemanticKirErrorV1>>()
        + size_of::<Type>()
        + size_of::<usize>()
        + 2 * size_of::<Result<usize, ProductionSemanticKirErrorV1>>()
        + size_of::<ExecutionSiteV29>()
        + size_of::<SemanticTypeIdV1>()
        + size_of::<&ExecutionAvailabilityV29<'_>>()
        + size_of::<&SourceReferenceEmissionV29<'_, '_>>()
        + size_of::<&SemanticFunctionDeclV1>()
        + size_of::<&[SemanticTypeDeclV1]>()
        + size_of::<&[SemanticCallableDeclV1]>()
        + size_of::<&SemanticPlaceV1>()
        + size_of::<&mut dyn SemanticEmissionBudgetV1>()
        + size_of::<&Type>()
        + size_of::<&SemanticValueBindingV1>()
        + size_of::<Result<(), ProductionSemanticKirErrorV1>>();
    assert_eq!(
        source_slice_reborrow_representation_headers_v29().unwrap(),
        expected
    );
}
