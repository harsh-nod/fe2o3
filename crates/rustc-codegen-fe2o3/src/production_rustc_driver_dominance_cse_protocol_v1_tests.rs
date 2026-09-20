//! Closed request and fixture-wiring controls, not source execution evidence.
use super::*;

#[test]
fn dominance_case_roster_and_protocol_bind_exact_type_target_args_and_mode() {
    let cases = cases();
    assert_eq!(cases.len(), 16);
    assert_eq!(
        cases
            .iter()
            .map(|c| c.name())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        16
    );
    let case = cases[0];
    let request = Request {
        case: FixtureCase::DominanceCse(case),
        mode: Mode::Extract,
        args_sha256: [1; 32],
        source: vec![],
    };
    let encode = |request: Request| {
        serde_json::to_vec(&Report {
            request,
            result: Ok(Outcome::Extracted {
                llvm_sha256: [2; 32],
                llvm_bytes: 3,
                source_roots: vec![],
            }),
        })
        .unwrap()
    };
    let bytes = encode(request.clone());
    assert!(matches!(
        decode_report(Some(0), Some(&bytes), &request),
        Ok(Outcome::Extracted { .. })
    ));
    for status in [None, Some(1), Some(101)] {
        assert!(decode_report(status, Some(&bytes), &request).is_err());
    }
    assert!(decode_report(Some(0), None, &request).is_err());
    assert!(decode_report(Some(0), Some(b"{}"), &request).is_err());
    for changed in [
        Case {
            integer: Integer::U64,
            ..case
        },
        Case {
            target: Target::Gfx950,
            ..case
        },
    ] {
        let mut bad = request.clone();
        bad.case = FixtureCase::DominanceCse(changed);
        assert!(decode_report(Some(0), Some(&encode(bad)), &request).is_err());
    }
    let mut changed = request.clone();
    changed.args_sha256[0] ^= 1;
    assert!(decode_report(Some(0), Some(&encode(changed)), &request).is_err());
    let mut observed = request.clone();
    observed.mode = Mode::Observe;
    assert!(decode_report(Some(0), Some(&encode(observed.clone())), &observed).is_err());
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["request"].as_object_mut().unwrap().remove("case");
    assert!(
        decode_report(
            Some(0),
            Some(&serde_json::to_vec(&value).unwrap()),
            &request
        )
        .is_err()
    );
}

fn wiring(lib: &str, fixture: &str, manifest: &str) -> bool {
    lib.contains("#[cfg(feature = \"dominance-cse\")]\nmod dominance_cse;")
        && lib.contains("    feature = \"dominance-cse\",")
        && Integer::ALL.into_iter().all(|i| {
            manifest.contains(&format!("dominance-cse-{} = [\"dominance-cse\"]", i.name()))
                && fixture.contains(&format!(
                    "#[cfg(feature = \"dominance-cse-{}\")]\ndefine!({});",
                    i.name(),
                    i.name()
                ))
        })
}
#[test]
fn dominance_fixture_features_module_and_default_exclusion_stay_wired() {
    let lib = include_str!("../tests/fixtures/production-extraction-device/src/lib.rs");
    let fixture =
        include_str!("../tests/fixtures/production-extraction-device/src/dominance_cse.rs");
    let manifest = include_str!("../tests/fixtures/production-extraction-device/Cargo.toml");
    assert!(wiring(lib, fixture, manifest));
    assert!(!wiring(
        &lib.replace("mod dominance_cse;", ""),
        fixture,
        manifest
    ));
    assert!(!wiring(
        &lib.replace("    feature = \"dominance-cse\",", ""),
        fixture,
        manifest
    ));
    assert!(!wiring(
        lib,
        fixture,
        &manifest.replace("dominance-cse-u64 = [\"dominance-cse\"]", "")
    ));
    assert!(!wiring(
        lib,
        &fixture.replace("define!(u64);", ""),
        manifest
    ));
}
