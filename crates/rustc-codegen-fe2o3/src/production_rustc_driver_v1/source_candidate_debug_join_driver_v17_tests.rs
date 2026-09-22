//! Exactly two fresh rustc sessions in one fixed-environment child.
//! Both sessions must reach their actual callbacks. Never substitute
//! synthetic evidence or accept an early rustc failure as a stale-map refusal.

use super::*;
use crate::production_pipeline::source_candidate_debug_join_v17_tests::CandidateDebugEvidenceV17;

const JOIN_OUTPUT: &str = "FE2O3_TEST_SOURCE_CANDIDATE_DEBUG_JOIN_OUTPUT";
const JOIN_CHILD: &str = "production_rustc_driver_v1::source_bitselect_feasibility_v1_tests::roundtrip::machine::debug_join::actual_source_candidate_debug_join_child";
const JOIN_PREFIX: &str = "FE2O3_SOURCE_CANDIDATE_DEBUG_JOIN ";
const PAIR: [&str; 2] = ["default", "edited"];

struct JoinCallbacks {
    source: Option<RetainedInput>,
    plan: Gfx942OrderedProgramRegistersV1,
    previous: Option<CandidateDebugEvidenceV17>,
    calls: usize,
    result: Option<Result<CandidateDebugEvidenceV17, String>>,
}

impl Callbacks for JoinCallbacks {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        let input = self
            .source
            .take()
            .expect("exactly one move-only source per callback");
        self.result = Some(
            crate::production_rustc_driver_v1::transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )
            .and_then(|transaction| {
                transaction.observe_source_candidate_debug_join(
                    input,
                    self.plan,
                    self.previous.as_ref(),
                )
            }),
        );
        Compilation::Stop
    }
}

fn pair(directory: &Path) -> [RoundtripInvocation; 2] {
    let records = PAIR.map(|selector| derive(directory, selector));
    assert_eq!(
        records[0].crate_binding, records[1].crate_binding,
        "cannot reuse a process environment across different authenticated crate bindings"
    );
    assert_eq!(
        records[0].cargo_observation, records[1].cargo_observation,
        "cannot reuse a process environment across different dependency observations"
    );
    for (record, selector) in records.iter().zip(PAIR) {
        assert_eq!(record.phase, selector);
        assert_eq!(
            record.source_directory,
            paths::relative_case(directory, "positive")
        );
        assert!(record.args.len() <= 512);
        assert!(record.args.iter().map(String::len).sum::<usize>() <= 64 * 1024);
        super::super::super::super::require_canonical_overflow_checks_v1(&record.args).unwrap();
    }
    records
}

