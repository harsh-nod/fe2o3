// Child of the Phase1b test module. Module/relation clones below are explicitly
// caller-owned test subjects, not production source evidence or paid pipeline
// copies. Every fresh canonical output adopts/releases its exact V18 receipt.
use super::*;

use TileScalarPieceV29 as Piece;
use TileScalarSourceV29 as Source;
use TileScalarStageV29 as Stage;

fn graph_case(
    case: SourceCase,
    check: impl FnOnce(&ScopedTileScalarCandidateV29, &mut ArgumentBudgetV1<'_>),
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
    let candidate = scalar_candidate(case, ScopedTileOrderV29::Blocked, &mut budget);
    candidate.replay_with_budget(&mut budget).unwrap();
    check(&candidate, &mut budget);
    drop_scalar_candidate(candidate, &mut budget);
    assert_eq!(budget.storage(), SCHEDULE_FLOOR);
    budget.release_storage(SCHEDULE_FLOOR).unwrap();
}

fn original_operation(
    candidate: &ScopedTileScalarCandidateV29,
    point: TileScalarPointV29,
) -> &Operation {
    &candidate.input.pending.pending_module().functions[point.function]
        .body
        .as_ref()
        .unwrap()
        .blocks[point.block]
        .operations[point.operation]
}
fn source_rows(
    candidate: &ScopedTileScalarCandidateV29,
    select: impl Fn(&Operation) -> bool,
) -> Vec<TileScalarOriginV29> {
    candidate
        .relations
        .origins
        .iter()
        .filter(|row| match row.source {
            Source::Operation(point) => select(original_operation(candidate, point)),
            _ => false,
        })
        .copied()
        .collect()
}
fn load_rows(candidate: &ScopedTileScalarCandidateV29) -> Vec<TileScalarOriginV29> {
    source_rows(candidate, |operation| {
        matches!(
            operation.kind,
            OperationKind::Execution(kir::ExecutionOperationV15::MaskedTileLoadU32 { .. })
        )
    })
}
fn parts_rows(candidate: &ScopedTileScalarCandidateV29) -> Vec<TileScalarOriginV29> {
    source_rows(candidate, |operation| {
        matches!(
            operation.kind,
            OperationKind::Execution(kir::ExecutionOperationV15::FragmentIntoPartsU32 { .. })
        )
    })
}

fn fresh_graph_subject(
    candidate: &ScopedTileScalarCandidateV29,
    budget: &mut ArgumentBudgetV1<'_>,
    accepted: bool,
    change: impl FnOnce(&mut Module, &mut TileScalarRelationsV29),
) {
    let mut module = candidate.output.module().clone();
    let mut relations = TileScalarRelationsV29 {
        origins: candidate.relations.origins.clone(),
        pieces: candidate.relations.pieces.clone(),
    };
    change(&mut module, &mut relations);
    let floor = budget.storage();
    let (verified, receipt) =
        kir::VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            &module,
            candidate.input.pending.inner.limits.storage_layout_limits(),
            budget,
        )
        .expect("the negative subject must first pass fresh V18 admission");
    assert_eq!(budget.storage(), floor, "receipt is not already adopted");
    let paid = receipt.retained_storage();
    budget.reserve_storage(paid).unwrap();
    drop(module);
    let replay_floor = budget.storage();
    let result = scoped_tile_attempt_v29(budget, |budget| {
        replay_tile_scalar_graph_v29(&candidate.input, &verified, &relations, budget)
    });
    assert_eq!(
        budget.storage(),
        replay_floor,
        "replay scratch must be released"
    );
    assert_eq!(
        result,
        if accepted {
            Ok(())
        } else {
            Err(ScopedTileFailureKindV29::ReplayMismatch)
        }
    );
    drop(verified);
    budget.release_storage(paid).unwrap();
    assert_eq!(
        budget.storage(),
        floor,
        "original candidate and caller reservations survive"
    );
}

#[test]
fn fresh_v18_accepts_wrong_predicate_polarity_but_source_replay_refuses() {
    graph_case(SourceCase::Repeated, |candidate, budget| {
        let load = load_rows(candidate)[0];
        fresh_graph_subject(candidate, budget, false, |module, relations| {
            let point = operation_points(relations, load)
                .into_iter()
                .find(|&point| {
                    matches!(
                        output_operation(module, point).kind,
                        OperationKind::Compare {
                            predicate: ComparePredicate::LessThan,
                            ..
                        }
                    )
                })
                .unwrap();
            let OperationKind::Compare { predicate, .. } =
                &mut output_operation_mut(module, point).kind
            else {
                unreachable!()
            };
            *predicate = ComparePredicate::GreaterThanOrEqual;
        });
    });
}

