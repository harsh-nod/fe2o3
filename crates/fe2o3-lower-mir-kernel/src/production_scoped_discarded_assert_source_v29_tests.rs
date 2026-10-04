thread_local! {
    static ASSERTION_PHYSICAL_COMPLETED_V29: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static ASSERTION_REPEATED_LOCAL_V29: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

fn repeated_assertion_operand_owner_v29(
    owner: ProductionSemanticSsaOwnerV1,
) -> ProductionSemanticSsaOwnerV1 {
    let mode = ASSERTION_REPEATED_LOCAL_V29.get();
    if mode == 0 {
        return owner;
    }
    let semantic = owner.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let original = &functions[3];
    let SemanticTerminatorKindV1::Assert {
        condition,
        expected,
        target,
        unwind,
        ..
    } = original.blocks()[0].terminator().kind()
    else {
        panic!("original assertion");
    };
    let second = if mode == 1 {
        SemanticOperandV1::Copy(place(2, U32))
    } else {
        SemanticOperandV1::Move(place(2, U32))
    };
    let assertion = SemanticTerminatorKindV1::Assert {
        condition: condition.clone(),
        expected: *expected,
        message: SemanticAssertMessageV1::BoundsCheck {
            length: SemanticOperandV1::Copy(place(2, U32)),
            index: second,
        },
        target: target.clone(),
        unwind: *unwind,
    };
    functions[3] = function(
        130,
        original.role(),
        original.abi().clone(),
        original.locals().to_vec(),
        vec![
            block(140, original.blocks()[0].statements().to_vec(), assertion),
            block(
                141,
                original.blocks()[1].statements().to_vec(),
                SemanticTerminatorKindV1::Return,
            ),
        ],
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
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

fn run_original_assertion_v29(
    fixture: ScopedFixture,
    observer: ScopedSlotObserverV29,
    work_limit: usize,
    storage_limit: usize,
) -> (SourceOwnedResultV18<()>, usize, usize) {
    let ScopedFixture::AssertionSlots {
        move_condition,
        move_message,
        reinitialize,
    } = fixture
    else {
        panic!("original assertion fixture required");
    };
    let factory = || {
        repeated_assertion_operand_owner_v29(super::super::fixtures::assertion_slots_owner(
            move_condition,
            move_message,
            reinitialize,
        ))
    };
    let projection = factory();
    let owner = factory();
    assert_eq!(
        owner.source_semantic_sha256(),
        projection.source_semantic_sha256()
    );
    assert_eq!(owner.identity(), projection.identity());
    assert!(owner.occurrence_storage().is_none());
    let semantic = projection.source_semantic();
    let launch = ProductionSourceLaunchRosterV1::try_new(
        owner.source_semantic(),
        &[ProductionSourceLaunchRootInputV1::new(
            "lifecycle_fixture",
            [88; 32],
            ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [2, 1, 1]),
        )],
    )
    .unwrap();
    let roots = [root_input(&projection)];
    let classes = [
        ProductionScopeCallableCandidateV29::Ordinary,
        ProductionScopeCallableCandidateV29::Provider {
            function: HELPER,
            identity: semantic.functions()[1].identity(),
        },
        ProductionScopeCallableCandidateV29::Ordinary,
        ProductionScopeCallableCandidateV29::Ordinary,
        ProductionScopeCallableCandidateV29::Ordinary,
        ProductionScopeCallableCandidateV29::Derive {
            binding: SemanticFunctionIdentityV1::from_sha256([121; 32]),
            operation: SemanticCompilerIntrinsicIdentityV1::from_sha256([121; 32]),
            context: CONTEXT,
            workgroup: semantic.functions()[2].abi().source_input_types()[0],
        },
    ];
    let SemanticTerminatorKindV1::Call(derive) =
        semantic.functions()[1].blocks()[0].terminator().kind()
    else {
        panic!("original derive call");
    };
    let events = [
        (
            ROOT,
            1,
            0,
            ProductionScopeEventKindV29::Call {
                callee: SemanticCallableIdV1::from_index(1),
                kind: ProductionScopeCallKindV29::Provider,
            },
        ),
        (
            HELPER,
            0,
            semantic.functions()[1].blocks()[0].statements().len(),
            ProductionScopeEventKindV29::Call {
                callee: derive.callee(),
                kind: ProductionScopeCallKindV29::Derive,
            },
        ),
        (
            HELPER,
            1,
            0,
            ProductionScopeEventKindV29::Call {
                callee: SemanticCallableIdV1::from_index(2),
                kind: ProductionScopeCallKindV29::Ordinary,
            },
        ),
        (HELPER, 2, 0, ProductionScopeEventKindV29::Return),
    ]
    .map(
        |(function, block, statement_count, kind)| crate::ProductionScopeEventCandidateV29 {
            function,
            block: SemanticBlockIdV1::from_index(block),
            statement_count,
            kind,
        },
    );
    let _observer = ObserverGuard::install(observer);
    OBSERVED.set(0);
    ASSERTION_PHYSICAL_COMPLETED_V29.set(false);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let result = (|| -> SourceOwnedResultV18<()> {
        let prepared = ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
            owner,
            launch,
            ProductionExecutionSourceInputV29 {
                semantic_sha256: projection.source_semantic_sha256(),
                roots: &roots,
                classes: &classes,
                events: &events,
            },
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )?;
        prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
                source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                    scoped_raw_admission_v29::with_checked_source_memory_v29(relation, 0, None, budget,
                        |physical, budget| -> SourceOwnedResultV18<()> {
                            let mut failures = 0;
                            physical.visit_effects(budget, |effect, budget| {
                                if let scoped_raw_admission_v29::PendingSourceMemoryEffectV29::FailureRead { instance, .. } = effect {
                                    source.instance(0, instance.index(), budget)?;
                                    failures += 1;
                                }
                                Ok(())
                            })?;
                            assert_eq!(failures, 4, "two ordered diagnostic reads in each original helper");
                            let root = source.root(0, budget)?.1;
                            let body = inventory.functions()[root].function.body.as_ref().unwrap();
                            let mut alias_reads = [0; 3];
                            for (block, body) in body.blocks.iter().enumerate() {
                                for (operation, value) in body.operations.iter().enumerate() {
                                    let OperationKind::Storage(ScopedObjectOperationV29::ReadValue { address, .. }) = value.kind else { continue; };
                                    let coordinate = fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
                                        block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
                                            function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(root as u32), block: block as u32,
                                        }, operation: operation as u32,
                                    };
                                    let payload = relation.retained_object_payload_v29(0, coordinate, budget)?
                                        .expect("actual typed read keeps its original source attachment");
                                    let ScopedObjectRoleV29::ReadValue { source: endpoint, .. } = payload.source.role else { panic!("read recipe"); };
                                    let ScopedObjectIdentityV29::Reference { .. } = endpoint.object else { continue; };
                                    let ScopedObjectSourceV29::Place { local, prefix: 1, .. } = endpoint.source else { panic!("original alias dereference"); };
                                    let ordinal = usize::try_from(local.index() - 5).unwrap();
                                    assert!(ordinal < 3);
                                    let access = physical.access(payload.instance, payload.row, coordinate, address, budget)?
                                        .expect("each original raw alias read has checked activation alternatives");
                                    assert_eq!(access.operation_pointer(budget)?, (coordinate, address));
                                    let mut alternatives = 0;
                                    access.visit_alternatives(budget, |instance, local, slot, _, budget| {
                                        source.instance(0, instance, budget)?;
                                        assert_eq!(local.index(), ordinal as u32 + 2);
                                        let origin = &source.root_row(0)?.source_slots.slots[slot];
                                        assert_eq!(origin.instance.index(), instance);
                                        assert_eq!(origin.origin.identity.original_local(), Some(local.index()));
                                        alternatives += 1;
                                        Ok(())
                                    })?;
                                    assert!(alternatives > 0);
                                    alias_reads[ordinal] += 1;
                                }
                            }
                            assert_eq!(alias_reads, [2, 2, 2]);
                            ASSERTION_PHYSICAL_COMPLETED_V29.set(true);
                            Ok(())
                        })
                })
            }))
        })
    })();
    assert_eq!(budget.storage(), FLOOR, "{fixture:?}: {result:?}");
    (result, budget.work(), budget.peak_storage())
}

