//! Inert implementation controls, not rustc/HIR or numerical qualification.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::cell::Cell;

const WORK: usize = 1usize << 54;
const STORAGE: usize = 1usize << 31;
const FLOOR: usize = 23;

fn operand(role: SemanticMfmaOperandRoleV1) -> SemanticValueBindingV1 {
    SemanticValueBindingV1::MatrixFragment {
        values: (0..4)
            .map(|i| (ValueId(i + 1), Type::Scalar(ScalarType::Bf16)))
            .collect(),
        contract: SemanticMfmaOperandContractV1 {
            role,
            profile: SemanticMfmaProfileV1::Bf16F32M16N16K16,
            register_distribution: SemanticMfmaRegisterDistributionV1::Tile16x16,
            wave_width: 64,
        },
        storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
        wave: SemanticCurrentWaveV1::new(64),
    }
}
fn accumulator() -> SemanticValueBindingV1 {
    SemanticValueBindingV1::AccumulatorFragment {
        values: (0..4).map(|i| (ValueId(i + 21), Type::F32)).collect(),
        contract: SemanticMfmaAccumulatorContractV1 {
            profile: SemanticMfmaProfileV1::Bf16F32M16N16K16,
            wave_width: 64,
            distribution:
                fe2o3_mir_model::semantic_mir_v1::SemanticMfmaAccumulatorDistributionV1::RowMajor,
        },
        wave: SemanticCurrentWaveV1::new(64),
    }
}
fn block(id: u32, terminator: Terminator) -> BasicBlock {
    let mut result = BasicBlock::new(BlockId(id));
    result.terminator = Some(terminator);
    result
}
fn returning(id: u32) -> BasicBlock {
    block(id, Terminator::Return { values: vec![] })
}
fn branch(id: u32, target: u32, arguments: Vec<ValueId>) -> BasicBlock {
    block(
        id,
        Terminator::Branch {
            target: BlockId(target),
            arguments,
        },
    )
}
fn launch() -> RetainedRankedLaunchRootV1 {
    RetainedRankedLaunchRootV1 {
        selected_root: SemanticFunctionIdV1::from_index(0),
        launch_rank: 1,
        global_extents: [64, 1, 1],
        workgroup_extents: [64, 1, 1],
        full_physical_workgroups: true,
    }
}
fn module() -> Module {
    let mut first = block(
        0,
        Terminator::ConditionalBranch {
            condition: ValueId(0),
            then_target: BlockId(1),
            then_arguments: vec![],
            else_target: BlockId(2),
            else_arguments: vec![],
        },
    );
    // Unused real constant lets identity mutations remain numerically equal.
    first.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(30), Type::BOOL),
        OperationKind::Constant(Constant::Bool(true)),
    ));
    let mut call = branch(1, 2, vec![]);
    call.operations.push(Operation::new(
        (100..104)
            .map(|id| ValueDef::new(ValueId(id), Type::F32))
            .collect(),
        OperationKind::Call {
            callee: FunctionId::new("helper"),
            arguments: (1..13).map(ValueId).collect(),
        },
    ));
    let types = (0..12)
        .map(|i| {
            if i < 8 {
                Type::Scalar(ScalarType::Bf16)
            } else {
                Type::F32
            }
        })
        .collect::<Vec<_>>();
    let mut root_types = vec![Type::BOOL];
    root_types.extend(types.iter().cloned());
    let root = Function::kernel_entry(
        "root",
        Signature::new(root_types, vec![]),
        (0..13).map(ValueId).collect(),
        vec![first, call, returning(2)],
    );
    let mut matrix = block(
        0,
        Terminator::Return {
            values: (20..24).map(ValueId).collect(),
        },
    );
    matrix.operations.push(Operation::new(
        (20..24)
            .map(|id| ValueDef::new(ValueId(id), Type::F32))
            .collect(),
        OperationKind::Matrix(
            MatrixOperation::multiply_accumulate(
                [ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
                [ValueId(4), ValueId(5), ValueId(6), ValueId(7)],
                [ValueId(8), ValueId(9), ValueId(10), ValueId(11)],
            )
            .with_declared_tensor_layout(
                TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64()
                    .with_zero_filled_predicate_inputs(),
            ),
        ),
    ));
    let helper = Function::internal_helper(
        "helper",
        Signature::new(types, vec![Type::F32; 4]),
        (0..12).map(ValueId).collect(),
        vec![matrix],
    );
    let mut result = Module::new("inert_control");
    result.functions.extend([root, helper]);
    let mut kernel = Kernel::new(
        "root",
        FunctionId::new("root"),
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    result.kernels.push(kernel);
    result
}
struct Dropped<'a>(&'a Cell<bool>);
impl Drop for Dropped<'_> {
    fn drop(&mut self) {
        self.0.set(true);
    }
}

