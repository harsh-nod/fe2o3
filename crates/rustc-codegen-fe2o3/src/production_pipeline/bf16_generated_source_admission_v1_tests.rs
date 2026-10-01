//! Synthetic boundary and exact source-wiring controls, not rustc admission.
use super::*;

fn unavailable_consumer(consumer: &'static str) -> ProductionPipelineError {
    ProductionPipelineError::RankedProjection(
        crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1::StructuralValidation(
            fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::LocalHelperSourceConsumerUnavailable { consumer },
        ),
    )
}

#[test]
fn exact_nominal_ranked_consumer_only_is_an_expected_refusal() {
    assert!(require_nominal_ranked_refusal(unavailable_consumer(NOMINAL_RANKED_REFUSAL)).is_ok());
    for other in [
        "source-ranked projection",
        "",
        "BF16 nominal source-ranked projection changed",
    ] {
        assert!(require_nominal_ranked_refusal(unavailable_consumer(other)).is_err());
    }
}

#[test]
fn matching_error_text_from_a_different_typed_boundary_is_not_acceptance() {
    let error = unavailable(NOMINAL_RANKED_REFUSAL);
    assert!(require_nominal_ranked_refusal(error).is_err());
    assert!(require_nominal_ranked_refusal(
        ProductionPipelineError::RankedProjection(
            crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1::StructuralValidation(
                fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::CorrespondenceMismatch,
            ),
        ),
    ).is_err());
}

#[test]
fn only_identity_and_swap01_are_supported() {
    assert!(supported_permutation([0, 1, 2, 3]));
    assert!(supported_permutation([1, 0, 2, 3]));
    for wrong in [[0, 0, 2, 3], [3, 2, 1, 0], [0, 1, 3, 2], [0, 1, 2, 4]] {
        assert!(!supported_permutation(wrong));
    }
}

#[test]
fn copied_facts_are_fixed_bounded_and_not_live_owners() {
    fn copy<T: Copy>() {}
    copy::<Bf16GeneratedSourceAdmissionV1>();
    assert!(std::mem::size_of::<Bf16GeneratedSourceAdmissionV1>() <= 512);
    assert_eq!(SOURCE_CAP, 65_536);
}

#[test]
fn old_entry_selects_only_legacy_constructor_and_shared_postflight() {
    let s = include_str!("bf16_tile_values_v1.rs");
    let old = s
        .split("pub(super) fn materialize_with_bf16_tile_values_inspection_v1")
        .nth(1)
        .unwrap()
        .split("    fn materialize_bf16_tile_values_with_mode_v1")
        .next()
        .unwrap();
    assert!(old.contains("MaterializationModeV1::Legacy"));
    assert!(!old.contains("NominalInspection"));
    for preserved in [
        "self.prepare_bf16_tile_values_source_v1()?",
        "source_seed.with_relation(relation, budget, inspect)",
        "budget.work_ledger_identity_v1() != ledger",
        "budget.storage() < protected",
        "drop(source_seed)",
        "release_storage(owned)",
    ] {
        assert!(s.contains(preserved), "{preserved}");
    }
    assert_eq!(
        s.matches("try_materialize_bf16_nominal_with_budget_v1(")
            .count(),
        1
    );
    assert_eq!(s.matches("try_materialize_with_budget(").count(), 1);
}

#[test]
fn public_inspection_reaches_existing_normal_gate_and_has_no_test_seam() {
    let s = include_str!("bf16_generated_source_admission_v1.rs");
    assert!(s.contains("ordinary.verify_general_kernel_checks()"));
    assert!(s.contains("require_nominal_ranked_refusal(error)"));
    assert!(s.contains("owner.executable().canonical().identity().digest()"));
    for forbidden in [
        "for_test_v1(",
        "run_compiler(",
        "Work::new(",
        "Budget::new(",
        "publish_bf16",
    ] {
        assert!(!s.contains(forbidden), "{forbidden}");
    }
}
