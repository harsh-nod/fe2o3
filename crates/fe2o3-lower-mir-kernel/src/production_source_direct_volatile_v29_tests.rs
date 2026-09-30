thread_local! {
    static DIRECT_VOLATILE_LOOP_V29: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static DIRECT_VOLATILE_COMPLETED_V29: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn observe_direct_volatile_v29(
    original: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    assert!(is_stopped(&observe_initialization(
        original, instances, emitted, slots, budget
    )));
    let mut observed = 0;
    for (index, output) in emitted.iter().enumerate() {
        let instance = instances.id_at(index).unwrap();
        let source = instances.instance(instance).unwrap();
        if source.function().index() != 3 {
            continue;
        }
        let function = source.declaration();
        let SemanticStatementKindV1::Assign(assignment) =
            function.blocks()[3].statements()[0].kind()
        else {
            unreachable!()
        };
        let SemanticRvalueKindV1::Load(load) = assignment.value().kind() else {
            unreachable!()
        };
        let site = SourceReferenceSiteV29 {
            instance,
            block: SemanticBlockIdV1::from_index(3),
            statement: Some(0),
        };
        assert!(load.source().projections().is_empty());
        assert_eq!(load.source().local().index(), 2);
        for short in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(12 - usize::from(short));
            let mut exact = ArgumentBudgetV1::new(&mut work, 0);
            let result =
                check_source_direct_volatile_load_v29(instances, site, load, U32, &mut exact);
            if short {
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Work(_)
                        )
                    )
                ));
                assert_eq!(exact.work(), 0);
            } else {
                result.unwrap();
                assert_eq!(exact.work(), 12);
            }
            assert_eq!(exact.storage(), 0);
        }
        let clone = load.clone();
        for fault in 0..4 {
            let mut wrong_site = site;
            if fault == 1 {
                wrong_site.block = SemanticBlockIdV1::from_index(2);
            }
            if fault == 2 {
                wrong_site.statement = Some(usize::MAX);
            }
            let error = check_source_direct_volatile_load_v29(
                instances,
                wrong_site,
                if fault == 0 { &clone } else { load },
                if fault == 3 { UNIT } else { U32 },
                budget,
            )
            .unwrap_err();
            assert!(matches!(
                error,
                ProductionSemanticKirErrorV1::Unsupported {
                    detail: "source reference ordered load requires checked addressable effects",
                    ..
                }
            ));
        }
        let output = output.as_ref().unwrap();
        let anchors = output.scoped_memory_anchors.as_ref().unwrap();
        let occurrences = instances.occurrences(instance).unwrap();
        let body = output.function.body.as_ref().unwrap();
        let mut reads = 0;
        for row in &anchors.rows {
            let ScopedMemoryAnchorKindV29::Access {
                payload: Some(ScopedMemoryPayloadV29::Load { read, .. }),
                ..
            } = row.kind
            else {
                continue;
            };
            if read.site
                != (ExecutionSiteV29::Statement {
                    block: SsaBlockIdV1::new(3),
                    statement: 0,
                })
                || read.role != ExecutionOperandV29::RvaluePlace
            {
                continue;
            }
            let block = body
                .blocks
                .iter()
                .find(|block| block.id == row.block)
                .unwrap();
            let operation = &block.operations[row.position];
            assert!(matches!(operation.kind, OperationKind::Load { access, .. }
                if access.volatile && access.address_space == AddressSpace::Private));
            check_scoped_payload_v29(function, &occurrences, row, operation, budget)?;
            let mut changed = operation.clone();
            let OperationKind::Load { access, .. } = &mut changed.kind else {
                unreachable!()
            };
            access.volatile = false;
            assert!(matches!(
                check_scoped_payload_v29(function, &occurrences, row, &changed, budget),
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "scoped memory anchors differ from their source instance",
                    ..
                })
            ));
            reads += 1;
        }
        assert_eq!(reads, 1);
        observed += 1;
    }
    assert_eq!(observed, 2);
    complete_direct_volatile_source_again_v29(original, budget).unwrap_or_else(|error| {
        panic!("same original volatile source must complete physical admission: {error:?}")
    });
    DIRECT_VOLATILE_COMPLETED_V29.set(true);
    Err(unsupported(0, None, None, STOP))
}

