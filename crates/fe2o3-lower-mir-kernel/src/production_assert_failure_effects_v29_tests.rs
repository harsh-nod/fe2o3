use super::*;
use fe2o3_pliron::ProductionSemanticSsaErrorV1;

fn assertion_owner(
    hostile: bool,
    literal: Option<bool>,
    move_condition: bool,
) -> Result<ProductionSemanticSsaOwnerV1, ProductionSemanticSsaErrorV1> {
    let baseline = owner(Case::Shared);
    let source = baseline.source_semantic();
    let mut types = source.types().to_vec();
    let mut functions = source.functions().to_vec();
    let boolean = append_bool(&mut types);
    let mut locals = functions[2].locals().to_vec();
    let mut statements = without_unit(&functions[2]);
    let condition = match literal {
        Some(value) => SemanticOperandV1::Constant(SemanticConstantV1::new(
            boolean,
            SemanticConstantValueV1::Scalar(
                SemanticScalarValueV1::new(u128::from(value), 1).unwrap(),
            ),
        )),
        None => {
            locals.push(local(94, boolean, SemanticLocalRoleV1::Temporary));
            statements.push(assign(
                place(5, boolean),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                    boolean,
                    SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1, 1).unwrap()),
                ))),
            ));
            if move_condition {
                SemanticOperandV1::Move(place(5, boolean))
            } else {
                SemanticOperandV1::Copy(place(5, boolean))
            }
        }
    };
    let message = SemanticAssertMessageV1::BoundsCheck {
        length: SemanticOperandV1::Move(place(3, WORD)),
        index: if hostile {
            SemanticOperandV1::Copy(place(3, WORD))
        } else {
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                WORD,
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0, 8).unwrap()),
            ))
        },
    };
    let mut success = vec![assign(
        place(3, WORD),
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(3, WORD))),
    )];
    if literal.is_none() {
        success.push(assign(
            place(5, boolean),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(5, boolean))),
        ));
    }
    replace_closure(
        &mut functions,
        locals,
        vec![
            block(
                30,
                statements,
                SemanticTerminatorKindV1::Assert {
                    condition,
                    expected: true,
                    message,
                    target: SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::AssertSuccess,
                        SemanticBlockIdV1::from_index(1),
                    ),
                    unwind: SemanticUnwindActionV1::Unreachable,
                },
            ),
            block(31, success, SemanticTerminatorKindV1::Return),
        ],
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        source.callables().to_vec(),
        vec![ROOT],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let semantic =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let mut owner =
        ProductionSemanticSsaOwnerV1::try_new(semantic, ProductionSemanticSsaLimitsV1::default())?;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    Ok(owner)
}

#[test]
fn original_v979_move_then_copy_diagnostic_is_rejected_at_event_six() {
    assert!(matches!(assertion_owner(true, Some(true), false),
        Err(ProductionSemanticSsaErrorV1::Planner { function, error:
            fe2o3_mir_model::SsaPlannerErrorV1::UndefinedAtUse { block, event: 6, variable } })
            if function.index() == 2 && block.get() == 0 && variable.get() == 3));
}

#[test]
fn source_validity_does_not_depend_on_literal_success_or_failure() {
    for condition in [Some(true), Some(false), None] {
        assert!(matches!(assertion_owner(true, condition, false),
            Err(ProductionSemanticSsaErrorV1::Planner { error:
                fe2o3_mir_model::SsaPlannerErrorV1::UndefinedAtUse { variable, .. }, .. })
                if variable.get() == 3));
    }
}

#[test]
fn ordered_diagnostic_move_has_real_source_boundary_and_preserves_success() {
    for condition in [Some(true), Some(false), None] {
        let owner = assertion_owner(false, condition, false).unwrap();
        let reached = std::cell::Cell::new(false);
        run_owner(owner, |plan, _| {
            reached.set(true);
            for (index, instance) in plan.instances.instances().iter().enumerate() {
                if instance.function().index() != 2 {
                    continue;
                }
                let occurrences = plan
                    .instances
                    .occurrences(plan.instances.id_at(index).unwrap())
                    .unwrap();
                let boundary = occurrences
                    .terminal_failure_start(SsaBlockIdV1::new(0))
                    .unwrap();
                let tail: Vec<_> = occurrences
                    .events()
                    .iter()
                    .filter(|event| {
                        event.site() == execution_site_v29(SemanticBlockIdV1::from_index(0), None)
                            && event.ordinal() as usize >= boundary
                    })
                    .collect();
                assert_eq!(tail.len(), 2);
                assert_eq!(tail[0].operand(), ExecutionOperandV29::AssertMessage(0));
                assert_eq!(tail[0].role(), ExecutionEventV29::BaseUse);
                assert_eq!(tail[1].role(), ExecutionEventV29::MoveKill);
            }
            Ok(())
        })
        .unwrap();
        assert!(reached.get());
    }
}

#[test]
fn common_condition_move_is_not_rolled_back_with_diagnostics() {
    assert!(matches!(assertion_owner(false, None, true),
        Err(ProductionSemanticSsaErrorV1::Planner { error:
            fe2o3_mir_model::SsaPlannerErrorV1::UndefinedAtUse { variable, .. }, .. })
            if variable.get() == 5));
}
