use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    Constant, ExecutionRoleV15 as Role, Function, Kernel, LaunchDomain, LaunchExtent, ScalarType,
    Signature, Terminator, WorkgroupSize,
};

#[path = "tile_scalar_sim_v158_tests.rs"]
mod simulation;

const LAYOUTS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 16,
    edges: 32,
    containment_depth: 8,
    object_bytes: 4096,
};

#[test]
fn tile_scalar_whole_graph_checks_complete_operation_spans_independently() {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let input = owner(&fixture(), &mut budget);
    for mutation in 0..5 {
        let mut output = prepare_owned_tile_scalar_v18(
            &input,
            &selection(ExecutionTileLayoutV1::Striped),
            LAYOUTS,
            &mut budget,
        )
        .unwrap();
        let retained = output.retained_storage();
        budget.reserve_storage(retained).unwrap();
        let spans = output.projections_v159();
        assert_eq!(
            spans
                .iter()
                .map(|row| (row.first, row.end))
                .collect::<Vec<_>>(),
            [
                (0, 1),
                (1, 2),
                (2, 40),
                (40, 78),
                (78, 79),
                (0, 6),
                (6, 7),
                (7, 8),
                (0, 0)
            ]
        );
        output.replay_against(&input, &mut budget).unwrap();
        match mutation {
            0 => output.projections[2].first += 1,
            1 => output.projections[2].end -= 1,
            2 => output.projections[8].end = 1,
            3 => output.projections[5].input.operation = 1,
            _ => {
                output.projections.pop();
            }
        }
        assert!(output.replay_against(&input, &mut budget).is_err());
        drop(output);
        budget.release_storage(retained).unwrap();
    }
}

fn execution(id: u32, role: Role, operation: Execution) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), Type::Execution(role)),
        Kind::Execution(operation),
    )
}

fn fixture() -> Module {
    let mut module = Module::new("tile-whole-graph");
    let mut entry = BasicBlock::new(BlockId(7));
    entry.operations = vec![
        execution(10, Role::Context, Execution::ContextIssue),
        execution(
            11,
            Role::Workgroup,
            Execution::WorkgroupDerive {
                context: ValueId(10),
            },
        ),
        execution(
            12,
            Role::MaskedTileU32 {
                lanes: 64,
                elements: 3,
            },
            Execution::MaskedTileLoadU32 {
                workgroup: ValueId(11),
                input: ValueId(0),
                base: ValueId(1),
                lanes: 64,
                elements: 3,
            },
        ),
        execution(
            14,
            Role::MaskedTileU32 {
                lanes: 64,
                elements: 3,
            },
            Execution::MaskedTileLoadU32 {
                workgroup: ValueId(11),
                input: ValueId(0),
                base: ValueId(1),
                lanes: 64,
                elements: 3,
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(3), Type::INDEX),
            Kind::Constant(Constant::Index(0)),
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(9),
        then_arguments: vec![ValueId(1)],
        else_target: BlockId(9),
        else_arguments: vec![ValueId(3)],
    });
    let mut transport = BasicBlock::new(BlockId(9));
    transport
        .parameters
        .push(ValueDef::new(ValueId(100), Type::INDEX));
    transport.operations.push(execution(
        13,
        Role::LaneFragmentU32 {
            lanes: 64,
            elements: 3,
        },
        Execution::TileIntoFragmentU32 {
            tile: ValueId(12),
            lanes: 64,
            elements: 3,
        },
    ));
    transport.terminator = Some(Terminator::Branch {
        target: BlockId(8),
        arguments: vec![ValueId(100)],
    });
    let mut exit = BasicBlock::new(BlockId(8));
    exit.parameters
        .push(ValueDef::new(ValueId(101), Type::INDEX));
    let results = (0..6)
        .map(|index| {
            ValueDef::new(
                ValueId(20 + index),
                if index < 3 {
                    Type::Scalar(ScalarType::U32)
                } else {
                    Type::BOOL
                },
            )
        })
        .collect();
    exit.operations = vec![
        Operation::new(
            results,
            Kind::Execution(Execution::FragmentIntoPartsU32 {
                fragment: ValueId(13),
                lanes: 64,
                elements: 3,
            }),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(30), Type::Scalar(ScalarType::U32)),
            Kind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(20),
                rhs: ValueId(21),
            },
        ),
        Operation::new(
            vec![],
            Kind::Execution(Execution::ScopeEnd {
                workgroup: ValueId(11),
                discarded: vec![ValueId(14)],
            }),
        ),
    ];
    exit.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(
            vec![
                Type::slice(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadOnly,
                ),
                Type::INDEX,
                Type::BOOL,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![entry, exit, transport],
    ));
    let mut kernel = Kernel::new(
        "entry",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    module.kernels.push(kernel);
    module
}

