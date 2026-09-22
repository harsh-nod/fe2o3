use super::*;

fn dead_exit_kernel(s: ScalarType, n: u64, value: bool, outer: bool, failure: u8) -> Module {
    let mut module = kernel(s, n, false, failure);
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[2].operations[0] = Operation::effect_free(
        ValueDef::new(ValueId(30), scalar(s)),
        OperationKind::Binary {
            op: BinaryOp::Add,
            lhs: ValueId(20),
            rhs: ValueId(10),
        },
    );
    let literal = Operation::effect_free(
        ValueDef::new(ValueId(80), Type::BOOL),
        OperationKind::Constant(Constant::Bool(value)),
    );
    if outer {
        body.blocks[0].operations.push(literal);
    } else {
        body.blocks[2].operations.insert(1, literal);
    }
    let (yes, no, yes_args, no_args) = if value {
        (702, 703, vec![], vec![ValueId(25)])
    } else {
        (703, 702, vec![ValueId(25)], vec![])
    };
    body.blocks[2].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(80),
        then_target: BlockId(yes),
        then_arguments: yes_args,
        else_target: BlockId(no),
        else_arguments: no_args,
    });
    body.blocks
        .push(block(702, vec![], vec![], branch(41, &[30])));
    body.blocks.push(block(
        703,
        vec![ValueDef::new(ValueId(90), scalar(s))],
        vec![],
        Terminator::Unreachable,
    ));
    module
}

fn source_block(ordinal: u32) -> Block {
    Block {
        function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(0),
        block: ordinal,
    }
}

fn assert_custody(input: &Owner, owner: &OwnedLoopUnrollV1, n: u64, value: bool) {
    assert_eq!(owner.origins().selection.unwrap().iterations, n as u8);
    assert_ne!(
        input.canonical().canonical_bytes(),
        owner.output().canonical().canonical_bytes()
    );
    let trap = &input.module().functions[0].body.as_ref().unwrap().blocks[5];
    let actual: Vec<_> = owner.output().module().functions[0]
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .filter(|block| block.id == trap.id)
        .collect();
    assert_eq!(actual, [trap]);
    assert_eq!(
        owner
            .origins()
            .blocks
            .iter()
            .filter(|row| row.input == source_block(5))
            .count(),
        1
    );
    let dead = Edge {
        source: source_block(2),
        successor: u32::from(value),
    };
    let edges: Vec<_> = owner
        .origins()
        .edges
        .iter()
        .filter(|row| row.input == dead)
        .collect();
    assert_eq!(edges.len(), n.max(1) as usize);
    assert_eq!(
        edges.iter().filter(|row| row.output.is_some()).count(),
        n as usize
    );
    for row in edges {
        assert_eq!(row.output.is_none(), n == 0);
        assert!(matches!(
            row.copy,
            CopyRole::Body(_) | CopyRole::OmittedBody
        ));
    }
    assert_eq!(
        owner
            .origins()
            .arguments
            .iter()
            .filter(|row| row.input.edge == dead && row.output.is_some())
            .count(),
        n as usize,
    );
}

