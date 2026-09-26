use super::*;
use fe2o3_mir_model::semantic_mir_v1::{SemanticSwitchTargetV1, SemanticSwitchTargetsV1};

#[path = "production_source_reference_root_aggregate_v29_tests.rs"]
mod root_aggregate_representation_tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LogicalFlow {
    WholeMove,
    PartialMove,
    Reinitialize,
    PartialHelper,
    Loop,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LogicalCheck {
    OriginalStates,
    ZeroWidth,
    WrongProjection,
    CrossInstance,
}

fn field(index: u32, ty: SemanticTypeIdV1) -> SemanticProjectionV1 {
    SemanticProjectionV1::new(SemanticProjectionKindV1::Field(index), ty).unwrap()
}

fn logical_owner(
    input: Input,
    nested: bool,
    flow: LogicalFlow,
) -> Result<ProductionSemanticSsaOwnerV1, fe2o3_pliron::ProductionSemanticSsaErrorV1> {
    let original = nominal_owner(input, false);
    let source_owner = original.source_semantic();
    let mut types = source_owner.types().to_vec();
    let mixed = source_owner.functions()[0].locals()[1].ty();
    let selected = if nested {
        let size = types[mixed.index() as usize].layout().size_bytes().unwrap();
        let alignment = types[mixed.index() as usize].layout().alignment_bytes();
        aggregate(
            &mut types,
            vec![mixed, mixed],
            vec![0, size],
            size * 2,
            alignment,
            SemanticBackendReprV1::Memory { sized: true },
            None,
        )
    } else {
        mixed
    };
    let abi = |tag, kernel| {
        SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([tag; 32]),
            source_owner.target_layout_identity(),
            if kernel {
                SemanticCanonAbiV1::GpuKernel
            } else {
                SemanticCanonAbiV1::Rust
            },
            if kernel {
                SemanticExternAbiV1::GpuKernel
            } else {
                SemanticExternAbiV1::Rust
            },
            false,
            false,
            1,
            vec![SemanticAbiArgumentV1::source(value_abi(&types, selected))],
            ignored(UNIT),
        )
        .unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
        .unwrap()
    };
    let mut projections = if nested {
        vec![field(0, mixed)]
    } else {
        vec![]
    };
    projections.push(field(1, U32));
    let scalar_place =
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), projections, U32).unwrap();
    let move_field = || {
        assign(
            place(2, U32),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(scalar_place.clone())),
        )
    };
    let invoke = |target| call(1, vec![SemanticOperandV1::Move(place(1, selected))], target);
    let goto = |target| {
        SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::Goto,
            SemanticBlockIdV1::from_index(target),
        ))
    };
    let blocks = match flow {
        LogicalFlow::WholeMove => vec![
            block(220, vec![], invoke(1)),
            block(221, vec![], SemanticTerminatorKindV1::Return),
        ],
        LogicalFlow::PartialMove => vec![
            block(220, vec![move_field()], goto(1)),
            block(221, vec![], SemanticTerminatorKindV1::Return),
        ],
        LogicalFlow::Reinitialize => vec![
            block(
                220,
                vec![
                    move_field(),
                    assign(
                        scalar_place.clone(),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(2, U32))),
                    ),
                ],
                goto(1),
            ),
            block(221, vec![], invoke(2)),
            block(222, vec![], SemanticTerminatorKindV1::Return),
        ],
        LogicalFlow::PartialHelper => vec![
            block(220, vec![move_field()], invoke(1)),
            block(221, vec![], SemanticTerminatorKindV1::Return),
        ],
        LogicalFlow::Loop => vec![
            block(
                220,
                vec![assign(
                    place(2, U32),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(scalar_place.clone())),
                )],
                goto(1),
            ),
            block(
                221,
                vec![],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(place(2, U32)),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            SemanticControlFlowEdgeV1::new(
                                SemanticEdgeRoleV1::SwitchValue,
                                SemanticBlockIdV1::from_index(1),
                            ),
                        )],
                        SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::SwitchOtherwise,
                            SemanticBlockIdV1::from_index(2),
                        ),
                    )
                    .unwrap(),
                },
            ),
            block(222, vec![], invoke(3)),
            block(223, vec![], SemanticTerminatorKindV1::Return),
        ],
    };
    let root = function(
        202,
        SemanticFunctionRoleV1::KernelRoot,
        abi(203, true),
        vec![
            local(210, UNIT, SemanticLocalRoleV1::Return),
            local(211, selected, SemanticLocalRoleV1::Argument(0)),
            local(212, U32, SemanticLocalRoleV1::Temporary),
            local(213, CONTEXT, SemanticLocalRoleV1::Temporary),
        ],
        blocks,
    )
    .with_kernel_entry(source_owner.functions()[0].kernel_entry().unwrap().clone());
    let helper = function(
        206,
        SemanticFunctionRoleV1::InternalHelper,
        abi(205, false),
        vec![
            local(230, UNIT, SemanticLocalRoleV1::Return),
            local(231, selected, SemanticLocalRoleV1::Argument(0)),
        ],
        vec![block(240, vec![], SemanticTerminatorKindV1::Return)],
    );
    let (functions, callables) = if flow == LogicalFlow::PartialMove {
        // This source deliberately stops after its partial move; its module has
        // no helper call, so it must not retain an unreachable helper definition.
        (vec![root], vec![SemanticCallableDeclV1::defined(ROOT)])
    } else {
        (vec![root, helper], source_owner.callables().to_vec())
    };
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source_owner.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        vec![ROOT],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
}

