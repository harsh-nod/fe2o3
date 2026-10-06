use super::super::tests::{noop, with_checked};
use super::*;
use crate::kir_bridge_v1::canonical_ranked_v1::private_profile::NativeCanonicalPrivateProjectionV1;
use fe2o3_kernel_ir::{
    AmdGpuDiagnosticOperation as Diagnostic, BasicBlock, BlockId,
    CanonicalKernelIrWorkBudgetV1 as Work, Constant, Function, Module, Operation, Signature,
    ValueDef, ValueId,
};

pub(crate) fn fixture(declaration: usize) -> Module {
    let mut module = private::tests::fixture();
    let body = module.functions[0].body.as_mut().unwrap();
    let entry = &mut body.blocks[0];
    entry.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(40), Type::BOOL),
            OperationKind::Constant(Constant::Bool(true)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(41), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(17)),
        ),
    ]);
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(40),
        then_target: BlockId(92),
        then_arguments: vec![ValueId(41)],
        else_target: BlockId(94),
        else_arguments: vec![],
    });
    let mut middle = BasicBlock::new(BlockId(92));
    middle
        .parameters
        .push(ValueDef::new(ValueId(42), Type::Scalar(ScalarType::U32)));
    middle.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(40),
        then_target: BlockId(94),
        then_arguments: vec![],
        else_target: BlockId(93),
        else_arguments: vec![ValueId(42)],
    });
    let mut success = BasicBlock::new(BlockId(93));
    success
        .parameters
        .push(ValueDef::new(ValueId(43), Type::Scalar(ScalarType::U32)));
    success.terminator = Some(Terminator::Return { values: vec![] });
    let mut failure = BasicBlock::new(BlockId(94));
    failure.operations.push(Diagnostic::Trap.operation(None));
    failure.terminator = Some(Terminator::Unreachable);
    body.blocks.extend([middle, success, failure]);
    module
        .functions
        .insert(declaration, Diagnostic::Trap.declaration());
    module
}

fn root(module: &mut Module) -> &mut fe2o3_kernel_ir::FunctionBody {
    module
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "zeta")
        .unwrap()
        .body
        .as_mut()
        .unwrap()
}

pub(crate) fn with_projection<T>(
    module: &Module,
    run: impl FnOnce(&mut NativeCanonicalPrivateProjectionV1<'_>, &mut Budget<'_>) -> Result<T, Failure>,
) -> Result<T, Failure> {
    with_checked(module, |checked, budget| {
        protected(budget, |budget| {
            let inventory = checked.inventory(budget)?;
            let terminals = CanonicalTrapPairsGraphFactsV1::derive(inventory, budget)?;
            let facts = private::CanonicalPrivateGraphFactsV1::derive_with_terminals(
                inventory, &terminals, budget,
            )?;
            let mut projection = NativeCanonicalPrivateProjectionV1::import(&facts, budget)?;
            run(&mut projection, budget)
        })
    })
}

#[test]
fn shared_sink_retains_every_actual_condition_polarity_and_edge_payload() {
    with_checked(&fixture(1), |checked, budget| {
        protected(budget, |budget| {
            let inventory = checked.inventory(budget)?;
            let facts = CanonicalTrapPairsGraphFactsV1::derive(inventory, budget)?;
            assert!(facts.belongs_to(inventory));
            assert_eq!(
                facts.definitions(),
                &[FunctionCoordinate(0), FunctionCoordinate(2)]
            );
            assert_eq!(facts.pairs.len(), 1);
            let pair = &facts.pairs[0];
            assert_eq!(pair.call().block.block, 3);
            assert_eq!(pair.declaration(), FunctionCoordinate(1));
            assert_eq!(pair.incoming_edges(), 0..2);
            let first = &facts.incoming[0];
            let second = &facts.incoming[1];
            assert!(first.success_when());
            assert!(!second.success_when());
            assert_eq!(first.condition().value, ValueId(40));
            assert_eq!(
                first.definition().coordinate,
                second.definition().coordinate
            );
            assert_eq!(first.success().arguments, &[ValueId(41)]);
            assert_eq!(second.success().arguments, &[ValueId(42)]);
            for incoming in &facts.incoming {
                assert!(incoming.failure().arguments.is_empty());
                assert_eq!(incoming.failure().target, pair.terminal_block());
                assert!(std::ptr::eq(
                    incoming.failure(),
                    &inventory.edges()[if incoming.success_when() { 1 } else { 2 }]
                ));
            }
            Ok(())
        })
        .unwrap();
    });
}

