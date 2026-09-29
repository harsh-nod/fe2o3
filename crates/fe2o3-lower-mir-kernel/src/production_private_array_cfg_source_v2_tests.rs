use super::*;

#[derive(Clone, Copy, Debug)]
enum Shape {
    AcrossBlocks,
    Diamond { both: bool },
    Loop { initialized_before: bool },
    UnreachableKill,
    Kill(u8),
    MoveRead,
    ElementOnly(u64),
    UnknownWrite,
    AbortAfterRead,
}

fn go(target: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, target))
}
fn fork(first: u32, second: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::SwitchInt {
        discriminant: constant(ARRAY_SCALAR, 0, 4),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                edge(SemanticEdgeRoleV1::SwitchValue, first),
            )],
            edge(SemanticEdgeRoleV1::SwitchOtherwise, second),
        )
        .unwrap(),
    }
}
fn kill(kind: u8) -> SemanticStatementV1 {
    let local = SemanticLocalIdV1::from_index(1);
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        match kind {
            0 => SemanticStatementKindV1::StorageDead(local),
            1 => SemanticStatementKindV1::StorageLive(local),
            2 => SemanticStatementKindV1::Deinitialize(place(1, ARRAY_TYPE)),
            _ => SemanticStatementKindV1::Deinitialize(array_place(false)),
        },
    )
}
fn cfg_owner(shape: Shape) -> ProductionPreRankedKirOwnerV1 {
    try_cfg_owner(shape).unwrap()
}
fn try_cfg_owner(
    shape: Shape,
) -> Result<ProductionPreRankedKirOwnerV1, ProductionPreRankedKirErrorV1> {
    array_owner_with_cfg(ArrayCase::RetainedValueRead, false, |old| {
        let statements = old[0].statements();
        let initializer = statements[1].clone();
        // A genuine local-index write retains the array in mixed SSA. The
        // following lifetime reset means this fixture cannot count that write
        // as initialization of the subsequently observed generation.
        let mut entry = vec![statements[0].clone(), statements[2].clone(), kill(1)];
        let read = assignment(
            3,
            ARRAY_SCALAR,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(array_place(false))),
        );
        match shape {
            Shape::AcrossBlocks => {
                entry.push(initializer);
                vec![
                    block(211, entry, go(1)),
                    block(212, vec![read], SemanticTerminatorKindV1::Return),
                ]
            }
            Shape::Diamond { both } => vec![
                block(211, entry, fork(1, 2)),
                block(212, vec![initializer.clone()], go(3)),
                block(213, if both { vec![initializer] } else { vec![] }, go(3)),
                block(214, vec![read], SemanticTerminatorKindV1::Return),
            ],
            Shape::Loop { initialized_before } => {
                if initialized_before {
                    entry.push(initializer.clone());
                }
                vec![
                    block(211, entry, go(1)),
                    block(212, vec![read], fork(2, 3)),
                    block(213, vec![initializer], go(1)),
                    block(214, vec![], SemanticTerminatorKindV1::Return),
                ]
            }
            Shape::UnreachableKill => {
                entry.push(initializer);
                vec![
                    block(211, entry, go(1)),
                    block(212, vec![read], SemanticTerminatorKindV1::Return),
                    block(213, vec![kill(0)], go(1)),
                ]
            }
            Shape::Kill(kind) => {
                entry.push(initializer);
                vec![
                    block(211, entry, go(1)),
                    block(212, vec![kill(kind)], go(2)),
                    block(213, vec![read], SemanticTerminatorKindV1::Return),
                ]
            }
            Shape::MoveRead => {
                entry.push(initializer);
                let moved = assignment(
                    3,
                    ARRAY_SCALAR,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(array_place(false))),
                );
                vec![
                    block(211, entry, go(1)),
                    block(212, vec![moved], go(2)),
                    block(213, vec![read], SemanticTerminatorKindV1::Return),
                ]
            }
            Shape::ElementOnly(offset) => {
                let destination = SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(1),
                    vec![
                        SemanticProjectionV1::new(
                            SemanticProjectionKindV1::ConstantIndex {
                                offset,
                                minimum_length: 8,
                                from_end: false,
                            },
                            ARRAY_SCALAR,
                        )
                        .unwrap(),
                    ],
                    ARRAY_SCALAR,
                )
                .unwrap();
                entry.push(SemanticStatementV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        destination,
                        SemanticRvalueV1::new(
                            ARRAY_SCALAR,
                            SemanticRvalueKindV1::Use(constant(ARRAY_SCALAR, 7, 4)),
                        ),
                    )),
                ));
                vec![
                    block(211, entry, go(1)),
                    block(212, vec![read], SemanticTerminatorKindV1::Return),
                ]
            }
            Shape::UnknownWrite => {
                entry.push(initializer);
                let index = assignment(
                    2,
                    ARRAY_SCALAR,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Add,
                        left: constant(ARRAY_SCALAR, 0, 4),
                        right: constant(ARRAY_SCALAR, 1, 4),
                    },
                );
                vec![
                    block(211, entry, go(1)),
                    block(212, vec![index, statements[2].clone()], go(2)),
                    block(213, vec![read], SemanticTerminatorKindV1::Return),
                ]
            }
            Shape::AbortAfterRead => {
                entry.push(initializer);
                vec![
                    block(211, entry, go(1)),
                    block(212, vec![read], SemanticTerminatorKindV1::Abort),
                ]
            }
        }
    })
}