#[test]
fn direct_bool_dead_exit_sim_width_polarity_trip_location_and_complete_events() {
    let mut executions = 0;
    for s in [
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
    ] {
        for n in [0, 1, 3, 8] {
            for value in [false, true] {
                for outer in [false, true] {
                    with_input(dead_exit_kernel(s, n, value, outer, 0), |input, budget| {
                        let owner = run(input, budget);
                        assert_custody(input, &owner, n, value);
                        let retained = {
                            let (pair, receipt) =
                                owner.replay(input, owner.limits(), budget).unwrap();
                            budget.reserve_storage(receipt.retained_storage()).unwrap();
                            assert!(!pair.grants_authority());
                            let before = sim(input);
                            let after = sim(owner.output());
                            for control in [false, true] {
                                let request = request(s, control);
                                let unchanged = request.clone();
                                let mut a = Events::default();
                                let mut b = Events::default();
                                let old = before
                                    .simulate_observed_with_sink(
                                        &request,
                                        SimulationTargetV1::amdgpu_64(),
                                        SimulationLimitsV1::default(),
                                        &mut a,
                                    )
                                    .unwrap();
                                let new = after
                                    .simulate_observed_with_sink(
                                        &request,
                                        SimulationTargetV1::amdgpu_64(),
                                        SimulationLimitsV1::default(),
                                        &mut b,
                                    )
                                    .unwrap();
                                executions += 2;
                                assert_eq!(old.arguments(), new.arguments());
                                assert_eq!(old.conflict_assessment(), new.conflict_assessment());
                                assert_eq!(a.0, translated(&pair, &b.0));
                                assert!(!a.0.iter().any(|event| event.site.block == BlockId(703)));
                                assert_eq!(
                                    a.0.iter()
                                        .filter(|event| event.site.block == BlockId(97)
                                            && matches!(
                                                event.kind,
                                                Kind::Branch {
                                                    target: BlockId(702)
                                                }
                                            ))
                                        .count(),
                                    n as usize
                                );
                                assert_eq!(
                                    a.0.iter()
                                        .filter(|event| event.site.block == BlockId(41)
                                            && matches!(event.kind, Kind::MemoryWrite { .. }))
                                        .count(),
                                    n as usize + 1
                                );
                                assert_eq!(
                                    a.0.iter()
                                        .filter(|event| event.site.block == BlockId(97)
                                            && matches!(event.kind, Kind::MemoryWrite { .. }))
                                        .count(),
                                    2 * n as usize
                                );
                                for result in [&old, &new] {
                                    assert_eq!(result.invocations_executed(), 1);
                                    assert!(!result.grants_execution_authority());
                                    for (arg, last, initialized) in
                                        [(3, n, true), (4, n.saturating_sub(1), n != 0)]
                                    {
                                        let actual = result.buffer(arg).unwrap();
                                        let bytes = last.to_le_bytes();
                                        let expected = if initialized {
                                            bytes[..width(s)].to_vec()
                                        } else {
                                            vec![0xa5; width(s)]
                                        };
                                        assert_eq!(&actual.bytes()[..width(s)], expected);
                                        assert_eq!(
                                            &actual.bytes()[width(s)..],
                                            vec![0xa5; width(s)]
                                        );
                                        assert_eq!(
                                            &actual.initialized()[..width(s)],
                                            vec![initialized; width(s)]
                                        );
                                        assert_eq!(
                                            &actual.initialized()[width(s)..],
                                            vec![false; width(s)]
                                        );
                                    }
                                }
                                assert_eq!(request, unchanged);
                            }
                            receipt.retained_storage()
                        };
                        budget.release_storage(retained).unwrap();
                        release(owner, budget);
                    });
                }
            }
        }
    }
    assert_eq!(executions, 256);
}

#[test]
fn dead_exit_unroll_retains_partial_memory_and_arithmetic_failure_events() {
    let mut executions = 0;
    for failure in [1, 2] {
        for value in [false, true] {
            with_input(
                dead_exit_kernel(ScalarType::U32, 3, value, false, failure),
                |input, budget| {
                    let owner = run(input, budget);
                    assert_custody(input, &owner, 3, value);
                    let retained = {
                        let (pair, receipt) = owner.replay(input, owner.limits(), budget).unwrap();
                        budget.reserve_storage(receipt.retained_storage()).unwrap();
                        let before = sim(input);
                        let after = sim(owner.output());
                        let request = request(ScalarType::U32, false);
                        let mut a = Events::default();
                        let mut b = Events::default();
                        let first = before
                            .simulate_observed_with_sink(
                                &request,
                                SimulationTargetV1::amdgpu_64(),
                                SimulationLimitsV1::default(),
                                &mut a,
                            )
                            .unwrap_err();
                        let last = after
                            .simulate_observed_with_sink(
                                &request,
                                SimulationTargetV1::amdgpu_64(),
                                SimulationLimitsV1::default(),
                                &mut b,
                            )
                            .unwrap_err();
                        executions += 2;
                        let (
                            SimulationErrorV1::Execution(first),
                            SimulationErrorV1::Execution(mut last),
                        ) = (first, last)
                        else {
                            panic!("actual failure")
                        };
                        let site = last.site.as_mut().unwrap();
                        site.block = old_id(&pair, old_block(&pair, 0, site.block));
                        assert_eq!(first, last);
                        assert_eq!(first.site.as_ref().unwrap().block, BlockId(97));
                        if failure == 1 {
                            assert!(matches!(
                                first.kind,
                                SimulationExecutionErrorKindV1::UndefinedIntegerOperation(_)
                            ));
                        } else {
                            assert!(matches!(
                                first.kind,
                                SimulationExecutionErrorKindV1::UninitializedRead { .. }
                            ));
                        }
                        assert_eq!(a.0, translated(&pair, &b.0));
                        assert!(!a.0.iter().any(|event| event.site.block == BlockId(703)));
                        receipt.retained_storage()
                    };
                    budget.release_storage(retained).unwrap();
                    release(owner, budget);
                },
            );
        }
    }
    assert_eq!(executions, 8);
}