fn complete_direct_volatile_source_again_v29(
    original: &ExecutionLifecycleSourceV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSourceOwnedViewErrorV18> {
    struct RestoreObserver(Option<ScopedSlotObserverV29>);
    impl Drop for RestoreObserver {
        fn drop(&mut self) {
            SCOPED_SLOT_OBSERVER_V29.set(self.0);
        }
    }
    let _observer = RestoreObserver(SCOPED_SLOT_OBSERVER_V29.replace(None));
    // Separate complete production re-emission of this unchanged original,
    // not acceptance of the observer's mutated component clone.
    let owner = scoped_root_tests::fixtures::initialization_owner(InitializationFixtureV29 {
        looping: DIRECT_VOLATILE_LOOP_V29.get(),
        volatile: true,
        ..config()
    });
    assert_eq!(
        owner.source_semantic_sha256(),
        original.owner.source_semantic_sha256()
    );
    assert_eq!(owner.identity(), original.owner.identity());
    let launch = ProductionSourceLaunchRosterV1::try_new(
        owner.source_semantic(),
        &[ProductionSourceLaunchRootInputV1::new(
            "lifecycle_fixture",
            [88; 32],
            ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [2, 1, 1]),
        )],
    )
    .unwrap();
    let floor = budget.storage();
    let mut completed = false;
    let result = (|| -> Result<(), ProductionSourceOwnedViewErrorV18> {
        let prepared = ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
            owner,
            launch,
            original.input,
            ProductionSemanticKirLimitsV1::default(),
            budget,
        )?;
        prepared.with_source_consumer_v18(budget, |source, budget| {
            source.with_analysis_v18(budget, |scope| {
                scope.with_inventory_v1(|inventory, budget| {
                    source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                        scoped_raw_admission_v29::with_checked_source_memory_v29(
                            relation,
                            0,
                            None,
                            budget,
                            |_physical, budget| -> Result<(), ProductionSourceOwnedViewErrorV18> {
                                let root = source.root(0, budget)?.1;
                                let body =
                                    inventory.functions()[root].function.body.as_ref().unwrap();
                                let mut reads = 0;
                                for block in &body.blocks {
                                    for operation in &block.operations {
                                        let OperationKind::Load { access, .. } = operation.kind
                                        else {
                                            continue;
                                        };
                                        if !access.volatile {
                                            continue;
                                        }
                                        assert_eq!(access.address_space, AddressSpace::Private);
                                        assert_eq!(operation.results.len(), 1);
                                        assert_eq!(
                                            operation.results[0].ty,
                                            Type::Scalar(ScalarType::U32)
                                        );
                                        reads += 1;
                                    }
                                }
                                assert_eq!(
                                    reads, 2,
                                    "one genuine volatile read in each original helper invocation"
                                );
                                completed = true;
                                Ok(())
                            },
                        )
                    })
                })
            })
        })
    })();
    assert_eq!(budget.storage(), floor);
    if result.is_ok() {
        assert!(completed);
    }
    result
}

#[test]
fn direct_original_volatile_scalar_reads_preserve_source_effect_and_complete_memory_admission() {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            DIRECT_VOLATILE_LOOP_V29.set(self.0);
        }
    }
    let _restore = Restore(DIRECT_VOLATILE_LOOP_V29.get());
    for looping in [false, true] {
        DIRECT_VOLATILE_LOOP_V29.set(looping);
        DIRECT_VOLATILE_COMPLETED_V29.set(false);
        let result = init_run(
            InitializationFixtureV29 {
                looping,
                volatile: true,
                ..config()
            },
            observe_direct_volatile_v29,
        );
        assert!(is_stopped(&result), "looping={looping}: {result:?}");
        assert!(DIRECT_VOLATILE_COMPLETED_V29.get());
        assert_eq!(OBSERVED.get(), 1);
        DIRECT_VOLATILE_COMPLETED_V29.set(false);
        let uninitialized = init_run(
            InitializationFixtureV29 {
                looping,
                volatile: true,
                kill: Some(InitializationKillV29::StorageLive),
                ..config()
            },
            observe_direct_volatile_v29,
        );
        assert!(
            matches!(
                uninitialized,
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "source reference reads an uninitialized partial holder",
                    ..
                })
            ),
            "looping={looping}: {uninitialized:?}"
        );
        assert_eq!(OBSERVED.get(), 0);
        assert!(!DIRECT_VOLATILE_COMPLETED_V29.get());
    }
}

