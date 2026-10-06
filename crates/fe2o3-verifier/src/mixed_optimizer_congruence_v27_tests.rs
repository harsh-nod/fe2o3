fn with_pure_cse_v27<R>(
    module: Module,
    consume: impl FnOnce(&Inventory<'_>, &Inventory<'_>, Rows<'_>, &mut Budget<'_>) -> R,
) -> R {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(17).unwrap();
    let (owner, stored) =
        Owner::from_module_ref_with_verification_budget_v18(&module, LAYOUTS, &mut budget).unwrap();
    budget.reserve_storage(stored.retained_storage()).unwrap();
    let observed =
        fe2o3_pliron::optimize_neutral_kernel_ir_mixed_pure_cse_v18(&owner, LAYOUTS, &mut budget)
            .unwrap();
    budget
        .reserve_storage(observed.storage().retained_storage())
        .unwrap();
    assert_eq!(observed.execution().policy_version(), 10);
    let (input, ai) = Inventory::derive_v18(&owner, &mut budget).unwrap();
    budget.reserve_storage(ai.retained_storage()).unwrap();
    let (output, bi) = Inventory::derive_v18(observed.owner(), &mut budget).unwrap();
    budget.reserve_storage(bi.retained_storage()).unwrap();
    let (checked, ci) = check_canonical_kir_transition_v18(
        &input,
        &output,
        observed.occurrences().candidate(),
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(ci.retained_storage()).unwrap();
    let value = consume(&input, &output, checked.rows(), &mut budget);
    drop(checked);
    budget.release_storage(ci.retained_storage()).unwrap();
    value
}

fn cse_operators_v27(kind: usize, dominance: bool, different: bool) -> Module {
    use fe2o3_kernel_ir::{AccessMode, AddressSpace, CastKind, ComparePredicate, UnaryOp};
    let u32ty = Type::Scalar(ScalarType::U32);
    let slice = Type::slice(u32ty.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let (arguments, result) = match kind {
        0 => (vec![u32ty.clone(), u32ty.clone()], Type::BOOL),
        1 => (
            vec![Type::BOOL, u32ty.clone(), u32ty.clone()],
            u32ty.clone(),
        ),
        2 => (vec![u32ty.clone()], u32ty.clone()),
        3 => (vec![u32ty.clone()], Type::Scalar(ScalarType::U16)),
        4 => (vec![slice.clone()], Type::INDEX),
        5 => (
            vec![slice],
            Type::pointer(u32ty.clone(), AddressSpace::Global, AccessMode::ReadOnly),
        ),
        6 | 7 => (vec![u32ty.clone(), u32ty.clone()], u32ty),
        _ => unreachable!(),
    };
    let first = arguments.len() as u32;
    let arity = if kind == 7 { 2 } else { 1 };
    let operation = |result_id: u32, second: bool| {
        let kind = match kind {
            0 => OperationKind::Compare {
                predicate: if different && second {
                    ComparePredicate::NotEqual
                } else {
                    ComparePredicate::Equal
                },
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
            1 => OperationKind::Select {
                condition: ValueId(0),
                true_value: ValueId(if different && second { 2 } else { 1 }),
                false_value: ValueId(if different && second { 1 } else { 2 }),
            },
            2 => OperationKind::Unary {
                op: UnaryOp::Not,
                operand: ValueId(0),
            },
            3 => OperationKind::Cast {
                kind: CastKind::Truncate,
                value: ValueId(0),
                to: result.clone(),
            },
            4 => OperationKind::SliceLength { slice: ValueId(0) },
            5 => OperationKind::SliceData { slice: ValueId(0) },
            6 => OperationKind::Binary {
                op: if different && second {
                    BinaryOp::BitOr
                } else {
                    BinaryOp::BitXor
                },
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
            7 => OperationKind::Binary {
                op: BinaryOp::Checked(if different && second {
                    CheckedBinaryOperator::Subtract
                } else {
                    CheckedBinaryOperator::Add
                }),
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
            _ => unreachable!(),
        };
        let mut results = vec![ValueDef::new(ValueId(result_id), result.clone())];
        if arity == 2 {
            results.push(ValueDef::new(ValueId(result_id + 1), Type::BOOL));
        }
        Instruction::new(results, kind)
    };
    let mut entry = BasicBlock::new(BlockId(0));
    let mut second = BasicBlock::new(BlockId(1));
    entry.operations.push(operation(first, false));
    let target = if dominance { &mut second } else { &mut entry };
    target.operations.push(operation(first + arity, true));
    target.terminator = Some(Terminator::Return {
        values: (first..first + 2 * arity).map(ValueId).collect(),
    });
    if dominance {
        entry.terminator = Some(Terminator::Branch {
            target: BlockId(1),
            arguments: vec![],
        });
    }
    let mut returns = vec![result.clone()];
    if arity == 2 {
        returns.push(Type::BOOL);
    }
    returns.extend(returns.clone());
    let ids = (0..arguments.len() as u32).map(ValueId).collect();
    let mut module = Module::new("actual-policy10-shared-operators-v27");
    module.functions.push(Function::internal_helper(
        "entry",
        Signature::new(arguments, returns),
        ids,
        if dominance {
            vec![entry, second]
        } else {
            vec![entry]
        },
    ));
    module
}

#[test]
fn mixed_cfg_v27_uses_actual_local_and_dominance_cse_for_closed_total_grammar() {
    for kind in 0..8 {
        for dominance in [false, true] {
            with_pure_cse_v27(
                cse_operators_v27(kind, dominance, false),
                |input, output, rows, budget| {
                    assert_eq!(input.operations().len(), 2, "kind {kind}");
                    assert_eq!(output.operations().len(), 1, "kind {kind}");
                    assert_eq!(input.blocks().len(), output.blocks().len());
                    budget.reserve_storage(SOURCE_LIMIT).unwrap();
                    let floor = budget.storage();
                    let mut out = Writer::new(budget).unwrap();
                    let writer = out.budget.storage() - floor;
                    assert_eq!(
                        generate_cfg_v27(input, output, rows, &mut out).unwrap(),
                        output.blocks().len()
                    );
                    let text = out.finish().unwrap();
                    assert_eq!(budget.storage(), floor + writer);
                    assert!(text.contains("V27 exact total-operator congruence"));
                    assert!(text.contains("cfg_related_v26(n, o, op)"));
                    assert!(!text.contains("assume("));
                    assert!(!text.contains("external_body"));
                    if kind < 6 && kind != 1 {
                        assert!(text.contains("op(2, 0,"));
                        assert!(!text.contains("op(0, 0,"));
                        assert!(!text.contains("op(1, 0,"));
                    }
                    if kind == 1 {
                        assert!(text.contains("select_value_v28(base[0],base[1],base[2],)"));
                        assert!(!text.contains("op(2, 0,"));
                        assert!(!text.contains("op(0, 0,"));
                        if dominance {
                            assert!(
                                text.contains(
                                    "base[3] == select_value_v28(base[0],base[1],base[2])"
                                )
                            );
                        }
                    }
                    if dominance && kind < 6 && kind != 1 {
                        let block = text.split("proof fn block_simulation_1").nth(1).unwrap();
                        assert!(
                            block
                                .split(" ensures ")
                                .next()
                                .unwrap()
                                .contains("== op(2, 0,")
                        );
                        assert!(
                            text.contains("op: spec_fn(int, int, Seq<int>, int) -> int) -> bool")
                        );
                    }
                    crate::CanonicalGeneratedVerusProofInputV3::new(text.into_bytes()).unwrap();
                    budget.release_storage(writer).unwrap();
                },
            );
        }
    }
}

#[test]
fn mixed_cfg_v27_different_total_attributes_and_actual_arguments_remain_distinct() {
    for kind in [0, 1, 6, 7] {
        with_pure_cse_v27(
            cse_operators_v27(kind, true, true),
            |input, output, rows, budget| {
                assert_eq!(output.operations().len(), 2);
                budget.reserve_storage(SOURCE_LIMIT).unwrap();
                let mut out = Writer::new(budget).unwrap();
                generate_cfg_v27(input, output, rows, &mut out).unwrap();
                let text = out.finish().unwrap();
                if kind == 0 {
                    assert!(text.contains("op(2, 0,"));
                    assert!(text.contains("op(3, 0,"));
                }
                // Select is concrete; changing its branch order changes its equation.
                if kind == 1 {
                    assert!(text.contains("select_value_v28(base[0],base[1],base[2],)"));
                    assert!(text.contains("select_value_v28(base[0],base[2],base[1],)"));
                    assert!(!text.contains("op(2, 0,"));
                    assert!(!text.contains("op(3, 0,"));
                }
            },
        );
    }
}

#[test]
fn mixed_cfg_v27_preserves_old_profile_and_explicit_trap_occurrences() {
    for module in [
        swapping_loop(),
        reverse_blocks(),
        scalar_case(Constant::U32(1), BinaryOp::Add, false),
    ] {
        with_optimized(module, |input, output, rows, budget| {
            let render = |budget: &mut Budget<'_>, new| {
                budget.reserve_storage(SOURCE_LIMIT).unwrap();
                let mut out = Writer::new(budget).unwrap();
                if new {
                    generate_cfg_v27(input, output, rows, &mut out).unwrap();
                } else {
                    generate(input, output, rows, &mut out).unwrap();
                }
                let text = out.finish().unwrap();
                budget.release_storage(SOURCE_LIMIT).unwrap();
                text
            };
            let old = render(budget, false);
            let new = render(budget, true);
            assert!(!old.contains("V27 exact total-operator congruence"));
            assert!(!old.contains("cfg_related_v26(n, o, op)"));
            assert!(new.contains("cfg_related_v26(n, o, op)"));
            for (ordinal, row) in input.operations().iter().enumerate() {
                if matches!(
                    row.operation.kind,
                    OperationKind::Binary {
                        op: BinaryOp::Add,
                        ..
                    }
                ) {
                    assert!(new.contains(&format!("op({ordinal}, -1,")));
                }
            }
        });
    }
}

#[test]
fn mixed_cfg_v27_has_typed_exact_and_one_short_generation_limits() {
    with_pure_cse_v27(
        cse_operators_v27(0, true, false),
        |input, output, rows, _| {
            let run = |work_limit, storage_limit| {
                let mut work = Work::new(work_limit);
                let mut budget = Budget::new(&mut work, storage_limit);
                budget.reserve_storage(SOURCE_LIMIT + 17).unwrap();
                let floor = budget.storage();
                let mut out = Writer::new(&mut budget).unwrap();
                let writer = out.budget.storage() - floor;
                let result = generate_cfg_v27(input, output, rows, &mut out);
                drop(out);
                assert_eq!(budget.storage(), floor + writer);
                budget.release_storage(writer).unwrap();
                (result, budget.work(), budget.peak_storage())
            };
            let (result, work, storage) = run(LIMIT, LIMIT);
            result.unwrap();
            let (exact, actual_work, actual_storage) = run(work, storage);
            exact.unwrap();
            assert_eq!((actual_work, actual_storage), (work, storage));
            assert!(
                matches!(run(work-1,storage).0,Err(Error::Resource(Resource::Work(error))) if error.limit()==work-1&&error.actual()>work-1)
            );
            assert!(
                matches!(run(work,storage-1).0,Err(Error::Resource(Resource::Storage(error))) if error.limit()==storage-1&&error.actual()>storage-1)
            );
        },
    );
}

#[test]
fn mixed_cfg_v27_rejects_every_omitted_total_operator_live_dependency() {
    for kind in [0, 1, 3, 4, 5] {
        with_pure_cse_v27(
            cse_operators_v27(kind, true, false),
            |input, output, rows, budget| {
                budget.reserve_storage(SOURCE_LIMIT).unwrap();
                let floor = budget.storage();
                let mut out = Writer::new(budget).unwrap();
                let count =
                    cfg_relation::check_each_omission_cfg_v27(input, output, rows, &mut out)
                        .unwrap();
                assert!(count >= 3, "kind {kind}");
                drop(out);
                budget.release_storage(budget.storage() - floor).unwrap();
                assert_eq!(budget.storage(), floor);
            },
        );
    }
}
