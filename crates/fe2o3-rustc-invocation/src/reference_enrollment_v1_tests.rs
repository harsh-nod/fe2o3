use std::convert::Infallible;
use std::ffi::OsString;

use super::*;

const REQUEST: &str =
    r#"{"version":1,"bindings":[{"kernel":"demo::write","reference":"demo::spec"}]}"#;

fn decode(
    text: &str,
) -> Result<ReferenceEnrollmentRequestV1, ReferenceEnrollmentDecodeErrorV1<Infallible>> {
    ReferenceEnrollmentRequestV1::decode(text, |_| Ok(()))
}

fn environment(entries: &[(&str, &str)]) -> CompileEnvironmentV2 {
    CompileEnvironmentV2::from_child_environment(
        entries
            .iter()
            .map(|&(key, value)| (OsString::from(key), OsString::from(value))),
    )
    .unwrap()
}

#[test]
fn selectors_are_read_only_descriptions_with_no_name_matching_requirement() {
    let request = decode(REQUEST).unwrap();
    assert_eq!(request.version(), 1);
    assert_eq!(request.bindings()[0].kernel(), "demo::write");
    assert_eq!(request.bindings()[0].reference(), "demo::spec");
    let request = decode(
        r#"{"version":1,"bindings":[{"kernel":"a","reference":"spec"},{"kernel":"b","reference":"spec"}]}"#,
    ).unwrap();
    assert_eq!(request.bindings().len(), 2);
    assert_eq!(request.bindings()[1].reference(), "spec");
}

#[test]
fn rejects_unknown_duplicate_missing_fields_and_non_object_shapes() {
    for text in [
        r#"[1,[{"kernel":"a","reference":"b"}]]"#,
        r#"{"version":1,"bindings":[["a","b"]]}"#,
        r#"{"version":1,"version":1,"bindings":[]}"#,
        r#"{"version":1,"vers\u0069on":1,"bindings":[]}"#,
        r#"{"version":1,"bindings":[],"bindings":[]}"#,
        r#"{"version":1,"bindings":[],"authority":true}"#,
        r#"{"version":1,"bindings":[{"kernel":"a","kernel":"a","reference":"b"}]}"#,
        r#"{"version":1,"bindings":[{"kernel":"a","reference":"b","reference":"b"}]}"#,
        r#"{"version":1,"bindings":[{"kernel":"a","reference":"b","proof":true}]}"#,
        r#"{"bindings":[{"kernel":"a","reference":"b"}]}"#,
        r#"{"version":1,"bindings":[{"kernel":"a"}]}"#,
        r#"{"version":1,"bindings":null}"#,
        r#"{"version":1,"bindings":[]} null"#,
        r#"{"version":"1","bindings":[]}"#,
        r#"{"version":1,"bindings":[null]}"#,
    ] {
        assert_eq!(
            decode(text).unwrap_err().to_string(),
            "invalid reference enrollment request schema",
            "{text}"
        );
    }
}

