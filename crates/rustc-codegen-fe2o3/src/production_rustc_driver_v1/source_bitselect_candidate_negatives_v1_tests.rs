//! Draft nested test leaf: fresh generated-candidate resource/boundary refusals.
//! Install only under machine::fixture_cases::generated_negatives after review.
//! No baseline report/map/capture is consumed as a live compiler owner.
use super::*;

const NEGATIVE_OUTPUT: &str = "FE2O3_TEST_SOURCE_CANDIDATE_NEGATIVES_OUTPUT";
const NEGATIVE_INPUT: &str = "FE2O3_TEST_SOURCE_CANDIDATE_NEGATIVES_INPUT";
const NEGATIVE_CASE: &str = "FE2O3_TEST_SOURCE_CANDIDATE_NEGATIVES_CASE";
const NEGATIVE_CHILD: &str = "production_rustc_driver_v1::source_bitselect_feasibility_v1_tests::roundtrip::machine::fixture_cases::generated_negatives::actual_source_candidate_negative_child";
const PUBLICATION_CHILD: &str = "production_rustc_driver_v1::source_bitselect_feasibility_v1_tests::roundtrip::machine::fixture_cases::generated_negatives::source_candidate_negative_publication_child";
const NEGATIVE_PREFIX: &str = "FE2O3_SOURCE_CANDIDATE_NEGATIVE ";
const PUBLICATION_PREFIX: &str = "FE2O3_SOURCE_CANDIDATE_NEGATIVE_PUBLICATION ";
const PHYSICAL_REFUSAL: &str = "ordered-program source stages: production compilation semantic importer rejected semantic body construction: semantic body construction rejected inconsistent ordered program physical roles must be distinct v0..v63";
const BOUNDARY_REFUSAL: &str = "source-candidate fresh program/role binding differs";

#[derive(Clone, Copy)]
struct NegativeCase {
    name: &'static str,
    before: &'static str,
    after: &'static str,
    expected: &'static str,
    obligation: &'static str,
}

// All edits preserve valid Rust syntax and legal macro descriptors. A literal
// 64 still fits the marker's u8 argument: the backend, not rustc overflow/const
// evaluation, must reject the exact physical-profile resource bound.
const NEGATIVES: [NegativeCase; 4] = [
    NegativeCase {
        name: "resource-v64",
        before: "scratch(4); out(5);",
        after: "scratch(64); out(5);",
        expected: PHYSICAL_REFUSAL,
        obligation: "physical_register_profile_v0_through_v63",
    },
    NegativeCase {
        name: "hidden-input-clobber",
        before: "scratch(4); out(5);",
        after: "scratch(0); out(5);",
        expected: PHYSICAL_REFUSAL,
        obligation: "scratch_must_not_alias_read_only_input0_v0",
    },
    NegativeCase {
        name: "live-in-boundary",
        before: "in(0) = a;",
        after: "in(0) = b;",
        expected: BOUNDARY_REFUSAL,
        obligation: "actual_live_ins_must_match_fresh_source_parameter_roles",
    },
    NegativeCase {
        name: "instruction-boundary",
        before: "xor(out, input1, scratch);",
        after: "xor(out, input0, scratch);",
        expected: BOUNDARY_REFUSAL,
        obligation: "fresh_program_must_match_checked_bitselect_replacement",
    },
];
const NEGATIVE_SOURCE_FILES: usize = 4 + 2 * NEGATIVES.len();
const NEGATIVE_SOURCE_BYTES: u64 = NEGATIVE_SOURCE_FILES as u64 * BYTE_CAP as u64;

fn selected(name: &str) -> NegativeCase {
    *NEGATIVES
        .iter()
        .find(|case| case.name == name)
        .expect("closed generated-candidate negative selector")
}

