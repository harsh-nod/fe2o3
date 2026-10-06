use super::*;
use fe2o3_kernel_analysis::CanonicalKirInventoryV18;
use fe2o3_kernel_ir::{
    BasicBlock, BinaryOp, BlockId, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKirFunctionCoordinateV1, Function, Module,
    Operation, OperationKind, ScalarType, Signature, StorageLayoutLimitsV1, SwitchCase, Terminator,
    Type, ValueDef, ValueId, VerifiedCanonicalKernelIrModuleV18,
};
use fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1;
use fe2o3_pliron::{ProductionSemanticSsaLimitsV1, plan_semantic_function_ssa_with_module_v1};

#[derive(Clone, Copy)]
enum Change {
    None,
    Operator,
    Phi,
    Branch,
    MissingAssignment,
    InputLocator,
}

fn module(change: Change) -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let operation = |id, op, lhs, rhs| {
        Operation::effect_free(
            ValueDef::new(ValueId(id), scalar.clone()),
            OperationKind::Binary {
                op,
                lhs: ValueId(lhs),
                rhs: ValueId(rhs),
            },
        )
    };
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(Terminator::Switch {
        selector: ValueId(0),
        cases: vec![SwitchCase {
            value: 0,
            target: BlockId(if matches!(change, Change::Branch) {
                2
            } else {
                1
            }),
            arguments: vec![],
        }],
        default_target: BlockId(if matches!(change, Change::Branch) {
            1
        } else {
            2
        }),
        default_arguments: vec![],
    });
    let mut left = BasicBlock::new(BlockId(1));
    left.operations.push(operation(
        2,
        if matches!(change, Change::Operator) {
            BinaryOp::BitAnd
        } else {
            BinaryOp::BitXor
        },
        0,
        1,
    ));
    left.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![ValueId(if matches!(change, Change::Phi) { 0 } else { 2 })],
    });
    let mut right = BasicBlock::new(BlockId(2));
    right.operations.push(operation(3, BinaryOp::BitOr, 0, 1));
    right.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![ValueId(3)],
    });
    let mut merge = BasicBlock::new(BlockId(3));
    merge
        .parameters
        .push(ValueDef::new(ValueId(4), scalar.clone()));
    merge.operations.push(operation(5, BinaryOp::BitOr, 4, 0));
    merge.terminator = Some(Terminator::Return {
        values: vec![ValueId(5)],
    });
    let mut module = Module::new("original-mir-control-model");
    module.functions.push(Function::internal_helper(
        "diamond",
        Signature::new(vec![scalar.clone(), scalar.clone()], vec![scalar]),
        vec![ValueId(0), ValueId(1)],
        vec![entry, left, right, merge],
    ));
    module
}