fn assert_original_assertion_completed_v29(result: &SourceOwnedResultV18<()>) {
    assert!(result.is_ok(), "{result:?}");
    assert!(
        ASSERTION_PHYSICAL_COMPLETED_V29.get(),
        "final physical admission must execute"
    );
    assert_eq!(
        OBSERVED.get(),
        3,
        "inspect admission, reconstruction, and consuming source replay"
    );
}

fn assertion_source_resource_v29(error: ProductionSourceOwnedViewErrorV18) -> ArgumentResourceV1 {
    use ProductionPendingScopedSourceErrorV29 as Pending;
    use ProductionSourceOwnedViewErrorV18 as View;
    use fe2o3_kernel_ir::{
        BorrowedKernelIrVerificationErrorV1 as V, CanonicalKernelIrReplayAdmissionErrorV18 as C,
        KernelIrDecodeError as D, KernelIrEncodeError as E, StorageLayoutErrorV1 as L,
    };
    match error {
        View::Resource(error)
        | View::Source(Pending::Source(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error),
        ))
        | View::Source(Pending::Source(ProductionSemanticKirErrorV1::AssertOrigin(
            SemanticKirAssertOriginErrorV1::Resource(error),
        )))
        | View::Source(Pending::Canonical(C::Resource(error)))
        | View::Source(Pending::Canonical(C::Decode(D::Resource(error))))
        | View::Source(Pending::Canonical(C::Layout(L::Resource(error))))
        | View::Source(Pending::Canonical(C::Verification(V::Resource(error))))
        | View::Source(Pending::Occurrences(
            fe2o3_pliron::ProductionSemanticSsaOccurrenceErrorV1::Resource(error),
        )) => error,
        View::Source(Pending::Canonical(C::Encode(E::WorkLimit(limit))))
        | View::Source(Pending::Canonical(C::Decode(D::WorkLimit(limit))))
        | View::Source(Pending::Canonical(C::Decode(D::Encode(E::WorkLimit(limit))))) => {
            ArgumentResourceV1::Work(limit)
        }
        other => panic!("exact original-source resource refusal required: {other:?}"),
    }
}