#[test]
fn tile_replay_rejects_unused_table_extension_and_keeps_the_original_owner() {
    graph_case(SourceCase::Slots, |candidate, budget| {
        let identity = *candidate.output.identity();
        let module = candidate.output.module() as *const Module;
        fresh_graph_subject(candidate, budget, true, |_, _| {});
        fresh_graph_subject(candidate, budget, false, |module, _| {
            module.storage_layouts.push(kir::StorageLayoutV1 {
                size: 4,
                alignment: 4,
                kind: kir::StorageLayoutKindV1::Scalar(ScalarType::U32),
            });
        });
        assert_eq!(candidate.output.identity(), &identity);
        assert_eq!(candidate.output.module() as *const Module, module);
        candidate.replay_with_budget(budget).unwrap();
    });
}

#[test]
fn fresh_v18_accepts_removed_overflow_gate_but_source_replay_refuses() {
    graph_case(SourceCase::Repeated, |candidate, budget| {
        let load = load_rows(candidate)[0];
        fresh_graph_subject(candidate, budget, false, |module, relations| {
            let points = operation_points(relations, load);
            let true_value = points
                .iter()
                .find_map(|&point| {
                    let operation = output_operation(module, point);
                    matches!(
                        operation.kind,
                        OperationKind::Constant(Constant::Bool(true))
                    )
                    .then(|| operation.results[0].id)
                })
                .unwrap();
            let point = points
                .into_iter()
                .find(|&point| {
                    matches!(
                        output_operation(module, point).kind,
                        OperationKind::Binary {
                            op: BinaryOp::BitAnd,
                            ..
                        }
                    )
                })
                .unwrap();
            let OperationKind::Binary { rhs, .. } = &mut output_operation_mut(module, point).kind
            else {
                unreachable!()
            };
            assert_ne!(*rhs, true_value);
            *rhs = true_value;
        });
    });
}

#[test]
fn swapping_both_edge_target_and_argument_list_preserves_v18_not_tile_semantics() {
    graph_case(SourceCase::Repeated, |candidate, budget| {
        let load = load_rows(candidate)[0];
        fresh_graph_subject(candidate, budget, false, |module, relations| {
            let block = relations.pieces[load.first..load.first + load.count]
                .iter()
                .find_map(|piece| {
                    if piece.stage != Stage::Conditional {
                        return None;
                    }
                    match piece.piece {
                        Piece::Terminator { function, block } => Some((function, block)),
                        _ => None,
                    }
                })
                .unwrap();
            let terminator = module.functions[block.0].body.as_mut().unwrap().blocks[block.1]
                .terminator
                .as_mut()
                .unwrap();
            let Terminator::ConditionalBranch {
                then_target,
                then_arguments,
                else_target,
                else_arguments,
                ..
            } = terminator
            else {
                panic!("load gate");
            };
            assert_eq!((then_arguments.len(), else_arguments.len()), (0, 2));
            std::mem::swap(then_target, else_target);
            std::mem::swap(then_arguments, else_arguments);
        });
    });
}

fn selected_value(operation: &Operation) -> ValueId {
    let OperationKind::Select {
        true_value,
        false_value,
        ..
    } = operation.kind
    else {
        panic!("Parts select");
    };
    assert_eq!(true_value, false_value);
    true_value
}
fn replace_selected_value(operation: &mut Operation, replacement: ValueId) {
    let OperationKind::Select {
        true_value,
        false_value,
        ..
    } = &mut operation.kind
    else {
        panic!("Parts select");
    };
    *true_value = replacement;
    *false_value = replacement;
}

#[test]
fn later_parts_cannot_reuse_a_dominating_earlier_load_snapshot() {
    graph_case(SourceCase::Repeated, |candidate, budget| {
        let parts = parts_rows(candidate);
        assert_eq!(parts.len(), 2, "real repeated helper calls");
        let points = [
            operation_points(&candidate.relations, parts[0])[0],
            operation_points(&candidate.relations, parts[1])[0],
        ];
        assert_eq!(points[0].function, points[1].function);
        let graph = candidate.output.module();
        let function = &graph.functions[points[0].function];
        let body = function.body.as_ref().unwrap();
        // Bounded, caller-owned test oracle, not a production-ledger allocation.
        let cfg = kir::analyze_control_flow(function).unwrap();
        let floor = budget.storage();
        let index = AssertGraphIndexV1::build_functions(&graph.functions, true, budget).unwrap();
        let mut mutation = None;
        for (donor, target) in [(points[0], points[1]), (points[1], points[0])] {
            let wrong = selected_value(output_operation(graph, donor));
            assert_ne!(wrong, selected_value(output_operation(graph, target)));
            let definition = index
                .definition(
                    kir::CanonicalKirFunctionCoordinateV1(u32::try_from(donor.function).unwrap()),
                    wrong,
                    budget,
                )
                .unwrap();
            let kir::CanonicalKirDefinitionCoordinateV1::BlockArgument { block, .. } =
                definition.coordinate
            else {
                panic!("Load snapshots are actual generated join parameters");
            };
            assert_eq!(usize::try_from(block.function.0).unwrap(), donor.function);
            if cfg.dominates(
                body.blocks[usize::try_from(block.block).unwrap()].id,
                body.blocks[target.block].id,
            ) {
                assert!(mutation.replace((target, wrong)).is_none());
            }
        }
        index.release(budget).unwrap();
        assert_eq!(budget.storage(), floor);
        let (target, wrong) = mutation.expect("one genuine earlier snapshot dominates later Parts");
        fresh_graph_subject(candidate, budget, false, |module, _| {
            replace_selected_value(output_operation_mut(module, target), wrong);
        });
    });
}

