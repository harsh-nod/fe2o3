use super::*;

fn checked_fixture(ty: ScalarType, operator: CheckedBinaryOperator, both: bool) -> Module {
    let scalar = Type::Scalar(ty);
    let mut entry = block(10, branch(20, &[10]));
    entry.operations = vec![
        op(10, u32_ty(), OperationKind::Constant(Constant::U32(0))),
        op(11, u32_ty(), OperationKind::Constant(Constant::U32(1))),
    ];
    let mut header = block(20, conditional(21, 30, 60));
    header.parameters.push(ValueDef::new(ValueId(20), u32_ty()));
    header.operations.push(op(
        21,
        Type::BOOL,
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: ValueId(20),
            rhs: ValueId(0),
        },
    ));
    let mut body = block(40, branch(50, &[]));
    body.operations = vec![
        Operation::checked_binary(
            ValueDef::new(ValueId(30), scalar.clone()),
            ValueDef::new(ValueId(31), Type::BOOL),
            operator,
            ValueId(1),
            ValueId(2),
        ),
        op(
            32,
            scalar.clone(),
            OperationKind::Unary {
                op: UnaryOp::Not,
                operand: ValueId(30),
            },
        ),
    ];
    if both {
        body.operations.push(op(
            33,
            Type::BOOL,
            OperationKind::Unary {
                op: UnaryOp::Not,
                operand: ValueId(31),
            },
        ));
    }
    body.operations.push(Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(3),
            value: ValueId(32),
            access: MemoryAccess::new(AddressSpace::Global, 1),
        },
    ));
    if both {
        body.operations.push(Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(4),
                value: ValueId(33),
                access: MemoryAccess::new(AddressSpace::Global, 1),
            },
        ));
    }
    let mut latch = block(50, branch(20, &[35]));
    latch.operations.push(op(
        35,
        u32_ty(),
        OperationKind::Binary {
            op: BinaryOp::Add,
            lhs: ValueId(20),
            rhs: ValueId(11),
        },
    ));
    let mut module = Module::new("checked-licm-dynamic-conditional");
    module.functions.push(Function::kernel_entry(
        "checked_licm_impl",
        Signature::new(
            vec![
                u32_ty(),
                scalar.clone(),
                scalar.clone(),
                Type::pointer(scalar, AddressSpace::Global, AccessMode::ReadWrite),
                Type::pointer(Type::BOOL, AddressSpace::Global, AccessMode::ReadWrite),
                Type::BOOL,
            ],
            vec![],
        ),
        (0..6).map(ValueId).collect(),
        vec![
            entry,
            header,
            block(30, conditional(5, 40, 50)),
            body,
            latch,
            block(60, Terminator::Return { values: vec![] }),
        ],
    ));
    module.kernels.push(Kernel::new(
        "checked_licm",
        "checked_licm_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn shapes() -> [(ScalarType, u32, bool); 8] {
    [
        (ScalarType::I8, 8, true),
        (ScalarType::I16, 16, true),
        (ScalarType::I32, 32, true),
        (ScalarType::I64, 64, true),
        (ScalarType::U8, 8, false),
        (ScalarType::U16, 16, false),
        (ScalarType::U32, 32, false),
        (ScalarType::U64, 64, false),
    ]
}

fn expected(
    operator: CheckedBinaryOperator,
    bits: u32,
    signed: bool,
    a: u128,
    b: u128,
) -> (u128, bool) {
    let modulus = 1u128 << bits;
    if signed {
        let interpret = |x: u128| {
            if x >= modulus / 2 {
                x as i128 - modulus as i128
            } else {
                x as i128
            }
        };
        let (a, b) = (interpret(a), interpret(b));
        let value = match operator {
            CheckedBinaryOperator::Add => a + b,
            CheckedBinaryOperator::Subtract => a - b,
            CheckedBinaryOperator::Multiply => a * b,
        };
        (
            value as u128 & (modulus - 1),
            value < -(modulus as i128 / 2) || value >= modulus as i128 / 2,
        )
    } else {
        match operator {
            CheckedBinaryOperator::Add => ((a + b) & (modulus - 1), a + b >= modulus),
            CheckedBinaryOperator::Subtract => ((a + modulus - b) & (modulus - 1), a < b),
            CheckedBinaryOperator::Multiply => ((a * b) & (modulus - 1), a * b >= modulus),
        }
    }
}

fn request(
    ty: ScalarType,
    bits: u32,
    a: u128,
    b: u128,
    trips: u32,
    enabled: bool,
) -> SimulationRequestV1 {
    let target = SimulationTargetV1::amdgpu_64();
    let bytes = (bits / 8) as usize;
    SimulationRequestV1::new(
        "checked_licm",
        [1, 1, 1],
        [1, 1, 1],
        vec![
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(trips)),
            SimulationArgumentV1::Scalar(ScalarBitsV1::new(ty, a, target).unwrap()),
            SimulationArgumentV1::Scalar(ScalarBitsV1::new(ty, b, target).unwrap()),
            SimulationArgumentV1::Buffer(
                BufferArgumentV1::new(
                    ty,
                    AccessMode::ReadWrite,
                    1,
                    vec![0xa5; bytes * 2],
                    vec![false; bytes * 2],
                    target,
                )
                .unwrap(),
            ),
            SimulationArgumentV1::Buffer(
                BufferArgumentV1::new(
                    ScalarType::Bool,
                    AccessMode::ReadWrite,
                    1,
                    vec![0; 2],
                    vec![false; 2],
                    target,
                )
                .unwrap(),
            ),
            SimulationArgumentV1::Scalar(ScalarBitsV1::boolean(enabled)),
        ],
    )
}

