use super::*;

const PAIR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);

#[derive(Clone, Copy, Debug)]
pub(super) enum InactiveCase {
    Move,
    Deinitialize,
    Reinitialize,
    ReadRemoved,
    CopyPartial,
    MovePartial,
    Loop,
}

fn selected(local: u32) -> SemanticPlaceV1 {
    projected(
        local,
        &[
            (SemanticProjectionKindV1::Field(0), CAPTURE),
            (SemanticProjectionKindV1::Field(0), REFERENCE),
        ],
    )
}

fn jump(target: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
        SemanticEdgeRoleV1::Goto,
        SemanticBlockIdV1::from_index(target),
    ))
}

fn branch(reverse: bool, left: u32, right: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::SwitchInt {
        discriminant: SemanticOperandV1::Copy(place(3, WORD)),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::SwitchValue,
                    SemanticBlockIdV1::from_index(if reverse { right } else { left }),
                ),
            )],
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::SwitchOtherwise,
                SemanticBlockIdV1::from_index(if reverse { left } else { right }),
            ),
        )
        .unwrap(),
    }
}

pub(super) fn inactive_owner(case: InactiveCase, reverse: bool) -> ProductionSemanticSsaOwnerV1 {
    try_inactive_owner(case, reverse).unwrap()
}

fn try_inactive_owner(
    case: InactiveCase,
    reverse: bool,
) -> Result<ProductionSemanticSsaOwnerV1, fe2o3_pliron::ProductionSemanticSsaErrorV1> {
    try_owner_with(Case::Shared, |types, functions| {
        assert_eq!(types.len(), PAIR.index() as usize);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([93; 32]),
            SemanticLayoutIdentityV1::from_sha256([93; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(16),
                8,
                SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(
                SemanticAggregateTypeV1::new(vec![CAPTURE, REFERENCE]).unwrap(),
            ),
        ));
        let old = &functions[2];
        let mut locals = old.locals().to_vec();
        locals.push(local(93, PAIR, SemanticLocalRoleV1::Temporary));
        locals.push(local(94, REFERENCE, SemanticLocalRoleV1::Temporary));
        locals.push(local(95, PAIR, SemanticLocalRoleV1::Temporary));
        let mut initial: Vec<_> = old.blocks()[0]
            .statements()
            .iter()
            .filter(|statement| {
                !matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment)
                if assignment.destination().local().index() == 0)
            })
            .cloned()
            .collect();
        initial.push(assign(
            place(5, PAIR),
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Tuple,
                    vec![
                        SemanticOperandV1::Copy(place(1, CAPTURE)),
                        SemanticOperandV1::Copy(place(2, REFERENCE)),
                    ],
                )
                .unwrap(),
            ),
        ));
        let removed = || {
            if matches!(case, InactiveCase::Deinitialize) {
                vec![SemanticStatementV1::new(
                    source(),
                    SemanticStatementKindV1::Deinitialize(selected(5)),
                )]
            } else {
                vec![
                    assign(
                        place(6, REFERENCE),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Move(selected(5))),
                    ),
                    dead(6),
                ]
            }
        };
        let mut tail = Vec::new();
        if matches!(case, InactiveCase::Reinitialize) {
            tail.push(assign(
                selected(5),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(2, REFERENCE))),
            ));
        }
        if matches!(case, InactiveCase::CopyPartial | InactiveCase::MovePartial) {
            tail.push(assign(
                place(7, PAIR),
                SemanticRvalueKindV1::Use(if matches!(case, InactiveCase::MovePartial) {
                    SemanticOperandV1::Move(place(5, PAIR))
                } else {
                    SemanticOperandV1::Copy(place(5, PAIR))
                }),
            ));
        } else {
            let read = if matches!(case, InactiveCase::Reinitialize | InactiveCase::ReadRemoved) {
                selected(5)
            } else {
                projected(5, &[(SemanticProjectionKindV1::Field(1), REFERENCE)])
            };
            tail.push(assign(
                place(4, REFERENCE),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(read)),
            ));
        }
        tail.push(unit());
        let mut blocks = vec![
            block(93, initial, branch(reverse, 1, 2)),
            block(94, removed(), jump(3)),
            block(95, removed(), jump(3)),
            block(
                96,
                tail,
                if matches!(case, InactiveCase::Loop) {
                    branch(false, 3, 4)
                } else {
                    SemanticTerminatorKindV1::Return
                },
            ),
        ];
        if matches!(case, InactiveCase::Loop) {
            blocks.push(block(97, vec![], SemanticTerminatorKindV1::Return));
        }
        functions[2] = function(30, false, CAPTURE, locals, blocks);
    })
}

