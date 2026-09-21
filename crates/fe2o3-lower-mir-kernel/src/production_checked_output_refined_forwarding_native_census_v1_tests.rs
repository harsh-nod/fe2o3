use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirCrossBlockForwardingLimitsV1 as ForwardingLimits,
    CanonicalKirLoopLimitsV1 as RefinementLimits,
};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    CheckedBinaryOperator, ComparePredicate, Constant, Function, Kernel, LaunchDomain,
    LaunchExtent, MemoryAccess, Module, Operation, ScalarType, Signature, ValueDef, ValueId,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use fe2o3_mir_model::semantic_mir_v1::{SemanticLayoutIdentityV1, SemanticTargetDataLayoutV1};
const WORK: usize = 200_000_000;
const STORAGE: usize = 128 << 20;
fn block(
    id: u32,
    parameters: Vec<ValueDef>,
    operations: Vec<Operation>,
    terminator: Terminator,
) -> BasicBlock {
    let mut value = BasicBlock::new(BlockId(id));
    value.parameters = parameters;
    value.operations = operations;
    value.terminator = Some(terminator);
    value
}
fn graph(extra: Option<(ScalarType, BinaryOp)>) -> Module {
    let u = Type::Scalar(ScalarType::U64);
    let mut value = Module::new("composed-checked-add-private-load");
    value.functions.push(Function::kernel_entry(
        "root",
        Signature::new(vec![u.clone(), u.clone()], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![
            block(
                10,
                vec![],
                vec![
                    Operation::effect_free(
                        ValueDef::new(ValueId(2), u.clone()),
                        OperationKind::Constant(Constant::U64(1)),
                    ),
                    Operation::effect_free(
                        ValueDef::new(
                            ValueId(100),
                            Type::pointer(u.clone(), AddressSpace::Private, AccessMode::ReadWrite),
                        ),
                        OperationKind::Alloca {
                            element: u.clone(),
                            count: None,
                            address_space: AddressSpace::Private,
                            alignment: 8,
                        },
                    ),
                ],
                Terminator::Branch {
                    target: BlockId(20),
                    arguments: vec![ValueId(0)],
                },
            ),
            block(
                20,
                vec![ValueDef::new(ValueId(3), u.clone())],
                vec![Operation::effect_free(
                    ValueDef::new(ValueId(4), Type::BOOL),
                    OperationKind::Compare {
                        predicate: ComparePredicate::LessThan,
                        lhs: ValueId(3),
                        rhs: ValueId(1),
                    },
                )],
                Terminator::ConditionalBranch {
                    condition: ValueId(4),
                    then_target: BlockId(30),
                    then_arguments: vec![],
                    else_target: BlockId(50),
                    else_arguments: vec![],
                },
            ),
            block(
                30,
                vec![],
                vec![
                    Operation::checked_binary(
                        ValueDef::new(ValueId(5), u.clone()),
                        ValueDef::new(ValueId(6), Type::BOOL),
                        CheckedBinaryOperator::Add,
                        ValueId(3),
                        ValueId(2),
                    ),
                    Operation::new(
                        vec![],
                        OperationKind::Store {
                            pointer: ValueId(100),
                            value: ValueId(3),
                            access: MemoryAccess::new(AddressSpace::Private, 8),
                        },
                    ),
                ],
                Terminator::Branch {
                    target: BlockId(40),
                    arguments: vec![],
                },
            ),
            block(
                40,
                vec![],
                vec![Operation::effect_free(
                    ValueDef::new(ValueId(101), u),
                    OperationKind::Load {
                        pointer: ValueId(100),
                        access: MemoryAccess::new(AddressSpace::Private, 8),
                    },
                )],
                Terminator::Branch {
                    target: BlockId(20),
                    arguments: vec![ValueId(5)],
                },
            ),
            block(50, vec![], vec![], Terminator::Return { values: vec![] }),
        ],
    ));
    value.kernels.push(Kernel::new(
        "root",
        "root",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        },
    ));
    if let Some((scalar, op)) = extra {
        let ty = Type::Scalar(scalar);
        value.functions.push(Function::kernel_entry(
            "unrelated",
            Signature::new(vec![ty.clone(), ty.clone()], vec![]),
            vec![ValueId(0), ValueId(1)],
            vec![block(
                91,
                vec![],
                vec![Operation::effect_free(
                    ValueDef::new(ValueId(2), ty),
                    OperationKind::Binary {
                        op,
                        lhs: ValueId(0),
                        rhs: ValueId(1),
                    },
                )],
                Terminator::Return { values: vec![] },
            )],
        ));
        value.kernels.push(Kernel::new(
            "unrelated",
            "unrelated",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(1),
            },
        ));
    }
    value
}
fn with_chain(
    extra: Option<(ScalarType, BinaryOp)>,
    run: impl FnOnce(
        &RefinementPair<'_>,
        &ForwardingPair<'_>,
        &CanonicalKirInventoryV1<'_>,
        &CanonicalKirInventoryV1<'_>,
        &mut AssertOriginBudgetV1<'_>,
    ),
) {
    with_module(graph(extra), 1, 1, run)
}
fn with_module(
    module: Module,
    refinements: usize,
    selected: usize,
    run: impl FnOnce(
        &RefinementPair<'_>,
        &ForwardingPair<'_>,
        &CanonicalKirInventoryV1<'_>,
        &CanonicalKirInventoryV1<'_>,
        &mut AssertOriginBudgetV1<'_>,
    ),
) {
    let mut work = Work::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let sibling = vec![0x59u8; 43];
    let floor = std::mem::size_of_val(&sibling) + sibling.capacity();
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    {
        let (input, receipt) =
            Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let refined = fe2o3_kernel_opt::prepare_owned_induction_refinement_v1(
            &input,
            RefinementLimits::default(),
            &mut budget,
        )
        .unwrap();
        budget.reserve_storage(refined.retained_storage()).unwrap();
        assert_eq!(
            refined
                .origins()
                .iter()
                .filter(|row| matches!(row, Origin::CheckedAddSplit { .. }))
                .count(),
            refinements
        );
        let forwarded = fe2o3_kernel_opt::prepare_owned_cross_block_forwarding_v1(
            refined.output(),
            ForwardingLimits::default(),
            &mut budget,
        )
        .unwrap();
        budget
            .reserve_storage(forwarded.retained_storage())
            .unwrap();
        assert_eq!(
            forwarded
                .origins()
                .iter()
                .filter(|row| row.store.is_some())
                .count(),
            selected
        );
        let (refinement, receipt) = refined
            .replay_against(&input, RefinementLimits::default(), &mut budget)
            .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let (forwarding, receipt) = forwarded
            .replay_against(refined.output(), &mut budget)
            .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let (intermediate, receipt) =
            CanonicalKirInventoryV1::derive(refined.output(), &mut budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let (output, receipt) =
            CanonicalKirInventoryV1::derive(forwarded.output(), &mut budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        run(
            &refinement,
            &forwarding,
            &intermediate,
            &output,
            &mut budget,
        );
    }
    budget.release_storage(budget.storage() - floor).unwrap();
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(sibling, [0x59; 43]);
}
fn native_result(
    refinement: &RefinementPair<'_>,
    forwarding: &ForwardingPair<'_>,
    intermediate: &CanonicalKirInventoryV1<'_>,
    output: &CanonicalKirInventoryV1<'_>,
    composed: bool,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<()> {
    let private = private_memory::check(output, 1024, budget)?;
    let division = unsigned_division::check(
        output,
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([245; 32])),
        budget,
    )?;
    let helpers = scalar_helpers::check(output, budget)?;
    if composed {
        native_with_refinement_forwarding(
            output,
            &private,
            &division,
            &helpers,
            "composed Add test",
            |_, _| Ok(false),
            refinement,
            forwarding,
            intermediate,
            budget,
        )
    } else {
        native(
            output,
            &private,
            &division,
            &helpers,
            "composed Add test",
            |_, _| Ok(false),
            budget,
        )
    }
}
#[test]
fn refined_forwarding_allowance_requires_both_actual_pairs_and_keeps_shifted_store() {
    with_chain(
        None,
        |refinement, forwarding, intermediate, output, budget| {
            assert!(std::ptr::eq(refinement.output(), forwarding.input()));
            assert!(std::ptr::eq(forwarding.output(), output.owner()));
            assert!(matches!(
                super::super::induction_refinement::CheckedAdds::new(refinement, output, budget),
                Err(E::Unsupported {
                    phase: "checked induction Add",
                    detail: "actual pair/output owner"
                })
            ));
            assert!(matches!(
                native_result(refinement, forwarding, intermediate, output, false, budget),
                Err(E::Unsupported {
                    phase: "composed Add test",
                    detail: "closed opcode census"
                })
            ));
            native_result(refinement, forwarding, intermediate, output, true, budget).unwrap();
            let allowed =
                CheckedForwardedAdds::new(refinement, forwarding, intermediate, output, budget)
                    .unwrap();
            for (ordinal, row) in output.operations().iter().enumerate() {
                assert_eq!(
                    allowed.operation(output, ordinal, budget).unwrap(),
                    matches!(
                        row.operation.kind,
                        OperationKind::Binary {
                            op: BinaryOp::Add,
                            ..
                        }
                    )
                );
            }
            let selected = forwarding
                .origins()
                .iter()
                .find(|row| row.store.is_some())
                .unwrap();
            assert_eq!(
                selected.store.unwrap().operation,
                2,
                "old Store ordinal1 became synthetic false"
            );
            assert_eq!(selected.store.unwrap().block.block, 2);
            assert_eq!(
                (selected.input.block.block, selected.input.operation),
                (3, 0)
            );
            let old_store = refinement
                .origins()
                .iter()
                .find(|row| row.input().block.block == 2 && row.input().operation == 1)
                .unwrap();
            assert!(
                matches!(old_store, Origin::Unchanged { output, .. } if *output == selected.store.unwrap())
            );
        },
    );
}
#[test]
fn refined_forwarding_allowance_keeps_all_twenty_seven_ordinary_arithmetic_negatives() {
    let mut count = 0;
    for scalar in [
        ScalarType::I8,
        ScalarType::U8,
        ScalarType::I16,
        ScalarType::U16,
        ScalarType::I32,
        ScalarType::U32,
        ScalarType::I64,
        ScalarType::U64,
        ScalarType::Index,
    ] {
        for op in [BinaryOp::Add, BinaryOp::Subtract, BinaryOp::Multiply] {
            with_chain(
                Some((scalar, op)),
                |refinement, forwarding, intermediate, output, budget| {
                    assert!(matches!(
                        native_result(refinement, forwarding, intermediate, output, true, budget),
                        Err(E::Unsupported {
                            phase: "composed Add test",
                            detail: "closed opcode census"
                        })
                    ));
                },
            );
            count += 1;
        }
    }
    assert_eq!(count, 27);
}
#[test]
fn refined_forwarding_allowance_rejects_genuine_disconnected_equal_byte_middle_owners() {
    with_chain(None, |first, _, _, _, _| {
        with_chain(None, |_, second, intermediate, output, budget| {
            assert_eq!(
                first.output().canonical().canonical_bytes(),
                second.input().canonical().canonical_bytes()
            );
            assert!(!std::ptr::eq(first.output(), second.input()));
            let before = budget.work();
            assert!(matches!(
                CheckedForwardedAdds::new(first, second, intermediate, output, budget),
                Err(E::Unsupported {
                    phase: "refined forwarding Add",
                    detail: "actual L-to-R-to-F endpoints"
                })
            ));
            assert_eq!(budget.work(), before + 7);
        });
    });
}
#[test]
fn refined_forwarding_allowance_rejects_foreign_final_inventory_and_out_of_range_ordinal() {
    with_chain(
        None,
        |refinement, forwarding, intermediate, output, budget| {
            let (other_owner, receipt) = Owner::from_module_ref_with_verification_budget_v12(
                output.owner().module(),
                budget,
            )
            .unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let (other, receipt) = CanonicalKirInventoryV1::derive(&other_owner, budget).unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            assert!(matches!(
                CheckedForwardedAdds::new(refinement, forwarding, intermediate, &other, budget),
                Err(E::Unsupported {
                    phase: "refined forwarding Add",
                    detail: "actual L-to-R-to-F endpoints"
                })
            ));
            let (same, receipt) = CanonicalKirInventoryV1::derive(output.owner(), budget).unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let allowed =
                CheckedForwardedAdds::new(refinement, forwarding, intermediate, output, budget)
                    .unwrap();
            assert!(matches!(
                allowed.operation(&same, 0, budget),
                Err(E::Unsupported {
                    phase: "refined forwarding Add",
                    detail: "retained two-pair final inventory"
                })
            ));
            assert!(matches!(
                allowed.operation(output, output.operations().len(), budget),
                Err(E::Unsupported {
                    phase: "refined forwarding Add",
                    detail: "actual final ordinal"
                })
            ));
        },
    );
}
#[test]
fn refined_forwarding_allowance_exact_header_denial_precedes_backing_and_keeps_history() {
    with_chain(
        None,
        |refinement, forwarding, intermediate, output, outer| {
            let floor = outer.storage();
            let header = std::mem::size_of::<CheckedForwardedAdds<'_, '_, '_>>()
                - std::mem::size_of::<Vec<bool>>();
            let attempted = floor + header;
            let mut work = Work::new(WORK);
            work.charge_work(17).unwrap();
            {
                let mut budget = AssertOriginBudgetV1::new(&mut work, attempted - 1);
                budget.reserve_storage(floor).unwrap();
                match CheckedForwardedAdds::new(
                    refinement,
                    forwarding,
                    intermediate,
                    output,
                    &mut budget,
                ) {
                    Err(E::Resource(AssertOriginResourceV1::Storage(error))) => {
                        assert_eq!((error.actual(), error.limit()), (attempted, attempted - 1))
                    }
                    _ => panic!("exact final allowance header refusal"),
                }
                assert_eq!(
                    (
                        budget.work(),
                        budget.storage(),
                        budget.peak_storage(),
                        budget.failed_storage()
                    ),
                    (24, floor, floor, Some(attempted))
                );
            }
            assert_eq!(work.failed_work(), None);
        },
    );
}
#[test]
fn refined_forwarding_allowance_exact_work_short_preserves_both_pairs_and_first_denial() {
    with_chain(
        None,
        |refinement, forwarding, intermediate, output, outer| {
            let floor = outer.storage();
            let measure = |limit| {
                let mut work = Work::new(limit);
                work.charge_work(17).unwrap();
                let (result, used, peak, failed_storage) = {
                    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
                    budget.reserve_storage(floor).unwrap();
                    let result = CheckedForwardedAdds::new(
                        refinement,
                        forwarding,
                        intermediate,
                        output,
                        &mut budget,
                    )
                    .map(drop);
                    let used = budget.work();
                    let peak = budget.peak_storage();
                    let failed_storage = budget.failed_storage();
                    budget.release_storage(budget.storage() - floor).unwrap();
                    assert_eq!(budget.storage(), floor);
                    (result, used, peak, failed_storage)
                };
                (result, used, peak, work.failed_work(), failed_storage)
            };
            let full = measure(WORK);
            full.0.unwrap();
            let exact = measure(full.1);
            exact.0.unwrap();
            assert_eq!(
                (exact.1, exact.2, exact.3, exact.4),
                (full.1, full.2, None, None)
            );
            let short = measure(full.1 - 1);
            match short.0 {
                Err(E::Resource(AssertOriginResourceV1::Work(error))) => {
                    assert_eq!((error.actual(), error.limit()), (full.1, full.1 - 1))
                }
                _ => panic!("last complete input-row charge"),
            }
            assert_eq!(
                (short.1, short.2, short.3, short.4),
                (full.1 - 28, full.2, Some(full.1), None)
            );
        },
    );
}

#[test]
fn refined_forwarding_allowance_requires_retained_sum_operands_and_synthetic_false_relation() {
    with_chain(None, |refinement, forwarding, _, output, budget| {
        let Origin::CheckedAddSplit {
            sum_output,
            false_output,
            ..
        } = *refinement
            .origins()
            .iter()
            .find(|row| matches!(row, Origin::CheckedAddSplit { .. }))
            .unwrap()
        else {
            panic!("actual checked split");
        };
        for changed in 0..3 {
            let mut module = output.owner().module().clone();
            let coordinate = if changed == 2 {
                false_output
            } else {
                sum_output
            };
            let operation = &mut module.functions[coordinate.block.function.0 as usize]
                .body
                .as_mut()
                .unwrap()
                .blocks[coordinate.block.block as usize]
                .operations[coordinate.operation as usize];
            if changed == 2 {
                operation.kind = OperationKind::Constant(Constant::Bool(true));
            } else {
                let OperationKind::Binary { lhs, rhs, .. } = operation.kind else {
                    panic!("actual sum");
                };
                operation.kind = OperationKind::Binary {
                    op: if changed == 0 {
                        BinaryOp::Add
                    } else {
                        BinaryOp::Subtract
                    },
                    lhs,
                    rhs: if changed == 0 { ValueId(1) } else { rhs },
                };
            }
            let (changed, receipt) =
                Owner::from_module_ref_with_verification_budget_v12(&module, budget).unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            assert!(matches!(
                fe2o3_kernel_analysis::check_canonical_kir_cross_block_forwarding_v1(
                    refinement.output(),
                    &changed,
                    forwarding.origins(),
                    ForwardingLimits::default(),
                    budget
                ),
                Err(
                    fe2o3_kernel_analysis::CanonicalKirCrossBlockForwardingErrorV1::Mismatch(
                        "retained operation"
                    )
                )
            ));
            drop(changed);
            budget.release_storage(receipt.retained_storage()).unwrap();
        }
    });
}

#[test]
fn refined_forwarding_allowance_nested_loops_keep_both_shifted_store_coordinates() {
    let u = Type::Scalar(ScalarType::U64);
    let mut module = graph(None);
    let body = module.functions[0].body.as_mut().unwrap();
    let branch = |target, arguments| Terminator::Branch {
        target: BlockId(target),
        arguments,
    };
    let condition = |value, yes, no, arguments| Terminator::ConditionalBranch {
        condition: ValueId(value),
        then_target: BlockId(yes),
        then_arguments: arguments,
        else_target: BlockId(no),
        else_arguments: vec![],
    };
    let compare = |result, value| {
        Operation::effect_free(
            ValueDef::new(ValueId(result), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(value),
                rhs: ValueId(1),
            },
        )
    };
    let store = |value| {
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(100),
                value: ValueId(value),
                access: MemoryAccess::new(AddressSpace::Private, 8),
            },
        )
    };
    let load = |result| {
        Operation::effect_free(
            ValueDef::new(ValueId(result), u.clone()),
            OperationKind::Load {
                pointer: ValueId(100),
                access: MemoryAccess::new(AddressSpace::Private, 8),
            },
        )
    };
    let entry = body.blocks[0].clone();
    body.blocks = vec![
        entry,
        block(
            20,
            vec![ValueDef::new(ValueId(3), u.clone())],
            vec![compare(4, 3)],
            condition(4, 30, 80, vec![ValueId(0)]),
        ),
        block(
            30,
            vec![ValueDef::new(ValueId(7), u.clone())],
            vec![compare(10, 7)],
            condition(10, 40, 60, vec![]),
        ),
        block(
            40,
            vec![],
            vec![
                Operation::checked_binary(
                    ValueDef::new(ValueId(8), u.clone()),
                    ValueDef::new(ValueId(9), Type::BOOL),
                    CheckedBinaryOperator::Add,
                    ValueId(7),
                    ValueId(2),
                ),
                store(7),
            ],
            branch(50, vec![]),
        ),
        block(50, vec![], vec![load(101)], branch(30, vec![ValueId(8)])),
        block(
            60,
            vec![],
            vec![
                Operation::checked_binary(
                    ValueDef::new(ValueId(5), u.clone()),
                    ValueDef::new(ValueId(6), Type::BOOL),
                    CheckedBinaryOperator::Add,
                    ValueId(3),
                    ValueId(2),
                ),
                store(3),
            ],
            branch(70, vec![]),
        ),
        block(70, vec![], vec![load(102)], branch(20, vec![ValueId(5)])),
        block(80, vec![], vec![], Terminator::Return { values: vec![] }),
    ];
    for canonical in [false, true] {
        let mut candidate = module.clone();
        if canonical {
            let body = candidate.functions[0].body.as_mut().unwrap();
            body.blocks[1].terminator = Some(condition(4, 25, 80, vec![]));
            body.blocks
                .insert(2, block(25, vec![], vec![], branch(30, vec![ValueId(0)])));
        }
        // A conditional incoming edge is not a dedicated inner preheader.
        // Keep that original shape as a control, then require both real splits.
        with_module(
            candidate,
            if canonical { 2 } else { 1 },
            2,
            |refinement, forwarding, intermediate, output, budget| {
                native_result(refinement, forwarding, intermediate, output, true, budget).unwrap();
                let selected: Vec<_> = forwarding
                    .origins()
                    .iter()
                    .filter_map(|row| row.store)
                    .collect();
                assert_eq!(selected.len(), 2);
                assert_eq!(
                    selected
                        .iter()
                        .map(|site| (site.block.block, site.operation))
                        .collect::<Vec<_>>(),
                    if canonical {
                        [(4, 2), (6, 2)]
                    } else {
                        [(3, 1), (5, 2)]
                    }
                );
                for store in selected {
                    assert!(refinement.origins().iter().any(|row| matches!(row,
                Origin::Unchanged { input, output } if input.block == store.block && input.operation == 1 && *output == store)));
                }
            },
        );
    }
}