#[test]
fn equally_typed_parts_masks_cannot_swap_components() {
    graph_case(SourceCase::Repeated, |candidate, budget| {
        let parts = parts_rows(candidate)[0];
        fresh_graph_subject(candidate, budget, false, |module, relations| {
            let points = operation_points(relations, parts);
            assert_eq!(points.len(), 4);
            let masks = [points[2], points[3]];
            for point in masks {
                assert_eq!(output_operation(module, point).results[0].ty, Type::BOOL);
            }
            let a = selected_value(output_operation(module, masks[0]));
            let b = selected_value(output_operation(module, masks[1]));
            assert_ne!(a, b);
            replace_selected_value(output_operation_mut(module, masks[0]), b);
            replace_selected_value(output_operation_mut(module, masks[1]), a);
        });
    });
}

fn remove_pool_piece(relations: &mut TileScalarRelationsV29, index: usize) {
    let owning: Vec<_> = relations
        .origins
        .iter()
        .enumerate()
        .filter(|(_, row)| row.first <= index && index < row.first + row.count)
        .map(|(ordinal, _)| ordinal)
        .collect();
    assert_eq!(owning.len(), 1);
    relations.origins[owning[0]].count -= 1;
    relations.pieces.remove(index);
    for row in &mut relations.origins {
        if row.first > index {
            row.first -= 1;
        }
    }
}

#[test]
fn duplicate_physical_result_ownership_is_not_certified_by_valid_graph() {
    graph_case(SourceCase::Repeated, |candidate, budget| {
        fresh_graph_subject(candidate, budget, false, |_, relations| {
            let indices: Vec<_> = relations
                .pieces
                .iter()
                .enumerate()
                .filter_map(|(index, piece)| {
                    matches!(piece.piece, Piece::Result { .. }).then_some(index)
                })
                .take(2)
                .collect();
            assert_eq!(indices.len(), 2);
            assert_ne!(
                relations.pieces[indices[0]].piece,
                relations.pieces[indices[1]].piece
            );
            relations.pieces[indices[1]].piece = relations.pieces[indices[0]].piece;
        });
    });
}

#[test]
fn checked_arithmetic_overflow_result_must_have_its_own_origin_piece() {
    graph_case(SourceCase::Repeated, |candidate, budget| {
        fresh_graph_subject(candidate, budget, false, |module, relations| {
            let index = relations
                .pieces
                .iter()
                .position(|piece| match piece.piece {
                    Piece::Result {
                        operation,
                        result: 1,
                    } => matches!(
                        output_operation(module, operation).kind,
                        OperationKind::Binary {
                            op: BinaryOp::Checked(_),
                            ..
                        }
                    ),
                    _ => false,
                })
                .unwrap();
            remove_pool_piece(relations, index);
        });
    });
}

#[test]
fn physical_join_parameter_and_operation_result_categories_cannot_be_exchanged() {
    graph_case(SourceCase::Repeated, |candidate, budget| {
        fresh_graph_subject(candidate, budget, false, |module, relations| {
            let parameter = relations
                .pieces
                .iter()
                .position(|piece| {
                    piece.stage == Stage::Join
                        && matches!(piece.piece, Piece::BlockParameter { parameter: 0, .. })
                })
                .unwrap();
            let result = relations
                .pieces
                .iter()
                .position(|piece| match piece.piece {
                    Piece::Result {
                        operation,
                        result: 0,
                    } => {
                        output_operation(module, operation).results[0].ty
                            == Type::Scalar(ScalarType::U32)
                    }
                    _ => false,
                })
                .unwrap();
            let saved = relations.pieces[parameter].piece;
            relations.pieces[parameter].piece = relations.pieces[result].piece;
            relations.pieces[result].piece = saved;
        });
    });
}

