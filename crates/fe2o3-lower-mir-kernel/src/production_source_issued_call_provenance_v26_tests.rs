fn pointer(space: AddressSpace, access: AccessMode) -> Type {
    Type::pointer(Type::Scalar(ScalarType::U32), space, access)
}

pub(super) fn transport_graph(fault: u8) -> Function {
    let global = pointer(AddressSpace::Global, AccessMode::ReadWrite);
    let generic = pointer(AddressSpace::Generic, AccessMode::ReadWrite);
    let readonly = pointer(AddressSpace::Generic, AccessMode::ReadOnly);
    let mut entry = BasicBlock::new(BlockId(10));
    for (input, output) in [(0, 3), (1, 4)] {
        entry.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(output), generic.clone()),
            OperationKind::Cast {
                kind: if fault == 1 && input == 0 {
                    CastKind::Bitcast
                } else {
                    CastKind::PointerToGeneric
                },
                value: ValueId(if fault == 2 && input == 0 { 1 } else { input }),
                to: if fault == 3 && input == 0 {
                    Type::pointer(
                        Type::Scalar(ScalarType::U64),
                        AddressSpace::Generic,
                        AccessMode::ReadWrite,
                    )
                } else {
                    generic.clone()
                },
            },
        ));
    }
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(20),
        arguments: vec![ValueId(if fault == 4 { 4 } else { 3 })],
    });
    let mut middle = BasicBlock::new(BlockId(20));
    middle
        .parameters
        .push(ValueDef::new(ValueId(5), generic.clone()));
    middle.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(6), readonly.clone()),
        OperationKind::Cast {
            kind: if fault == 5 {
                CastKind::PointerToGeneric
            } else {
                CastKind::RestrictPointerAccess
            },
            value: ValueId(5),
            to: readonly.clone(),
        },
    ));
    middle.terminator = Some(Terminator::Branch {
        target: BlockId(30),
        arguments: vec![ValueId(6)],
    });
    let mut end = BasicBlock::new(BlockId(30));
    end.parameters.push(ValueDef::new(ValueId(7), readonly));
    end.terminator = Some(Terminator::Return { values: vec![] });
    if fault == 6 {
        middle.parameters[0].ty = pointer(AddressSpace::Global, AccessMode::ReadWrite);
    }
    if fault == 7 {
        middle.operations[0].results[0].ty = pointer(AddressSpace::Generic, AccessMode::ReadWrite);
    }
    if fault == 8 {
        middle.terminator = Some(Terminator::Branch {
            target: BlockId(20),
            arguments: vec![ValueId(5)],
        });
        entry.terminator = Some(Terminator::Branch {
            target: BlockId(30),
            arguments: vec![ValueId(7)],
        });
    }
    if fault == 9 {
        entry.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(2),
            then_target: BlockId(20),
            then_arguments: vec![ValueId(3)],
            else_target: BlockId(20),
            else_arguments: vec![ValueId(4)],
        });
    }
    let input = match fault {
        10 => pointer(AddressSpace::Private, AccessMode::ReadWrite),
        11 => pointer(AddressSpace::Workgroup, AccessMode::ReadWrite),
        14 => pointer(AddressSpace::Global, AccessMode::ReadOnly),
        15 => Type::pointer(
            Type::Scalar(ScalarType::U64),
            AddressSpace::Global,
            AccessMode::ReadWrite,
        ),
        _ => global.clone(),
    };
    Function::definition(
        "issued-call-transport",
        fe2o3_kernel_ir::Signature::new(vec![input, global, Type::BOOL], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![entry, middle, end],
    )
}

fn run_transport(
    fault: u8,
    work: usize,
    storage: usize,
) -> (
    Result<(), ProductionSemanticKirErrorV1>,
    usize,
    usize,
    usize,
) {
    let function = transport_graph(fault);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage);
    budget.reserve_storage(17).unwrap();
    let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
        let actual = SourceIssuedActualV29::from_function(&function, budget)?;
        check_source_issued_pointer_transports_v26(
            &function,
            &actual,
            &[
                SourceIssuedPointerTransportV26 {
                    pointer: ValueId(0),
                    issuer: ValueId(0),
                },
                SourceIssuedPointerTransportV26 {
                    pointer: ValueId(5),
                    issuer: ValueId(0),
                },
                SourceIssuedPointerTransportV26 {
                    pointer: ValueId(if fault == 13 { 4 } else { 7 }),
                    issuer: ValueId(if fault == 12 { 1 } else { 0 }),
                },
            ],
            budget,
        )
    });
    (
        result,
        budget.work(),
        budget.storage(),
        budget.peak_storage(),
    )
}

#[test]
fn issued_call_pointer_replay_checks_casts_edges_pointee_access_and_exact_global_origin() {
    let (result, _, held, _) = run_transport(0, usize::MAX, usize::MAX);
    result.unwrap();
    assert_eq!(held, 17);
    for fault in 1..=15 {
        let (result, _, held, _) = run_transport(fault, usize::MAX, usize::MAX);
        assert!(
            matches!(
                result,
                Err(ProductionSemanticKirErrorV1::Unsupported { .. })
            ),
            "fault {fault}: {result:?}"
        );
        assert_eq!(held, 17, "fault {fault}");
    }
}

#[test]
fn issued_call_pointer_replay_has_exact_and_one_short_cumulative_limits() {
    let (result, work, held, peak) = run_transport(0, usize::MAX, usize::MAX);
    result.unwrap();
    let (result, exact_work, exact_held, exact_peak) = run_transport(0, work, peak);
    result.unwrap();
    assert_eq!((exact_work, exact_held, exact_peak), (work, held, peak));
    for (work_limit, storage_limit, short_work) in [(work - 1, peak, true), (work, peak - 1, false)]
    {
        let (result, _, held, _) = run_transport(0, work_limit, storage_limit);
        assert!(
            matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    )
                )
            ) && short_work
                || matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Storage(_)
                        )
                    )
                ) && !short_work,
            "{result:?}"
        );
        assert_eq!(held, 17);
    }
}