fn assert_whole_array_source_refusal(shape: Shape, expected_block: u32) {
    let error = try_cfg_owner(shape).unwrap_err();
    assert!(
        matches!(error,
            ProductionPreRankedKirErrorV1::Lowering(ProductionSemanticKirErrorV1::Unsupported {
                function: 0, block: Some(block), statement: Some(0),
                detail: "retained array read requires whole-array initialization",
            }) if block == expected_block),
        "{shape:?}: {error:?}"
    );
}

fn assert_partial_move_source_refusal(shape: Shape) {
    let error = try_cfg_owner(shape).unwrap_err();
    assert!(
        matches!(error,
            ProductionPreRankedKirErrorV1::Lowering(ProductionSemanticKirErrorV1::SemanticSsa(
                fe2o3_pliron::ProductionSemanticSsaErrorV1::PartialMove {
                    function, block: 2, statement: Some(0), local: 1,
                    violation: fe2o3_pliron::SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
                }
            )) if function == ARRAY_ROOT),
        "{shape:?}: {error:?}"
    );
}

fn recipe() -> fe2o3_pliron::ProductionRankedKernelV1 {
    // Only the source/physical initialization prerequisite is being tested;
    // this empty ranked candidate grants no complete ranked/native authority.
    fe2o3_pliron::ProductionRankedKernelV1::new(
        "private_array_relation",
        0,
        vec![fe2o3_pliron::ProductionRankedBlockV1::new(
            vec![],
            fe2o3_pliron::ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap()
}

fn relation<'a>(
    owner: &'a ProductionPreRankedKirOwnerV1,
    recipe: &'a fe2o3_pliron::ProductionRankedKernelV1,
    budget: &mut dyn CorrelationChargeV18,
) -> Result<PrivateArrayFinalRelationV1<'a>, ProductionMirPlironTranslationErrorV1> {
    let module = owner.executable().module();
    let instance = owner.correspondence.private_arrays.instances[0];
    PrivateArrayFinalRelationV1::new(
        module,
        &owner.correspondence,
        Some(owner.semantic_ssa().source_semantic()),
        instance.owner,
        instance.function,
        &module.functions[instance.module_function_ordinal],
        recipe,
        10_000,
        budget,
    )
    .map(|value| value.expect("genuine retained private array instance"))
}

fn read_results(
    relation: &PrivateArrayFinalRelationV1<'_>,
    budget: &mut dyn CorrelationChargeV18,
) -> Vec<bool> {
    relation
        .effects
        .iter()
        .filter(|effect| effect.access == PrivateArrayAccessV1::Read)
        .map(|effect| {
            let slot = relation
                .slots
                .iter()
                .find(|slot| slot.local == effect.local)
                .unwrap();
            let mut work = PrivateArrayCorrelationWorkV1 { budget };
            let offset = private_array_exact_relation_v1(
                relation.semantic.types(),
                relation.function,
                relation.body,
                relation.owner,
                relation.function_id,
                slot,
                effect,
                relation.max_operations,
                &mut work,
            )
            .unwrap_or_else(|error| match error {
                PrivateArrayRelationErrorV1::Work(error) => {
                    panic!("exact source array relation exhausted work: {error:?}")
                }
                PrivateArrayRelationErrorV1::InvalidSource(reason) => {
                    panic!("exact source array relation has invalid source: {reason}")
                }
                PrivateArrayRelationErrorV1::Incomplete(reason) => {
                    panic!("exact source array relation is incomplete: {reason}")
                }
                PrivateArrayRelationErrorV1::Mismatch(reason) => {
                    panic!("exact source array relation mismatched: {reason}")
                }
            });
            match relation.check_read_initialization(effect, slot, offset, &mut work) {
                Ok(()) => true,
                Err(ProductionMirPlironTranslationErrorV1::AllocationOriginMismatch {
                    location,
                }) => {
                    assert_eq!(
                        location,
                        FunctionOperationLocation::new(
                            effect.memory_location.block,
                            effect.memory_location.operation
                        )
                    );
                    false
                }
                Err(other) => panic!("unexpected initialization error: {other:?}"),
            }
        })
        .collect()
}

#[test]
fn private_array_cfg_genuine_cross_block_diamond_and_loop_initialization() {
    for shape in [
        Shape::AcrossBlocks,
        Shape::Diamond { both: true },
        Shape::Loop {
            initialized_before: true,
        },
        Shape::UnreachableKill,
    ] {
        let owner = cfg_owner(shape);
        owner.semantic_ssa().verify_replay().unwrap();
        let recipe = recipe();
        let mut budget = UnsupportedIndexCorrelationBudgetV1 {
            remaining: usize::MAX,
        };
        let relation = relation(&owner, &recipe, &mut budget).unwrap();
        assert_eq!(read_results(&relation, &mut budget), [true]);
        assert!(!owner.grants_artifact_or_launch_authority());
    }
}

#[test]
fn private_array_cfg_conditional_and_zero_trip_initialization_remain_refused() {
    // The production source gate rejects these before a final relation exists.
    // The actual solver's exhaustive path-state tests cover the same bad paths.
    for (shape, block) in [
        (Shape::Diamond { both: false }, 3),
        (
            Shape::Loop {
                initialized_before: false,
            },
            1,
        ),
    ] {
        assert_whole_array_source_refusal(shape, block);
    }
}

#[test]
fn private_array_cfg_storage_lifetime_deinitialize_and_moved_element_kill_history() {
    // Constant-index Deinitialize bypasses the static partial-move census;
    // the retained-array gate still rejects its later uninitialized read.
    for kind in [0, 1, 3] {
        assert_whole_array_source_refusal(Shape::Kill(kind), 2);
    }
    for shape in [Shape::Kill(2), Shape::MoveRead] {
        assert_partial_move_source_refusal(shape);
    }
}

#[test]
fn private_array_cfg_cached_read_is_bound_to_exact_source_row_and_element() {
    let owner = cfg_owner(Shape::AcrossBlocks);
    let recipe = recipe();
    let mut budget = UnsupportedIndexCorrelationBudgetV1 {
        remaining: usize::MAX,
    };
    let relation = relation(&owner, &recipe, &mut budget).unwrap();
    let read = relation
        .effects
        .iter()
        .find(|effect| effect.access == PrivateArrayAccessV1::Read)
        .unwrap();
    let slot = &relation.slots[0];
    let mut work = PrivateArrayCorrelationWorkV1 {
        budget: &mut budget,
    };
    relation
        .check_read_initialization(read, slot, 0, &mut work)
        .unwrap();
    assert!(matches!(
        relation.check_read_initialization(read, slot, 1, &mut work),
        Err(ProductionMirPlironTranslationErrorV1::AllocationOriginMismatch { .. })
    ));
    let equal_row = *read;
    assert!(matches!(
        relation.check_read_initialization(&equal_row, slot, 0, &mut work),
        Err(ProductionMirPlironTranslationErrorV1::AllocationOriginMismatch { .. })
    ));
}

#[test]
fn private_array_cfg_sparse_exact_element_and_unknown_write_overlap_are_not_whole_array_proofs() {
    // Even the observed element alone does not satisfy the earlier whole-array
    // source contract. A later unknown write starts from an admissible full array.
    for offset in [0, 1] {
        assert_whole_array_source_refusal(Shape::ElementOnly(offset), 1);
    }
    let owner = cfg_owner(Shape::UnknownWrite);
    let recipe = recipe();
    let mut budget = UnsupportedIndexCorrelationBudgetV1 {
        remaining: usize::MAX,
    };
    let relation = relation(&owner, &recipe, &mut budget).unwrap();
    assert_eq!(read_results(&relation, &mut budget), [false]);
}

#[test]
fn private_array_cfg_source_physical_role_and_owner_mismatch_remain_refused() {
    for change in 0..3 {
        let mut owner = cfg_owner(Shape::AcrossBlocks);
        let effects = &mut owner.correspondence.private_arrays.effects;
        let read = effects
            .iter_mut()
            .find(|effect| effect.access == PrivateArrayAccessV1::Read)
            .unwrap();
        match change {
            0 => read.semantic_block = 0,
            1 => read.owner = SemanticFunctionIdV1::from_index(1),
            _ => read.offset = ValueId(read.offset.0 + 1),
        }
        let recipe = recipe();
        let mut budget = UnsupportedIndexCorrelationBudgetV1 {
            remaining: usize::MAX,
        };
        assert!(matches!(
            relation(&owner, &recipe, &mut budget),
            Err(ProductionMirPlironTranslationErrorV1::KernelShape)
        ));
    }
}

#[test]
fn private_array_cfg_physical_paths_cannot_invent_or_omit_source_blocks() {
    let owner = cfg_owner(Shape::AcrossBlocks);
    let recipe = recipe();
    for change in 0..3 {
        let mut module = owner.executable().module().clone();
        let ordinal = owner.correspondence.private_arrays.instances[0].module_function_ordinal;
        let body = module.functions[ordinal].body.as_mut().unwrap();
        match change {
            0 => {
                body.blocks[0].terminator = Some(Terminator::Branch {
                    target: body.blocks[0].id,
                    arguments: vec![],
                })
            }
            1 => {
                body.blocks.pop();
            }
            _ => body.blocks.swap(0, 1),
        }
        let mut budget = UnsupportedIndexCorrelationBudgetV1 {
            remaining: usize::MAX,
        };
        assert!(matches!(
            PrivateArrayFinalRelationV1::new(
                &module,
                &owner.correspondence,
                Some(owner.semantic_ssa().source_semantic()),
                ARRAY_ROOT,
                ARRAY_ROOT,
                &module.functions[ordinal],
                &recipe,
                10_000,
                &mut budget
            ),
            Err(ProductionMirPlironTranslationErrorV1::KernelShape)
        ));
    }
}

#[test]
fn private_array_cfg_terminal_failure_sink_preserves_genuine_cross_block_initialization() {
    let owner = cfg_owner(Shape::AbortAfterRead);
    let recipe = recipe();
    let mut budget = UnsupportedIndexCorrelationBudgetV1 {
        remaining: usize::MAX,
    };
    let checked = relation(&owner, &recipe, &mut budget).unwrap();
    assert_eq!(read_results(&checked, &mut budget), [true]);
    let source_count = checked.function.blocks().len();
    let sink = checked.body.blocks.last().unwrap();
    assert_eq!(sink.id.0 as usize, source_count);
    assert!(matches!(sink.terminator, Some(Terminator::Unreachable)));
    assert!(
        owner
            .correspondence
            .synthetic_operation_spans
            .iter()
            .any(|span| {
                span.correspondence_owner == checked.owner
                    && span.semantic_function == checked.function_id
                    && span.kernel_ir_block == sink.id
                    && span.rule == SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap
                    && span.first_operation_ordinal == 0
                    && span.operation_count == 1
            })
    );
}

#[test]
fn private_array_cfg_terminal_sink_cannot_hide_memory_parameters_or_returning_edges() {
    let owner = cfg_owner(Shape::AbortAfterRead);
    let recipe = recipe();
    let ordinal = owner.correspondence.private_arrays.instances[0].module_function_ordinal;
    for change in 0..13 {
        let mut module = owner.executable().module().clone();
        let body = module.functions[ordinal].body.as_mut().unwrap();
        let sink_index = body.blocks.len() - 1;
        let sink_id = body.blocks[sink_index].id;
        match change {
            0 => body.blocks[sink_index].terminator = Some(Terminator::Return { values: vec![] }),
            1 => {
                body.blocks[sink_index].terminator = Some(Terminator::Branch {
                    target: body.blocks[1].id,
                    arguments: vec![],
                })
            }
            2 => body.blocks[sink_index].operations.push(Operation::new(
                vec![],
                OperationKind::Constant(Constant::Bool(false)),
            )),
            3 => {
                body.blocks[sink_index].operations[0] =
                    AmdGpuDiagnosticOperation::DebugTrap.operation(None)
            }
            4 => body.blocks[sink_index]
                .parameters
                .push(ValueDef::new(ValueId(9000), Type::BOOL)),
            5 => {
                body.blocks[0].terminator = Some(Terminator::Branch {
                    target: sink_id,
                    arguments: vec![],
                })
            }
            6 => {
                body.blocks[1].terminator = Some(Terminator::Branch {
                    target: sink_id,
                    arguments: vec![ValueId(9000)],
                })
            }
            7 => {
                body.blocks.pop();
            }
            8 => body.blocks.push(body.blocks[sink_index].clone()),
            9 => {
                let store = body.blocks[0]
                    .operations
                    .iter()
                    .find(|operation| matches!(operation.kind, OperationKind::Store { .. }))
                    .expect("genuine private initializer store")
                    .clone();
                body.blocks[sink_index].operations[0] = store;
            }
            10 => body.blocks[sink_index].operations[0]
                .results
                .push(ValueDef::new(ValueId(9000), Type::BOOL)),
            11 => {
                let OperationKind::Call { arguments, .. } =
                    &mut body.blocks[sink_index].operations[0].kind
                else {
                    panic!("trap call");
                };
                arguments.push(ValueId(9000));
            }
            _ => body.blocks[sink_index].id = BlockId(sink_id.0 + 1),
        }
        let mut budget = UnsupportedIndexCorrelationBudgetV1 {
            remaining: usize::MAX,
        };
        let checked = PrivateArrayFinalRelationV1::new(
            &module,
            &owner.correspondence,
            Some(owner.semantic_ssa().source_semantic()),
            ARRAY_ROOT,
            ARRAY_ROOT,
            &module.functions[ordinal],
            &recipe,
            10_000,
            &mut budget,
        );
        assert!(
            matches!(
                checked,
                Err(ProductionMirPlironTranslationErrorV1::KernelShape)
            ),
            "mutation {change}"
        );
    }
}

#[test]
fn private_array_cfg_existing_production_translation_consumes_cross_block_initialization() {
    use fe2o3_pliron::{
        ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedKernelV1,
        ProductionRankedTerminatorV1, ProductionSessionLimitsV1,
        compile_ranked_kernel_for_lowering_v1,
    };
    for shape in [Shape::AcrossBlocks, Shape::Kill(2)] {
        if matches!(shape, Shape::Kill(2)) {
            // This source is rejected by SSA before ranked materialization.
            assert_partial_move_source_refusal(shape);
            continue;
        }
        let owner = cfg_owner(shape);
        let layout = owner.source_launch().roots()[0].layout();
        let rows = &owner.correspondence.private_arrays;
        let slot = rows.slots[0];
        let view = ProductionRankedValueIdV1::new(0);
        let origin = (1u64 << 63) + u64::from(slot.local) + 1;
        let mut entry = vec![
            ProductionRankedOperationV1::ExecutionLayout {
                grid_identity: layout.grid_identity(),
                global_extents: layout.global_extents(),
                workgroup_extents: layout.workgroup_extents(),
                subgroup_size: layout.subgroup_size(),
                full_physical_workgroups: layout.full_physical_workgroups(),
            },
            ProductionRankedOperationV1::ViewInSpace {
                result: view,
                element_width: 32,
                writable: true,
                shape: vec![8],
                dynamic_extents: vec![],
                memory_space: dialect_kernel::MemorySpaceAttr::Private,
                allocation_origin: origin,
                noalias_class: origin,
            },
        ];
        let mut next = 1;
        let mut sources = Vec::new();
        let mut blocks = Vec::new();
        let source_blocks = &owner.semantic_ssa().source_semantic().functions()[0].blocks();
        for (block, source_block) in source_blocks.iter().enumerate() {
            let mut operations = if block == 0 {
                std::mem::take(&mut entry)
            } else {
                vec![]
            };
            for effect in rows
                .effects
                .iter()
                .filter(|effect| effect.semantic_block as usize == block)
            {
                let (ordinal, offset) = match effect.original_index {
                    PrivateArrayIndexV1::InitializerElement { component, .. } => {
                        (component, u64::from(component))
                    }
                    _ => (0, 0),
                };
                let index = ProductionRankedValueIdV1::new(next);
                next += 1;
                operations.push(ProductionRankedOperationV1::IndexConstant {
                    result: index,
                    value: offset,
                });
                sources.push(ProductionRankedAccessSourceV1::new(
                    effect.semantic_block,
                    Some(effect.semantic_statement),
                    ordinal,
                    block as u32,
                    operations.len() as u32,
                ));
                operations.push(ProductionRankedOperationV1::Access {
                    kind: match effect.access {
                        PrivateArrayAccessV1::Read => dialect_kernel::AccessKindAttr::Read,
                        PrivateArrayAccessV1::Write => dialect_kernel::AccessKindAttr::Write,
                    },
                    view: ProductionRankedValueV1::Local(view),
                    indices: vec![ProductionRankedValueV1::Local(index)],
                });
            }
            let terminator = match source_block.terminator().kind() {
                SemanticTerminatorKindV1::Goto(edge) => ProductionRankedTerminatorV1::Branch {
                    target: edge.target().index(),
                },
                SemanticTerminatorKindV1::Return => ProductionRankedTerminatorV1::Return,
                _ => panic!("straight-line fixture only"),
            };
            blocks.push(ProductionRankedBlockV1::new(operations, terminator));
        }
        let kernel = ProductionRankedKernelV1::new("private_array_relation", 0, blocks).unwrap();
        let lowering = compile_ranked_kernel_for_lowering_v1(
            ProductionConstructionV1::ranked_kernel("private_array_relation", kernel).unwrap(),
            ProductionSessionLimitsV1::default(),
        )
        .unwrap();
        assert!(lowering.all_mandatory_reports_are_clean());
        let result = validate_mir_pliron_translation_with_semantic_v1(
            Some(owner.semantic_ssa().source_semantic()),
            owner.executable().module(),
            &owner.correspondence,
            "private_array_relation",
            &lowering,
            &sources,
            &[],
            10_000,
        );
        match shape {
            Shape::AcrossBlocks => {
                result.unwrap();
                let mut changed = owner.executable().module().clone();
                let ordinal = rows.instances[0].module_function_ordinal;
                let body = changed.functions[ordinal].body.as_mut().unwrap();
                let write = rows
                    .effects
                    .iter()
                    .find(|effect| effect.access == PrivateArrayAccessV1::Write)
                    .unwrap();
                let extra = body.blocks[write.memory_location.block_ordinal].operations
                    [write.memory_location.operation]
                    .clone();
                let last = body.blocks.last_mut().unwrap();
                let extra_location = FunctionOperationLocation::new(last.id, last.operations.len());
                last.operations.push(extra);
                assert!(matches!(validate_mir_pliron_translation_with_semantic_v1(
                    Some(owner.semantic_ssa().source_semantic()), &changed, &owner.correspondence,
                    "private_array_relation", &lowering, &sources, &[], 10_000),
                    Err(ProductionMirPlironTranslationErrorV1::UnattributedExecutableEffect { location })
                        if location == extra_location));

                let mut changed = owner.executable().module().clone();
                let body = changed.functions[ordinal].body.as_mut().unwrap();
                let writes = rows
                    .effects
                    .iter()
                    .filter(|effect| effect.access == PrivateArrayAccessV1::Write)
                    .take(2)
                    .collect::<Vec<_>>();
                assert_eq!(
                    writes[0].memory_location.block_ordinal,
                    writes[1].memory_location.block_ordinal
                );
                body.blocks[writes[0].memory_location.block_ordinal]
                    .operations
                    .swap(
                        writes[0].memory_location.operation,
                        writes[1].memory_location.operation,
                    );
                assert!(matches!(
                    validate_mir_pliron_translation_with_semantic_v1(
                        Some(owner.semantic_ssa().source_semantic()),
                        &changed,
                        &owner.correspondence,
                        "private_array_relation",
                        &lowering,
                        &sources,
                        &[],
                        10_000
                    ),
                    Err(ProductionMirPlironTranslationErrorV1::KernelShape)
                ));
            }
            _ => unreachable!(),
        }
    }
}

fn cleanup() -> ScopedSourceCleanupV29 {
    ScopedSourceCleanupV29 {
        denied: std::cell::Cell::new(false),
        fault: std::cell::RefCell::new(None),
        fault_storage: std::cell::Cell::new(None),
        fault_skip: std::cell::Cell::new(0),
    }
}

#[test]
fn private_array_cfg_existing_outer_scope_drops_owned_rows_before_error_and_panic_refund() {
    let owner = cfg_owner(Shape::AcrossBlocks);
    let recipe = recipe();
    for unwind in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let completed = std::cell::Cell::new(false);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_canonical_call_scratch_v1(&mut budget, |budget| {
                let cleanup = cleanup();
                let ledger = CorrelationLedgerV18::new(budget, &cleanup);
                let mut charge = SourceCorrelationChargeV18 {
                    ledger: &ledger,
                    finite: UnsupportedIndexCorrelationBudgetV1 {
                        remaining: usize::MAX,
                    },
                    finite_denied: false,
                };
                let relation = relation(&owner, &recipe, &mut charge)
                    .map_err(ProductionSemanticKirErrorV1::MirPlironTranslation)?;
                assert_eq!(read_results(&relation, &mut charge), [true]);
                assert!(ledger.budget.borrow().storage() > FLOOR);
                completed.set(true);
                if unwind {
                    std::panic::resume_unwind(Box::new(719u32));
                }
                Err::<(), _>(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            })
        }));
        assert!(completed.get());
        assert_eq!(budget.storage(), FLOOR);
        assert!(budget.work() > 2);
        if unwind {
            assert_eq!(*outcome.unwrap_err().downcast::<u32>().unwrap(), 719);
        } else {
            assert!(matches!(
                outcome.unwrap(),
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            ));
        }
    }
}

