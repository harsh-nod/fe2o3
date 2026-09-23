//! Pure public-boundary controls; actual rustc/CPU qualification is separate.
use super::*;
fn request() -> SourceLocalOrderRecipeRequestV1 {
    SourceLocalOrderRecipeRequestV1::create(
        "src/lib.rs",
        [1; 32],
        SourceLocalOrderOrderV1::SourceOrder,
        SourceLocalOrderRelationV1::XorBeforeOr,
        SourceLocalOrderStrengthV1::Exact,
        SourceLocalOrderSourceBindingModeV1::RebindCurrent,
    )
    .unwrap()
}
fn recipe() -> Vec<u8> {
    codec::Recipe::new(
        codec::InstanceBinding {
            function: [1; 32],
            item: [2; 32],
            monomorphization: [3; 32],
            generic_types: [4; 32],
            const_arguments: [5; 32],
        },
        codec::Origin {
            source: [6; 32],
            semantic: [7; 32],
            bound: [8; 32],
        },
        codec::Order::SourceOrder,
        codec::Constraint {
            relation: codec::Relation::XorBeforeOr,
            strength: codec::Strength::Exact,
        },
        codec::SourceBinding::RebindCurrent {},
    )
    .encode()
    .unwrap()
}
fn output() -> SourceLocalOrderRecipeOutputV1 {
    let identity = SourceLocalOrderIdentityObservationV1 {
        digest: [0; 32],
        canonical_length: 0,
    };
    SourceLocalOrderRecipeOutputV1 {
        llvm: "test-only inert stub".into(),
        created_recipe: None,
        evidence: SourceLocalOrderRecipeEvidenceV1 {
            source_initializer: [0; 4],
            source_sha256: [0; 32],
            semantic_sha256: [0; 32],
            instance_axes: [[0; 32]; 5],
            original: identity,
            input: identity,
            output: identity,
            requested_order: SourceLocalOrderOrderV1::SourceOrder,
            requested_relation: SourceLocalOrderRelationV1::XorBeforeOr,
            strength: SourceLocalOrderStrengthV1::Exact,
            source_binding_mode: SourceLocalOrderSourceBindingModeV1::RebindCurrent,
            actual_relation: SourceLocalOrderRelationV1::XorBeforeOr,
            constraint_outcome: SourceLocalOrderConstraintOutcomeV1::Honored {
                relation: SourceLocalOrderRelationV1::XorBeforeOr,
            },
            region: [0; 4],
            output_result_order: [0; 3],
            prefix_execution_bytes: [0; 256],
            transition_sha256: [0; 32],
            transition_bytes: 0,
            transition_rows: [0; 9],
            fresh_formal_counts: [0; 5],
            llvm_sha256: [0; 32],
            descriptor_sha256: [0; 32],
            recipe_sha256: [0; 32],
            canonical_work: 0,
            canonical_peak_storage: 0,
            created: true,
        },
    }
}
#[test]
fn create_and_replay_snapshots_return_original_owned_request_without_recipe_file_claim() {
    let create = request();
    assert!(create.is_create());
    assert_eq!(create.replay_recipe_bytes(), None);
    assert!(
        create.retained_input_storage()
            >= size_of::<SourceLocalOrderRecipeRequestV1>() + create.source_path().len()
    );
    let mut bytes = recipe();
    let expected = bytes.clone();
    let replay = SourceLocalOrderRecipeRequestV1::replay("src/lib.rs", [1; 32], &bytes).unwrap();
    bytes[0] = b' ';
    assert_eq!(replay.replay_recipe_bytes(), Some(expected.as_slice()));
    assert!(!replay.is_create());
    let retained = replay.retained_input_storage();
    let attempt = SourceLocalOrderRecipeAttemptV1::new(
        replay,
        Err(SourceLocalOrderRecipeFailureV1::new(
            SourceLocalOrderRecipeFailurePhaseV1::Request,
            "unit refusal".into(),
        )),
        0,
        0,
    );
    assert_eq!(attempt.request().retained_input_storage(), retained);
    assert_eq!(attempt.callback_count(), 0);
    assert_eq!(attempt.compiler_callback_count(), 0);
    let (request, result) = attempt.into_parts();
    assert!(result.is_err());
    assert_eq!(request.replay_recipe_bytes(), Some(expected.as_slice()));
}
#[test]
fn request_path_and_recipe_bounds_fail_without_starting_rustc() {
    for path in ["", "/tmp/absolute.rs", "../escape.rs", "src/../lib.rs"] {
        assert!(
            SourceLocalOrderRecipeRequestV1::create(
                path,
                [0; 32],
                SourceLocalOrderOrderV1::SourceOrder,
                SourceLocalOrderRelationV1::XorBeforeOr,
                SourceLocalOrderStrengthV1::Exact,
                SourceLocalOrderSourceBindingModeV1::ExactRevision
            )
            .is_err()
        );
    }
    for bytes in [Vec::new(), vec![b' '; codec::BYTE_CAP + 1], b"{}".to_vec()] {
        let error = SourceLocalOrderRecipeRequestV1::replay("src/lib.rs", [0; 32], &bytes)
            .err()
            .unwrap();
        assert_eq!(error.phase(), SourceLocalOrderRecipeFailurePhaseV1::Request);
    }
}
#[test]
fn exact_current_source_hash_is_an_independent_in_callback_refusal() {
    assert!(require_current_source_revision(&[1; 32], &[1; 32]).is_ok());
    let error = require_current_source_revision(&[1; 32], &[2; 32]).unwrap_err();
    assert_eq!(
        error.phase(),
        SourceLocalOrderRecipeFailurePhaseV1::SourceCurrentness
    );
    assert_eq!(
        error.diagnostic(),
        "local-order recipe expected current source revision differs"
    );
}
#[test]
fn bounded_argv_and_normal_driver_preflight_do_not_enter_callback() {
    assert!(validate_arguments(&[]).is_err());
    assert!(validate_arguments(&vec![String::new(); 4097]).is_err());
    assert!(validate_arguments(&["x".repeat(1024 * 1024 + 1)]).is_err());
    assert!(validate_arguments(&["x".repeat(1024 * 1024)]).is_ok());
    let attempt = crate::production_rustc_driver_v1::source_local_order_recipe_driver_v1::
        run_source_local_order_recipe_driver_v1(&[],request());
    assert_eq!(attempt.callback_count(), 0);
    assert_eq!(attempt.compiler_callback_count(), 0);
    assert_eq!(
        attempt.result().unwrap_err().phase(),
        SourceLocalOrderRecipeFailurePhaseV1::Request
    );
}
#[test]
fn no_callback_reentry_or_genuine_fatal_can_expose_a_prior_success() {
    assert!(finish_callback(Some(Ok(output())), 1, false).is_ok());
    for (calls, fatal) in [(0, false), (2, false), (1, true), (2, true)] {
        let error = finish_callback(Some(Ok(output())), calls, fatal).unwrap_err();
        assert_eq!(
            error.phase(),
            SourceLocalOrderRecipeFailurePhaseV1::Frontend
        );
        assert_eq!(error.compiler_fatal(), fatal);
    }
    assert!(finish_callback(None, 1, false).is_err());
}
#[test]
fn first_exact_refusal_is_preserved_and_fatal_flag_is_added() {
    let error = SourceLocalOrderRecipeFailureV1::new(
        SourceLocalOrderRecipeFailurePhaseV1::Constraint,
        "exact refusal".into(),
    );
    let failure = finish_callback(Some(Err(error)), 2, true).unwrap_err();
    assert_eq!(
        failure.phase(),
        SourceLocalOrderRecipeFailurePhaseV1::Constraint
    );
    assert_eq!(failure.diagnostic(), "exact refusal");
    assert!(failure.compiler_fatal());
}
#[test]
fn bounded_utf8_diagnostics_and_byte_copy_refuse_without_overwrite() {
    let error = SourceLocalOrderRecipeFailureV1::new(
        SourceLocalOrderRecipeFailurePhaseV1::Request,
        "é".repeat(5000),
    );
    assert!(error.diagnostic().len() <= DIAGNOSTIC_BYTE_CAP);
    assert!(
        error
            .diagnostic()
            .is_char_boundary(error.diagnostic().len())
    );
    assert!(copy_bytes(&[], 8).is_err());
    assert!(copy_bytes(&[0; 9], 8).is_err());
    assert_eq!(copy_bytes(&[1, 2, 3], 3).unwrap(), vec![1, 2, 3]);
}
#[test]
fn test_observer_is_cleared_on_early_refusal_without_invocation() {
    let observer: test_support::Observer =
        Box::new(|_, _| panic!("early refusal must not inspect a graph"));
    let result = test_support::with_observer(Some(observer), || {
        Err(SourceLocalOrderRecipeFailureV1::new(
            SourceLocalOrderRecipeFailurePhaseV1::RecipeBinding,
            "binding refusal".into(),
        ))
    });
    assert_eq!(result.unwrap_err().diagnostic(), "binding refusal");
    assert!(test_support::with_observer(None, || Ok(output())).is_ok());
}
