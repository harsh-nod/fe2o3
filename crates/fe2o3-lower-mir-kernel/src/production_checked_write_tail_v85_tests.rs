use super::*;

fn write_tail(precondition: bool, element: ScalarType) -> (CheckedWriteInputsV85, Vec<Operation>) {
    let inputs = CheckedWriteInputsV85 {
        slice: ValueId(0),
        index: ValueId(1),
        precondition: precondition.then_some(ValueId(2)),
        value: ValueId(3),
        element,
    };
    let pointer = Type::pointer(
        Type::Scalar(element),
        AddressSpace::Global,
        AccessMode::WriteOnly,
    );
    let predicate = ValueId(if precondition { 6 } else { 5 });
    let mut rows = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(4), Type::INDEX),
            OperationKind::SliceLength {
                slice: inputs.slice,
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(5), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: inputs.index,
                rhs: ValueId(4),
            },
        ),
    ];
    if precondition {
        rows.push(Operation::effect_free(
            ValueDef::new(predicate, Type::BOOL),
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: ValueId(2),
                rhs: ValueId(5),
            },
        ));
    }
    rows.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(7), Type::INDEX),
            OperationKind::Constant(Constant::Index(0)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(8), Type::INDEX),
            OperationKind::Select {
                condition: predicate,
                true_value: inputs.index,
                false_value: ValueId(7),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(9), pointer.clone()),
            OperationKind::SliceData {
                slice: inputs.slice,
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(10), pointer),
            OperationKind::GetElementPointer {
                base: ValueId(9),
                offset: ValueId(8),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::GuardedStore {
                pointer: ValueId(10),
                predicate,
                value: inputs.value,
                access: MemoryAccess::new(
                    AddressSpace::Global,
                    strided_read_scalar_alignment_v1(&Type::Scalar(element)).unwrap(),
                ),
            },
        ),
    ]);
    (inputs, rows)
}

fn check(
    inputs: CheckedWriteInputsV85,
    rows: &[Operation],
) -> Result<CheckedWriteTailV85, ProductionSemanticKirErrorV1> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(128);
    let mut budget = ArgumentBudgetV1::new(&mut work, 0);
    let result = check_checked_write_tail_v85(inputs, rows, &mut budget);
    assert_eq!(budget.storage(), 0);
    assert_eq!(budget.work(), 128);
    // Replay borrows actual mapped operations; it need not own a contiguous copy.
    let borrowed: Vec<_> = rows.iter().collect();
    let mut replay_work = CanonicalKernelIrWorkBudgetV1::new(128);
    let mut replay_budget = ArgumentBudgetV1::new(&mut replay_work, 0);
    let replay = check_checked_write_tail_v85(inputs, &borrowed, &mut replay_budget);
    match (&result, &replay) {
        (Ok(expected), Ok(actual)) => assert_eq!(expected, actual),
        (Err(_), Err(_)) => {}
        _ => panic!("owned/borrowed suffix differs: {result:?} / {replay:?}"),
    }
    assert_eq!(replay_budget.storage(), 0);
    assert_eq!(replay_budget.work(), 128);
    result
}

#[test]
fn checked_write_suffix_replays_both_guard_shapes_and_scalar_layouts() {
    for precondition in [false, true] {
        for element in [
            ScalarType::U8,
            ScalarType::U16,
            ScalarType::U32,
            ScalarType::U64,
            ScalarType::F32,
            ScalarType::F64,
        ] {
            let (inputs, rows) = write_tail(precondition, element);
            assert_eq!(
                check(inputs, &rows).unwrap(),
                CheckedWriteTailV85 {
                    length: ValueId(4),
                    extent: ValueId(5),
                    predicate: ValueId(if precondition { 6 } else { 5 }),
                    zero: ValueId(7),
                    offset: ValueId(8),
                    data: ValueId(9),
                    pointer: ValueId(10),
                }
            );
        }
    }
}

