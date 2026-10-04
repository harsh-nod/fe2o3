//! Inert transport fixtures only. These never enter a graph/source/proof API.
use super::results::*;

fn child(target: &str, mode: &str) -> Child {
    let agreement = matches!(mode, "f" | "late-agreement" | "late-replay");
    Child {
        schema: CHILD_SCHEMA.into(),
        target: target.into(),
        mode: mode.into(),
        callbacks: 1,
        agreements: u64::from(agreement),
        replay_completed: u64::from(matches!(mode, "f" | "late-replay")),
        installed: u64::from(mode == "f"),
        actual_f_identity: agreement.then(|| Identity {
            sha256: "1".repeat(64),
            canonical_length: 42,
        }),
        terminal: match mode {
            "work" | "storage" => "resource-refusal",
            "late-agreement" => "injected-after-agreement",
            "late-replay" => "injected-after-replay",
            _ => "conditional-finalizer-required",
        }
        .into(),
        refusal: "FE2O3-COND-FINALIZER-001: inert transport fixture, not proof".into(),
        resource: ["work", "storage"].contains(&mode).then(|| mode.into()),
        fault_observed: mode.starts_with("late-"),
        work: u64::from(mode != "work"),
        floor_before: 19,
        floor_after: 19,
        account_preserved: true,
        source_to_final_output_checked: agreement,
        qualification_credit: false,
        grants_artifact_or_launch_authority: false,
        native_output_emitted: false,
    }
}

fn matrix(suite: &str) -> Matrix {
    let modes = if suite == "main" {
        &MAIN_MODES[..]
    } else {
        &LATE_MODES[..]
    };
    Matrix {
        schema: SCHEMA.into(),
        suite: suite.into(),
        preparation_sha256: "1".repeat(64),
        rows: ["gfx942", "gfx950"]
            .into_iter()
            .flat_map(|target| {
                modes.iter().map(move |mode| Row {
                    target: target.into(),
                    mode: (*mode).into(),
                    captured_args_sha256: "2".repeat(64),
                    replay_args_sha256: "3".repeat(64),
                    child: child(target, mode),
                    exit_code: 0,
                    stdout_sha256: "4".repeat(64),
                    stderr_sha256: "5".repeat(64),
                })
            })
            .collect(),
        source_unchanged: true,
        inputs_unchanged: true,
        tools_unchanged: true,
        qualification_credit: false,
        grants_artifact_or_launch_authority: false,
        native_output_emitted: false,
    }
}

#[test]
fn final_child_protocol_distinguishes_cpu_labels_from_full_target_ids() {
    for profile in [super::Profile::Gfx942, super::Profile::Gfx950] {
        for mode in MAIN_MODES.into_iter().chain(LATE_MODES) {
            let good = child(profile.cpu(), mode);
            good.check(profile.cpu(), mode).unwrap();
            assert_eq!(
                decode_child(&serde_json::to_vec(&good).unwrap()).unwrap(),
                good
            );
            assert_ne!(profile.cpu(), profile.device_target());
            let full_id = child(profile.device_target(), mode);
            assert!(full_id.check(profile.device_target(), mode).is_err());
            assert!(decode_child(&serde_json::to_vec(&full_id).unwrap()).is_err());
            assert!(good.check(profile.device_target(), mode).is_err());
        }
    }
}

#[test]
fn final_results_require_complete_ordered_main_and_late_rosters() {
    for suite in ["main", "late"] {
        let good = matrix(suite);
        good.check().unwrap();
        for index in 0..good.rows.len() {
            let mut bad = good.clone();
            bad.rows.remove(index);
            assert!(bad.check().is_err());
            let mut bad = good.clone();
            bad.rows[index].exit_code = 1;
            assert!(bad.check().is_err());
            let mut bad = good.clone();
            bad.rows[index].target = "gfx000".into();
            assert!(bad.check().is_err());
            let mut bad = good.clone();
            bad.rows[index].child.callbacks = 0;
            assert!(bad.check().is_err());
            let mut bad = good.clone();
            bad.rows[index].captured_args_sha256 = bad.rows[index].replay_args_sha256.clone();
            assert!(bad.check().is_err());
        }
        let mut bad = good.clone();
        bad.rows.swap(0, 1);
        assert!(bad.check().is_err());
        let mut bad = good.clone();
        bad.rows.push(good.rows[0].clone());
        assert!(bad.check().is_err());
        for key in [
            "source_unchanged",
            "inputs_unchanged",
            "tools_unchanged",
            "qualification_credit",
            "grants_artifact_or_launch_authority",
            "native_output_emitted",
        ] {
            let mut value = serde_json::to_value(&good).unwrap();
            value[key] = serde_json::Value::Bool(!value[key].as_bool().unwrap());
            assert!(
                serde_json::from_value::<Matrix>(value)
                    .unwrap()
                    .check()
                    .is_err()
            );
        }
    }
}

