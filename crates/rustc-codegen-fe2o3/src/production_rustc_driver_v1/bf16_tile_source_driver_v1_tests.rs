use super::*;

// Inert parser/report controls do not construct a rustc owner or claim a genuine
// frontend execution. Actual public-driver qualification remains separate.
fn request_bytes() -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "schema": "fe2o3-bf16-tile-source-promotion-request-v1",
        "semantic_sha256": "1".repeat(64), "canonical_sha256": "2".repeat(64),
        "mir_sha256": "3".repeat(64), "original_sha256": "4".repeat(64),
        "original_path": "src/original.rs", "candidate_path": "src/candidate.rs",
        "helper_name": "__fe2o3_bf16_tile_identity", "return_order": "identity",
    }))
    .unwrap()
}
#[test]
fn source_driver_requires_original_overflow_policy_before_io() {
    let args = ["rustc".into()];
    for result in [
        run_bf16_tile_source_inspection_driver_v1(&args, Path::new("/unopened-bf16-source")),
        run_bf16_tile_source_promotion_driver_v1(
            &args,
            Path::new("/unopened-bf16-source"),
            Path::new("/unopened-bf16-request"),
        ),
    ] {
        assert!(result.unwrap_err().contains("exactly one canonical"));
    }
}
#[test]
fn source_driver_output_names_are_explicit_and_bounded() {
    assert!(validate_output(Path::new("")).is_err());
    assert!(validate_output(Path::new(&"x".repeat(PATH_CAP + 1))).is_err());
    assert!(validate_output(Path::new("x\0y")).is_err());
    assert!(validate_output(Path::new("fresh-output")).is_ok());
}
#[test]
fn request_stream_honors_existing_exact_limit_and_stops_after_refusal_byte() {
    assert!(input::read_stream(&mut Cursor::new([])).is_err());
    let exact = input::read_stream(&mut Cursor::new(vec![b' '; input::REQUEST_CAP])).unwrap();
    assert_eq!(exact.bytes().len(), input::REQUEST_CAP);
    let mut larger = Cursor::new(vec![b' '; input::REQUEST_CAP + 100]);
    assert!(input::read_stream(&mut larger).is_err());
    assert_eq!(larger.position(), (input::REQUEST_CAP + 1) as u64);
}
#[test]
fn request_stream_errors_do_not_produce_accepted_bytes() {
    struct Broken;
    impl std::io::Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("injected read failure"))
        }
    }
    assert!(
        input::read_stream(&mut Broken)
            .err()
            .unwrap()
            .contains("injected read failure")
    );
}
#[test]
fn original_borrowed_request_parser_is_used_without_extra_authority() {
    let bytes = request_bytes();
    let parsed = Bf16TileSourcePublishRequestV1::parse(&bytes).unwrap();
    assert_eq!(parsed.semantic_sha256, [0x11; 32]);
    assert_eq!(parsed.canonical_sha256, [0x22; 32]);
    assert_eq!(parsed.original_path, "src/original.rs");
    assert_eq!(parsed.return_order.permutation(), [0, 1, 2, 3]);
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["resume"] = true.into();
    assert!(Bf16TileSourcePublishRequestV1::parse(&serde_json::to_vec(&value).unwrap()).is_err());
    value.as_object_mut().unwrap().remove("resume");
    value["return_order"] = "arbitrary".into();
    assert!(Bf16TileSourcePublishRequestV1::parse(&serde_json::to_vec(&value).unwrap()).is_err());
}
#[test]
fn original_request_exact_bound_is_not_enlarged() {
    let mut bytes = request_bytes();
    bytes.resize(input::REQUEST_CAP, b' ');
    assert!(Bf16TileSourcePublishRequestV1::parse(&bytes).is_ok());
    bytes.push(b' ');
    assert!(Bf16TileSourcePublishRequestV1::parse(&bytes).is_err());
}
#[test]
fn report_serialization_is_bounded_during_encoding_and_has_one_final_lf() {
    let mut bytes = [0; REPORT_CAP];
    let report = base_report("inspect");
    let encoded = encode_report(&report, &mut bytes).unwrap();
    assert!(encoded.ends_with(b"\n"));
    assert!(!encoded[..encoded.len() - 1].contains(&b'\n'));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(encoded).unwrap(),
        report
    );
    assert!(encode_report(&"x".repeat(REPORT_CAP), &mut bytes).is_err());
}
#[test]
fn report_publication_facts_are_lossless_and_not_admission() {
    let published = PublishedBf16TileSourceV1 {
        original_sha256: [1; 32],
        candidate_sha256: [2; 32],
        original_bytes: 100,
        candidate_bytes: 200,
        candidate_device: u64::MAX,
        candidate_inode: u64::MAX - 1,
        return_order: crate::production_tiled_region_source_v1::Bf16TileReturnOrderV1::Swap01,
    };
    let value = facts(&published);
    assert_eq!(value["candidate_device"], u64::MAX.to_string());
    assert_eq!(value["candidate_inode"], (u64::MAX - 1).to_string());
    assert_eq!(value["candidate_sha256"], "02".repeat(32));
    assert_eq!(value["return_order"], "swap01");
    for field in [
        "original_overwritten",
        "fresh_compilation_observed",
        "grants_compiler_or_launch_authority",
    ] {
        assert_eq!(value[field], false);
    }
}
#[test]
fn handled_failures_keep_both_original_reasons() {
    assert_eq!(combine_failures(None, None), None);
    assert_eq!(
        combine_failures(Some("publication".into()), None).as_deref(),
        Some("publication")
    );
    assert_eq!(
        combine_failures(None, Some("postflight".into())).as_deref(),
        Some("postflight")
    );
    assert_eq!(
        combine_failures(Some("publication".into()), Some("postflight".into())).as_deref(),
        Some("publication; postflight")
    );
}
#[test]
fn base_report_cannot_confuse_inspection_with_normal_or_hardware_results() {
    let value = base_report("inspect");
    assert_eq!(value["status"], "failed");
    assert_eq!(value["selection"], serde_json::Value::Null);
    assert_eq!(value["publication"], serde_json::Value::Null);
    for field in [
        "source_postflight_ok",
        "selection_is_compiler_custody",
        "candidate_compiled",
        "simulation_performed",
        "normal_ranked_admission_performed",
        "native_execution",
        "hardware_observed",
        "grants_artifact_or_launch_authority",
    ] {
        assert_eq!(value[field], false);
    }
    assert_eq!(value["memory_measurement"], "unavailable");
}