#[test]
fn checked_licm_moves_one_or_both_result_users_without_splitting_the_operation() {
    for (ty, _, _) in shapes() {
        for operator in [
            CheckedBinaryOperator::Add,
            CheckedBinaryOperator::Subtract,
            CheckedBinaryOperator::Multiply,
        ] {
            for both in [false, true] {
                with_input(checked_fixture(ty, operator, both), |input, budget| {
                    let tail = prepare_owned_licm_v1(input, budget).unwrap();
                    budget.reserve_storage(tail.retained_storage()).unwrap();
                    let moved: Vec<_> = tail
                        .origins()
                        .iter()
                        .filter(|row| row.hoist.is_some())
                        .collect();
                    assert_eq!(moved.len(), if both { 3 } else { 2 });
                    let out = &tail.output().module().functions[0]
                        .body
                        .as_ref()
                        .unwrap()
                        .blocks;
                    assert_eq!(
                        out[0].operations[2].results,
                        [
                            ValueDef::new(ValueId(30), Type::Scalar(ty)),
                            ValueDef::new(ValueId(31), Type::BOOL),
                        ]
                    );
                    assert!(
                        out[3]
                            .operations
                            .iter()
                            .all(|op| matches!(op.kind, OperationKind::Store { .. }))
                    );
                    replay(&tail, input, budget);
                    let again = prepare_owned_licm_v1(tail.output(), budget).unwrap();
                    budget.reserve_storage(again.retained_storage()).unwrap();
                    assert!(again.origins().iter().all(|row| row.hoist.is_none()));
                    assert_eq!(
                        again.output().canonical().canonical_bytes(),
                        tail.output().canonical().canonical_bytes()
                    );
                    release(again, budget);
                    release(tail, budget);
                });
            }
        }
    }
}