#[test]
fn final_join_does_not_own_another_original_terminator() {
    graph_case(SourceCase::Repeated, |candidate, budget| {
        fresh_graph_subject(candidate, budget, false, |_, relations| {
            let moved = relations
                .origins
                .iter()
                .find(|row| match row.source {
                    Source::Terminator { function, block } => {
                        match relations.pieces[row.first].piece {
                            Piece::Terminator {
                                function: physical_function,
                                block: physical_block,
                            } => function == physical_function && block != physical_block,
                            _ => false,
                        }
                    }
                    _ => false,
                })
                .copied()
                .unwrap();
            assert_eq!(moved.count, 1);
            let other = relations
                .origins
                .iter()
                .find(|row| {
                    row.first != moved.first && matches!(row.source, Source::Terminator { .. })
                })
                .copied()
                .unwrap();
            assert_eq!(other.count, 1);
            let saved = relations.pieces[moved.first].piece;
            relations.pieces[moved.first].piece = relations.pieces[other.first].piece;
            relations.pieces[other.first].piece = saved;
        });
    });
}

// Preserve real source ownership while adding duplicate successor occurrences.
fn duplicate_successor_source_owner() -> ProductionSemanticSsaOwnerV1 {
    let template = source_owner(SourceCase::BranchParts);
    let semantic = template.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let helper = functions[3].clone();
    let mut blocks = helper.blocks().to_vec();
    let SemanticTerminatorKindV1::SwitchInt {
        discriminant,
        targets,
    } = blocks[2].terminator().kind()
    else {
        panic!("source split");
    };
    assert_eq!(targets.values().len(), 1);
    let target = targets.values()[0].edge().target();
    let mut values = targets.values().to_vec();
    values.push(SemanticSwitchTargetV1::new(
        1,
        SemanticControlFlowEdgeV1::new(SemanticEdgeRoleV1::SwitchValue, target),
    ));
    let kind = SemanticTerminatorKindV1::SwitchInt {
        discriminant: discriminant.clone(),
        targets: SemanticSwitchTargetsV1::new(
            values,
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::SwitchOtherwise,
                targets.otherwise().target(),
            ),
        )
        .unwrap(),
    };
    blocks[2] = SemanticBasicBlockV1::new(
        blocks[2].identity(),
        blocks[2].source(),
        blocks[2].statements().to_vec(),
        SemanticTerminatorV1::new(blocks[2].terminator().source(), kind),
    )
    .unwrap();
    functions[3] = SemanticFunctionDeclV1::new(
        helper.identity(),
        helper.role(),
        helper.item_definition_identity(),
        helper.monomorphization_identity(),
        helper.generic_type_arguments_identity(),
        helper.const_generic_arguments_identity(),
        helper.source(),
        helper.abi().clone(),
        helper.locals().to_vec(),
        helper.entry(),
        blocks,
    )
    .unwrap();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        semantic.allocations().to_vec(),
        semantic.statics().to_vec(),
        semantic.vtables().to_vec(),
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn duplicate_successor_candidate(
    budget: &mut ArgumentBudgetV1<'_>,
) -> ScopedTileScalarCandidateV29 {
    scalar_candidate_from_source_v29(
        duplicate_successor_source_owner,
        ScopedTileOrderV29::Blocked,
        64,
        budget,
    )
}

#[test]
fn duplicate_target_successor_occurrences_cannot_collapse_to_one_origin() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
    let candidate = duplicate_successor_candidate(&mut budget);
    candidate.replay_with_budget(&mut budget).unwrap();
    let mut selected = None;
    for (function, source) in candidate
        .input
        .pending
        .pending_module()
        .functions
        .iter()
        .enumerate()
    {
        let Some(body) = &source.body else { continue };
        for (block, item) in body.blocks.iter().enumerate() {
            let mut edges = Vec::new();
            item.terminator
                .as_ref()
                .unwrap()
                .try_visit_edges_v1(|target, arguments| {
                    edges.push((target, arguments));
                    Ok::<_, std::convert::Infallible>(())
                })
                .unwrap();
            if edges.len() == 3 && edges[0] == edges[1] {
                selected = Some((function, block));
                break;
            }
        }
        if selected.is_some() {
            break;
        }
    }
    let (function, block) =
        selected.expect("source lowering must retain both genuine edge occurrences");
    fresh_graph_subject(&candidate, &mut budget, false, |_, relations| {
        let source = Source::Edge {
            function,
            block,
            edge: 1,
        };
        let row_index = relations
            .origins
            .iter()
            .position(|row| row.source == source)
            .unwrap();
        let row = relations.origins[row_index];
        assert_eq!(row.count, 1);
        remove_pool_piece(relations, row.first);
        assert_eq!(relations.origins[row_index].count, 0);
        relations.origins.remove(row_index);
    });
    drop_scalar_candidate(candidate, &mut budget);
    assert_eq!(budget.storage(), SCHEDULE_FLOOR);
    budget.release_storage(SCHEDULE_FLOOR).unwrap();
}