fn logical_case(
    input: Input,
    nested: bool,
    flow: LogicalFlow,
    check: LogicalCheck,
) -> Result<(), ProductionSemanticKirErrorV1> {
    const LIMIT: usize = 20_000_000;
    let mut owner = logical_owner(input, nested, flow).unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let capture = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    let demands =
        source_storage_demands_v29::SourceStorageDemandsV29::collect(&owner, &mut budget).unwrap();
    let mut layouts = source_storage_v29::SourceStorageLayoutsV29::new_with_limits(
        &owner,
        demands.types(&owner, &mut budget).unwrap(),
        ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
        &mut budget,
    )
    .unwrap();
    let whole_type = owner.source_semantic().functions()[0].locals()[1].ty();
    assert!(layouts.row_for(&owner, whole_type, &mut budget).is_err());
    assert!(layouts.row_for(&owner, CONTEXT, &mut budget).is_err());
    let table = budget.storage();
    let mut called = false;
    let result =
        with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
            let floor = budget.storage();
            let result = source_storage_v29::with_source_storage_root_v29(
                &mut layouts,
                instances,
                budget,
                |plan, root, budget| {
                    called = true;
                    plan.check_owner(instances, budget)?;
                    let initial = plan.entries[instances.root().index()].unwrap();
                    let initial = &plan.states[initial];
                    let argument = plan.storage_snapshots[initial[1]
                        .storage
                        .expect("mixed original argument needs a logical snapshot")];
                    let temporary = plan.storage_snapshots[initial[3]
                        .storage
                        .expect("nominal ZST local needs a logical snapshot")];
                    assert!(root.snapshot_initialized(argument, &[], budget)?);
                    assert!(!root.snapshot_initialized(temporary, &[], budget)?);
                    let child = instances
                        .calls(instances.root())
                        .unwrap()
                        .first()
                        .and_then(|call| call.child());
                    if let Some(child) = child {
                        let entry = plan.entries[child.index()].unwrap();
                        let parameter =
                            plan.storage_snapshots[plan.states[entry][1].storage.unwrap()];
                        assert!(root.snapshot_initialized(parameter, &[], budget)?);
                        if check == LogicalCheck::CrossInstance {
                            root.join_snapshots(argument, parameter, budget)?;
                            panic!("different original instances joined");
                        }
                    }
                    if check == LogicalCheck::WrongProjection {
                        root.snapshot_initialized(temporary, &[field(0, U32)], budget)?;
                        panic!("changed nominal projection was accepted");
                    }
                    if check == LogicalCheck::ZeroWidth {
                        assert_eq!(
                            owner.source_semantic().types()[CONTEXT.index() as usize]
                                .layout()
                                .size_bytes(),
                            Some(0)
                        );
                        let initialized = root.mutate_snapshot(
                            temporary,
                            &[],
                            source_storage_v29::SourceStorageRootMutationV29::Initialize,
                            budget,
                        )?;
                        assert!(root.snapshot_initialized(initialized, &[], budget)?);
                        let moved = root.mutate_snapshot(
                            initialized,
                            &[],
                            source_storage_v29::SourceStorageRootMutationV29::Deinitialize,
                            budget,
                        )?;
                        assert!(!root.snapshot_initialized(moved, &[], budget)?);
                        for joined in [
                            root.join_snapshots(initialized, moved, budget)?,
                            root.join_snapshots(moved, initialized, budget)?,
                        ] {
                            assert!(!root.snapshot_initialized(joined, &[], budget)?);
                        }
                        let copied =
                            root.snapshot_copy_from(moved, &[], initialized, &[], budget)?;
                        assert!(root.snapshot_initialized(copied, &[], budget)?);
                        let restarted = root.snapshot_lifetime(copied, true, budget)?;
                        assert!(!root.snapshot_initialized(restarted, &[], budget)?);
                        // These are initialization facts, never a Context issue or a
                        // nominal identity equation. No KIR value is produced here.
                        let before = root.snapshot_statistics(budget)?;
                        for _ in 0..8 {
                            assert_eq!(
                                root.mutate_snapshot(
                                    initialized,
                                    &[],
                                    source_storage_v29::SourceStorageRootMutationV29::Initialize,
                                    budget
                                )?,
                                initialized
                            );
                            assert_eq!(
                                root.snapshot_copy_from(moved, &[], initialized, &[], budget)?,
                                copied
                            );
                            assert_eq!(
                                root.join_snapshots(initialized, initialized, budget)?,
                                initialized
                            );
                        }
                        assert_eq!(root.snapshot_statistics(budget)?, before);
                    }
                    let next = plan
                        .blocks
                        .iter()
                        .find(|row| row.instance == instances.root() && row.block.index() == 1)
                        .unwrap();
                    let after = plan.storage_snapshots[plan.states[next.entry][1].storage.unwrap()];
                    match flow {
                        LogicalFlow::WholeMove => {
                            assert!(!root.snapshot_initialized(after, &[], budget)?)
                        }
                        LogicalFlow::Reinitialize | LogicalFlow::Loop => {
                            assert!(root.snapshot_initialized(after, &[], budget)?)
                        }
                        LogicalFlow::PartialMove => {
                            assert!(!root.snapshot_initialized(after, &[], budget)?);
                            let types = owner.source_semantic().types();
                            let mixed = if nested {
                                let SemanticTypeShapeV1::Aggregate(fields) =
                                    types[whole_type.index() as usize].shape()
                                else {
                                    panic!("nested source type changed")
                                };
                                fields.fields()[0]
                            } else {
                                whole_type
                            };
                            let SemanticTypeShapeV1::Aggregate(fields) =
                                types[mixed.index() as usize].shape()
                            else {
                                panic!("mixed source type changed")
                            };
                            let mut scalar_path = if nested {
                                vec![field(0, mixed)]
                            } else {
                                vec![]
                            };
                            let mut nominal_path = scalar_path.clone();
                            scalar_path.push(field(1, U32));
                            nominal_path.push(field(0, fields.fields()[0]));
                            assert!(!root.snapshot_initialized(after, &scalar_path, budget)?);
                            assert!(root.snapshot_initialized(after, &nominal_path, budget)?);
                            if nested {
                                assert!(root.snapshot_initialized(
                                    after,
                                    &[field(1, mixed)],
                                    budget
                                )?);
                            }
                        }
                        LogicalFlow::PartialHelper => {
                            panic!("partial whole-value argument reached callback")
                        }
                    }
                    Ok(())
                },
            );
            assert_eq!(budget.storage(), floor);
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result)
        })
        .unwrap();
    assert_eq!(budget.storage(), table);
    let release = layouts.release(&mut budget);
    assert_eq!(release.is_err(), result.is_err());
    demands.discard(&mut budget).unwrap();
    drop(owner);
    budget.release_storage(capture.retained_storage()).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(
        called,
        flow != LogicalFlow::PartialHelper,
        "{flow:?}, nested={nested}: {result:?}"
    );
    result
}

