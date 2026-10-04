use super::*;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, Function, Kernel, LaunchDomain,
    LaunchExtent, Module, Operation, Signature, Terminator,
    VerifiedCanonicalKernelIrModuleV12 as Owner, WorkgroupSize,
};

#[path = "production_checked_output_wave_broadcast_v1_tests.rs"]
mod broadcast;

fn module(width: WaveWidth, tile: u32, kind: WaveF32ReductionKindV1) -> Module {
    let mut module = Module::new("wave-reduction-census");
    let wave = WaveOperation::full(
        WaveOperationKind::ReduceF32 {
            value: ValueId(17),
            tile_width: tile,
            kind,
        },
        width,
    );
    module.required_capabilities = wave.required_capabilities();
    let mut block = BasicBlock::new(BlockId(97));
    block.operations.push(Operation::new(
        vec![ValueDef::new(ValueId(91), Type::F32)],
        OperationKind::Wave(wave),
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::kernel_entry(
        "wave",
        Signature::new(vec![Type::F32, Type::F32], vec![]),
        vec![ValueId(17), ValueId(19)],
        vec![block],
    ));
    let mut kernel = Kernel::new(
        "wave",
        "wave",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    module.kernels.push(kernel);
    module
}

fn with_inventory(
    module: &Module,
    consume: impl FnOnce(&CanonicalKirInventoryV1<'_>, &mut AssertOriginBudgetV1<'_>),
) {
    let mut work = Work::new(usize::MAX);
    let mut budget = AssertOriginBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(19).unwrap();
    let (owner, owner_storage) =
        Owner::from_module_ref_with_verification_budget_v12(module, &mut budget).unwrap();
    budget
        .reserve_storage(owner_storage.retained_storage())
        .unwrap();
    let (inventory, storage) = CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let floor = budget.storage();
    consume(&inventory, &mut budget);
    assert_eq!(budget.storage(), floor);
    drop(inventory);
    budget.release_storage(storage.retained_storage()).unwrap();
    drop(owner);
    budget
        .release_storage(owner_storage.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), 19);
}

#[test]
fn wave_reduction_census_all_static_tiles_and_kinds_are_exact_and_repeatable() {
    for kind in [WaveF32ReductionKindV1::Sum, WaveF32ReductionKindV1::Maximum] {
        for tile in [1, 2, 4, 8, 16, 32, 64] {
            with_inventory(
                &module(WaveWidth::Wave64, tile, kind),
                |inventory, budget| {
                    let row = &inventory.operations()[0];
                    let floor = budget.storage();
                    let before = budget.work();
                    for _ in 0..3 {
                        assert!(native(inventory, 0, row, budget).unwrap());
                        assert_eq!(budget.storage(), floor);
                    }
                    assert_eq!(budget.work() - before, 3 * (96 + 2));
                    assert_eq!(
                        row.convergence(),
                        fe2o3_kernel_analysis::CanonicalKirBehaviorAnalysisV1::NotAnalyzed
                    );
                },
            );
        }
    }
}

#[test]
fn wave_reduction_census_rejects_copied_foreign_and_wrong_site_rows_without_debit() {
    let source = module(WaveWidth::Wave64, 16, WaveF32ReductionKindV1::Sum);
    with_inventory(&source, |inventory, budget| {
        let row = &inventory.operations()[0];
        assert!(native(inventory, 0, row, budget).unwrap());
        let copied = CanonicalKirOperationRefV1 {
            coordinate: row.coordinate,
            operation: row.operation,
            results: row.results.clone(),
            operands: row.operands.clone(),
            effects: row.effects.clone(),
        };
        let before = (budget.work(), budget.storage());
        for (ordinal, row) in [(0, &copied), (1, row), (usize::MAX, row)] {
            assert!(matches!(
                native(inventory, ordinal, row, budget),
                Err(E::Unsupported {
                    phase: "wave reduction",
                    detail: "same borrowed operation site"
                })
            ));
            assert_eq!((budget.work(), budget.storage()), before);
        }
        with_inventory(&source, |foreign, _| {
            assert!(matches!(
                native(inventory, 0, &foreign.operations()[0], budget),
                Err(E::Unsupported {
                    phase: "wave reduction",
                    detail: "same borrowed operation site"
                })
            ));
            assert_eq!((budget.work(), budget.storage()), before);
        });
        assert!(native(inventory, 0, row, budget).unwrap());
    });
}

#[test]
fn wave_reduction_census_keeps_other_wave_families_and_wave32_closed() {
    with_inventory(
        &module(WaveWidth::Wave32, 16, WaveF32ReductionKindV1::Sum),
        |inventory, budget| {
            assert!(!native(inventory, 0, &inventory.operations()[0], budget).unwrap());
        },
    );
    let mut broadcast = module(WaveWidth::Wave64, 16, WaveF32ReductionKindV1::Sum);
    let body = broadcast.functions[0].body.as_mut().unwrap();
    body.blocks[0].operations.insert(
        0,
        Operation::new(
            vec![ValueDef::new(ValueId(41), Type::Scalar(ScalarType::U32))],
            OperationKind::Constant(fe2o3_kernel_ir::Constant::U32(0)),
        ),
    );
    let OperationKind::Wave(wave) = &mut body.blocks[0].operations[1].kind else {
        unreachable!()
    };
    wave.kind = WaveOperationKind::ShuffleIndex {
        value: ValueId(41),
        source_lane: ValueId(41),
        tile_width: 16,
    };
    body.blocks[0].operations[1].results[0].ty = Type::Scalar(ScalarType::U32);
    with_inventory(&broadcast, |inventory, budget| {
        let before = (budget.work(), budget.storage());
        assert!(!native(inventory, 1, &inventory.operations()[1], budget).unwrap());
        assert_eq!((budget.work(), budget.storage()), before);
    });
}

#[test]
fn wave_reduction_census_does_not_extend_helper_or_nonwave_grammar() {
    let mut graph = module(WaveWidth::Wave64, 16, WaveF32ReductionKindV1::Sum);
    let mut helper = graph.functions[0].clone();
    helper.id = "helper".into();
    helper.role = FunctionRole::InternalHelper;
    graph.functions.push(helper);
    graph.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(Operation::new(
            vec![ValueDef::new(ValueId(93), Type::F32)],
            OperationKind::Constant(fe2o3_kernel_ir::Constant::F32Bits(0)),
        ));
    with_inventory(&graph, |inventory, budget| {
        assert!(native(inventory, 0, &inventory.operations()[0], budget).unwrap());
        let before = (budget.work(), budget.storage());
        assert!(!native(inventory, 1, &inventory.operations()[1], budget).unwrap());
        assert_eq!((budget.work(), budget.storage()), before);
        assert!(!native(inventory, 2, &inventory.operations()[2], budget).unwrap());
        assert_eq!(budget.work() - before.0, 96);
        assert_eq!(budget.storage(), before.1);
        assert!(native(inventory, 0, &inventory.operations()[0], budget).unwrap());
    });
}

#[test]
fn wave_reduction_census_exact_and_short_query_resources_restore_nonzero_floor() {
    with_inventory(
        &module(WaveWidth::Wave64, 8, WaveF32ReductionKindV1::Maximum),
        |inventory, _| {
            let bytes = independent_headers();
            assert_eq!(headers().unwrap(), bytes);
            for (limit, storage, expected_work, success) in [
                (98, 23 + bytes, 98, true),
                (97, 23 + bytes, 96, false),
                (95, 23 + bytes, 0, false),
                (98, 22 + bytes, 0, false),
            ] {
                // Fresh ledgers isolate this private query's additive equation;
                // neither the meter nor copied observations confer owner authority.
                let mut work = Work::new(limit);
                let mut budget = AssertOriginBudgetV1::new(&mut work, storage);
                budget.reserve_storage(23).unwrap();
                let result = native(inventory, 0, &inventory.operations()[0], &mut budget);
                if success {
                    assert!(result.unwrap());
                } else if storage == 22 + bytes {
                    assert!(
                        matches!(result, Err(E::Resource(AssertOriginResourceV1::Storage(error)))
                    if error.limit() == storage && error.actual() == storage + 1)
                    );
                } else {
                    assert!(
                        matches!(result, Err(E::Resource(AssertOriginResourceV1::Work(error)))
                    if error.limit() == limit && error.actual() == if limit < 96 { 96 } else { 98 })
                    );
                }
                assert_eq!(budget.work(), expected_work);
                assert_eq!(budget.storage(), 23);
            }
        },
    );
}

fn independent_headers() -> usize {
    use std::mem::size_of;
    fn h<T>() -> usize {
        size_of::<T>() + size_of::<Option<T>>() + 2 * size_of::<R<T>>()
    }
    // Caller/check frames, dense query returns, payload copies, and iteration.
    2 * h::<&CanonicalKirInventoryV1<'_>>()
        + 2 * h::<&CanonicalKirOperationRefV1<'_>>()
        + h::<&CanonicalKirFunctionRefV1<'_>>()
        + h::<&CanonicalKirBlockRefV1<'_>>()
        + 3 * h::<&CanonicalKirDefinitionRefV1<'_>>()
        + 2 * h::<&CanonicalKirUseRefV1>()
        + h::<&CanonicalKirKernelRefV1<'_>>()
        + 2 * h::<&WaveOperation>()
        + h::<&ValueDef>()
        + h::<&[ValueDef]>()
        + h::<&[CanonicalKirOperationRefV1<'_>]>()
        + h::<&[CanonicalKirFunctionRefV1<'_>]>()
        + h::<&[CanonicalKirBlockRefV1<'_>]>()
        + h::<&[CanonicalKirDefinitionRefV1<'_>]>()
        + h::<&[CanonicalKirUseRefV1]>()
        + h::<&[CanonicalKirKernelRefV1<'_>]>()
        + h::<std::slice::Iter<'_, CanonicalKirKernelRefV1<'_>>>()
        + 2 * h::<&std::ops::Range<usize>>()
        + h::<&OperationKind>()
        + 3 * h::<&Type>()
        + h::<Type>()
        + h::<ScalarType>()
        + h::<WaveOperationKind>()
        + h::<WaveF32ReductionKindV1>()
        + h::<WaveWidth>()
        + h::<fe2o3_kernel_ir::Convergence>()
        + h::<SynchronizationScope>()
        + 2 * h::<ValueId>()
        + h::<(ValueId, u32, Option<ValueId>)>()
        + h::<u32>()
        + h::<bool>()
        + h::<usize>()
        + h::<fe2o3_kernel_ir::CompilerOrderingEffectSummaryV12>()
        + h::<CanonicalKirDefinitionCoordinateV1>()
        + h::<CanonicalKirUseCoordinateV1>()
        + 2 * h::<CanonicalKirOperationCoordinateV1>()
        + 2 * h::<&mut AssertOriginBudgetV1<'_>>()
        + 9 * size_of::<usize>()
        + 2 * size_of::<R<()>>()
        + 2 * size_of::<Result<(), AssertOriginResourceV1>>()
}

#[test]
fn wave_reduction_census_is_not_a_target_or_uniformity_certificate() {
    let source = module(WaveWidth::Wave64, 4, WaveF32ReductionKindV1::Sum);
    with_inventory(&source, |inventory, budget| {
        assert!(native(inventory, 0, &inventory.operations()[0], budget).unwrap());
    });
    let bound = dialect_amdgcn::bind_production_target_v1(
        &source,
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
    )
    .unwrap();
    let error = dialect_amdgcn::lower_compiler_module_to_gfx942_xnack_minus_llvm_ir(bound.module())
        .unwrap_err();
    assert!(error.diagnostics().iter().any(|row|
        row.code == dialect_amdgcn::LoweringDiagnosticCode::UnsupportedWaveOperation));
}

#[path = "production_checked_output_wave_reduction_source_v1_tests.rs"]
mod source_tests;

#[test]
fn wave_reduction_census_does_not_waive_divergent_control_or_partial_workgroups() {
    use fe2o3_kernel_ir::{
        Axis, ComparePredicate, Constant, IndexKind, IntrinsicKind, IntrinsicOperation,
    };
    for divergent in [false, true] {
        let mut candidate = module(WaveWidth::Wave64, 16, WaveF32ReductionKindV1::Sum);
        if divergent {
            let body = candidate.functions[0].body.as_mut().unwrap();
            let mut wave = body.blocks.remove(0);
            wave.id = BlockId(7);
            wave.terminator = Some(Terminator::Branch {
                target: BlockId(41),
                arguments: vec![],
            });
            let mut entry = BasicBlock::new(BlockId(97));
            entry.operations = vec![
                Operation::new(
                    vec![ValueDef::new(ValueId(23), Type::INDEX)],
                    OperationKind::Intrinsic(IntrinsicOperation::new(
                        IntrinsicKind::InvocationIndex {
                            kind: IndexKind::Local,
                            axis: Axis::X,
                        },
                        Type::INDEX,
                    )),
                ),
                Operation::new(
                    vec![ValueDef::new(ValueId(24), Type::INDEX)],
                    OperationKind::Constant(Constant::Index(0)),
                ),
                Operation::new(
                    vec![ValueDef::new(ValueId(25), Type::BOOL)],
                    OperationKind::Compare {
                        predicate: ComparePredicate::Equal,
                        lhs: ValueId(23),
                        rhs: ValueId(24),
                    },
                ),
            ];
            entry.terminator = Some(Terminator::ConditionalBranch {
                condition: ValueId(25),
                then_target: BlockId(7),
                then_arguments: vec![],
                else_target: BlockId(41),
                else_arguments: vec![],
            });
            let mut exit = BasicBlock::new(BlockId(41));
            exit.terminator = Some(Terminator::Return { values: vec![] });
            body.blocks = vec![entry, wave, exit];
        } else {
            candidate.kernels[0].workgroup_size = Some(WorkgroupSize::new(32, 1, 1));
        }
        with_inventory(&candidate, |inventory, budget| {
            let mut visited = 0;
            for (ordinal, row) in inventory.operations().iter().enumerate() {
                if matches!(row.operation.kind, OperationKind::Wave(_)) {
                    assert!(native(inventory, ordinal, row, budget).unwrap());
                    visited += 1;
                }
            }
            assert_eq!(visited, 1);
        });
        let bound = dialect_amdgcn::bind_production_target_v1(
            &candidate,
            fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
        )
        .unwrap();
        let error =
            dialect_amdgcn::lower_compiler_module_to_gfx950_xnack_minus_llvm_ir(bound.module())
                .unwrap_err();
        let expected = if divergent {
            dialect_amdgcn::LoweringDiagnosticCode::UnprovenBarrierConvergence
        } else {
            dialect_amdgcn::LoweringDiagnosticCode::UnsupportedWaveOperation
        };
        assert!(
            error.diagnostics().iter().any(|row| row.code == expected),
            "{error:?}"
        );
    }
}
