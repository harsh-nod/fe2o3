use super::*;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirFunctionCoordinateV1 as Function,
    ComparePredicate, Function as KirFunction, Module, Operation, Signature, Terminator, ValueDef,
    ValueId,
};
const W: usize = 100_000_000;
const S: usize = 64 * 1024 * 1024;
const FLOOR: usize = 37;
fn ty(scalar: ScalarType) -> Type {
    Type::Scalar(scalar)
}
fn site(function: usize, block: usize, operation: usize) -> Site {
    Site {
        block: Block {
            function: Function(function.try_into().unwrap()),
            block: block.try_into().unwrap(),
        },
        operation: operation.try_into().unwrap(),
    }
}
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
fn branch(id: u32, arguments: Vec<ValueId>) -> Terminator {
    Terminator::Branch {
        target: BlockId(id),
        arguments,
    }
}
fn literal(scalar: ScalarType, value: u64) -> Constant {
    match scalar {
        ScalarType::U8 => Constant::U8(value.try_into().unwrap()),
        ScalarType::U16 => Constant::U16(value.try_into().unwrap()),
        ScalarType::U32 => Constant::U32(value.try_into().unwrap()),
        ScalarType::U64 => Constant::U64(value),
        ScalarType::I32 => Constant::I32(value.try_into().unwrap()),
        _ => panic!("fixture integer"),
    }
}
fn constant(id: u32, scalar: ScalarType, value: u64) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), ty(scalar)),
        OperationKind::Constant(literal(scalar, value)),
    )
}
fn fixture(scalar: ScalarType, step: u64, swapped: bool) -> Module {
    let mut module = Module::new("induction-refinement-pair");
    let (lhs, rhs) = if swapped { (2, 3) } else { (3, 2) };
    module.functions.push(KirFunction::internal_helper(
        "f",
        Signature::new(vec![ty(scalar), ty(scalar)], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![
            block(
                90,
                vec![],
                vec![constant(2, scalar, step)],
                branch(11, vec![ValueId(0)]),
            ),
            block(
                11,
                vec![ValueDef::new(ValueId(3), ty(scalar))],
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
                    then_target: BlockId(70),
                    then_arguments: vec![],
                    else_target: BlockId(100),
                    else_arguments: vec![],
                },
            ),
            block(
                70,
                vec![],
                vec![Operation::checked_binary(
                    ValueDef::new(ValueId(5), ty(scalar)),
                    ValueDef::new(ValueId(6), Type::BOOL),
                    CheckedBinaryOperator::Add,
                    ValueId(lhs),
                    ValueId(rhs),
                )],
                branch(11, vec![ValueId(5)]),
            ),
            block(100, vec![], vec![], Terminator::Return { values: vec![] }),
        ],
    ));
    module
}
fn admit(module: &Module) -> (Owner, usize) {
    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    let (owner, storage) =
        Owner::from_module_ref_with_verification_budget_v12(module, &mut budget).unwrap();
    (owner, storage.retained_storage())
}
// Declarative expected graph construction, never calls the optimization producer.
fn expected(module: &Module, selected: &[(usize, usize, usize, usize)]) -> (Module, Vec<Row>) {
    let mut output = module.clone();
    let mut rows = Vec::new();
    for (f, function) in output.functions.iter_mut().enumerate() {
        let Some(body) = function.body.as_mut() else {
            continue;
        };
        for (b, block) in body.blocks.iter_mut().enumerate() {
            let old = std::mem::take(&mut block.operations);
            for (o, mut operation) in old.into_iter().enumerate() {
                let input = site(f, b, o);
                let target = site(f, b, block.operations.len());
                if let Some((_, _, _, fact)) = selected
                    .iter()
                    .find(|(ff, bb, oo, _)| (*ff, *bb, *oo) == (f, b, o))
                {
                    let flag = operation.results.pop().unwrap();
                    let OperationKind::Binary { lhs, rhs, .. } = operation.kind else {
                        panic!("checked fixture")
                    };
                    operation.kind = OperationKind::Binary {
                        op: BinaryOp::Add,
                        lhs,
                        rhs,
                    };
                    block.operations.push(operation);
                    let false_output = site(f, b, block.operations.len());
                    block.operations.push(Operation::effect_free(
                        flag,
                        OperationKind::Constant(Constant::Bool(false)),
                    ));
                    rows.push(Row::CheckedAddSplit {
                        input,
                        sum_output: target,
                        false_output,
                        induction_row_ordinal: *fact,
                    });
                } else {
                    block.operations.push(operation);
                    rows.push(Row::Unchanged {
                        input,
                        output: target,
                    });
                }
            }
        }
    }
    (output, rows)
}
fn run_pair(input: &Module, output: &Module, rows: &[Row]) -> Result<()> {
    let rows = rows.to_vec();
    let (a, a_size) = admit(input);
    let (b, b_size) = admit(output);
    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    let sibling = [0x53u8; FLOOR];
    let floor = a_size + b_size + rows.capacity() * size_of::<Row>() + sibling.len();
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result =
        check_canonical_kir_induction_refinement_v1(&a, &b, &rows, Limits::default(), &mut budget)
            .map(|(checked_pair, receipt)| {
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                {
                    let pair = checked_pair;
                    assert!(std::ptr::eq(pair.input(), &a));
                    assert!(std::ptr::eq(pair.output(), &b));
                    assert_eq!(pair.origins(), rows.as_slice());
                    assert_eq!(pair.limits(), Limits::default());
                    assert!(!pair.grants_authority());
                    assert_eq!(
                        receipt.retained_storage(),
                        size_of::<CheckedCanonicalKirInductionRefinementV1<'_>>()
                    );
                }
                budget.release_storage(receipt.retained_storage()).unwrap();
            });
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(sibling, [0x53; FLOOR]);
    result
}
fn output_block(module: &mut Module) -> &mut BasicBlock {
    &mut module.functions[0].body.as_mut().unwrap().blocks[2]
}

