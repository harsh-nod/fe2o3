use super::*;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, ComparePredicate, Function, Module,
    Signature, Terminator, ValueId,
};
const W: usize = 1_000_000_000;
const S: usize = 1024 * 1024 * 1024;
const FLOOR: usize = 43;
fn scalar(s: ScalarType) -> Type {
    Type::Scalar(s)
}
fn constant(s: ScalarType, v: u64) -> Constant {
    match s {
        ScalarType::U8 => Constant::U8(v.try_into().unwrap()),
        ScalarType::U16 => Constant::U16(v.try_into().unwrap()),
        ScalarType::U32 => Constant::U32(v.try_into().unwrap()),
        ScalarType::U64 => Constant::U64(v),
        ScalarType::I8 => Constant::I8(v.try_into().unwrap()),
        ScalarType::I16 => Constant::I16(v.try_into().unwrap()),
        ScalarType::I32 => Constant::I32(v.try_into().unwrap()),
        ScalarType::I64 => Constant::I64(v.try_into().unwrap()),
        ScalarType::Index => Constant::Index(v),
        _ => panic!("fixture scalar"),
    }
}
fn literal(id: u32, s: ScalarType, v: u64) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), scalar(s)),
        OperationKind::Constant(constant(s, v)),
    )
}
fn branch(id: u32, values: &[u32]) -> Terminator {
    Terminator::Branch {
        target: BlockId(id),
        arguments: values.iter().copied().map(ValueId).collect(),
    }
}
fn conditional(condition: u32, yes: u32, no: u32) -> Terminator {
    Terminator::ConditionalBranch {
        condition: ValueId(condition),
        then_target: BlockId(yes),
        then_arguments: vec![],
        else_target: BlockId(no),
        else_arguments: vec![],
    }
}
fn block(
    id: u32,
    params: Vec<ValueDef>,
    operations: Vec<Operation>,
    term: Terminator,
) -> BasicBlock {
    let mut b = BasicBlock::new(BlockId(id));
    b.parameters = params;
    b.operations = operations;
    b.terminator = Some(term);
    b
}
fn fixture(s: ScalarType, literals: Option<(u64, u64)>, step: u64, checked: bool) -> Module {
    let mut entry = vec![literal(10, s, step)];
    let (initial, bound) = if let Some((a, b)) = literals {
        entry.extend([literal(11, s, a), literal(12, s, b)]);
        (11, 12)
    } else {
        (0, 1)
    };
    let update = if checked {
        Operation::checked_binary(
            ValueDef::new(ValueId(30), scalar(s)),
            ValueDef::new(ValueId(31), Type::BOOL),
            CheckedBinaryOperator::Add,
            ValueId(20),
            ValueId(10),
        )
    } else {
        Operation::effect_free(
            ValueDef::new(ValueId(30), scalar(s)),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(20),
                rhs: ValueId(10),
            },
        )
    };
    let mut m = Module::new("owned-induction-refinement");
    m.functions.push(Function::internal_helper(
        "refine",
        Signature::new(vec![scalar(s), scalar(s)], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![
            block(13, vec![], entry, branch(41, &[initial])),
            block(
                41,
                vec![ValueDef::new(ValueId(20), scalar(s))],
                vec![Operation::effect_free(
                    ValueDef::new(ValueId(21), Type::BOOL),
                    OperationKind::Compare {
                        predicate: ComparePredicate::LessThan,
                        lhs: ValueId(20),
                        rhs: ValueId(bound),
                    },
                )],
                conditional(21, 97, 701),
            ),
            block(97, vec![], vec![update], branch(41, &[30])),
            block(701, vec![], vec![], Terminator::Return { values: vec![] }),
        ],
    ));
    m
}
fn admit(m: &Module) -> (Owner, usize) {
    let mut w = Work::new(W);
    let mut b = Budget::new(&mut w, S);
    let (owner, receipt) = Owner::from_module_ref_with_verification_budget_v12(m, &mut b).unwrap();
    (owner, receipt.retained_storage())
}
fn with_input(m: Module, run: impl FnOnce(&Owner, &mut Budget<'_>)) {
    let (input, bytes) = admit(&m);
    let sibling = vec![0x85u8; FLOOR];
    let floor = bytes + sibling.capacity();
    let mut w = Work::new(W);
    let mut b = Budget::new(&mut w, S);
    b.reserve_storage(floor).unwrap();
    let ledger = b.work_ledger_identity_v1();
    run(&input, &mut b);
    assert_eq!(b.storage(), floor);
    assert!(b.work_ledger_identity_v1() == ledger);
    assert_eq!(input.module(), &m);
    assert_eq!(sibling, [0x85; FLOOR]);
}
fn release(owner: OwnedInductionRefinementContinuationV1, budget: &mut Budget<'_>) {
    let bytes = owner.retained_storage();
    drop(owner);
    budget.release_storage(bytes).unwrap();
}
fn verify(owner: &OwnedInductionRefinementContinuationV1, input: &Owner, budget: &mut Budget<'_>) {
    let floor = budget.storage();
    let retained = {
        let (pair, receipt) = owner.replay_against(input, owner.limits(), budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert!(std::ptr::eq(pair.input(), input));
        assert!(std::ptr::eq(pair.output(), owner.output()));
        assert_eq!(pair.origins(), owner.origins());
        assert_eq!(pair.limits(), owner.limits());
        assert!(!pair.grants_authority());
        receipt.retained_storage()
    };
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), floor);
}
fn selected(owner: &OwnedInductionRefinementContinuationV1) -> usize {
    owner
        .origins()
        .iter()
        .filter(|r| matches!(r, Row::CheckedAddSplit { .. }))
        .count()
}
fn ids(m: &Module) -> Vec<Vec<ValueDef>> {
    m.functions
        .iter()
        .filter_map(|f| {
            f.body.as_ref().map(|b| {
                let mut result: Vec<_> = b
                    .parameters
                    .iter()
                    .zip(&f.signature.parameters)
                    .map(|(id, ty)| ValueDef::new(*id, ty.clone()))
                    .collect();
                for block in &b.blocks {
                    result.extend(block.parameters.clone());
                    for op in &block.operations {
                        result.extend(op.results.clone());
                    }
                }
                result.sort_by_key(|v| v.id.0);
                result
            })
        })
        .collect()
}
#[test]
fn owned_induction_refinement_unsigned_widths_keep_both_ids_and_all_uses() {
    for s in [
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
    ] {
        with_input(fixture(s, None, 1, true), |input, budget| {
            let owner =
                prepare_owned_induction_refinement_v1(input, Limits::default(), budget).unwrap();
            budget.reserve_storage(owner.retained_storage()).unwrap();
            assert_eq!(selected(&owner), 1);
            assert_eq!(ids(input.module()), ids(owner.output().module()));
            let body = &owner.output().module().functions[0]
                .body
                .as_ref()
                .unwrap()
                .blocks[2];
            assert_eq!(body.operations.len(), 2);
            assert_eq!(body.operations[0].results[0].id, ValueId(30));
            assert_eq!(body.operations[1].results[0].id, ValueId(31));
            assert_eq!(
                body.operations[1].kind,
                OperationKind::Constant(Constant::Bool(false))
            );
            assert_eq!(
                body.terminator,
                input.module().functions[0].body.as_ref().unwrap().blocks[2].terminator
            );
            verify(&owner, input, budget);
            release(owner, budget);
        });
    }
}
#[test]
fn owned_induction_refinement_literal_strides_and_symbolic_boundaries() {
    for (s, a, b, step) in [
        (ScalarType::U8, 1, 8, 3),
        (ScalarType::U8, 250, 255, 1),
        (ScalarType::U16, 65526, 65535, 3),
        (ScalarType::U32, u32::MAX as u64 - 9, u32::MAX as u64, 3),
        (ScalarType::U64, u64::MAX - 9, u64::MAX, 3),
    ] {
        with_input(fixture(s, Some((a, b)), step, true), |input, budget| {
            let owner =
                prepare_owned_induction_refinement_v1(input, Limits::default(), budget).unwrap();
            budget.reserve_storage(owner.retained_storage()).unwrap();
            assert_eq!(selected(&owner), 1);
            verify(&owner, input, budget);
            release(owner, budget);
        });
    }
}
fn nested() -> Module {
    let mut m = fixture(ScalarType::U32, None, 1, true);
    let body = m.functions[0].body.as_mut().unwrap();
    body.blocks[0]
        .operations
        .push(literal(40, ScalarType::U32, 0));
    body.blocks[1].terminator = Some(conditional(21, 80, 701));
    body.blocks.extend([
        block(80, vec![], vec![], branch(81, &[40])),
        block(
            81,
            vec![ValueDef::new(ValueId(41), scalar(ScalarType::U32))],
            vec![Operation::effect_free(
                ValueDef::new(ValueId(42), Type::BOOL),
                OperationKind::Compare {
                    predicate: ComparePredicate::LessThan,
                    lhs: ValueId(41),
                    rhs: ValueId(1),
                },
            )],
            conditional(42, 82, 97),
        ),
        block(
            82,
            vec![],
            vec![Operation::checked_binary(
                ValueDef::new(ValueId(43), scalar(ScalarType::U32)),
                ValueDef::new(ValueId(44), Type::BOOL),
                CheckedBinaryOperator::Add,
                ValueId(41),
                ValueId(10),
            )],
            branch(81, &[43]),
        ),
    ]);
    m
}
#[test]
fn owned_induction_refinement_nested_multiple_functions_and_sparse_ids() {
    let mut m = nested();
    let body = m.functions[0].body.as_mut().unwrap();
    body.blocks[0].operations[0].results[0].id = ValueId(u32::MAX - 1);
    for block in &mut body.blocks {
        for operation in &mut block.operations {
            if let OperationKind::Binary { rhs, .. } = &mut operation.kind
                && *rhs == ValueId(10)
            {
                *rhs = ValueId(u32::MAX - 1);
            }
        }
    }
    let mut other = m.functions[0].clone();
    other.id = "other".into();
    m.functions.push(other);
    with_input(m, |input, budget| {
        let owner =
            prepare_owned_induction_refinement_v1(input, Limits::default(), budget).unwrap();
        budget.reserve_storage(owner.retained_storage()).unwrap();
        assert_eq!(selected(&owner), 4);
        assert_eq!(ids(input.module()), ids(owner.output().module()));
        verify(&owner, input, budget);
        release(owner, budget);
    });
}
#[test]
fn owned_induction_refinement_excluded_updates_are_exact_noops() {
    let mut subtract = fixture(ScalarType::U32, None, 1, true);
    if let OperationKind::Binary { op, .. } =
        &mut subtract.functions[0].body.as_mut().unwrap().blocks[2].operations[0].kind
    {
        *op = BinaryOp::Checked(CheckedBinaryOperator::Subtract);
    }
    for m in [
        fixture(ScalarType::U32, None, 1, false),
        fixture(ScalarType::U32, None, 2, true),
        fixture(ScalarType::I8, None, 1, true),
        fixture(ScalarType::I16, None, 1, true),
        fixture(ScalarType::I32, None, 1, true),
        fixture(ScalarType::I64, None, 1, true),
        fixture(ScalarType::Index, None, 1, true),
        fixture(ScalarType::U8, Some((250, 255)), 2, true),
        fixture(ScalarType::U32, Some((4, 4)), 1, true),
        subtract,
    ] {
        with_input(m, |input, budget| {
            let owner =
                prepare_owned_induction_refinement_v1(input, Limits::default(), budget).unwrap();
            budget.reserve_storage(owner.retained_storage()).unwrap();
            assert_eq!(selected(&owner), 0);
            assert_eq!(
                input.canonical().canonical_bytes(),
                owner.output().canonical().canonical_bytes()
            );
            verify(&owner, input, budget);
            release(owner, budget);
        });
    }
}
fn record() -> String {
    let mut text = String::new();
    with_input(nested(), |input, budget| {
        let owner =
            prepare_owned_induction_refinement_v1(input, Limits::default(), budget).unwrap();
        budget.reserve_storage(owner.retained_storage()).unwrap();
        assert_eq!(selected(&owner), 2);
        text = format!(
            "{:?}\n{:?}",
            owner.output().canonical().canonical_bytes(),
            owner.origins()
        );
        release(owner, budget);
    });
    text
}
#[test]
fn owned_induction_refinement_deterministic_child() {
    if std::env::var("FE2O3_INDUCTION_REFINEMENT_CHILD").as_deref() == Ok("1") {
        println!(
            "INDUCTION_REFINEMENT_RECORD_BEGIN\n{}\nINDUCTION_REFINEMENT_RECORD_END",
            record()
        );
    }
}
#[test]
fn owned_induction_refinement_two_processes_match_bytes_and_complete_origins() {
    let run = || {
        let result=std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact","owned_induction_refinement_v1::tests::owned_induction_refinement_deterministic_child","--nocapture","--test-threads=1"]).env("FE2O3_INDUCTION_REFINEMENT_CHILD","1").output().unwrap();
        assert!(result.status.success());
        let text = String::from_utf8(result.stdout).unwrap();
        assert!(text.contains("1 passed; 0 failed"));
        let begin = "INDUCTION_REFINEMENT_RECORD_BEGIN\n";
        let end = "\nINDUCTION_REFINEMENT_RECORD_END";
        assert_eq!(text.matches(begin).count(), 1);
        assert_eq!(text.matches(end).count(), 1);
        text.split_once(begin)
            .unwrap()
            .1
            .split_once(end)
            .unwrap()
            .0
            .to_owned()
    };
    let first = run();
    assert_eq!(first, run());
    assert_eq!(first, record());
    assert_eq!(record(), record());
}
#[test]
fn owned_induction_refinement_replay_checks_input_receipt_and_all_limits() {
    with_input(fixture(ScalarType::U32, None, 1, true), |input, budget| {
        let owner =
            prepare_owned_induction_refinement_v1(input, Limits::default(), budget).unwrap();
        let mut work = Work::new(W);
        let mut short = Budget::new(&mut work, S);
        short.reserve_storage(owner.retained_storage() - 1).unwrap();
        assert!(matches!(
            owner.replay_against(input, Limits::default(), &mut short),
            Err(Error::Resource(Resource::Accounting))
        ));
        budget.reserve_storage(owner.retained_storage()).unwrap();
        let floor = budget.storage();
        for field in 0..7 {
            for delta in [-1isize, 1] {
                let mut limits = Limits::default();
                let slot = match field {
                    0 => &mut limits.functions,
                    1 => &mut limits.blocks,
                    2 => &mut limits.edges,
                    3 => &mut limits.definitions,
                    4 => &mut limits.operations,
                    5 => &mut limits.loops,
                    _ => &mut limits.rows,
                };
                *slot = slot.checked_add_signed(delta).unwrap();
                let before = budget.work();
                assert!(matches!(
                    owner.replay_against(input, limits, budget),
                    Err(Error::LimitsMismatch)
                ));
                assert_eq!(budget.work(), before + 7);
                assert_eq!(budget.storage(), floor);
            }
        }
        let mut m = input.module().clone();
        m.id = "foreign".into();
        let (foreign, bytes) = admit(&m);
        budget.reserve_storage(bytes).unwrap();
        assert!(matches!(
            owner.replay_against(&foreign, Limits::default(), budget),
            Err(Error::ForeignInput)
        ));
        drop(foreign);
        budget.release_storage(bytes).unwrap();
        verify(&owner, input, budget);
        release(owner, budget);
    });
}

#[path = "induction_refinement_resources_v1_tests.rs"]
mod resources_tests;
#[path = "induction_refinement_sim_v1_tests.rs"]
mod sim_tests;
