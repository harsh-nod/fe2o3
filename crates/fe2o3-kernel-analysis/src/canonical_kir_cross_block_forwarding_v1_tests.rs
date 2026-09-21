use super::*;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKirFunctionCoordinateV1 as FunctionId, Function, Operation, Signature, Terminator,
    ValueDef,
};
const WORK: usize = 100_000_000;
const STORAGE: usize = 64 << 20;
fn ty() -> Type {
    Type::Scalar(ScalarType::U32)
}
fn jump(target: u32) -> Terminator {
    Terminator::Branch {
        target: BlockId(target),
        arguments: vec![],
    }
}
fn branch(yes: u32, no: u32) -> Terminator {
    Terminator::ConditionalBranch {
        condition: ValueId(1),
        then_target: BlockId(yes),
        then_arguments: vec![],
        else_target: BlockId(no),
        else_arguments: vec![],
    }
}
fn block(id: u32, operations: Vec<Operation>, terminator: Terminator) -> BasicBlock {
    let mut result = BasicBlock::new(BlockId(id));
    result.operations = operations;
    result.terminator = Some(terminator);
    result
}
fn allocation(id: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(
            ValueId(id),
            Type::pointer(ty(), AddressSpace::Private, AccessMode::ReadWrite),
        ),
        Kind::Alloca {
            element: ty(),
            count: None,
            address_space: AddressSpace::Private,
            alignment: 4,
        },
    )
}
fn store(pointer: u32, value: u32) -> Operation {
    Operation::new(
        vec![],
        Kind::Store {
            pointer: ValueId(pointer),
            value: ValueId(value),
            access: MemoryAccess::new(AddressSpace::Private, 4),
        },
    )
}
fn load() -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(101), ty()),
        Kind::Load {
            pointer: ValueId(100),
            access: MemoryAccess::new(AddressSpace::Private, 4),
        },
    )
}
fn fixture() -> Module {
    let mut module = Module::new("cross-block-pair");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![ty(), Type::BOOL], vec![ty()]),
        vec![ValueId(0), ValueId(1)],
        vec![
            block(10, vec![allocation(100), store(100, 0)], branch(20, 30)),
            block(20, vec![], jump(40)),
            block(30, vec![], jump(40)),
            block(
                40,
                vec![load()],
                Terminator::Return {
                    values: vec![ValueId(101)],
                },
            ),
        ],
    ));
    module
}
fn blocks(module: &mut Module) -> &mut Vec<BasicBlock> {
    &mut module.functions[0].body.as_mut().unwrap().blocks
}
fn site(block: usize, operation: usize) -> Site {
    Site {
        block: Block {
            function: FunctionId(0),
            block: block as u32,
        },
        operation: operation as u32,
    }
}
fn rewritten(input: &Module, selected: bool) -> (Module, Vec<Row>) {
    let mut output = input.clone();
    let mut rows = Vec::new();
    let mut anchor = None;
    for (b, block) in input.functions[0]
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .enumerate()
    {
        for (o, op) in block.operations.iter().enumerate() {
            if matches!(
                op.kind,
                Kind::Store {
                    pointer: ValueId(100),
                    ..
                }
            ) && anchor.is_none()
            {
                anchor = Some(site(b, o));
            }
        }
    }
    for (b, block) in input.functions[0]
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .enumerate()
    {
        for (o, op) in block.operations.iter().enumerate() {
            let at = site(b, o);
            let chosen = selected
                && matches!(
                    op.kind,
                    Kind::Load {
                        pointer: ValueId(100),
                        ..
                    }
                );
            if chosen {
                blocks(&mut output)[b].operations[o].kind = Kind::Binary {
                    op: BinaryOp::BitOr,
                    lhs: ValueId(0),
                    rhs: ValueId(0),
                };
            }
            rows.push(Row {
                input: at,
                output: at,
                store: if chosen { anchor } else { None },
            });
        }
    }
    (output, rows)
}
fn admit(module: &Module) -> Owner {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    Owner::from_module_ref_with_verification_budget_v12(module, &mut budget)
        .unwrap()
        .0
}
fn run(
    input: &Owner,
    output: &Owner,
    rows: &[Row],
    work_limit: usize,
    storage_limit: usize,
) -> (Result<()>, usize, usize, Option<usize>, Option<usize>) {
    let sibling = vec![0xa7u8; 31];
    let floor = size_of_val(&sibling) + sibling.capacity();
    let mut work = Work::new(work_limit);
    work.charge_work(17).unwrap();
    let (result, used, peak, storage_failure) = {
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = check_canonical_kir_cross_block_forwarding_v1(
            input,
            output,
            rows,
            Limits::default(),
            &mut budget,
        )
        .map(|(pair, receipt)| {
            assert!(std::ptr::eq(pair.input(), input));
            assert!(std::ptr::eq(pair.output(), output));
            assert!(std::ptr::eq(pair.origins(), rows));
            assert!(!pair.grants_authority());
            assert_eq!(
                receipt.retained_storage(),
                size_of::<CheckedCanonicalKirCrossBlockForwardingV1<'_>>()
            );
        });
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(sibling, [0xa7; 31]);
        (
            result,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        )
    };
    (result, used, peak, work.failed_work(), storage_failure)
}
fn accept(input: &Module, selected: bool) {
    let (output, rows) = rewritten(input, selected);
    run(&admit(input), &admit(&output), &rows, WORK, STORAGE)
        .0
        .unwrap();
}
fn reject(input: &Module, detail: &'static str) {
    let (output, rows) = rewritten(input, true);
    assert_eq!(
        run(&admit(input), &admit(&output), &rows, WORK, STORAGE).0,
        Err(Error::Mismatch(detail))
    );
}