#[test]
fn failed_postflight_preserves_known_publication_in_the_written_receipt() {
    let mut report = base_report("promote");
    report["publication_effect"] = "may_have_created_candidate".into();
    report["publication"] = serde_json::json!({"candidate_sha256": "12".repeat(32)});
    let mut retained = None;
    let result = finish_report_with_writer(
        &report,
        Some("original postflight failure".into()),
        |bytes| {
            retained = Some(serde_json::from_slice::<serde_json::Value>(bytes).unwrap());
            Ok(())
        },
    );
    assert!(result.unwrap_err().contains("original postflight failure"));
    assert_eq!(retained.unwrap(), report);
    assert_eq!(report["source_postflight_ok"], false);
    assert_eq!(report["status"], "failed");
}
#[test]
fn failed_report_write_preserves_failure_and_possible_publication_warning() {
    let report = base_report("promote");
    let failure = finish_report_with_writer(
        &report,
        Some("source refused after publication".into()),
        |_| Err("injected output failure".into()),
    )
    .unwrap_err();
    for text in [
        "source refused after publication",
        "injected output failure",
        "candidate/output may remain",
        "no rollback",
    ] {
        assert!(failure.contains(text));
    }
}
#[test]
fn oversized_report_never_invokes_the_publication_writer() {
    let mut entered = false;
    assert!(
        finish_report_with_writer(&"x".repeat(REPORT_CAP), None, |_| {
            entered = true;
            Ok(())
        })
        .is_err()
    );
    assert!(!entered);
}