#[test]
fn original_read_volatility_distinguishes_terminal_effects_from_pointer_holder_reads_and_copies() {
    for mode in 0..3 {
        let owner = scoped_root_tests::fixtures::initialization_owner(InitializationFixtureV29 {
            volatile: mode != 2,
            address_read: mode == 1,
            copy_read: mode == 2,
            ..config()
        });
        let function = &owner.source_semantic().functions()[3];
        let role = if mode == 2 {
            ExecutionOperandV29::RvalueOperand(0)
        } else {
            ExecutionOperandV29::RvaluePlace
        };
        let site = ExecutionSiteV29::Statement {
            block: SsaBlockIdV1::new(3),
            statement: u32::try_from(function.blocks()[3].statements().len() - 1).unwrap(),
        };
        let place = scoped_object_original_place_v29(function, site, role).unwrap();
        for prefix in 0..=usize::from(mode == 1) {
            let expected = mode != 2 && (mode != 1 || prefix == 1);
            // This tests only the attribute equation. The fabricated occurrence
            // is never supplied to source/currentness admission or publication.
            let read = ScopedMemoryReadV29 {
                site,
                role,
                prefix: prefix as u32,
                ty: scoped_payload_prefix_type_v29(function, place, prefix).unwrap(),
                occurrence: ScopedMemoryOccurrenceV29::Retained { event: usize::MAX },
            };
            for short in [false, true] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(6 - usize::from(short));
                let mut budget = ArgumentBudgetV1::new(&mut work, 0);
                let result =
                    check_scoped_read_volatility_v29(function, read, expected, &mut budget);
                if short {
                    assert!(matches!(
                        result,
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Work(_)
                            )
                        )
                    ));
                    assert_eq!(budget.work(), 0);
                } else {
                    result.unwrap();
                    assert_eq!(budget.work(), 6);
                }
                assert_eq!(budget.storage(), 0);
            }
            let mut work = CanonicalKernelIrWorkBudgetV1::new(6);
            let mut budget = ArgumentBudgetV1::new(&mut work, 0);
            assert!(matches!(
                check_scoped_read_volatility_v29(function, read, !expected, &mut budget),
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "scoped memory anchors differ from their source instance",
                    ..
                })
            ));
            assert_eq!((budget.work(), budget.storage()), (6, 0));
        }
    }
}

fn audit_direct_cell_effect_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let instances = plan.instances;
    let mut checked = 0;
    for cell in &plan.cells.rows {
        let function = instances.instance(cell.instance).unwrap();
        if function.function().index() != 3 || cell.local.index() != 2 {
            continue;
        }
        assert_eq!(cell.kind, SourceBackingKindV29::Scalar);
        let SemanticStatementKindV1::Assign(assignment) =
            function.declaration().blocks()[3].statements()[0].kind()
        else {
            unreachable!()
        };
        let SemanticRvalueKindV1::Load(load) = assignment.value().kind() else {
            unreachable!()
        };
        let expected = load.volatility() == SemanticVolatilityV1::Volatile;
        let site = SourceReferenceSiteV29 {
            instance: cell.instance,
            block: SemanticBlockIdV1::from_index(3),
            statement: Some(0),
        };
        let total = 10 + if expected { 12 } else { 0 };
        for limit in [total, total - 1] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut exact = ArgumentBudgetV1::new(&mut work, 0);
            let result =
                source_reference_cell_read_volatility_v29(instances, *cell, site, &mut exact);
            if limit == total {
                assert_eq!(result.unwrap(), expected);
                assert_eq!(exact.work(), total);
            } else {
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Work(_)
                        )
                    )
                ));
                assert_eq!(exact.work(), if expected { 10 } else { 0 });
            }
            assert_eq!(exact.storage(), 0);
        }
        // These changed fixed rows exercise only the effect equation,
        // never source membership, initialization, or pointer authority.
        for fault in 0..5 {
            let mut changed = *cell;
            let mut changed_site = site;
            match fault {
                0 => changed.instance = instances.root(),
                1 => changed.local = SemanticLocalIdV1::from_index(1),
                2 => changed.ty = UNIT,
                3 => changed_site.block = SemanticBlockIdV1::from_index(u32::MAX),
                4 => changed_site.statement = Some(usize::MAX),
                _ => unreachable!(),
            }
            assert!(
                matches!(
                    source_reference_cell_read_volatility_v29(
                        instances,
                        changed,
                        changed_site,
                        budget,
                    ),
                    Err(ProductionSemanticKirErrorV1::Unsupported {
                        function: 0,
                        block: None,
                        statement: None,
                        detail: "execution call parameters differ from their source instance",
                    })
                ),
                "fault {fault}"
            );
        }
        assert_eq!(
            source_reference_cell_read_volatility_v29(instances, *cell, site, budget)?,
            expected
        );
        checked += 1;
    }
    assert_eq!(checked, 2, "both genuine repeated helper cells");
    Ok(())
}

