use super::physical_cfg::{Event, row_start, transfer};
use super::queue::CanonicalKirPrivateDataflowQueueV1 as Queue;
use super::*;
use fe2o3_kernel_ir::{
    AccessMode, BasicBlock, BlockId, Function, MemoryAccess, Module, Operation, ScalarType,
    Signature, Terminator, ValueDef,
};
type AssertOriginBudgetV1<'w> = Budget<'w>;
type AssertOriginResourceV1 = Resource;
type E = Error;

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

pub(super) fn local() -> Module {
    let mut input = split();
    let body = input.functions[0].body.as_mut().unwrap();
    let mut second = body.blocks.pop().unwrap();
    body.blocks[0].operations.append(&mut second.operations);
    body.blocks[0].terminator = second.terminator;
    input
}

#[test]
fn neutral_local_and_cross_block_have_independently_expected_anchors() {
    for input in [local(), split()] {
        with_inventory(&input, |inventory, floor| {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            let (proof, receipt) = check_canonical_kir_private_memory_v1(
                inventory,
                CanonicalKirPrivateMemoryLimitsV1 { max_cells: 1 },
                &mut budget,
            )
            .unwrap();
            assert_eq!(budget.storage(), floor);
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            assert!(proof.is_for(inventory));
            assert!(std::ptr::eq(proof.inventory(), inventory));
            assert_eq!(proof.latest_stores(), &[None, None, None, Some(2)]);
            assert!(proof.definition(1));
            assert!(!proof.definition(0));
            assert!(!proof.definition(usize::MAX));
            assert!(!proof.operation(usize::MAX));
            assert!(proof.operation(0) && proof.operation(2) && proof.operation(3));
            assert!(!proof.operation(1));
            let address = proof.address(1).unwrap();
            assert_eq!(
                (
                    address.allocation(),
                    address.start(),
                    address.length(),
                    address.offset(),
                    address.alignment(),
                    address.stride()
                ),
                (0, 0, 1, 0, 4, 4),
            );
            assert!(proof.address(usize::MAX).is_none());
            assert!(!proof.grants_authority());
            drop(proof);
            budget.release_storage(receipt.retained_storage()).unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn neutral_multiple_functions_keep_distinct_dense_cell_anchors() {
    let mut input = split();
    let mut second = input.functions[0].clone();
    second.id = "g".into();
    input.functions.push(second);
    with_inventory(&input, |inventory, floor| {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let (proof, receipt) = check_canonical_kir_private_memory_v1(
            inventory,
            CanonicalKirPrivateMemoryLimitsV1 { max_cells: 2 },
            &mut budget,
        )
        .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert_eq!(
            proof.latest_stores(),
            &[None, None, None, Some(2), None, None, None, Some(6)],
        );
        assert_eq!(proof.address(1).unwrap().start(), 0);
        assert_eq!(proof.address(5).unwrap().start(), 1);
        drop(proof);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

fn both_refuse(input: &Module, expected: &'static str) {
    with_inventory(input, |inventory, floor| {
        for scoped in [false, true] {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            let error = if scoped {
                check_canonical_kir_private_memory_v1(
                    inventory,
                    CanonicalKirPrivateMemoryLimitsV1 { max_cells: 2 },
                    &mut budget,
                )
                .err()
                .expect("physical refusal")
            } else {
                check_canonical_kir_private_memory_retaining_scratch_v1(inventory, 2, &mut budget)
                    .err()
                    .expect("legacy refusal")
            };
            assert!(matches!(error,
                Error::Unsupported { phase: "private", detail } if detail == expected));
            if scoped {
                assert_eq!(budget.storage(), floor);
            }
            budget.release_storage(budget.storage() - floor).unwrap();
        }
    });
}

#[test]
fn neutral_refuses_missing_and_distinct_reaching_stores() {
    let mut missing = split();
    missing.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .pop();
    both_refuse(&missing, "Load requires one exact reaching Store");
    let input = module(vec![
        block(100, vec![allocation(), constant()], conditional(200, 300)),
        block(200, vec![store()], branch(400)),
        block(300, vec![store()], branch(400)),
        block(400, vec![load()], ret()),
    ]);
    both_refuse(&input, "Load requires one exact reaching Store");
}

#[test]
fn neutral_ordinary_access_alignment_and_pointer_escape_refusals_are_unchanged() {
    let mut input = local();
    let OperationKind::Load { access, .. } =
        &mut input.functions[0].body.as_mut().unwrap().blocks[0].operations[3].kind
    else {
        unreachable!()
    };
    access.volatile = true;
    both_refuse(&input, "one ordinary nonvolatile memory effect");
    let OperationKind::Load { access, .. } =
        &mut input.functions[0].body.as_mut().unwrap().blocks[0].operations[3].kind
    else {
        unreachable!()
    };
    access.volatile = false;
    access.alignment = 8;
    both_refuse(
        &input,
        "access alignment follows allocation and element offset",
    );
    let mut escaped = local();
    escaped.functions[0].signature.results = vec![Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    )];
    escaped.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Return {
        values: vec![ValueId(10)],
    });
    both_refuse(&escaped, "no private pointer control transport");
}

#[test]
fn neutral_exact_subject_distinguishes_equal_byte_inventory_instances() {
    let input = split();
    with_inventory(&input, |inventory, floor| {
        let mut preparation = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut prep = Budget::new(&mut preparation, STORAGE);
        prep.reserve_storage(floor).unwrap();
        let (other, other_storage) =
            CanonicalKirInventoryV1::derive(inventory.owner(), &mut prep).unwrap();
        prep.reserve_storage(other_storage.retained_storage())
            .unwrap();
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget
            .reserve_storage(floor + other_storage.retained_storage())
            .unwrap();
        let (proof, receipt) = check_canonical_kir_private_memory_v1(
            inventory,
            CanonicalKirPrivateMemoryLimitsV1 { max_cells: 1 },
            &mut budget,
        )
        .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert!(std::ptr::eq(inventory.owner(), other.owner()));
        assert!(!proof.is_for(&other));
        drop(proof);
        budget.release_storage(receipt.retained_storage()).unwrap();
    });
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
