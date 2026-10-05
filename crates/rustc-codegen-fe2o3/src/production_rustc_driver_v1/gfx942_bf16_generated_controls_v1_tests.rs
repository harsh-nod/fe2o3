//! Inert parser/oracle/typed-refusal controls; no compiler authority is fabricated.
use super::*;

fn config_value() -> Value {
    json!({"schema":"fe2o3-bf16-generated-session-config-v1","cwd":"/task",
        "session":0,"record":"prepare/invocation.json","record_sha256":"11".repeat(32),
        "candidate":"candidate/src/lib.rs","helper_name":"__fe2o3_generated_bf16",
        "sidecar":"evidence/cpu.bin","observation":"evidence/observation.json"})
}
#[test]
fn closed_configs_reject_unknown_and_duplicate_fields() {
    let mut value = config_value();
    assert!(serde_json::from_value::<Config>(value.clone()).is_ok());
    value["extra"] = json!(true);
    assert!(serde_json::from_value::<Config>(value).is_err());
    let encoded = serde_json::to_string(&config_value()).unwrap();
    let duplicate = encoded.replacen("{", "{\"session\":0,", 1);
    assert!(serde_json::from_str::<Config>(&duplicate).is_err());
    let mut value = config_value();
    value["session"] = json!(-1);
    assert!(serde_json::from_value::<Config>(value).is_err());
    let mut value = config_value();
    value["session"] = json!(1.5);
    assert!(serde_json::from_value::<Config>(value).is_err());
}
#[test]
fn task_paths_and_exact_digest_grammar_do_not_accept_traversal() {
    for bad in ["", "/absolute", "../outside", "safe/../../outside"] {
        assert!(inputs::relative(bad).is_err());
    }
    assert!(inputs::relative("candidate-identity/src/lib.rs").is_ok());
    assert!(!inputs::lower_digest(&"F".repeat(64)));
    assert!(!inputs::lower_digest(&"1".repeat(63)));
    assert!(inputs::lower_digest(&"ab".repeat(32)));
}
#[test]
fn every_final_frame_integer_is_lossless_text_and_booleans_stay_booleans() {
    let mut value = json!({"maximum":u64::MAX,"safe":7,"signed":-1,
        "nested":[0,1,true,false,null],"text":"0"});
    integral_strings(&mut value, 0, &mut 0).unwrap();
    assert_eq!(value["maximum"], "18446744073709551615");
    assert_eq!(value["safe"], "7");
    assert_eq!(value["signed"], "-1");
    assert_eq!(value["nested"], json!(["0", "1", true, false, null]));
    assert_eq!(value["text"], "0");
    assert!(integral_strings(&mut json!(1.25), 0, &mut 0).is_err());
    let mut nodes = 16384;
    assert!(integral_strings(&mut json!(0), 0, &mut nodes).is_err());
    assert!(integral_strings(&mut json!(0), 33, &mut 0).is_err());
}
#[test]
fn only_exact_typed_post_materialization_bf16_normal_refusal_matches() {
    use crate::production_pipeline::ProductionPipelineError as P;
    use crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1 as R;
    use fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1 as K;
    let expected = P::RankedProjection(R::StructuralValidation(
        K::LocalHelperSourceConsumerUnavailable {
            consumer: "BF16 nominal source-ranked projection",
        },
    ));
    assert!(is_expected_normal_refusal(&expected));
    for consumer in [
        "source-ranked projection",
        "helper parameter is not an exact by-value scalar aggregate or shared slice",
        "prefix BF16 nominal source-ranked projection suffix",
    ] {
        let wrong = P::RankedProjection(R::StructuralValidation(
            K::LocalHelperSourceConsumerUnavailable { consumer },
        ));
        assert!(!is_expected_normal_refusal(&wrong));
    }
    assert!(!is_expected_normal_refusal(&P::ExtractionCannotPublish));
}
#[test]
fn originals_do_not_inherit_the_requested_future_swap() {
    for session in 0..4 {
        let stream = tap::Stream::new(session, PathBuf::from("inert-not-opened")).unwrap();
        assert_eq!(
            stream.requested_permutation(),
            if session == 3 {
                [1, 0, 2, 3]
            } else {
                [0, 1, 2, 3]
            }
        );
    }
    assert!(tap::Stream::new(4, PathBuf::from("inert-not-opened")).is_err());
}
#[test]
fn independent_nonsymmetric_cases_distinguish_wrong_return_and_output_order() {
    use crate::production_rustc_driver_v1::gfx942_tiled_region_qualification_v1_tests::observation::cpu::oracle;
    for pattern in [1, 2] {
        for length in [64, 13] {
            let dense = oracle::expected(pattern);
            let mut return_diff = false;
            let mut output_diff = false;
            for lane in 0..64 {
                for c in 0..4 {
                    return_diff |= oracle::lane_word(&dense, lane, c)
                        != oracle::lane_word(&dense, lane, [1, 0, 2, 3][c]);
                }
                if lane < length {
                    output_diff |=
                        oracle::lane_word(&dense, lane, 0) != oracle::lane_word(&dense, lane, 1);
                }
            }
            assert!(return_diff && output_diff);
        }
    }
    let zeros = oracle::expected(0);
    assert!((0..64)
        .all(|lane| oracle::lane_word(&zeros, lane, 0) == oracle::lane_word(&zeros, lane, 1)));
}
#[test]
fn bounded_diagnostic_stops_at_utf8_boundary_without_ellipsis_overflow() {
    let value = "é".repeat(400);
    let observed = diagnostic(&value);
    assert!(observed.len() <= 512);
    assert!(observed.is_char_boundary(observed.len()));
    assert_eq!(observed.len(), 512);
    assert_eq!(diagnostic(&"short"), "short");
}
#[test]
fn original_selected_fixture_pin_and_closed_sidecar_completion_are_fixed() {
    let bytes = include_bytes!("../../tests/fixtures/tiled-region-inspection-v1/src/lib.rs");
    assert_eq!(bytes.len() as u64, ORIGINAL_BYTES);
    assert_eq!(digest(bytes), ORIGINAL_SHA);
    assert_eq!(
        super::super::gfx942_bf16_publication_sidecar_v1_tests::COMPLETE_BYTES,
        105440
    );
}
