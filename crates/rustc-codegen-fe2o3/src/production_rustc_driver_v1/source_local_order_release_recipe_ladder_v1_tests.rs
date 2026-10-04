//! 25 main + 3 explicit fault attempts; no automatic repair or hidden calls.
use super::*;

fn child(directory: &Path, step: cases_fixture::Step) -> Value {
    let record = cases_fixture::derive(directory, step);
    paths::write_new(
        &directory.join(format!("{}.invocation.json", step.id)),
        &serde_json::to_vec_pretty(&record).unwrap(),
    );
    let mut command = Command::new(std::env::current_exe().unwrap());
    let stdout = checked(
        sanitized(&mut command)
            .current_dir(repository())
            .args([
                "--exact",
                CHILD,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(INPUT, directory)
            .env(STEP, step.id)
            .env(CRATE_BINDING_ID_ENV_V1, &record.crate_binding)
            .env(
                CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
                &record.cargo_observation,
            )
            .env("CARGO_MANIFEST_DIR", fixture())
            .env("CARGO_PKG_NAME", PACKAGE)
            .env("CARGO_PKG_VERSION", "0.1.0")
            .env("CARGO_CRATE_NAME", CRATE_NAME),
        directory,
        step.id,
        None,
    );
    let stdout = std::str::from_utf8(&stdout).unwrap();
    let reports = stdout
        .lines()
        .filter_map(|line| line.strip_prefix(PREFIX))
        .collect::<Vec<_>>();
    assert_eq!(reports.len(), 1);
    assert!(reports[0].len() <= 128 * 1024);
    assert_eq!(
        stdout
            .lines()
            .filter(|line| line.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;"))
            .count(),
        1
    );
    let report: Value = serde_json::from_str(reports[0]).unwrap();
    assert_eq!(report["invocation"], serde_json::to_value(&record).unwrap());
    assert_eq!(report["compiler_callback_count"], 1);
    assert_eq!(
        report["callback_count"],
        if step.id == "probe-reentry" { 2 } else { 1 }
    );
    assert_eq!(report["fault_probe"], cases_fixture::is_fault(step));
    if step.id == "probe-fatal" {
        let stderr =
            read_bounded(&directory.join(format!("{}.stderr", step.id)), 64 * 1024).unwrap();
        let stderr = std::str::from_utf8(&stderr).unwrap();
        assert_eq!(
            stderr
                .lines()
                .filter(|line| line
                    .contains("controlled rustc fatal after actual local-order recipe callback"))
                .count(),
            1,
            "genuine rustc fatal diagnostic independently retained"
        );
    }
    assert_eq!(report["same_release_adapter"], true);
    assert_eq!(report["test_only_actual_l_observer"], true);
    assert_eq!(report["normal_example_executed"], false);
    for key in [
        "serialized_owner_read",
        "native_object_emitted",
        "hardware_observed",
        "grants_artifact_or_launch_authority",
    ] {
        assert_eq!(report[key], false);
    }
    if matches!(step.expected, cases_fixture::Expected::Success) {
        assert_eq!(report["observation"]["status"], "accepted");
        assert_eq!(report["observation"]["simulation_runs"], 30);
        let llvm = report["observation"]["llvm_text"].as_str().unwrap();
        assert!(!llvm.is_empty() && llvm.len() <= 48 * 1024);
        paths::write_new(&directory.join(format!("{}.ll", step.id)), llvm.as_bytes());
    } else {
        assert_eq!(report["observation"]["status"], "refused");
        assert_eq!(report["observation"]["simulation_runs"], 0);
        assert!(report["observation"].get("llvm_text").is_none());
        assert!(report["observation"].get("evidence").is_none());
    }
    report
}
fn row<'a>(rows: &'a [Value], id: &str) -> &'a Value {
    assert!(cases_fixture::STEPS.iter().any(|step| step.id == id));
    let mut matches = rows.iter().filter(|row| row["invocation"]["step"] == id);
    let row = matches.next().expect("fixed qualified row");
    assert!(matches.next().is_none());
    row
}
fn observation<'a>(rows: &'a [Value], id: &str) -> &'a Value {
    &row(rows, id)["observation"]
}
fn facts<'a>(rows: &'a [Value], id: &str) -> &'a Value {
    let row = observation(rows, id);
    assert_eq!(row["status"], "accepted");
    &row["evidence"]
}
fn same_program(rows: &[Value], left: &str, right: &str) {
    for field in [
        "source_sha256",
        "semantic_sha256",
        "instance_axes",
        "original",
        "input",
        "output",
        "actual_relation",
        "region",
        "output_result_order",
        "prefix_execution_bytes",
        "transition_sha256",
        "transition_bytes",
        "transition_rows",
        "fresh_formal_counts",
        "llvm_sha256",
        "descriptor_sha256",
        "descriptor_producer",
        "composition",
    ] {
        assert_eq!(
            facts(rows, left)[field],
            facts(rows, right)[field],
            "same current program field {field}: {left}/{right}"
        );
    }
    assert_eq!(
        observation(rows, left)["llvm_text"],
        observation(rows, right)["llvm_text"]
    );
}
fn independent_joins(rows: &[Value]) {
    for (create, replay, repeated) in [
        ("create-source-exact", "replay-source", "repeat-source"),
        ("create-reverse-exact", "replay-reverse", "repeat-reverse"),
    ] {
        same_program(rows, create, replay);
        same_program(rows, replay, repeated);
        assert_eq!(
            facts(rows, replay),
            facts(rows, repeated),
            "fresh deterministic evidence"
        );
        assert_eq!(
            observation(rows, replay)["oracle"],
            observation(rows, repeated)["oracle"]
        );
        assert_eq!(
            facts(rows, create)["recipe_sha256"],
            facts(rows, replay)["recipe_sha256"]
        );
        assert_eq!(facts(rows, create)["created"], true);
        assert_eq!(facts(rows, replay)["created"], false);
    }
    let source = facts(rows, "create-source-exact");
    let reverse = facts(rows, "create-reverse-exact");
    assert_eq!(source["original"], reverse["original"]);
    assert_eq!(source["input"], reverse["input"]);
    assert_eq!(source["input"], source["output"]);
    assert_ne!(source["output"], reverse["output"]);
    assert_ne!(source["llvm_sha256"], reverse["llvm_sha256"]);
    let input_order = source["output_result_order"].as_array().unwrap();
    assert_eq!(input_order.len(), 3);
    assert_eq!(
        reverse["output_result_order"],
        json!([input_order[1], input_order[0], input_order[2]])
    );
    assert_eq!(source["actual_relation"], "xor_before_or");
    assert_eq!(reverse["actual_relation"], "or_before_xor");

    for (original, rebind, edited) in [
        (
            "create-source-exact",
            "create-source-rebind",
            "edited-source-rebind",
        ),
        (
            "create-reverse-exact",
            "create-reverse-rebind",
            "edited-reverse-rebind",
        ),
    ] {
        same_program(rows, original, rebind);
        let old = facts(rows, rebind);
        let current = facts(rows, edited);
        assert_eq!(old["instance_axes"], current["instance_axes"]);
        assert_ne!(old["source_sha256"], current["source_sha256"]);
        assert_ne!(old["source_initializer"], current["source_initializer"]);
        assert_eq!(old["recipe_sha256"], current["recipe_sha256"]);
        assert_eq!(current["source_binding_mode"], "rebind_current");
        assert_eq!(old["requested_order"], current["requested_order"]);
        assert_eq!(old["actual_relation"], current["actual_relation"]);
        // Comment/rename need not change semantic graph hashes. Fresh current
        // checks and changed source/span evidence, not invented hash inequality.
    }
    for (created, replay) in [
        ("regenerate-source", "new-source-replay"),
        ("regenerate-reverse", "new-reverse-replay"),
    ] {
        same_program(rows, created, replay);
        assert_ne!(
            source["instance_axes"],
            facts(rows, created)["instance_axes"]
        );
        assert_ne!(source["original"], facts(rows, created)["original"]);
        assert_ne!(
            source["recipe_sha256"],
            facts(rows, created)["recipe_sha256"]
        );
        assert_eq!(facts(rows, created)["created"], true);
        assert_eq!(facts(rows, replay)["created"], false);
    }
    same_program(rows, "create-source-exact", "advisory-mismatch");
    let advisory = facts(rows, "advisory-mismatch");
    assert_eq!(
        advisory["constraint_outcome"],
        json!({"status":"not_honored","requested":"or_before_xor","actual":"xor_before_or"})
    );
    assert_eq!(advisory["requested_order"], "source_order");
    assert_eq!(advisory["strength"], "advisory");
}

