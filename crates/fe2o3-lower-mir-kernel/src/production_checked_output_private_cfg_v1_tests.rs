use super::*;
use fe2o3_kernel_ir::{AccessMode, BasicBlock, MemoryAccess, Signature, ValueDef};

pub(super) const WORK: usize = 50_000_000;
pub(super) const STORAGE: usize = 64 * 1024 * 1024;

fn branch(target: u32) -> Terminator {
    Terminator::Branch {
        target: BlockId(target),
        arguments: vec![],
    }
}

fn conditional(left: u32, right: u32) -> Terminator {
    Terminator::ConditionalBranch {
        condition: ValueId(900),
        then_target: BlockId(left),
        then_arguments: vec![],
        else_target: BlockId(right),
        else_arguments: vec![],
    }
}

fn block(id: u32, operations: Vec<Operation>, terminator: Terminator) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.operations = operations;
    block.terminator = Some(terminator);
    block
}

fn allocation() -> Operation {
    Operation::effect_free(
        ValueDef::new(
            ValueId(10),
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            ),
        ),
        OperationKind::Alloca {
            element: Type::Scalar(ScalarType::U32),
            count: None,
            address_space: AddressSpace::Private,
            alignment: 4,
        },
    )
}

fn constant() -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(20), Type::Scalar(ScalarType::U32)),
        OperationKind::Constant(Constant::U32(99)),
    )
}

fn store() -> Operation {
    Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(10),
            value: ValueId(20),
            access: MemoryAccess::new(AddressSpace::Private, 4),
        },
    )
}

fn load() -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(40), Type::Scalar(ScalarType::U32)),
        OperationKind::Load {
            pointer: ValueId(10),
            access: MemoryAccess::new(AddressSpace::Private, 4),
        },
    )
}

fn ret() -> Terminator {
    Terminator::Return { values: vec![] }
}

fn module(blocks: Vec<BasicBlock>) -> Module {
    let mut module = Module::new("private-cfg-component");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![Type::Scalar(ScalarType::Bool)], vec![]),
        vec![ValueId(900)],
        blocks,
    ));
    module
}

pub(super) fn split() -> Module {
    module(vec![
        block(100, vec![allocation(), constant(), store()], branch(300)),
        block(300, vec![load()], ret()),
    ])
}