fn refused(module: &Module) {
    with_checked(module, |checked, budget| {
        let floor = budget.storage();
        let error =
            with_canonical_trap_policy_checks_v1(checked, budget, |_, _| -> Result<(), Failure> {
                panic!("invalid terminal shape reached callback")
            })
            .unwrap_err();
        assert!(matches!(
            error.failure(),
            Failure::PrivateRequirement {
                requirement: private::CanonicalPrivateRequirementV1::TerminalPairs,
                ..
            }
        ));
        assert_eq!(error.observation().work_upper_bound(), 0);
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn direct_entry_unconditional_side_entry_or_both_failure_edges_are_not_assertion_pairs() {
    let mut entry = fixture(2);
    root(&mut entry).blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(94),
        arguments: vec![],
    });
    // Keep the unconditional-entry case valid SSA: detached copies of the
    // old success chain cannot use the entry's definitions by dominance.
    root(&mut entry)
        .blocks
        .retain(|block| matches!(block.id, BlockId(91) | BlockId(94)));
    refused(&entry);
    let mut side_entry = fixture(2);
    root(&mut side_entry).blocks[2].terminator = Some(Terminator::Branch {
        target: BlockId(94),
        arguments: vec![],
    });
    refused(&side_entry);
    let mut duplicate = fixture(2);
    root(&mut duplicate).blocks[1].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(40),
        then_target: BlockId(94),
        then_arguments: vec![],
        else_target: BlockId(94),
        else_arguments: vec![],
    });
    root(&mut duplicate)
        .blocks
        .retain(|block| block.id != BlockId(93));
    refused(&duplicate);
    let mut lone = noop();
    lone.functions[0].body.as_mut().unwrap().blocks[0].operations =
        vec![Diagnostic::Trap.operation(None)];
    lone.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Unreachable);
    lone.functions.push(Diagnostic::Trap.declaration());
    refused(&lone);
    let mut detached = BasicBlock::new(BlockId(95));
    detached.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(51), Type::BOOL),
        OperationKind::Constant(Constant::Bool(true)),
    ));
    detached.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(51),
        then_target: BlockId(96),
        then_arguments: vec![],
        else_target: BlockId(91),
        else_arguments: vec![],
    });
    let mut success = BasicBlock::new(BlockId(96));
    success.terminator = Some(Terminator::Return { values: vec![] });
    lone.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .extend([detached, success]);
    // A detached incoming edge does not cover execution through function entry.
    refused(&lone);
}

#[test]
fn naked_unreachable_extra_sink_operations_and_other_diagnostics_fail_closed() {
    let mut naked = fixture(2);
    root(&mut naked).blocks[3].operations.clear();
    refused(&naked);
    let mut extra = fixture(2);
    root(&mut extra).blocks[3].operations.insert(
        0,
        Operation::effect_free(
            ValueDef::new(ValueId(50), Type::BOOL),
            OperationKind::Constant(Constant::Bool(false)),
        ),
    );
    refused(&extra);
    let mut other = fixture(2);
    root(&mut other).blocks[3].operations[0] = Diagnostic::DebugTrap.operation(None);
    other.functions[2] = Diagnostic::DebugTrap.declaration();
    refused(&other);
}