#[test]
fn direct_cell_effect_equation_uses_original_source_and_independent_work_limits() {
    for volatile in [false, true] {
        let mut completed = false;
        let result = with_selected_pointer_test_plan_v29(
            scoped_root_tests::fixtures::initialization_owner(InitializationFixtureV29 {
                volatile,
                ..config()
            }),
            |plan, budget| {
                audit_direct_cell_effect_v29(plan, budget)?;
                completed = true;
                Ok(())
            },
        );
        assert!(result.is_ok(), "volatile={volatile}: {result:?}");
        assert!(completed, "the original-demand cell equation must run");
    }
}

thread_local! {
    static DIRECT_CELL_EFFECT_FAULT_V29: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static DIRECT_CELL_EFFECT_OBSERVED_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn observe_direct_cell_effect_candidate_v29(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _slots: &OwnedScopedSourceSlotsV29,
    _budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut reads = 0;
    for (index, lowered) in emitted.iter_mut().enumerate() {
        let instance = instances.instance(instances.id_at(index).unwrap()).unwrap();
        if instance.function().index() != 3 {
            continue;
        }
        let SemanticStatementKindV1::Assign(assignment) =
            instance.declaration().blocks()[3].statements()[0].kind()
        else {
            unreachable!()
        };
        let SemanticRvalueKindV1::Load(load) = assignment.value().kind() else {
            unreachable!()
        };
        let lowered = lowered.as_mut().unwrap();
        let anchors = &lowered.scoped_memory_anchors.as_ref().unwrap().rows;
        for anchor in anchors {
            let ScopedMemoryAnchorKindV29::Access {
                payload: Some(ScopedMemoryPayloadV29::Load { read, .. }),
                ..
            } = anchor.kind
            else {
                continue;
            };
            if read.site
                != (ExecutionSiteV29::Statement {
                    block: SsaBlockIdV1::new(3),
                    statement: 0,
                })
                || read.role != ExecutionOperandV29::RvaluePlace
            {
                continue;
            }
            assert_eq!(read.prefix, 0);
            let body = lowered.function.body.as_mut().unwrap();
            let block = body
                .blocks
                .iter_mut()
                .find(|block| block.id == anchor.block)
                .unwrap();
            let OperationKind::Load { access, .. } = &mut block.operations[anchor.position].kind
            else {
                unreachable!()
            };
            assert_eq!(
                access.volatile,
                load.volatility() == SemanticVolatilityV1::Volatile
            );
            if DIRECT_CELL_EFFECT_FAULT_V29.get() {
                access.volatile = !access.volatile;
            }
            reads += 1;
        }
    }
    assert_eq!(reads, 2);
    OBSERVED.set(OBSERVED.get() + 1);
    DIRECT_CELL_EFFECT_OBSERVED_V29.set(DIRECT_CELL_EFFECT_OBSERVED_V29.get() + 1);
    Ok(())
}

#[test]
fn direct_cell_actual_volatility_cannot_be_added_or_removed_from_original_source() {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            DIRECT_CELL_EFFECT_FAULT_V29.set(self.0);
        }
    }
    let _restore = Restore(DIRECT_CELL_EFFECT_FAULT_V29.get());
    for looping in [false, true] {
        for volatile in [false, true] {
            for fault in [false, true] {
                DIRECT_CELL_EFFECT_FAULT_V29.set(fault);
                DIRECT_CELL_EFFECT_OBSERVED_V29.set(0);
                let (result, _, _, completed) = run_original_repeated_source_v29(
                    || {
                        scoped_root_tests::fixtures::initialization_owner(
                            InitializationFixtureV29 {
                                looping,
                                volatile,
                                ..config()
                            },
                        )
                    },
                    observe_direct_cell_effect_candidate_v29,
                    10_000_000,
                    10_000_000,
                );
                if fault {
                    assert!(!completed);
                    assert!(DIRECT_CELL_EFFECT_OBSERVED_V29.get() > 0);
                    assert!(
                        matches!(
                            result,
                            Err(ProductionSourceOwnedViewErrorV18::Source(
                                ProductionPendingScopedSourceErrorV29::Source(
                                    ProductionSemanticKirErrorV1::Unsupported {
                                        function: 0,
                                        block: None,
                                        statement: None,
                                        detail: "execution call parameters differ from their source instance"
                                            | "scoped memory anchors differ from their source instance",
                                    }
                                )
                            ))
                        ),
                        "looping={looping}, volatile={volatile}: {result:?}"
                    );
                } else {
                    assert!(
                        result.is_ok(),
                        "looping={looping}, volatile={volatile}: {result:?}"
                    );
                    assert!(completed);
                    assert_eq!(DIRECT_CELL_EFFECT_OBSERVED_V29.get(), 3);
                    assert_eq!(OBSERVED.get(), 3);
                }
            }
        }
    }
}