#[test]
fn dead_exit_unroll_live_dynamic_derived_cycle_call_and_liveout_stay_noops() {
    for mode in 0..9 {
        let mut module = dead_exit_kernel(ScalarType::U32, 3, false, false, 0);
        let body = module.functions[0].body.as_mut().unwrap();
        match mode {
            0 => body.blocks[2].operations[1].kind = OperationKind::Constant(Constant::Bool(true)),
            1 => {
                let Some(Terminator::ConditionalBranch { condition, .. }) =
                    &mut body.blocks[2].terminator
                else {
                    panic!("condition")
                };
                *condition = ValueId(2);
            }
            2 => {
                body.blocks[2].operations[1].kind = OperationKind::Compare {
                    predicate: ComparePredicate::NotEqual,
                    lhs: ValueId(11),
                    rhs: ValueId(11),
                }
            }
            3 => {
                body.blocks[4].terminator = Some(Terminator::ConditionalBranch {
                    condition: ValueId(2),
                    then_target: BlockId(702),
                    then_arguments: vec![],
                    else_target: BlockId(41),
                    else_arguments: vec![ValueId(30)],
                })
            }
            4 => {
                body.blocks[2].operations.push(Operation::new(
                    vec![],
                    OperationKind::Call {
                        callee: fe2o3_kernel_ir::FunctionId::from("other"),
                        arguments: vec![],
                    },
                ));
                module.functions.push(Function::external_import(
                    "other",
                    Signature::new(vec![], vec![]),
                ));
            }
            5 => body.blocks[3].operations.push(Operation::effect_free(
                ValueDef::new(ValueId(91), scalar(ScalarType::U32)),
                OperationKind::Binary {
                    op: BinaryOp::Add,
                    lhs: ValueId(20),
                    rhs: ValueId(10),
                },
            )),
            6 => {
                body.blocks[2].terminator = Some(Terminator::Unreachable);
                body.blocks[4].terminator = Some(Terminator::Return { values: vec![] });
            }
            7 => body.blocks[2].operations.push(Operation::effect_free(
                ValueDef::new(
                    ValueId(92),
                    Type::pointer(
                        scalar(ScalarType::U32),
                        AddressSpace::Private,
                        AccessMode::ReadWrite,
                    ),
                ),
                OperationKind::Alloca {
                    element: scalar(ScalarType::U32),
                    count: None,
                    address_space: AddressSpace::Private,
                    alignment: 4,
                },
            )),
            8 => {
                let Some(Terminator::ConditionalBranch {
                    else_target,
                    else_arguments,
                    ..
                }) = &mut body.blocks[2].terminator
                else {
                    panic!("condition")
                };
                *else_target = BlockId(701);
                *else_arguments = vec![ValueId(25)];
                body.blocks[4].terminator = Some(Terminator::Return { values: vec![] });
            }
            _ => unreachable!(),
        }
        no_op(module);
    }
}

