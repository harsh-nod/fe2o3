//! Three real source sessions: old current analysis, edited stale rejection,
//! then current edited analysis recovery. The seed is published by the normal
//! external library consumer, not synthesized from a prior compiler graph.
use super::*;
use crate::production_pipeline::source_candidate_launch_analysis_v17_tests::SourceLaunchAnalysisEvidenceV17;

const OUTPUT: &str = "FE2O3_TEST_SOURCE_CANDIDATE_LAUNCH_ANALYSIS_OUTPUT";
const CHILD: &str = "production_rustc_driver_v1::source_bitselect_feasibility_v1_tests::roundtrip::machine::launch_analysis::actual_source_candidate_launch_analysis_child";
const PREFIX: &str = "FE2O3_SOURCE_CANDIDATE_LAUNCH_ANALYSIS ";
const MODES: [&str; 3] = ["default-current", "edited-stale", "edited-current"];
const SELECTORS: [&str; 3] = ["default", "edited", "edited"];
const REPORT_CAP: usize = 64 * 1024;

enum Observation {
    Current(SourceLaunchAnalysisEvidenceV17),
    Stale(Value),
}

struct AnalysisCallbacks {
    source: Option<RetainedInput>,
    plan: Gfx942OrderedProgramRegistersV1,
    old: Option<SourceLaunchAnalysisEvidenceV17>,
    calls: usize,
    result: Option<Result<Observation, String>>,
}

impl Callbacks for AnalysisCallbacks {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        let input = self
            .source
            .take()
            .expect("one retained source per actual callback");
        let old = self.old.take();
        self.result = Some(
            crate::production_rustc_driver_v1::transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )
            .and_then(|transaction| match old {
                Some(old) => transaction
                    .refuse_stale_source_candidate_launch_analysis_v17(input, old)
                    .map(Observation::Stale),
                None => transaction
                    .observe_source_candidate_launch_analysis_v17(input, self.plan)
                    .map(Observation::Current),
            }),
        );
        Compilation::Stop
    }
}

fn records(directory: &Path) -> [RoundtripInvocation; 3] {
    let records = SELECTORS.map(|selector| derive(directory, selector));
    for record in &records {
        assert_eq!(record.crate_binding, records[0].crate_binding);
        assert_eq!(record.cargo_observation, records[0].cargo_observation);
        assert_eq!(
            record.source_directory,
            paths::relative_case(directory, "positive")
        );
        assert!(record.args.len() <= 512);
        assert!(record.args.iter().map(String::len).sum::<usize>() <= 64 * 1024);
        crate::production_rustc_driver_v1::require_canonical_overflow_checks_v1(&record.args)
            .unwrap();
    }
    assert_ne!(records[0].source_sha256, records[1].source_sha256);
    assert_eq!(records[1], records[2]);
    records
}

fn fixed_environment(record: &RoundtripInvocation) {
    assert_eq!(std::env::current_dir().unwrap(), repository());
    assert_eq!(
        std::env::var(CRATE_BINDING_ID_ENV_V1).unwrap(),
        record.crate_binding
    );
    assert_eq!(
        std::env::var(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2).unwrap(),
        record.cargo_observation,
    );
    assert_eq!(
        std::env::var("CARGO_MANIFEST_DIR").unwrap(),
        fixture().to_str().unwrap()
    );
    assert_eq!(std::env::var("CARGO_PKG_NAME").unwrap(), PACKAGE);
    assert_eq!(std::env::var("CARGO_PKG_VERSION").unwrap(), "0.1.0");
    assert_eq!(std::env::var("CARGO_CRATE_NAME").unwrap(), CRATE_NAME);
}

fn report_sha256(value: &Value) -> String {
    let words = value.as_array().expect("actual SHA256 array");
    assert_eq!(words.len(), 32);
    let mut bytes = [0_u8; 32];
    for (byte, word) in bytes.iter_mut().zip(words) {
        *byte = u8::try_from(word.as_u64().expect("SHA256 byte")).unwrap();
    }
    crate::production_rustc_driver_v1::lower_hex_v1(&bytes)
}