#[test]
fn exact_component_roles_are_not_numeric_or_scalar_array_equivalence() {
    assert!(
        bf16_live_components_v1(
            &operand(SemanticMfmaOperandRoleV1::A),
            Bf16CallInstanceRoleV1::Lhs
        )
        .is_ok()
    );
    assert!(
        bf16_live_components_v1(
            &operand(SemanticMfmaOperandRoleV1::B),
            Bf16CallInstanceRoleV1::Rhs
        )
        .is_ok()
    );
    assert!(
        bf16_live_components_v1(
            &operand(SemanticMfmaOperandRoleV1::A),
            Bf16CallInstanceRoleV1::Rhs
        )
        .is_err()
    );
    assert!(
        bf16_live_components_v1(
            &operand(SemanticMfmaOperandRoleV1::B),
            Bf16CallInstanceRoleV1::Lhs
        )
        .is_err()
    );
    assert!(bf16_live_components_v1(&accumulator(), Bf16CallInstanceRoleV1::Result).is_ok());
    assert!(bf16_live_components_v1(&accumulator(), Bf16CallInstanceRoleV1::Lhs).is_err());
    let aggregate = SemanticValueBindingV1::Aggregate(
        (0..4)
            .map(|i| SemanticValueBindingV1::Value {
                id: ValueId(i + 1),
                ty: Type::F32,
            })
            .collect(),
    );
    assert!(bf16_live_components_v1(&aggregate, Bf16CallInstanceRoleV1::Values).is_ok());
    assert!(bf16_live_components_v1(&aggregate, Bf16CallInstanceRoleV1::Zero).is_err());
}
#[test]
fn wrong_wave_layout_profile_type_and_duplicate_id_refuse() {
    for which in 0..6 {
        let mut binding = operand(SemanticMfmaOperandRoleV1::A);
        let SemanticValueBindingV1::MatrixFragment {
            values,
            contract,
            storage_layout,
            wave,
        } = &mut binding
        else {
            unreachable!()
        };
        match which {
            0 => wave.width = 32,
            1 => contract.wave_width = 32,
            2 => *storage_layout = SemanticMfmaStorageLayoutV1::ColumnMajor,
            3 => contract.profile = SemanticMfmaProfileV1::Fp8E4M3F32M16N16K128,
            4 => values[1].0 = values[0].0,
            _ => values[0].1 = Type::F32,
        }
        assert!(bf16_live_components_v1(&binding, Bf16CallInstanceRoleV1::Lhs).is_err());
    }
}
#[test]
fn context_is_empty_logical_transport_not_a_fabricated_pointer() {
    assert_eq!(
        bf16_live_components_v1(
            &SemanticValueBindingV1::MatrixContext,
            Bf16CallInstanceRoleV1::Context
        )
        .unwrap(),
        [ValueId(0); 4]
    );
    assert!(
        bf16_live_components_v1(
            &SemanticValueBindingV1::Value {
                id: ValueId(0),
                ty: Type::INDEX
            },
            Bf16CallInstanceRoleV1::Context
        )
        .is_err()
    );
    let bindings = vec![
        SemanticValueBindingV1::MatrixContext,
        operand(SemanticMfmaOperandRoleV1::A),
        operand(SemanticMfmaOperandRoleV1::B),
        accumulator(),
    ];
    assert!(bf16_check_call_bindings_v1(&bindings).is_ok());
    assert!(bf16_check_call_bindings_v1(&bindings[..3]).is_err());
}
#[test]
fn envelope_checked_arithmetic_and_original_phase_cap() {
    let envelope = bf16_envelope_arithmetic_v1(32, 32, 64, 64, 26).unwrap();
    assert!(envelope.storage < STORAGE);
    assert!(envelope.work < WORK);
    for index in 0..5 {
        let mut row = [32, 32, 64, 64, 26];
        row[index] = usize::MAX;
        assert_eq!(
            bf16_envelope_arithmetic_v1(row[0], row[1], row[2], row[3], row[4]),
            Err(ArgumentResourceV1::Arithmetic)
        );
    }
}
#[test]
fn envelope_zero_one_short_and_work_denial_precede_output() {
    let envelope = bf16_envelope_arithmetic_v1(0, 0, 0, 0, 0).unwrap();
    for ceiling in [FLOOR, FLOOR + envelope.storage - 1] {
        let mut work = Work::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, ceiling);
        budget.reserve_storage(FLOOR).unwrap();
        let entered = Cell::new(false);
        let result = bf16_emission_scope_v1(&mut budget, |budget| {
            budget.reserve_storage(envelope.storage)?;
            budget.charge_work(envelope.work)?;
            entered.set(true);
            Ok(())
        });
        assert!(result.is_err());
        assert!(!entered.get());
        assert_eq!(budget.storage(), FLOOR);
        assert!(budget.failed_storage().is_some());
    }
    let mut work = Work::new(envelope.work - 1);
    let mut budget = ArgumentBudgetV1::new(&mut work, FLOOR + envelope.storage);
    budget.reserve_storage(FLOOR).unwrap();
    let entered = Cell::new(false);
    assert!(
        bf16_emission_scope_v1(&mut budget, |budget| {
            budget.reserve_storage(envelope.storage)?;
            budget.charge_work(envelope.work)?;
            entered.set(true);
            Ok(())
        })
        .is_err()
    );
    assert!(!entered.get());
    assert!(budget.failed_work().is_some());
    assert_eq!(budget.storage(), FLOOR);
}