#[test]
fn induction_refinement_pair_unsigned_widths_and_swapped_operand_exact_splits() {
    for scalar in [
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
    ] {
        for swapped in [false, true] {
            let input = fixture(scalar, 1, swapped);
            let (output, rows) = expected(&input, &[(0, 2, 0, 0)]);
            run_pair(&input, &output, &rows).unwrap();
            assert_eq!(rows.len(), 3);
            assert_eq!(
                output.functions[0].body.as_ref().unwrap().blocks[2]
                    .operations
                    .len(),
                2
            );
        }
    }
}
#[test]
fn induction_refinement_pair_requires_complete_ordered_one_to_many_origins() {
    let input = fixture(ScalarType::U32, 1, false);
    let (output, rows) = expected(&input, &[(0, 2, 0, 0)]);
    let mut mutations = vec![rows[..2].to_vec()];
    let mut duplicate = rows.clone();
    duplicate.push(rows[2]);
    mutations.push(duplicate);
    let mut reordered = rows.clone();
    reordered.swap(0, 1);
    mutations.push(reordered);
    for (input_site, sum, false_site, fact) in [
        (site(0, 1, 0), site(0, 2, 0), site(0, 2, 1), 0),
        (site(0, 2, 0), site(0, 2, 1), site(0, 2, 0), 0),
        (site(0, 2, 0), site(0, 2, 0), site(0, 2, 1), 1),
    ] {
        let mut bad = rows.clone();
        bad[2] = Row::CheckedAddSplit {
            input: input_site,
            sum_output: sum,
            false_output: false_site,
            induction_row_ordinal: fact,
        };
        mutations.push(bad);
    }
    for bad in mutations {
        assert!(matches!(
            run_pair(&input, &output, &bad),
            Err(Error::Mismatch(_))
        ));
    }
}
#[test]
fn induction_refinement_pair_rejects_changed_result_operand_constant_and_adjacency() {
    let input = fixture(ScalarType::U32, 1, false);
    let (output, rows) = expected(&input, &[(0, 2, 0, 0)]);
    for mutation in 0..6 {
        let mut bad = output.clone();
        let block = output_block(&mut bad);
        match mutation {
            0 => block.operations[1].kind = OperationKind::Constant(Constant::Bool(true)),
            1 => {
                if let OperationKind::Binary { op, .. } = &mut block.operations[0].kind {
                    *op = BinaryOp::Subtract;
                }
            }
            2 => {
                if let OperationKind::Binary { lhs, rhs, .. } = &mut block.operations[0].kind {
                    std::mem::swap(lhs, rhs);
                }
            }
            3 => block.operations[1].results[0].id = ValueId(60),
            4 => block.operations.swap(0, 1),
            5 => block.operations.push(Operation::effect_free(
                ValueDef::new(ValueId(99), Type::BOOL),
                OperationKind::Constant(Constant::Bool(false)),
            )),
            _ => unreachable!(),
        }
        assert!(matches!(
            run_pair(&input, &bad, &rows),
            Err(Error::Mismatch(_))
        ));
    }
    let mut invalid = output;
    output_block(&mut invalid).operations[1].results[0].ty = Type::Scalar(ScalarType::U32);
    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    assert!(
        Owner::from_module_ref_with_verification_budget_v12(&invalid, &mut budget).is_err(),
        "invalid result type is rejected by owner admission, not credited as pair replay"
    );
}
#[test]
fn induction_refinement_pair_preserves_all_metadata_cfg_and_effect_payloads() {
    let input = fixture(ScalarType::U32, 1, false);
    let (output, rows) = expected(&input, &[(0, 2, 0, 0)]);
    for mutation in 0..3 {
        let mut bad = output.clone();
        match mutation {
            0 => bad.id = "different".into(),
            1 => bad.functions[0].id = "different".into(),
            2 => {
                bad.functions[0].body.as_mut().unwrap().blocks[3].terminator =
                    Some(Terminator::Unreachable)
            }
            _ => unreachable!(),
        }
        assert!(matches!(
            run_pair(&input, &bad, &rows),
            Err(Error::Mismatch(_))
        ));
    }
    let mut input = input;
    output_block(&mut input)
        .operations
        .push(constant(99, ScalarType::U32, 7));
    let (mut output, rows) = expected(&input, &[(0, 2, 0, 0)]);
    output_block(&mut output).operations[2].kind = OperationKind::Constant(Constant::U32(8));
    assert!(matches!(
        run_pair(&input, &output, &rows),
        Err(Error::Mismatch("unchanged complete operation"))
    ));
    let mut input = fixture(ScalarType::U32, 1, false);
    input.functions[0].signature.parameters.push(Type::pointer(
        ty(ScalarType::U32),
        fe2o3_kernel_ir::AddressSpace::Global,
        fe2o3_kernel_ir::AccessMode::ReadWrite,
    ));
    input.functions[0]
        .body
        .as_mut()
        .unwrap()
        .parameters
        .push(ValueId(10));
    output_block(&mut input).operations.push(Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(10),
            value: ValueId(5),
            access: fe2o3_kernel_ir::MemoryAccess::new(fe2o3_kernel_ir::AddressSpace::Global, 4),
        },
    ));
    let (mut output, rows) = expected(&input, &[(0, 2, 0, 0)]);
    if let OperationKind::Store { value, .. } = &mut output_block(&mut output).operations[2].kind {
        *value = ValueId(3);
    }
    assert!(matches!(
        run_pair(&input, &output, &rows),
        Err(Error::Mismatch("unchanged complete operation"))
    ));
}
#[test]
fn induction_refinement_pair_rejects_unproved_guard_update_and_bound_mutants() {
    for mutation in 0..5 {
        let mut input = fixture(ScalarType::U32, 1, false);
        let body = input.functions[0].body.as_mut().unwrap();
        match mutation {
            0 => {
                if let OperationKind::Compare { predicate, .. } =
                    &mut body.blocks[1].operations[0].kind
                {
                    *predicate = ComparePredicate::GreaterThan;
                }
            }
            1 => body.blocks[0].operations[0].kind = OperationKind::Constant(Constant::U32(2)),
            2 => {
                if let OperationKind::Compare { lhs, rhs, .. } =
                    &mut body.blocks[1].operations[0].kind
                {
                    std::mem::swap(lhs, rhs);
                }
            }
            3 => {
                if let OperationKind::Compare { predicate, .. } =
                    &mut body.blocks[1].operations[0].kind
                {
                    *predicate = ComparePredicate::LessThanOrEqual;
                }
            }
            4 => {
                if let Some(Terminator::ConditionalBranch {
                    then_target,
                    else_target,
                    ..
                }) = &mut body.blocks[1].terminator
                {
                    std::mem::swap(then_target, else_target);
                }
            }
            _ => unreachable!(),
        }
        let (output, rows) = expected(&input, &[(0, 2, 0, 0)]);
        assert!(matches!(
            run_pair(&input, &output, &rows),
            Err(Error::Mismatch(_))
        ));
    }
    let mut input = fixture(ScalarType::U32, 1, false);
    let body = input.functions[0].body.as_mut().unwrap();
    let update = body.blocks[2].operations.pop().unwrap();
    body.blocks[1].operations.push(update);
    let (output, rows) = expected(&input, &[(0, 1, 1, 0)]);
    assert!(
        matches!(run_pair(&input, &output, &rows), Err(Error::Mismatch(_))),
        "header-block dominance does not prove the update follows the taken body edge"
    );
}
#[test]
fn induction_refinement_pair_first_qualifying_row_and_zero_selection_are_exact() {
    let input = fixture(ScalarType::U32, 2, false);
    let (output, rows) = expected(&input, &[]);
    run_pair(&input, &output, &rows).unwrap();
    assert_eq!(input, output);
    let input = fixture(ScalarType::U32, 1, false);
    let (output, rows) = expected(&input, &[]);
    assert!(
        matches!(run_pair(&input, &output, &rows), Err(Error::Mismatch(_))),
        "eligible update cannot be silently omitted"
    );
    let mut input = fixture(ScalarType::U32, 1, false);
    let mut second = input.functions[0].clone();
    second.id = "g".into();
    input.functions.push(second);
    let (output, rows) = expected(&input, &[(0, 2, 0, 0), (1, 2, 0, 1)]);
    run_pair(&input, &output, &rows).unwrap();
}

#[path = "canonical_kir_induction_refinement_resources_v1_tests.rs"]
mod resources_tests;
