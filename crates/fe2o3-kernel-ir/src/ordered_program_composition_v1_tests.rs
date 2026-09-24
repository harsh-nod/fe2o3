use super::*;
use crate::*;
#[path = "../tests/fixtures/ordered_composition_v1.rs"]
mod fixture;
use crate as ordered_composition_fixture_ir;

fn canonical(module: &Module) -> VerifiedCanonicalKernelIrModuleV17 {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    VerifiedCanonicalKernelIrModuleV17::from_module_ref_with_verification_budget_v17(
        module,
        &mut budget,
    )
    .unwrap()
    .0
}
fn compose(
    module: &Module,
) -> Result<VerifiedOrderedProgramCompositionV1, OrderedProgramCompositionErrorV1> {
    let owner = canonical(module);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    VerifiedOrderedProgramCompositionV1::try_from_canonical_v17(owner, &mut budget).map(|p| p.0)
}
fn body(module: &mut Module, function: usize) -> &mut FunctionBody {
    module.functions[function].body.as_mut().unwrap()
}

#[test]
fn singleton_and_multiple_direct_programs_keep_exact_canonical_bytes() {
    for count in [1, 2, 8] {
        for steps in [1, 3, 8, 16] {
            let module = fixture::module(count, &[], &[], steps);
            let source = encode_module_v17(&module).unwrap();
            let owner = compose(&module).unwrap();
            assert_eq!(owner.canonical().canonical_bytes(), source);
            assert_eq!(owner.definitions().len(), count);
            assert_eq!(owner.occurrences().len(), count);
            assert_eq!(
                owner.expanded_instruction_count(),
                count * usize::from(steps)
            );
            assert!(owner.calls().is_empty());
            assert!(owner.helpers().is_empty());
        }
    }
}
#[test]
fn one_shared_helper_called_twice_has_one_definition_and_two_paths() {
    let owner = compose(&fixture::module(0, &[1], &[0, 0], 8)).unwrap();
    assert_eq!(owner.definitions().len(), 1);
    assert_eq!(owner.helpers().len(), 1);
    assert_eq!(owner.calls().len(), 2);
    assert_eq!(owner.occurrences().len(), 2);
    assert_eq!(
        owner.occurrences()[0].definition(),
        owner.occurrences()[1].definition()
    );
    assert_ne!(
        owner.occurrences()[0].incoming_call(),
        owner.occurrences()[1].incoming_call()
    );
    assert_eq!(owner.expanded_instruction_count(), 16);
}
#[test]
fn two_helpers_and_direct_region_keep_distinct_function_coordinates() {
    let mut module = fixture::module(1, &[1, 2], &[0, 1, 0], 3);
    module.functions.rotate_left(1);
    let owner = compose(&module).unwrap();
    assert_eq!(owner.root_function_ordinal(), 2);
    assert_eq!(owner.definitions().len(), 4);
    assert_eq!(owner.occurrences().len(), 5);
    assert!(
        owner
            .definitions()
            .iter()
            .all(|d| d.site().block_ordinal() == 0 && d.site().block().0 != 0)
    );
    let encoded = encode_module_v17(&module).unwrap();
    assert_eq!(decode_module_v17(&encoded).unwrap(), module);
}
#[test]
fn scalar_only_helper_is_retained_and_not_fabricated_as_region() {
    let owner = compose(&fixture::module(1, &[0], &[0, 0], 1)).unwrap();
    assert_eq!(owner.calls().len(), 2);
    assert_eq!(owner.helpers().len(), 1);
    assert_eq!(owner.occurrences().len(), 1);
    assert_eq!(owner.definitions().len(), 1);
}
#[test]
fn expanded_limit_counts_shared_helper_occurrences_not_only_definitions() {
    assert_eq!(
        compose(&fixture::module(0, &[5], &[0, 0], 1)),
        Err(OrderedProgramCompositionErrorV1::OccurrenceLimit)
    );
    assert!(compose(&fixture::module(0, &[4], &[0, 0], 16)).is_ok());
}
#[test]
fn static_and_call_limits_and_unreferenced_helpers_refuse() {
    assert_eq!(
        compose(&fixture::module(9, &[], &[], 1)),
        Err(OrderedProgramCompositionErrorV1::DefinitionLimit)
    );
    assert_eq!(
        compose(&fixture::module(1, &[0], &[0; 9], 1)),
        Err(OrderedProgramCompositionErrorV1::CallLimit)
    );
    assert_eq!(
        compose(&fixture::module(1, &[0], &[], 1)),
        Err(OrderedProgramCompositionErrorV1::UnreferencedHelper)
    );
    assert_eq!(
        compose(&fixture::module(0, &[0], &[0], 1)),
        Err(OrderedProgramCompositionErrorV1::MissingProgram)
    );
}
#[test]
fn changed_target_wave_and_workgroup_refuse() {
    let mut module = fixture::module(1, &[], &[], 1);
    module.kernels[0].workgroup_size = Some(WorkgroupSize::new(128, 1, 1));
    assert_eq!(
        compose(&module),
        Err(OrderedProgramCompositionErrorV1::Launch)
    );
    let mut module = fixture::module(1, &[], &[], 1);
    module
        .required_capabilities
        .insert(TargetCapability::WaveWidth(WaveWidth::Wave32));
    // Conflicting declared wave widths are refused by the prior V17 owner.
    assert!(verify_module(&module).is_err());
    let mut module = fixture::module(1, &[], &[], 1);
    module.functions[0]
        .required_capabilities
        .insert(TargetCapability::Extension {
            namespace: AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE.into(),
            name: "gfx942:xnack+".into(),
        });
    assert_eq!(
        compose(&module),
        Err(OrderedProgramCompositionErrorV1::TargetCapabilities)
    );
}
#[test]
fn helper_nested_call_and_memory_refuse() {
    let mut module = fixture::module(0, &[1, 0], &[0, 1], 1);
    body(&mut module, 1).blocks[0]
        .operations
        .push(Operation::effect_free(
            ValueDef::new(ValueId(101), Type::Scalar(ScalarType::U32)),
            OperationKind::Call {
                callee: FunctionId::new("helper_1"),
                arguments: vec![ValueId(10), ValueId(11), ValueId(12)],
            },
        ));
    assert_eq!(
        compose(&module),
        Err(OrderedProgramCompositionErrorV1::HelperEffect)
    );
    let mut module = fixture::module(0, &[1], &[0], 1);
    body(&mut module, 1).blocks[0]
        .operations
        .push(Operation::effect_free(
            ValueDef::new(
                ValueId(102),
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
        ));
    assert_eq!(
        compose(&module),
        Err(OrderedProgramCompositionErrorV1::HelperSignature)
    );
}
#[test]
fn conditional_root_region_is_not_full_prefix() {
    let mut module = fixture::module(1, &[], &[], 1);
    let root = body(&mut module, 0);
    let original = root.blocks.remove(0);
    let mut entry = BasicBlock::new(BlockId(5));
    entry.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(50), Type::BOOL),
        OperationKind::Constant(Constant::Bool(true)),
    ));
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(50),
        then_target: BlockId(40),
        then_arguments: vec![],
        else_target: BlockId(99),
        else_arguments: vec![],
    });
    let mut tail = BasicBlock::new(BlockId(99));
    tail.terminator = Some(Terminator::Return { values: vec![] });
    root.blocks = vec![entry, original, tail];
    assert_eq!(
        compose(&module),
        Err(OrderedProgramCompositionErrorV1::PrefixControlFlow)
    );
}
#[test]
fn unconditional_helper_transport_and_safe_constant_shift_are_accepted() {
    let mut module = fixture::module(0, &[1], &[0], 1);
    let helper = body(&mut module, 1);
    let old = helper.blocks[0].operations.remove(0);
    helper.blocks[0].operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(90), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(31)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(91), Type::Scalar(ScalarType::U32)),
            OperationKind::Binary {
                op: BinaryOp::ShiftLeft,
                lhs: ValueId(10),
                rhs: ValueId(90),
            },
        ),
    ];
    helper.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(17),
        arguments: vec![ValueId(91)],
    });
    let mut second = BasicBlock::new(BlockId(17));
    second
        .parameters
        .push(ValueDef::new(ValueId(92), Type::Scalar(ScalarType::U32)));
    let OperationKind::Gfx942OrderedProgram(p) = old.kind else {
        panic!()
    };
    second.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(100), Type::Scalar(ScalarType::U32)),
        OperationKind::Gfx942OrderedProgram(
            Gfx942OrderedProgramV1::new(
                p.source(),
                p.registers(),
                [ValueId(92), ValueId(11), ValueId(12)],
                *p.program(),
            )
            .unwrap(),
        ),
    ));
    second.terminator = Some(Terminator::Return {
        values: vec![ValueId(100)],
    });
    helper.blocks.push(second);
    assert!(compose(&module).is_ok());
    if let OperationKind::Constant(Constant::U32(n)) =
        &mut body(&mut module, 1).blocks[0].operations[0].kind
    {
        *n = 32;
    }
    assert_eq!(
        compose(&module),
        Err(OrderedProgramCompositionErrorV1::HelperEffect)
    );
}
#[test]
fn helper_division_and_unproven_dynamic_shift_refuse() {
    for op in [
        BinaryOp::Divide,
        BinaryOp::Remainder,
        BinaryOp::ShiftLeft,
        BinaryOp::ShiftRight,
    ] {
        let mut module = fixture::module(0, &[1], &[0], 1);
        body(&mut module, 1).blocks[0]
            .operations
            .push(Operation::effect_free(
                ValueDef::new(ValueId(103), Type::Scalar(ScalarType::U32)),
                OperationKind::Binary {
                    op,
                    lhs: ValueId(10),
                    rhs: ValueId(11),
                },
            ));
        assert_eq!(
            compose(&module),
            Err(OrderedProgramCompositionErrorV1::HelperEffect)
        );
    }
}
#[test]
fn structural_keys_require_expected_canonical_identity() {
    let owner = compose(&fixture::module(0, &[1], &[0, 0], 1)).unwrap();
    let other = compose(&fixture::module(0, &[1], &[0, 0], 3)).unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
    let mut budget = Budget::new(&mut work, 0);
    let identity = owner.canonical().identity();
    assert!(
        owner
            .definition_operation(identity, owner.definitions()[0].key(), &mut budget)
            .is_ok()
    );
    assert!(
        owner
            .call_operation(identity, owner.calls()[0].key(), &mut budget)
            .is_ok()
    );
    assert!(
        owner
            .helper_function(identity, owner.helpers()[0].key(), &mut budget)
            .is_ok()
    );
    assert!(
        owner
            .occurrence(identity, owner.occurrences()[1].key(), &mut budget)
            .is_ok()
    );
    assert_eq!(
        owner.definition_operation(
            other.canonical().identity(),
            owner.definitions()[0].key(),
            &mut budget
        ),
        Err(OrderedProgramCompositionErrorV1::Identity)
    );
}
#[test]
fn resources_keep_exact_floor_peak_work_and_first_denial() {
    let module = fixture::module(0, &[4], &[0, 0], 16);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.charge_work(7).unwrap();
    budget.reserve_storage(19).unwrap();
    let (_owner, receipt) = VerifiedOrderedProgramCompositionV1::try_from_canonical_v17(
        canonical(&module),
        &mut budget,
    )
    .unwrap();
    let (used, peak) = (budget.work(), budget.peak_storage());
    assert_eq!(budget.storage(), 19);
    assert!(receipt.retained_storage() > 0);
    for (work_limit, storage_limit, success) in [
        (used, peak, true),
        (used - 1, peak, false),
        (used, peak - 1, false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(19).unwrap();
        let result = VerifiedOrderedProgramCompositionV1::try_from_canonical_v17(
            canonical(&module),
            &mut budget,
        );
        assert_eq!(result.is_ok(), success);
        assert_eq!(budget.storage(), 19);
        if success {
            assert_eq!(budget.work(), used);
            assert_eq!(budget.peak_storage(), peak);
        }
    }
}
#[test]
fn profile_error_restores_floor_without_refunding_work() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(23).unwrap();
    budget.charge_work(11).unwrap();
    let result = VerifiedOrderedProgramCompositionV1::try_from_canonical_v17(
        canonical(&fixture::module(0, &[5], &[0, 0], 1)),
        &mut budget,
    );
    assert_eq!(
        result,
        Err(OrderedProgramCompositionErrorV1::OccurrenceLimit)
    );
    assert_eq!(budget.storage(), 23);
    assert!(budget.work() > 11);
}