fn fixed_child_environment(record: &RoundtripInvocation) {
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

#[test]
#[ignore = "two sequential real rustc sessions; parent controls one fixed environment"]
fn actual_source_candidate_debug_join_child() {
    let directory = PathBuf::from(std::env::var_os(INPUT).expect("prepared output"));
    require_current_source();
    let records = pair(&directory);
    for (record, selector) in records.iter().zip(PAIR) {
        let expected: RoundtripInvocation = serde_json::from_slice(
            &read_bounded(
                &directory
                    .join("positive")
                    .join(format!("debug-{selector}.invocation.json")),
                64 * 1024,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            *record, expected,
            "fresh live source/loader/dependency invocation"
        );
        fixed_child_environment(record);
    }
    let mut previous: Option<CandidateDebugEvidenceV17> = None;
    let mut reports = Vec::with_capacity(2);
    for (record, selector) in records.iter().zip(PAIR) {
        // This loop cardinality is literally two. No retries or reset budget.
        // Re-derive before and after EACH actual session, not just at report time.
        assert_eq!(derive(&directory, selector), *record);
        fixed_child_environment(record);
        let source_path = record.source_directory.join(source_files(selector).0);
        let source = RetainedInput::open(source_path.to_str().unwrap(), true).unwrap();
        let mut callbacks = JoinCallbacks {
            source: Some(source),
            plan: register_plan(selector),
            previous,
            calls: 0,
            result: None,
        };
        // No environment/cwd mutations here, no nested rustc session and no
        // source/compiler owner retained in previous.
        rustc_driver::run_compiler(&record.args, &mut callbacks);
        assert_eq!(
            callbacks.calls, 1,
            "both sessions must reach their actual callback"
        );
        let evidence = callbacks
            .result
            .take()
            .expect("actual callback observation")
            .expect("actual compiler-produced catalog and debugger consumer checks");
        assert_eq!(evidence.report["whole_kernel_simulation"]["runs"], 30);
        assert_eq!(evidence.report["actual_captures"], 1);
        assert_eq!(
            evidence.report["exact_source_identity_mismatch_refusals"],
            if selector == "edited" { 3 } else { 0 },
        );
        assert_eq!(derive(&directory, selector), *record);
        fixed_child_environment(record);
        assert_eq!(hash(&source_path), record.source_sha256);
        reports.push(json!({"invocation":record,"observation":evidence.report.clone()}));
        // Drop prior owned evidence now. Current evidence is retained only so
        // the second callback can submit the genuine old objects to its consumer.
        drop(callbacks);
        previous = Some(evidence);
    }
    assert_eq!(reports.len(), 2);
    assert_eq!(
        pair(&directory),
        records,
        "both source/loader/dependency pairs remain current"
    );
    require_current_source();
    assert!(
        fs::read_dir(directory.join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
    let report = json!({
        "kind":"private_actual_source_candidate_debug_join",
        "observations":reports,
        "actual_rustc_callbacks":2,"fixed_environment_sequential_sessions":2,
        "positive_whole_kernel_oracle_runs":60,"actual_debug_capture_runs":2,
        "positive_execution_total":62,"total_step_limit":8_500_000,
        "exact_source_identity_mismatch_refusals":3,
        "old_evidence_used_only_as_negative_input":true,
        "public_bundle_created":false,"portable_capture_import":false,
        "production_resume":false,"hardware_observed":false,
        "grants_artifact_or_launch_authority":false,
    });
    let bytes = serde_json::to_vec(&report).unwrap();
    assert!(bytes.len() <= 64 * 1024);
    println!("\n{JOIN_PREFIX}{}", std::str::from_utf8(&bytes).unwrap());
}

pub(super) fn run_pair_child(directory: &Path) -> Value {
    let records = pair(directory);
    for (record, selector) in records.iter().zip(PAIR) {
        let bytes = serde_json::to_vec_pretty(record).unwrap();
        assert!(bytes.len() <= 64 * 1024);
        paths::write_new(
            &directory
                .join("positive")
                .join(format!("debug-{selector}.invocation.json")),
            &bytes,
        );
    }
    let mut child = Command::new(std::env::current_exe().unwrap());
    // Existing supervisor enforces a process-group deadline and combined stream
    // cap. Its partial-stream failure limitation is documented in the handoff.
    // The NEW profile is still exactly one child, not a server.
    let bytes = checked(
        sanitized(&mut child)
            .current_dir(repository())
            .args([
                "--exact",
                JOIN_CHILD,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(INPUT, directory)
            .env(CRATE_BINDING_ID_ENV_V1, &records[0].crate_binding)
            .env(
                CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
                &records[0].cargo_observation,
            )
            .env("CARGO_MANIFEST_DIR", fixture())
            .env("CARGO_PKG_NAME", PACKAGE)
            .env("CARGO_PKG_VERSION", "0.1.0")
            .env("CARGO_CRATE_NAME", CRATE_NAME),
        directory,
        "actual-source-debug-join-pair",
        None,
    );
    let stdout = std::str::from_utf8(&bytes).unwrap();
    assert_eq!(
        stdout
            .lines()
            .filter(|line| line.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;"))
            .count(),
        1
    );
    let mut observations = stdout
        .lines()
        .filter_map(|line| line.strip_prefix(JOIN_PREFIX));
    let line = observations
        .next()
        .expect("single genuine pair observation");
    assert!(line.len() <= 64 * 1024 && observations.next().is_none());
    let report: Value = serde_json::from_str(line).unwrap();
    assert_eq!(report["kind"], "private_actual_source_candidate_debug_join");
    for (index, record) in records.iter().enumerate() {
        assert_eq!(
            report["observations"][index]["invocation"],
            serde_json::to_value(record).unwrap()
        );
    }
    assert_eq!(pair(directory), records);
    report
}

#[test]
#[ignore = "fresh source publication plus fixed-environment real dual-session qualification"]
fn actual_source_candidate_debug_join_ladder() {
    let directory = PathBuf::from(std::env::var_os(JOIN_OUTPUT).expect("fresh absolute output"));
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
    let baseline = super::super::run_child(&directory, "positive", "baseline");
    assert_eq!(
        baseline["observation"]["baseline"]["kernel_ir_version"],
        "V8"
    );
    let candidate_sha = hash(&case.join("candidate.rs"));
    // Reuses existing move-only input, bounded edit and create-new publication.
    // It also writes the two UNUSED existing wrong/stale fixtures. Those are
    // charged in the same ten-file footprint, not counted as executed negatives.
    let edits = fixture_cases::run_edits(&directory);
    let edited_sha = hash(&case.join("edited.rs"));
    assert_ne!(candidate_sha, edited_sha);
    let joined = run_pair_child(&directory);
    assert_eq!(joined["actual_rustc_callbacks"], 2);
    assert_eq!(joined["exact_source_identity_mismatch_refusals"], 3);
    assert_eq!(hash(&case.join("original.rs")), original_sha);
    assert_eq!(hash(&case.join("candidate.rs")), candidate_sha);
    assert_eq!(hash(&case.join("edited.rs")), edited_sha);
    require_current_source();
    let (source_files, source_bytes) = fixture_cases::footprint(&directory);
    let report = json!({
        "kind":"private_actual_source_candidate_debug_join_ladder",
        "baseline":baseline,"edit_publications":edits,"joined":joined,
        "actual_frontend_callbacks_total":3,"baseline_owner_version":"V8",
        "fresh_diagnostic_owner_version":"V17",
        "source_files":source_files,"source_bytes":source_bytes,
        "source_file_limit":10,"source_byte_limit":10*128*1024,
        "source_directory":paths::relative_root(&directory),
        "original_sha256":original_sha,"candidate_sha256":candidate_sha,
        "edited_sha256":edited_sha,"new_wire_schema":false,
        "public_v17_source_map_support":false,"llvm_or_native_emission":false,
        "production_resume":false,"hardware_observed":false,
        "grants_artifact_or_launch_authority":false,
    });
    let bytes = serde_json::to_vec_pretty(&report).unwrap();
    assert!(bytes.len() <= 512 * 1024);
    paths::write_new(&directory.join("debug-join-observation.json"), &bytes);
}
