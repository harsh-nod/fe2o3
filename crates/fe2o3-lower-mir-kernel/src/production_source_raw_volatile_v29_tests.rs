#[test]
fn original_raw_volatile_read_query_preserves_source_identity_and_bounded_work() {
    for looping in [false, true] {
        let mut completed = false;
        with_selected_pointer_test_plan_v29(
            scoped_root_tests::fixtures::initialization_owner(InitializationFixtureV29 {
                looping, address_read: true, volatile: true, ..config()
            }), |plan, budget| {
                assert_eq!(plan.raw_accesses.len(), 2);
                for row in plan.raw_accesses.values() {
                    let function = plan.instances.instance(row.site.instance).unwrap().declaration();
                    let statement = &function.blocks()[row.site.block.index() as usize]
                        .statements()[row.site.statement.unwrap()];
                    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else { unreachable!() };
                    let SemanticRvalueKindV1::Load(load) = assignment.value().kind() else { unreachable!() };
                    assert_eq!(load.volatility(), SemanticVolatilityV1::Volatile);
                    assert_eq!(load.source().projections().len(), 1);
                    let before = budget.work();
                    let floor = budget.storage();
                    plan.check_resolved_raw_volatile_load_v29(row.site, load, budget)?;
                    assert_eq!(budget.work() - before,
                        5 + 12 + load.source().projections().len() + 10
                            + 16 * (plan.raw_accesses.len().checked_ilog2().unwrap_or(0) as usize + 2));
                    assert_eq!(budget.storage(), floor);
                    for limit in [12, 11] {
                        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
                        let mut exact = ArgumentBudgetV1::new(&mut work, 0);
                        let result = check_source_raw_volatile_load_v29(plan.instances, row.site, load, U32, &mut exact);
                        if limit == 12 { result.unwrap(); assert_eq!(exact.work(), 12); }
                        else {
                            assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Work(_)))));
                            assert_eq!(exact.work(), 0);
                        }
                        assert_eq!(exact.storage(), 0);
                    }
                    let clone = load.clone();
                    for fault in 0..5 {
                        let mut site = row.site;
                        match fault {
                            1 => site.instance = plan.instances.root(),
                            2 => site.block = SemanticBlockIdV1::from_index(u32::MAX),
                            3 => site.statement = Some(usize::MAX),
                            _ => {},
                        }
                        assert!(matches!(check_source_raw_volatile_load_v29(plan.instances, site,
                            if fault == 0 { &clone } else { load }, if fault == 4 { UNIT } else { U32 }, budget),
                            Err(ProductionSemanticKirErrorV1::Unsupported {
                                detail: "source reference ordered load requires checked addressable effects", ..
                            })), "fault {fault}");
                    }
                }
                completed = true;
                Ok(())
            },
        ).unwrap();
        assert!(completed);
    }
}

fn volatile_restart_address_owner_v29() -> ProductionSemanticSsaOwnerV1 {
    let template = old_scalar_address_across_restart_owner_v29();
    let semantic = template.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let original = &functions[3];
    let mut blocks = original.blocks().to_vec();
    let mut statements = blocks[3].statements().to_vec();
    let SemanticStatementKindV1::Assign(assignment) = statements[0].kind() else {
        unreachable!()
    };
    let SemanticRvalueKindV1::Load(load) = assignment.value().kind() else {
        unreachable!()
    };
    statements[0] = assign(
        assignment.destination().clone(),
        SemanticRvalueKindV1::Load(fe2o3_mir_model::semantic_mir_v1::SemanticMemoryLoadV1::new(
            load.source().clone(),
            SemanticVolatilityV1::Volatile,
            None,
        )),
    );
    blocks[3] = block(143, statements, blocks[3].terminator().kind().clone());
    functions[3] = function(
        130,
        original.role(),
        original.abi().clone(),
        original.locals().to_vec(),
        blocks,
    );
    scoped_root_tests::fixtures::build(
        semantic.types().to_vec(),
        functions,
        semantic.callables().to_vec(),
    )
}

#[test]
fn original_raw_volatile_read_does_not_revive_an_expired_address() {
    let mut entered = false;
    let result =
        with_selected_pointer_test_plan_v29(volatile_restart_address_owner_v29(), |_, _| {
            entered = true;
            Ok(())
        });
    assert!(!entered);
    assert!(
        matches!(
            result,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "source raw pointer outlived its storage activation",
                ..
            })
        ),
        "the original volatile effect cannot refresh an old pointer: {result:?}"
    );
}

