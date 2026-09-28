use super::*;
use fe2o3_kernel_ir::{BinaryOp, Constant};

#[derive(Clone, Copy)]
enum Lane {
    Constant(u32),
    Masked(u32),
    Dynamic,
}

fn graph(width: WaveWidth, tile: u32, lane: Lane) -> Module {
    let mut graph = module(width, tile, WaveF32ReductionKindV1::Sum);
    graph.functions[0]
        .signature
        .parameters
        .push(Type::Scalar(ScalarType::U32));
    let body = graph.functions[0].body.as_mut().unwrap();
    body.parameters.push(ValueId(21));
    let mut prefix = Vec::new();
    let source_lane = match lane {
        Lane::Constant(value) | Lane::Masked(value) => {
            prefix.push(Operation::new(
                vec![ValueDef::new(ValueId(41), Type::Scalar(ScalarType::U32))],
                OperationKind::Constant(Constant::U32(value)),
            ));
            if matches!(lane, Lane::Masked(_)) {
                prefix.push(Operation::new(
                    vec![ValueDef::new(ValueId(42), Type::Scalar(ScalarType::U32))],
                    OperationKind::Binary {
                        op: BinaryOp::BitAnd,
                        lhs: ValueId(21),
                        rhs: ValueId(41),
                    },
                ));
                ValueId(42)
            } else {
                ValueId(41)
            }
        }
        Lane::Dynamic => ValueId(21),
    };
    let wave = &mut body.blocks[0].operations[0];
    wave.kind = OperationKind::Wave(WaveOperation::full(
        WaveOperationKind::BroadcastF32 {
            value: ValueId(17),
            source_lane,
            tile_width: tile,
        },
        width,
    ));
    prefix.append(&mut body.blocks[0].operations);
    body.blocks[0].operations = prefix;
    graph
}

#[test]
fn wave_broadcast_census_all_tiles_exact_operands_and_native_bounded_lanes() {
    for tile in [1, 2, 4, 8, 16, 32, 64] {
        for lane in [
            Lane::Constant(0),
            Lane::Constant(tile - 1),
            Lane::Masked(tile - 1),
        ] {
            let module = graph(WaveWidth::Wave64, tile, lane);
            with_inventory(&module, |inventory, budget| {
                let ordinal = inventory.operations().len() - 1;
                let row = &inventory.operations()[ordinal];
                let floor = budget.storage();
                let work = budget.work();
                for _ in 0..3 {
                    assert!(native(inventory, ordinal, row, budget).unwrap());
                }
                assert_eq!(budget.work() - work, 3 * (96 + 32 + 2));
                assert_eq!(budget.storage(), floor);
                assert_eq!(row.operands.len(), 2);
                assert_eq!(
                    row.convergence(),
                    fe2o3_kernel_analysis::CanonicalKirBehaviorAnalysisV1::NotAnalyzed
                );
            });
            let bound = dialect_amdgcn::bind_production_target_v1(
                &module,
                fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
            )
            .unwrap();
            let llvm =
                dialect_amdgcn::lower_compiler_module_to_gfx950_xnack_minus_llvm_ir(bound.module())
                    .unwrap();
            assert!(llvm.contains("llvm.amdgcn.ds.bpermute"));
        }
    }
}

#[test]
fn wave_broadcast_census_foreign_copied_and_wrong_site_rows_have_zero_debit() {
    let module = graph(WaveWidth::Wave64, 16, Lane::Constant(7));
    with_inventory(&module, |inventory, budget| {
        let row = &inventory.operations()[1];
        assert!(native(inventory, 1, row, budget).unwrap());
        let copy = CanonicalKirOperationRefV1 {
            coordinate: row.coordinate,
            operation: row.operation,
            results: row.results.clone(),
            operands: row.operands.clone(),
            effects: row.effects.clone(),
        };
        let before = (budget.work(), budget.storage());
        for (ordinal, candidate) in [(1, &copy), (0, row), (usize::MAX, row)] {
            assert!(matches!(
                native(inventory, ordinal, candidate, budget),
                Err(E::Unsupported {
                    phase: "wave reduction",
                    detail: "same borrowed operation site"
                })
            ));
            assert_eq!((budget.work(), budget.storage()), before);
        }
        with_inventory(&module, |foreign, _| {
            assert!(matches!(
                native(inventory, 1, &foreign.operations()[1], budget),
                Err(E::Unsupported {
                    phase: "wave reduction",
                    detail: "same borrowed operation site"
                })
            ));
            assert_eq!((budget.work(), budget.storage()), before);
        });
        assert!(native(inventory, 1, row, budget).unwrap());
    });
}

