//! Wiring controls, not admitted-source evidence. Actual neutral/formal engine
//! controls live in the lowerer; the root-owned paired frontend checks outputs.
fn compact(source: &str) -> String {
    source.split_whitespace().collect()
}

#[test]
fn default_attachment_and_formal_entries_select_legacy_budgets() {
    let source = include_str!("../production_pipeline.rs");
    let attach = source
        .split("    fn attach_target_neutral_checks(\n")
        .nth(1)
        .unwrap()
        .split("    // Only the helper translation")
        .next()
        .unwrap();
    assert!(attach.contains("self.attach_target_neutral_checks_with_translation_budget_v1(None)"));
    let formal = source
        .split("    fn admit_formal_memory(\n")
        .nth(1)
        .unwrap()
        .split("    // Formal obligation derivation")
        .next()
        .unwrap();
    assert!(formal.contains("self.admit_formal_memory_with_translation_budget_v1(None)"));
}

#[test]
fn existing_custody_only_transitions_still_call_legacy_entries() {
    let source = include_str!("retained_target_pipeline_v1.rs");
    let attach = source
        .split("fn attach_target_neutral_checks_retained_v1(")
        .nth(1)
        .unwrap()
        .split("\n}\n")
        .next()
        .unwrap();
    assert!(compact(attach).contains("owner.attach_target_neutral_checks().map_err(Box::new)"));
    let formal = source
        .split("fn admit_formal_memory_retained_v1(")
        .nth(1)
        .unwrap()
        .split("\n}\n")
        .next()
        .unwrap();
    assert!(compact(formal).contains("owner.admit_formal_memory().map_err(Box::new)"));
}

#[test]
fn paid_transitions_borrow_the_existing_account_without_reconstruction() {
    let source = include_str!("retained_target_pipeline_v1.rs");
    for (method, call) in [
        (
            "fn attach_target_neutral_checks_with_source_translation_budget_v1(",
            "owner.attach_target_neutral_checks_with_translation_budget_v1(Some(original_account))",
        ),
        (
            "fn admit_formal_memory_with_source_translation_budget_v1(",
            "owner.admit_formal_memory_with_translation_budget_v1(Some(original_account))",
        ),
    ] {
        let body = source
            .split(method)
            .nth(1)
            .unwrap()
            .split("\n}\n")
            .next()
            .unwrap();
        assert!(body.contains("self.try_map(|owner, original_account|"));
        assert!(compact(body).contains(&compact(call)));
        for forbidden in [
            "OwnedBudget::new",
            "Work::new",
            "Budget::new",
            "finish_copy",
            ".clone(",
        ] {
            assert!(!body.contains(forbidden), "{forbidden}");
        }
    }
}

#[test]
fn budgeted_formal_entry_is_a_public_typed_consuming_api() {
    let _: fn(
        fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> std::result::Result<
        fe2o3_lower_mir_kernel::ProductionFormalMemoryOwnerV1,
        fe2o3_lower_mir_kernel::ProductionFormalMemoryErrorV1,
    > = fe2o3_lower_mir_kernel::ProductionFormalMemoryOwnerV1::
        try_admit_with_bounded_translation_budget_v1;
}