fn with_original_assertion_cursor_v29(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    instance: ProductionCallInstanceIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
    consume: impl for<'cursor> FnOnce(
        ExecutionAvailabilityV29<'cursor>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    with_scoped_source_test_layouts_v29(
        source,
        ProductionSemanticKirLimitsV1::default(),
        budget,
        |demands, layouts, budget| {
            let demands = demands.root_lens(source.owner, 0, budget)?;
            let descriptor = source
                .kernel_argument_abi
                .map(|profile| profile.descriptor_root(source.owner, 0, budget))
                .transpose()?;
            source_storage_v29::with_source_storage_descriptor_demands_root_v29(
                layouts,
                instances,
                descriptor,
                demands,
                budget,
                |plan, _, budget| {
                    let references = SourceReferenceEmissionV29::new(plan, budget)?;
                    let result = with_source_reference_availability_v29(
                        instances,
                        instance,
                        Some(&references),
                        budget,
                        consume,
                    );
                    let cleanup = references.abort_scope(instances, budget);
                    result.and(cleanup).map_err(Into::into)
                },
            )
        },
    )
}

fn with_original_assertion_frame_v29(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    instance: ProductionCallInstanceIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
    consume: impl for<'cursor> FnOnce(
        ExecutionAvailabilityV29<'cursor>,
        &'cursor ExecutionInstanceLayoutsV29<'cursor, 'cursor>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    with_scoped_source_test_layouts_v29(
        source,
        ProductionSemanticKirLimitsV1::default(),
        budget,
        |demands, layouts, budget| {
            let demands = demands.root_lens(source.owner, 0, budget)?;
            let descriptor = source
                .kernel_argument_abi
                .map(|profile| profile.descriptor_root(source.owner, 0, budget))
                .transpose()?;
            source_storage_v29::with_source_storage_descriptor_demands_root_v29(
                layouts,
                instances,
                descriptor,
                demands,
                budget,
                |plan, _, budget| {
                    with_execution_instance_layouts_v29(plan, budget, |signatures, budget| {
                        let references = SourceReferenceEmissionV29::new(plan, budget)?;
                        let result = with_source_reference_availability_v29(
                            instances,
                            instance,
                            Some(&references),
                            budget,
                            |cursor, budget| consume(cursor, signatures, budget),
                        );
                        let cleanup = references.abort_scope(instances, budget);
                        result.and(cleanup)
                    })
                    .map_err(Into::into)
                },
            )
        },
    )
}