#[test]
fn issued_call_global_origin_classification_is_scoped_and_not_source_authority() {
    for (fault, expected) in [
        (0, Some(ValueId(0))),
        (2, Some(ValueId(1))),
        (1, None),
        (9, None),
        (10, None),
        (11, None),
        (14, None),
        (15, None),
    ] {
        let function = transport_graph(fault);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(17).unwrap();
        assert_eq!(
            source_issued_global_pointer_origin_v26(&function, ValueId(7), &mut budget).unwrap(),
            expected,
            "fault {fault}"
        );
        assert_eq!(budget.storage(), 17);
    }
    // A different valid Global origin is classifiable, but it still fails the
    // exact retained issuer relation tested by the transport parent above.
    let function = transport_graph(0);
    let run = |work_limit, storage_limit| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(17).unwrap();
        let result = source_issued_global_pointer_origin_v26(&function, ValueId(7), &mut budget);
        assert_eq!(budget.storage(), 17);
        (result, budget.work(), budget.peak_storage())
    };
    let (result, work, peak) = run(usize::MAX, usize::MAX);
    assert_eq!(result.unwrap(), Some(ValueId(0)));
    let (result, exact_work, exact_peak) = run(work, peak);
    assert_eq!(result.unwrap(), Some(ValueId(0)));
    assert_eq!((exact_work, exact_peak), (work, peak));
    assert!(matches!(
        run(work - 1, peak).0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(
                _
            ))
        )
    ));
    assert!(matches!(
        run(work, peak - 1).0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(_)
            )
        )
    ));
}

#[test]
fn issued_call_pointer_quote_has_independent_header_and_work_equations() {
    // Seven indexed definitions need four binary-search work units. The walk
    // can visit at most eight links for each of the three requested relations.
    let expected_work = 3 * 8 * (40 + 3 * 4);
    let expected_header = std::mem::size_of::<(
        [usize; 16],
        Option<ValueId>,
        Result<Option<ValueId>, ProductionSemanticKirErrorV1>,
    )>();
    for short in [0, 1, 2] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(expected_work - usize::from(short == 1));
        let limit = 17 + expected_header - usize::from(short == 2);
        let mut budget = ArgumentBudgetV1::new(&mut work, limit);
        budget.reserve_storage(17).unwrap();
        let result = source_issued_pointer_walk_quote_v26(7, 3, &mut budget);
        match short {
            0 => {
                result.unwrap();
                assert_eq!(
                    (budget.work(), budget.storage()),
                    (expected_work, 17 + expected_header)
                );
                budget.release_storage(expected_header).unwrap();
            }
            1 => assert!(
                matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(error))) if error.actual() == expected_work && error.limit() == expected_work - 1)
            ),
            2 => assert!(
                matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(error))) if error.actual() == 17 + expected_header && error.limit() == limit)
            ),
            _ => unreachable!(),
        }
        assert_eq!(budget.storage(), 17);
    }
}

#[test]
fn issued_call_transport_preserves_both_restriction_and_generic_cast_orders() {
    for restriction_first in [false, true] {
        for fault in 0..3 {
            let mut function = transport_graph(0);
            let body = function.body.as_mut().unwrap();
            if restriction_first {
                let global_read = pointer(AddressSpace::Global, AccessMode::ReadOnly);
                body.blocks[0].operations[0].results[0].ty = global_read.clone();
                body.blocks[0].operations[0].kind = OperationKind::Cast {
                    kind: CastKind::RestrictPointerAccess,
                    value: ValueId(0),
                    to: global_read.clone(),
                };
                body.blocks[1].parameters[0].ty = global_read;
                body.blocks[1].operations[0].kind = OperationKind::Cast {
                    kind: CastKind::PointerToGeneric,
                    value: ValueId(5),
                    to: pointer(AddressSpace::Generic, AccessMode::ReadOnly),
                };
            }
            let operation = &mut body.blocks[0].operations[0];
            let OperationKind::Cast { kind, value, .. } = &mut operation.kind else {
                unreachable!()
            };
            match fault {
                0 => (),
                1 => *kind = CastKind::Bitcast,
                2 => *value = ValueId(1),
                _ => unreachable!(),
            }
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
            budget.reserve_storage(17).unwrap();
            let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
                let actual = SourceIssuedActualV29::from_function(&function, budget)?;
                check_source_issued_pointer_transports_v26(
                    &function,
                    &actual,
                    &[SourceIssuedPointerTransportV26 {
                        pointer: ValueId(7),
                        issuer: ValueId(0),
                    }],
                    budget,
                )?;
                if restriction_first {
                    check_source_issued_pointer_transports_v26(
                        &function,
                        &actual,
                        &[SourceIssuedPointerTransportV26 {
                            pointer: ValueId(7),
                            issuer: ValueId(3),
                        }],
                        budget,
                    )?;
                }
                Ok(())
            });
            assert_eq!(
                result.is_ok(),
                fault == 0,
                "restriction_first={restriction_first}, fault={fault}: {result:?}"
            );
            assert_eq!(budget.storage(), 17);
            if fault == 0 {
                assert_eq!(
                    source_issued_global_pointer_origin_v26(&function, ValueId(7), &mut budget)
                        .unwrap(),
                    Some(ValueId(0))
                );
                assert_eq!(budget.storage(), 17);
            }
        }
    }
}
