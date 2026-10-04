use super::*;

mod checked_write_v85 {
    include!("production_checked_write_tail_v85_tests.rs");
}

// These are independent graph/equation controls, not original-source admission.
// The unchanged actual rustc source and native-policy matrices exercise that
// distinct boundary through the production source-access census.
fn physical() -> SourceIssuedPhysicalV29 {
    SourceIssuedPhysicalV29 {
        present: ValueId(3),
        pointer: ValueId(5),
        element: ScalarType::U32,
        access: AccessMode::ReadWrite,
    }
}

fn tail() -> [Operation; 4] {
    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    [
        Operation::effect_free(
            ValueDef::new(ValueId(2), Type::INDEX),
            OperationKind::SliceLength { slice: ValueId(0) },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(3), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(1),
                rhs: ValueId(2),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(4), pointer.clone()),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(5), pointer),
            OperationKind::GetElementPointer {
                base: ValueId(4),
                offset: ValueId(1),
            },
        ),
    ]
}

fn check_tail(
    rows: &[Operation; 4],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    check_source_issued_tail_v29(
        physical(),
        ValueId(0),
        ValueId(1),
        &rows[0],
        &rows[1],
        &rows[2],
        &rows[3],
        budget,
    )
}

#[test]
fn issued_pointer_tail_preserves_receiver_index_predicate_type_and_result_identity() {
    for fault in 0..12 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        let mut rows = tail();
        check_tail(&rows, &mut budget).unwrap();
        match fault {
            0 => rows[0].kind = OperationKind::SliceLength { slice: ValueId(7) },
            1 => {
                rows[1].kind = OperationKind::Compare {
                    predicate: ComparePredicate::LessThanOrEqual,
                    lhs: ValueId(1),
                    rhs: ValueId(2),
                }
            }
            2 => {
                rows[1].kind = OperationKind::Compare {
                    predicate: ComparePredicate::LessThan,
                    lhs: ValueId(7),
                    rhs: ValueId(2),
                }
            }
            3 => {
                rows[1].kind = OperationKind::Compare {
                    predicate: ComparePredicate::LessThan,
                    lhs: ValueId(1),
                    rhs: ValueId(7),
                }
            }
            4 => rows[2].kind = OperationKind::SliceData { slice: ValueId(7) },
            5 => {
                rows[3].kind = OperationKind::GetElementPointer {
                    base: ValueId(7),
                    offset: ValueId(1),
                }
            }
            6 => {
                rows[3].kind = OperationKind::GetElementPointer {
                    base: ValueId(4),
                    offset: ValueId(7),
                }
            }
            7 => rows[3].results[0].id = ValueId(7),
            8 => rows[1].results[0].id = ValueId(7),
            9 => {
                rows[2].results[0].ty = Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                )
            }
            10 => {
                rows[3].results[0].ty = Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadOnly,
                )
            }
            11 => rows[3].results.push(ValueDef::new(ValueId(7), Type::INDEX)),
            _ => unreachable!(),
        }
        let error = check_tail(&rows, &mut budget).unwrap_err();
        assert!(
            matches!(error, ProductionSemanticKirErrorV1::Unsupported { detail, .. }
            if detail == "source issued pointer differs from its original issuer or actual guard"),
            "fault {fault}: {error:?}"
        );
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn issued_pointer_tail_has_independent_exact_and_one_short_work() {
    for work_limit in [22, 21] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        let result = check_tail(&tail(), &mut budget);
        if work_limit == 22 {
            result.unwrap();
        } else {
            assert!(
                matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
            ArgumentResourceV1::Work(error))) if error.actual() == 22 && error.limit() == 21)
            );
        }
        assert_eq!(budget.storage(), 0);
    }
}

fn graph(
    ty: Type,
    cases: Vec<fe2o3_kernel_ir::IntegerSwitchCase>,
    default_target: BlockId,
    cast: CastKind,
) -> Function {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(1), ty.clone()),
        OperationKind::Cast {
            kind: cast,
            value: ValueId(0),
            to: ty,
        },
    ));
    entry.terminator = Some(Terminator::IntegerSwitch {
        selector: ValueId(1),
        cases,
        default_target,
        default_arguments: vec![],
    });
    let mut success = BasicBlock::new(BlockId(1));
    success.terminator = Some(Terminator::Return { values: vec![] });
    let mut absent = BasicBlock::new(BlockId(2));
    absent.terminator = Some(Terminator::Return { values: vec![] });
    Function::definition(
        "issued_pointer_guard",
        fe2o3_kernel_ir::Signature::new(vec![Type::BOOL], vec![]),
        vec![ValueId(0)],
        vec![entry, success, absent],
    )
}

