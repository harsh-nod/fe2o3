use super::*;
use fe2o3_kernel_ir::LocalFrameControlKindV1;

#[test]
fn actual_same_owner_call_groups_have_qualified_bounded_lookup_and_no_duplicate_keys() {
    let owner = unit_owner(UnitCase::ReadThenWrite, &[2, 2]);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget
        .reserve_storage(FLOOR + owner.retained_analysis_storage_v1())
        .unwrap();
    let (inventory, inventory_storage) =
        CanonicalKirInventoryV1::derive(owner.executable(), &mut budget).unwrap();
    budget
        .reserve_storage(inventory_storage.retained_storage())
        .unwrap();
    let incoming = budget.storage();
    with_canonical_call_scratch_v1(&mut budget, |budget| {
        let (groups, calls) = build_canonical_call_index_v1(
            CanonicalCallSubjectV1::owner(&owner), &inventory, budget,
        )?;
        assert_eq!((groups.len(), calls.len()), (4, 4));
        let lookup = UnitLocalCallLookupV1::build(&groups, budget)?;
        let height = (usize::BITS - groups.len().leading_zeros()) as usize;
        for (index, group) in groups.iter().enumerate() {
            let source = group.function.source;
            let before = budget.work();
            assert_eq!(lookup.caller(source.correspondence_owner, source.semantic_function, budget)?, index);
            assert!(budget.work() - before <= 3 * height + 1);
        }
        // The first sorted key visits floor(log2(G)) + 1 nodes. Each
        // iteration charges 1 + 2; returning the stored ordinal charges 1.
        let first = &lookup.callers[0];
        let exact = 3 * height + 1;
        let floor = budget.storage();
        for limit in [exact - 1, exact] {
            let mut query_work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut query = ArgumentBudgetV1::new(&mut query_work, STORAGE);
            query.reserve_storage(floor)?;
            let result = lookup.caller(
                SemanticFunctionIdV1::from_index(first.key[0] as u32),
                SemanticFunctionIdV1::from_index(first.key[1] as u32), &mut query,
            );
            if limit == exact {
                assert_eq!(result?, first.group);
                assert_eq!(query.work(), exact);
            } else {
                assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Work(error)
                )) if error.actual() == exact && error.limit() == exact - 1));
                assert_eq!(query.work(), exact - 1);
            }
            assert_eq!((query.storage(), query.peak_storage()), (floor, floor));
        }
        let before = budget.work();
        assert!(matches!(lookup.caller(SemanticFunctionIdV1::from_index(99),
            SemanticFunctionIdV1::from_index(2), budget),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)));
        assert!(budget.work() - before <= 3 * height);

        // Private builder-negative only: duplicate real group references,
        // without constructing an admitted owner or a second call receipt.
        budget.reserve_storage(std::mem::size_of::<[CanonicalCallGroupV1<'_>; 2]>())?;
        let copy = || CanonicalCallGroupV1 {
            function: ProductionCanonicalCallFunctionV1 {
                source: groups[0].function.source, canonical: groups[0].function.canonical,
            },
            calls: groups[0].calls, spans: groups[0].spans, direct: groups[0].direct,
            components: groups[0].components, ignored: groups[0].ignored,
            borrowed: groups[0].borrowed,
        };
        let duplicate = [copy(), copy()];
        let floor = budget.storage();
        assert!(matches!(with_canonical_call_scratch_v1(budget, |budget| {
            UnitLocalCallLookupV1::build(&duplicate, budget).map(|_| ())
        }), Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)));
        assert_eq!(budget.storage(), floor);
        drop(duplicate);
        budget.release_storage(std::mem::size_of::<[CanonicalCallGroupV1<'_>; 2]>())?;
        drop(lookup);
        drop(calls);
        drop(groups);
        Ok(())
    }).unwrap();
    assert_eq!(budget.storage(), incoming);
    drop(inventory);
    budget
        .release_storage(inventory_storage.retained_storage())
        .unwrap();
    let retained = owner.retained_analysis_storage_v1();
    drop(owner);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn source_assertions_keep_true_and_false_polarities_and_the_inactive_failure_sink() {
    for expected in [true, false] {
        let owner = unit_owner(UnitCase::CastAssert { expected }, &[1]);
        let rows = &owner.helper_memory.unit_source;
        assert_eq!(rows.associations.len(), 1);
        let association = rows.associations[0];
        let function = &owner.executable().module().functions[association.key.physical];
        assert_eq!(function.body.as_ref().unwrap().blocks.len(), 5);
        let mut assertions = 0;
        let mut inactive = 0;
        let mut returns = 0;
        for row in &rows.control {
            match row.kind {
                UnitLocalControlKindV1::Assert {
                    target,
                    physical_target,
                    predicate,
                    length,
                    index,
                    expected: observed,
                    selected_successor,
                    inactive_block,
                    ..
                } => {
                    assert_eq!(observed, expected);
                    assert_eq!(selected_successor, u32::from(!expected));
                    assert_eq!(target.index(), row.source_block.unwrap().index() + 1);
                    assert_eq!(rows.values[predicate].known_bits, Some(u64::from(expected)));
                    assert_eq!(rows.values[length].known_bits, Some(8));
                    assert_eq!(
                        rows.values[index].known_bits,
                        Some(if assertions == 2 { 1 } else { 0 })
                    );
                    let UnitLocalNativeValueV1::Scalar {
                        value: condition,
                        scalar: ScalarType::Bool,
                        ..
                    } = rows.values[predicate].native
                    else {
                        panic!("actual typed condition")
                    };
                    assert!(
                        matches!(owner.helper_memory.control[row.physical_control].kind(),
                        LocalFrameControlKindV1::Selected {
                            condition: actual, value, successor, target, inactive,
                        } if actual == condition && value == expected
                            && u32::from(successor) == selected_successor
                            && target == physical_target && inactive == inactive_block)
                    );
                    assertions += 1;
                }
                UnitLocalControlKindV1::InactiveTrap { .. } => {
                    assert!(row.source_block.is_none());
                    assert_eq!(
                        owner.helper_memory.control[row.physical_control].kind(),
                        LocalFrameControlKindV1::InactiveTrap
                    );
                    inactive += 1;
                }
                UnitLocalControlKindV1::Return {
                    local,
                    unit_type,
                    unit_value,
                } => {
                    assert_eq!((local.index(), unit_type), (0, UNIT));
                    assert!(matches!(
                        rows.values[unit_value].native,
                        UnitLocalNativeValueV1::IgnoredUnit
                    ));
                    assert!(matches!(
                        rows.values[unit_value].recipe,
                        UnitLocalValueRecipeV1::Unit
                    ));
                    assert_eq!(rows.values[unit_value].role, None);
                    returns += 1;
                }
                UnitLocalControlKindV1::Goto { .. } => panic!("fixture has only Assert and Return"),
            }
        }
        assert_eq!((assertions, inactive, returns), (3, 1, 1));
        assert!(
            rows.values
                .iter()
                .any(|row| matches!(row.recipe, UnitLocalValueRecipeV1::Cast { .. }))
        );
        assert!(
            rows.values
                .iter()
                .any(|row| matches!(row.recipe, UnitLocalValueRecipeV1::Load { .. }))
        );
    }
}

#[test]
fn verified_same_outcome_comparison_cannot_replace_the_actual_source_recipe() {
    for expected in [true, false] {
        with_pending_candidate(
            UnitCase::CastAssert { expected },
            |module| {
                let helper = module
                    .functions
                    .iter_mut()
                    .find(|function| function.role == fe2o3_kernel_ir::FunctionRole::InternalHelper)
                    .unwrap();
                let operations = &mut helper.body.as_mut().unwrap().blocks[0].operations;
                let operation = operations
                    .iter_mut()
                    .find(|operation| matches!(operation.kind, OperationKind::Compare { .. }))
                    .unwrap();
                let OperationKind::Compare { predicate, .. } = &mut operation.kind else {
                    unreachable!()
                };
                if expected {
                    assert_eq!(*predicate, fe2o3_kernel_ir::ComparePredicate::LessThan);
                    *predicate = fe2o3_kernel_ir::ComparePredicate::LessThanOrEqual;
                } else {
                    assert_eq!(
                        *predicate,
                        fe2o3_kernel_ir::ComparePredicate::GreaterThanOrEqual
                    );
                    *predicate = fe2o3_kernel_ir::ComparePredicate::GreaterThan;
                }
            },
            |subject, origins, budget| {
                // Both native predicates keep the original selected success edge;
                // verification and genuine origin sealing have already succeeded.
                assert!(matches!(
                    SealedHelperMemoryV1::derive_with_origins_v1(subject, Some(origins), budget),
                    Err(ProductionPreRankedKirErrorV1::Lowering(
                        ProductionSemanticKirErrorV1::CorrespondenceMismatch
                    ))
                ));
            },
        );
    }
}

#[test]
fn retained_bool_assert_load_is_not_given_a_fabricated_statement_site() {
    with_pending_candidate(
        UnitCase::RetainedBoolAssert,
        |_| {},
        |subject, origins, budget| {
            let plan = subject
                .semantic_ssa
                .plan_for_function(SemanticFunctionIdV1::from_index(1))
                .unwrap();
            assert!(
                !plan
                    .plan()
                    .promoted_variables()
                    .iter()
                    .any(|variable| variable.get() == 5)
            );
            let floor = budget.storage();
            let physical = SealedHelperMemoryV1::derive(subject, budget).unwrap();
            let storage = physical.storage.retained_storage();
            budget.reserve_storage(storage).unwrap();
            assert!(physical.unit_source.associations.is_empty());
            assert!(physical.accesses.iter().any(|row| row.kind()
                == fe2o3_kernel_ir::LocalFrameAccessKindV1::Read
                && row.initializing_store().is_some()));
            assert!(physical.control.iter().any(|row| matches!(
                row.kind(),
                LocalFrameControlKindV1::Selected { value: true, .. }
            )));
            drop(physical);
            budget.release_storage(storage).unwrap();
            assert!(matches!(
                SealedHelperMemoryV1::derive_with_origins_v1(subject, Some(origins), budget),
                Err(ProductionPreRankedKirErrorV1::Lowering(
                    ProductionSemanticKirErrorV1::Unsupported {
                        function: 1,
                        block: None,
                        statement: None,
                        detail: "local helper native memory access requires a statement site",
                    }
                ))
            ));
            assert_eq!(budget.storage(), floor);
        },
    );
    let (ssa, launch) = unit_source(UnitCase::RetainedBoolAssert, &[1]);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    assert!(matches!(
        ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        ),
        Err(ProductionPreRankedKirErrorV1::Lowering(
            ProductionSemanticKirErrorV1::Unsupported {
                function: 1,
                block: None,
                statement: None,
                detail: "local helper native memory access requires a statement site",
            }
        ))
    ));
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn unit_calls_bind_the_actual_empty_continuation_and_exact_helper_return() {
    let owner = unit_owner(UnitCase::ReadThenWrite, &[2, 2]);
    let rows = &owner.helper_memory.unit_source;
    for call in &rows.calls {
        let association = rows.associations[call.callee_association];
        assert_eq!(call.return_control, association.return_control);
        assert_eq!(call.caller.root, association.key.root);
        assert_eq!(call.destination_local.index(), 0);
        assert_eq!(call.unit_type, UNIT);
        let body = owner.executable().module().functions[call.caller.physical]
            .body
            .as_ref()
            .unwrap();
        let continuation = body
            .blocks
            .iter()
            .find(|block| block.id == call.continuation_physical)
            .unwrap();
        assert!(continuation.parameters.is_empty());
        let call_block = body.blocks.iter().find(|block| {
            matches!(block.terminator.as_ref(), Some(fe2o3_kernel_ir::Terminator::Branch { target, .. })
                if *target == call.continuation_physical)
        }).unwrap();
        assert!(matches!(call_block.terminator.as_ref(),
            Some(fe2o3_kernel_ir::Terminator::Branch { arguments, .. }) if arguments.is_empty()));
        for value in &rows.values[call.ignored_values.0..call.ignored_values.1] {
            assert_eq!(value.source_type, UNIT);
            assert!(matches!(value.native, UnitLocalNativeValueV1::IgnoredUnit));
        }
        assert!(matches!(
            rows.control[call.return_control].kind,
            UnitLocalControlKindV1::Return { .. }
        ));
    }
}

#[test]
fn normal_admission_does_not_change_the_legacy_raw_pure_lowering_gate() {
    let (ssa, _) = unit_source(UnitCase::Initializer, &[1]);
    assert!(matches!(
        lower_module(&ssa, ProductionSemanticKirLimitsV1::default(), None),
        Err(ProductionSemanticKirErrorV1::HelperEffectsUnavailable { function: 1, .. })
    ));
    let owner = unit_owner(UnitCase::Initializer, &[1]);
    assert_eq!(
        owner.helper_source_policy_v1(),
        ProductionHelperSourcePolicyV1::UnitLocal
    );
    assert_eq!(owner.empty_effect_helpers().iter().count(), 0);
}

#[test]
fn complete_nonempty_callee_effects_cannot_skip_any_actual_local_call_binding() {
    use fe2o3_kernel_analysis::{CanonicalKirCallEffectDecisionV1, CanonicalKirCallEffectsV1};
    use fe2o3_kernel_ir::{
        CanonicalKirBlockCoordinateV1, CanonicalKirFunctionCoordinateV1,
        CanonicalKirOperationCoordinateV1,
    };
    let owner = unit_owner(UnitCase::ReadThenWrite, &[2, 2]);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    let retained = owner.retained_analysis_storage_v1();
    budget.reserve_storage(FLOOR + retained).unwrap();
    let (inventory, inventory_storage) =
        CanonicalKirInventoryV1::derive(owner.executable(), &mut budget).unwrap();
    budget
        .reserve_storage(inventory_storage.retained_storage())
        .unwrap();
    let (effects, effects_storage) =
        CanonicalKirCallEffectsV1::derive(&inventory, &mut budget).unwrap();
    budget
        .reserve_storage(effects_storage.retained_storage())
        .unwrap();
    let rows = &owner.helper_memory.unit_source;
    for body in &rows.bodies {
        assert_eq!(
            effects
                .decision(
                    CanonicalKirFunctionCoordinateV1(body.physical as u32),
                    &mut budget
                )
                .unwrap(),
            CanonicalKirCallEffectDecisionV1::CompleteNonempty
        );
    }
    let mut actual_calls = 0;
    for (function_ordinal, function) in owner.executable().module().functions.iter().enumerate() {
        for (block_ordinal, block) in function.body.as_ref().unwrap().blocks.iter().enumerate() {
            for (operation_ordinal, operation) in block.operations.iter().enumerate() {
                if !matches!(operation.kind, OperationKind::Call { .. }) {
                    continue;
                }
                assert!(
                    !operation
                        .has_complete_effect_summary_with_budget_v1(&mut budget)
                        .unwrap()
                );
                let coordinate = CanonicalKirOperationCoordinateV1 {
                    block: CanonicalKirBlockCoordinateV1 {
                        function: CanonicalKirFunctionCoordinateV1(function_ordinal as u32),
                        block: block_ordinal as u32,
                    },
                    operation: operation_ordinal as u32,
                };
                assert_eq!(
                    rows.calls
                        .iter()
                        .filter(|call| call.call == coordinate)
                        .count(),
                    1
                );
                actual_calls += 1;
            }
        }
    }
    assert_eq!((actual_calls, rows.calls.len()), (4, 4));
    drop(effects);
    budget
        .release_storage(effects_storage.retained_storage())
        .unwrap();
    drop(inventory);
    budget
        .release_storage(inventory_storage.retained_storage())
        .unwrap();
    drop(owner);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn private_receipt_candidate_cannot_bypass_the_closed_ranked_attachment_boundary() {
    let owner = unit_owner(UnitCase::Initializer, &[1]);
    // The public receipt constructor is separately required to refuse. This
    // deliberately private candidate tests attachment's defense in depth only.
    let candidate = ProductionMaterializedRankedModuleReceiptV1 {
        materialized: owner,
        roots: Vec::new().into_boxed_slice(),
    };
    assert!(matches!(
        ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks(candidate),
        Err(
            ProductionSemanticKirErrorV1::LocalHelperSourceConsumerUnavailable {
                consumer: "ranked attachment",
            }
        )
    ));
}

#[test]
fn undefined_or_killed_unit_continuation_is_rejected_by_the_actual_ssa_planner() {
    let (seed, _) = unit_source(UnitCase::Initializer, &[1]);
    let semantic = seed.source_semantic();
    let original = &semantic.functions()[0];
    for killed in [false, true] {
        let source = SemanticSourceProvenanceV1::unavailable();
        let mut before_call = Vec::new();
        if killed {
            before_call.push(assignment(
                1,
                UNIT,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                    UNIT,
                    SemanticConstantValueV1::ZeroSized,
                ))),
            ));
            before_call.push(SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(1)),
            ));
        }
        let root = SemanticFunctionDeclV1::new(
            original.identity(),
            original.role(),
            original.item_definition_identity(),
            original.monomorphization_identity(),
            original.generic_type_arguments_identity(),
            original.const_generic_arguments_identity(),
            source,
            original.abi().clone(),
            vec![
                original.locals()[0].clone(),
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([191; 32]),
                    UNIT,
                    SemanticLocalRoleV1::Temporary,
                    source,
                ),
            ],
            SemanticBlockIdV1::from_index(0),
            vec![
                block(
                    210,
                    before_call,
                    original.blocks()[0].terminator().kind().clone(),
                ),
                block(
                    215,
                    vec![assignment(
                        0,
                        UNIT,
                        SemanticRvalueKindV1::Use(value(1, UNIT)),
                    )],
                    SemanticTerminatorKindV1::Return,
                ),
            ],
        )
        .unwrap()
        .with_kernel_entry(original.kernel_entry().unwrap().clone());
        let admitted = InertSemanticMirRequestV1::new(
            semantic.target(),
            semantic.types().to_vec(),
            vec![],
            vec![],
            vec![],
            vec![root, semantic.functions()[1].clone()],
            vec![ARRAY_ROOT],
        )
        .unwrap()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
        let source = ProductionSemanticMirOwnerV1::try_new(
            admitted,
            ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap();
        let error =
            ProductionSemanticSsaOwnerV1::try_new(source, ProductionSemanticSsaLimitsV1::default())
                .unwrap_err();
        assert!(
            matches!(error,
            fe2o3_pliron::ProductionSemanticSsaErrorV1::Planner {
                function,
                error: fe2o3_mir_model::SsaPlannerErrorV1::UndefinedAtUse { block, event, variable },
            } if function == ARRAY_ROOT && block == SsaBlockIdV1::new(1)
                && event == 0 && variable.get() == 1),
            "killed={killed}: {error:?}"
        );
    }
}