fn selection(layout: ExecutionTileLayoutV1) -> [TileScalarFunctionSelectionV18; 1] {
    [TileScalarFunctionSelectionV18 {
        function: CanonicalKirFunctionCoordinateV1(0),
        layout,
    }]
}

fn owner(module: &Module, budget: &mut Budget<'_>) -> Owner {
    let (owner, storage) =
        Owner::from_module_ref_with_verification_budget_v18(module, LAYOUTS, budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    owner
}

#[test]
fn tile_scalar_whole_graph_retains_discarded_reads_cross_block_transport_and_parallel_edges() {
    for layout in [
        ExecutionTileLayoutV1::Blocked,
        ExecutionTileLayoutV1::Striped,
    ] {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(37).unwrap();
        let original = fixture();
        let input = owner(&original, &mut budget);
        let floor = budget.storage();
        let result =
            prepare_owned_tile_scalar_v18(&input, &selection(layout), LAYOUTS, &mut budget)
                .unwrap();
        assert_eq!(budget.storage(), floor);
        budget.reserve_storage(result.retained_storage()).unwrap();
        result.replay_against(&input, &mut budget).unwrap();
        assert!(!result.grants_authority());
        let before = original.functions[0].body.as_ref().unwrap();
        let after = result.output().module().functions[0].body.as_ref().unwrap();
        assert_eq!(before.blocks.len(), after.blocks.len());
        for (a, b) in before.blocks.iter().zip(&after.blocks) {
            assert_eq!(a.id, b.id);
            assert_eq!(a.parameters, b.parameters);
            assert_eq!(a.terminator, b.terminator);
        }
        let reads = after.blocks[0]
            .operations
            .iter()
            .filter(|operation| matches!(operation.kind, Kind::GuardedLoad { .. }))
            .count();
        assert_eq!(
            reads, 6,
            "discarded second tile still reads all active components at its load site"
        );
        assert!(
            after.blocks[2].operations.is_empty(),
            "fragment transport is erased independently of physical block order"
        );
        assert_eq!(
            &after.blocks[1].operations[6],
            &before.blocks[1].operations[1]
        );
        for (index, copy) in after.blocks[1].operations[..6].iter().enumerate() {
            assert_eq!(
                copy.results,
                [before.blocks[1].operations[0].results[index].clone()]
            );
            assert!(copy.memory_effects().is_empty());
        }
        assert!(
            matches!(&after.blocks[1].operations[7].kind, Kind::Execution(Execution::ScopeEnd { workgroup: ValueId(11), discarded }) if discarded.is_empty())
        );
        assert_eq!(input.module(), &original);
    }
}

fn compare_mutation(mut candidate: Module, mutate: impl FnOnce(&mut Module)) -> bool {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let input = owner(&fixture(), &mut budget);
    mutate(&mut candidate);
    let Ok((output, storage)) =
        Owner::from_module_ref_with_verification_budget_v18(&candidate, LAYOUTS, &mut budget)
    else {
        return false;
    };
    budget.reserve_storage(storage.retained_storage()).unwrap();
    resources::scoped(&mut budget, |meter| {
        let (inventory, storage) =
            meter.derive(|budget| Ok(Inventory::derive_v18(&input, budget)?))?;
        meter.reserve(storage.retained_storage())?;
        let plan = derive(
            &inventory,
            &selection(ExecutionTileLayoutV1::Blocked),
            meter,
        )?;
        check::compare(&inventory, &output, &plan, meter)
    })
    .is_ok()
}

#[test]
fn tile_scalar_whole_graph_independent_checker_rejects_scalar_cfg_and_untouched_mutations() {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let input = owner(&fixture(), &mut budget);
    let lowered = prepare_owned_tile_scalar_v18(
        &input,
        &selection(ExecutionTileLayoutV1::Blocked),
        LAYOUTS,
        &mut budget,
    )
    .unwrap();
    let candidate = lowered.output().module().clone();
    assert!(compare_mutation(candidate.clone(), |_| {}));
    assert!(!compare_mutation(candidate.clone(), |m| m.id = "foreign".into()));
    assert!(!compare_mutation(candidate.clone(), |m| {
        let blocks = &mut m.functions[0].body.as_mut().unwrap().blocks;
        let Some(Terminator::ConditionalBranch { else_arguments, .. }) = &mut blocks[0].terminator
        else {
            panic!()
        };
        else_arguments[0] = ValueId(1);
    }));
    assert!(!compare_mutation(candidate.clone(), |m| {
        let blocks = &mut m.functions[0].body.as_mut().unwrap().blocks;
        let Kind::Binary { rhs, .. } = &mut blocks[1].operations[6].kind else {
            panic!()
        };
        *rhs = ValueId(22);
    }));
    assert!(!compare_mutation(candidate.clone(), |m| {
        let blocks = &mut m.functions[0].body.as_mut().unwrap().blocks;
        let Kind::Select { true_value, .. } = &mut blocks[1].operations[0].kind else {
            panic!()
        };
        *true_value = ValueId(21);
    }));
    assert!(!compare_mutation(candidate, |m| {
        let operations = &mut m.functions[0].body.as_mut().unwrap().blocks[0].operations;
        let load = operations
            .iter()
            .rposition(|op| matches!(op.kind, Kind::GuardedLoad { .. }))
            .unwrap();
        operations.remove(load);
    }));
}

#[test]
fn tile_scalar_whole_graph_refuses_missing_geometry_invalid_policy_and_role_parameters() {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let input = owner(&fixture(), &mut budget);
    let repeated = [selection(ExecutionTileLayoutV1::Blocked)[0]; 2];
    assert!(prepare_owned_tile_scalar_v18(&input, &repeated, LAYOUTS, &mut budget).is_err());
    let invalid = [TileScalarFunctionSelectionV18 {
        function: CanonicalKirFunctionCoordinateV1(99),
        layout: ExecutionTileLayoutV1::Blocked,
    }];
    assert!(prepare_owned_tile_scalar_v18(&input, &invalid, LAYOUTS, &mut budget).is_err());
    let mut module = fixture();
    module.kernels[0].workgroup_size = None;
    let input = owner(&module, &mut budget);
    assert!(
        prepare_owned_tile_scalar_v18(
            &input,
            &selection(ExecutionTileLayoutV1::Blocked),
            LAYOUTS,
            &mut budget
        )
        .is_err()
    );
    let mut module = fixture();
    module.functions[0].body.as_mut().unwrap().blocks[2].parameters[0].ty =
        Type::Execution(Role::MaskedTileU32 {
            lanes: 64,
            elements: 3,
        });
    assert!(
        Owner::from_module_ref_with_verification_budget_v18(&module, LAYOUTS, &mut budget).is_err()
    );
}

#[test]
fn tile_scalar_whole_graph_empty_selection_preserves_complete_module() {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let input = owner(&fixture(), &mut budget);
    let output = prepare_owned_tile_scalar_v18(&input, &[], LAYOUTS, &mut budget).unwrap();
    assert_eq!(output.output().module(), input.module());
}

#[test]
fn tile_scalar_whole_graph_keeps_unselected_function_and_rejects_foreign_replay() {
    let mut module = fixture();
    let mut other = module.functions[0].clone();
    other.id = "other".into();
    module.functions.push(other);
    let mut kernel = module.kernels[0].clone();
    kernel.id = "other".into();
    kernel.entry = "other".into();
    module.kernels.push(kernel);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let input = owner(&module, &mut budget);
    let output = prepare_owned_tile_scalar_v18(
        &input,
        &selection(ExecutionTileLayoutV1::Blocked),
        LAYOUTS,
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(output.retained_storage()).unwrap();
    assert_eq!(
        output.output().module().functions[1],
        input.module().functions[1]
    );
    module.id = "different-source".into();
    let foreign = owner(&module, &mut budget);
    assert!(matches!(
        output.replay_against(&foreign, &mut budget),
        Err(Error::ForeignInput)
    ));
}

#[test]
fn tile_scalar_whole_graph_preserves_repeated_workgroup_epochs_and_scope_owners() {
    let mut module = fixture();
    module.functions[0].body.as_mut().unwrap().blocks[1]
        .operations
        .extend([
            execution(
                110,
                Role::Workgroup,
                Execution::WorkgroupDerive {
                    context: ValueId(10),
                },
            ),
            execution(
                111,
                Role::MaskedTileU32 {
                    lanes: 64,
                    elements: 2,
                },
                Execution::MaskedTileLoadU32 {
                    workgroup: ValueId(110),
                    input: ValueId(0),
                    base: ValueId(1),
                    lanes: 64,
                    elements: 2,
                },
            ),
            Operation::new(
                vec![],
                Kind::Execution(Execution::ScopeEnd {
                    workgroup: ValueId(110),
                    discarded: vec![ValueId(111)],
                }),
            ),
        ]);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let input = owner(&module, &mut budget);
    let output = prepare_owned_tile_scalar_v18(
        &input,
        &selection(ExecutionTileLayoutV1::Striped),
        LAYOUTS,
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(output.retained_storage()).unwrap();
    output.replay_against(&input, &mut budget).unwrap();
    let blocks = &output.output().module().functions[0]
        .body
        .as_ref()
        .unwrap()
        .blocks;
    let scopes: Vec<_> = blocks[1]
        .operations
        .iter()
        .filter_map(|operation| match &operation.kind {
            Kind::Execution(Execution::ScopeEnd {
                workgroup,
                discarded,
            }) => {
                assert!(discarded.is_empty());
                Some(*workgroup)
            }
            _ => None,
        })
        .collect();
    assert_eq!(scopes, [ValueId(11), ValueId(110)]);
    assert_eq!(
        blocks[1]
            .operations
            .iter()
            .filter(|operation| matches!(operation.kind, Kind::GuardedLoad { .. }))
            .count(),
        2
    );
}

fn bounded(work_limit: usize, storage_limit: usize) -> (Result<()>, usize, usize) {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    let result = (|| {
        budget.reserve_storage(37)?;
        let (input, storage) =
            Owner::from_module_ref_with_verification_budget_v18(&fixture(), LAYOUTS, &mut budget)?;
        budget.reserve_storage(storage.retained_storage())?;
        let floor = budget.storage();
        let output = prepare_owned_tile_scalar_v18(
            &input,
            &selection(ExecutionTileLayoutV1::Striped),
            LAYOUTS,
            &mut budget,
        )?;
        assert_eq!(budget.storage(), floor);
        budget.reserve_storage(output.retained_storage())?;
        output.replay_against(&input, &mut budget)?;
        Ok(())
    })();
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn tile_scalar_whole_graph_exact_and_one_short_work_and_storage() {
    let (result, work, storage) = bounded(usize::MAX, usize::MAX);
    result.unwrap();
    bounded(work, storage).0.unwrap();
    assert!(bounded(work - 1, storage).0.is_err());
    assert!(bounded(work, storage - 1).0.is_err());
}