#[test]
fn live_side_exit_remains_a_noop_and_reaches_the_actual_trap() {
    let mut module = dead_exit_kernel(ScalarType::U32, 3, false, false, 0);
    module.functions[0].body.as_mut().unwrap().blocks[2].operations[1].kind =
        OperationKind::Constant(Constant::Bool(true));
    with_input(module, |input, budget| {
        let owner = run(input, budget);
        assert_eq!(owner.origins().selection, None);
        assert_eq!(
            owner.output().canonical().canonical_bytes(),
            input.canonical().canonical_bytes()
        );
        let retained = {
            let (pair, receipt) = owner.replay(input, owner.limits(), budget).unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let request = request(ScalarType::U32, false);
            let mut a = Events::default();
            let mut b = Events::default();
            let before = sim(input)
                .simulate_observed_with_sink(
                    &request,
                    SimulationTargetV1::amdgpu_64(),
                    SimulationLimitsV1::default(),
                    &mut a,
                )
                .unwrap_err();
            let after = sim(owner.output())
                .simulate_observed_with_sink(
                    &request,
                    SimulationTargetV1::amdgpu_64(),
                    SimulationLimitsV1::default(),
                    &mut b,
                )
                .unwrap_err();
            let (SimulationErrorV1::Execution(before), SimulationErrorV1::Execution(after)) =
                (before, after)
            else {
                panic!("real trap")
            };
            assert_eq!(before, after);
            assert!(matches!(
                before.kind,
                SimulationExecutionErrorKindV1::ReachedUnreachable
            ));
            assert_eq!(before.site.as_ref().unwrap().block, BlockId(703));
            assert_eq!(a.0, translated(&pair, &b.0));
            assert_eq!(
                a.0.iter()
                    .filter(|e| e.site.block == BlockId(97)
                        && matches!(
                            e.kind,
                            Kind::Branch {
                                target: BlockId(703)
                            }
                        ))
                    .count(),
                1
            );
            receipt.retained_storage()
        };
        budget.release_storage(retained).unwrap();
        release(owner, budget);
    });
}

#[test]
fn dead_exit_pair_rejects_forged_constant_edge_argument_and_lineage() {
    with_input(
        dead_exit_kernel(ScalarType::U64, 3, false, false, 0),
        |input, budget| {
            let mut owner = run(input, budget);
            for mode in 0..3 {
                let mut module = owner.output().module().clone();
                let body = module.functions[0].body.as_mut().unwrap();
                let cloned = body
                    .blocks
                    .iter_mut()
                    .find(|block| {
                        block.operations.iter().any(|op| {
                            matches!(op.kind, OperationKind::Constant(Constant::Bool(false)))
                        })
                    })
                    .unwrap();
                match mode {
                    0 => {
                        let constant = cloned
                            .operations
                            .iter_mut()
                            .find(|op| {
                                matches!(op.kind, OperationKind::Constant(Constant::Bool(false)))
                            })
                            .unwrap();
                        constant.kind = OperationKind::Constant(Constant::Bool(true));
                    }
                    1 => {
                        let Some(Terminator::ConditionalBranch {
                            then_target,
                            else_target,
                            then_arguments,
                            else_arguments,
                            ..
                        }) = &mut cloned.terminator
                        else {
                            panic!("clone branch")
                        };
                        std::mem::swap(then_target, else_target);
                        std::mem::swap(then_arguments, else_arguments);
                    }
                    2 => {
                        let Some(Terminator::ConditionalBranch { then_arguments, .. }) =
                            &mut cloned.terminator
                        else {
                            panic!("clone branch")
                        };
                        then_arguments[0] = ValueId(11);
                    }
                    _ => unreachable!(),
                }
                let (hostile, storage) = admit(&module);
                budget.reserve_storage(storage).unwrap();
                let floor = budget.storage();
                assert!(
                    check_pair(input, &hostile, owner.origins(), owner.limits(), budget).is_err()
                );
                assert_eq!(budget.storage(), floor);
                drop(hostile);
                budget.release_storage(storage).unwrap();
            }
            let position = owner
                .rows
                .edges
                .iter()
                .position(|row| {
                    row.input
                        == (Edge {
                            source: source_block(2),
                            successor: 0,
                        })
                        && row.output.is_some()
                })
                .unwrap();
            let saved = owner.rows.edges[position];
            owner.rows.edges[position].output = None;
            let floor = budget.storage();
            assert!(owner.replay(input, owner.limits(), budget).is_err());
            assert_eq!(budget.storage(), floor);
            owner.rows.edges[position] = saved;
            replay(&owner, input, budget);
            release(owner, budget);
        },
    );
}