#[test]
fn checked_licm_moves_overflow_only_users_and_preserves_unused_value_result() {
    for (ty, bits, signed) in shapes() {
        let mask = (1u128 << bits) - 1;
        for operator in [
            CheckedBinaryOperator::Add,
            CheckedBinaryOperator::Subtract,
            CheckedBinaryOperator::Multiply,
        ] {
            let mut module = checked_fixture(ty, operator, true);
            blocks(&mut module)[3].operations.retain(|operation| {
                !matches!(
                    operation.kind,
                    OperationKind::Unary {
                        operand: ValueId(30),
                        ..
                    } | OperationKind::Store {
                        pointer: ValueId(3),
                        ..
                    }
                )
            });
            with_input(module, |input, budget| {
                let tail = prepare_owned_licm_v1(input, budget).unwrap();
                budget.reserve_storage(tail.retained_storage()).unwrap();
                assert_eq!(tail.input_identity(), input.canonical().identity());
                assert_ne!(
                    tail.output().canonical().identity(),
                    input.canonical().identity()
                );
                assert_eq!(
                    tail.origins()
                        .iter()
                        .filter(|row| row.hoist.is_some())
                        .count(),
                    2
                );
                let moved = &tail.output().module().functions[0]
                    .body
                    .as_ref()
                    .unwrap()
                    .blocks[0]
                    .operations[2];
                assert_eq!(
                    moved.results,
                    [
                        ValueDef::new(ValueId(30), Type::Scalar(ty)),
                        ValueDef::new(ValueId(31), Type::BOOL)
                    ]
                );
                let (pair, storage) = tail.replay_against(input, budget).unwrap();
                budget.reserve_storage(storage.retained_storage()).unwrap();
                for (a, b) in [
                    (0, 0),
                    (mask, 1),
                    (mask / 2, 1),
                    (mask / 2 + 1, mask),
                    (mask, mask),
                ] {
                    let input_request = request(ty, bits, a, b, 2, true);
                    let mut old = Effects::default();
                    let mut new = Effects::default();
                    let x = sim(input)
                        .simulate_observed_with_sink(
                            &input_request,
                            SimulationTargetV1::amdgpu_64(),
                            SimulationLimitsV1::default(),
                            &mut old,
                        )
                        .unwrap();
                    let y = sim(tail.output())
                        .simulate_observed_with_sink(
                            &input_request,
                            SimulationTargetV1::amdgpu_64(),
                            SimulationLimitsV1::default(),
                            &mut new,
                        )
                        .unwrap();
                    assert_eq!(x.arguments(), y.arguments());
                    assert_eq!(translated_effects(&pair, &old.0), new.0);
                    assert_eq!(
                        x.buffer(4).unwrap().bytes()[0],
                        u8::from(!expected(operator, bits, signed, a, b).1)
                    );
                    assert_eq!(x.buffer(4).unwrap().initialized(), &[true, false]);
                    assert!(
                        x.buffer(3)
                            .unwrap()
                            .initialized()
                            .iter()
                            .all(|initialized| !initialized)
                    );
                    assert!(
                        x.buffer(3)
                            .unwrap()
                            .bytes()
                            .iter()
                            .all(|byte| *byte == 0xa5)
                    );
                }
                drop(pair);
                budget.release_storage(storage.retained_storage()).unwrap();
                release(tail, budget);
            });
        }
    }
}