#[test]
fn final_child_protocol_rejects_omissions_wrong_types_and_partial_progress() {
    let good = child("gfx942", "f");
    let value = serde_json::to_value(&good).unwrap();
    decode_child(&serde_json::to_vec(&value).unwrap()).unwrap();
    for key in value.as_object().unwrap().keys() {
        let mut bad = value.clone();
        bad.as_object_mut().unwrap().remove(key);
        assert!(
            decode_child(&serde_json::to_vec(&bad).unwrap()).is_err(),
            "omitted {key}"
        );
    }
    for key in [
        "callbacks",
        "agreements",
        "replay_completed",
        "installed",
        "work",
        "floor_before",
        "floor_after",
    ] {
        for changed in [
            serde_json::json!(1.0),
            serde_json::json!(true),
            serde_json::json!("1"),
            serde_json::Value::Null,
        ] {
            let mut bad = value.clone();
            bad[key] = changed;
            assert!(
                decode_child(&serde_json::to_vec(&bad).unwrap()).is_err(),
                "wrong integer type {key}"
            );
        }
    }
    for key in [
        "account_preserved",
        "source_to_final_output_checked",
        "qualification_credit",
        "grants_artifact_or_launch_authority",
        "native_output_emitted",
        "fault_observed",
    ] {
        for changed in [
            serde_json::json!(1),
            serde_json::json!("false"),
            serde_json::Value::Null,
        ] {
            let mut bad = value.clone();
            bad[key] = changed;
            assert!(decode_child(&serde_json::to_vec(&bad).unwrap()).is_err());
        }
    }
    for key in ["callbacks", "agreements", "replay_completed", "installed"] {
        let mut bad = value.clone();
        bad[key] = serde_json::json!(0);
        assert!(decode_child(&serde_json::to_vec(&bad).unwrap()).is_err());
    }
    for replacement in [
        serde_json::Value::Null,
        serde_json::json!({"sha256":"0".repeat(64),"canonical_length":42}),
        serde_json::json!({"sha256":"1".repeat(64),"canonical_length":0}),
    ] {
        let mut bad = value.clone();
        bad["actual_f_identity"] = replacement;
        assert!(decode_child(&serde_json::to_vec(&bad).unwrap()).is_err());
    }
    let mut bad = value;
    bad["proof_bypass"] = serde_json::json!(false);
    assert!(decode_child(&serde_json::to_vec(&bad).unwrap()).is_err());
}

#[test]
fn final_child_protocol_distinguishes_fixed6_resource_and_late_refusals() {
    for mode in MAIN_MODES.into_iter().chain(LATE_MODES) {
        let good = child("gfx950", mode);
        good.check("gfx950", mode).unwrap();
        assert!(good.check("gfx942", mode).is_err());
        let mut bad = good.clone();
        bad.qualification_credit = true;
        assert!(bad.check("gfx950", mode).is_err());
        let mut bad = good.clone();
        bad.floor_after = 20;
        assert!(bad.check("gfx950", mode).is_err());
        let mut bad = good.clone();
        bad.native_output_emitted = true;
        assert!(bad.check("gfx950", mode).is_err());
        if mode != "f" {
            let mut bad = good.clone();
            bad.installed = 1;
            assert!(bad.check("gfx950", mode).is_err());
        }
        if mode.starts_with("late-") {
            let mut bad = good.clone();
            bad.fault_observed = false;
            assert!(bad.check("gfx950", mode).is_err());
            let mut bad = good.clone();
            bad.agreements = 0;
            assert!(bad.check("gfx950", mode).is_err());
        }
    }
}
