// These are default projector/roster custody tests, not final KIR attachment
// tests. The inherited fixture's private array effect has a separate checker.

fn assert_same_owner_induction_report_v2(program: &ProductionRankedSemanticProgramV1) {
    let ssa = program.materialized.semantic_ssa();
    for root in &program.roots {
        let selection = ssa
            .source_semantic()
            .select_kernel_body_for_root_v1(root.semantic_root)
            .unwrap();
        let plan = ssa.plan_for_function(selection.body()).unwrap();
        let expected =
            fe2o3_mir_model::analyze_semantic_u32_induction_no_overflow_with_ssa_plan_v2(
                ssa.source_semantic(),
                selection.body(),
                plan.plan(),
                fe2o3_mir_model::SemanticU32InductionAnalysisLimitsV1::default(),
            )
            .unwrap();
        assert_eq!(root.semantic_u32_induction, expected);
        assert!(!expected.grants_authority());
        assert!(!expected.authorizes_compiler_transform());
    }
}

#[test]
fn default_reachable_induction_excludes_dead_definitions_and_checked_statements() {
    let function = assertion_root(
        vec![
            (A_UNIT, SemanticLocalRoleV1::Return),
            (A_U32, SemanticLocalRoleV1::Temporary),
            (A_U64, SemanticLocalRoleV1::Temporary),
            (A_CHECKED, SemanticLocalRoleV1::Temporary),
        ],
        Vec::new(),
        vec![
            block(
                221,
                vec![typed_assignment(
                    1,
                    A_U32,
                    SemanticRvalueKindV1::Use(typed_constant(A_U32, 0, 4)),
                )],
                SemanticTerminatorKindV1::Return,
            ),
            block(
                222,
                vec![
                    typed_assignment(
                        1,
                        A_U32,
                        SemanticRvalueKindV1::Use(typed_constant(A_U32, u32::MAX.into(), 4)),
                    ),
                    typed_assignment(
                        2,
                        A_U64,
                        SemanticRvalueKindV1::Use(typed_constant(A_U64, u64::MAX.into(), 8)),
                    ),
                    typed_assignment(
                        3,
                        A_CHECKED,
                        SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                            SemanticCheckedBinaryOpV1::Add,
                            typed_operand(2, A_U64),
                            typed_constant(A_U64, 1, 8),
                        )),
                    ),
                ],
                SemanticTerminatorKindV1::Return,
            ),
        ],
    );
    assert_eq!(function.blocks()[0].statements().len(), 3);
    assert_eq!(function.blocks()[1].statements().len(), 3);
    let program = assertion_project(assertion_materialized(function)).unwrap();
    assert_same_owner_induction_report_v2(&program);
    let report = &program.roots[0].semantic_u32_induction;
    assert!(report.uses_reachable_scope_v2());
    assert_eq!(report.ssa_scope_work_units_v2(), 3);
    assert_eq!(report.reachable_block_count_v2(), Some(1));
    assert_eq!(report.reachable_statement_count_v2(), Some(3));
    assert!(
        report
            .block_is_reachable_v2(SemanticBlockIdV1::from_index(0))
            .unwrap()
    );
    assert!(
        !report
            .block_is_reachable_v2(SemanticBlockIdV1::from_index(1))
            .unwrap()
    );
    assert_eq!(report.checked_additions_examined(), 0);
    assert!(report.certificates().is_empty());
    let receipt = program.into_verified_roster_receipt().unwrap();
    receipt.verify_equivalence().unwrap();
}

#[test]
fn default_reachable_induction_receipt_rechecks_the_actual_selected_body() {
    let program = assertion_project(wrapped_literal_assertion()).unwrap();
    assert_same_owner_induction_report_v2(&program);
    assert_eq!(program.roots[0].semantic_root, ROOT);
    assert_eq!(
        program.roots[0].semantic_u32_induction.function(),
        SemanticFunctionIdV1::from_index(1),
    );
    let receipt = program.into_verified_roster_receipt().unwrap();
    receipt.verify_equivalence().unwrap();
    assert_eq!(
        receipt.source_order_roots[0]
            .verification
            .semantic_u32_induction
            .function(),
        SemanticFunctionIdV1::from_index(1),
    );
}

#[test]
fn default_reachable_induction_receipt_rejects_a_real_foreign_source_report() {
    let program =
        assertion_project(assertion_materialized(literal_assertion(true, true, false))).unwrap();
    let foreign = assertion_project(assertion_materialized(literal_assertion(
        false, false, false,
    )))
    .unwrap();
    assert_same_owner_induction_report_v2(&program);
    assert_same_owner_induction_report_v2(&foreign);
    let foreign_report = foreign.roots[0].semantic_u32_induction.clone();
    assert_ne!(
        program.roots[0]
            .semantic_u32_induction
            .semantic_mir_sha256(),
        foreign_report.semantic_mir_sha256(),
    );
    let mut receipt = program.into_verified_roster_receipt().unwrap();
    receipt.verify_equivalence().unwrap();
    receipt.source_order_roots[0]
        .verification
        .semantic_u32_induction = foreign_report;
    assert!(matches!(
        receipt.verify_equivalence(),
        Err(ProductionRankedVerificationErrorV1::RosterMetadata(
            "changed per-root semantic induction custody"
        ))
    ));
}

#[test]
fn default_reachable_induction_receipt_creation_rejects_a_same_source_v1_report() {
    let mut program =
        assertion_project(assertion_materialized(literal_assertion(true, true, false))).unwrap();
    assert_same_owner_induction_report_v2(&program);
    let legacy = fe2o3_mir_model::analyze_semantic_u32_induction_no_overflow_v1(
        program.materialized.semantic_ssa().source_semantic(),
        program.roots[0].semantic_u32_induction.function(),
    )
    .unwrap();
    assert!(!legacy.uses_reachable_scope_v2());
    assert_eq!(legacy.ssa_scope_work_units_v2(), 0);
    assert_eq!(
        legacy.semantic_mir_sha256(),
        program.roots[0]
            .semantic_u32_induction
            .semantic_mir_sha256(),
    );
    program.roots[0].semantic_u32_induction = legacy;
    assert!(matches!(
        program.into_verified_roster_receipt(),
        Err(ProductionRankedVerificationErrorV1::RosterMetadata(
            "changed per-root semantic induction custody"
        ))
    ));
}