#[test]
fn actual_mixed_nominal_c1_entries_and_whole_moves_retain_logical_state_without_rows() {
    for input in [Input::CapturedContext, Input::CapturedWorkgroupReference] {
        for nested in [false, true] {
            logical_case(
                input,
                nested,
                LogicalFlow::WholeMove,
                LogicalCheck::OriginalStates,
            )
            .unwrap();
        }
    }
}

#[test]
fn nominal_zero_width_state_never_comes_from_empty_bytes_or_missing_rows() {
    logical_case(
        Input::CapturedContext,
        false,
        LogicalFlow::WholeMove,
        LogicalCheck::ZeroWidth,
    )
    .unwrap();
}

#[test]
fn actual_mixed_partial_move_preserves_nominal_and_nested_siblings() {
    for nested in [false, true] {
        logical_case(
            Input::CapturedContext,
            nested,
            LogicalFlow::PartialMove,
            LogicalCheck::OriginalStates,
        )
        .unwrap();
    }
}

#[test]
fn actual_mixed_reinitialization_restores_whole_helper_transfer() {
    for nested in [false, true] {
        logical_case(
            Input::CapturedContext,
            nested,
            LogicalFlow::Reinitialize,
            LogicalCheck::OriginalStates,
        )
        .unwrap();
    }
}

#[test]
fn actual_mixed_no_change_loop_reuses_logical_snapshots_before_helper_transfer() {
    for nested in [false, true] {
        logical_case(
            Input::CapturedContext,
            nested,
            LogicalFlow::Loop,
            LogicalCheck::ZeroWidth,
        )
        .unwrap();
    }
}

#[test]
fn actual_mixed_partial_whole_argument_rejects_before_callback() {
    for nested in [false, true] {
        // The original SSA owner rejects this source before a C1 callback can
        // exist. Require that exact admission boundary rather than bypass it.
        assert!(matches!(
            logical_owner(Input::CapturedContext, nested, LogicalFlow::PartialHelper),
            Err(fe2o3_pliron::ProductionSemanticSsaErrorV1::PartialMove {
                function: ROOT,
                block: 0,
                statement: None,
                local: 1,
                violation: fe2o3_pliron::SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
            })
        ));
    }
}

#[test]
fn logical_nominal_snapshot_queries_reject_changed_type_path_and_instance() {
    for check in [LogicalCheck::WrongProjection, LogicalCheck::CrossInstance] {
        assert!(
            logical_case(Input::CapturedContext, false, LogicalFlow::WholeMove, check).is_err()
        );
    }
}