#[test]
fn graph_only_distinct_erased_sources_share_a_gap_without_physical_ownership_collision() {
    use kir::{ExecutionOperationV15 as Ex, ExecutionRoleV15 as Role};
    let context = ValueId(10);
    let nominal = |id, role, operation| {
        Operation::new(
            vec![ValueDef::new(ValueId(id), Type::Execution(role))],
            OperationKind::Execution(operation),
        )
    };
    let mut block = BasicBlock::new(BlockId(7));
    block.operations = vec![
        nominal(10, Role::Context, Ex::ContextIssue),
        nominal(11, Role::Workgroup, Ex::WorkgroupDerive { context }),
        Operation::new(
            vec![],
            OperationKind::Execution(Ex::ScopeEnd {
                workgroup: ValueId(11),
                discarded: vec![],
            }),
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut original = Module::new("raw_graph_coincident_erased_anchors");
    original.functions.push(Function::kernel_entry(
        "entry",
        kir::Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    original.kernels.push(kir::Kernel::new(
        "entry",
        "entry",
        kir::LaunchDomain::D1 {
            x: kir::LaunchExtent::Static(64),
        },
    ));
    // This is raw graph-boundary evidence, never a fabricated Pending/source owner.
    max_id_graph_boundary_tests::graph_only_emit_and_check(&original, |output, relations| {
        let mut expected = original.clone();
        expected.functions[0].body.as_mut().unwrap().blocks[0]
            .operations
            .clear();
        assert_eq!(output, &expected);
        assert_eq!(relations.origins.len(), 7);
        assert_eq!(relations.pieces.len(), 7);
        let gap = TileScalarPointV29 {
            function: 0,
            block: 0,
            operation: 0,
        };
        for operation in 0..3 {
            let source = Source::Operation(TileScalarPointV29 { operation, ..gap });
            let rows: Vec<_> = relations
                .origins
                .iter()
                .filter(|row| row.source == source)
                .collect();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].first, operation * 2 + 1);
            assert_eq!(rows[0].count, 1);
            assert_eq!(
                relations.pieces[rows[0].first],
                TileScalarTaggedPieceV29 {
                    piece: Piece::Anchor(gap),
                    component: None,
                    stage: Stage::Erased,
                }
            );
        }
        let erased: Vec<_> = relations
            .pieces
            .iter()
            .filter_map(|piece| match piece.piece {
                Piece::ErasedValue(id) => Some(id),
                _ => None,
            })
            .collect();
        assert_eq!(erased, [ValueId(10), ValueId(11)]);
        assert_eq!(
            relations.pieces[0].piece,
            Piece::Block {
                function: 0,
                block: 0
            }
        );
        assert_eq!(
            relations.pieces[6].piece,
            Piece::Terminator {
                function: 0,
                block: 0
            }
        );
        for ordinal in [0, 6] {
            let piece = relations.pieces[ordinal];
            assert_eq!((piece.stage, piece.component), (Stage::Preserved, None));
        }
    });
}

// Proposal: include as a child of materialization_tests; no production hooks.
// Evidence is admitted inert Semantic MIR -> SSA -> owned Pending -> Candidate,
// not rustc authentication, GPU execution, or fabricated source custody.

fn preservation_source_owner() -> ProductionSemanticSsaOwnerV1 {
    let template = source_owner(SourceCase::Repeated);
    let semantic = template.source_semantic();
    assert_eq!(semantic.functions().len(), 4);
    assert_eq!(semantic.callables().len(), 9);
    let boolean = SemanticTypeIdV1::from_index(
        u32::try_from(
            semantic
                .types()
                .iter()
                .position(|ty| {
                    matches!(
                        ty.shape(),
                        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool)
                    )
                })
                .unwrap(),
        )
        .unwrap(),
    );
    let source_contract = semantic.functions()[0]
        .kernel_entry()
        .unwrap()
        .source_contract();
    let ordinary = |tag: u8, symbol: &[u8], looping: bool| {
        let blocks = if looping {
            vec![
                block(
                    tag + 4,
                    vec![],
                    SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::Goto,
                        SemanticBlockIdV1::from_index(1),
                    )),
                ),
                block(
                    tag + 5,
                    vec![],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: SemanticOperandV1::Copy(place(1, U32)),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(
                                0,
                                SemanticControlFlowEdgeV1::new(
                                    SemanticEdgeRoleV1::SwitchValue,
                                    SemanticBlockIdV1::from_index(3),
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
                block(
                    tag + 6,
                    vec![],
                    SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::Goto,
                        SemanticBlockIdV1::from_index(1),
                    )),
                ),
                block(tag + 7, vec![], SemanticTerminatorKindV1::Return),
            ]
        } else {
            // Same genuine diagnostic-declaration producer as ModuleFixture::Mixed.
            vec![
                block(
                    tag + 4,
                    vec![],
                    SemanticTerminatorKindV1::Assert {
                        condition: SemanticOperandV1::Constant(SemanticConstantV1::new(
                            boolean,
                            SemanticConstantValueV1::Scalar(
                                SemanticScalarValueV1::new(1, 1).unwrap(),
                            ),
                        )),
                        expected: true,
                        message: SemanticAssertMessageV1::NullPointerDereference,
                        target: SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::AssertSuccess,
                            SemanticBlockIdV1::from_index(1),
                        ),
                        unwind: SemanticUnwindActionV1::Unreachable,
                    },
                ),
                block(tag + 5, vec![], SemanticTerminatorKindV1::Return),
            ]
        };
        function(
            tag,
            SemanticFunctionRoleV1::KernelRoot,
            abi(tag + 1, true, &[U32]),
            vec![
                local(tag + 2, UNIT, SemanticLocalRoleV1::Return),
                local(tag + 3, U32, SemanticLocalRoleV1::Argument(0)),
            ],
            blocks,
        )
        .with_kernel_entry(SemanticKernelEntryV1::new(
            SemanticLinkSymbolV1::new(symbol.to_vec()).unwrap(),
            SemanticKernelBindingIdentityV1::from_sha256([tag + 8; 32]),
            source_contract,
        ))
    };
    let mut functions = semantic.functions().to_vec();
    // Defined callable rows MUST remain the complete function-index prefix.
    // Two ordinary roots and a second tile root shift non-body callables by three.
    for prior in &mut functions {
        let blocks = prior
            .blocks()
            .iter()
            .map(|old| {
                let terminator = match old.terminator().kind() {
                    SemanticTerminatorKindV1::Call(call) => {
                        let index = call.callee().index();
                        SemanticTerminatorKindV1::Call(
                            SemanticDirectCallV1::new_callable(
                                SemanticCallableIdV1::from_index(if index >= 4 {
                                    index + 3
                                } else {
                                    index
                                }),
                                call.arguments().to_vec(),
                                call.destination().cloned(),
                                call.unwind(),
                            )
                            .unwrap(),
                        )
                    }
                    SemanticTerminatorKindV1::TailCall(_) => panic!("unexpected fixture tail call"),
                    other => other.clone(),
                };
                SemanticBasicBlockV1::new(
                    old.identity(),
                    old.source(),
                    old.statements().to_vec(),
                    SemanticTerminatorV1::new(old.terminator().source(), terminator),
                )
                .unwrap()
            })
            .collect();
        let mut replacement = SemanticFunctionDeclV1::new(
            prior.identity(),
            prior.role(),
            prior.item_definition_identity(),
            prior.monomorphization_identity(),
            prior.generic_type_arguments_identity(),
            prior.const_generic_arguments_identity(),
            prior.source(),
            prior.abi().clone(),
            prior.locals().to_vec(),
            prior.entry(),
            blocks,
        )
        .unwrap();
        if let Some(entry) = prior.kernel_entry() {
            replacement = replacement.with_kernel_entry(entry.clone());
        }
        *prior = replacement;
    }
    functions.push(ordinary(140, b"z_ordinary_cycle", true));
    functions.push(ordinary(150, b"a_ordinary_assert", false));
    let tile = &functions[0];
    assert_eq!(tile.entry(), SemanticBlockIdV1::from_index(0));
    let second_tile = function(
        170,
        SemanticFunctionRoleV1::KernelRoot,
        tile.abi().clone(),
        tile.locals().to_vec(),
        tile.blocks().to_vec(),
    )
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"zz_tile_second".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([178; 32]),
        source_contract,
    ));
    functions.push(second_tile);
    let mut callables: Vec<_> = (0..functions.len())
        .map(|index| {
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(index as u32))
        })
        .collect();
    callables.extend_from_slice(&semantic.callables()[4..]);
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        semantic.allocations().to_vec(),
        semantic.statics().to_vec(),
        semantic.vtables().to_vec(),
        functions,
        callables,
        vec![
            ROOT,
            SemanticFunctionIdV1::from_index(4),
            SemanticFunctionIdV1::from_index(5),
            SemanticFunctionIdV1::from_index(6),
        ],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn preservation_prepared(
    order: ScopedTileOrderV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> PreparedScopedTileSourceV29 {
    use kernel_argument_abi_v18::tests::{fixture_descriptor_ownership_v18, FixtureKernelAbiV18};
    let floor = budget.storage();
    let projected = fixture_descriptor_ownership_v18(preservation_source_owner());
    let owner = fixture_descriptor_ownership_v18(preservation_source_owner());
    let semantic = projected.source_semantic();
    let launch_inputs: Vec<_> = semantic
        .roots()
        .iter()
        .enumerate()
        .map(|(ordinal, root)| {
            let entry = semantic.functions()[root.index() as usize]
                .kernel_entry()
                .unwrap();
            ProductionSourceLaunchRootInputV1::new(
                std::str::from_utf8(entry.export_symbol().as_bytes()).unwrap(),
                *entry.kernel_binding_identity().as_bytes(),
                ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [ordinal as u32 + 1, 1, 1]),
            )
        })
        .collect();
    let launch =
        ProductionSourceLaunchRosterV1::try_new(owner.source_semantic(), &launch_inputs).unwrap();
    let second_function = SemanticFunctionIdV1::from_index(6);
    let second_source = &semantic.functions()[second_function.index() as usize];
    let SemanticTerminatorKindV1::Call(second_issuer) =
        second_source.blocks()[0].terminator().kind()
    else {
        panic!("second tile root issuer");
    };
    let SemanticTerminatorKindV1::Call(second_helper) =
        second_source.blocks()[1].terminator().kind()
    else {
        panic!("second tile root provider");
    };
    let mut second = root_input(&projected);
    second.root = second_function;
    second.root_identity = second_source.identity();
    second.issuer = second_issuer.callee();
    second.helper_arguments = second_helper.arguments();
    assert_eq!(second_helper.callee().index(), HELPER.index());
    let roots = [root_input(&projected), second];
    assert!(roots.iter().all(|root| root.issuer.index() == 7));
    let workgroup = semantic.functions()[2].abi().source_input_types()[0];
    let mut classes =
        vec![ProductionScopeCallableCandidateV29::Ordinary; semantic.callables().len()];
    classes[1] = ProductionScopeCallableCandidateV29::Provider {
        function: HELPER,
        identity: semantic.functions()[1].identity(),
    };
    let SemanticTerminatorKindV1::Call(derive) =
        semantic.functions()[1].blocks()[0].terminator().kind()
    else {
        panic!("derive call");
    };
    assert_eq!(derive.callee().index(), 8);
    classes[derive.callee().index() as usize] = ProductionScopeCallableCandidateV29::Derive {
        binding: SemanticFunctionIdentityV1::from_sha256([121; 32]),
        operation: SemanticCompilerIntrinsicIdentityV1::from_sha256([121; 32]),
        context: CONTEXT,
        workgroup,
    };
    let events = [
        (
            ROOT,
            1,
            0,
            ProductionScopeEventKindV29::Call {
                callee: SemanticCallableIdV1::from_index(1),
                kind: ProductionScopeCallKindV29::Provider,
            },
        ),
        (
            HELPER,
            0,
            semantic.functions()[1].blocks()[0].statements().len(),
            ProductionScopeEventKindV29::Call {
                callee: derive.callee(),
                kind: ProductionScopeCallKindV29::Derive,
            },
        ),
        (
            HELPER,
            1,
            0,
            ProductionScopeEventKindV29::Call {
                callee: SemanticCallableIdV1::from_index(2),
                kind: ProductionScopeCallKindV29::Ordinary,
            },
        ),
        (HELPER, 2, 0, ProductionScopeEventKindV29::Return),
        (
            second_function,
            1,
            0,
            ProductionScopeEventKindV29::Call {
                callee: second_helper.callee(),
                kind: ProductionScopeCallKindV29::Provider,
            },
        ),
    ]
    .map(
        |(function, block, statement_count, kind)| ProductionScopeEventCandidateV29 {
            function,
            block: SemanticBlockIdV1::from_index(block),
            statement_count,
            kind,
        },
    );
    let profile = FixtureKernelAbiV18::new(&owner);
    let profile_roots = profile.roots();
    let pending = ProductionPendingScopedSourceOwnerV29::try_materialize_with_kernel_abi_budget_v18(
        owner,
        launch,
        ProductionExecutionSourceInputV29 {
            semantic_sha256: projected.source_semantic_sha256(),
            roots: &roots,
            classes: &classes,
            events: &events,
        },
        ProductionKernelArgumentAbiInputV18 { roots: &profile_roots },
        ProductionSemanticKirLimitsV1::default(),
        budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), floor + pending.adopted_storage());
    assert_eq!(pending.inner.pending.roots.len(), 4);
    for ordinal in [0, 3] {
        let root = &pending.inner.pending.roots[ordinal];
        assert_eq!(root.function_ordinal, ordinal);
        assert!(root.requires_context_issue);
    }
    for root in &pending.inner.pending.roots[1..3] {
        assert!(!root.requires_context_issue);
        assert!(root.insertions.is_empty());
        assert!(!root.sidecars.rows.is_empty());
        for sidecar in &root.sidecars.rows {
            assert!(
                sidecar
                    .lifecycle_events
                    .as_ref()
                    .expect("ordinary source retains a real empty lifecycle receipt")
                    .rows
                    .is_empty()
            );
        }
    }
    let mut donor = Some((pending, ScopedTileScheduleInputV29 { order }));
    let prepared = prepare_scoped_tile_source_v29(&mut donor, budget).unwrap();
    assert!(donor.is_none());
    assert_eq!(budget.storage(), floor + prepared.adopted_storage());
    prepared
}