fn type_chain(count: usize) -> Vec<SemanticTypeDeclV1> {
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticAggregateTypeV1, SemanticLayoutIdentityV1, SemanticTypeIdentityV1,
        SemanticTypeLayoutV1,
    };
    (0..count)
        .map(|i| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([i as u8; 32]),
                SemanticLayoutIdentityV1::from_sha256([i as u8; 32]),
                SemanticTypeLayoutV1::new(Some(1), 1).unwrap(),
                if i == 0 {
                    SemanticTypeShapeV1::Unit
                } else if i % 2 == 0 {
                    SemanticTypeShapeV1::Tuple(
                        SemanticAggregateTypeV1::new(vec![SemanticTypeIdV1::from_index(
                            (i - 1) as u32,
                        )])
                        .unwrap(),
                    )
                } else {
                    SemanticTypeShapeV1::Array {
                        element: SemanticTypeIdV1::from_index((i - 1) as u32),
                        length: 1,
                    }
                },
            )
        })
        .collect()
}
#[test]
fn cached_aggregate_and_array_depth_is_bounded_before_planning() {
    let mut work = Work::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, BF16_TYPE_SCAN_STORAGE_V1 + FLOOR);
    budget.reserve_storage(FLOOR).unwrap();
    assert!(bf16_bounded_types_v1(&type_chain(8), &mut budget).is_ok());
    assert_eq!(budget.storage(), FLOOR);
    assert!(bf16_bounded_types_v1(&type_chain(9), &mut budget).is_err());
    assert_eq!(budget.storage(), FLOOR);
}
#[test]
fn type_scan_one_short_refuses_before_visiting_input() {
    let types = type_chain(8);
    let mut work = Work::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, BF16_TYPE_SCAN_STORAGE_V1 - 1);
    assert!(bf16_bounded_types_v1(&types, &mut budget).is_err());
    assert_eq!(budget.work(), 0);
    assert_eq!(budget.storage(), 0);
    assert!(budget.failed_storage().is_some());
}