#[test]
fn checked_write_suffix_rejects_changed_operands_types_effects_and_ssa() {
    for precondition in [false, true] {
        for fault in 0..34 {
            let (mut inputs, mut rows) = write_tail(precondition, ScalarType::U32);
            let extra = usize::from(precondition);
            let zero = 2 + extra;
            let select = 3 + extra;
            let data = 4 + extra;
            let address = 5 + extra;
            let store = 6 + extra;
            match fault {
                0 => rows[0].kind = OperationKind::SliceLength { slice: ValueId(99) },
                1 => {
                    rows[1].kind = OperationKind::Compare {
                        predicate: ComparePredicate::LessThanOrEqual,
                        lhs: inputs.index,
                        rhs: ValueId(4),
                    }
                }
                2 => {
                    rows[1].kind = OperationKind::Compare {
                        predicate: ComparePredicate::LessThan,
                        lhs: ValueId(99),
                        rhs: ValueId(4),
                    }
                }
                3 => {
                    rows[1].kind = OperationKind::Compare {
                        predicate: ComparePredicate::LessThan,
                        lhs: inputs.index,
                        rhs: ValueId(99),
                    }
                }
                4 => rows[zero].kind = OperationKind::Constant(Constant::Index(1)),
                5 => rows[zero].kind = OperationKind::Constant(Constant::U64(0)),
                6 => {
                    let OperationKind::Select { condition, .. } = &mut rows[select].kind else {
                        unreachable!()
                    };
                    *condition = ValueId(99);
                }
                7 => {
                    let OperationKind::Select { true_value, .. } = &mut rows[select].kind else {
                        unreachable!()
                    };
                    *true_value = ValueId(99);
                }
                8 => {
                    let OperationKind::Select { false_value, .. } = &mut rows[select].kind else {
                        unreachable!()
                    };
                    *false_value = ValueId(99);
                }
                9 => rows[data].kind = OperationKind::SliceData { slice: ValueId(99) },
                10 => {
                    rows[address].kind = OperationKind::GetElementPointer {
                        base: ValueId(99),
                        offset: ValueId(8),
                    }
                }
                11 => {
                    rows[address].kind = OperationKind::GetElementPointer {
                        base: ValueId(9),
                        offset: inputs.index,
                    }
                }
                12 => {
                    let OperationKind::GuardedStore { pointer, .. } = &mut rows[store].kind else {
                        unreachable!()
                    };
                    *pointer = ValueId(99);
                }
                13 => {
                    let OperationKind::GuardedStore { predicate, .. } = &mut rows[store].kind
                    else {
                        unreachable!()
                    };
                    *predicate = ValueId(99);
                }
                14 => {
                    let OperationKind::GuardedStore { value, .. } = &mut rows[store].kind else {
                        unreachable!()
                    };
                    *value = ValueId(99);
                }
                15 => {
                    let OperationKind::GuardedStore { access, .. } = &mut rows[store].kind else {
                        unreachable!()
                    };
                    access.volatile = true;
                }
                16 => {
                    let OperationKind::GuardedStore { access, .. } = &mut rows[store].kind else {
                        unreachable!()
                    };
                    *access = MemoryAccess::new(AddressSpace::Private, 4);
                }
                17 => {
                    let OperationKind::GuardedStore { access, .. } = &mut rows[store].kind else {
                        unreachable!()
                    };
                    *access = MemoryAccess::new(AddressSpace::Global, 1);
                }
                18 => {
                    rows[store].kind = OperationKind::Store {
                        pointer: ValueId(10),
                        value: inputs.value,
                        access: MemoryAccess::new(AddressSpace::Global, 4),
                    }
                }
                19 => rows[store]
                    .results
                    .push(ValueDef::new(ValueId(99), Type::BOOL)),
                20 => rows[0].results.clear(),
                21 => rows[1].results[0].ty = Type::INDEX,
                22 => rows[zero].results[0].ty = Type::Scalar(ScalarType::U64),
                23 => rows[select].results[0].ty = Type::Scalar(ScalarType::U64),
                24 => {
                    rows[data].results[0].ty = Type::pointer(
                        Type::Scalar(ScalarType::U32),
                        AddressSpace::Global,
                        AccessMode::ReadWrite,
                    )
                }
                25 => {
                    rows[address].results[0].ty = Type::pointer(
                        Type::Scalar(ScalarType::U64),
                        AddressSpace::Global,
                        AccessMode::WriteOnly,
                    )
                }
                26 => {
                    rows[address].results[0].ty = Type::pointer(
                        Type::Scalar(ScalarType::U32),
                        AddressSpace::Private,
                        AccessMode::WriteOnly,
                    )
                }
                27 => {
                    rows.push(rows[store].clone());
                }
                28 => {
                    rows.remove(0);
                }
                29 => rows.swap(data, address),
                30 => inputs.precondition = if precondition { None } else { Some(ValueId(2)) },
                31 => {
                    // Keep the tail equations consistent while aliasing two definitions.
                    rows[zero].results[0].id = ValueId(4);
                    let OperationKind::Select { false_value, .. } = &mut rows[select].kind else {
                        unreachable!()
                    };
                    *false_value = ValueId(4);
                }
                32 => {
                    // Keep all pointer uses consistent while clobbering an input.
                    rows[address].results[0].id = inputs.slice;
                    let OperationKind::GuardedStore { pointer, .. } = &mut rows[store].kind else {
                        unreachable!()
                    };
                    *pointer = inputs.slice;
                }
                33 => rows[0]
                    .results
                    .push(ValueDef::new(ValueId(99), Type::INDEX)),
                _ => unreachable!(),
            }
            assert!(
                check(inputs, &rows).is_err(),
                "precondition={precondition}, fault={fault}"
            );
        }
    }
    for fault in 0..5 {
        let (inputs, mut rows) = write_tail(true, ScalarType::U32);
        match fault {
            0 => {
                rows[2].kind = OperationKind::Binary {
                    op: BinaryOp::BitOr,
                    lhs: ValueId(2),
                    rhs: ValueId(5),
                }
            }
            1 => {
                rows[2].kind = OperationKind::Binary {
                    op: BinaryOp::BitAnd,
                    lhs: ValueId(99),
                    rhs: ValueId(5),
                }
            }
            2 => {
                rows[2].kind = OperationKind::Binary {
                    op: BinaryOp::BitAnd,
                    lhs: ValueId(2),
                    rhs: ValueId(99),
                }
            }
            3 => rows[2].results[0].ty = Type::INDEX,
            4 => {
                // Matching Select/store predicates must still retain the mapping precondition.
                let OperationKind::Select { condition, .. } = &mut rows[4].kind else {
                    unreachable!()
                };
                *condition = ValueId(5);
                let OperationKind::GuardedStore { predicate, .. } = &mut rows[7].kind else {
                    unreachable!()
                };
                *predicate = ValueId(5);
            }
            _ => unreachable!(),
        }
        assert!(
            check(inputs, &rows).is_err(),
            "combined predicate fault={fault}"
        );
    }
}