#[test]
fn admitted_mixed_roots_keep_ordinary_cycle_import_and_non_topological_splicing() {
    for order in [ScopedTileOrderV29::Blocked, ScopedTileOrderV29::Striped] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
        budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
        let prepared = preservation_prepared(order, &mut budget);
        let original = prepared.pending.pending_module();
        assert_eq!(original.kernels.len(), 4);
        assert_eq!(
            original.functions.len(),
            5,
            "four roots and one diagnostic import"
        );
        assert_eq!(original.functions[0].id.as_str(), "lifecycle_fixture");
        assert_eq!(original.functions[1].id.as_str(), "z_ordinary_cycle");
        assert_eq!(original.functions[2].id.as_str(), "a_ordinary_assert");
        assert_eq!(original.functions[3].id.as_str(), "zz_tile_second");
        assert_eq!(
            original.functions[4].role,
            fe2o3_kernel_ir::FunctionRole::ExternalImport
        );
        assert!(original.functions[4].body.is_none());
        for ordinal in [0, 3] {
            let tile = &original.functions[ordinal];
            let tile_edges = preservation_edges(tile);
            assert!(!preservation_has_cycle(&tile_edges));
            assert!(
                tile_edges
                    .iter()
                    .enumerate()
                    .any(|(from, edges)| { edges.iter().any(|&to| to < from) })
            );
            let physical_loads = tile
                .body
                .as_ref()
                .unwrap()
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter(|operation| {
                    matches!(
                        operation.kind,
                        OperationKind::Execution(
                            kir::ExecutionOperationV15::MaskedTileLoadU32 { .. }
                        )
                    )
                })
                .count();
            assert_eq!(physical_loads, 2);
            assert_eq!(
                prepared
                    .selections
                    .iter()
                    .filter(|row| row.root == ordinal)
                    .count(),
                2
            );
        }
        assert!(preservation_has_cycle(&preservation_edges(
            &original.functions[1]
        )));
        assert!(!preservation_has_cycle(&preservation_edges(
            &original.functions[2]
        )));
        assert_eq!(
            prepared.selections.len(),
            4,
            "both real helper Loads from each tile root are selected"
        );
        max_id_graph_boundary_tests::assert_missing_tile_root(&prepared, 3, &mut budget);
        let candidate = materialize_scoped_tile_source_v29(prepared, &mut budget)
            .unwrap_or_else(|failure| panic!("mixed module: {:?}", failure.summary));
        candidate.replay_with_budget(&mut budget).unwrap();
        let original = candidate.input.pending.pending_module();
        let output = candidate.output.module();
        assert_eq!(output.kernels, original.kernels);
        assert_eq!(output.functions.len(), original.functions.len());
        for ordinal in [1, 2, 4] {
            assert_eq!(
                output.functions[ordinal], original.functions[ordinal],
                "ordinary cycle/assertion/import must be field-exact, not reconstructed"
            );
        }
        for ordinal in [1, 2] {
            let rows: Vec<_> = candidate
                .relations
                .origins
                .iter()
                .filter(|row| preservation_source_function(row.source) == ordinal)
                .collect();
            assert!(!rows.is_empty());
            for row in rows {
                assert_eq!(row.count, 1);
                let piece = candidate.relations.pieces[row.first];
                assert_eq!(piece.stage, TileScalarStageV29::Preserved);
                assert_eq!(piece.component, None);
            }
        }
        assert!(
            candidate
                .relations
                .origins
                .iter()
                .all(|row| preservation_source_function(row.source) != 4),
            "bodyless declarations have no invented SSA parameter or block rows"
        );
        for ordinal in [0, 3] {
            assert_ne!(output.functions[ordinal], original.functions[ordinal]);
            let output_edges = preservation_edges(&output.functions[ordinal]);
            assert!(!preservation_has_cycle(&output_edges));
            assert!(
                output_edges
                    .iter()
                    .enumerate()
                    .any(|(from, edges)| { edges.iter().any(|&to| to < from) })
            );
        }
        drop_scalar_candidate(candidate, &mut budget);
        assert_eq!(budget.storage(), SCHEDULE_FLOOR);
        budget.release_storage(SCHEDULE_FLOOR).unwrap();
    }
}

#[path = "production_scoped_tile_transport_v29_tests.rs"]
mod transport_tests;