#[test]
fn wave_broadcast_census_exact_second_operand_resource_cuts_restore_floor() {
    with_inventory(
        &graph(WaveWidth::Wave64, 8, Lane::Masked(7)),
        |inventory, _| {
            let bytes = independent_headers();
            assert_eq!(headers().unwrap(), bytes);
            for (limit, storage, accepted, attempted, success) in [
                (130, 23 + bytes, 130, 0, true),
                (129, 23 + bytes, 128, 130, false),
                (127, 23 + bytes, 96, 128, false),
                (95, 23 + bytes, 0, 96, false),
                (130, 22 + bytes, 0, 0, false),
            ] {
                let mut work = Work::new(limit);
                let mut budget = AssertOriginBudgetV1::new(&mut work, storage);
                budget.reserve_storage(23).unwrap();
                let result = native(inventory, 2, &inventory.operations()[2], &mut budget);
                if success {
                    assert!(result.unwrap());
                } else if storage == 22 + bytes {
                    assert!(
                        matches!(result, Err(E::Resource(AssertOriginResourceV1::Storage(e))) if e.limit() == storage && e.actual() == storage + 1)
                    );
                } else {
                    assert!(
                        matches!(result, Err(E::Resource(AssertOriginResourceV1::Work(e))) if e.limit() == limit && e.actual() == attempted)
                    );
                }
                assert_eq!(budget.work(), accepted);
                assert_eq!(budget.storage(), 23);
            }
        },
    );
}

#[test]
fn wave_broadcast_schema_census_does_not_supply_native_lane_bounds_or_target_authority() {
    for lane in [Lane::Dynamic, Lane::Constant(16), Lane::Masked(31)] {
        let module = graph(WaveWidth::Wave64, 16, lane);
        let mut work = Work::new(usize::MAX);
        let mut budget = AssertOriginBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(23).unwrap();
        let error =
            Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap_err();
        assert!(
            matches!(
                error,
                fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV12::Verification(_)
            ),
            "{error:?}"
        );
        assert!(
            error
                .to_string()
                .contains("statically bounded tile-local source lane")
        );
        assert_eq!(budget.storage(), 23);
    }
    let module = graph(WaveWidth::Wave64, 16, Lane::Constant(0));
    let bound = dialect_amdgcn::bind_production_target_v1(
        &module,
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
    )
    .unwrap();
    let error = dialect_amdgcn::lower_compiler_module_to_gfx942_xnack_minus_llvm_ir(bound.module())
        .unwrap_err();
    assert!(error.diagnostics().iter().any(|row| row.code == dialect_amdgcn::LoweringDiagnosticCode::UnsupportedWaveOperation), "{error:?}");
}

#[test]
fn wave_broadcast_census_keeps_wave32_and_helper_context_closed() {
    with_inventory(
        &graph(WaveWidth::Wave32, 16, Lane::Constant(0)),
        |inventory, budget| {
            assert!(!native(inventory, 1, &inventory.operations()[1], budget).unwrap());
        },
    );
    let mut module = graph(WaveWidth::Wave64, 16, Lane::Constant(0));
    let mut helper = module.functions[0].clone();
    helper.id = "helper".into();
    helper.role = FunctionRole::InternalHelper;
    module.functions.push(helper);
    with_inventory(&module, |inventory, budget| {
        assert!(native(inventory, 1, &inventory.operations()[1], budget).unwrap());
        assert!(!native(inventory, 3, &inventory.operations()[3], budget).unwrap());
    });
}

#[test]
fn wave_broadcast_census_does_not_waive_control_convergence_or_full_workgroups() {
    use fe2o3_kernel_ir::{Axis, ComparePredicate, IndexKind, IntrinsicKind, IntrinsicOperation};
    for divergent in [false, true] {
        let mut module = graph(WaveWidth::Wave64, 16, Lane::Constant(0));
        if divergent {
            let body = module.functions[0].body.as_mut().unwrap();
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
            module.kernels[0].workgroup_size = Some(WorkgroupSize::new(32, 1, 1));
        }
        with_inventory(&module, |inventory, budget| {
            let mut count = 0;
            for (ordinal, row) in inventory.operations().iter().enumerate() {
                if matches!(row.operation.kind, OperationKind::Wave(_)) {
                    assert!(native(inventory, ordinal, row, budget).unwrap());
                    count += 1;
                }
            }
            assert_eq!(count, 1);
        });
        let bound = dialect_amdgcn::bind_production_target_v1(
            &module,
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

#[test]
fn wave_broadcast_census_malformed_operand_types_results_and_tiles_have_no_owner() {
    for fault in 0..6 {
        let mut module = graph(WaveWidth::Wave64, 16, Lane::Constant(0));
        let body = module.functions[0].body.as_mut().unwrap();
        let op = &mut body.blocks[0].operations[1];
        let OperationKind::Wave(wave) = &mut op.kind else {
            unreachable!()
        };
        let WaveOperationKind::BroadcastF32 {
            value,
            source_lane,
            tile_width,
        } = &mut wave.kind
        else {
            unreachable!()
        };
        match fault {
            0 => *value = ValueId(21),
            1 => *source_lane = ValueId(17),
            2 => op.results[0].ty = Type::Scalar(ScalarType::U32),
            3 => op.results.push(ValueDef::new(ValueId(92), Type::F32)),
            4 => *tile_width = 0,
            5 => *tile_width = 3,
            _ => unreachable!(),
        }
        let mut work = Work::new(usize::MAX);
        let mut budget = AssertOriginBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(23).unwrap();
        let error =
            Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap_err();
        assert!(
            matches!(
                error,
                fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV12::Verification(_)
            ),
            "fault {fault}: {error:?}"
        );
        assert_eq!(budget.storage(), 23);
    }
}