fn run(change: Change, work: usize, storage: usize) -> (Result<()>, usize, usize) {
    let (types, original) = super::super::control::tests::fixture(false, false);
    let plan = plan_semantic_function_ssa_with_module_v1(
        SemanticFunctionIdV1::from_index(0),
        &original,
        &types,
        &[],
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let module = module(change);
    let mut work = Work::new(work);
    let mut budget = Budget::new(&mut work, storage);
    let result = (|| {
        let limits = StorageLayoutLimitsV1 {
            rows: 1,
            edges: 1,
            containment_depth: 1,
            object_bytes: 16,
        };
        let (owner, receipt) =
            VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
                &module,
                limits,
                &mut budget,
            )
            .map_err(|_| Error::Statement("control test canonical admission"))?;
        budget.reserve_storage(receipt.retained_storage())?;
        let (inventory, receipt) = CanonicalKirInventoryV18::derive_v18(&owner, &mut budget)?;
        budget.reserve_storage(receipt.retained_storage())?;
        budget.reserve_storage(super::super::super::super::SOURCE_LIMIT)?;
        let mut writer = Writer::new(&mut budget)?;
        let source = SourceControl::derive(&types, &original, plan.plan(), &mut writer)?;
        let mut bindings = vector(source.blocks.len(), &mut writer)?;
        for (block, source) in source.blocks.iter().enumerate() {
            let source = source.as_ref().unwrap();
            let mut live = vector(source.live.len(), &mut writer)?;
            for row in &source.live {
                let id = match row.local {
                    1 => {
                        if matches!(change, Change::InputLocator) {
                            1
                        } else {
                            0
                        }
                    }
                    2 => 1,
                    3 if block == 3 => 4,
                    _ => panic!("unexpected original live local"),
                };
                let definition = inventory
                    .definition_index_for_value(
                        CanonicalKirFunctionCoordinateV1(0),
                        ValueId(id),
                        writer.budget,
                    )?
                    .unwrap();
                live.push((row.local, Some(definition)));
            }
            let mut assignments = vector(source.program.assignments.len(), &mut writer)?;
            for _ in &source.program.assignments {
                let id = match block {
                    1 => 2,
                    2 => 3,
                    3 => 5,
                    _ => panic!("original assignment block"),
                };
                let definition = inventory
                    .definition_index_for_value(
                        CanonicalKirFunctionCoordinateV1(0),
                        ValueId(id),
                        writer.budget,
                    )?
                    .unwrap();
                assignments.push(Some(definition));
            }
            if block == 1 && matches!(change, Change::MissingAssignment) {
                assignments.clear();
            }
            bindings.push(Some(BlockBindings {
                physical: CanonicalKirBlockCoordinateV1 {
                    function: CanonicalKirFunctionCoordinateV1(0),
                    block: block as u32,
                },
                live,
                assignments,
            }));
        }
        for block in 0..source.blocks.len() {
            let mut target = TargetBlock::derive(&inventory, block, &mut writer)?;
            let pair = BlockPair::check(
                &source,
                block,
                &bindings,
                &inventory,
                &mut target,
                &mut writer,
            )?;
            assert_eq!(
                pair.source_observations.len(),
                source.blocks[block]
                    .as_ref()
                    .unwrap()
                    .program
                    .assignments
                    .len()
            );
            assert_eq!(
                pair.source_observations.len(),
                pair.target_observations.len()
            );
            assert_eq!(
                pair.source_nodes.len(),
                source.blocks[block].as_ref().unwrap().program.nodes.len()
            );
        }
        Ok(())
    })();
    let work = budget.work();
    let peak = budget.peak_storage();
    budget.release_storage(budget.storage()).unwrap();
    assert_eq!(budget.storage(), 0);
    (result, work, peak)
}

#[test]
fn original_mir_control_pair_checks_all_actual_blocks_assignments_and_phi_arguments() {
    run(Change::None, 100_000_000, 64 * 1024 * 1024).0.unwrap();
}

#[test]
fn original_mir_control_pair_rejects_actual_operator_phi_branch_and_locator_changes() {
    for change in [
        Change::Operator,
        Change::Phi,
        Change::Branch,
        Change::MissingAssignment,
        Change::InputLocator,
    ] {
        assert!(matches!(
            run(change, 100_000_000, 64 * 1024 * 1024).0,
            Err(Error::Statement(
                "original MIR concrete CFG step differs from actual canonical operations"
            ))
        ));
        run(Change::None, 100_000_000, 64 * 1024 * 1024).0.unwrap();
    }
}

#[test]
fn original_mir_control_pair_has_exact_and_one_short_work_and_storage() {
    let measured = run(Change::None, 100_000_000, 64 * 1024 * 1024);
    measured.0.unwrap();
    let exact = run(Change::None, measured.1, measured.2);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2), (measured.1, measured.2));
    assert!(matches!(run(Change::None, measured.1 - 1, measured.2).0,
        Err(Error::Resource(Resource::Work(error))) if error.actual() == measured.1 && error.limit() == measured.1 - 1));
    assert!(matches!(run(Change::None, measured.1, measured.2 - 1).0,
        Err(Error::Resource(Resource::Storage(error))) if error.actual() == measured.2 && error.limit() == measured.2 - 1));
}