pub(super) fn joined_inactive(plan: &SourceReferencePlanV29<'_, '_>) -> usize {
    let helper = capture_instance(plan, 0);
    let block = plan
        .blocks
        .iter()
        .find(|row| row.instance == helper && row.block.index() == 3)
        .unwrap();
    let pair = plan.states[block.entry][5].node.unwrap();
    let SourceReferenceNodeKindV29::Aggregate { first, .. } = plan.nodes[pair].kind else {
        panic!("pair shape lost");
    };
    let capture = plan.children[first];
    let SourceReferenceNodeKindV29::Aggregate { first, .. } = plan.nodes[capture].kind else {
        panic!("capture shape lost");
    };
    let absent = plan.children[first];
    assert_eq!(plan.nodes[absent].kind, SourceReferenceNodeKindV29::Absent);
    absent
}

#[test]
fn source_inactive_two_predecessor_removals_preserve_actual_cfg_transport_and_siblings() {
    for reverse in [false, true] {
        for case in [
            InactiveCase::Move,
            InactiveCase::Deinitialize,
            InactiveCase::Reinitialize,
            InactiveCase::Loop,
        ] {
            super::super::cell_emission_tests::with_cell_source_lowered(
                inactive_owner(case, reverse),
                |plan, _, emitted, _| {
                    let node = joined_inactive(plan);
                    assert_eq!(plan.nodes[node].inactive.unwrap().count, 2);
                    assert_eq!(emitted.len(), plan.instances.instances().len());
                    Ok(())
                },
            )
            .unwrap_or_else(|error| panic!("{case:?}, reverse={reverse}: {error:?}"));
        }
    }
}

#[test]
fn source_inactive_partial_reads_and_whole_copies_remain_denied() {
    for reverse in [false, true] {
        drop(try_inactive_owner(InactiveCase::Reinitialize, reverse).unwrap());
        for case in [
            InactiveCase::ReadRemoved,
            InactiveCase::CopyPartial,
            InactiveCase::MovePartial,
        ] {
            let error = try_inactive_owner(case, reverse)
                .err()
                .expect("partial source value admitted by SSA");
            assert_eq!(
                error,
                fe2o3_pliron::ProductionSemanticSsaErrorV1::PartialMove {
                    function: SemanticFunctionIdV1::from_index(2),
                    block: 3,
                    statement: Some(0),
                    local: 5,
                    violation: fe2o3_pliron::SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
                }
            );
        }
    }
}

pub(super) fn rebuilt_inactive(
    plan: &SourceReferencePlanV29<'_, '_>,
    emission: &SourceReferenceEmissionV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
    let node = joined_inactive(plan);
    let types = source_reference_cfg_node_types_v29(plan, node, budget)?;
    assert_eq!(types, [Type::Scalar(ScalarType::U64)]);
    let values = [ValueDef::new(ValueId(600), Type::Scalar(ScalarType::U64))];
    let mut values = values.iter();
    let binding = source_reference_rebuild_node_v29(
        emission,
        node,
        true,
        &mut [].iter(),
        &mut values,
        &mut 0,
        budget,
    )?;
    assert!(values.next().is_none());
    Ok(binding)
}

#[test]
fn source_inactive_binding_cannot_be_observed_reauthenticated_or_passed_as_a_value() {
    run_cells(inactive_owner(InactiveCase::Move, false), |plan, budget| {
        let emission = SourceReferenceEmissionV29::new(plan, budget)?;
        let binding = rebuilt_inactive(plan, &emission, budget)?;
        assert!(!semantic_binding_contains_execution_v29(&binding));
        assert!(!semantic_binding_can_restore_from_unique_source_v1(
            &binding
        ));
        assert!(binding.value().is_err());
        assert!(binding.values().is_err());
        let mut copy = emission_clone_binding_v1(&binding, budget)?;
        assert!(
            reauthenticate_capabilities_from_enum_payload_v1(
                &mut copy,
                SemanticLocalIdV1::from_index(5),
                0
            )
            .is_err()
        );
        assert!(source_reference_node_types_v29(plan, joined_inactive(plan), budget).is_err());
        assert!(
            source_reference_call_shape_v29(&emission, joined_inactive(plan), &binding, budget)
                .is_err()
        );
        let mut leaves = [];
        source_reference_merge_node_v29(
            &emission,
            joined_inactive(plan),
            &binding,
            &binding,
            &mut leaves.iter_mut(),
            &mut 0,
            budget,
        )?;
        Ok(())
    })
    .unwrap();
}

