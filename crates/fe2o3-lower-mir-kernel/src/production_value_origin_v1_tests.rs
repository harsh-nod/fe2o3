use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1,
    Function as KirFunction, Module, ScalarType, Signature, Terminator, Type, ValueDef,
};

const LIMIT: usize = 10_000_000;

fn slice() -> Type {
    Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    )
}

fn block(id: u32, parameter: Option<u32>) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    if let Some(parameter) = parameter {
        block
            .parameters
            .push(ValueDef::new(ValueId(parameter), slice()));
    }
    block.terminator = Some(Terminator::Return { values: vec![] });
    block
}

fn branch(target: u32, argument: u32) -> Option<Terminator> {
    Some(Terminator::Branch {
        target: BlockId(target),
        arguments: vec![ValueId(argument)],
    })
}

fn conditional(a: u32, a_argument: u32, b: u32, b_argument: u32) -> Option<Terminator> {
    Some(Terminator::ConditionalBranch {
        condition: ValueId(99),
        then_target: BlockId(a),
        then_arguments: vec![ValueId(a_argument)],
        else_target: BlockId(b),
        else_arguments: vec![ValueId(b_argument)],
    })
}

fn inspect(blocks: Vec<BasicBlock>, expected: &[(u32, Option<u32>)]) {
    let mut module = Module::new("whole-value-origin");
    module.functions.push(KirFunction::internal_helper(
        "origin",
        Signature::new(vec![Type::BOOL, slice(), slice()], vec![]),
        vec![ValueId(99), ValueId(98), ValueId(97)],
        blocks,
    ));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(17).unwrap();
    let (owner, owner_storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            &module,
            &mut budget,
        )
        .unwrap();
    budget
        .reserve_storage(owner_storage.retained_storage())
        .unwrap();
    let (inventory, inventory_storage) =
        CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
    budget
        .reserve_storage(inventory_storage.retained_storage())
        .unwrap();
    let floor = budget.storage();
    let function = inventory.functions()[0].coordinate;
    for &(value, argument) in expected {
        let actual = resolve_whole_value_origin_v1(
            &inventory,
            &owner,
            function,
            ValueId(value),
            &mut budget,
        )
        .unwrap();
        assert_eq!(
            actual,
            argument.map(|argument| Definition::FunctionArgument { function, argument }),
            "value %{value}"
        );
        assert_eq!(budget.storage(), floor);
    }
    drop(inventory);
    budget
        .release_storage(inventory_storage.retained_storage())
        .unwrap();
    drop(owner);
    budget
        .release_storage(owner_storage.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), 17);
}

#[test]
fn whole_carrier_transport_accepts_grounded_loops() {
    let mut entry = block(77, None);
    entry.terminator = branch(18, 98);
    let mut header = block(18, Some(400));
    header.terminator = conditional(18, 400, 19, 400);
    inspect(
        vec![entry, header, block(19, Some(500))],
        &[(98, Some(1)), (400, Some(1)), (500, Some(1))],
    );
}

#[test]
fn every_duplicate_successor_occurrence_must_carry_the_same_slice() {
    for different in [false, true] {
        let mut entry = block(u32::MAX, None);
        entry.terminator = conditional(18, 98, 18, if different { 97 } else { 98 });
        inspect(
            vec![entry, block(18, Some(400))],
            &[(400, (!different).then_some(1))],
        );
    }
}

#[test]
fn an_ungrounded_cycle_does_not_manufacture_a_slice() {
    let mut left = block(18, Some(400));
    left.terminator = branch(19, 400);
    let mut right = block(19, Some(500));
    right.terminator = branch(18, 500);
    inspect(
        vec![block(77, None), left, right, block(20, Some(600))],
        &[(400, None), (500, None), (600, None)],
    );
}

#[test]
fn an_ungrounded_incoming_cycle_contaminates_a_grounded_join() {
    let mut entry = block(77, None);
    entry.terminator = branch(20, 98);
    let mut dormant = block(18, Some(400));
    dormant.terminator = conditional(18, 400, 20, 400);
    inspect(
        vec![entry, dormant, block(20, Some(600))],
        &[(98, Some(1)), (400, None), (600, None)],
    );
}

#[test]
fn entry_parameter_retains_unknown_initial_value_despite_a_grounded_backedge() {
    let mut entry = block(77, Some(400));
    entry.terminator = conditional(77, 98, 20, 400);
    inspect(
        vec![entry, block(20, Some(500))],
        &[(400, None), (500, None)],
    );
}

#[test]
fn irreducible_cycle_requires_every_entry_to_agree() {
    for different in [false, true] {
        let mut entry = block(77, None);
        entry.terminator = conditional(18, 98, 19, if different { 97 } else { 98 });
        let mut left = block(18, Some(400));
        left.terminator = conditional(19, 400, 20, 400);
        let mut right = block(19, Some(500));
        right.terminator = conditional(18, 500, 20, 500);
        let expected = (!different).then_some(1);
        inspect(
            vec![entry, left, right, block(20, Some(600))],
            &[(400, expected), (500, expected), (600, expected)],
        );
    }
}
