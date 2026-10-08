use super::*;

fn args(values: &[&str]) -> Result<Case, String> {
    parse(values.iter().map(OsString::from))
}
fn valid() -> [&'static str; 5] {
    [
        "--native-v5-data-copy",
        "--producer-source",
        "src/lib.rs",
        "--device",
        "0x000000000000abcd",
    ]
}
fn outputs() -> (Vec<u32>, Vec<u8>) {
    let producer: Vec<_> = (0..ELEMENTS as u32).collect();
    let bytes = producer.iter().flat_map(|v| v.to_le_bytes()).collect();
    (producer, bytes)
}

#[test]
fn explicit_mode_source_and_single_canonical_device_are_required() {
    let valid = valid();
    assert_eq!(
        args(&valid).unwrap(),
        Case {
            producer_source: "src/lib.rs".into(),
            device: 0xabcd
        }
    );
    for n in 0..valid.len() {
        assert!(args(&valid[..n]).is_err());
    }
    for mode in [
        "--native-v5",
        "--native-v5-shards",
        "",
        "--receipt-coexistence",
    ] {
        let mut bad = valid;
        bad[0] = mode;
        assert!(args(&bad).is_err());
    }
    for source in ["", "a\0b", "a\nb", "a\rb"] {
        let mut bad = valid;
        bad[2] = source;
        assert!(args(&bad).is_err());
    }
    let long = "x".repeat(4097);
    let mut bad = valid;
    bad[2] = &long;
    assert!(args(&bad).is_err());
    let mut extra = valid.to_vec();
    extra.push("fallback");
    assert!(args(&extra).is_err());
}

#[test]
fn zero_foreign_spelling_and_multiple_device_arguments_are_refused() {
    for device in [
        "0x0000000000000000",
        "0xabcd",
        "0x000000000000ABCD",
        "0X000000000000abcd",
        "000000000000abcd",
        "0x000000000000abcd,0x000000000000abce",
    ] {
        let mut bad = valid();
        bad[4] = device;
        assert!(args(&bad).is_err());
    }
    let mut bad = valid();
    bad[3] = "--devices";
    assert!(args(&bad).is_err());
}

#[test]
fn exact_independent_values_and_both_extents_are_required() {
    let (producer, destination) = outputs();
    let before = Version {
        attempt: 0,
        lineage: 0,
    };
    let after = Version {
        attempt: 1,
        lineage: 1,
    };
    verify(&producer, &destination, before, after).unwrap();
    for member in 0..ELEMENTS {
        let mut wrong = producer.clone();
        wrong[member] = u32::MAX;
        assert!(verify(&wrong, &destination, before, after).is_err());
        let mut wrong = destination.clone();
        wrong[member * 4] ^= 1;
        assert!(verify(&producer, &wrong, before, after).is_err());
    }
    assert!(verify(&producer[..ELEMENTS - 1], &destination, before, after).is_err());
    assert!(verify(&producer, &destination[..BYTES - 1], before, after).is_err());
    let mut extra = producer.clone();
    extra.push(0);
    assert!(verify(&extra, &destination, before, after).is_err());
    let mut extra = destination.clone();
    extra.push(0);
    assert!(verify(&producer, &extra, before, after).is_err());
}

#[test]
fn paired_wrong_outputs_do_not_satisfy_an_independent_oracle() {
    assert!(
        verify(
            &[0; ELEMENTS],
            &[0; BYTES],
            Version {
                attempt: 0,
                lineage: 0
            },
            Version {
                attempt: 1,
                lineage: 1
            }
        )
        .is_err()
    );
}

#[test]
fn exactly_one_attempt_and_success_lineage_not_lineage_increment_are_required() {
    let (producer, destination) = outputs();
    let before = Version {
        attempt: 9,
        lineage: 3,
    };
    let correct = Version {
        attempt: 10,
        lineage: 10,
    };
    verify(&producer, &destination, before, correct).unwrap();
    for after in [
        before,
        Version {
            attempt: 10,
            lineage: 3,
        },
        Version {
            attempt: 10,
            lineage: 4,
        },
        Version {
            attempt: 11,
            lineage: 11,
        },
        Version {
            attempt: 8,
            lineage: 8,
        },
    ] {
        assert!(verify(&producer, &destination, before, after).is_err());
    }
    assert!(
        verify(
            &producer,
            &destination,
            Version {
                attempt: 0,
                lineage: 1
            },
            correct
        )
        .is_err()
    );
    assert!(
        verify(
            &producer,
            &destination,
            Version {
                attempt: u64::MAX,
                lineage: 0
            },
            Version {
                attempt: 0,
                lineage: 0
            }
        )
        .is_err()
    );
}

#[test]
fn report_is_closed_scalar_metadata_not_native_overlap_or_milestone_evidence() {
    let (producer, destination) = outputs();
    let report = verify(
        &producer,
        &destination,
        Version {
            attempt: 0,
            lineage: 0,
        },
        Version {
            attempt: 1,
            lineage: 1,
        },
    )
    .unwrap();
    assert!(report.fields(0).is_err());
    let fields = report.fields(0xabcd).unwrap();
    assert!(fields.contains("\"destination_bytes\":148"));
    assert!(fields.contains("\"destination_attempt_after\":1"));
    assert!(fields.contains("\"destination_lineage_after\":1"));
    assert!(!fields.contains("physical_overlap_measured"));
    assert!(!fields.contains("qualified"));
    assert!(!fields.contains("milestone"));
}
