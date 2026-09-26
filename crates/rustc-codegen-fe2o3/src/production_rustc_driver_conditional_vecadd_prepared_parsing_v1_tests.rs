use super::*;

#[allow(dead_code)]
#[path = "../../fe2o3-kernel-descriptor/tests/support/conditional_invocation_v2.rs"]
mod report_contract;

// These tests use synthetic parser inputs only. They claim no actual compiler,
// source proof, protected execution, qualification, or GPU observation.
fn parser_args(target: &str) -> Vec<String> {
    [
        "/tool/rustc",
        "--crate-name",
        "fe2o3_production_extraction_fixture",
        "src/lib.rs",
        "--crate-type",
        "lib",
        "--target",
        "amdgcn-amd-amdhsa",
        "--emit=dep-info,metadata",
        "-C",
        &format!("target-cpu={target}"),
        "-Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32",
        "-Coverflow-checks=on",
        "--cfg",
        "feature=\"conditional-vecadd\"",
        "-Cmetadata=1234",
        "--out-dir",
        "/old",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn parser_record() -> serde_json::Value {
    let source: Vec<_> = (0..9)
        .map(|n| (format!("synthetic-{n}"), [0u8; 32]))
        .collect();
    let stamp = || FileStamp {
        path: "/synthetic".into(),
        bytes: 0,
        sha256: "0".repeat(64),
    };
    json(&Preparation {
        schema: SCHEMA.into(),
        workspace: "/synthetic".into(),
        source: source.clone(),
        executable: stamp(),
        cargo: stamp(),
        rustc: stamp(),
        toolchain_manifest: stamp(),
        sysroot: "/synthetic".into(),
        replay_support_files: REPLAY_SUPPORT_PATHS
            .into_iter()
            .map(|path| FileStamp {
                path: path.into(),
                ..stamp()
            })
            .collect(),
        invocations: TARGETS
            .into_iter()
            .map(|target| Invocation {
                target: target.into(),
                fixture: fixture(Case::Annotated, target),
                cwd: "/synthetic".into(),
                args_sha256: [0; 32],
                environment_sha256: [0; 32],
                files: vec![],
                request: Request {
                    case: Case::Annotated,
                    stage: Stage::Consuming,
                    source: source.clone(),
                    args_sha256: [0; 32],
                },
            })
            .collect(),
        proof_executed: false,
        qualification_credit: false,
        grants_artifact_or_launch_authority: false,
        hardware_observed: false,
    })
    .unwrap()
}

#[test]
fn prepared_vecadd_support_roster_rejects_missing_extra_duplicate_reordered_and_alias_paths() {
    assert!(
        REPLAY_SUPPORT_PATHS
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    );
    let original = parser_record();
    let parse = |value: &serde_json::Value| parse_preparation(&serde_json::to_vec(value).unwrap());
    assert!(parse(&original).is_ok());
    for case in 0..7 {
        let mut changed = original.clone();
        let rows = changed["replay_support_files"].as_array_mut().unwrap();
        match case {
            0 => {
                rows.pop();
            }
            1 => {
                rows.push(rows[0].clone());
            }
            2 => {
                rows[1] = rows[0].clone();
            }
            3 => {
                rows.swap(0, 1);
            }
            4 => {
                rows[0]["path"] = format!("/sysroot/{}", REPLAY_SUPPORT_PATHS[0]).into();
            }
            5 => {
                rows[0]["path"] = format!("./{}", REPLAY_SUPPORT_PATHS[0]).into();
            }
            _ => {
                rows[0]["path"] = "lib/foreign.so".into();
            }
        }
        assert!(parse(&changed).is_err(), "hostile support roster {case}");
    }
}

#[test]
fn prepared_vecadd_support_schema_rejects_the_old_full_sysroot_inventory() {
    let mut record = parser_record();
    let old = record
        .as_object_mut()
        .unwrap()
        .remove("replay_support_files")
        .unwrap();
    record["sysroot_files"] = old;
    assert!(parse_preparation(&serde_json::to_vec(&record).unwrap()).is_err());
}

// Minimal diagnostic JSON and an inert caller-authored contract, not retained
// proof custody. Full 54-row validation remains in the opt-in parent.
fn formula_v2_report_pair(cpu: [u8; 32]) -> (serde_json::Value, serde_json::Value) {
    use fe2o3_kernel_descriptor::{
        encode_conditional_invocation_contract_v2, encoded_conditional_invocation_contract_v2_len,
    };
    let fixture = report_contract::Fixture::new(2, 2);
    let input = report_contract::input(&fixture, cpu);
    let mut bytes =
        vec![
            0;
            encoded_conditional_invocation_contract_v2_len(&input, &mut report_contract::free)
                .unwrap()
        ];
    encode_conditional_invocation_contract_v2(&input, &mut bytes, &mut report_contract::free)
        .unwrap();
    let sidecar = serde_json::json!({
        "statement": input.theorem.statement_identity,
        "semantic_root": 7,
        "cpu_input_commitment": cpu,
    });
    let detail = serde_json::json!({
        "conditional_formula": {"statement": input.theorem.statement_identity},
        "retained_proof_events": {"events": [
            {"Retained": {"root": 7}},
            {"ReplayAccepted": {"root": 7, "contract": bytes}},
        ]},
    });
    (sidecar, detail)
}

#[test]
fn prepared_vecadd_formula_v2_report_join_accepts_matching_content() {
    for cpu in [[17; 32], [23; 32]] {
        let (sidecar, detail) = formula_v2_report_pair(cpu);
        check_formula_v2_report_join(&sidecar, &detail).unwrap();
    }
}

#[test]
fn prepared_vecadd_formula_v2_report_join_rejects_identity_substitution() {
    let (sidecar, detail) = formula_v2_report_pair([17; 32]);
    for field in ["cpu_input_commitment", "statement", "semantic_root"] {
        let mut changed = sidecar.clone();
        changed[field] = if field == "semantic_root" {
            8.into()
        } else {
            json(&[23u8; 32]).unwrap()
        };
        assert!(
            check_formula_v2_report_join(&changed, &detail).is_err(),
            "{field}"
        );
        changed.as_object_mut().unwrap().remove(field);
        assert!(
            check_formula_v2_report_join(&changed, &detail).is_err(),
            "missing {field}"
        );
    }
    for pointer in [
        "/conditional_formula/statement",
        "/retained_proof_events/events/0/Retained/root",
        "/retained_proof_events/events/1/ReplayAccepted/root",
    ] {
        let mut changed = detail.clone();
        *changed.pointer_mut(pointer).unwrap() = if pointer.ends_with("root") {
            8.into()
        } else {
            json(&[23u8; 32]).unwrap()
        };
        assert!(
            check_formula_v2_report_join(&sidecar, &changed).is_err(),
            "{pointer}"
        );
    }
    let (foreign_sidecar, foreign_detail) = formula_v2_report_pair([23; 32]);
    let mut changed = detail.clone();
    changed["retained_proof_events"]["events"][1] =
        foreign_detail["retained_proof_events"]["events"][1].clone();
    assert!(check_formula_v2_report_join(&sidecar, &changed).is_err());
    // Even a matching CPU field cannot hide a foreign contract's statement.
    let mut changed_sidecar = sidecar;
    changed_sidecar["cpu_input_commitment"] = foreign_sidecar["cpu_input_commitment"].clone();
    assert!(check_formula_v2_report_join(&changed_sidecar, &changed).is_err());
}

#[test]
fn prepared_vecadd_formula_v2_report_join_requires_one_accepted_contract() {
    let (sidecar, detail) = formula_v2_report_pair([17; 32]);
    for case in 0..6 {
        let mut changed = detail.clone();
        let events = changed["retained_proof_events"]["events"]
            .as_array_mut()
            .unwrap();
        match case {
            0 => {
                events.pop();
            }
            1 => {
                events.push(events[1].clone());
            }
            2 => {
                events[1]["ReplayAccepted"]
                    .as_object_mut()
                    .unwrap()
                    .remove("contract");
            }
            3 => {
                events[1]["ReplayAccepted"] = serde_json::Value::Null;
            }
            4 => {
                events[1]["Retained"] = events[0]["Retained"].clone();
            }
            _ => {
                events.clear();
            }
        }
        assert!(
            check_formula_v2_report_join(&sidecar, &changed).is_err(),
            "case {case}"
        );
    }
}

#[test]
fn prepared_vecadd_formula_v2_report_join_strictly_decodes_contract() {
    let (sidecar, detail) = formula_v2_report_pair([17; 32]);
    let valid = detail["retained_proof_events"]["events"][1]["ReplayAccepted"]["contract"].clone();
    for case in 0..7 {
        let mut bytes: Vec<u8> = serde_json::from_value(valid.clone()).unwrap();
        match case {
            0 => {
                bytes[0] ^= 1;
            }
            1 => {
                bytes.truncate(8);
            }
            2 => {
                bytes.push(0);
            }
            3 => {
                bytes = report_contract::Fixture::new(2, 2).wire();
            }
            4 => {
                bytes = vec![0; fe2o3_kernel_descriptor::MAX_CONDITIONAL_INVOCATION_BYTES_V2 + 1];
            }
            _ => {}
        }
        let mut changed = detail.clone();
        changed["retained_proof_events"]["events"][1]["ReplayAccepted"]["contract"] = match case {
            5 => serde_json::json!([256]),
            6 => serde_json::json!("not contract bytes"),
            _ => serde_json::json!(bytes),
        };
        assert!(
            check_formula_v2_report_join(&sidecar, &changed).is_err(),
            "case {case}"
        );
    }
}

#[test]
fn prepared_vecadd_parsing_rejects_hostile_selection_and_authority() {
    let original = parser_record();
    let parse = |v: &serde_json::Value| parse_preparation(&serde_json::to_vec(v).unwrap());
    assert!(parse(&original).is_ok());
    for field in [
        "proof_executed",
        "qualification_credit",
        "grants_artifact_or_launch_authority",
        "hardware_observed",
    ] {
        let mut changed = original.clone();
        changed[field] = true.into();
        assert!(parse(&changed).is_err());
    }
    for pointer in [
        "/schema",
        "/invocations/0/target",
        "/invocations/0/request/case",
        "/invocations/0/request/stage",
    ] {
        let mut changed = original.clone();
        *changed.pointer_mut(pointer).unwrap() = "foreign".into();
        assert!(parse(&changed).is_err());
    }
    let mut changed = original.clone();
    changed["invocations"][1]["target"] = "gfx942".into();
    assert!(parse(&changed).is_err());
    changed = original.clone();
    changed["invocations"].as_array_mut().unwrap().pop();
    assert!(parse(&changed).is_err());
    for pointer in [
        "",
        "/invocations/0",
        "/invocations/0/fixture",
        "/invocations/0/fixture/compilerInput",
    ] {
        let mut changed = original.clone();
        changed.pointer_mut(pointer).unwrap()["unexpected"] = true.into();
        assert!(parse(&changed).is_err());
    }
    let mut changed = original;
    changed["invocations"][0]["request"]["args_sha256"][0] = 1.into();
    assert!(parse(&changed).is_err());
    assert!(parse_preparation(&vec![b' '; JSON_CAP + 1]).is_err());
}

#[test]
fn prepared_vecadd_exact_target_feature_and_output_normalization() {
    for target in TARGETS {
        let args = parser_args(target);
        exact_selection(&args, target).unwrap();
        assert!(
            exact_selection(
                &args,
                if target == "gfx942" {
                    "gfx950"
                } else {
                    "gfx942"
                }
            )
            .is_err()
        );
        for extra in [
            vec!["-Ctarget-cpu=gfx942"],
            vec!["--cfg", "feature=\"default\""],
            vec!["--cfg", "feature = \"conditional-vecadd-input-guard\""],
            vec!["--cfg", "feature=\"conditional-vecadd\""],
            vec!["--cfg", "foreign_cfg"],
            vec!["-Coverflow-checks=off"],
            vec!["@foreign"],
            vec!["--emit=link"],
            vec!["-Zcodegen-backend=/foreign"],
            vec!["-Cincremental=/foreign"],
            vec!["-L/foreign"],
        ] {
            let mut changed = args.clone();
            changed.extend(extra.into_iter().map(str::to_owned));
            assert!(exact_selection(&changed, target).is_err());
        }
        let normalized = normalized_arguments(
            &args,
            Path::new("/sysroot"),
            Path::new("/prepared/compiler-output"),
        )
        .unwrap();
        assert_eq!(
            option(&normalized, "--out-dir").unwrap(),
            ["/prepared/compiler-output"]
        );
        assert_eq!(option(&normalized, "--sysroot").unwrap(), ["/sysroot"]);
        assert_eq!(&normalized[..args.len() - 1], &args[..args.len() - 1]);
        let mut changed = args;
        changed.push("--out-dir=/foreign".into());
        assert!(normalized_arguments(&changed, Path::new("/sysroot"), Path::new("/out")).is_err());
    }
}

#[test]
fn prepared_vecadd_replay_redirects_only_outputs_and_rebinds_the_child_request() {
    let sysroot = Path::new("/sysroot");
    let input = Path::new("/prepared/compiler-output");
    let output = Path::new("/results/compiler-output");
    let args = normalized_arguments(&parser_args("gfx942"), sysroot, input).unwrap();
    let captured = serde_json::to_vec(&args).unwrap();
    let request = Request {
        case: Case::Annotated,
        stage: Stage::Consuming,
        source: vec![("synthetic".into(), [1; 32])],
        args_sha256: Sha256::digest(&captured).into(),
    };
    let (bytes, replay) = replay_arguments(&captured, &request, sysroot, output).unwrap();
    let actual: Vec<String> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        option(&actual, "--out-dir").unwrap(),
        ["/results/compiler-output"]
    );
    assert_eq!(option(&actual, "--emit").unwrap(), ["dep-info,metadata"]);
    assert_eq!(normalized_arguments(&actual, sysroot, input).unwrap(), args);
    assert_eq!(replay.args_sha256, <[u8; 32]>::from(Sha256::digest(&bytes)));
    assert_ne!(replay.args_sha256, request.args_sha256);
    assert_eq!(replay.source, request.source);
    assert_eq!((replay.case, replay.stage), (request.case, request.stage));
    assert!(replay_arguments(&bytes, &request, sysroot, output).is_err());
}

#[test]
fn prepared_vecadd_raw_capture_and_environment_are_bounded_and_sanitized() {
    for bytes in [b"A=x\0A=y\0".as_slice(), b"=x\0", b"A=x", b"A\0"] {
        assert!(raw_environment(bytes).is_err());
    }
    assert!(raw_arguments(b"/cwd\0").is_err());
    assert!(raw_arguments(b"/cwd\0rustc\0\xff\0").is_err());
    let raw = raw_environment(b"PATH=/bin\0CARGO_PKG_DESCRIPTION=\xff=description\0RUSTC_WRAPPER=bad\0CARGO_MAKEFLAGS=bad\0MAKEFLAGS=bad\0MFLAGS=bad\0LD_PRELOAD=bad\0FE2O3_TEST_CONDITIONAL_VECADD_V1=bad\0RUSTFLAGS=bad\0CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS=bad\0").unwrap();
    let environment = normalized_environment(raw, &parser_args("gfx942")).unwrap();
    assert_eq!(
        environment
            .iter()
            .find(|(k, _)| k == "CARGO_PKG_DESCRIPTION")
            .unwrap()
            .1,
        b"\xff=description"
    );
    for key in [
        "RUSTC_WRAPPER",
        "CARGO_MAKEFLAGS",
        "MAKEFLAGS",
        "MFLAGS",
        "LD_PRELOAD",
        REQUEST,
        "RUSTFLAGS",
        "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
    ] {
        assert!(!environment.iter().any(|(k, _)| k == key));
    }
    assert!(
        environment
            .iter()
            .any(|(k, _)| k == CRATE_BINDING_ID_ENV_V1)
    );
}

#[test]
fn prepared_vecadd_inventory_rejects_links_changed_dependencies_and_outputs() {
    let scratch = crate::test_temp_dir::TestTempDir::create("vecadd-preparation-parser");
    let root = scratch.path().canonicalize().unwrap();
    let dependencies = root.join("dependencies");
    fresh_directory(&dependencies).unwrap();
    new_file(
        &dependencies.join("actual.rmeta"),
        b"synthetic parser bytes",
    )
    .unwrap();
    let before = tree(&dependencies).unwrap();
    fs::write(dependencies.join("actual.rmeta"), b"changed parser bytes").unwrap();
    assert_ne!(before, tree(&dependencies).unwrap());
    std::os::unix::fs::symlink(
        dependencies.join("actual.rmeta"),
        dependencies.join("alias"),
    )
    .unwrap();
    assert!(tree(&dependencies).is_err());
    assert!(absolute_stamp(&dependencies.join("alias")).is_err());
    let output = root.join("compiler-output");
    fresh_directory(&output).unwrap();
    empty_output(&output).unwrap();
    new_file(&output.join("root.d"), b"depfile is also forbidden").unwrap();
    assert!(empty_output(&output).is_err());
    dep_info_only(&output).unwrap();
    new_file(
        &output.join("root.rmeta"),
        b"compiled metadata is forbidden",
    )
    .unwrap();
    assert!(dep_info_only(&output).is_err());
    assert!(fresh_directory(&output).is_err());
    assert!(canonical_directory(Path::new("relative")).is_err());
}