#[test]
fn private_array_cfg_shared_scratch_keeps_first_error_and_refuses_foreign_slot() {
    let mut original_work = CanonicalKernelIrWorkBudgetV1::new(0);
    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(100);
    let mut original = ArgumentBudgetV1::new(&mut original_work, 1000);
    let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, 1000);
    original.reserve_storage(FLOOR).unwrap();
    foreign.reserve_storage(FLOOR).unwrap();
    let first_cleanup = cleanup();
    let ledger = CorrelationLedgerV18::new(&mut original, &first_cleanup);
    let mut charge = SourceCorrelationChargeV18 {
        ledger: &ledger,
        finite: UnsupportedIndexCorrelationBudgetV1 {
            remaining: usize::MAX,
        },
        finite_denied: false,
    };
    assert!(charge.charge_many(1).is_none());
    let first = ledger.failure.get().expect("original work denial");
    assert!(charge.reserve_private_array_scratch(1).is_none());
    assert!(charge.release_private_array_scratch(0).is_none());
    assert_eq!(ledger.failure.get(), Some(first));
    drop(charge);
    drop(ledger);
    let next_cleanup = cleanup();
    let ledger = CorrelationLedgerV18::new(&mut original, &next_cleanup);
    let mut charge = SourceCorrelationChargeV18 {
        ledger: &ledger,
        finite: UnsupportedIndexCorrelationBudgetV1 {
            remaining: usize::MAX,
        },
        finite_denied: false,
    };
    let original_slot = std::mem::replace(&mut *ledger.budget.borrow_mut(), &mut foreign);
    assert!(charge.reserve_private_array_scratch(1).is_none());
    assert_eq!(ledger.failure.get(), Some(ArgumentResourceV1::Accounting));
    let _foreign_slot = std::mem::replace(&mut *ledger.budget.borrow_mut(), original_slot);
    assert!(charge.reserve_private_array_scratch(0).is_none());
    assert_eq!(ledger.failure.get(), Some(ArgumentResourceV1::Accounting));
    assert!(next_cleanup.is_denied());
}