#[test]
fn helpers_require_exact_three_u32_arguments_and_single_return() {
    let mut module = fixture::module(0, &[1], &[0], 1);
    module.functions[1]
        .signature
        .parameters
        .push(Type::Scalar(ScalarType::U32));
    body(&mut module, 1).parameters.push(ValueId(13));
    let OperationKind::Call { arguments, .. } =
        &mut body(&mut module, 0).blocks[0].operations[0].kind
    else {
        panic!()
    };
    arguments.push(ValueId(2));
    assert_eq!(
        compose(&module),
        Err(OrderedProgramCompositionErrorV1::HelperSignature)
    );

    let mut module = fixture::module(0, &[1], &[0], 1);
    body(&mut module, 1).blocks[0].terminator = Some(Terminator::Unreachable);
    assert_eq!(
        compose(&module),
        Err(OrderedProgramCompositionErrorV1::HelperControlFlow)
    );
}
#[test]
fn helper_conditional_and_unreachable_extra_block_refuse() {
    let mut module = fixture::module(0, &[1], &[0], 1);
    let helper = body(&mut module, 1);
    helper.blocks[0].operations.push(Operation::effect_free(
        ValueDef::new(ValueId(110), Type::BOOL),
        OperationKind::Constant(Constant::Bool(true)),
    ));
    helper.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(110),
        then_target: BlockId(74),
        then_arguments: vec![],
        else_target: BlockId(75),
        else_arguments: vec![],
    });
    for id in [74, 75] {
        let mut block = BasicBlock::new(BlockId(id));
        block.terminator = Some(Terminator::Return {
            values: vec![ValueId(100)],
        });
        helper.blocks.push(block);
    }
    assert_eq!(
        compose(&module),
        Err(OrderedProgramCompositionErrorV1::HelperControlFlow)
    );
    let mut module = fixture::module(0, &[1], &[0], 1);
    let mut dead = BasicBlock::new(BlockId(74));
    dead.terminator = Some(Terminator::Unreachable);
    body(&mut module, 1).blocks.push(dead);
    assert_eq!(
        compose(&module),
        Err(OrderedProgramCompositionErrorV1::HelperControlFlow)
    );
}
#[test]
fn ordinary_conditional_tail_is_retained_without_claiming_memory_safety() {
    let mut module = fixture::module(1, &[], &[], 1);
    let root = body(&mut module, 0);
    let store = root.blocks[0].operations.pop().unwrap();
    root.blocks[0].operations.push(Operation::effect_free(
        ValueDef::new(ValueId(110), Type::BOOL),
        OperationKind::Constant(Constant::Bool(true)),
    ));
    root.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(110),
        then_target: BlockId(41),
        then_arguments: vec![],
        else_target: BlockId(42),
        else_arguments: vec![],
    });
    let mut then_block = BasicBlock::new(BlockId(41));
    then_block.operations.push(store);
    then_block.terminator = Some(Terminator::Return { values: vec![] });
    let mut else_block = BasicBlock::new(BlockId(42));
    else_block.terminator = Some(Terminator::Return { values: vec![] });
    root.blocks.extend([then_block, else_block]);
    let bytes = encode_module_v17(&module).unwrap();
    let owner = compose(&module).unwrap();
    assert_eq!(owner.canonical().canonical_bytes(), bytes);
    assert_eq!(owner.occurrences().len(), 1);
}
#[test]
fn wrapping_helper_arithmetic_retains_both_checked_results() {
    for op in [
        CheckedBinaryOperator::Add,
        CheckedBinaryOperator::Subtract,
        CheckedBinaryOperator::Multiply,
    ] {
        let mut module = fixture::module(0, &[1], &[0], 1);
        body(&mut module, 1).blocks[0].operations.insert(
            0,
            Operation::new(
                vec![
                    ValueDef::new(ValueId(90), Type::Scalar(ScalarType::U32)),
                    ValueDef::new(ValueId(91), Type::BOOL),
                ],
                OperationKind::Binary {
                    op: BinaryOp::Checked(op),
                    lhs: ValueId(10),
                    rhs: ValueId(11),
                },
            ),
        );
        // Exact checked result feeds the authored region; the overflow SSA stays in the module.
        body(&mut module, 1).blocks[0].operations[1] = fixture::region(
            [ValueId(90), ValueId(11), ValueId(12)],
            ValueId(100),
            1,
            0,
            1,
        );
        let owner = compose(&module).unwrap();
        let helper = &owner.canonical().module().functions[1];
        assert_eq!(
            helper.body.as_ref().unwrap().blocks[0].operations[0]
                .results
                .len(),
            2
        );
    }
    for op in [BinaryOp::Add, BinaryOp::Subtract, BinaryOp::Multiply] {
        let mut module = fixture::module(0, &[1], &[0], 1);
        body(&mut module, 1).blocks[0]
            .operations
            .push(Operation::effect_free(
                ValueDef::new(ValueId(103), Type::Scalar(ScalarType::U32)),
                OperationKind::Binary {
                    op,
                    lhs: ValueId(10),
                    rhs: ValueId(11),
                },
            ));
        assert_eq!(
            compose(&module),
            Err(OrderedProgramCompositionErrorV1::HelperEffect)
        );
    }
}
#[test]
fn unknown_capability_and_additional_function_roles_stay_closed() {
    let mut module = fixture::module(0, &[1], &[0], 1);
    module.functions[1]
        .required_capabilities
        .insert(TargetCapability::Extension {
            namespace: "unreviewed".into(),
            name: "helper_effect".into(),
        });
    assert_eq!(
        compose(&module),
        Err(OrderedProgramCompositionErrorV1::TargetCapabilities)
    );
    assert_eq!(
        compose(&fixture::module(1, &[0, 0, 0], &[0, 1, 2], 1)),
        Err(OrderedProgramCompositionErrorV1::Root)
    );
}
#[test]
fn one_owned_ledger_retains_canonical_and_roster_receipts_and_denial_history() {
    let module = fixture::module(1, &[1], &[0, 0], 3);
    let mut ledger = CanonicalKernelIrOwnedVerificationResourceBudgetV1::new(
        CanonicalKernelIrWorkBudgetV1::new(100_000_000),
        64 * 1024 * 1024,
    );
    ledger.with_budget(|budget| {
        budget.reserve_storage(29).unwrap();
        budget.charge_work(17).unwrap();
        assert!(budget.reserve_storage(usize::MAX).is_err());
        assert!(budget.charge_work(usize::MAX).is_err());
    });
    let first_work_denial = ledger.failed_work();
    let first_storage_denial = ledger.failed_storage();
    let (canonical, canonical_storage) = ledger
        .with_budget(|budget| {
            VerifiedCanonicalKernelIrModuleV17::from_module_ref_with_verification_budget_v17(
                &module, budget,
            )
        })
        .unwrap();
    assert_eq!(ledger.storage(), 29);
    ledger
        .with_budget(|budget| budget.reserve_storage(canonical_storage.retained_storage()))
        .unwrap();
    let canonical_floor = ledger.storage();
    let prior_work = ledger.work();
    let (owner, extra) = ledger
        .with_budget(|budget| {
            VerifiedOrderedProgramCompositionV1::try_from_canonical_v17(canonical, budget)
        })
        .unwrap();
    assert_eq!(ledger.storage(), canonical_floor);
    assert!(ledger.work() > prior_work);
    assert_eq!(ledger.failed_work(), first_work_denial);
    assert_eq!(ledger.failed_storage(), first_storage_denial);
    ledger
        .with_budget(|budget| budget.reserve_storage(extra.retained_storage()))
        .unwrap();
    assert_eq!(ledger.storage(), canonical_floor + extra.retained_storage());
    drop(owner);
    ledger.with_budget(|budget| {
        budget.release_storage(extra.retained_storage()).unwrap();
        budget
            .release_storage(canonical_storage.retained_storage())
            .unwrap();
    });
    assert_eq!(ledger.storage(), 29);
}

#[path = "formal_memory_obligations/ordered_composition_v1_tests.rs"]
mod formal_composition_v1;