#[test]
fn original_raw_volatile_read_still_requires_initialized_referent_storage() {
    for kill in [
        InitializationKillV29::StorageLive,
        InitializationKillV29::Deinitialize,
    ] {
        let admitted =
            scoped_root_tests::fixtures::try_initialization_owner(InitializationFixtureV29 {
                address_read: true,
                volatile: true,
                kill: Some(kill),
                ..config()
            });
        if matches!(kill, InitializationKillV29::Deinitialize) {
            assert!(
                matches!(admitted, Err(ProductionSemanticSsaErrorV1::PartialMove {
                function, block: 3, statement: Some(0), local: 2,
                violation: fe2o3_pliron::SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
            }) if function == SemanticFunctionIdV1::from_index(3)),
                "Deinitialize must fail original SSA before the raw source query"
            );
            continue;
        }
        let mut entered = false;
        let result = with_selected_pointer_test_plan_v29(
            admitted.expect("StorageLive remains an original C2 initialization obligation"),
            |_, _| {
                entered = true;
                Ok(())
            },
        );
        assert!(!entered);
        assert!(
            matches!(
                result,
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "source reference reads an uninitialized partial holder",
                    ..
                })
            ),
            "{kill:?}: {result:?}"
        );
    }
}

thread_local! {
    static RAW_VOLATILE_FAULT_V29: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static RAW_VOLATILE_OBSERVED_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn inspect_raw_volatile_candidate_v29(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _slots: &OwnedScopedSourceSlotsV29,
    _budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut seen = 0;
    for (index, lowered) in emitted.iter_mut().enumerate() {
        let instance = instances.instance(instances.id_at(index).unwrap()).unwrap();
        if instance.function().index() != 3 {
            continue;
        }
        let original = &instance.declaration().blocks()[3].statements()[1];
        let SemanticStatementKindV1::Assign(assignment) = original.kind() else {
            unreachable!()
        };
        let SemanticRvalueKindV1::Load(load) = assignment.value().kind() else {
            unreachable!()
        };
        let lowered = lowered.as_mut().unwrap();
        let anchors = lowered.scoped_memory_anchors.as_mut().unwrap();
        for anchor in &anchors.rows {
            let ScopedMemoryAnchorKindV29::Object(payload_index) = anchor.kind else {
                continue;
            };
            let payload = &mut anchors.objects[payload_index];
            let ScopedObjectRoleV29::ReadValue {
                read: ScopedObjectReadOriginV29::Original(read),
                ..
            } = payload.role
            else {
                continue;
            };
            if read.site
                != (ExecutionSiteV29::Statement {
                    block: SsaBlockIdV1::new(3),
                    statement: 1,
                })
                || read.role != ExecutionOperandV29::RvaluePlace
            {
                continue;
            }
            assert_eq!((read.prefix, read.ty), (1, U32));
            let block = lowered
                .function
                .body
                .as_mut()
                .unwrap()
                .blocks
                .iter_mut()
                .find(|block| block.id == anchor.block)
                .unwrap();
            let operation = &mut block.operations[anchor.position];
            let OperationKind::Storage(ScopedObjectOperationV29::ReadValue { access, .. }) =
                &mut operation.kind
            else {
                panic!("original raw scalar read needs its actual typed effect");
            };
            assert_eq!(
                access.volatile,
                load.volatility() == SemanticVolatilityV1::Volatile
            );
            if RAW_VOLATILE_FAULT_V29.get() {
                access.volatile = !access.volatile;
                let OperationKind::Storage(changed) = operation.kind else {
                    unreachable!()
                };
                // Keep candidate metadata coherent: only original source replay
                // can reject the altered effect, not stored-operation inequality.
                payload.operation = changed;
            }
            seen += 1;
        }
    }
    assert_eq!(seen, 2);
    RAW_VOLATILE_OBSERVED_V29.set(RAW_VOLATILE_OBSERVED_V29.get() + 1);
    OBSERVED.set(OBSERVED.get() + 1);
    Ok(())
}

#[test]
fn original_raw_volatile_effects_complete_same_candidate_and_reject_coherent_flag_changes() {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            RAW_VOLATILE_FAULT_V29.set(self.0);
        }
    }
    let _restore = Restore(RAW_VOLATILE_FAULT_V29.get());
    for looping in [false, true] {
        for volatile in [false, true] {
            for fault in [false, true] {
                RAW_VOLATILE_FAULT_V29.set(fault);
                RAW_VOLATILE_OBSERVED_V29.set(0);
                let (result, _, _, completed) = run_original_repeated_source_v29(
                    || {
                        scoped_root_tests::fixtures::initialization_owner(
                            InitializationFixtureV29 {
                                looping,
                                address_read: true,
                                volatile,
                                ..config()
                            },
                        )
                    },
                    inspect_raw_volatile_candidate_v29,
                    10_000_000,
                    10_000_000,
                );
                if fault {
                    assert!(!completed);
                    assert!(RAW_VOLATILE_OBSERVED_V29.get() > 0);
                    assert!(
                        matches!(
                            result,
                            Err(ProductionSourceOwnedViewErrorV18::Source(
                                ProductionPendingScopedSourceErrorV29::Source(
                                    ProductionSemanticKirErrorV1::Unsupported {
                                        detail: "scoped memory anchors differ from their source instance",
                                        ..
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
                    assert_eq!((RAW_VOLATILE_OBSERVED_V29.get(), OBSERVED.get()), (3, 3));
                }
            }
        }
    }
}
