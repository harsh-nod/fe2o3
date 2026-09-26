use super::*;

fn opposite_owner(expected: bool) -> ProductionPreRankedKirOwnerV1 {
    let seed = literal(expected, false);
    let source = seed.semantic_ssa().source_semantic();
    let function = &source.functions()[0];
    let blocks = vec![
        body(
            181,
            vec![],
            assertion(
                operand(SemanticTypeIdV1::from_index(1), u128::from(!expected), 1),
                expected,
                SemanticAssertMessageV1::NullPointerDereference,
                1,
            ),
        ),
        body(182, vec![], SemanticTerminatorKindV1::Return),
    ];
    rebuild(
        &seed,
        source.types().to_vec(),
        vec![copy_function(function, function.locals().to_vec(), blocks)],
    )
}
fn unknown_owner(bounds: bool) -> ProductionPreRankedKirOwnerV1 {
    let seed = literal(true, false);
    let source = seed.semantic_ssa().source_semantic();
    let function = &source.functions()[0];
    let boolean = SemanticTypeIdV1::from_index(1);
    let unit = SemanticTypeIdV1::from_index(0);
    let attrs = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::ZeroExtend,
        0,
        None,
    )
    .unwrap();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([183; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            boolean,
            SemanticAbiPassModeV1::Direct(attrs),
        ))],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
    .unwrap();
    let mut types = source.types().to_vec();
    let message = if bounds {
        types.push(word_type());
        SemanticAssertMessageV1::BoundsCheck {
            length: operand(SemanticTypeIdV1::from_index(2), 8, 8),
            index: operand(SemanticTypeIdV1::from_index(2), 0, 8),
        }
    } else {
        SemanticAssertMessageV1::NullPointerDereference
    };
    let changed = SemanticFunctionDeclV1::new(
        function.identity(),
        function.role(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        function.source(),
        abi,
        vec![
            function.locals()[0].clone(),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([184; 32]),
                boolean,
                SemanticLocalRoleV1::Argument(0),
                function.source(),
            ),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![
            body(185, vec![], assertion(local(1, boolean), true, message, 1)),
            body(186, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .unwrap()
    .with_kernel_entry(function.kernel_entry().unwrap().clone());
    rebuild(&seed, types, vec![changed])
}
#[test]
fn actual_opposite_source_conditions_and_polarities_are_refuted() {
    for expected in [true, false] {
        let owner = opposite_owner(expected);
        let error = run(&owner, |_, _| -> Result<(), Af> {
            panic!("refuted source cannot reach consumer")
        })
        .unwrap_err();
        assert!(matches!(error.failure, Af::Refuted { .. }));
        assert!(error.policies.is_some());
    }
}
#[test]
fn actual_unknown_bool_is_not_a_proof_even_with_a_complete_graph_pair() {
    let owner = unknown_owner(false);
    let error = run(&owner, |_, _| -> Result<(), Af> {
        panic!("unknown source cannot reach consumer")
    })
    .unwrap_err();
    assert!(matches!(
        error.failure,
        Af::NotProved {
            reason: fe2o3_mir_model::SemanticAssertionNotProvedV1::Unknown,
            ..
        }
    ));
}
#[test]
fn bounds_deferred_never_becomes_a_source_fact() {
    let owner = unknown_owner(true);
    let error = run(&owner, |_, _| -> Result<(), Af> {
        panic!("deferred bounds cannot reach consumer")
    })
    .unwrap_err();
    assert!(matches!(
        error.failure,
        Af::NotProved {
            reason: fe2o3_mir_model::SemanticAssertionNotProvedV1::BoundsNeedsIndependentRule,
            ..
        }
    ));
}
#[test]
fn real_overshift_is_refuted_without_reusing_safe_shift_evidence() {
    run(&shift_owner(3), |_, _| Ok(())).unwrap();
    let error = run(&shift_owner(64), |_, _| -> Result<(), Af> {
        panic!("overshift")
    })
    .unwrap_err();
    assert!(matches!(error.failure, Af::Refuted { .. }));
}
#[test]
fn unreachable_only_source_assertion_orphan_trap_is_a_terminal_pair_refusal() {
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let owner = original_assertion_owner(Fixture::Unreachable, false, &mut budget);
    assert_eq!(owner.assert_origins().source_site_count(), 0);
    let error = run(&owner, |_, _| -> Result<(), Af> {
        panic!("missing source obligation")
    })
    .unwrap_err();
    assert!(matches!(
        &error.failure,
        Af::Policy(ProductionCanonicalRankedPolicyErrorV1::Policy(error))
            if matches!(error.failure(),
                fe2o3_pliron::CanonicalRankedPolicyFailureV1::PrivateRequirement {
                    requirement: fe2o3_pliron::CanonicalPrivateRequirementV1::TerminalPairs,
                    ..
                })
    ));
    assert!(error.policies.is_none());
    assert!(error.source.is_some());
}

pub(super) fn unmaterialized_assertion_owner() -> ProductionPreRankedKirOwnerV1 {
    let seed = literal(true, false);
    let source = seed.semantic_ssa().source_semantic();
    let function = &source.functions()[0];
    let boolean = SemanticTypeIdV1::from_index(1);
    // One real incoming failure edge admits C; the second source assertion is
    // unreachable and must still be refused by B's complete source roster.
    rebuild(
        &seed,
        source.types().to_vec(),
        vec![copy_function(
            function,
            function.locals().to_vec(),
            vec![
                body(
                    201,
                    vec![],
                    assertion(
                        operand(boolean, 1, 1),
                        true,
                        SemanticAssertMessageV1::NullPointerDereference,
                        1,
                    ),
                ),
                body(202, vec![], SemanticTerminatorKindV1::Return),
                body(
                    203,
                    vec![],
                    assertion(
                        operand(boolean, 1, 1),
                        true,
                        SemanticAssertMessageV1::NullPointerDereference,
                        3,
                    ),
                ),
                body(204, vec![], SemanticTerminatorKindV1::Return),
            ],
        )],
    )
}

#[test]
fn unmaterialized_source_assertion_is_an_explicit_refusal_not_empty_clean() {
    let owner = unmaterialized_assertion_owner();
    assert_eq!(owner.assert_origins().source_site_count(), 1);
    assert_eq!(
        owner.semantic_ssa().source_semantic().functions()[0]
            .blocks()
            .iter()
            .filter(|block| matches!(
                block.terminator().kind(),
                SemanticTerminatorKindV1::Assert { .. }
            ))
            .count(),
        2
    );
    cr_run_owner(&owner, 1 << 48, S, |_, budget| {
        let function = SemanticFunctionIdV1::from_index(0);
        assert!(owner.assert_origins().is_materialized_block(
            function, function, SemanticBlockIdV1::from_index(0), budget,
        ).unwrap());
        assert!(!owner.assert_origins().is_materialized_block(
            function, function, SemanticBlockIdV1::from_index(2), budget,
        ).unwrap());
        Ok(())
    })
    .0
    .unwrap();
    let error = run(&owner, |_, _| -> Result<(), Af> {
        panic!("missing source obligation")
    })
    .unwrap_err();
    assert!(matches!(
        error.failure,
        Af::Binding {
            detail: "source assertion is outside the original materialization roster",
            ..
        }
    ));
    assert!(error.policies.is_some());
    assert!(error.source.is_some());
}

#[test]
fn reader_only_changed_sealed_locator_fields_are_rejected_against_real_owner() {
    // These inert reader inputs are deliberately unauthenticated, not owners
    // produced by a bypass. C and the source metadata use genuine actual N.
    let owner = literal(true, false);
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let floor = owner.unit_local_source_storage_floor_v1().unwrap() + SIBLING;
    budget.reserve_storage(floor).unwrap();
    owner.with_checked_canonical_ranked_source_v1(&mut budget, |view, budget| {
        let source = view.source;
        fe2o3_pliron::with_canonical_trap_policy_checks_v1(view.checked, budget, |policies, budget| {
            let original = &source.contracts.assertions[0];
            canonical_assertion_v1::read_test_row(source, policies, original, budget).unwrap();
            for damage in 0..6 {
                let mut row = ProductionCanonicalRankedAssertionV1 { span: original.span, binding: original.binding };
                match damage {
                    0 => row.binding.expected = !row.binding.expected,
                    1 => row.binding.semantic_success = SemanticBlockIdV1::from_index(0),
                    _ => {
                        let SemanticKirAssertConditionOutcomeV1::Emitted { condition_use, definition, success_edge, failure_edge } = &mut row.binding.outcome else { panic!("emitted fixture"); };
                        match damage {
                            2 => *condition_use = fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::TerminatorOperand { block: success_edge.source, operand: 1 },
                            3 => *definition = fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::FunctionArgument { function: success_edge.source.function, argument: 0 },
                            4 => success_edge.successor = failure_edge.successor,
                            _ => failure_edge.source.block = failure_edge.source.block.saturating_add(1),
                        }
                    }
                }
                let error = canonical_assertion_v1::read_test_row(source, policies, &row, budget).unwrap_err();
                assert!(matches!(error, Af::Binding { detail: "original assertion attachment", .. }));
            }
            Ok(())
        }).unwrap();
        Ok(())
    }).unwrap();
    assert_eq!(budget.storage(), floor);
}
#[test]
fn actual_source_local_limits_refuse_before_global_denial_and_drop_partial_backing() {
    let owner = literal(true, false);
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let floor = owner.unit_local_source_storage_floor_v1().unwrap() + SIBLING;
    budget.reserve_storage(floor).unwrap();
    owner.with_checked_canonical_ranked_source_v1(&mut budget, |view, budget| {
        let source = view.source;
        fe2o3_pliron::with_canonical_trap_policy_checks_v1(view.checked, budget, |policies, budget| {
            for storage in [false, true] {
                let paid = budget.storage();
                let limits = if storage { fe2o3_mir_model::SemanticAssertionLimitsV1::new(usize::MAX, 0) } else { fe2o3_mir_model::SemanticAssertionLimitsV1::new(0, usize::MAX) };
                let error = canonical_assertion_v1::read_test_limits(source, policies, limits, budget).unwrap_err();
                assert!(matches!(error, Af::SourceQuery(crate::ProductionSemanticAssertionQueryErrorV1::Analysis(
                    fe2o3_mir_model::SemanticAssertionErrorV1::WorkLimit { .. } | fe2o3_mir_model::SemanticAssertionErrorV1::StorageLimit { .. }
                ))));
                assert_eq!(budget.storage(), paid);
                assert_eq!(budget.failed_storage(), None);
            }
            Ok(())
        }).unwrap();
        Ok(())
    }).unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(work.failed_work(), None);
}

#[test]
fn actual_checked_wrap_and_changed_message_do_not_reuse_safe_arithmetic_proof() {
    for (left, right, message) in [(u64::MAX as u128, 1, u64::MAX as u128), (2, 3, 4)] {
        let owner = checked_owner(left, right, message);
        let error = run(&owner, |_, _| -> Result<(), Af> {
            panic!("hostile checked assertion");
        })
        .unwrap_err();
        assert!(matches!(
            error.failure,
            Af::NotProved { .. } | Af::Refuted { .. }
        ));
    }
}

#[test]
fn genuine_unitlocal_nonbounds_assertion_remains_an_upstream_refusal() {
    let ssa = unit_assertion_ssa(SemanticAssertMessageV1::NullPointerDereference);
    let semantic = ssa.source_semantic();
    let entry = semantic.functions()[0].kernel_entry().unwrap();
    let inputs = [crate::ProductionSourceLaunchRootInputV1::new(
        std::str::from_utf8(entry.export_symbol().as_bytes()).unwrap(),
        *entry.kernel_binding_identity().as_bytes(),
        crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
    )];
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(semantic, &inputs).unwrap();
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let error = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .err()
    .expect("the old UnitLocal source frame does not admit this assertion family");
    assert!(matches!(error, ProductionPreRankedKirErrorV1::Lowering(_)));
    assert!(
        error
            .to_string()
            .contains("local helper assertion requires a bounds diagnostic")
    );
    // B has not run: no fabricated source owner or reader-only bypass is used.
}

mod coverage {
    use super::*;
    include!("production_canonical_assertion_coverage_v1_tests.rs");
}
