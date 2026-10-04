//! Pure controls only; the ignored operational child is separately qualified.
use super::*;
use std::io::{Cursor, Write};

fn config() -> Config {
    Config {
        schema: SCHEMA.into(),
        mode: Mode::Ordinary,
        source: "src/lib.rs".into(),
        source_sha256: "11".repeat(32),
        rustc_args: vec!["rustc".into()],
        oracle: None,
        intent: Intent::Create {
            order: "source_order".into(),
            relation: "xor_before_or".into(),
            strength: "exact".into(),
            binding: "rebind_current".into(),
        },
    }
}
fn samples() -> Vec<Sample> {
    (0..35)
        .map(|i| Sample {
            ordinal: i + 1,
            calibration: i < 5,
            elapsed_ns: if i < 5 { u64::MAX } else { (i - 4) as u64 },
            normal_bytes: 2,
            normal_sha256: "00".repeat(32),
            exact_external_oracle_equal: true,
        })
        .collect()
}
#[test]
fn exactly_five_calibration_thirty_measured_and_nearest_ranks() {
    assert_eq!(
        stats(&samples()).unwrap(),
        Stats {
            p50_ns: 15,
            p95_ns: 29,
            max_ns: 30
        }
    );
    assert!(stats(&samples()[..34]).is_err());
    let mut rows = samples();
    rows[5].calibration = true;
    assert!(stats(&rows).is_err());
    rows[5].calibration = false;
    rows[7].ordinal = 99;
    assert!(stats(&rows).is_err());
    rows[7].ordinal = 8;
    rows[8].exact_external_oracle_equal = false;
    assert!(stats(&rows).is_err());
}
#[test]
fn target_comparison_keeps_outlier_and_uses_inclusive_p95() {
    let mut rows = samples();
    for r in &mut rows[5..] {
        r.elapsed_ns = TARGET_NS;
    }
    rows[34].elapsed_ns = TARGET_NS + 1;
    let value = stats(&rows).unwrap();
    assert_eq!(value.p95_ns, TARGET_NS);
    assert!(value.max_ns > TARGET_NS);
    rows[33].elapsed_ns = TARGET_NS + 1;
    assert!(stats(&rows).unwrap().p95_ns > TARGET_NS);
}
#[test]
fn exact_external_byte_oracle_does_not_normalize_json() {
    assert_eq!(compare(b"{\"a\":1}\n", Some(b"{\"a\":1}\n")).unwrap(), true);
    assert!(compare(b"{ \"a\":1}\n", Some(b"{\"a\":1}\n")).is_err());
    assert!(compare(b"{\"a\":1}", Some(b"{\"a\":1}\n")).is_err());
    assert_eq!(compare(b"x", None).unwrap(), false);
}
#[test]
fn bounded_serialization_and_read_preserve_exact_endpoint() {
    assert_eq!(json_bytes(&7_u32, 2).unwrap(), b"7\n");
    assert!(json_bytes(&7_u32, 1).is_err());
    let mut writer = Bounded::new(3);
    writer.write_all(b"ab\n").unwrap();
    assert!(writer.write_all(b"x").is_err());
    assert_eq!(writer.bytes, b"ab\n");
    assert_eq!(read_payload(&mut Cursor::new(b"abc"), 3).unwrap(), b"abc");
    assert!(read_payload(&mut Cursor::new(b"abcd"), 3).is_err());
    assert!(read_payload(&mut Cursor::new(b""), 3).is_err());
    assert!(read_payload(&mut Cursor::new(b"a"), usize::MAX).is_err());
}
#[test]
fn current_request_admission_is_used_and_invalid_closed_options_refuse() {
    let mut value = config();
    let (request, guard) = prepare_request(&value).unwrap();
    assert!(request.is_create());
    assert!(guard.is_none());
    assert_eq!(request.source_path(), "src/lib.rs");
    value.source = "../escape.rs".into();
    assert!(prepare_request(&value).is_err());
    value.source = "src/lib.rs".into();
    if let Intent::Create { order, .. } = &mut value.intent {
        *order = "caller_pass_list".into();
    }
    assert!(prepare_request(&value).is_err());
}
#[test]
fn hashes_and_diagnostics_are_bounded_without_lossy_paths() {
    assert_eq!(hash(&"00".repeat(32)).unwrap(), [0; 32]);
    for bad in ["", "GG", &"A".repeat(64), &"0".repeat(63)] {
        assert!(hash(bad).is_err());
    }
    assert_eq!(
        sha(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(diagnostic("é".repeat(300)).len(), 512);
}
#[test]
fn config_is_closed_and_retained_metric_is_not_synthesized() {
    let text = r#"{"schema":"fe2o3-recipe-series-input-v1","mode":"ordinary","source":"src/lib.rs","source_sha256":"","intent":{"action":"create","order":"source_order","relation":"xor_before_or","strength":"exact","binding":"rebind_current"},"rustc_args":["rustc"],"oracle":null}"#;
    assert!(serde_json::from_str::<Config>(text).is_ok());
    let extra = text.replace("\"oracle\":null", "\"oracle\":null,\"resume_owner\":{}");
    assert!(serde_json::from_str::<Config>(&extra).is_err());
    assert!(serde_json::from_str::<Config>(&text.replace("\"ordinary\"", "\"cancel\"")).is_err());
}

#[test]
fn upfront_reservations_are_finite_nonresetting_and_checked() {
    let mut reservation = Reservations::new(true).unwrap();
    assert_eq!(
        reservation
            .envelope
            .maximum_selected_retained_source_read_bytes,
        110_100_585
    );
    assert_eq!(
        reservation.envelope.maximum_selected_recipe_read_bytes,
        860_265
    );
    assert_eq!(
        reservation
            .envelope
            .maximum_additional_capture_path_read_bytes,
        2_293_795
    );
    for _ in 0..CALLS {
        reservation.next().unwrap();
    }
    assert_eq!(reservation.remaining, 0);
    assert!(reservation.next().is_err());
    assert_eq!(reservation.remaining, 0);
    assert_eq!(
        Reservations::new(false)
            .unwrap()
            .envelope
            .maximum_selected_recipe_read_bytes,
        0
    );
    assert!(checked_envelope(usize::MAX, true).is_err());
}

#[test]
fn normal_record_starts_on_a_new_line_and_retains_write_failures() {
    let mut output = b"test exact ... ".to_vec();
    emit_normal_to(&mut output, b"{}\n").unwrap();
    assert_eq!(output, b"test exact ... \nFE2O3_RECIPE_NORMAL_V1 {}\n");
    let mut short = Bounded::new(1);
    assert!(emit_normal_to(&mut short, b"{}\n").is_err());
    assert_eq!(short.bytes, b"\n");
    let mut untouched = Vec::new();
    assert!(emit_normal_to(&mut untouched, &vec![0; NORMAL_LIMIT + 1]).is_err());
    assert!(untouched.is_empty());
}