fn require_current(report: &Value, edited: bool) {
    assert_eq!(report["kind"], "actual_source_launch_analysis_current_v17");
    assert_eq!(report["fresh"]["semantic_version"], "V32");
    assert_eq!(report["fresh"]["kernel_ir_version"], "V17");
    assert_eq!(report["source_sha256"], report["fresh"]["candidate_sha256"]);
    assert_eq!(
        report["semantic_sha256"],
        report["fresh"]["semantic_sha256"]
    );
    assert_eq!(
        report["canonical_sha256"],
        report["fresh"]["kernel_ir_sha256"]
    );
    assert_eq!(report["whole_kernel_simulation"]["runs"], 30);
    assert_eq!(
        report["whole_kernel_simulation"]["cumulative_step_limit"],
        4_000_000
    );
    assert!(report["whole_kernel_simulation"]["steps"].as_u64().unwrap() <= 4_000_000);
    assert_eq!(
        report["whole_kernel_simulation"]["output_and_canaries_checked"],
        true
    );
    assert_eq!(report["whole_kernel_simulation"]["immutable_inputs"], true);
    assert_eq!(
        report["whole_kernel_simulation"]["simulation_is_proof"],
        false
    );
    assert_eq!(
        report["whole_kernel_simulation"]["hardware_observed"],
        false
    );
    assert_eq!(report["actual_current_owner_materialized"], true);
    assert_eq!(report["source_currentness_rechecked"], true);
    assert_eq!(report["analysis_roots"], 1);
    assert_eq!(report["retained_extra_roster_payload_cap"], 16 * 1024);
    assert!(
        report["retained_extra_roster_payload_bytes"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(
        report["retained_extra_roster_payload_bytes"]
            .as_u64()
            .unwrap()
            <= 16 * 1024
    );
    assert_eq!(
        report["register_plan"],
        if edited {
            json!([32, 33, 34, 35, 36])
        } else {
            json!([4, 5, 0, 1, 2])
        },
    );
    for field in [
        "old_analysis_used_for_success",
        "ranked_checks",
        "functional_proof",
        "proof_invalidation_qualified",
        "production_resume",
        "hardware_observed",
        "source_authentication_claim",
        "grants_artifact_or_launch_authority",
        "resource_accounting_is_rss",
    ] {
        assert_eq!(report[field], false, "{field} remains unavailable");
    }
}

fn require_stale(report: &Value) {
    assert_eq!(
        report["kind"],
        "actual_source_launch_analysis_stale_refusal_v17"
    );
    assert_eq!(
        report["consumer"],
        "ProductionOrderedProgramPreRankedKirOwnerV17::try_materialize_with_budget",
    );
    assert_ne!(report["old_source_sha256"], report["current_source_sha256"]);
    assert_ne!(
        report["old_semantic_sha256"],
        report["current_semantic_sha256"]
    );
    assert_eq!(report["old_register_plan"], json!([4, 5, 0, 1, 2]));
    assert_eq!(report["current_register_plan"], json!([32, 33, 34, 35, 36]));
    assert_eq!(report["workgroup"], json!([64, 1, 1]));
    assert_eq!(report["subgroup"], 64);
    assert_eq!(report["full_physical_workgroups"], true);
    assert_eq!(report["exact_stale_analysis_refusals"], 1);
    assert_eq!(report["error"], "Lowering::Unsupported");
    assert_eq!(report["function"], 0);
    assert!(report["block"].is_null());
    assert!(report["statement"].is_null());
    assert_eq!(
        report["detail"],
        "ordered program requires one exact V32 gfx942 source and launch root",
    );
    assert_eq!(report["negative_work_units"], 8);
    assert_eq!(report["prefix_work_units"], 7);
    assert_eq!(report["accepted_work_units"], 15);
    assert_eq!(report["work_limit"], 15);
    assert_eq!(
        report["peak_storage_bytes"],
        report["incoming_storage_bytes"]
    );
    assert!(report["incoming_storage_bytes"].as_u64().unwrap() > 0);
    assert!(report["incoming_storage_bytes"].as_u64().unwrap() <= 16 * 1024);
    assert_eq!(report["returned_storage_bytes"], 0);
    assert_eq!(report["storage_limit"], 16 * 1024);
    for field in [
        "same_geometry",
        "incoming_storage_restored",
        "caller_storage_released",
        "same_work_ledger",
        "source_currentness_rechecked",
        "old_analysis_used_only_as_negative_input",
    ] {
        assert_eq!(report[field], true);
    }
    for field in [
        "new_executable_owner_returned",
        "ranked_checks",
        "functional_proof",
        "proof_invalidation_qualified",
        "production_resume",
        "hardware_observed",
        "resource_accounting_is_rss",
        "source_authentication_claim",
        "grants_artifact_or_launch_authority",
    ] {
        assert_eq!(report[field], false, "{field} remains unavailable");
    }
}

fn require_join(report: &Value) {
    assert_eq!(
        report["kind"],
        "private_actual_source_launch_analysis_join_v17"
    );
    assert_eq!(report["actual_rustc_callbacks"], 3);
    assert_eq!(report["actual_current_owner_materializations"], 2);
    assert_eq!(report["exact_stale_analysis_refusals"], 1);
    assert_eq!(report["positive_whole_kernel_oracle_runs"], 60);
    assert_eq!(report["positive_simulation_step_limit"], 8_000_000);
    let observations = report["observations"].as_array().unwrap();
    assert_eq!(observations.len(), 3);
    for (row, mode) in observations.iter().zip(MODES) {
        assert_eq!(row["mode"], mode);
    }
    let default = &observations[0]["observation"];
    let stale = &observations[1]["observation"];
    let current = &observations[2]["observation"];
    require_current(default, false);
    require_stale(stale);
    require_current(current, true);
    assert_eq!(stale["old_source_sha256"], default["source_sha256"]);
    assert_eq!(stale["old_semantic_sha256"], default["semantic_sha256"]);
    assert_eq!(stale["old_canonical_sha256"], default["canonical_sha256"]);
    assert_eq!(stale["current_source_sha256"], current["source_sha256"]);
    assert_eq!(stale["current_semantic_sha256"], current["semantic_sha256"]);
    assert_ne!(default["canonical_sha256"], current["canonical_sha256"]);
    assert_eq!(
        default["fresh"]["descriptors"],
        current["fresh"]["descriptors"]
    );
    for field in [
        "ranked_checks",
        "functional_proof",
        "proof_invalidation_qualified",
        "production_resume",
        "native_emission",
        "hardware_observed",
        "source_authentication_claim",
        "grants_artifact_or_launch_authority",
    ] {
        assert_eq!(report[field], false);
    }
}

#[test]
#[ignore = "three sequential actual rustc sessions; use the bounded parent ladder"]
fn actual_source_candidate_launch_analysis_child() {
    let directory = PathBuf::from(std::env::var_os(INPUT).expect("prepared absolute output"));
    require_current_source();
    let prepared = records(&directory);
    for (index, record) in prepared.iter().enumerate() {
        let retained: RoundtripInvocation = serde_json::from_slice(
            &read_bounded(
                &directory
                    .join("positive")
                    .join(format!("launch-{}.invocation.json", MODES[index])),
                64 * 1024,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(*record, retained);
        fixed_environment(record);
    }
    let mut old = None;
    let mut observations = Vec::with_capacity(3);
    for index in 0..3 {
        let record = &prepared[index];
        assert_eq!(derive(&directory, SELECTORS[index]), *record);
        fixed_environment(record);
        let source_path = record
            .source_directory
            .join(source_files(SELECTORS[index]).0);
        let source = RetainedInput::open(source_path.to_str().unwrap(), true).unwrap();
        let mut callbacks = AnalysisCallbacks {
            source: Some(source),
            plan: register_plan(SELECTORS[index]),
            old: if index == 1 { old.take() } else { None },
            calls: 0,
            result: None,
        };
        if index == 1 {
            assert!(callbacks.old.is_some(), "genuine prior analysis required");
        }
        rustc_driver::run_compiler(&record.args, &mut callbacks);
        assert_eq!(
            callbacks.calls, 1,
            "each case must reach its actual callback"
        );
        let report = match callbacks.result.take().expect("actual callback").unwrap() {
            Observation::Current(evidence) => {
                assert!(index == 0 || index == 2);
                require_current(&evidence.report, index == 2);
                let report = evidence.report.clone();
                if index == 0 {
                    old = Some(evidence);
                } else {
                    drop(evidence);
                }
                report
            }
            Observation::Stale(report) => {
                assert_eq!(index, 1);
                require_stale(&report);
                report
            }
        };
        assert!(serde_json::to_vec(&report).unwrap().len() <= 16 * 1024);
        let source_field = if index == 1 {
            "current_source_sha256"
        } else {
            "source_sha256"
        };
        assert_eq!(report_sha256(&report[source_field]), record.source_sha256);
        assert_eq!(derive(&directory, SELECTORS[index]), *record);
        assert_eq!(hash(&source_path), record.source_sha256);
        fixed_environment(record);
        observations.push(json!({"mode":MODES[index],"invocation":record,"observation":report}));
        drop(callbacks);
    }
    assert!(
        old.is_none(),
        "old roster was consumed only by the negative"
    );
    assert_eq!(records(&directory), prepared);
    require_current_source();
    assert!(
        fs::read_dir(directory.join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
    let report = json!({
        "kind":"private_actual_source_launch_analysis_join_v17",
        "observations":observations,"actual_rustc_callbacks":3,
        "fixed_environment_sequential_sessions":3,
        "actual_current_owner_materializations":2,"exact_stale_analysis_refusals":1,
        "positive_whole_kernel_oracle_runs":60,"positive_simulation_step_limit":8_000_000,
        "old_analysis_used_only_as_negative_input":true,
        "ranked_checks":false,"functional_proof":false,"proof_invalidation_qualified":false,
        "production_resume":false,"native_emission":false,"hardware_observed":false,
        "source_authentication_claim":false,"grants_artifact_or_launch_authority":false,
    });
    require_join(&report);
    let bytes = serde_json::to_vec(&report).unwrap();
    assert!(bytes.len() <= REPORT_CAP);
    println!("\n{PREFIX}{}", std::str::from_utf8(&bytes).unwrap());
}

fn run_child(directory: &Path) -> Value {
    let prepared = records(directory);
    for (index, record) in prepared.iter().enumerate() {
        let bytes = serde_json::to_vec_pretty(record).unwrap();
        assert!(bytes.len() <= 64 * 1024);
        paths::write_new(
            &directory
                .join("positive")
                .join(format!("launch-{}.invocation.json", MODES[index])),
            &bytes,
        );
    }
    let mut child = Command::new(std::env::current_exe().unwrap());
    let bytes = checked(
        sanitized(&mut child)
            .current_dir(repository())
            .args([
                "--exact",
                CHILD,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(INPUT, directory)
            .env(CRATE_BINDING_ID_ENV_V1, &prepared[0].crate_binding)
            .env(
                CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
                &prepared[0].cargo_observation,
            )
            .env("CARGO_MANIFEST_DIR", fixture())
            .env("CARGO_PKG_NAME", PACKAGE)
            .env("CARGO_PKG_VERSION", "0.1.0")
            .env("CARGO_CRATE_NAME", CRATE_NAME),
        directory,
        "actual-source-launch-analysis",
        None,
    );
    let stdout = std::str::from_utf8(&bytes).unwrap();
    assert_eq!(
        stdout
            .lines()
            .filter(|line| line.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;"))
            .count(),
        1,
    );
    let mut lines = stdout.lines().filter_map(|line| line.strip_prefix(PREFIX));
    let line = lines
        .next()
        .expect("one genuine source-launch analysis report");
    assert!(line.len() <= REPORT_CAP && lines.next().is_none());
    let report: Value = serde_json::from_str(line).unwrap();
    require_join(&report);
    for (index, record) in prepared.iter().enumerate() {
        assert_eq!(
            report["observations"][index]["invocation"],
            serde_json::to_value(record).unwrap(),
        );
    }
    assert_eq!(records(directory), prepared);
    report
}

#[test]
#[ignore = "public normal-library promotion plus three real source callbacks; fresh scoped output"]
fn actual_source_candidate_launch_analysis_ladder() {
    let consumer = PathBuf::from(
        std::env::var_os(headless_machine::CONSUMER).expect("absolute normal-library consumer"),
    );
    let consumer_before = headless_machine::consumer_observation(&consumer);
    let directory = PathBuf::from(std::env::var_os(OUTPUT).expect("fresh absolute output"));
    assert!(directory.is_absolute());
    fs::create_dir(&directory).unwrap();
    let directory = directory.canonicalize().unwrap();
    let source_root = paths::create_root(&directory);
    require_current_source();
    super::super::prepare(&directory);
    fs::create_dir(directory.join("positive")).unwrap();
    let case = source_root.join("positive");
    fs::create_dir(&case).unwrap();
    paths::write_new(&case.join("original.rs"), FIXTURE_FILES[2].1);
    paths::write_new(&case.join("original-loader.rs"), ORIGINAL_LOADER);
    paths::write_new(&case.join("candidate-loader.rs"), CANDIDATE_LOADER);
    let original_sha = hash(&case.join("original.rs"));
    let publication = headless_machine::publish_with_normal_consumer(&directory, &consumer);
    let candidate_sha = hash(&case.join("candidate.rs"));
    assert_eq!(publication["candidate_sha256"], candidate_sha);
    let edits = fixture_cases::run_edits(&directory);
    let edited_sha = hash(&case.join("edited.rs"));
    assert_ne!(candidate_sha, edited_sha);
    let joined = run_child(&directory);
    let first = &joined["observations"][0]["invocation"];
    let edited = &joined["observations"][2]["invocation"];
    assert_eq!(first["source_sha256"], candidate_sha);
    assert_eq!(edited["source_sha256"], edited_sha);
    assert_eq!(hash(&case.join("original.rs")), original_sha);
    assert_eq!(hash(&case.join("candidate.rs")), candidate_sha);
    assert_eq!(hash(&case.join("edited.rs")), edited_sha);
    assert_eq!(
        headless_machine::consumer_observation(&consumer),
        consumer_before
    );
    require_current_source();
    let (source_files, source_bytes) = fixture_cases::footprint(&directory);
    let mut report = json!({
        "kind":"private_public_seed_source_launch_analysis_ladder_v17",
        "normal_consumer_publication":publication,"edit_publications":edits,"joined":joined,
        "normal_library_publication_processes":1,"actual_fresh_callbacks":3,
        "actual_current_owner_materializations":2,"exact_stale_analysis_refusals":1,
        "positive_whole_kernel_oracle_runs":60,"positive_simulation_step_limit":8_000_000,
        "instruction_profile":"unchanged exact three-op bitselect with a real register edit",
        "source_directory":paths::relative_root(&directory),
        "source_files":source_files,"source_bytes":source_bytes,
        "source_file_limit":10,"source_byte_limit":10*128*1024,
        "original_sha256":original_sha,"candidate_sha256":candidate_sha,
        "edited_sha256":edited_sha,"fresh_owner_version":"V17",
    });
    // Keep macro expansion bounded; these disjoint fields remain top-level.
    let Value::Object(claims) = json!({
        "ranked_checks":false,"functional_proof":false,"proof_invalidation_qualified":false,
        "new_wire_schema":false,"native_emission":false,"production_resume":false,
        "hardware_observed":false,"source_authentication_claim":false,
        "grants_artifact_or_launch_authority":false,
    }) else {
        unreachable!("literal JSON object");
    };
    report
        .as_object_mut()
        .expect("literal JSON object")
        .extend(claims);
    let bytes = serde_json::to_vec_pretty(&report).unwrap();
    assert!(bytes.len() <= 512 * 1024);
    paths::write_new(&directory.join("launch-analysis-observation.json"), &bytes);
}