fn bounded_type(index: u8, shape: SemanticTypeShapeV1) -> SemanticTypeDeclV1 {
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticLayoutIdentityV1, SemanticTypeIdentityV1, SemanticTypeLayoutV1,
    };
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([index; 32]),
        SemanticLayoutIdentityV1::from_sha256([index; 32]),
        SemanticTypeLayoutV1::new(Some(1), 1).unwrap(),
        shape,
    )
}
#[test]
fn expanded_type_nodes_count_array_multiplicity_and_shared_children() {
    use fe2o3_mir_model::semantic_mir_v1::SemanticAggregateTypeV1;
    let id = SemanticTypeIdV1::from_index;
    let check = |types: Vec<SemanticTypeDeclV1>| {
        let mut work = Work::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, BF16_TYPE_SCAN_STORAGE_V1 + FLOOR);
        budget.reserve_storage(FLOOR).unwrap();
        let result = bf16_bounded_types_v1(&types, &mut budget);
        assert_eq!(budget.storage(), FLOOR);
        result
    };
    // The array wrapper is itself a node: 1 + 255 is exactly the ceiling.
    assert!(
        check(vec![
            bounded_type(0, SemanticTypeShapeV1::Unit),
            bounded_type(
                1,
                SemanticTypeShapeV1::Array {
                    element: id(0),
                    length: 255
                }
            )
        ])
        .is_ok()
    );
    assert!(
        check(vec![
            bounded_type(0, SemanticTypeShapeV1::Unit),
            bounded_type(
                1,
                SemanticTypeShapeV1::Array {
                    element: id(0),
                    length: 256
                }
            )
        ])
        .is_err()
    );
    // Individually small arrays still multiply when nested: 1 + 16 * 16 = 257.
    assert!(
        check(vec![
            bounded_type(0, SemanticTypeShapeV1::Unit),
            bounded_type(
                1,
                SemanticTypeShapeV1::Array {
                    element: id(0),
                    length: 15
                }
            ),
            bounded_type(
                2,
                SemanticTypeShapeV1::Array {
                    element: id(1),
                    length: 16
                }
            )
        ])
        .is_err()
    );
    // A cache hit saves traversal work, never the expanded size of each use.
    assert!(
        check(vec![
            bounded_type(0, SemanticTypeShapeV1::Unit),
            bounded_type(
                1,
                SemanticTypeShapeV1::Array {
                    element: id(0),
                    length: 15
                }
            ),
            bounded_type(
                2,
                SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![id(1); 16]).unwrap())
            )
        ])
        .is_err()
    );
}
#[test]
fn bounded_type_cycles_and_oversized_arrays_refuse_before_planning() {
    let id = SemanticTypeIdV1::from_index;
    for types in [
        vec![bounded_type(
            0,
            SemanticTypeShapeV1::Array {
                element: id(0),
                length: 0,
            },
        )],
        vec![
            bounded_type(0, SemanticTypeShapeV1::Unit),
            bounded_type(
                1,
                SemanticTypeShapeV1::Array {
                    element: id(0),
                    length: u64::MAX,
                },
            ),
        ],
    ] {
        let mut work = Work::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, BF16_TYPE_SCAN_STORAGE_V1);
        assert!(bf16_bounded_types_v1(&types, &mut budget).is_err());
        assert_eq!(budget.storage(), 0);
    }
}
#[test]
fn memoized_type_scan_exact_and_one_short_work_are_original_ledger_charges() {
    let types = type_chain(8);
    let mut work = Work::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, BF16_TYPE_SCAN_STORAGE_V1);
    bf16_bounded_types_v1(&types, &mut budget).unwrap();
    let required = budget.work();
    assert!(required > 0);
    for (limit, accepted) in [(required, true), (required - 1, false)] {
        let mut work = Work::new(limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, BF16_TYPE_SCAN_STORAGE_V1 + FLOOR);
        budget.reserve_storage(FLOOR).unwrap();
        assert_eq!(bf16_bounded_types_v1(&types, &mut budget).is_ok(), accepted);
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.failed_work().is_some(), !accepted);
    }
}

