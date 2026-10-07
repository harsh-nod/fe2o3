//! Boundary controls, not genuine source/target or Worker qualification.
use super::*;

#[test]
fn identity_and_swap01_require_the_actual_source_order() {
    for order in [[0, 1, 2, 3], [1, 0, 2, 3]] {
        assert!(source_profile(1, 0, 1, order, order, false).is_ok());
        assert!(source_profile(SOURCE_CAP, 0, 1, order, order, false).is_ok());
    }
    assert!(source_profile(1, 0, 1, [0, 1, 2, 3], [1, 0, 2, 3], false).is_err());
    assert!(source_profile(1, 0, 1, [1, 0, 2, 3], [0, 1, 2, 3], false).is_err());
}

#[test]
fn malformed_order_is_not_a_new_source_profile() {
    for order in [[0, 0, 2, 3], [3, 2, 1, 0], [0, 1, 3, 2], [0, 1, 2, 4]] {
        assert!(source_profile(1, 0, 1, order, order, false).is_err());
    }
}

#[test]
fn source_size_owner_and_authority_boundaries_refuse() {
    for bytes in [0, SOURCE_CAP + 1, usize::MAX] {
        assert!(source_profile(bytes, 0, 1, [0, 1, 2, 3], [0, 1, 2, 3], false).is_err());
    }
    assert!(source_profile(1, 1, 1, [0, 1, 2, 3], [0, 1, 2, 3], false).is_err());
    assert!(source_profile(1, 0, 1, [0, 1, 2, 3], [0, 1, 2, 3], true).is_err());
}

#[test]
fn connector_is_non_test_typed_custody_without_worker_or_default_dispatch() {
    let source = include_str!("bf16_same_owner_handoff_v1.rs");
    let body = source.split("#[cfg(test)]").next().unwrap();
    for required in [
        "self.prepare_bf16_tile_values_source_v1()?",
        "ssa.with_retained_materialization_budget_v1(",
        "materialize_prepared_bf16_source_v1(",
        "verify_bf16_nominal_equivalence_with_budget_v1(budget)",
        "std::ptr::eq(emission.owner(), &materialized)",
        ".verify_private_nominal_kernel_checks_v1()",
        ".verify_private_bf16_output_guarded_safety_v1(requested_return)?",
        ".revalidate_private_bf16_worker_handoff_retained_v1(requested_return)?",
    ] {
        assert!(body.contains(required), "{required}");
    }
    for forbidden in [
        "for_test_v1(",
        "execute_v2(",
        "PinnedWorkerV1",
        "run_compiler(",
        "Work::new(",
        "Budget::new(",
        "pub fn ",
        "serde",
    ] {
        assert!(!body.contains(forbidden), "{forbidden}");
    }
    assert!(
        body.find("verify_bf16_nominal_equivalence").unwrap()
            < body.find(".verify_private_nominal_kernel_checks").unwrap()
    );
    assert!(
        body.find(".revalidate_private_bf16_worker_handoff")
            .unwrap()
            < body.find("Ok(Bf16SameOwnerHandoffV1").unwrap()
    );
}

#[test]
fn public_refusal_entry_and_legacy_constructor_remain_separate() {
    let public = include_str!("bf16_generated_source_admission_v1.rs");
    assert!(public.contains("ordinary.verify_general_kernel_checks()"));
    assert!(public.contains("require_nominal_ranked_refusal(error)"));
    assert!(!public.contains("prepare_bf16_same_owner_handoff_v1"));
    let source = include_str!("bf16_tile_values_v1.rs");
    let legacy = source
        .split("pub(super) fn materialize_with_bf16_tile_values_inspection_v1")
        .nth(1)
        .unwrap()
        .split("fn materialize_bf16_tile_values_with_mode_v1")
        .next()
        .unwrap();
    assert!(legacy.contains("MaterializationModeV1::Legacy"));
    assert!(!legacy.contains("NominalInspection"));
    assert_eq!(
        source
            .matches("try_materialize_bf16_nominal_with_budget_v1(")
            .count(),
        1
    );
    assert_eq!(source.matches("try_materialize_with_budget(").count(), 1);
    assert_eq!(
        source
            .matches("source_seed.with_relation(relation, budget, inspect)")
            .count(),
        1
    );
}

#[test]
fn opaque_handoff_adds_no_payload_extraction_or_clone() {
    let source = include_str!("bf16_same_owner_handoff_v1.rs");
    let opaque = source
        .split("pub(crate) struct Bf16SameOwnerHandoffV1")
        .nth(1)
        .unwrap()
        .split("impl<'tcx>")
        .next()
        .unwrap();
    assert!(
        opaque.contains(
            "phase: RetainedMaterializationPhaseV1<PrivateBf16WorkerHandoffCompilationV1>"
        )
    );
    assert!(!opaque.contains("pub(crate) phase"));
    assert!(!opaque.contains("into_"));
    assert!(!opaque.contains("Clone"));
    assert!(opaque.contains("const fn grants_artifact_or_launch_authority(&self) -> bool"));
    assert!(opaque.contains("false"));
}