#[test]
fn checked_licm_simulates_overflow_zero_trip_and_conditional_effects_against_integer_oracle() {
    for (ty, bits, signed) in shapes() {
        let mask = (1u128 << bits) - 1;
        for operator in [
            CheckedBinaryOperator::Add,
            CheckedBinaryOperator::Subtract,
            CheckedBinaryOperator::Multiply,
        ] {
            with_input(checked_fixture(ty, operator, true), |input, budget| {
                let tail = prepare_owned_licm_v1(input, budget).unwrap();
                budget.reserve_storage(tail.retained_storage()).unwrap();
                let (pair, size) = tail.replay_against(input, budget).unwrap();
                budget.reserve_storage(size.retained_storage()).unwrap();
                let (before, after) = (sim(input), sim(tail.output()));
                for (a, b) in [
                    (0, 0),
                    (mask, 1),
                    (mask / 2, 1),
                    (mask / 2 + 1, mask),
                    (mask, mask),
                ] {
                    for trips in [0, 1, 3] {
                        for enabled in [false, true] {
                            let input = request(ty, bits, a, b, trips, enabled);
                            let mut old = Effects::default();
                            let mut new = Effects::default();
                            let x = before
                                .simulate_observed_with_sink(
                                    &input,
                                    SimulationTargetV1::amdgpu_64(),
                                    SimulationLimitsV1::default(),
                                    &mut old,
                                )
                                .unwrap();
                            let y = after
                                .simulate_observed_with_sink(
                                    &input,
                                    SimulationTargetV1::amdgpu_64(),
                                    SimulationLimitsV1::default(),
                                    &mut new,
                                )
                                .unwrap();
                            assert_eq!(x.arguments(), y.arguments());
                            assert_eq!(translated_effects(&pair, &old.0), new.0);
                            let executed = enabled && trips != 0;
                            let (value, overflow) = expected(operator, bits, signed, a, b);
                            let bytes = (bits / 8) as usize;
                            let data = x.buffer(3).unwrap();
                            let flag = x.buffer(4).unwrap();
                            if executed {
                                assert_eq!(
                                    &data.bytes()[..bytes],
                                    &((!value) & mask).to_le_bytes()[..bytes]
                                );
                                assert_eq!(flag.bytes()[0], u8::from(!overflow));
                            } else {
                                assert_eq!(&data.bytes()[..bytes], vec![0xa5; bytes]);
                                assert_eq!(flag.bytes()[0], 0);
                            }
                            assert_eq!(&data.bytes()[bytes..], vec![0xa5; bytes]);
                            assert_eq!(&data.initialized()[..bytes], vec![executed; bytes]);
                            assert_eq!(&data.initialized()[bytes..], vec![false; bytes]);
                            assert_eq!(flag.initialized(), &[executed, false]);
                            assert_eq!(x.conflict_assessment(), y.conflict_assessment());
                        }
                    }
                }
                drop(pair);
                budget.release_storage(size.retained_storage()).unwrap();
                release(tail, budget);
            });
        }
    }
}

#[test]
fn checked_licm_keeps_loop_carried_pairs_and_undefined_division_at_original_sites() {
    let mut carried = checked_fixture(ScalarType::U32, CheckedBinaryOperator::Add, true);
    if let OperationKind::Binary { lhs, .. } = &mut blocks(&mut carried)[3].operations[0].kind {
        *lhs = ValueId(20);
    }
    with_input(carried, |input, budget| {
        let tail = prepare_owned_licm_v1(input, budget).unwrap();
        budget.reserve_storage(tail.retained_storage()).unwrap();
        assert!(tail.origins().iter().all(|row| row.hoist.is_none()));
        assert_eq!(
            tail.output().canonical().canonical_bytes(),
            input.canonical().canonical_bytes()
        );
        replay(&tail, input, budget);
        release(tail, budget);
    });
    let mut guarded = checked_fixture(ScalarType::U32, CheckedBinaryOperator::Add, true);
    blocks(&mut guarded)[3].operations.insert(
        3,
        op(
            60,
            u32_ty(),
            OperationKind::Binary {
                op: BinaryOp::Divide,
                lhs: ValueId(1),
                rhs: ValueId(2),
            },
        ),
    );
    with_input(guarded, |input, budget| {
        let tail = prepare_owned_licm_v1(input, budget).unwrap();
        budget.reserve_storage(tail.retained_storage()).unwrap();
        let (pair, size) = tail.replay_against(input, budget).unwrap();
        budget.reserve_storage(size.retained_storage()).unwrap();
        let division = pair
            .origins()
            .iter()
            .find(|row| row.input.block.block == 3 && row.input.operation == 3)
            .unwrap();
        assert!(division.hoist.is_none());
        for enabled in [false, true] {
            let request = request(ScalarType::U32, 32, 1, 0, 1, enabled);
            let mut old = Effects::default();
            let mut new = Effects::default();
            let x = sim(input).simulate_observed_with_sink(
                &request,
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1::default(),
                &mut old,
            );
            let y = sim(tail.output()).simulate_observed_with_sink(
                &request,
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1::default(),
                &mut new,
            );
            if enabled {
                let (
                    fe2o3_kir_sim::SimulationErrorV1::Execution(x),
                    fe2o3_kir_sim::SimulationErrorV1::Execution(y),
                ) = (x.unwrap_err(), y.unwrap_err())
                else {
                    panic!("division must fail during execution, not preflight");
                };
                assert_eq!(x.kind, y.kind);
                assert!(matches!(
                    x.kind,
                    fe2o3_kir_sim::SimulationExecutionErrorKindV1::UndefinedIntegerOperation(_)
                ));
            } else {
                assert_eq!(x.unwrap().arguments(), y.unwrap().arguments());
            }
            assert_eq!(translated_effects(&pair, &old.0), new.0);
        }
        drop(pair);
        budget.release_storage(size.retained_storage()).unwrap();
        release(tail, budget);
    });
}