fn case(value: Constant, target: u32) -> fe2o3_kernel_ir::IntegerSwitchCase {
    fe2o3_kernel_ir::IntegerSwitchCase {
        value,
        target: BlockId(target),
        arguments: vec![],
    }
}

fn guard_result(function: &Function) -> Result<bool, ProductionSemanticKirErrorV1> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
    let values = SourceIssuedActualV29::from_function(function, &mut budget)?;
    let guards = source_issued_guards_v29(function, &values, &mut budget)?;
    let mut result = false;
    fe2o3_kernel_ir::with_function_control_flow_v1(
        function,
        Default::default(),
        &mut budget,
        |view| {
            for guard in &guards {
                assert_eq!(guard.present, ValueId(0));
                result |= view.success_edge_dominates(guard.block, guard.edge, BlockId(1))?;
            }
            Ok(())
        },
    )
    .map_err(|error| match error {
        fe2o3_kernel_ir::FunctionControlFlowScopeErrorV1::Resource(error) => error.into(),
        _ => source_issued_error_v29(),
    })?;
    Ok(result)
}

#[test]
fn issued_pointer_actual_guards_keep_case_default_and_duplicate_edge_roles() {
    for (zero, one, other) in [
        (Constant::I8(0), Constant::I8(1), Constant::I8(2)),
        (Constant::U32(0), Constant::U32(1), Constant::U32(2)),
        (Constant::U64(0), Constant::U64(1), Constant::U64(2)),
        (Constant::Index(0), Constant::Index(1), Constant::Index(2)),
    ] {
        let ty = zero.ty();
        assert!(
            guard_result(&graph(
                ty.clone(),
                vec![case(one.clone(), 1)],
                BlockId(2),
                CastKind::ZeroExtend
            ))
            .unwrap()
        );
        assert!(
            guard_result(&graph(
                ty.clone(),
                vec![case(zero.clone(), 2)],
                BlockId(1),
                CastKind::ZeroExtend
            ))
            .unwrap()
        );
        // Both booleans going to default is not a guard, despite dominance.
        assert!(
            !guard_result(&graph(
                ty.clone(),
                vec![case(other, 2)],
                BlockId(1),
                CastKind::ZeroExtend
            ))
            .unwrap()
        );
        assert!(
            !guard_result(&graph(
                ty.clone(),
                vec![case(zero.clone(), 1), case(one.clone(), 1)],
                BlockId(2),
                CastKind::ZeroExtend
            ))
            .unwrap()
        );
        assert!(
            !guard_result(&graph(
                ty.clone(),
                vec![case(one.clone(), 1)],
                BlockId(2),
                CastKind::SignExtend
            ))
            .unwrap()
        );
        let duplicate = guard_result(&graph(
            ty,
            vec![case(one.clone(), 1), case(one, 2)],
            BlockId(2),
            CastKind::ZeroExtend,
        ))
        .unwrap_err();
        assert!(
            matches!(duplicate, ProductionSemanticKirErrorV1::Unsupported { detail, .. }
            if detail == "source issued pointer differs from its original issuer or actual guard")
        );
    }
}

