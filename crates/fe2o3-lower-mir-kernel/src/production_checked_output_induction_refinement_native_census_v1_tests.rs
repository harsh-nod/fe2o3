use super::*;
use fe2o3_kernel_analysis::CanonicalKirLoopLimitsV1 as Limits;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, CheckedBinaryOperator,
    ComparePredicate, Constant, Function, Kernel, LaunchDomain, LaunchExtent, Module, Operation,
    ScalarType, Signature, ValueDef, ValueId, VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use fe2o3_mir_model::semantic_mir_v1::{SemanticLayoutIdentityV1, SemanticTargetDataLayoutV1};
const WORK: usize = 100_000_000;
const STORAGE: usize = 64 * 1024 * 1024;
fn block(id: u32, params: Vec<ValueDef>, ops: Vec<Operation>, term: Terminator) -> BasicBlock {
    let mut b = BasicBlock::new(BlockId(id));
    b.parameters = params;
    b.operations = ops;
    b.terminator = Some(term);
    b
}
fn graph(extra: Option<(ScalarType, BinaryOp)>) -> Module {
    let u = Type::Scalar(ScalarType::U64);
    let mut m = Module::new("typed-induction-census");
    m.functions.push(Function::kernel_entry(
        "root",
        Signature::new(vec![u.clone(), u.clone()], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![
            block(
                10,
                vec![],
                vec![Operation::effect_free(
                    ValueDef::new(ValueId(2), u.clone()),
                    OperationKind::Constant(Constant::U64(1)),
                )],
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
                    else_target: BlockId(40),
                    else_arguments: vec![],
                },
            ),
            block(
                30,
                vec![],
                vec![Operation::checked_binary(
                    ValueDef::new(ValueId(5), u),
                    ValueDef::new(ValueId(6), Type::BOOL),
                    CheckedBinaryOperator::Add,
                    ValueId(3),
                    ValueId(2),
                )],
                Terminator::Branch {
                    target: BlockId(20),
                    arguments: vec![ValueId(5)],
                },
            ),
            block(40, vec![], vec![], Terminator::Return { values: vec![] }),
        ],
    ));
    m.kernels.push(Kernel::new(
        "root",
        "root",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        },
    ));
    if let Some((scalar, op)) = extra {
        let ty = Type::Scalar(scalar);
        m.functions.push(Function::kernel_entry(
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
        m.kernels.push(Kernel::new(
            "unrelated",
            "unrelated",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(1),
            },
        ));
    }
    m
}
fn with_pair(
    extra: Option<(ScalarType, BinaryOp)>,
    run: impl FnOnce(&Pair<'_>, &CanonicalKirInventoryV1<'_>, &mut AssertOriginBudgetV1<'_>),
) {
    let mut work = Work::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let sibling = [0x65u8; 29];
    budget.reserve_storage(sibling.len()).unwrap();
    {
        let (input, storage) =
            Owner::from_module_ref_with_verification_budget_v12(&graph(extra), &mut budget)
                .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let tail = fe2o3_kernel_opt::prepare_owned_induction_refinement_v1(
            &input,
            Limits::default(),
            &mut budget,
        )
        .unwrap();
        budget.reserve_storage(tail.retained_storage()).unwrap();
        assert_eq!(
            tail.origins()
                .iter()
                .filter(|r| matches!(r, Origin::CheckedAddSplit { .. }))
                .count(),
            1
        );
        let (pair, storage) = tail
            .replay_against(&input, Limits::default(), &mut budget)
            .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let (inventory, storage) =
            CanonicalKirInventoryV1::derive(tail.output(), &mut budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        run(&pair, &inventory, &mut budget);
    }
    budget
        .release_storage(budget.storage() - sibling.len())
        .unwrap();
    assert_eq!(budget.storage(), sibling.len());
    assert_eq!(sibling, [0x65; 29]);
}
fn native_result(
    pair: &Pair<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    typed: bool,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<()> {
    let private = private_memory::check(inventory, 1024, budget)?;
    let division = unsigned_division::check(
        inventory,
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([249; 32])),
        budget,
    )?;
    let helpers = scalar_helpers::check(inventory, budget)?;
    if typed {
        native_with_induction_refinement(
            inventory,
            &private,
            &division,
            &helpers,
            "typed induction test",
            |_, _| Ok(false),
            pair,
            budget,
        )
    } else {
        native(
            inventory,
            &private,
            &division,
            &helpers,
            "typed induction test",
            |_, _| Ok(false),
            budget,
        )
    }
}
#[test]
fn source_induction_refinement_native_admits_only_the_actual_checked_sum() {
    with_pair(None, |pair, inventory, budget| {
        assert!(matches!(
            native_result(pair, inventory, false, budget),
            Err(E::Unsupported {
                phase: "typed induction test",
                detail: "closed opcode census"
            })
        ));
        native_result(pair, inventory, true, budget).unwrap();
        let allowed = CheckedAdds::new(pair, inventory, budget).unwrap();
        for (ordinal, row) in inventory.operations().iter().enumerate() {
            assert_eq!(
                allowed.operation(inventory, ordinal, budget).unwrap(),
                matches!(
                    row.operation.kind,
                    OperationKind::Binary {
                        op: BinaryOp::Add,
                        ..
                    }
                )
            );
        }
    });
}
#[test]
fn source_induction_refinement_native_keeps_all_unrelated_integer_arithmetic_closed() {
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
            with_pair(Some((scalar, op)), |pair, inventory, budget| {
                assert!(matches!(
                    native_result(pair, inventory, true, budget),
                    Err(E::Unsupported {
                        phase: "typed induction test",
                        detail: "closed opcode census"
                    })
                ));
            });
        }
    }
}
#[test]
fn source_induction_refinement_native_rejects_equal_graph_foreign_owner_and_inventory() {
    with_pair(None, |pair, inventory, budget| {
        let (other, storage) =
            Owner::from_module_ref_with_verification_budget_v12(inventory.owner().module(), budget)
                .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let (other, storage) = CanonicalKirInventoryV1::derive(&other, budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let before = budget.work();
        assert!(matches!(
            CheckedAdds::new(pair, &other, budget),
            Err(E::Unsupported {
                phase: "checked induction Add",
                detail: "actual pair/output owner"
            })
        ));
        assert_eq!(budget.work(), before + 3);
        let allowed = CheckedAdds::new(pair, inventory, budget).unwrap();
        let (same, storage) = CanonicalKirInventoryV1::derive(inventory.owner(), budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        assert!(matches!(
            allowed.operation(&same, 0, budget),
            Err(E::Unsupported {
                phase: "checked induction Add",
                detail: "retained pair/output inventory"
            })
        ));
    });
}