#[test]
fn source_inactive_original_plan_shape_and_archived_payload_are_required() {
    run_cells(inactive_owner(InactiveCase::Move, false), |plan, budget| {
        let emission = SourceReferenceEmissionV29::new(plan, budget)?;
        let original = rebuilt_inactive(plan, &emission, budget)?;
        let SemanticValueBindingV1::SourceInactive(binding) = &original else {
            panic!("inactive binding absent");
        };
        for field in 0..6 {
            let mut changed = binding.clone();
            match field {
                0 => changed.owner ^= 1,
                1 => changed.source[0] ^= 1,
                2 => changed.root = plan.instances.id_at(1).unwrap(),
                3 => {
                    changed.node = plan.inactive_removals
                        [plan.inactive_choices[plan.nodes[binding.node].inactive.unwrap().first]]
                        .prior
                }
                4 => changed.source_type = WORD,
                _ => changed.values[0].ty = Type::Scalar(ScalarType::U32),
            }
            assert!(
                source_reference_validate_inactive_v29(plan, &changed, &mut 0, budget).is_err(),
                "field {field}"
            );
        }
        let mut changed = binding.clone();
        changed.values[0].id = ValueId(601);
        assert!(
            source_reference_merge_node_v29(
                &emission,
                binding.node,
                &SemanticValueBindingV1::SourceInactive(changed),
                &original,
                &mut [].iter_mut(),
                &mut 0,
                budget
            )
            .is_err()
        );
        let prior = plan.inactive_removals
            [plan.inactive_choices[plan.nodes[binding.node].inactive.unwrap().first]]
            .prior;
        assert!(
            source_reference_merge_node_v29(
                &emission,
                prior,
                &original,
                &original,
                &mut [].iter_mut(),
                &mut 0,
                budget
            )
            .is_err()
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn source_inactive_missing_or_wrong_archive_never_qualifies_a_read() {
    run_cells(inactive_owner(InactiveCase::Move, false), |plan, budget| {
        let emission = SourceReferenceEmissionV29::new(plan, budget)?;
        let binding = rebuilt_inactive(plan, &emission, budget)?;
        let local = vec![Some(binding.clone())];
        let definition = plan
            .instances
            .instance(plan.root)
            .unwrap()
            .ssa()
            .plan()
            .entry_definitions()
            .first()
            .unwrap()
            .value();
        assert!(
            check_execution_archive_v29(
                &local,
                &SemanticSsaBindingsV1::default(),
                &place(0, REFERENCE),
                definition,
                budget
            )
            .is_err()
        );
        let mut archive = SemanticSsaBindingsV1::default();
        archive.insert(definition, binding);
        assert!(
            check_execution_archive_v29(
                &local,
                &archive,
                &projected(0, &[(SemanticProjectionKindV1::Dereference, WORD)]),
                definition,
                budget
            )
            .is_err()
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn source_inactive_receipts_rejoin_the_original_removal_and_typed_subobject() {
    run_cells(inactive_owner(InactiveCase::Move, false), |plan, budget| {
        let node = joined_inactive(plan);
        let shape = plan.nodes[node].inactive.unwrap();
        assert_eq!(shape.count, 2);
        for &index in &plan.inactive_choices[shape.first..shape.first + shape.count] {
            let original = &plan.inactive_removals[index];
            source_reference_check_inactive_type_v29(plan, node, original, budget)?;
            for field in 0..8 {
                let mut receipt = SourceReferenceInactiveRemovalV29 {
                    site: original.site,
                    source: original.source,
                    source_local: original.source_local,
                    kind: original.kind,
                    instance: original.instance,
                    local: original.local,
                    generation: original.generation,
                    ty: original.ty,
                    projections: original.projections.clone(),
                    prior: original.prior,
                    after: original.after,
                };
                match field {
                    0 => receipt.source ^= 1,
                    1 => receipt.site.instance = plan.root,
                    2 => receipt.source_local = SemanticLocalIdV1::from_index(0),
                    3 => receipt.kind = SourceReferenceRemovalKindV29::Deinitialize,
                    4 => receipt.ty = WORD,
                    5 => receipt.projections = 0..0,
                    6 => receipt.prior = node,
                    _ => receipt.after = usize::MAX,
                }
                assert!(
                    source_reference_check_inactive_type_v29(plan, node, &receipt, budget).is_err(),
                    "field {field}"
                );
            }
        }
        Ok(())
    })
    .unwrap();
}

fn partial_return_owner(
    partial: bool,
) -> Result<ProductionSemanticSsaOwnerV1, fe2o3_pliron::ProductionSemanticSsaErrorV1> {
    try_owner_with(Case::Shared, |_, functions| {
        let mut locals = functions[2].locals().to_vec();
        locals[0] = local(30, CAPTURE, SemanticLocalRoleV1::Return);
        let mut statements = vec![assign(
            place(0, CAPTURE),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, CAPTURE))),
        )];
        if partial {
            statements.push(SemanticStatementV1::new(
                source(),
                SemanticStatementKindV1::Deinitialize(projected(
                    0,
                    &[(SemanticProjectionKindV1::Field(0), REFERENCE)],
                )),
            ));
        }
        functions[2] = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([30; 32]),
            SemanticFunctionRoleV1::InternalHelper,
            SemanticItemDefinitionIdentityV1::from_sha256([30; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([30; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([30; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([30; 32]),
            source(),
            output_capture_abi(),
            locals,
            SemanticBlockIdV1::from_index(0),
            vec![block(30, statements, SemanticTerminatorKindV1::Return)],
        )
        .unwrap();
        let worker = &functions[1];
        let SemanticTerminatorKindV1::Call(call) = worker.blocks()[0].terminator().kind() else {
            panic!("original helper call missing");
        };
        let call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(2),
            call.arguments().to_vec(),
            Some(SemanticCallDestinationV1::new(
                place(3, CAPTURE),
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1::from_index(1),
                ),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        functions[1] = function(
            20,
            false,
            WORD,
            worker.locals().to_vec(),
            vec![
                block(
                    20,
                    worker.blocks()[0].statements().to_vec(),
                    SemanticTerminatorKindV1::Call(call),
                ),
                block(21, vec![unit()], SemanticTerminatorKindV1::Return),
            ],
        );
    })
}

#[test]
fn source_inactive_partial_return_is_denied_before_helper_transfer() {
    assert!(matches!(partial_return_owner(true),
        Err(fe2o3_pliron::ProductionSemanticSsaErrorV1::PartialMove {
            function, block: 0, statement: None, local: 0,
            violation: fe2o3_pliron::SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
        }) if function.index() == 2));
    run_cells(partial_return_owner(false).unwrap(), |plan, _| {
        let helper = capture_instance(plan, 0);
        let value = plan.nodes[plan.returns[helper.index()].unwrap()];
        assert!(value.storage.is_some());
        Ok(())
    })
    .unwrap();
}

#[test]
fn source_inactive_return_check_preserves_the_true_implicit_unit_case() {
    let owner = owner_with(Case::Shared, |_, functions| {
        let old = &functions[2];
        let statements = old.blocks()[0]
            .statements()
            .iter()
            .filter(|statement| {
                !matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment)
                if assignment.destination().local().index() == 0)
            })
            .cloned()
            .collect();
        functions[2] = function(
            30,
            false,
            CAPTURE,
            old.locals().to_vec(),
            vec![block(30, statements, SemanticTerminatorKindV1::Return)],
        );
    });
    super::super::cell_emission_tests::with_cell_source_lowered(owner, |plan, _, emitted, _| {
        assert_eq!(emitted.len(), plan.instances.instances().len());
        for ordinal in 0..2 {
            let helper = capture_instance(plan, ordinal);
            assert_eq!(plan.nodes[plan.returns[helper.index()].unwrap()].ty, UNIT);
        }
        Ok(())
    })
    .unwrap();
}