#[test]
fn unmatched_declaration_is_rejected_but_unused_exact_trap_is_identity_only() {
    let mut external = fixture(2);
    external.functions.push(Function::declaration(
        "unused",
        Signature::new(vec![], vec![]),
    ));
    refused(&external);
    let mut unused = noop();
    unused.functions.insert(0, Diagnostic::Trap.declaration());
    with_checked(&unused, |checked, budget| {
        with_canonical_trap_policy_checks_v1(checked, budget, |policies, budget| {
            assert_eq!(policies.module_function_count(budget)?, 2);
            assert_eq!(policies.definition_count(budget)?, 1);
            assert_eq!(
                policies.definition_coordinate(0, budget)?,
                FunctionCoordinate(1)
            );
            assert_eq!(policies.pair_count(budget)?, 0);
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn mismatched_reserved_signature_or_capability_is_refused_by_the_real_owner() {
    for change in 0..2 {
        let mut module = fixture(1);
        if change == 0 {
            module.functions[1].signature.parameters.push(Type::BOOL);
        } else {
            module.functions[1].required_capabilities.clear();
        }
        let mut work = Work::new(1 << 48);
        let mut budget = Budget::new(&mut work, 1 << 32);
        assert!(Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).is_err());
    }
}

#[test]
fn byte_identical_foreign_inventory_cannot_borrow_the_terminal_capability() {
    let module = fixture(1);
    with_checked(&module, |checked, budget| {
        protected(budget, |budget| {
            let inventory = checked.inventory(budget)?;
            let facts = CanonicalTrapPairsGraphFactsV1::derive(inventory, budget)?;
            let (foreign_owner, storage) =
                Owner::from_module_ref_with_verification_budget_v12(&module, budget)
                    .map_err(|_| Failure::Callback("foreign owner construction"))?;
            budget.reserve_storage(storage.retained_storage())?;
            let (foreign_inventory, storage) =
                Inventory::derive(&foreign_owner, budget).map_err(resources::inventory_error)?;
            budget.reserve_storage(storage.retained_storage())?;
            assert_eq!(
                inventory.owner().canonical().canonical_bytes(),
                foreign_owner.canonical().canonical_bytes()
            );
            let before = budget.work();
            let error = private::CanonicalPrivateGraphFactsV1::derive_with_terminals(
                &foreign_inventory,
                &facts,
                budget,
            )
            .err()
            .expect("same bytes are not the same borrowed inventory");
            assert!(matches!(error, Failure::ExactGraph));
            assert_eq!(budget.work(), before);
            drop(foreign_inventory);
            drop(foreign_owner);
            drop(facts);
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn source_predicate_truth_is_not_inferred_from_terminal_shape() {
    let mut module = fixture(0);
    root(&mut module).blocks[0].operations[1].kind = OperationKind::Constant(Constant::Bool(false));
    with_checked(&module, |checked, budget| {
        with_canonical_trap_policy_checks_v1(checked, budget, |policies, budget| {
            assert_eq!(policies.pair_count(budget)?, 1);
            assert_eq!(policies.pending_obligations().iter().count(), 19);
            assert!(!policies.ranked_verification_is_complete());
            assert!(!policies.grants_artifact_or_launch_authority());
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn graph_fact_header_and_descriptor_work_have_literal_exact_one_short_boundaries() {
    let bytes = size_of::<CheckedCanonicalTrapPoliciesV1<'_, '_>>()
        + size_of::<std::thread::Result<Result<(), Failure>>>()
        + size_of::<std::thread::Result<()>>();
    for (limit, success) in [(bytes, true), (bytes - 1, false)] {
        let mut work = Work::new(0);
        let mut budget = Budget::new(&mut work, limit);
        assert_eq!(
            resources::reserve_header::<()>(&mut budget).is_ok(),
            success
        );
        assert_eq!(budget.storage(), if success { bytes } else { 0 });
        assert_eq!(budget.work(), 0);
    }
    let callee = Diagnostic::Trap.intrinsic_function_id();
    let work_bound = 8 * (callee.as_str().len() + 2);
    for (limit, success) in [(work_bound, true), (work_bound - 1, false)] {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, 0);
        assert_eq!(resources::is_trap(&callee, &mut budget).is_ok(), success);
        assert_eq!(budget.work(), if success { work_bound } else { 0 });
        assert_eq!(budget.storage(), 0);
    }
}