#[test]
fn private_array_cfg_shared_ledger_exact_and_one_short_cumulative_resources() {
    for shape in [Shape::Diamond { both: true }, Shape::AbortAfterRead] {
        let owner = cfg_owner(shape);
        let recipe = recipe();
        let run = |work_limit: usize, storage_limit: usize| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
            budget.reserve_storage(FLOOR).unwrap();
            let cleanup = cleanup();
            let ledger = CorrelationLedgerV18::new(&mut budget, &cleanup);
            let mut charge = SourceCorrelationChargeV18 {
                ledger: &ledger,
                finite: UnsupportedIndexCorrelationBudgetV1 {
                    remaining: usize::MAX,
                },
                finite_denied: false,
            };
            let outcome = (|| {
                let first = relation(&owner, &recipe, &mut charge)?;
                let retained_first = ledger.budget.borrow().storage();
                assert!(retained_first > FLOOR);
                let second = relation(&owner, &recipe, &mut charge)?;
                assert_eq!(
                    ledger.budget.borrow().storage() - retained_first,
                    retained_first - FLOOR
                );
                drop((first, second));
                Ok::<_, ProductionMirPlironTranslationErrorV1>(())
            })();
            let failure = ledger.failure.get();
            if outcome.is_err() {
                assert!(failure.is_some());
                assert!(charge.charge_many(0).is_none());
                assert!(charge.reserve_private_array_scratch(0).is_none());
                assert_eq!(ledger.failure.get(), failure);
            }
            drop(charge);
            drop(ledger);
            let used = budget.work();
            let peak = budget.peak_storage();
            let storage = budget.storage();
            // The enclosing correlation scope owns failed/retained scratch until
            // every relation has dropped. Restore only that same-ledger floor.
            budget.release_storage(storage - FLOOR).unwrap();
            assert_eq!(budget.storage(), FLOOR);
            (outcome.is_ok(), used, peak, failure)
        };
        let measured = run(usize::MAX, usize::MAX);
        assert!(measured.0);
        assert!(run(measured.1, measured.2).0);
        let short_work = run(measured.1 - 1, measured.2);
        assert!(!short_work.0);
        assert!(short_work.3.is_some());
        let short_storage = run(measured.1, measured.2 - 1);
        assert!(!short_storage.0);
        assert!(short_storage.3.is_some());
    }
}