#[test]
fn issued_pointer_actual_value_index_has_independent_resources_and_loglinear_work() {
    for count in [1usize, 16, 64, 1024] {
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let function = Function::definition(
            "issued_pointer_index",
            fe2o3_kernel_ir::Signature::new(vec![Type::INDEX; count], vec![]),
            (0..count).map(|id| ValueId(id as u32)).collect(),
            vec![block],
        );
        let logarithm = (usize::BITS - count.leading_zeros()) as usize + 1;
        // Two block visits (2+1), constructor(3), N copies, N-1 adjacent
        // identity checks, and the existing prepaid sort bound 4*N*log.
        let expected_work = 5 + 2 * count + 4 * count * logarithm;
        let expected_storage = std::mem::size_of::<SourceIssuedActualV29<'_>>()
            + std::mem::size_of::<Result<SourceIssuedActualV29<'_>, ProductionSemanticKirErrorV1>>(
            )
            + std::mem::size_of::<Vec<SourceIssuedActualValueV29<'_>>>()
            + std::mem::size_of::<
                Result<Vec<SourceIssuedActualValueV29<'_>>, ProductionSemanticKirErrorV1>,
            >()
            + count * std::mem::size_of::<SourceIssuedActualValueV29<'_>>();
        for (work_limit, storage_limit, succeeds) in [
            (expected_work, expected_storage, true),
            (expected_work - 1, expected_storage, false),
            (expected_work, expected_storage - 1, false),
        ] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
            let result = SourceIssuedActualV29::from_function(&function, &mut budget);
            if succeeds {
                let index = result.unwrap();
                assert_eq!(index.values.len(), count);
                assert_eq!(budget.work(), expected_work);
                assert_eq!(budget.storage(), expected_storage);
                drop(index);
            } else {
                let error = match result {
                    Err(error) => error,
                    Ok(_) => panic!("one-short index must refuse"),
                };
                if work_limit < expected_work {
                    assert!(
                        matches!(error, ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(error)) if error.actual() == expected_work && error.limit() == work_limit),
                        "{error:?}"
                    );
                } else {
                    assert!(
                        matches!(error, ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(error)) if error.actual() == expected_storage && error.limit() == storage_limit),
                        "{error:?}"
                    );
                }
            }
            // This test owns the complete closed constructor scratch, including
            // a failed reservation's envelopes; no source receipt is involved.
            budget.release_storage(budget.storage()).unwrap();
            assert_eq!(budget.storage(), 0);
        }
        assert!(expected_work < 8 * count * logarithm + 16);
    }
}

#[test]
fn issued_pointer_actual_root_input_rejects_same_typed_parameter_permutation() {
    let ty = Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let archived = SemanticValueBindingV1::Value {
        id: ValueId(11),
        ty: ty.clone(),
    };
    for fault in 0..6 {
        let mut block = BasicBlock::new(BlockId(0));
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(13), ty.clone()),
            OperationKind::Select {
                condition: ValueId(10),
                true_value: ValueId(11),
                false_value: ValueId(12),
            },
        ));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let function = Function::definition(
            "issued_pointer_root_identity",
            fe2o3_kernel_ir::Signature::new(vec![Type::BOOL, ty.clone(), ty.clone()], vec![]),
            vec![ValueId(10), ValueId(11), ValueId(12)],
            vec![block],
        );
        let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 100_000);
        let actual = SourceIssuedActualV29::from_function(&function, &mut budget).unwrap();
        // Ordinal 1 is a physical ordinal, not the source argument number. The
        // original shape walk supplies it after flattened/erased arguments.
        check_source_issued_root_input_v29(&actual, 1, ValueId(11), &archived, &mut budget)
            .unwrap();
        drop(actual);
        let mut changed = function.clone();
        let mut expected = archived.clone();
        let mut slice = ValueId(11);
        let mut ordinal = 1;
        match fault {
            0 => changed.body.as_mut().unwrap().parameters.swap(1, 2),
            1 => {
                expected = SemanticValueBindingV1::Value {
                    id: ValueId(12),
                    ty: ty.clone(),
                }
            }
            2 => {
                slice = ValueId(12);
            }
            3 => {
                ordinal = 2;
            }
            4 => {
                slice = ValueId(13);
                expected = SemanticValueBindingV1::Value {
                    id: slice,
                    ty: ty.clone(),
                };
            }
            5 => {
                changed.signature.parameters[1] = Type::slice(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadOnly,
                )
            }
            _ => unreachable!(),
        }
        let actual = SourceIssuedActualV29::from_function(&changed, &mut budget).unwrap();
        let error =
            check_source_issued_root_input_v29(&actual, ordinal, slice, &expected, &mut budget)
                .unwrap_err();
        assert!(
            matches!(error, ProductionSemanticKirErrorV1::Unsupported { detail, .. }
            if detail == "source issued pointer differs from its original issuer or actual guard"),
            "fault {fault}: {error:?}"
        );
    }
}

