//! Closed protocol controls are not successful source-execution evidence.
use super::*;

fn unqualified_observation(case: Case) -> Observation {
    serde_json::from_value(serde_json::json!({
        "case": case,
        "source_digest": ([1u8; 32]), "original_digest": ([2u8; 32]), "original_bytes": 123,
        "component_digest": ([3u8; 32]), "component_bytes": 124, "baseline_i_digest": ([4u8; 32]),
        "original_order": ROOTS, "component_order": ROOTS,
        "proved_pairs": 3, "changed": true, "component_replay_work": 1, "baseline_replay_work": 1,
        "source_roots": [], "roots": [],
        "simulation": { "case": case, "identity": ([3u8; 32]), "canonical_bytes": 124, "scenarios": [] },
        "baseline_llvm_sha256": ([5u8; 32]), "baseline_llvm_bytes": 100
    })).unwrap()
}

#[test]
fn commutative_protocol_requires_exact_family_case_mode_and_successful_child() {
    let all = cases();
    assert_eq!(all.len(), 16);
    assert_eq!(
        all.iter()
            .map(|c| name(*c))
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        16
    );
    let case = all[0];
    let request = Request {
        case: FixtureCase::CommutativeCse(case),
        mode: Mode::Observe,
        args_sha256: [6; 32],
        source: vec![],
    };
    let encode = |request: Request| {
        serde_json::to_vec(&Report {
            request,
            result: Ok(Outcome::ObservedCommutative(Box::new(
                unqualified_observation(case),
            ))),
        })
        .unwrap()
    };
    let bytes = encode(request.clone());
    let Outcome::ObservedCommutative(report) =
        decode_report(Some(0), Some(&bytes), &request).unwrap()
    else {
        panic!("new protocol family")
    };
    // A structurally valid subprocess receipt is not enough to qualify source.
    assert!(validate(case, &report).is_err());
    for status in [None, Some(1), Some(101)] {
        assert!(decode_report(status, Some(&bytes), &request).is_err());
    }
    assert!(decode_report(Some(0), None, &request).is_err());
    assert!(decode_report(Some(0), Some(b"{}"), &request).is_err());
    for changed in [
        FixtureCase::DominanceCse(case),
        FixtureCase::CommutativeCse(Case {
            integer: Integer::U64,
            ..case
        }),
        FixtureCase::CommutativeCse(Case {
            target: Target::Gfx950,
            ..case
        }),
    ] {
        let mut substituted = request.clone();
        substituted.case = changed;
        assert!(decode_report(Some(0), Some(&encode(substituted)), &request).is_err());
    }
    let mut altered = request.clone();
    altered.args_sha256[0] ^= 1;
    assert!(decode_report(Some(0), Some(&encode(altered)), &request).is_err());
    let mut wrong_mode = request.clone();
    wrong_mode.mode = Mode::Extract;
    assert!(decode_report(Some(0), Some(&encode(wrong_mode.clone())), &wrong_mode).is_err());
    let refusal = serde_json::to_vec(&Report {
        request: request.clone(),
        result: Err("actual source refusal".into()),
    })
    .unwrap();
    assert!(decode_report(Some(0), Some(&refusal), &request).is_err());
}

fn wiring(lib: &str, source: &str, manifest: &str) -> bool {
    lib.contains("#[cfg(feature = \"commutative-cse\")]\nmod commutative_cse;")
        && lib.contains("    feature = \"commutative-cse\",")
        && source.matches("let quotient = choose / choose;").count() == 3
        && source.contains("*element = rhs & lhs;")
        && source.contains("*element = rhs | lhs;")
        && source.contains("*element = rhs ^ lhs;")
        && Integer::ALL.into_iter().all(|i| {
            manifest.contains(&format!(
                "commutative-cse-{} = [\"commutative-cse\"]",
                i.name()
            )) && source.contains(&format!(
                "#[cfg(feature = \"commutative-cse-{}\")]\ndefine!({});",
                i.name(),
                i.name()
            ))
        })
}

#[test]
fn commutative_fixture_wiring_preserves_separate_exact_cse_and_guarded_division() {
    let lib = include_str!("../tests/fixtures/production-extraction-device/src/lib.rs");
    let source =
        include_str!("../tests/fixtures/production-extraction-device/src/commutative_cse.rs");
    let exact = include_str!("../tests/fixtures/production-extraction-device/src/dominance_cse.rs");
    let manifest = include_str!("../tests/fixtures/production-extraction-device/Cargo.toml");
    assert!(wiring(lib, source, manifest));
    for changed in [
        lib.replace("mod commutative_cse;", ""),
        lib.replace("    feature = \"commutative-cse\",", ""),
    ] {
        assert!(!wiring(&changed, source, manifest));
    }
    assert!(!wiring(
        lib,
        &source.replace("choose / choose", "1"),
        manifest
    ));
    assert!(!wiring(
        lib,
        &source.replace("rhs & lhs", "lhs & rhs"),
        manifest
    ));
    assert!(!wiring(
        lib,
        source,
        &manifest.replace("commutative-cse-u64 = [\"commutative-cse\"]", "")
    ));
    assert!(!wiring(lib, &source.replace("define!(u64);", ""), manifest));
    assert!(exact.contains("*element = lhs & rhs;"));
    assert!(!exact.contains("choose / choose"));
    assert!(lib.contains("mod dominance_cse;"));
    assert!(manifest.contains("dominance-cse-u64 = [\"dominance-cse\"]"));
}