fn parse_report(bytes: &[u8], prefix: &str) -> Value {
    let text = std::str::from_utf8(bytes).unwrap();
    assert_eq!(
        text.lines()
            .filter(|line| line.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;"))
            .count(),
        1
    );
    let mut lines = text.lines().filter_map(|line| line.strip_prefix(prefix));
    let line = lines.next().expect("one exact successful child report");
    assert!(line.len() <= 64 * 1024 && lines.next().is_none());
    serde_json::from_str(line).unwrap()
}

#[test]
#[ignore = "bounded create-new publication child; run the generated-negative ladder"]
fn source_candidate_negative_publication_child() {
    let directory = PathBuf::from(std::env::var_os(NEGATIVE_INPUT).expect("prepared output"));
    assert_eq!(std::env::current_dir().unwrap(), repository());
    require_current_source();
    let relative = paths::relative_case(&directory, "positive");
    let absolute = paths::absolute_case(&directory, "positive");
    let original_sha = hash(&absolute.join("original.rs"));
    let candidate_sha = hash(&absolute.join("candidate.rs"));
    let mut meter = EditMeter::default();
    let mut publications = Vec::with_capacity(NEGATIVES.len());
    for case in NEGATIVES {
        let source = format!("{}.rs", case.name);
        let publication = publish_variant(
            &relative,
            "candidate.rs",
            &source,
            &mut meter,
            |bytes, meter| replace_exact(bytes, case.before, case.after, meter),
        )
        .unwrap();
        let loader =
            format!("#![no_std]\n#[path = \"{source}\"] mod source_bitselect_feasibility;\n");
        assert!(loader.len() <= 256);
        paths::write_new(
            &absolute.join(format!("{}-loader.rs", case.name)),
            loader.as_bytes(),
        );
        publications.push(json!({"selector":case.name,"publication":publication}));
    }
    assert_eq!(hash(&absolute.join("original.rs")), original_sha);
    assert_eq!(hash(&absolute.join("candidate.rs")), candidate_sha);
    require_current_source();
    let bytes = serde_json::to_vec(&json!({
        "publications":publications,"edit_accounting":meter,
        "edit_work_limit":32*1024*1024,"edit_payload_limit":1024*1024,
        "original_and_generated_candidate_unchanged":true,
        "source_admission_reused":false,
    }))
    .unwrap();
    assert!(bytes.len() <= 64 * 1024);
    println!(
        "\n{PUBLICATION_PREFIX}{}",
        std::str::from_utf8(&bytes).unwrap()
    );
}

fn derive_negative(directory: &Path, name: &str) -> RoundtripInvocation {
    let case = selected(name);
    let folder = paths::absolute_case(directory, "positive");
    let source = folder.join(format!("{}.rs", case.name));
    let loader = folder.join(format!("{}-loader.rs", case.name));
    paths::checked_source_file(&source);
    paths::checked_source_file(&loader);
    let (args, crate_binding, cargo_observation) = invocation_for_fixture_source(
        directory,
        &fixture(),
        PACKAGE,
        CRATE_NAME,
        Some(FEATURES[0]),
        &loader,
        "gfx942",
    );
    RoundtripInvocation {
        case: "positive".into(),
        phase: case.name.into(),
        source_directory: paths::relative_case(directory, "positive"),
        args,
        crate_binding,
        cargo_observation,
        source_sha256: hash(&source),
        loader_sha256: hash(&loader),
        fixture_sha256: FIXTURE_FILES.map(|(path, _)| hash(&fixture().join(path))),
        artifacts_sha256: hash(&directory.join("dependencies.stdout")),
        metadata_sha256: hash(&directory.join("metadata.stdout")),
    }
}

#[derive(Default)]
struct NegativeCallbacks {
    source: Option<RetainedInput>,
    calls: usize,
    fresh_method_entries: usize,
    result: Option<Result<(Value, String), String>>,
}

impl Callbacks for NegativeCallbacks {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        let input = self
            .source
            .take()
            .expect("one move-only retained edited source");
        let transaction = super::super::super::super::super::transaction_in_active_session_v1(
            tcx,
            crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
        )
        .expect("negative must reach a genuine collected-source transaction");
        self.fresh_method_entries += 1;
        // Keep the expected valid original replacement plan. Never construct an
        // invalid owner or change an expectation to match malformed source.
        self.result = Some(
            transaction.observe_source_bitselect_candidate_machine(input, register_plan("default")),
        );
        Compilation::Stop
    }
}

