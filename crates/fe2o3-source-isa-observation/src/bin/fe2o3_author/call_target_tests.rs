//! Closed additive CLI parsing only; no bundle imports or subprocess execution.
use super::*;

fn selected() -> AuthoringRegionSelectorV1 {
    AuthoringRegionSelectorV1 {
        bundle_identity: "11".repeat(32),
        canonical_kir_digest: "22".repeat(32),
        target: "gfx942:xnack-".into(),
        operations: vec![
            fe2o3_source_isa_observation::multilevel_authoring_v1::AuthoringOperationCoordinateV1 {
                function: 0,
                block: 1,
                operation: 2,
            },
        ],
    }
}

fn arguments() -> Vec<String> {
    vec![
        "call-target".into(),
        "--selector".into(),
        serde_json::to_string(&selected()).unwrap(),
    ]
}

#[test]
fn call_target_command_is_additive_and_preserves_legacy_dispatch() {
    let query = parse(&arguments()).unwrap();
    let Query::CallTarget(parsed) = query else {
        panic!("exact additive command required");
    };
    assert_eq!(parsed, selected());
    let mut old = arguments();
    old[0] = "select".into();
    assert!(matches!(parse(&old), Ok(Query::Select(_))));
    assert!(matches!(parse(&["inspect".into()]), Ok(Query::Inspect)));
    old[0] = "materialize".into();
    old.extend(["--helper".into(), "compute".into()]);
    assert!(matches!(parse(&old), Ok(Query::Materialize(_, _))));
    old[0] = "materialize-const-u32".into();
    assert!(matches!(parse(&old), Ok(Query::MaterializeConstU32(_, _))));
}

#[test]
fn call_target_command_rejects_caller_target_overrides_and_extra_flags() {
    for extra in [
        vec!["--callee", "opaque-right"],
        vec!["--function", "2"],
        vec!["--physical-abi", "v0"],
        vec!["--resume"],
        vec!["--selector", "{}"],
    ] {
        let mut bad = arguments();
        bad.extend(extra.into_iter().map(str::to_owned));
        assert!(parse(&bad).is_err());
    }
    let mut bad = arguments();
    bad.swap(1, 2);
    assert!(parse(&bad).is_err());
    assert!(parse(&["call-target".into()]).is_err());
}

#[test]
fn call_target_command_rejects_malformed_unknown_and_oversized_selector_json() {
    for encoded in ["", "{", "null", "[]", "{}"] {
        let mut bad = arguments();
        bad[2] = encoded.into();
        assert!(parse(&bad).is_err());
    }
    for nested in [false, true] {
        let mut value = serde_json::to_value(selected()).unwrap();
        if nested {
            value["operations"][0]["callee"] = "opaque-left".into();
        } else {
            value["callee"] = "opaque-left".into();
        }
        let mut bad = arguments();
        bad[2] = value.to_string();
        assert!(parse(&bad).is_err());
    }
    let mut value = serde_json::to_value(selected()).unwrap();
    value["operations"][0]["function"] = (-1).into();
    let mut bad = arguments();
    bad[2] = value.to_string();
    assert!(parse(&bad).is_err());
    bad[2] = " ".repeat(MAX_SELECTOR_BYTES + 1);
    assert!(parse(&bad).is_err());
}