#[test]
#[ignore = "pinned actual source, serialized Cargo, fresh absolute output directory"]
fn actual_source_local_order_release_recipe_ladder() {
    let directory = PathBuf::from(std::env::var_os(OUTPUT).expect("fresh release recipe output"));
    assert!(directory.is_absolute());
    fs::create_dir(&directory).unwrap();
    let directory = directory.canonicalize().unwrap();
    let source_root = paths::create_root(&directory);
    require_current_source();
    super::super::super::prepare(&directory);
    for case in cases_fixture::SOURCES {
        let case_dir = source_root.join(case);
        fs::create_dir(&case_dir).unwrap();
        paths::write_new(
            &case_dir.join("source.rs"),
            cases_fixture::source(case).as_bytes(),
        );
    }
    let mut observations = Vec::with_capacity(cases_fixture::STEPS.len());
    let mut fault_observations = Vec::with_capacity(cases_fixture::FAULTS.len());
    let mut initial_recipes = Vec::with_capacity(4);
    for (index, step) in cases_fixture::STEPS.into_iter().enumerate() {
        assert!(observations.len() < 25);
        observations.push(child(&directory, step));
        if index < 4 {
            let cases_fixture::Mode::Create {
                saved: Some(name), ..
            } = step.mode
            else {
                panic!("initial source recipes");
            };
            initial_recipes.push((name, hash(&io::recipe_path(&directory, name))));
        }
    }
    assert_eq!(observations.len(), 25);
    assert_eq!(
        observations
            .iter()
            .filter(|row| row["observation"]["status"] == "accepted")
            .count(),
        15
    );
    assert_eq!(
        observations
            .iter()
            .filter(|row| row["observation"]["status"] == "refused")
            .count(),
        10
    );
    let callbacks = observations
        .iter()
        .map(|row| row["compiler_callback_count"].as_u64().unwrap())
        .sum::<u64>();
    let main_methods = observations
        .iter()
        .map(|row| row["callback_count"].as_u64().unwrap())
        .sum::<u64>();
    let simulations = observations
        .iter()
        .map(|row| row["observation"]["simulation_runs"].as_u64().unwrap())
        .sum::<u64>();
    assert_eq!(callbacks, 25);
    assert_eq!(main_methods, 25);
    assert_eq!(simulations, 450);
    independent_joins(&observations);
    let successful = observations
        .iter()
        .filter(|row| row["observation"]["status"] == "accepted");
    let oracle_steps = successful
        .clone()
        .map(|row| {
            row["observation"]["oracle"]["oracle_accounting"]["steps"]
                .as_u64()
                .unwrap()
        })
        .sum::<u64>();
    let oracle_payload = successful
        .map(|row| {
            row["observation"]["oracle"]["oracle_accounting"]["prepaid_host_payload"]
                .as_u64()
                .unwrap()
        })
        .sum::<u64>();
    assert!(oracle_steps <= 15 * 8_000_000);
    assert!(oracle_payload <= 15 * 2 * 1024 * 1024);
    for step in cases_fixture::FAULTS {
        assert!(fault_observations.len() < 3);
        fault_observations.push(child(&directory, step));
    }
    assert_eq!(fault_observations.len(), 3);
    assert!(
        fault_observations
            .iter()
            .all(|row| row["observation"]["status"] == "refused"
                && row["observation"]["simulation_runs"] == 0)
    );
    let fault_compiler_callbacks = fault_observations
        .iter()
        .map(|row| row["compiler_callback_count"].as_u64().unwrap())
        .sum::<u64>();
    let fault_method_callbacks = fault_observations
        .iter()
        .map(|row| row["callback_count"].as_u64().unwrap())
        .sum::<u64>();
    assert_eq!(fault_compiler_callbacks, 3);
    assert_eq!(fault_method_callbacks, 4);
    let mut recipes = Vec::with_capacity(cases_fixture::RECIPE_NAMES.len());
    for name in cases_fixture::RECIPE_NAMES {
        let path = io::recipe_path(&directory, name);
        let mut retained = io::RetainedRecipe::open(&path).unwrap();
        retained.recheck().unwrap();
        recipes.push(json!({"name":name,"bytes":retained.bytes().len(),"sha256":hash(&path)}));
    }
    for (name, before) in initial_recipes {
        assert_eq!(
            hash(&io::recipe_path(&directory, name)),
            before,
            "old recipe overwritten"
        );
    }
    let mut source_bytes = 0u64;
    for case in cases_fixture::SOURCES {
        let path = cases_fixture::absolute(&directory, case);
        source_bytes += fs::metadata(&path).unwrap().len();
        assert_eq!(
            fs::read_dir(path.parent().unwrap())
                .unwrap()
                .take(2)
                .count(),
            1
        );
    }
    assert!(source_bytes <= 12 * 64 * 1024);
    assert_eq!(fs::read_dir(&source_root).unwrap().take(13).count(), 12);
    require_current_source();
    let mut report = json!({
        "schema":"task-source-local-order-release-recipe-qualification-v1",
        "observations":observations,"fault_observations":fault_observations,"recipes":recipes,"source_directory":paths::relative_root(&directory),
        "source_files":12,"source_bytes":source_bytes,
        "main_driver_attempts":25,"main_compiler_callbacks":callbacks,"main_method_callbacks":main_methods,
        "total_driver_attempts":28,"total_method_callbacks":main_methods+fault_method_callbacks,
        "fault_driver_attempts":3,"fault_compiler_callbacks":fault_compiler_callbacks,
        "fault_method_callbacks":fault_method_callbacks,"total_compiler_callbacks":callbacks+fault_compiler_callbacks,
        "oracle_steps":oracle_steps,"oracle_prepaid_host_payload":oracle_payload,
    });
    let Value::Object(summary) = json!({
        "main_positive_attempts":15,"main_refused_attempts":10,
        "fault_refused_attempts":3,"total_positive_attempts":15,"total_refused_attempts":13,
        "simulation_runs":simulations,
        "exact_schedules":2,"same_source_repeats":2,"edited_exact_refusals":2,"edited_rebind_successes":2,
        "changed_item_refusals":1,"explicit_regenerations":2,"fresh_regenerated_replays":2,
        "source_precondition_refusals":6,"exact_constraint_refusals":1,"advisory_mismatches_reported":1,
        "same_release_adapter":true,"test_only_actual_l_observer":true,"normal_example_executed":false,
        "caller_retained_recipe_files":true,"api_recipe_file_custody":false,
        "existing_policy6_modified":false,"serialized_owner_read":false,
        "native_object_emitted":false,"hardware_observed":false,"grants_artifact_or_launch_authority":false,
    }) else {
        unreachable!("literal report object")
    };
    report.as_object_mut().unwrap().extend(summary);
    let report = serde_json::to_vec_pretty(&report).unwrap();
    assert!(report.len() <= 512 * 1024);
    paths::write_new(&directory.join("observation.json"), &report);
}