#[test]
fn checked_licm_preserves_conditional_overflow_trap_and_pretrap_effects() {
    use fe2o3_kernel_ir::AmdGpuDiagnosticOperation as Diagnostic;
    let mut module = checked_fixture(ScalarType::U32, CheckedBinaryOperator::Add, true);
    let stores = blocks(&mut module)[3].operations.split_off(3);
    blocks(&mut module)[3].terminator = Some(conditional(31, 80, 70));
    let mut store = block(70, branch(50, &[]));
    store.operations = stores;
    let mut trap = block(80, Terminator::Unreachable);
    trap.operations.push(Diagnostic::Trap.operation(None));
    blocks(&mut module).extend([store, trap]);
    module.functions[0].required_capabilities = Diagnostic::Trap.required_capabilities();
    module.functions.push(Diagnostic::Trap.declaration());
    with_input(module, |input, budget| {
        let tail = prepare_owned_licm_v1(input, budget).unwrap();
        budget.reserve_storage(tail.retained_storage()).unwrap();
        let (pair, storage) = tail.replay_against(input, budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let trap = pair
            .origins()
            .iter()
            .find(|row| {
                input.module().functions[row.input.block.function.0 as usize]
                    .body
                    .as_ref()
                    .unwrap()
                    .blocks[row.input.block.block as usize]
                    .operations[row.input.operation as usize]
                    == Diagnostic::Trap.operation(None)
            })
            .unwrap();
        assert!(trap.hoist.is_none());
        assert_eq!(trap.input, trap.output);
        for (a, b) in [(1, 2), (u32::MAX as u128, 1)] {
            for trips in [0, 1, 3] {
                for enabled in [false, true] {
                    let input_request = request(ScalarType::U32, 32, a, b, trips, enabled);
                    let mut old = Effects::default();
                    let mut new = Effects::default();
                    let x = sim(input).simulate_observed_with_sink(
                        &input_request,
                        SimulationTargetV1::amdgpu_64(),
                        SimulationLimitsV1::default(),
                        &mut old,
                    );
                    let y = sim(tail.output()).simulate_observed_with_sink(
                        &input_request,
                        SimulationTargetV1::amdgpu_64(),
                        SimulationLimitsV1::default(),
                        &mut new,
                    );
                    if enabled && trips != 0 && a + b > u32::MAX as u128 {
                        let (
                            fe2o3_kir_sim::SimulationErrorV1::Execution(x),
                            fe2o3_kir_sim::SimulationErrorV1::Execution(y),
                        ) = (
                            x.err().expect("original overflow traps"),
                            y.err().expect("relocated overflow traps"),
                        )
                        else {
                            panic!("overflow trap must occur during execution, not preflight");
                        };
                        assert_eq!(x.kind, y.kind);
                        assert!(matches!(
                            x.kind,
                            fe2o3_kir_sim::SimulationExecutionErrorKindV1::ReachedUnreachable
                        ));
                        assert!(old.0.is_empty());
                    } else {
                        assert_eq!(x.unwrap().arguments(), y.unwrap().arguments());
                    }
                    assert_eq!(translated_effects(&pair, &old.0), new.0);
                }
            }
        }
        drop(pair);
        budget.release_storage(storage.retained_storage()).unwrap();
        release(tail, budget);
    });
}

#[test]
fn checked_licm_exact_work_and_initial_storage_limits_preserve_siblings() {
    let (input, retained) = admit(&checked_fixture(
        ScalarType::I64,
        CheckedBinaryOperator::Multiply,
        true,
    ));
    let sibling = vec![0xa5u8; 43];
    let floor = retained + sibling.capacity();
    let mut work = Work::new(WORK);
    let (needed, peak) = {
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let tail = prepare_owned_licm_v1(&input, &mut budget).unwrap();
        assert_eq!(
            tail.origins()
                .iter()
                .filter(|row| row.hoist.is_some())
                .count(),
            3
        );
        drop(tail);
        assert_eq!(budget.storage(), floor);
        (budget.work(), budget.peak_storage())
    };
    for limit in [needed, needed - 1] {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, peak);
        budget.reserve_storage(floor).unwrap();
        let result = prepare_owned_licm_v1(&input, &mut budget);
        if limit == needed {
            drop(result.unwrap());
        } else {
            assert!(
                matches!(result, Err(Error::Resource(Resource::Work(error))) if error.limit() == limit && error.actual() == needed)
            );
        }
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.failed_storage(), None);
        assert_eq!(budget.work(), limit);
    }
    let denied = floor + size_of::<Meter<'_, '_>>() + header().unwrap();
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, denied - 1);
    budget.reserve_storage(floor).unwrap();
    assert!(
        matches!(prepare_owned_licm_v1(&input, &mut budget), Err(Error::Resource(Resource::Storage(error))) if error.limit() == denied - 1 && error.actual() == denied)
    );
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.work(), 0);
    assert_eq!(budget.failed_storage(), Some(denied));
    drop(budget);
    let mut work = Work::new(needed);
    let mut budget = Budget::new(&mut work, peak - 1);
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let error = prepare_owned_licm_v1(&input, &mut budget)
        .err()
        .expect("measured peak-minus-one must refuse");
    assert!(
        matches!(error, Error::Pair(PairError::Loops(LoopError::Resource(Resource::Storage(limit))))
        if limit.limit() == peak - 1 && limit.actual() == peak)
    );
    assert_eq!(budget.failed_storage(), Some(peak));
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert!(budget.work() < needed);
    assert!(budget.peak_storage() < peak);
    assert_eq!(sibling, vec![0xa5; 43]);
}

