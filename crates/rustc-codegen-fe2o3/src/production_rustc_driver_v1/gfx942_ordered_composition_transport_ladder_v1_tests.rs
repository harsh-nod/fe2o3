//! Fresh actual publication and source-role matrix. Reports remain inert.
use super::*;
fn role_session(root: &Path, variant: &str, started: std::time::Instant) -> Value {
    let session = std::time::Instant::now();
    let mode = transport_roles::MODE;
    let name = transport_roles::case(variant).unwrap();
    let record = inputs::invocation(root, variant, variant);
    let source = inputs::snapshot(&inputs::source(root, variant), 72 * 1024);
    inputs::write_invocation(root, &name, &record);
    let mut child = Command::new(std::env::current_exe().unwrap());
    inputs::environment(&mut child, root, &record);
    child
        .args(["--exact", CHILD, "--ignored", "--nocapture"])
        .env(INPUT_ENV, root)
        .env(VARIANT_ENV, variant)
        .env(MODE_ENV, mode)
        .env("FE2O3_EXTRACT_ORDERED_COMPOSITION_V1", "1")
        .env(CRATE_BINDING_ID_ENV_V1, &record.crate_binding)
        .env(
            CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
            &record.cargo_observation,
        );
    timely(session.elapsed(), 300).unwrap();
    timely(started.elapsed(), 2400).unwrap();
    let bytes = checked(&mut child, root, &name, None);
    timely(session.elapsed(), 300).unwrap();
    timely(started.elapsed(), 2400).unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    let frames = text
        .lines()
        .filter_map(|line| line.strip_prefix(transport_roles::FRAME_PREFIX))
        .collect::<Vec<_>>();
    assert_eq!(frames.len(), 1);
    assert_eq!(
        text.lines()
            .filter(|line| line.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;"))
            .count(),
        1
    );
    let observed: Value = serde_json::from_str(frames[0]).unwrap();
    assert_eq!(observed["schema"], transport_roles::SCHEMA);
    assert_eq!(observed["variant"], variant);
    assert_eq!(observed["mode"], mode);
    assert_eq!(
        observed["invocation"],
        serde_json::to_value(&record).unwrap()
    );
    assert_eq!(observed["source"], serde_json::to_value(&source).unwrap());
    assert_eq!(observed["actual_fresh_frontend"], true);
    for key in [
        "runtime_conditions_discharged",
        "source_custody_exported",
        "hardware_observed",
        "protected_authority",
    ] {
        assert_eq!(observed[key], false);
    }
    inputs::recheck(root, variant, variant, &record, &source);
    timely(session.elapsed(), 300).unwrap();
    timely(started.elapsed(), 2400).unwrap();
    publish_json(root, &format!("{name}.accepted.json"), &observed);
    timely(session.elapsed(), 300).unwrap();
    timely(started.elapsed(), 2400).unwrap();
    observed
}

fn joined_observation(role: &Value, normal: &Value) -> Result<(), &'static str> {
    if role["schema"] != transport_roles::SCHEMA
        || role["source_profile"] != normal["source_profile"]
        || normal["fresh_checked_owner"] != true
        || role["compiler_owner_replayed_by_normal_descriptor"] != true
    {
        return Err("fresh transport/normal profile");
    }
    for key in [
        "semantic_identity",
        "canonical_identity",
        "canonical_sha256",
        "llvm_sha256",
        "descriptor_sha256",
        "handoff_sha256",
        "cpu",
    ] {
        if role[key].is_null() || role[key] != normal[key] {
            return Err("fresh transport/normal identity");
        }
    }
    for key in [
        "machine_abi_transport_proved",
        "native_functional_equivalence",
        "runtime_conditions_discharged",
        "source_custody_exported",
        "hardware_observed",
        "protected_authority",
    ] {
        if role[key] != false {
            return Err("transport authority escalation");
        }
    }
    if role["role_substitution_refusals"] != 18
        || role["descriptor_extension_refusals"] != 3
        || role["pointer_and_predicate_recorded_not_proved"] != true
        || role["output_condition"]
            != json!({"parameter":0,"minimum_bytes":512,
            "initialized_read":false,"write_permission":true})
    {
        return Err("transport bounded profile");
    }
    Ok(())
}
fn publish_transport_report(root: &Path, report: &Value, started: std::time::Instant) {
    let positive = root.join("transport-observation.json");
    let rejected = root.join("transport-observation.unaccepted.json");
    assert!(!positive.exists() && !rejected.exists());
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let bytes = serde_json::to_vec_pretty(report).unwrap();
        assert!(bytes.len() <= 4 * 1024 * 1024);
        timely(started.elapsed(), 2400).unwrap();
        publisher::create(&positive, &bytes, 4 * 1024 * 1024);
        timely(started.elapsed(), 2400).unwrap();
        let mut out = std::io::stdout().lock();
        out.write_all(b"Fresh source transport roles joined current normal outputs; machine transport pending.\n").unwrap();
        out.flush().unwrap();
        timely(started.elapsed(), 2400).unwrap();
    }));
    if let Err(error) = result {
        if positive.exists() {
            assert!(!rejected.exists());
            fs::rename(positive, rejected).unwrap();
        }
        std::panic::resume_unwind(error);
    }
}
#[test]
#[ignore = "root-owned fresh public source plus transport matrix; current completed build pins required"]
fn actual_transport_roles_ladder() {
    let started = std::time::Instant::now();
    let build = pins::Build::read();
    let root = PathBuf::from(std::env::var_os(OUTPUT_ENV).expect("fresh transport output"));
    assert!(!root.exists());
    let sources = qualification::inputs::current_sources();
    let lock = read_bounded(&repository().join("Cargo.lock"), 1024 * 1024).unwrap();
    // Run the existing actual producer on this NEW output tree. Its schema and
    // report remain unchanged; it is not accepted as a transport-role report.
    super::actual_promoted_normal_ladder();
    timely(started.elapsed(), 2400).unwrap();
    let base_bytes = read_bounded(&root.join("observation.json"), 4 * 1024 * 1024).unwrap();
    let base: Value = serde_json::from_slice(&base_bytes).unwrap();
    assert_eq!(
        base["schema"],
        "fe2o3-test-composition-promoted-normal-ladder-v1"
    );
    assert_eq!(base["workload_children"], 23);
    assert_eq!(base["build_inputs"], serde_json::to_value(&build).unwrap());
    let dependencies = publisher::preparation(&root, "package-original");
    let dependency_before = qualification::inputs::dependency_snapshot(&dependencies).0;
    assert_eq!(
        base["dependencies"],
        serde_json::to_value(&dependency_before).unwrap()
    );
    let original = inputs::snapshot(&inputs::source(&root, "original"), 72 * 1024);
    let snapshots = ["copy", "preserve", "edit"]
        .map(|v| (v, inputs::snapshot(&inputs::source(&root, v), 72 * 1024)));
    let mut roles = Vec::new();
    for variant in ["copy", "preserve", "edit"] {
        let row = role_session(&root, variant, started);
        let matches: Vec<_> = base["normal_sessions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|n| n["variant"] == variant && n["mode"] == "observe")
            .collect();
        assert_eq!(matches.len(), 1);
        let normal = matches[0];
        assert_eq!(row["source"], normal["source"]);
        assert_eq!(row["invocation"], normal["invocation"]);
        joined_observation(&row["observation"], &normal["observation"]).unwrap();
        let role_output = root.join(format!("{variant}-transport-roles.output"));
        let normal_output = root.join(format!("{variant}-observe.output"));
        for name in [
            "canonical-v17.bin",
            "canonical.ll",
            "worker.ll",
            "handoff-v2.bin",
            "descriptor-v1.bin",
        ] {
            assert_eq!(
                read_bounded(&role_output.join(name), 4 * 1024 * 1024).unwrap(),
                read_bounded(&normal_output.join(name), 4 * 1024 * 1024).unwrap()
            );
        }
        let observed: Value = serde_json::from_slice(
            &read_bounded(&role_output.join("transport-roles.json"), 32 * 1024).unwrap(),
        )
        .unwrap();
        assert_eq!(observed, row["observation"]);
        roles.push(row);
    }
    assert_eq!(
        inputs::snapshot(&inputs::source(&root, "original"), 72 * 1024),
        original
    );
    for (variant, snapshot) in snapshots {
        assert_eq!(
            inputs::snapshot(&inputs::source(&root, variant), 72 * 1024),
            snapshot
        );
    }
    assert_eq!(
        qualification::inputs::dependency_snapshot(&dependencies).0,
        dependency_before
    );
    assert_eq!(qualification::inputs::current_sources(), sources);
    assert_eq!(
        read_bounded(&repository().join("Cargo.lock"), 1024 * 1024).unwrap(),
        lock
    );
    assert_eq!(
        read_bounded(&root.join("observation.json"), 4 * 1024 * 1024).unwrap(),
        base_bytes
    );
    build.recheck();
    timely(started.elapsed(), 2400).unwrap();
    let report = json!({
        "schema":"fe2o3-test-composition-transport-ladder-v1",
        "workload_children":26,"actual_extractor_invocations":13,
        "source_publications":3,"actual_normal_sessions":10,"actual_transport_sessions":3,
        "normal_cpu_cases":128,"transport_cpu_cases":96,
        "cpu_counts_overlap_recompiled_source_graphs":true,
        "role_substitution_refusals":54,"transport_descriptor_extension_refusals":9,
        "normal_report_bytes":base_bytes.len(),"normal_report_sha256":digest(&base_bytes),
        "normal_report":base,"transport_sessions":roles,"build_inputs":build,
        "provider_sources":sources,"dependencies":dependency_before,
        "fresh_source_custody_constructed_in_callback":true,
        "source_custody_from_files":false,"source_custody_exported":false,
        "machine_abi_transport_proved":false,"native_functional_equivalence":false,
        "runtime_conditions_discharged":false,"hardware_observed":false,
        "protected_authority":false,"historical_evidence_rewritten":false,
        "cleanup_scope":"bounded direct child/process group only; outer root supervisor required",
        "acceptance":"completed successful new parent and root runner; JSON alone is historical"
    });
    publish_transport_report(&root, &report, started);
}
#[test]
fn transport_ladder_join_refuses_missing_identity_and_authority() {
    let normal = json!({"source_profile":"copy","fresh_checked_owner":true,
        "semantic_identity":"s","canonical_identity":"c","canonical_sha256":"b",
        "llvm_sha256":"l","descriptor_sha256":"d","handoff_sha256":"h","cpu":{"cases":32}});
    let mut role = normal.clone();
    role["schema"] = json!(transport_roles::SCHEMA);
    role["compiler_owner_replayed_by_normal_descriptor"] = json!(true);
    for key in [
        "machine_abi_transport_proved",
        "native_functional_equivalence",
        "runtime_conditions_discharged",
        "source_custody_exported",
        "hardware_observed",
        "protected_authority",
    ] {
        role[key] = json!(false);
    }
    role["role_substitution_refusals"] = json!(18);
    role["descriptor_extension_refusals"] = json!(3);
    role["pointer_and_predicate_recorded_not_proved"] = json!(true);
    role["output_condition"] = json!({"parameter":0,"minimum_bytes":512,
        "initialized_read":false,"write_permission":true});
    joined_observation(&role, &normal).unwrap();
    for key in [
        "semantic_identity",
        "canonical_identity",
        "canonical_sha256",
        "llvm_sha256",
        "descriptor_sha256",
        "handoff_sha256",
        "cpu",
    ] {
        let mut changed = role.clone();
        changed[key] = Value::Null;
        assert!(joined_observation(&changed, &normal).is_err());
        changed[key] = json!("foreign");
        assert!(joined_observation(&changed, &normal).is_err());
    }
    for key in [
        "machine_abi_transport_proved",
        "native_functional_equivalence",
        "runtime_conditions_discharged",
        "source_custody_exported",
        "hardware_observed",
        "protected_authority",
    ] {
        let mut changed = role.clone();
        changed[key] = json!(true);
        assert!(joined_observation(&changed, &normal).is_err());
    }
}
