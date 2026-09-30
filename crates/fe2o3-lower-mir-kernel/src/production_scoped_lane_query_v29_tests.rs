fn scoped_lane_owner_v29(nested: bool) -> ProductionSemanticSsaOwnerV1 {
    use fe2o3_mir_model::semantic_mir_v1::*;
    let base = entrance_control_owner(false);
    let semantic = base.source_semantic();
    let mut types = semantic.types().to_vec();
    let representation = types[U32.index() as usize].layout().backend_repr().clone();
    let lane = aggregate(&mut types, vec![U32], vec![0], 4, 4, representation, None);
    let mut functions = semantic.functions().to_vec();
    let mut callables = semantic.callables().to_vec();
    if nested {
        callables.push(SemanticCallableDeclV1::defined(
            SemanticFunctionIdV1::from_index(3),
        ));
    }
    let intrinsic = SemanticCallableIdV1::from_index(callables.len() as u32);
    let value = SemanticAbiValueV1::new(
        lane,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap(),
        ),
    );
    callables.push(SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([181; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([182; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([183; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([184; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([185; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            SemanticFunctionAbiV1::new(
                SemanticAbiIdentityV1::from_sha256([186; 32]),
                SemanticLayoutIdentityV1::from_sha256([187; 32]),
                SemanticCanonAbiV1::Rust,
                false,
                false,
                vec![],
                value,
            )
            .unwrap(),
        ),
        operation: SemanticCompilerIntrinsicOperationV1::WaveLaneCurrent {
            lane,
            wave_width: 64,
        },
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([188; 32]),
    });
    let call = |callee, arguments, destination, target| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                callee,
                arguments,
                Some(SemanticCallDestinationV1::new(
                    destination,
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(target),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let helper = &functions[2];
    if nested {
        functions[2] = function(
            210,
            helper.role(),
            helper.abi().clone(),
            helper.locals().to_vec(),
            vec![
                block(
                    214,
                    vec![],
                    call(
                        SemanticCallableIdV1::from_index(3),
                        vec![SemanticOperandV1::Copy(place(1, U32))],
                        place(0, UNIT),
                        1,
                    ),
                ),
                block(
                    215,
                    vec![],
                    call(
                        SemanticCallableIdV1::from_index(3),
                        vec![SemanticOperandV1::Copy(place(1, U32))],
                        place(0, UNIT),
                        2,
                    ),
                ),
                block(216, vec![], SemanticTerminatorKindV1::Return),
            ],
        );
        functions.push(function(
            220,
            SemanticFunctionRoleV1::InternalHelper,
            abi(221, false, &[U32]),
            vec![
                local(222, UNIT, SemanticLocalRoleV1::Return),
                local(223, U32, SemanticLocalRoleV1::Argument(0)),
                local(224, lane, SemanticLocalRoleV1::Temporary),
            ],
            vec![
                block(225, vec![], call(intrinsic, vec![], place(2, lane), 1)),
                block(226, vec![], SemanticTerminatorKindV1::Return),
            ],
        ));
    } else {
        let mut locals = helper.locals().to_vec();
        locals.push(local(216, lane, SemanticLocalRoleV1::Temporary));
        functions[2] = function(
            210,
            helper.role(),
            helper.abi().clone(),
            locals,
            vec![
                block(214, vec![], call(intrinsic, vec![], place(2, lane), 1)),
                block(217, vec![], SemanticTerminatorKindV1::Return),
            ],
        );
    }
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn repeated_lane_owner_v29() -> ProductionSemanticSsaOwnerV1 {
    scoped_lane_owner_v29(false)
}
fn nested_lane_owner_v29() -> ProductionSemanticSsaOwnerV1 {
    scoped_lane_owner_v29(true)
}

thread_local! {
    static SCOPED_LANE_TEST_VISITS_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SCOPED_LANE_TEST_MODE_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

struct ScopedLaneObserverResetV29(Option<ScopedLaneObserverV29>);
impl Drop for ScopedLaneObserverResetV29 {
    fn drop(&mut self) {
        SCOPED_LANE_OBSERVER_V29.set(self.0);
    }
}

fn scoped_lane_probe_v29(
    transport: &ScopedLaneQueryTransportV29,
    map: &ProductionInstanceCorrespondenceV1<'_, '_>,
    child: ProductionCallInstanceIdV1,
    function: &Function,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), CallInstanceEmissionErrorV1> {
    let floor = budget.storage();
    let mut scratch = 0;
    let result = (|| {
        let permit = ScopedLaneCalleeSourceV29 {
            transport,
            map,
            child,
        }
        .prepare(function, budget, &mut scratch)?;
        permit.check(function, budget)?;
        Ok(())
    })();
    budget.release_storage(scratch).unwrap();
    assert_eq!(budget.storage(), floor);
    result
}

fn inspect_scoped_lane_v29(
    references: &SourceReferencePlanV29<'_, '_>,
    map: &mut ProductionInstanceCorrespondenceV1<'_, '_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    transport: Option<&ScopedLaneQueryTransportV29>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let Some(transport) = transport else {
        return Ok(());
    };
    SCOPED_LANE_TEST_VISITS_V29.set(SCOPED_LANE_TEST_VISITS_V29.get() + 1);
    let mode = SCOPED_LANE_TEST_MODE_V29.get();
    let children: Vec<_> = transport
        .chains
        .iter()
        .enumerate()
        .filter(|(_, chain)| chain.count != 0)
        .map(|(index, _)| index)
        .collect();
    assert!(children.len() >= 2);
    assert_eq!(children.len(), transport.rows.len());
    let first = children[0];
    let child = map.plan.id_at(first).unwrap();
    let function = &emitted[first].as_ref().unwrap().function;
    for &index in &children {
        scoped_lane_probe_v29(
            transport,
            map,
            map.plan.id_at(index).unwrap(),
            &emitted[index].as_ref().unwrap().function,
            budget,
        )
        .unwrap();
    }
    if mode == 0 {
        let mut scratch = 0;
        let floor = budget.storage();
        let index = call_splice_index_v1(function, budget, &mut scratch).unwrap();
        assert_eq!(
            call_splice_check_body_v1(function, &index, true, budget),
            Err(CallInstanceEmissionErrorV1::CalleeCollective)
        );
        let permit = ScopedLaneCalleeSourceV29 {
            transport,
            map,
            child,
        }
        .prepare(function, budget, &mut scratch)
        .unwrap();
        assert!(
            call_splice_check_body_with_scoped_queries_v29(
                function,
                &index,
                true,
                None,
                None,
                None,
                Some(&permit),
                budget
            )
            .unwrap()
                > 0
        );
        assert_eq!(
            permit.check(&function.clone(), budget),
            Err(CallInstanceEmissionErrorV1::CalleeCollective)
        );
        drop(permit);
        drop(index);
        budget.release_storage(scratch).unwrap();
        assert_eq!(budget.storage(), floor);
    } else if mode == 1 {
        for fault in 0..10 {
            let mut changed = function.clone();
            let operation = changed
                .body
                .as_mut()
                .unwrap()
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.operations)
                .find(|operation| matches!(operation.kind, OperationKind::Wave(_)))
                .unwrap();
            match fault {
                0 => operation.results[0].id = ValueId(u32::MAX),
                1 => operation.results[0].ty = Type::Scalar(ScalarType::U64),
                2 => operation.kind = OperationKind::Constant(Constant::U32(0)),
                3 => {
                    operation.kind = OperationKind::Wave(WaveOperation::full(
                        WaveOperationKind::LaneId,
                        WaveWidth::Wave32,
                    ))
                }
                4 => operation.results.clear(),
                5 => operation.results.push(operation.results[0].clone()),
                6 => {
                    let duplicate = operation.clone();
                    changed.body.as_mut().unwrap().blocks[0]
                        .operations
                        .push(duplicate);
                }
                7 => {
                    let OperationKind::Wave(wave) = &mut operation.kind else {
                        unreachable!()
                    };
                    wave.active_lanes = 32;
                }
                8 => {
                    operation.kind = OperationKind::Wave(WaveOperation::full(
                        WaveOperationKind::Any {
                            predicate: ValueId(0),
                        },
                        WaveWidth::Wave64,
                    ))
                }
                9 => {
                    let OperationKind::Wave(wave) = &mut operation.kind else {
                        unreachable!()
                    };
                    wave.convergence = fe2o3_kernel_ir::Convergence::uniform(
                        fe2o3_kernel_ir::SynchronizationScope::Workgroup,
                    );
                }
                _ => unreachable!(),
            }
            assert_eq!(
                scoped_lane_probe_v29(transport, map, child, &changed, budget),
                Err(CallInstanceEmissionErrorV1::CalleeCollective),
                "actual operation fault {fault}"
            );
        }
        let source = transport.rows[transport.chains[first].first.unwrap()];
        let original = map.spans.rows[source.span].clone();
        for fault in 0..6 {
            match fault {
                0 => map.spans.rows[source.span].instance = map.plan.id_at(children[1]).unwrap(),
                1 => map.spans.rows[source.span].segments = [None, None],
                2 => map.spans.rows[source.span].segments[1] = original.segments[0],
                3 => {
                    map.spans.rows[source.span].segments[0]
                        .as_mut()
                        .unwrap()
                        .count = 2
                }
                4 => {
                    map.spans.rows[source.span].segments[0]
                        .as_mut()
                        .unwrap()
                        .first += 1
                }
                5 => {
                    let other = transport.rows[transport.chains[children[1]].first.unwrap()];
                    map.spans.rows[source.span].source =
                        InstanceSpanSourceV1::Terminator(other.source);
                }
                _ => unreachable!(),
            }
            assert_eq!(
                scoped_lane_probe_v29(transport, map, child, function, budget),
                Err(CallInstanceEmissionErrorV1::CalleeCollective),
                "mapped source fault {fault}"
            );
            map.spans.rows[source.span] = original.clone();
        }
        for fault in 0..7 {
            let mut hypothesis = transport.clone();
            match fault {
                0 => hypothesis.rows[0].value = ValueId(u32::MAX),
                1 => hypothesis.rows[0].next = Some(0),
                2 => hypothesis.chains[first].count += 1,
                3 => hypothesis.chains[first].first = hypothesis.chains[children[1]].first,
                4 => hypothesis.map ^= 1,
                5 => hypothesis.source_plan ^= 1,
                6 => hypothesis.chains[first].seed = None,
                _ => unreachable!(),
            }
            assert_eq!(
                scoped_lane_probe_v29(&hypothesis, map, child, function, budget),
                Err(CallInstanceEmissionErrorV1::CalleeCollective),
                "transport hypothesis fault {fault}"
            );
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut foreign = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        let before = (foreign.work(), foreign.storage());
        assert_eq!(
            scoped_lane_probe_v29(transport, map, child, function, &mut foreign),
            Err(CallInstanceEmissionErrorV1::Resource(
                ArgumentResourceV1::Accounting
            ))
        );
        assert_eq!((foreign.work(), foreign.storage()), before);
        let mut hypothesis = transport.clone();
        hypothesis.floor = budget.storage() + 1;
        let before = (budget.work(), budget.storage());
        assert_eq!(
            scoped_lane_probe_v29(&hypothesis, map, child, function, budget),
            Err(CallInstanceEmissionErrorV1::Resource(
                ArgumentResourceV1::Accounting
            ))
        );
        assert_eq!((budget.work(), budget.storage()), before);
    } else if mode == 2 {
        let floor = budget.storage();
        let mut scratch = 0;
        let rebuilt =
            ScopedLaneQueryTransportV29::new(references, map, emitted, budget, &mut scratch)
                .unwrap()
                .unwrap();
        assert_eq!(rebuilt.rows.len(), transport.rows.len());
        drop(rebuilt);
        budget.release_storage(scratch).unwrap();
        assert_eq!(budget.storage(), floor);
        for fault in 0..5 {
            let original_function = emitted[first].as_ref().unwrap().function.clone();
            let original_instance = emitted[first].as_ref().unwrap().source_call_instance;
            let mut removed_archive = None;
            let row = emitted[first].as_mut().unwrap();
            match fault {
                0 => {
                    let operation = row
                        .function
                        .body
                        .as_mut()
                        .unwrap()
                        .blocks
                        .iter_mut()
                        .flat_map(|block| &mut block.operations)
                        .find(|operation| matches!(operation.kind, OperationKind::Wave(_)))
                        .unwrap();
                    operation.kind = OperationKind::Constant(Constant::U32(0));
                }
                1 => {
                    let operation = row
                        .function
                        .body
                        .as_ref()
                        .unwrap()
                        .blocks
                        .iter()
                        .flat_map(|block| &block.operations)
                        .find(|operation| matches!(operation.kind, OperationKind::Wave(_)))
                        .unwrap()
                        .clone();
                    row.function.body.as_mut().unwrap().blocks[0]
                        .operations
                        .push(operation);
                }
                2 => removed_archive = row.execution_observation.take(),
                3 => row.source_call_instance = Some(map.plan.id_at(children[1]).unwrap()),
                4 => {
                    let operation = row
                        .function
                        .body
                        .as_mut()
                        .unwrap()
                        .blocks
                        .iter_mut()
                        .flat_map(|block| &mut block.operations)
                        .find(|operation| matches!(operation.kind, OperationKind::Wave(_)))
                        .unwrap();
                    operation.results[0].id = ValueId(u32::MAX);
                }
                _ => unreachable!(),
            }
            let mut scratch = 0;
            let refused =
                ScopedLaneQueryTransportV29::new(references, map, emitted, budget, &mut scratch);
            assert!(
                matches!(refused, Err(CallInstanceEmissionErrorV1::CalleeCollective)),
                "original/actual census fault {fault}: {:?}",
                refused.as_ref().err()
            );
            drop(refused);
            budget.release_storage(scratch).unwrap();
            let row = emitted[first].as_mut().unwrap();
            row.function = original_function;
            row.source_call_instance = original_instance;
            if let Some(archive) = removed_archive {
                row.execution_observation = Some(archive);
            }
            assert_eq!(budget.storage(), floor);
        }
    } else {
        assert_eq!(mode, 3);
        let body = function.body.as_ref().unwrap();
        let blocks = body.blocks.len();
        let operations: usize = body.blocks.iter().map(|block| block.operations.len()).sum();
        let count = transport.chains[first].count;
        let search = (usize::BITS - count.leading_zeros()) as usize + 1;
        // Owner/type checks, two graph walks, index sorting, exact source rows,
        // complete census and the final borrowed permit check.
        let required_work = 4
            + function.id.as_str().len()
            + 2 * (blocks + operations)
            + 5 * count * search
            + 15 * count;
        let required_storage = std::mem::size_of::<Vec<ScopedLaneActualV29<'_>>>()
            + 2 * std::mem::size_of::<
                Result<Vec<ScopedLaneActualV29<'_>>, CallInstanceEmissionErrorV1>,
            >()
            + count * std::mem::size_of::<ScopedLaneActualV29<'_>>()
            + std::mem::size_of::<ScopedLaneCalleeV29<'_>>()
            + 2 * std::mem::size_of::<Result<ScopedLaneCalleeV29<'_>, CallInstanceEmissionErrorV1>>(
            );
        let floor = budget.storage();
        let work = budget.work();
        scoped_lane_probe_v29(transport, map, child, function, budget).unwrap();
        assert_eq!(budget.work() - work, required_work);
        for short in [false, true] {
            let available = required_storage - usize::from(short);
            let filler = budget.storage_limit() - floor - available;
            budget.reserve_storage(filler).unwrap();
            let result = scoped_lane_probe_v29(transport, map, child, function, budget);
            match (short, result) {
                (false, Ok(())) => {}
                (
                    true,
                    Err(CallInstanceEmissionErrorV1::Resource(ArgumentResourceV1::Storage(error))),
                ) => {
                    assert_eq!(error.limit(), budget.storage_limit());
                    assert_eq!(error.actual(), budget.storage_limit() + 1);
                }
                (_, other) => panic!("source-bound lane permit storage boundary: {other:?}"),
            }
            assert_eq!(budget.storage(), floor + filler);
            budget.release_storage(filler).unwrap();
        }
    }
    let function = &emitted[first].as_ref().unwrap().function;
    scoped_lane_probe_v29(transport, map, child, function, budget).unwrap();
    Ok(())
}

fn inspect_no_original_lane_query_v29(
    references: &SourceReferencePlanV29<'_, '_>,
    map: &mut ProductionInstanceCorrespondenceV1<'_, '_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    transport: Option<&ScopedLaneQueryTransportV29>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    assert!(transport.is_none());
    let Some(index) = emitted.iter().position(|row| {
        row.as_ref()
            .is_some_and(|row| row.source_call_instance != Some(map.plan.root()))
    }) else {
        return Ok(());
    };
    let floor = budget.storage();
    let original = emitted[index].as_ref().unwrap().function.clone();
    emitted[index]
        .as_mut()
        .unwrap()
        .function
        .body
        .as_mut()
        .unwrap()
        .blocks[0]
        .operations
        .push(Operation::effect_free(
            ValueDef::new(ValueId(u32::MAX - 1), Type::Scalar(ScalarType::U32)),
            OperationKind::Wave(WaveOperation::full(
                WaveOperationKind::LaneId,
                WaveWidth::Wave64,
            )),
        ));
    let mut scratch = 0;
    let rebuilt =
        ScopedLaneQueryTransportV29::new(references, map, emitted, budget, &mut scratch).unwrap();
    assert!(
        rebuilt.is_none(),
        "an actual Wave must not manufacture an original issuer"
    );
    let changed = &emitted[index].as_ref().unwrap().function;
    let index_view = call_splice_index_v1(changed, budget, &mut scratch).unwrap();
    assert_eq!(
        call_splice_check_body_v1(changed, &index_view, true, budget),
        Err(CallInstanceEmissionErrorV1::CalleeCollective)
    );
    drop(index_view);
    drop(rebuilt);
    emitted[index].as_mut().unwrap().function = original;
    budget.release_storage(scratch).unwrap();
    assert_eq!(budget.storage(), floor);
    SCOPED_LANE_TEST_VISITS_V29.set(SCOPED_LANE_TEST_VISITS_V29.get() + 1);
    Ok(())
}

#[test]
fn scoped_lane_query_absent_original_issuer_never_admits_injected_wave() {
    SCOPED_LANE_TEST_VISITS_V29.set(0);
    let _reset = ScopedLaneObserverResetV29(
        SCOPED_LANE_OBSERVER_V29.replace(Some(inspect_no_original_lane_query_v29)),
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(|| entrance_control_owner(false), &mut budget);
    let mut completed = false;
    prepared
        .with_source_consumer_v18(&mut budget, |_, _| -> SourceOwnedResultV18<()> {
            completed = true;
            Ok(())
        })
        .unwrap();
    assert!(completed);
    assert!(SCOPED_LANE_TEST_VISITS_V29.get() > 0);
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn scoped_lane_query_exact_operation_and_census_resource_boundaries() {
    // These are inert matching/resource hypotheses, not original-source admission.
    let operation = Operation::effect_free(
        ValueDef::new(ValueId(4), Type::Scalar(ScalarType::U32)),
        OperationKind::Wave(WaveOperation::full(
            WaveOperationKind::LaneId,
            WaveWidth::Wave64,
        )),
    );
    for limit in [5, 4] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        let result = check_scoped_lane_operation_v29(&operation, ValueId(4), &mut budget);
        match (limit, result) {
            (5, Ok(())) => assert_eq!(budget.work(), 5),
            (4, Err(CallInstanceEmissionErrorV1::Resource(ArgumentResourceV1::Work(error)))) => {
                assert_eq!(error.limit(), 4);
                assert_eq!(error.actual(), 5);
            }
            (_, other) => panic!("exact lane matcher boundary: {other:?}"),
        }
        assert_eq!(budget.storage(), 0);
    }
    for count in [0usize, 1, 16, 256, 4096] {
        let mut block = fe2o3_kernel_ir::BasicBlock::new(BlockId(0));
        block.operations = (0..count)
            .map(|index| {
                Operation::effect_free(
                    ValueDef::new(ValueId(index as u32), Type::Scalar(ScalarType::U32)),
                    operation.kind.clone(),
                )
            })
            .collect();
        block.terminator = Some(Terminator::Return { values: vec![] });
        let function = Function::internal_helper(
            "lane_census",
            fe2o3_kernel_ir::Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        );
        let bits = (usize::BITS - count.leading_zeros()) as usize + 1;
        let required_work = 2 * (1 + count) + 4 * count * bits + count;
        let required_storage = std::mem::size_of::<Vec<ScopedLaneActualV29<'_>>>()
            + 2 * std::mem::size_of::<
                Result<Vec<ScopedLaneActualV29<'_>>, CallInstanceEmissionErrorV1>,
            >()
            + count * std::mem::size_of::<ScopedLaneActualV29<'_>>();
        for (work_limit, storage_limit, kind) in [
            (required_work, required_storage, 0),
            (required_work - 1, required_storage, 1),
            (required_work, required_storage - 1, 2),
        ] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
            let mut scratch = 0;
            let result = scoped_lane_actual_v29(&function, &mut budget, &mut scratch);
            match (kind, result) {
                (0, Ok(rows)) => {
                    assert_eq!(rows.len(), count);
                    assert_eq!(budget.work(), required_work);
                    assert_eq!(budget.storage(), required_storage);
                    drop(rows);
                }
                (
                    1,
                    Err(CallInstanceEmissionErrorV1::Resource(ArgumentResourceV1::Work(error))),
                ) => {
                    assert_eq!(error.limit(), required_work - 1);
                    assert_eq!(error.actual(), required_work);
                }
                (
                    2,
                    Err(CallInstanceEmissionErrorV1::Resource(ArgumentResourceV1::Storage(error))),
                ) => {
                    assert_eq!(error.limit(), required_storage - 1);
                    assert_eq!(error.actual(), required_storage);
                }
                (_, other) => panic!(
                    "lane census count={count}, boundary={kind}: {:?}",
                    other.as_ref().err()
                ),
            }
            budget.release_storage(scratch).unwrap();
            assert_eq!(budget.storage(), 0);
        }
        assert!(
            required_work <= 2 + 75 * count.max(1),
            "independent N log N bound through N=4096"
        );
    }
}

#[test]
fn scoped_lane_query_none_path_still_prepays_new_fixed_splice_envelopes() {
    let required = std::mem::size_of::<Option<ScopedLaneCalleeSourceV29<'_, '_, '_>>>()
        + std::mem::size_of::<Option<ScopedLaneCalleeV29<'_>>>()
        + std::mem::size_of::<Result<Option<ScopedLaneCalleeV29<'_>>, CallInstanceEmissionErrorV1>>(
        );
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1);
        let mut budget = ArgumentBudgetV1::new(&mut work, required - usize::from(short));
        let signature = || fe2o3_kernel_ir::Signature::new(vec![], vec![]);
        let caller = Function::internal_helper("header_caller", signature(), vec![], vec![]);
        // The exact invalid-role refusal is deliberately after header prepay
        // and before graph indexing, isolating this None-path boundary.
        let callee = Function::external_import("header_callee", signature());
        let result = splice_production_call_instance_with_source_queries_v29(
            caller,
            callee,
            FunctionOperationLocation {
                block: BlockId(0),
                operation_index: 0,
            },
            BlockId(1),
            BlockId(2),
            None,
            None,
            None,
            None,
            &mut budget,
        );
        match (short, result) {
            (false, Err(CallInstanceEmissionErrorV1::InvalidRole)) => assert_eq!(budget.work(), 1),
            (
                true,
                Err(CallInstanceEmissionErrorV1::Resource(ArgumentResourceV1::Storage(error))),
            ) => {
                assert_eq!(error.limit(), required - 1);
                assert_eq!(error.actual(), required);
                assert_eq!(budget.work(), 0);
            }
            (_, other) => panic!("optional envelope boundary: {other:?}"),
        }
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn scoped_lane_query_repeated_and_nested_original_helpers_reach_complete_source() {
    for nested in [false, true] {
        for mode in 0..4 {
            SCOPED_LANE_TEST_VISITS_V29.set(0);
            SCOPED_LANE_TEST_MODE_V29.set(mode);
            let _reset = ScopedLaneObserverResetV29(
                SCOPED_LANE_OBSERVER_V29.replace(Some(inspect_scoped_lane_v29)),
            );
            let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
            budget.reserve_storage(MODULE_FLOOR).unwrap();
            let owner = if nested {
                nested_lane_owner_v29
            } else {
                repeated_lane_owner_v29
            };
            let prepared = scalar_payload_prepared_from_v18(owner, &mut budget);
            let mut completed = false;
            prepared
                .with_source_consumer_v18(
                    &mut budget,
                    |source, budget| -> SourceOwnedResultV18<()> {
                        source.with_analysis_v18(budget, |scope| {
                            scope.with_inventory_v1(|inventory, budget| {
                                let mut waves = 0;
                                for row in inventory.operations() {
                                    budget.charge_work(1)?;
                                    if matches!(row.operation.kind, OperationKind::Wave(_)) {
                                        assert_eq!(
                                            row.operation.kind,
                                            OperationKind::Wave(WaveOperation::full(
                                                WaveOperationKind::LaneId,
                                                WaveWidth::Wave64
                                            ))
                                        );
                                        waves += 1;
                                    }
                                }
                                assert_eq!(waves, if nested { 4 } else { 2 });
                                Ok::<(), ProductionSourceOwnedViewErrorV18>(())
                            })
                        })?;
                        completed = true;
                        Ok(())
                    },
                )
                .unwrap();
            assert!(completed, "nested={nested}, mode={mode}");
            assert!(SCOPED_LANE_TEST_VISITS_V29.get() > 0);
            assert_eq!(budget.storage(), MODULE_FLOOR);
        }
    }
}