fn transported_root_graph(ty: &Type) -> Function {
    let mut entry = BasicBlock::new(BlockId(90));
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(10),
        then_target: BlockId(20),
        then_arguments: vec![ValueId(11)],
        else_target: BlockId(20),
        else_arguments: vec![ValueId(11)],
    });
    let mut loop_block = BasicBlock::new(BlockId(20));
    loop_block.parameters = vec![ValueDef::new(ValueId(21), ty.clone())];
    loop_block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(22), ty.clone()),
        OperationKind::Select {
            condition: ValueId(10),
            true_value: ValueId(11),
            false_value: ValueId(11),
        },
    ));
    loop_block.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(10),
        then_target: BlockId(20),
        then_arguments: vec![ValueId(21)],
        else_target: BlockId(30),
        else_arguments: vec![ValueId(21)],
    });
    let mut exit = BasicBlock::new(BlockId(30));
    exit.parameters = vec![ValueDef::new(ValueId(31), ty.clone())];
    exit.terminator = Some(Terminator::Return { values: vec![] });
    Function::definition(
        "issued_pointer_transported_root",
        fe2o3_kernel_ir::Signature::new(vec![Type::BOOL, ty.clone(), ty.clone()], vec![]),
        vec![ValueId(10), ValueId(11), ValueId(12)],
        vec![entry, loop_block, exit],
    )
}

fn check_transported_root(
    function: &Function,
    archived: &SemanticValueBindingV1,
    receiver: ValueId,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
    let actual = SourceIssuedActualV29::from_function(function, &mut budget)?;
    let mut transport =
        source_issued_root_transport_v29(&actual, 1, receiver, archived, &mut budget)?;
    assert!(!transport.checked);
    let floor = budget.storage();
    fe2o3_kernel_ir::with_function_control_flow_v1(
        function,
        Default::default(),
        &mut budget,
        |view| {
            transport.checked =
                view.unique_value_origin(transport.receiver)? == Some(transport.input);
            Ok(())
        },
    )
    .map_err(|error| match error {
        fe2o3_kernel_ir::FunctionControlFlowScopeErrorV1::Resource(error) => error.into(),
        _ => source_issued_error_v29(),
    })?;
    assert_eq!(budget.storage(), floor);
    if !transport.checked {
        return Err(source_issued_error_v29());
    }
    Ok(())
}

#[test]
fn issued_pointer_transport_checks_original_input_before_actual_all_edge_identity() {
    let ty = Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let archived = SemanticValueBindingV1::Value {
        id: ValueId(11),
        ty: ty.clone(),
    };
    for fault in 0..7 {
        let mut function = transported_root_graph(&ty);
        check_transported_root(&function, &archived, ValueId(31)).unwrap();
        let mut expected = archived.clone();
        let mut receiver = ValueId(31);
        let body = function.body.as_mut().unwrap();
        match fault {
            0 => {
                let Some(Terminator::ConditionalBranch { else_arguments, .. }) =
                    &mut body.blocks[0].terminator
                else {
                    unreachable!()
                };
                else_arguments[0] = ValueId(12);
            }
            1 => body.parameters.swap(1, 2),
            2 => {
                expected = SemanticValueBindingV1::Value {
                    id: ValueId(12),
                    ty: ty.clone(),
                }
            }
            3 => {
                expected = SemanticValueBindingV1::Value {
                    id: ValueId(31),
                    ty: ty.clone(),
                }
            }
            4 => receiver = ValueId(22), // An operation is not an admitted transport edge.
            5 => {
                body.blocks[2].parameters[0].ty = Type::slice(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadOnly,
                )
            }
            6 => {
                let Some(Terminator::ConditionalBranch { then_arguments, .. }) =
                    &mut body.blocks[1].terminator
                else {
                    unreachable!()
                };
                then_arguments[0] = ValueId(12);
            }
            _ => unreachable!(),
        }
        let error = check_transported_root(&function, &expected, receiver).unwrap_err();
        assert!(
            matches!(error, ProductionSemanticKirErrorV1::Unsupported { detail, .. }
            if detail == "source issued pointer differs from its original issuer or actual guard"),
            "fault {fault}: {error:?}"
        );
    }
}