#[test]
fn checked_write_suffix_precharges_exact_work_and_borrows_storage() {
    type Inputs = (ValueId, ValueId, Option<ValueId>, ValueId, ScalarType);
    type Output = [ValueId; 7];
    type Frame<'a> = (
        Inputs,
        Output,
        &'a [Operation],
        [&'a Operation; 8],
        [&'a ValueDef; 7],
        [ValueId; 11],
        [usize; 4],
        Result<Output, ProductionSemanticKirErrorV1>,
        Result<(), ProductionSemanticKirErrorV1>,
        Option<u32>,
    );
    assert_eq!(size_of::<CheckedWriteInputsV85>(), size_of::<Inputs>());
    assert_eq!(size_of::<CheckedWriteTailV85>(), size_of::<Output>());
    let header = size_of::<Frame<'_>>() + std::mem::align_of::<Frame<'_>>();
    assert_eq!(checked_write_tail_headers_v85().unwrap(), header);
    let (inputs, rows) = write_tail(true, ScalarType::U32);
    for storage in [header, header - 1] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(128);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage);
        let reserved = budget.reserve_storage(header);
        if storage == header {
            reserved.unwrap();
            check_checked_write_tail_v85(inputs, &rows, &mut budget).unwrap();
            assert_eq!(budget.storage(), header);
            budget.release_storage(header).unwrap();
        } else {
            assert!(matches!(reserved, Err(ArgumentResourceV1::Storage(error))
                if error.actual() == header && error.limit() == header - 1));
            assert_eq!(budget.work(), 0);
            assert_eq!(budget.failed_storage(), Some(header));
            for _ in 0..2 {
                assert!(matches!(budget.check_prior_denials_v1(),
                    Err(ArgumentResourceV1::Storage(error))
                    if error.actual() == header && error.limit() == header - 1));
            }
            assert_eq!(budget.work(), 0);
        }
        assert_eq!(budget.storage(), 0);
    }
    for limit in [128, 127] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        let result = check_checked_write_tail_v85(inputs, &rows, &mut budget);
        if limit == 128 {
            result.unwrap();
            assert_eq!(budget.work(), 128);
            assert_eq!(budget.failed_work(), None);
        } else {
            assert!(
                matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Work(error))) if error.actual() == 128 && error.limit() == 127)
            );
            assert_eq!(budget.failed_work(), Some(128));
            for _ in 0..2 {
                assert!(matches!(budget.check_prior_denials_v1(),
                    Err(ArgumentResourceV1::Work(error))
                    if error.actual() == 128 && error.limit() == 127));
            }
            assert_eq!(budget.work(), 0);
        }
        assert_eq!(budget.storage(), 0);
    }
}
