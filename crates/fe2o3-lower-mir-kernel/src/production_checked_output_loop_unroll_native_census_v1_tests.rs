use super::*;
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
    let mut block = BasicBlock::new(BlockId(id));
    block.parameters = parameters;
    block.operations = operations;
    block.terminator = Some(terminator);
    block
}
fn constant(id: u32, n: u64) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U64)),
        OperationKind::Constant(Constant::U64(n)),
    )
}
fn graph(extra: Option<(ScalarType, BinaryOp)>) -> Module {
    let u = Type::Scalar(ScalarType::U64);
    let mut module = Module::new("unroll-checked-add-census");
    module.functions.push(Function::kernel_entry(
        "root",
        Signature::new(vec![], vec![]),
        vec![],
        vec![
            block(
                10,
                vec![],
                vec![
                    constant(0, 0),
                    constant(1, 3),
                    constant(2, 1),
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
    module.kernels.push(Kernel::new(
        "root",
        "root",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        },
    ));
    if let Some((scalar, op)) = extra {
        let ty = Type::Scalar(scalar);
        module.functions.push(Function::kernel_entry(
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
        module.kernels.push(Kernel::new(
            "unrelated",
            "unrelated",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(1),
            },
        ));
    }
    module
}
fn with_chain(
    extra: Option<(ScalarType, BinaryOp)>,
    run: impl FnOnce(
        &RefinementPair<'_>,
        &ForwardingPair<'_>,
        &CanonicalKirInventoryV1<'_>,
        &CanonicalKirInventoryV1<'_>,
        &UnrollPair<'_, '_, '_>,
        &CanonicalKirInventoryV1<'_>,
        &mut AssertOriginBudgetV1<'_>,
    ),
) {
    let mut work = Work::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let sibling = vec![0x63u8; 31];
    let floor = std::mem::size_of_val(&sibling) + sibling.capacity();
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    {
        let (l, receipt) =
            Owner::from_module_ref_with_verification_budget_v12(&graph(extra), &mut budget)
                .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let r = fe2o3_kernel_opt::prepare_owned_induction_refinement_v1(
            &l,
            Default::default(),
            &mut budget,
        )
        .unwrap();
        budget.reserve_storage(r.retained_storage()).unwrap();
        assert_eq!(r.origins().iter().filter(|r| matches!(r, fe2o3_kernel_analysis::CanonicalKirInductionRefinementOriginV1::CheckedAddSplit { .. })).count(), 1);
        let f = fe2o3_kernel_opt::prepare_owned_cross_block_forwarding_v1(
            r.output(),
            Default::default(),
            &mut budget,
        )
        .unwrap();
        budget.reserve_storage(f.retained_storage()).unwrap();
        assert_eq!(f.origins().iter().filter(|r| r.store.is_some()).count(), 1);
        let (u, receipt) = fe2o3_kernel_opt::unroll_canonical_kir_loops_v1(
            f.output(),
            Default::default(),
            &mut budget,
        )
        .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert_eq!(u.origins().selection.unwrap().iterations, 3);
        let (rp, receipt) = r
            .replay_against(&l, Default::default(), &mut budget)
            .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let (fp, receipt) = f.replay_against(r.output(), &mut budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let (up, receipt) = u.replay(f.output(), u.limits(), &mut budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let (ri, receipt) = CanonicalKirInventoryV1::derive(r.output(), &mut budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let (fi, receipt) = CanonicalKirInventoryV1::derive(f.output(), &mut budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let (ui, receipt) = CanonicalKirInventoryV1::derive(u.output(), &mut budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        run(&rp, &fp, &ri, &fi, &up, &ui, &mut budget);
    }
    budget.release_storage(budget.storage() - floor).unwrap();
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(sibling, [0x63; 31]);
}
#[test]
fn unrolled_add_permission_requires_three_actual_pairs_and_exact_copy_sites() {
    with_chain(None, |r, f, ri, fi, u, ui, budget| {
        let allowed = CheckedUnrolledAdds::new(r, f, ri, fi, u, ui, budget).unwrap();
        let mut sums = 0;
        for (ordinal, operation) in ui.operations().iter().enumerate() {
            let actual = allowed.operation(ui, ordinal, budget).unwrap();
            assert_eq!(
                actual,
                matches!(
                    operation.operation.kind,
                    OperationKind::Binary {
                        op: BinaryOp::Add,
                        ..
                    }
                )
            );
            sums += usize::from(actual);
        }
        assert_eq!(sums, 3);
        assert!(matches!(
            allowed.operation(ui, ui.operations().len(), budget),
            Err(E::Unsupported {
                phase: "unrolled refined Add",
                detail: "actual final ordinal"
            })
        ));
        let (other, receipt) = CanonicalKirInventoryV1::derive(ui.owner(), budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert!(matches!(
            allowed.operation(&other, 0, budget),
            Err(E::Unsupported {
                phase: "unrolled refined Add",
                detail: "retained three-pair final inventory"
            })
        ));
    });
}
#[test]
fn unrolled_add_permission_keeps_all_twenty_seven_ordinary_arithmetic_negatives() {
    let mut cases = 0;
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
            with_chain(Some((scalar, op)), |r, f, ri, fi, u, ui, budget| {
                let private = private_memory::check(ui, 1024, budget).unwrap();
                let division = unsigned_division::check(
                    ui,
                    SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(
                        [91; 32],
                    )),
                    budget,
                )
                .unwrap();
                let helpers = scalar_helpers::check(ui, budget).unwrap();
                assert!(matches!(
                    native_with_loop_unroll(
                        ui,
                        &private,
                        &division,
                        &helpers,
                        "unrolled ordinary negative",
                        |_, _| Ok(false),
                        r,
                        f,
                        ri,
                        fi,
                        u,
                        budget
                    ),
                    Err(E::Unsupported {
                        phase: "unrolled ordinary negative",
                        detail: "closed opcode census"
                    })
                ));
            });
            cases += 1;
        }
    }
    assert_eq!(cases, 27);
}
#[test]
fn unrolled_add_permission_rejects_equal_bytes_foreign_final_owner() {
    with_chain(None, |r, f, ri, fi, u, ui, budget| {
        let (other, receipt) =
            Owner::from_module_ref_with_verification_budget_v12(ui.owner().module(), budget)
                .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert_eq!(
            other.canonical().canonical_bytes(),
            ui.owner().canonical().canonical_bytes()
        );
        let (other, receipt) = CanonicalKirInventoryV1::derive(&other, budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert!(matches!(
            CheckedUnrolledAdds::new(r, f, ri, fi, u, &other, budget),
            Err(E::Unsupported {
                phase: "unrolled refined Add",
                detail: "actual L/R/F/U endpoints"
            })
        ));
    });
}
#[test]
fn unrolled_add_permission_header_short_is_exact_and_precedes_backing() {
    with_chain(None, |r, f, ri, fi, u, ui, budget| {
        let floor = budget.storage();
        let header = std::mem::size_of::<CheckedUnrolledAdds<'_, '_, '_, '_, '_, '_>>()
            - std::mem::size_of::<refined_forwarding::CheckedForwardedAdds<'_, '_, '_>>()
            - std::mem::size_of::<Vec<Option<usize>>>();
        assert!(header > 0);
        let mut work = Work::new(WORK);
        work.charge_work(17).unwrap();
        {
            let mut limited = AssertOriginBudgetV1::new(&mut work, floor + header - 1);
            limited.reserve_storage(floor).unwrap();
            match CheckedUnrolledAdds::new(r, f, ri, fi, u, ui, &mut limited) {
                Err(E::Resource(AssertOriginResourceV1::Storage(error))) => assert_eq!(
                    (error.actual(), error.limit()),
                    (floor + header, floor + header - 1)
                ),
                Err(error) => panic!("exact permission header: {error:?}"),
                Ok(_) => panic!("short private header admitted"),
            }
            assert_eq!(
                (
                    limited.work(),
                    limited.storage(),
                    limited.peak_storage(),
                    limited.failed_storage()
                ),
                (26, floor, floor, Some(floor + header))
            );
        }
        assert_eq!(work.failed_work(), None);
    });
}

#[test]
fn unrolled_add_permission_cannot_obtain_a_pair_for_wrong_clone_rows() {
    with_chain(None, |_, _, _, fi, u, ui, budget| {
        let mut operations = scratch::<
            fe2o3_kernel_analysis::CanonicalKirLoopUnrollOriginV1<
                CanonicalKirOperationCoordinateV1,
            >,
        >(u.origins().operations.len(), budget)
        .unwrap();
        operations.extend_from_slice(u.origins().operations);
        let index = operations
            .iter()
            .position(|r| matches!(r.copy, CopyRole::Body(1)))
            .unwrap();
        let old = operations[index];
        operations[index].copy = CopyRole::Body(0);
        let mut rows = u.origins();
        rows.operations = &operations;
        assert!(matches!(
            fe2o3_kernel_analysis::check_canonical_kir_loop_unroll_pair_v1(
                fi.owner(),
                ui.owner(),
                rows,
                u.limits(),
                budget
            ),
            Err(
                fe2o3_kernel_analysis::CanonicalKirLoopUnrollErrorV1::Mismatch(
                    "complete ordered origin"
                )
            )
        ));
        operations[index] = old;
        operations[index] = operations[0];
        let mut rows = u.origins();
        rows.operations = &operations;
        assert!(matches!(
            fe2o3_kernel_analysis::check_canonical_kir_loop_unroll_pair_v1(
                fi.owner(),
                ui.owner(),
                rows,
                u.limits(),
                budget
            ),
            Err(
                fe2o3_kernel_analysis::CanonicalKirLoopUnrollErrorV1::Mismatch(
                    "complete ordered origin"
                )
            )
        ));
    });
}

#[test]
fn unrolled_add_permission_cannot_obtain_a_pair_for_valid_ir_wrong_sum() {
    with_chain(None, |_, _, _, fi, u, ui, budget| {
        let (mut candidate, receipt) = ui
            .owner()
            .copy_module_for_transformation_v12(budget)
            .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let body = candidate.functions[0].body.as_mut().unwrap();
        let mut changed = false;
        for block in &mut body.blocks {
            for operation in &mut block.operations {
                if let OperationKind::Binary {
                    op: BinaryOp::Add,
                    lhs,
                    rhs,
                } = &mut operation.kind
                {
                    assert_ne!(lhs, rhs);
                    *lhs = *rhs;
                    changed = true;
                    break;
                }
            }
            if changed {
                break;
            }
        }
        assert!(changed);
        let (bad, receipt) =
            Owner::from_module_ref_with_verification_budget_v12(&candidate, budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert!(matches!(
            fe2o3_kernel_analysis::check_canonical_kir_loop_unroll_pair_v1(
                fi.owner(),
                &bad,
                u.origins(),
                u.limits(),
                budget
            ),
            Err(
                fe2o3_kernel_analysis::CanonicalKirLoopUnrollErrorV1::Mismatch(
                    "cloned operation payload/operand order"
                )
            )
        ));
    });
}