#[test]
fn version_empty_roster_and_unsorted_or_duplicate_roots_keep_distinct_errors() {
    assert_eq!(
        decode(&REQUEST.replace("\"version\":1", "\"version\":2"))
            .unwrap_err()
            .to_string(),
        "unsupported reference enrollment request version"
    );
    assert_eq!(
        decode(r#"{"version":1,"bindings":[]}"#)
            .unwrap_err()
            .to_string(),
        "reference enrollment binding count outside limits"
    );
    for keys in [["a", "a"], ["b", "a"]] {
        let text = serde_json::json!({"version":1,"bindings":[
            {"kernel":keys[0],"reference":"r"}, {"kernel":keys[1],"reference":"r"}
        ]})
        .to_string();
        assert_eq!(
            decode(&text).unwrap_err().to_string(),
            "reference enrollment roots must be unique and sorted"
        );
    }
    assert_eq!(
        decode(r#"{"version":1,"bindings":[{"kernel":"a","reference":"r"},{"kernel":"\u0061","reference":"r"}]}"#)
            .unwrap_err().to_string(),
        "reference enrollment roots must be unique and sorted"
    );
}

#[test]
fn selector_limits_count_decoded_utf8_bytes_and_reject_controls() {
    for value in [
        String::new(),
        "x".repeat(MAX_REFERENCE_ENROLLMENT_SELECTOR_BYTES_V1 + 1),
        "\u{e9}".repeat(MAX_REFERENCE_ENROLLMENT_SELECTOR_BYTES_V1 / 2 + 1),
        "a\nb".to_owned(),
        "\u{85}".to_owned(),
    ] {
        for field in ["kernel", "reference"] {
            let mut json: serde_json::Value = serde_json::from_str(REQUEST).unwrap();
            json["bindings"][0][field] = value.clone().into();
            assert_eq!(
                decode(&json.to_string()).unwrap_err().to_string(),
                "invalid reference enrollment selector"
            );
        }
    }
    let mut json: serde_json::Value = serde_json::from_str(REQUEST).unwrap();
    for field in ["kernel", "reference"] {
        json["bindings"][0][field] = "\u{e9}"
            .repeat(MAX_REFERENCE_ENROLLMENT_SELECTOR_BYTES_V1 / 2)
            .into();
    }
    assert!(decode(&json.to_string()).is_ok());
    assert_eq!(
        decode(r#"{"version":1,"bindings":[{"kernel":"a\u0062","reference":"r"}]}"#)
            .unwrap()
            .bindings()[0]
            .kernel(),
        "ab"
    );
}

#[test]
fn encoded_byte_bound_precedes_roster_and_schema_validation() {
    assert_eq!(MAX_REFERENCE_ENROLLMENT_BYTES_V1, 4096);
    assert_eq!(MAX_REFERENCE_ENROLLMENT_BINDINGS_V1, 256);
    assert_eq!(MAX_REFERENCE_ENROLLMENT_SELECTOR_BYTES_V1, 1024);
    let padded = format!(
        "{REQUEST}{}",
        " ".repeat(MAX_REFERENCE_ENROLLMENT_BYTES_V1 - REQUEST.len())
    );
    assert!(decode(&padded).is_ok());
    assert_eq!(
        decode(&(padded + " ")).unwrap_err().to_string(),
        "reference enrollment request exceeds byte limit"
    );
    let rows = |count| {
        serde_json::json!({"version":1,"bindings":
            (0..count).map(|i| serde_json::json!({"kernel":format!("k{i:04}"),"reference":"r"}))
                .collect::<Vec<_>>()
        })
        .to_string()
    };
    assert!(decode(&rows(64)).is_ok());
    assert_eq!(
        decode(&rows(MAX_REFERENCE_ENROLLMENT_BINDINGS_V1 + 1))
            .unwrap_err()
            .to_string(),
        "reference enrollment request exceeds byte limit"
    );
    let mut charges = Vec::new();
    let error = ReferenceEnrollmentRequestV1::decode(
        &" ".repeat(MAX_REFERENCE_ENROLLMENT_BYTES_V1 + 1),
        |amount| {
            charges.push(amount);
            Ok::<_, Infallible>(())
        },
    )
    .unwrap_err();
    assert_eq!(charges, [1]);
    assert_eq!(
        error.to_string(),
        "reference enrollment request exceeds byte limit"
    );
}

#[test]
fn decoder_prepays_the_exact_quote_before_schema_validation() {
    for text in [REQUEST, "not-json"] {
        let mut charges = Vec::new();
        let result = ReferenceEnrollmentRequestV1::decode(text, |amount| {
            charges.push(amount);
            Ok::<_, Infallible>(())
        });
        assert_eq!(
            charges,
            [
                1,
                text.len() * 64 + std::mem::size_of::<ReferenceEnrollmentRequestV1>()
            ]
        );
        assert_eq!(result.is_ok(), text == REQUEST);
    }
}

#[test]
fn environment_scan_stops_at_the_request_and_keeps_charges_cumulative() {
    let captured = environment(&[
        ("AAA", "ignored"),
        (REFERENCE_ENROLLMENT_ENV_V1, REQUEST),
        ("ZZZ", "not-json"),
    ]);
    let mut total = 17;
    let mut charges = Vec::new();
    for _ in 0..2 {
        let request = ReferenceEnrollmentRequestV1::from_environment(&captured, |amount| {
            total += amount;
            charges.push(amount);
            Ok::<_, Infallible>(())
        })
        .unwrap()
        .unwrap();
        assert_eq!(request, decode(REQUEST).unwrap());
    }
    let once = [
        4,
        REFERENCE_ENROLLMENT_ENV_V1.len() + 1,
        1,
        REQUEST.len() * 64 + std::mem::size_of::<ReferenceEnrollmentRequestV1>(),
    ];
    assert_eq!(charges, once.repeat(2));
    assert_eq!(total, 17 + 2 * once.iter().sum::<usize>());
}

#[test]
fn absence_charges_only_captured_keys_without_decoding_other_values() {
    let captured = environment(&[("AAA", "not-json"), ("ZZZ", REQUEST)]);
    let mut charges = Vec::new();
    let result = ReferenceEnrollmentRequestV1::from_environment(&captured, |amount| {
        charges.push(amount);
        Ok::<_, Infallible>(())
    })
    .unwrap();
    assert!(result.is_none());
    assert_eq!(charges, [4, 4]);
    let mut called = false;
    assert!(
        ReferenceEnrollmentRequestV1::from_environment(&environment(&[]), |_| {
            called = true;
            Ok::<_, Infallible>(())
        })
        .unwrap()
        .is_none()
    );
    assert!(!called);
}

#[derive(Debug, Eq, PartialEq)]
struct Refused(usize);

#[test]
fn charge_refusals_are_returned_unchanged_before_decoding() {
    for stop in [1, 2] {
        let mut calls = 0;
        let error = ReferenceEnrollmentRequestV1::decode("not-json", |_| {
            calls += 1;
            if calls == stop {
                Err(Refused(stop))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(calls, stop);
        assert_eq!(error, ReferenceEnrollmentDecodeErrorV1::Work(Refused(stop)));
    }
    let captured = environment(&[(REFERENCE_ENROLLMENT_ENV_V1, "not-json")]);
    for stop in [1, 2, 3] {
        let mut calls = 0;
        let error = ReferenceEnrollmentRequestV1::from_environment(&captured, |_| {
            calls += 1;
            if calls == stop {
                Err(Refused(stop))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(calls, stop);
        assert_eq!(error, ReferenceEnrollmentDecodeErrorV1::Work(Refused(stop)));
    }
}