fn deterministic_checked_record() -> String {
    let mut record = String::new();
    with_input(
        checked_fixture(ScalarType::I64, CheckedBinaryOperator::Multiply, true),
        |input, budget| {
            let tail = prepare_owned_licm_v1(input, budget).unwrap();
            budget.reserve_storage(tail.retained_storage()).unwrap();
            record = format!(
                "{:?}|{:?}|{}|{}",
                tail.output().canonical().canonical_bytes(),
                tail.origins(),
                budget.work(),
                budget.peak_storage()
            );
            release(tail, budget);
        },
    );
    record
}

#[test]
fn checked_licm_deterministic_child() {
    println!("CHECKED_LICM_V44 {}", deterministic_checked_record());
}

#[test]
fn checked_licm_two_processes_preserve_bytes_lineage_and_resource_counts() {
    let expected = deterministic_checked_record();
    let child = "owned_licm_v1::tests::checked_v44_tests::checked_licm_deterministic_child";
    for _ in 0..2 {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", child, "--nocapture", "--test-threads=1"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert_eq!(
            stdout
                .lines()
                .filter_map(|line| line.strip_prefix("CHECKED_LICM_V44 "))
                .collect::<Vec<_>>(),
            [expected.as_str()]
        );
        assert!(stdout.contains("1 passed; 0 failed; 0 ignored"));
    }
}