#[test]
#[ignore = "actual rustc callback; run the generated-negative ladder"]
fn actual_source_candidate_negative_child() {
    let directory = PathBuf::from(std::env::var_os(NEGATIVE_INPUT).expect("prepared output"));
    let name = std::env::var(NEGATIVE_CASE).expect("closed negative case");
    let case = selected(&name);
    require_current_source();
    let actual = derive_negative(&directory, &name);
    let retained: RoundtripInvocation = serde_json::from_slice(
        &read_bounded(
            &directory
                .join("positive")
                .join(format!("{name}.negative.invocation.json")),
            64 * 1024,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        actual, retained,
        "current exact source/loader/dependency invocation"
    );
    assert_eq!(std::env::current_dir().unwrap(), repository());
    assert_eq!(
        std::env::var(CRATE_BINDING_ID_ENV_V1).unwrap(),
        actual.crate_binding
    );
    assert_eq!(
        std::env::var(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2).unwrap(),
        actual.cargo_observation
    );
    assert_eq!(
        std::env::var("CARGO_MANIFEST_DIR").unwrap(),
        fixture().to_str().unwrap()
    );
    assert_eq!(std::env::var("CARGO_PKG_NAME").unwrap(), PACKAGE);
    assert_eq!(std::env::var("CARGO_PKG_VERSION").unwrap(), "0.1.0");
    assert_eq!(std::env::var("CARGO_CRATE_NAME").unwrap(), CRATE_NAME);
    super::super::super::super::super::require_canonical_overflow_checks_v1(&actual.args).unwrap();
    let source_path = actual.source_directory.join(format!("{name}.rs"));
    let mut callbacks = NegativeCallbacks {
        source: Some(RetainedInput::open(source_path.to_str().unwrap(), true).unwrap()),
        ..Default::default()
    };
    let output = directory.join("positive").join(format!("{name}.ll"));
    assert!(!output.exists(), "no preexisting LLVM for a negative");
    rustc_driver::run_compiler(&actual.args, &mut callbacks);
    assert_eq!(callbacks.calls, 1, "no arbitrary rustc failure accepted");
    assert_eq!(
        callbacks.fresh_method_entries, 1,
        "must enter the existing fresh path"
    );
    let diagnostic = callbacks
        .result
        .expect("actual fresh-candidate result")
        .expect_err("specific existing resource/boundary refusal");
    assert_eq!(
        diagnostic, case.expected,
        "exact intended refusal, not arbitrary error"
    );
    assert_eq!(hash(&source_path), actual.source_sha256);
    assert!(!output.exists());
    require_current_source();
    assert!(
        fs::read_dir(directory.join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
    let bytes = serde_json::to_vec(&json!({
        "invocation":actual,"selector":case.name,"obligation":case.obligation,
        "diagnostic":diagnostic,"actual_rustc_callbacks":callbacks.calls,
        "genuine_collected_transaction":true,"fresh_method_entries":callbacks.fresh_method_entries,
        "candidate_rejected":true,"llvm_written":false,"old_map_or_capture_imported":false,
        "source_map_available":false,"production_resume":false,"hardware_observed":false,
        "grants_artifact_or_launch_authority":false,
    }))
    .unwrap();
    assert!(bytes.len() <= 64 * 1024);
    println!(
        "\n{NEGATIVE_PREFIX}{}",
        std::str::from_utf8(&bytes).unwrap()
    );
}

fn run_negative(directory: &Path, name: &str) -> Value {
    let record = derive_negative(directory, name);
    paths::write_new(
        &directory
            .join("positive")
            .join(format!("{name}.negative.invocation.json")),
        &serde_json::to_vec_pretty(&record).unwrap(),
    );
    let mut child = Command::new(std::env::current_exe().unwrap());
    let bytes = checked(
        sanitized(&mut child)
            .current_dir(repository())
            .args(["--exact", NEGATIVE_CHILD, "--ignored", "--nocapture"])
            .env(NEGATIVE_INPUT, directory)
            .env(NEGATIVE_CASE, name)
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
        &format!("candidate-negative-{name}"),
        None,
    );
    let report = parse_report(&bytes, NEGATIVE_PREFIX);
    assert_eq!(report["invocation"], serde_json::to_value(record).unwrap());
    assert_eq!(report["selector"], name);
    assert_eq!(report["diagnostic"], selected(name).expected);
    assert_eq!(report["actual_rustc_callbacks"], 1);
    assert_eq!(report["fresh_method_entries"], 1);
    report
}

fn source_footprint(directory: &Path) -> (usize, u64) {
    let source_root = repository().join(paths::relative_root(directory));
    let mut roots = fs::read_dir(source_root).unwrap();
    assert_eq!(
        roots.next().unwrap().unwrap().file_name().to_str().unwrap(),
        "positive"
    );
    assert!(roots.next().is_none());
    let mut allowed = vec![
        "original.rs".to_owned(),
        "original-loader.rs".to_owned(),
        "candidate.rs".to_owned(),
        "candidate-loader.rs".to_owned(),
    ];
    for case in NEGATIVES {
        allowed.push(format!("{}.rs", case.name));
        allowed.push(format!("{}-loader.rs", case.name));
    }
    assert_eq!(allowed.len(), NEGATIVE_SOURCE_FILES);
    let mut count = 0usize;
    let mut bytes = 0u64;
    for entry in fs::read_dir(paths::absolute_case(directory, "positive")).unwrap() {
        count += 1;
        assert!(count <= NEGATIVE_SOURCE_FILES);
        let entry = entry.unwrap();
        assert!(
            allowed
                .iter()
                .any(|name| name == entry.file_name().to_str().unwrap())
        );
        paths::checked_source_file(&entry.path());
        bytes = bytes
            .checked_add(fs::symlink_metadata(entry.path()).unwrap().len())
            .unwrap();
        assert!(bytes <= NEGATIVE_SOURCE_BYTES);
    }
    assert_eq!(count, NEGATIVE_SOURCE_FILES);
    (count, bytes)
}

#[test]
#[ignore = "fresh generated candidates; serialized Cargo; resource-guarded root runner"]
fn actual_source_candidate_negative_ladder() {
    let directory = PathBuf::from(std::env::var_os(NEGATIVE_OUTPUT).expect("fresh task output"));
    assert!(directory.is_absolute());
    fs::create_dir(&directory).unwrap();
    let directory = directory.canonicalize().unwrap();
    let source_root = paths::create_root(&directory);
    require_current_source();
    super::super::super::prepare(&directory);
    fs::create_dir(directory.join("positive")).unwrap();
    let case = source_root.join("positive");
    fs::create_dir(&case).unwrap();
    paths::write_new(&case.join("original.rs"), FIXTURE_FILES[2].1);
    paths::write_new(&case.join("original-loader.rs"), ORIGINAL_LOADER);
    paths::write_new(&case.join("candidate-loader.rs"), CANDIDATE_LOADER);
    let original_sha = hash(&case.join("original.rs"));
    let baseline = super::super::super::run_child(&directory, "positive", "baseline");
    assert_eq!(
        baseline["observation"]["baseline"]["kernel_ir_version"],
        "V8"
    );
    let candidate_sha = hash(&case.join("candidate.rs"));
    let positive = super::super::run_machine_child(&directory, "default");
    assert_eq!(
        baseline["observation"]["candidate_sha256"],
        positive["observation"]["fresh"]["candidate_sha256"]
    );
    assert_eq!(
        positive["observation"]["fresh"]["source_map_available"],
        false
    );
    assert_eq!(
        positive["observation"]["whole_kernel_simulation"]["runs"],
        30
    );
    let mut command = Command::new(std::env::current_exe().unwrap());
    let publications = parse_report(
        &checked(
            sanitized(&mut command)
                .current_dir(repository())
                .args(["--exact", PUBLICATION_CHILD, "--ignored", "--nocapture"])
                .env(NEGATIVE_INPUT, &directory),
            &directory,
            "candidate-negative-publications",
            None,
        ),
        PUBLICATION_PREFIX,
    );
    let observations: Vec<_> = NEGATIVES
        .iter()
        .map(|case| run_negative(&directory, case.name))
        .collect();
    assert_eq!(hash(&case.join("original.rs")), original_sha);
    assert_eq!(hash(&case.join("candidate.rs")), candidate_sha);
    require_current_source();
    let (source_files, source_bytes) = source_footprint(&directory);
    let report = serde_json::to_vec_pretty(&json!({
        "kind":"private_generated_candidate_negative_observation_v1",
        "baseline":baseline,"positive":positive,"publications":publications,
        "observations":observations,"actual_callbacks":6,"exact_refusals":4,
        "positive_whole_kernel_simulation_runs":30,
        "source_directory":paths::relative_root(&directory),
        "source_files":source_files,"source_bytes":source_bytes,
        "source_file_limit":NEGATIVE_SOURCE_FILES,"source_byte_limit":NEGATIVE_SOURCE_BYTES,
        "original_sha256":original_sha,"generated_candidate_sha256":candidate_sha,
        "stale_map_capture_case":"pending_no_generated_candidate_map_import_owner",
        "old_map_or_capture_imported":false,"production_resume":false,
        "functional_proof":false,"native_inspected":false,"hardware_observed":false,
        "grants_artifact_or_launch_authority":false,
    }))
    .unwrap();
    assert!(report.len() <= 512 * 1024);
    paths::write_new(&directory.join("negative-observation.json"), &report);
}

#[test]
fn source_candidate_negative_edits_are_single_bounded_source_mutations() {
    // Synthetic edit mechanics only, never an actual callback or compiler refusal.
    let baseline = format!("prefix\n{LOW}\n    xor(out, input1, scratch);\nsuffix");
    for case in NEGATIVES {
        let mut meter = EditMeter::default();
        let changed =
            replace_exact(baseline.as_bytes(), case.before, case.after, &mut meter).unwrap();
        assert_ne!(changed, baseline.as_bytes());
        assert_eq!(
            std::str::from_utf8(&changed)
                .unwrap()
                .matches(case.after)
                .count(),
            1
        );
        assert!(replace_exact(b"unrelated", case.before, case.after, &mut meter).is_err());
        let ambiguous = format!("{}\n{}", case.before, case.before);
        assert!(replace_exact(ambiguous.as_bytes(), case.before, case.after, &mut meter).is_err());
    }
    assert_eq!(NEGATIVE_SOURCE_FILES, 12);
    assert!(NEGATIVE_SOURCE_BYTES <= paths::MAX_SOURCE_BYTES);
}