pub(super) fn with_inventory<T>(
    module: &Module,
    action: impl FnOnce(&CanonicalKirInventoryV1<'_>, usize) -> T,
) -> T {
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let (owner, storage) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(module, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let (inventory, storage) = CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    action(&inventory, budget.storage())
}

fn anchors(module: &Module, expected: &[(usize, usize)]) {
    with_inventory(module, |inventory, floor| {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let proof = super::super::check(inventory, 1024, &mut budget).unwrap();
        assert!(proof.is_for(inventory));
        let actual: Vec<_> = proof
            .latest_stores
            .iter()
            .enumerate()
            .filter_map(|(read, store)| store.map(|store| (read, store)))
            .collect();
        assert_eq!(actual, expected);
        for &(read, store) in expected {
            let load = &inventory.operations()[read];
            let stored = &inventory.operations()[store];
            let OperationKind::Load {
                pointer: load_pointer,
                ..
            } = load.operation.kind
            else {
                panic!("actual Load ordinal")
            };
            let OperationKind::Store {
                pointer: store_pointer,
                value,
                ..
            } = stored.operation.kind
            else {
                panic!("actual Store ordinal")
            };
            assert_eq!(
                load.coordinate.block.function,
                stored.coordinate.block.function
            );
            let function = load.coordinate.block.function;
            let read_definition = index(inventory, function, load_pointer, &mut budget).unwrap();
            let store_definition = index(inventory, function, store_pointer, &mut budget).unwrap();
            let read_address = proof.definitions[read_definition].unwrap();
            let store_address = proof.definitions[store_definition].unwrap();
            assert_eq!(
                (
                    read_address.allocation,
                    read_address.start + read_address.offset
                ),
                (
                    store_address.allocation,
                    store_address.start + store_address.offset
                )
            );
            let value = index(inventory, function, value, &mut budget).unwrap();
            let definitions: Vec<_> = inventory
                .operations()
                .iter()
                .filter(|row| row.results.contains(&value))
                .collect();
            assert_eq!(definitions.len(), 1);
            assert_eq!(
                definitions[0].operation.kind,
                OperationKind::Constant(Constant::U32(99))
            );
        }
    });
}

fn refuses(module: &Module, detail: &'static str) {
    with_inventory(module, |inventory, floor| {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(super::super::check(inventory, 1024, &mut budget),
            Err(E::Unsupported { phase: "private", detail: actual }) if actual == detail));
    });
}

#[test]
fn exact_anchor_crosses_sparse_blocks_and_multiple_functions() {
    let mut input = split();
    anchors(&input, &[(3, 2)]);
    let mut second = input.functions[0].clone();
    second.id = "g".into();
    input.functions.push(second);
    anchors(&input, &[(3, 2), (7, 6)]);
}

#[test]
fn common_store_survives_diamonds_duplicate_edges_and_layout_permutations() {
    let mut input = module(vec![
        block(
            100,
            vec![allocation(), constant(), store()],
            conditional(200, 400),
        ),
        block(200, vec![], conditional(300, 300)),
        block(400, vec![], branch(300)),
        block(300, vec![load()], ret()),
    ]);
    for _ in 0..3 {
        anchors(&input, &[(3, 2)]);
        input.functions[0].body.as_mut().unwrap().blocks[1..].rotate_left(1);
    }
}

#[test]
fn both_switch_grammars_preserve_duplicate_successor_occurrences() {
    for integer in [false, true] {
        let mut input = split();
        let body = input.functions[0].body.as_mut().unwrap();
        body.blocks[0].operations.push(Operation::effect_free(
            ValueDef::new(ValueId(29), Type::Scalar(ScalarType::Index)),
            OperationKind::Constant(Constant::Index(0)),
        ));
        body.blocks[0].terminator = Some(if integer {
            Terminator::IntegerSwitch {
                selector: ValueId(20),
                cases: vec![
                    fe2o3_kernel_ir::IntegerSwitchCase {
                        value: Constant::U32(0),
                        target: BlockId(300),
                        arguments: vec![],
                    },
                    fe2o3_kernel_ir::IntegerSwitchCase {
                        value: Constant::U32(1),
                        target: BlockId(300),
                        arguments: vec![],
                    },
                ],
                default_target: BlockId(300),
                default_arguments: vec![],
            }
        } else {
            Terminator::Switch {
                selector: ValueId(29),
                cases: vec![
                    fe2o3_kernel_ir::SwitchCase {
                        value: 0,
                        target: BlockId(300),
                        arguments: vec![],
                    },
                    fe2o3_kernel_ir::SwitchCase {
                        value: 1,
                        target: BlockId(300),
                        arguments: vec![],
                    },
                ],
                default_target: BlockId(300),
                default_arguments: vec![],
            }
        });
        anchors(&input, &[(4, 2)]);
    }
}

#[test]
fn direct_gep_aliases_join_by_allocation_cell_and_independent_cells_stay_distinct() {
    for separate in [false, true] {
        let mut input = split();
        let body = input.functions[0].body.as_mut().unwrap();
        let pointer = body.blocks[0].operations[0].results[0].ty.clone();
        let mut alloc = allocation();
        let OperationKind::Alloca { count, .. } = &mut alloc.kind else {
            unreachable!()
        };
        *count = Some(ValueId(30));
        let index_constant = |id, value| {
            Operation::effect_free(
                ValueDef::new(ValueId(id), Type::Scalar(ScalarType::Index)),
                OperationKind::Constant(Constant::Index(value)),
            )
        };
        body.blocks[0].operations = vec![
            index_constant(30, 2),
            alloc,
            constant(),
            index_constant(31, 0),
            index_constant(32, u64::from(separate)),
            Operation::effect_free(
                ValueDef::new(ValueId(11), pointer.clone()),
                OperationKind::GetElementPointer {
                    base: ValueId(10),
                    offset: ValueId(31),
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(12), pointer),
                OperationKind::GetElementPointer {
                    base: ValueId(10),
                    offset: ValueId(32),
                },
            ),
            store(),
        ];
        let mut second_store = store();
        let OperationKind::Store { pointer, .. } = &mut second_store.kind else {
            unreachable!()
        };
        *pointer = ValueId(12);
        if separate {
            body.blocks[0].operations.push(second_store);
        }
        let OperationKind::Load { pointer, .. } = &mut body.blocks[1].operations[0].kind else {
            unreachable!()
        };
        *pointer = ValueId(12);
        anchors(&input, if separate { &[(9, 8)] } else { &[(8, 7)] });
    }
}

#[test]
fn shared_anchor_survives_reachable_loop_and_irreducible_scc() {
    let looped = module(vec![
        block(100, vec![allocation(), constant(), store()], branch(200)),
        block(200, vec![load()], conditional(200, 300)),
        block(300, vec![], ret()),
    ]);
    anchors(&looped, &[(3, 2)]);
    let irreducible = module(vec![
        block(
            100,
            vec![allocation(), constant(), store()],
            conditional(200, 300),
        ),
        block(200, vec![load()], conditional(300, 400)),
        block(300, vec![], conditional(200, 400)),
        block(400, vec![], ret()),
    ]);
    anchors(&irreducible, &[(3, 2)]);
}

#[test]
fn equal_value_stores_and_late_bypass_do_not_form_a_unique_anchor() {
    for left_store in [false, true] {
        let input = module(vec![
            block(100, vec![allocation(), constant()], conditional(200, 400)),
            block(200, vec![store()], branch(300)),
            block(300, vec![load()], ret()),
            block(
                400,
                if left_store { vec![store()] } else { vec![] },
                branch(500),
            ),
            block(500, vec![], branch(300)),
        ]);
        refuses(&input, "Load requires one exact reaching Store");
    }
}

#[test]
fn backedge_does_not_initialize_first_iteration_or_unreachable_cycle() {
    let input = module(vec![
        block(100, vec![allocation(), constant()], branch(200)),
        block(200, vec![load()], branch(300)),
        block(300, vec![store()], branch(200)),
    ]);
    refuses(&input, "Load requires one exact reaching Store");
    let entry_backedge = module(vec![
        block(100, vec![allocation(), constant(), load()], branch(200)),
        block(200, vec![store()], branch(100)),
    ]);
    refuses(&entry_backedge, "Load requires one exact reaching Store");
    // V12 correctly disallows cross-block definitions inside an unreachable
    // SCC. Keep all definitions local and put the Store after its first read.
    let dead = module(vec![
        block(100, vec![], ret()),
        block(
            200,
            vec![allocation(), constant(), load(), store()],
            branch(200),
        ),
    ]);
    refuses(&dead, "cross-block Load is structurally reachable");
}

#[test]
fn reexecuted_allocation_kills_previous_iteration_store() {
    let input = module(vec![
        block(100, vec![constant()], branch(200)),
        block(200, vec![allocation()], branch(300)),
        block(300, vec![load(), store()], branch(200)),
    ]);
    refuses(&input, "Load requires one exact reaching Store");
}

#[test]
fn fallback_reconciles_local_anchors_and_preserves_dead_local_control() {
    let mut input = split();
    input.functions[0].body.as_mut().unwrap().blocks[1]
        .operations
        .insert(0, store());
    // Add another cross-block read to force fallback while the first read has
    // an exact local anchor. Both must retain their own actual Store ordinal.
    let body = input.functions[0].body.as_mut().unwrap();
    body.blocks[1].terminator = Some(branch(400));
    let mut read = load();
    read.results[0].id = ValueId(41);
    body.blocks.push(block(400, vec![read], ret()));
    let mut alloc = allocation();
    alloc.results[0].id = ValueId(71);
    let mut write = store();
    let OperationKind::Store { pointer, value, .. } = &mut write.kind else {
        unreachable!()
    };
    *pointer = ValueId(71);
    *value = ValueId(81);
    let mut read = load();
    read.results[0].id = ValueId(91);
    let OperationKind::Load { pointer, .. } = &mut read.kind else {
        unreachable!()
    };
    *pointer = ValueId(71);
    // Do not transport any definition between unreachable blocks.
    let value = Operation::effect_free(
        ValueDef::new(ValueId(81), Type::Scalar(ScalarType::U32)),
        OperationKind::Constant(Constant::U32(99)),
    );
    body.blocks
        .push(block(500, vec![alloc, value, write, read], ret()));
    anchors(&input, &[(4, 3), (5, 3), (9, 8)]);
}

#[test]
fn volatile_access_is_still_refused_before_dataflow() {
    let mut input = split();
    let body = input.functions[0].body.as_mut().unwrap();
    let OperationKind::Load { access, .. } = &mut body.blocks[1].operations[0].kind else {
        unreachable!()
    };
    access.volatile = true;
    refuses(&input, "one ordinary nonvolatile memory effect");
}

#[test]
fn transfer_reset_kills_its_whole_extent_without_erasing_other_cells() {
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let events = [
        Event::Reset {
            start: 8,
            length: 2,
        },
        Event::Read(8),
        Event::Read(9),
        Event::Read(10),
    ];
    let mut state = [Some(11), Some(12), Some(13)];
    let mut reads = vec![];
    transfer(
        &events,
        0..events.len(),
        8,
        &mut state,
        &mut budget,
        |ordinal, anchor, _| {
            reads.push((ordinal, anchor));
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(state, [None, None, Some(13)]);
    assert_eq!(reads, vec![(1, None), (2, None), (3, Some(13))]);
}

#[test]
fn ring_queue_has_fixed_capacity_checked_indices_and_exact_fifo() {
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let mut queue = Queue::new(2, &mut budget).unwrap();
    queue.push(1, &mut budget).unwrap();
    queue.push(1, &mut budget).unwrap();
    queue.push(0, &mut budget).unwrap();
    assert_eq!(queue.length, 2);
    assert_eq!(queue.pop(&mut budget).unwrap(), Some(1));
    queue.push(1, &mut budget).unwrap();
    assert_eq!(queue.pop(&mut budget).unwrap(), Some(0));
    assert_eq!(queue.pop(&mut budget).unwrap(), Some(1));
    assert_eq!(queue.pop(&mut budget).unwrap(), None);
    assert!(matches!(
        queue.push(2, &mut budget),
        Err(E::Resource(AssertOriginResourceV1::Arithmetic))
    ));
    assert!(matches!(
        row_start(usize::MAX, 2),
        Err(E::Resource(AssertOriginResourceV1::Arithmetic))
    ));
    queue.reset(&mut budget).unwrap();
    assert_eq!(queue.length, 0);
    queue.length = queue.rows.len();
    assert!(matches!(
        queue.push(0, &mut budget),
        Err(E::Resource(AssertOriginResourceV1::Arithmetic))
    ));
    assert_eq!(queue.length, 2);
    assert!(queue.queued.iter().all(|queued| !queued));
}