#[test]
fn cross_block_pair_checks_diamond_duplicate_edges_and_sparse_ids() {
    let mut input = fixture();
    accept(&input, true);
    blocks(&mut input)[0].terminator = Some(branch(20, 20));
    blocks(&mut input)[2].terminator = Some(Terminator::Unreachable);
    accept(&input, true);
    blocks(&mut input)[2].terminator = Some(jump(40));
    // Syntactic unreachable predecessors are not removed from MemorySSA.
    reject(&input, "each phi needs an actual Store path");
}
#[test]
fn cross_block_pair_preserves_duplicate_legacy_and_typed_switch_occurrences() {
    let mut input = fixture();
    blocks(&mut input)[0].terminator = Some(Terminator::Switch {
        selector: ValueId(0),
        cases: vec![
            fe2o3_kernel_ir::SwitchCase {
                value: 0,
                target: BlockId(20),
                arguments: vec![],
            },
            fe2o3_kernel_ir::SwitchCase {
                value: 1,
                target: BlockId(20),
                arguments: vec![],
            },
        ],
        default_target: BlockId(30),
        default_arguments: vec![],
    });
    accept(&input, true);
    blocks(&mut input)[0].terminator = Some(Terminator::IntegerSwitch {
        selector: ValueId(0),
        cases: vec![
            fe2o3_kernel_ir::IntegerSwitchCase {
                value: fe2o3_kernel_ir::Constant::U32(0),
                target: BlockId(20),
                arguments: vec![],
            },
            fe2o3_kernel_ir::IntegerSwitchCase {
                value: fe2o3_kernel_ir::Constant::U32(1),
                target: BlockId(20),
                arguments: vec![],
            },
        ],
        default_target: BlockId(30),
        default_arguments: vec![],
    });
    accept(&input, true);
}
#[test]
fn cross_block_pair_every_phi_is_grounded_not_just_one_terminal_per_label() {
    let mut input = fixture();
    blocks(&mut input)[0].terminator = Some(jump(40));
    blocks(&mut input)[1].terminator = Some(branch(20, 40));
    blocks(&mut input)[2].terminator = Some(Terminator::Unreachable);
    // Load phi A = phi(StoreS, B), B = phi(B). StoreS dominates the reachable
    // load, but this separate predecessor SCC has no actual Store terminal.
    reject(&input, "each phi needs an actual Store path");
    accept(&input, false);
    blocks(&mut input)[0].terminator = Some(branch(20, 40));
    accept(&input, true);
}
#[test]
fn cross_block_pair_grounded_irreducible_phi_cycle_and_duplicate_occurrences() {
    let mut input = fixture();
    blocks(&mut input)[1].terminator = Some(branch(30, 40));
    blocks(&mut input)[2].terminator = Some(branch(20, 40));
    accept(&input, true);
    blocks(&mut input)[2].operations.push(store(100, 0));
    reject(&input, "every phi input is the same Store or phi");
}
#[test]
fn cross_block_pair_exact_store_identity_and_all_memory_defs_are_cuts() {
    let mut input = fixture();
    blocks(&mut input)[1].operations.push(store(100, 0));
    reject(&input, "every phi input is the same Store or phi");
    let mut input = fixture();
    blocks(&mut input)[1].operations.push(allocation(200));
    reject(&input, "every phi input is the same Store or phi");
    let mut input = fixture();
    blocks(&mut input)[0].operations.remove(1);
    let (output, mut rows) = rewritten(&input, true);
    rows.last_mut().unwrap().store = Some(site(0, 0));
    assert_eq!(
        run(&admit(&input), &admit(&output), &rows, WORK, STORAGE).0,
        Err(Error::Mismatch("exact ordinary direct-slot Store"))
    );
}
#[test]
fn cross_block_pair_retains_conservative_totality_and_prior_read_cuts() {
    let mut input = fixture();
    blocks(&mut input)[1]
        .operations
        .push(Operation::effect_free(
            ValueDef::new(ValueId(200), ty()),
            Kind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(0),
                rhs: ValueId(0),
            },
        ));
    reject(&input, "complete predecessor interval");
    let mut input = fixture();
    blocks(&mut input)[1]
        .operations
        .push(Operation::effect_free(
            ValueDef::new(ValueId(200), ty()),
            Kind::Binary {
                op: BinaryOp::BitXor,
                lhs: ValueId(0),
                rhs: ValueId(0),
            },
        ));
    accept(&input, true);
    let mut input = fixture();
    let mut read = load();
    read.results[0].id = ValueId(200);
    blocks(&mut input)[1].operations.push(read);
    let (mut output, mut rows) = rewritten(&input, true);
    blocks(&mut output)[1].operations[0] = blocks(&mut input)[1].operations[0].clone();
    rows.iter_mut()
        .find(|row| row.input == site(1, 0))
        .unwrap()
        .store = None;
    assert_eq!(
        run(&admit(&input), &admit(&output), &rows, WORK, STORAGE).0,
        Err(Error::Mismatch("complete predecessor interval"))
    );
}
#[test]
fn cross_block_pair_checks_every_retained_operation_coordinate_and_metadata() {
    let input = fixture();
    let (mut output, mut rows) = rewritten(&input, true);
    let a = admit(&input);
    let b = admit(&output);
    rows.swap(0, 1);
    assert_eq!(
        run(&a, &b, &rows, WORK, STORAGE).0,
        Err(Error::Mismatch("complete unchanged coordinates"))
    );
    rows.swap(0, 1);
    rows.pop();
    assert_eq!(
        run(&a, &b, &rows, WORK, STORAGE).0,
        Err(Error::Mismatch("complete operation cardinality"))
    );
    let (_, rows) = rewritten(&input, true);
    blocks(&mut output)[3].operations[0].kind = Kind::Binary {
        op: BinaryOp::BitXor,
        lhs: ValueId(0),
        rhs: ValueId(0),
    };
    assert_eq!(
        run(&a, &admit(&output), &rows, WORK, STORAGE).0,
        Err(Error::Mismatch("exact cross-block typed Load copy"))
    );
}
#[test]
fn cross_block_pair_work_denial_is_exact_and_preserves_floor_and_history() {
    for selected in [false, true] {
        let input = fixture();
        let (output, rows) = rewritten(&input, selected);
        let a = admit(&input);
        let b = admit(&output);
        let full = run(&a, &b, &rows, WORK, STORAGE);
        full.0.unwrap();
        let exact = run(&a, &b, &rows, full.1, full.2);
        exact.0.unwrap();
        assert_eq!(
            (exact.1, exact.2, exact.3, exact.4),
            (full.1, full.2, None, None)
        );
        let short = run(&a, &b, &rows, full.1 - 1, full.2);
        match short.0 {
            Err(Error::Resource(Resource::Work(error))) => {
                assert_eq!((error.actual(), error.limit()), (full.1, full.1 - 1));
            }
            other => panic!("exact final pair charge refusal: {other:?}"),
        }
        assert_eq!(
            (short.1, short.2, short.3, short.4),
            (full.1 - 1, full.2, Some(full.1), None)
        );
    }
}
#[test]
fn cross_block_pair_storage_first_denial_observation_requires_strict_successor() {
    for selected in [false, true] {
        let input = fixture();
        let (output, rows) = rewritten(&input, selected);
        let a = admit(&input);
        let b = admit(&output);
        let full = run(&a, &b, &rows, WORK, STORAGE);
        full.0.unwrap();
        let short = run(&a, &b, &rows, WORK, full.2 - 1);
        let Err(Error::ControlFlow(
            fe2o3_kernel_ir::CanonicalKirControlFlowScopeErrorV1::Resource(Resource::Storage(
                limit,
            )),
        )) = &short.0
        else {
            panic!("exact dominator-interval stack refusal: {:?}", short.0)
        };
        assert_eq!((limit.actual(), limit.limit()), (full.2, full.2 - 1));
        let body = input.functions[0].body.as_ref().unwrap();
        let blocks = body.blocks.len();
        let edges = body
            .blocks
            .iter()
            .map(|b| match b.terminator.as_ref().unwrap() {
                Terminator::Branch { .. } => 1,
                Terminator::ConditionalBranch { .. } => 2,
                Terminator::Return { .. } => 0,
                _ => panic!("unchanged acyclic diamond"),
            })
            .sum::<usize>();
        let operations = body
            .blocks
            .iter()
            .map(|b| b.operations.len())
            .sum::<usize>();
        assert_eq!((blocks, edges, operations), (4, 4, 3));
        assert_eq!(
            rows.iter().filter(|r| r.store.is_some()).count(),
            usize::from(selected)
        );
        // The all-reachable diamond's interval walk costs 10B+1; its final
        // reducibility scan costs 13B+11E+7. Queries cost 3O + (4+11)S.
        let tail_work =
            23 * blocks + 11 * edges + 8 + 3 * operations + 15 * usize::from(selected) + 1;
        assert_eq!(
            (short.1, short.2, short.3, short.4),
            (full.1 - tail_work, full.2 - 2 * blocks, None, Some(full.2))
        );
    }
}

use std::mem::size_of_val;

#[test]
fn cross_block_pair_prepays_all_thirteen_scratch_headers_before_backing() {
    assert_eq!(scratch_headers().unwrap(), 13 * size_of::<Vec<u8>>());
    let input = fixture();
    let (output, rows) = rewritten(&input, true);
    let a = admit(&input);
    let b = admit(&output);
    let sibling = vec![0xa7u8; 31];
    let floor = size_of_val(&sibling) + sibling.capacity();
    let prefix = floor
        + size_of::<Meter<'_, '_>>()
        + size_of::<CheckedCanonicalKirCrossBlockForwardingV1<'_>>();
    let attempted = prefix + scratch_headers().unwrap();
    let short = run(&a, &b, &rows, WORK, attempted - 1);
    match short.0 {
        Err(Error::Resource(Resource::Storage(error))) => {
            assert_eq!((error.actual(), error.limit()), (attempted, attempted - 1))
        }
        other => panic!("exact scratch header prepayment: {other:?}"),
    }
    assert_eq!(
        (short.1, short.2, short.3, short.4),
        (30, prefix, None, Some(attempted))
    );
}