#[test]
fn exact_transfer_retains_conservative_receipt_until_drop() {
    let envelope = bf16_envelope_arithmetic_v1(0, 0, 0, 0, 0).unwrap();
    let mut work = Work::new(envelope.work);
    let mut budget = ArgumentBudgetV1::new(&mut work, FLOOR + envelope.storage);
    budget.reserve_storage(FLOOR).unwrap();
    let dropped = Cell::new(false);
    let output = bf16_emission_scope_v1(&mut budget, |budget| {
        budget.reserve_storage(envelope.storage)?;
        budget.charge_work(envelope.work)?;
        Ok(Dropped(&dropped))
    })
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
    assert!(!dropped.get());
    budget.reserve_storage(envelope.storage).unwrap();
    assert_eq!(budget.peak_storage(), FLOOR + envelope.storage);
    assert_eq!(budget.work(), envelope.work);
    drop(output);
    assert!(dropped.get());
    budget.release_storage(envelope.storage).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}
#[test]
fn error_panic_and_sticky_denial_drop_outputs_before_scope_refund() {
    for which in 0..3 {
        let mut work = Work::new(1);
        let mut budget = ArgumentBudgetV1::new(&mut work, 128);
        budget.reserve_storage(FLOOR).unwrap();
        let dropped = Cell::new(false);
        let result: Result<Dropped<'_>, ProductionPreRankedKirErrorV1> =
            bf16_emission_scope_v1(&mut budget, |budget| {
                budget.reserve_storage(64)?;
                let output = Dropped(&dropped);
                match which {
                    0 => Err(bf16_emission_refusal_v1("injected").into()),
                    1 => panic!("inert emission unwind"),
                    _ => {
                        let _ = budget.charge_work(2);
                        Ok(output)
                    }
                }
            });
        assert!(result.is_err());
        assert!(dropped.get());
        assert_eq!(budget.storage(), FLOOR);
        if which == 2 {
            assert!(budget.failed_work().is_some());
        }
    }
}
#[test]
fn incoming_sticky_denial_does_not_enter_or_refund_caller_floor() {
    let mut work = Work::new(0);
    let mut budget = ArgumentBudgetV1::new(&mut work, 64);
    budget.reserve_storage(FLOOR).unwrap();
    assert!(budget.charge_work(1).is_err());
    let entered = Cell::new(false);
    assert!(
        bf16_emission_scope_v1(&mut budget, |_| {
            entered.set(true);
            Ok(())
        })
        .is_err()
    );
    assert!(!entered.get());
    assert_eq!(budget.storage(), FLOOR);
}
#[test]
fn uniform_abi_guard_has_full_wave_but_unknown_or_divergent_guard_refuses() {
    let mut work = Work::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    let mut graph = module();
    assert!(
        bf16_full_wave_v1(
            &graph,
            &graph.functions[0],
            &graph.functions[1],
            BlockId(1),
            0,
            launch(),
            &mut budget
        )
        .is_ok()
    );
    let body = graph.functions[0].body.as_mut().unwrap();
    let Some(Terminator::ConditionalBranch { condition, .. }) = body.blocks[0].terminator.as_mut()
    else {
        unreachable!()
    };
    *condition = ValueId(999);
    assert!(
        bf16_full_wave_v1(
            &graph,
            &graph.functions[0],
            &graph.functions[1],
            BlockId(1),
            0,
            launch(),
            &mut budget
        )
        .is_err()
    );
}
#[test]
fn exact_launch_missing_block_and_cycles_refuse() {
    let mut work = Work::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    let mut graph = module();
    for which in 0..4 {
        let mut contract = launch();
        match which {
            0 => contract.full_physical_workgroups = false,
            1 => contract.global_extents[0] = 63,
            2 => contract.workgroup_extents[0] = 32,
            _ => contract.launch_rank = 2,
        }
        assert!(
            bf16_full_wave_v1(
                &graph,
                &graph.functions[0],
                &graph.functions[1],
                BlockId(1),
                0,
                contract,
                &mut budget
            )
            .is_err()
        );
    }
    assert!(
        bf16_full_wave_v1(
            &graph,
            &graph.functions[0],
            &graph.functions[1],
            BlockId(91),
            0,
            launch(),
            &mut budget
        )
        .is_err()
    );
    graph.functions[1].body.as_mut().unwrap().blocks[0] = branch(0, 0, vec![]);
    assert!(bf16_acyclic_cfg_v1(&graph.functions[1], &mut budget).is_err());
}
#[test]
fn literal_identity_edges_preserve_ids_not_equal_values() {
    let mut target = block(
        1,
        Terminator::Return {
            values: vec![ValueId(11)],
        },
    );
    target
        .parameters
        .push(ValueDef::new(ValueId(11), Type::F32));
    let f = Function::internal_helper(
        "identity",
        Signature::new(vec![Type::F32], vec![Type::F32]),
        vec![ValueId(4)],
        vec![branch(0, 1, vec![ValueId(4)]), target],
    );
    let mut work = Work::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    assert_eq!(
        bf16_transport_origin_v1(&f, ValueId(11), &mut budget).unwrap(),
        ValueId(4)
    );
    assert_eq!(
        bf16_transport_origin_v1(&f, ValueId(12), &mut budget).unwrap(),
        ValueId(12)
    );
    let mut changed = f;
    changed
        .body
        .as_mut()
        .unwrap()
        .blocks
        .push(branch(2, 1, vec![ValueId(4)]));
    assert!(bf16_transport_origin_v1(&changed, ValueId(11), &mut budget).is_err());
}
#[test]
fn whole_graph_replay_rejects_same_valued_id_and_branch_and_extra_operations() {
    let original = module();
    let mut work = Work::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    assert!(bf16_exact_graph_v1(&original, &original, &mut budget).is_ok());
    for which in 0..3 {
        let mut changed = original.clone();
        let body = changed.functions[0].body.as_mut().unwrap();
        match which {
            0 => body.blocks[0].operations[0].results[0].id = ValueId(9),
            1 => {
                body.blocks[1].terminator = Some(Terminator::Branch {
                    target: BlockId(0),
                    arguments: vec![],
                })
            }
            _ => body.blocks[1].operations.push(Operation::effect_free(
                ValueDef::new(ValueId(99), Type::BOOL),
                OperationKind::Constant(Constant::Bool(true)),
            )),
        }
        assert!(bf16_exact_graph_v1(&original, &changed, &mut budget).is_err());
    }
}
#[test]
fn retained_source_assert_refuses_instead_of_eliding() {
    use fe2o3_mir_model::semantic_mir_v1::{SemanticConstantV1, SemanticControlFlowEdgeV1};
    let ty = SemanticTypeIdV1::from_index(0);
    let term = SemanticTerminatorKindV1::Assert {
        condition: SemanticOperandV1::Constant(SemanticConstantV1::new(
            ty,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1, 1).unwrap()),
        )),
        expected: true,
        message: SemanticAssertMessageV1::BoundsCheck {
            length: SemanticOperandV1::Constant(SemanticConstantV1::new(
                ty,
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1, 1).unwrap()),
            )),
            index: SemanticOperandV1::Constant(SemanticConstantV1::new(
                ty,
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0, 1).unwrap()),
            )),
        },
        target: SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::AssertSuccess,
            SemanticBlockIdV1::from_index(1),
        ),
        unwind: SemanticUnwindActionV1::Unreachable,
    };
    assert!(bf16_require_no_assert_v1(&term).is_err());
    assert!(bf16_require_no_assert_v1(&SemanticTerminatorKindV1::Return).is_ok());
}
#[test]
fn legacy_owner_receipts_measure_shared_header_without_nominal_admission() {
    use super::assert_origins_v1_tests::{Fixture, materialize};
    let mut work = Work::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let owner = materialize(Fixture::Literal(true), true, &mut budget);
    assert_eq!(budget.storage(), FLOOR);
    assert!(owner.bf16_call_instance_emission_v1().is_none());
    assert_eq!(
        owner.helper_source_policy_v1(),
        ProductionHelperSourcePolicyV1::RawEmpty
    );
    assert!(
        owner.helper_memory_storage_v1().retained_storage()
            >= std::mem::size_of::<SealedHelperMemoryV1>()
    );
    let storage = owner.retained_analysis_storage_v1();
    assert_eq!(
        storage,
        owner.executable_storage().retained_storage()
            + owner.assert_origin_storage().payload_storage()
            + owner.helper_memory_storage_v1().retained_storage()
    );
    budget.reserve_storage(storage).unwrap();
    assert!(
        owner
            .verify_bf16_nominal_equivalence_with_budget_v1(&mut budget)
            .is_err()
    );
    assert_eq!(budget.storage(), FLOOR + storage);
    drop(owner);
    budget.release_storage(storage).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}