#[derive(Debug)]
struct Observation {
    result: Result<()>,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
fn observe(input: &Owner, floor: usize, w: usize, s: usize) -> Observation {
    let mut work = Work::new(w);
    let (result, peak, failed_storage) = {
        let mut budget = Budget::new(&mut work, s);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = unroll_canonical_kir_loops_v1(input, Limits::default(), &mut budget).map(
            |(owner, storage)| {
                budget.reserve_storage(storage.retained_storage()).unwrap();
                assert_eq!(storage.retained_storage(), owner.retained);
                assert_eq!(
                    owner.retained,
                    retained(owner.output_storage, &owner.rows).unwrap()
                );
                drop(owner);
                budget.release_storage(storage.retained_storage()).unwrap();
            },
        );
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        (result, budget.peak_storage(), budget.failed_storage())
    };
    Observation {
        result,
        work: work.work(),
        peak,
        failed_work: work.failed_work(),
        failed_storage,
    }
}

#[test]
fn dead_exit_unroll_work_storage_and_unwind_cleanup_are_paid() {
    let module = dead_exit_kernel(ScalarType::U32, 3, false, false, 0);
    let (input, storage) = admit(&module);
    let sibling = [0x72u8; FLOOR];
    let floor = storage + sibling.len();
    let full = observe(&input, floor, W, S);
    assert!(full.result.is_ok(), "{full:?}");
    let exact = observe(&input, floor, full.work, full.peak);
    assert!(exact.result.is_ok(), "{exact:?}");
    assert_eq!(
        (
            exact.work,
            exact.peak,
            exact.failed_work,
            exact.failed_storage
        ),
        (full.work, full.peak, None, None)
    );
    let short = observe(&input, floor, full.work - 1, full.peak);
    let Err(Error::Resource(Resource::Work(error))) = short.result else {
        panic!("final transfer work: {short:?}")
    };
    assert_eq!(
        (error.actual(), error.limit(), short.work),
        (full.work, full.work - 1, full.work - 1)
    );
    assert_eq!(
        (short.failed_work, short.failed_storage),
        (Some(full.work), None)
    );
    let scope = size_of::<Meter<'_, '_>>();
    let denied = observe(&input, floor, W, floor + scope - 1);
    let Err(Error::Resource(Resource::Storage(error))) = denied.result else {
        panic!("scope storage: {denied:?}")
    };
    assert_eq!(
        (error.actual(), error.limit(), denied.work, denied.peak),
        (floor + scope, floor + scope - 1, 0, floor)
    );
    assert_eq!(denied.failed_storage, Some(floor + scope));
    let peak_short = observe(&input, floor, W, full.peak - 1);
    assert!(peak_short.result.is_err(), "{peak_short:?}");
    assert_eq!(
        (peak_short.failed_work, peak_short.failed_storage),
        (None, Some(full.peak))
    );
    assert!(peak_short.peak < full.peak);

    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    budget.reserve_storage(floor).unwrap();
    let before = budget.work();
    let result = resources::scoped(&mut budget, |meter: &mut Meter<'_, '_>| -> Result<()> {
        let (owner, receipt) =
            meter.derive(|b| unroll_canonical_kir_loops_v1(&input, Limits::default(), b))?;
        meter.reserve(receipt.retained_storage())?;
        assert_custody(&input, &owner, 3, false);
        panic!("drop actual unrolled owner before scope restoration");
    });
    assert!(matches!(result, Err(Error::Panicked)));
    assert_eq!(budget.storage(), floor);
    assert!(budget.work() > before);
    assert_eq!(input.module(), &module);
    assert_eq!(sibling, [0x72; FLOOR]);
}
