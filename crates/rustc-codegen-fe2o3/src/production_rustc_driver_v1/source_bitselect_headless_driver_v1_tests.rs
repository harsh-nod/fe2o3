//! Child of the existing roundtrip test harness. Uses its real fixture/provider
//! preparation and source directory policy; this is not a mock compiler test.
use super::*;
use crate::production_rustc_driver_v1::source_bitselect_promotion_driver_v1::run_bitselect_source_promotion_driver_v1;
use crate::source_bitselect_promotion_v1::{
    BitselectPromotionRequestV1, CandidatePublicationStateV1, FailurePhaseV1,
};
use fe2o3_kernel_ir::Gfx942OrderedProgramRegistersV1;
use sha2::Sha256;

const HEADLESS_CHILD: &str = "production_rustc_driver_v1::source_bitselect_feasibility_v1_tests::roundtrip::headless::actual_source_bitselect_headless_child";
const HEADLESS_PREFIX: &str = "FE2O3_SOURCE_BITSELECT_HEADLESS ";
const HEADLESS_CASES: [&str; 5] = [
    "positive",
    "wrong-target",
    "wrong-launch",
    "stale-original",
    "existing-candidate",
];

const SELECTED_LAUNCH: &str = "#[cfg(feature = \"source-bitselect-feasibility\")]\n#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]\npub fn choose_bits(";
const WRONG_SELECTED_LAUNCH: &str = "#[cfg(feature = \"source-bitselect-feasibility\")]\n#[kernel(typed, launch(required = [128, 1, 1], max = [128, 1, 1]))]\npub fn choose_bits(";

fn wrong_launch_source(original: &str) -> String {
    // Other feature-gated kernels share these launch bounds. Mutate only the
    // selected entry's complete attribute/signature prefix.
    assert_eq!(original.matches(SELECTED_LAUNCH).count(), 1);
    original.replacen(SELECTED_LAUNCH, WRONG_SELECTED_LAUNCH, 1)
}

#[test]
fn source_candidate_headless_wrong_launch_preserves_sibling_kernels() {
    let original = std::str::from_utf8(FIXTURE_FILES[2].1).unwrap();
    let changed = wrong_launch_source(original);
    let (before, after) = original.split_once(SELECTED_LAUNCH).unwrap();
    let (changed_before, changed_after) = changed.split_once(WRONG_SELECTED_LAUNCH).unwrap();
    assert_eq!(before, changed_before);
    assert_eq!(after, changed_after);
    assert_eq!(changed.matches(WRONG_SELECTED_LAUNCH).count(), 1);
    assert!(!changed.contains(SELECTED_LAUNCH));
    for sibling in ["ambiguous_bits", "alias_bits"] {
        let prefix = format!(
            "#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]\npub fn {sibling}("
        );
        assert!(changed.contains(&prefix));
    }
    assert!(std::panic::catch_unwind(|| wrong_launch_source("")).is_err());
    let duplicated = format!("{original}\n{original}");
    assert!(std::panic::catch_unwind(|| wrong_launch_source(&duplicated)).is_err());
}

fn record(directory: &Path, case: &str) -> RoundtripInvocation {
    derive_roundtrip(directory, case, "baseline")
}

#[test]
#[ignore = "actual isolated rustc child; use headless ladder"]
fn actual_source_bitselect_headless_child() {
    let directory =
        PathBuf::from(std::env::var_os(ROUNDTRIP_INPUT).expect("preparation directory"));
    let case = std::env::var(ROUNDTRIP_CASE).expect("case");
    assert!(HEADLESS_CASES.contains(&case.as_str()));
    require_current_source();
    let actual = record(&directory, &case);
    let saved: RoundtripInvocation = serde_json::from_slice(
        &read_bounded(
            &directory.join(&case).join("headless.invocation.json"),
            64 * 1024,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(actual, saved);
    assert_eq!(
        std::env::current_dir().unwrap(),
        repository().canonicalize().unwrap()
    );
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
    let relative = paths::relative_case(&directory, &case);
    let expected: [u8; 32] = read_bounded(&directory.join(&case).join("expected-revision.bin"), 32)
        .unwrap()
        .try_into()
        .unwrap();
    let request = BitselectPromotionRequestV1::new(
        relative.join("original.rs").to_str().unwrap(),
        relative.join("candidate.rs").to_str().unwrap(),
        expected,
        Gfx942OrderedProgramRegistersV1::new(4, 5, [0, 1, 2]).unwrap(),
    )
    .unwrap();
    let attempt = run_bitselect_source_promotion_driver_v1(&actual.args, request);
    let observation = match attempt.result() {
        Ok(published) => {
            assert_eq!(case, "positive");
            assert_eq!(published.original_sha256(), &expected);
            let bytes = read_bounded(&relative.join("candidate.rs"), 128 * 1024).unwrap();
            assert_eq!(published.candidate_bytes(), bytes.len());
            assert_eq!(
                published.candidate_sha256(),
                &<[u8; 32]>::from(Sha256::digest(&bytes))
            );
            json!({"published":true,"candidate_sha256":published.candidate_sha256(),"candidate_bytes":published.candidate_bytes()})
        }
        Err(error) => {
            assert!(
                !error.compiler_fatal(),
                "unrelated compiler fatal is not a profile refusal"
            );
            if case == "existing-candidate" {
                assert_eq!(error.phase(), FailurePhaseV1::Publication);
                assert_eq!(
                    error.publication(),
                    CandidatePublicationStateV1::MayHaveCreatedCandidate
                );
                assert!(error.diagnostic().starts_with(
                    "candidate publication failed without replacing an existing entry: "
                ));
                assert_eq!(
                    read_bounded(&relative.join("candidate.rs"), 64 * 1024).unwrap(),
                    EXISTING
                );
            } else {
                assert_eq!(error.phase(), FailurePhaseV1::Eligibility);
                assert_eq!(
                    error.publication(),
                    CandidatePublicationStateV1::NotAttempted
                );
                let expected = match case.as_str() {
                    "wrong-target" => {
                        "source-candidate requires authenticated gfx942 xnack-off wave64"
                    }
                    "wrong-launch" => {
                        "source-candidate requires required and maximum 64x1x1 bounds"
                    }
                    "stale-original" => "source promotion expected source revision differs",
                    _ => panic!(
                        "unexpected successful-profile refusal: {}",
                        error.diagnostic()
                    ),
                };
                assert_eq!(error.diagnostic(), expected);
                assert!(!relative.join("candidate.rs").exists());
            }
            json!({"published":false,"diagnostic":error.diagnostic()})
        }
    };
    assert_eq!(hash(&relative.join("original.rs")), actual.source_sha256);
    assert!(
        fs::read_dir(directory.join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
    require_current_source();
    let bytes = serde_json::to_vec(&json!({"invocation":actual,"observation":observation,
        "actual_rustc_callback":true,"fresh_frontend_admitted":false,"production_resume":false}))
    .unwrap();
    assert!(bytes.len() <= 64 * 1024);
    println!(
        "\n{HEADLESS_PREFIX}{}",
        std::str::from_utf8(&bytes).unwrap()
    );
}

fn run_headless_child(directory: &Path, case: &str) -> Value {
    let actual = record(directory, case);
    paths::write_new(
        &directory.join(case).join("headless.invocation.json"),
        &serde_json::to_vec_pretty(&actual).unwrap(),
    );
    let mut child = Command::new(std::env::current_exe().unwrap());
    let stdout = checked(
        sanitized(&mut child)
            .current_dir(repository())
            .args(["--exact", HEADLESS_CHILD, "--ignored", "--nocapture"])
            .env(ROUNDTRIP_INPUT, directory)
            .env(ROUNDTRIP_CASE, case)
            .env(CRATE_BINDING_ID_ENV_V1, &actual.crate_binding)
            .env(
                CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
                &actual.cargo_observation,
            )
            .env("CARGO_MANIFEST_DIR", fixture())
            .env("CARGO_PKG_NAME", PACKAGE)
            .env("CARGO_PKG_VERSION", "0.1.0")
            .env("CARGO_CRATE_NAME", CRATE_NAME),
        directory,
        &format!("headless-{case}"),
        None,
    );
    let text = std::str::from_utf8(&stdout).unwrap();
    let rows: Vec<_> = text
        .lines()
        .filter_map(|line| line.strip_prefix(HEADLESS_PREFIX))
        .collect();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].len() <= 64 * 1024);
    assert_eq!(
        text.lines()
            .filter(|line| line.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;"))
            .count(),
        1
    );
    let value: Value = serde_json::from_str(rows[0]).unwrap();
    assert_eq!(value["invocation"], serde_json::to_value(actual).unwrap());
    value
}

#[test]
#[ignore = "owner-approved integration only; pinned actual callbacks; fresh output directory"]
fn actual_source_bitselect_headless_ladder() {
    let directory =
        PathBuf::from(std::env::var_os(ROUNDTRIP_OUTPUT).expect("fresh output directory"));
    assert!(directory.is_absolute());
    fs::create_dir(&directory).unwrap();
    let directory = directory.canonicalize().unwrap();
    let source_root = paths::create_root(&directory);
    require_current_source();
    prepare(&directory);
    let mut observations = Vec::with_capacity(5);
    for case in HEADLESS_CASES {
        fs::create_dir(directory.join(case)).unwrap();
        let source = source_root.join(case);
        fs::create_dir(&source).unwrap();
        let mut original = String::from_utf8(FIXTURE_FILES[2].1.to_vec()).unwrap();
        if case == "wrong-launch" {
            original = wrong_launch_source(&original);
        }
        let expected: [u8; 32] = Sha256::digest(original.as_bytes()).into();
        if case == "stale-original" {
            // Genuine source revision changed before the new compiler session;
            // the retained request commitment intentionally remains older.
            original.push_str("\n// changed after the user's selection\n");
        }
        assert!(original.len() <= 64 * 1024);
        paths::write_new(&source.join("original.rs"), original.as_bytes());
        paths::write_new(&source.join("original-loader.rs"), ORIGINAL_LOADER);
        paths::write_new(&source.join("candidate-loader.rs"), CANDIDATE_LOADER);
        paths::write_new(
            &directory.join(case).join("expected-revision.bin"),
            &expected,
        );
        if case == "existing-candidate" {
            paths::write_new(&source.join("candidate.rs"), EXISTING);
        }
        observations.push(run_headless_child(&directory, case));
    }
    // The unchanged existing fresh-admission child reparses the actual new file.
    // Its V17 owner and 128-vector oracle are not reconstructed from our receipt.
    let fresh = run_child(&directory, "positive", "fresh");
    assert_eq!(fresh["observation"]["fresh_frontend_admitted"], true);
    assert_eq!(fresh["observation"]["boolean_oracle_cases"], 128);
    let report = serde_json::to_vec_pretty(&json!({"headless":observations,"fresh":fresh,
        "actual_frontend_callbacks":6,"public_wire_allocated":false,
        "final_native_qualification":false,"hardware_observed":false,"production_resume":false}))
    .unwrap();
    assert!(report.len() <= 256 * 1024);
    paths::write_new(&directory.join("headless-observation.json"), &report);
    require_current_source();
}

// Separate from the existing six-callback and external-edit ladders.
mod live_failures {
    use super::*;
    use crate::production_rustc_driver_v1::source_bitselect_promotion_driver_v1::{
        LIVE_FATAL_DIAGNOSTIC, run_live_callback_probe_v1,
    };
    use crate::source_bitselect_promotion_v1::live_test_support::AfterPublication;
    use std::os::unix::fs::MetadataExt as _;
    use std::sync::{Arc, Mutex};

    const OUTPUT: &str = "FE2O3_TEST_SOURCE_HEADLESS_LIVE_OUTPUT";
    const MODE: &str = "FE2O3_TEST_SOURCE_HEADLESS_LIVE_MODE";
    const CHILD: &str = "production_rustc_driver_v1::source_bitselect_feasibility_v1_tests::roundtrip::headless::live_failures::actual_source_bitselect_live_failure_child";
    const PREFIX: &str = "FE2O3_SOURCE_BITSELECT_LIVE_FAILURE ";
    const CASES: [(&str, &str); 3] = [
        ("positive", "repeat-success"),
        ("existing-candidate", "repeat-existing"),
        ("stale-original", "postpublication-original-change"),
    ];
    // A separate ladder keeps the earlier three-session/five-call qualification
    // unchanged. Both cases reuse actual provider-derived baseline invocations.
    const FATAL_OUTPUT: &str = "FE2O3_TEST_SOURCE_HEADLESS_FATAL_OUTPUT";
    const FATAL_CASES: [(&str, &str); 2] = [
        ("positive", "postcallback-fatal-success"),
        ("existing-candidate", "postcallback-fatal-existing"),
    ];

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct FileObservation {
        device: u64,
        inode: u64,
        mode: u32,
        modified: (i64, i64),
        changed: (i64, i64),
        bytes: Vec<u8>,
    }

    impl FileObservation {
        fn read(path: &Path) -> Self {
            let before = fs::symlink_metadata(path).unwrap();
            assert!(before.is_file() && before.len() <= 128 * 1024);
            let identity = |m: &fs::Metadata| {
                (
                    m.dev(),
                    m.ino(),
                    m.mode(),
                    m.len(),
                    m.mtime(),
                    m.mtime_nsec(),
                    m.ctime(),
                    m.ctime_nsec(),
                )
            };
            let mut file = fs::File::open(path).unwrap();
            assert_eq!(identity(&file.metadata().unwrap()), identity(&before));
            let mut bytes = Vec::new();
            std::io::Read::read_to_end(
                &mut std::io::Read::take(&mut file, 128 * 1024 + 1),
                &mut bytes,
            )
            .unwrap();
            assert_eq!(bytes.len() as u64, before.len());
            assert_eq!(identity(&file.metadata().unwrap()), identity(&before));
            let after = fs::symlink_metadata(path).unwrap();
            assert!(after.is_file());
            assert_eq!(identity(&after), identity(&before));
            Self {
                device: before.dev(),
                inode: before.ino(),
                mode: before.mode() & 0o7777,
                modified: (before.mtime(), before.mtime_nsec()),
                changed: (before.ctime(), before.ctime_nsec()),
                bytes,
            }
        }

        fn same_identity(&self, other: &Self) {
            assert_eq!(
                (self.device, self.inode, self.mode),
                (other.device, other.inode, other.mode)
            );
        }

        fn summary(&self) -> Value {
            let sha: [u8; 32] = Sha256::digest(&self.bytes).into();
            json!({"device_u64_decimal":self.device.to_string(),
                "inode_u64_decimal":self.inode.to_string(),"mode":self.mode,
                "bytes":self.bytes.len(),"sha256":sha,
                "mtime_seconds_decimal":self.modified.0.to_string(),"mtime_nanoseconds":self.modified.1,
                "ctime_seconds_decimal":self.changed.0.to_string(),"ctime_nanoseconds":self.changed.1})
        }
    }

    #[derive(Debug, Eq, PartialEq)]
    enum FirstOutcome {
        Published,
        Refused(FailurePhaseV1, CandidatePublicationStateV1, String, bool),
    }

    struct FirstObservation {
        original: FileObservation,
        candidate: FileObservation,
        outcome: FirstOutcome,
    }

    #[test]
    #[ignore = "actual isolated compiler child; use the separate live-failure ladder"]
    fn actual_source_bitselect_live_failure_child() {
        let directory = PathBuf::from(std::env::var_os(ROUNDTRIP_INPUT).unwrap());
        let case = std::env::var(ROUNDTRIP_CASE).unwrap();
        let mode = std::env::var(MODE).unwrap();
        assert!(
            CASES.contains(&(case.as_str(), mode.as_str()))
                || FATAL_CASES.contains(&(case.as_str(), mode.as_str()))
        );
        require_current_source();
        assert_eq!(
            std::env::current_dir().unwrap(),
            repository().canonicalize().unwrap()
        );
        let actual = record(&directory, &case);
        let saved: RoundtripInvocation = serde_json::from_slice(
            &read_bounded(
                &directory.join(&case).join("live.invocation.json"),
                64 * 1024,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(actual, saved);
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

        let relative = paths::relative_case(&directory, &case);
        let original_path = relative.join("original.rs");
        let candidate_path = relative.join("candidate.rs");
        let original_before = FileObservation::read(&original_path);
        assert_eq!(hash(&original_path), actual.source_sha256);
        assert!(!original_before.bytes.is_empty());
        let existing = if matches!(
            mode.as_str(),
            "repeat-existing" | "postcallback-fatal-existing"
        ) {
            let file = FileObservation::read(&candidate_path);
            assert_eq!(file.bytes, EXISTING);
            Some(file)
        } else {
            assert!(!candidate_path.try_exists().unwrap());
            None
        };
        let expected: [u8; 32] = Sha256::digest(&original_before.bytes).into();
        let registers = Gfx942OrderedProgramRegistersV1::new(4, 5, [0, 1, 2]).unwrap();
        let request = BitselectPromotionRequestV1::new(
            original_path.to_str().unwrap(),
            candidate_path.to_str().unwrap(),
            expected,
            registers,
        )
        .unwrap();

        let first = Arc::new(Mutex::new(None::<FirstObservation>));
        let observed_first = Arc::clone(&first);
        let late = Arc::new(Mutex::new(None::<(FileObservation, FileObservation)>));
        let hook: Option<AfterPublication> = if mode == "postpublication-original-change" {
            let before = original_before.clone();
            let observed_late = Arc::clone(&late);
            Some(Box::new(move |request| {
                let original = Path::new(request.original_path());
                assert_eq!(FileObservation::read(original), before);
                let candidate = FileObservation::read(Path::new(request.candidate_path()));
                assert_eq!(candidate.mode, 0o600);
                assert!(!candidate.bytes.is_empty());
                let mut changed = before.bytes.clone();
                changed[0] ^= 1;
                let mut file = fs::OpenOptions::new().write(true).open(original).unwrap();
                let metadata = file.metadata().unwrap();
                assert_eq!(
                    (metadata.dev(), metadata.ino(), metadata.mode() & 0o7777),
                    (before.device, before.inode, before.mode)
                );
                // Exactly one bounded edit of the already-existing owned original;
                // no truncate, create, candidate rewrite, or manufactured failure.
                std::io::Write::write_all(&mut file, &changed).unwrap();
                file.sync_all().unwrap();
                let after = FileObservation::read(original);
                before.same_identity(&after);
                assert_eq!(after.bytes, changed);
                assert_eq!(
                    after
                        .bytes
                        .iter()
                        .zip(&before.bytes)
                        .filter(|(a, b)| a != b)
                        .count(),
                    1
                );
                assert!(
                    observed_late
                        .lock()
                        .unwrap()
                        .replace((candidate, after))
                        .is_none()
                );
            }))
        } else {
            None
        };
        let repeat = matches!(mode.as_str(), "repeat-success" | "repeat-existing");
        let fatal_after_first = matches!(
            mode.as_str(),
            "postcallback-fatal-success" | "postcallback-fatal-existing"
        );
        let (attempt, compiler_entries, method_calls) = run_live_callback_probe_v1(
            &actual.args,
            request,
            repeat,
            fatal_after_first,
            hook,
            move |request, result| {
                let original = FileObservation::read(Path::new(request.original_path()));
                let candidate = FileObservation::read(Path::new(request.candidate_path()));
                let outcome = match result {
                    Ok(published) => {
                        assert_eq!(
                            published.original_sha256(),
                            request.expected_original_sha256()
                        );
                        assert_eq!(published.registers(), request.registers());
                        assert_eq!(published.candidate_bytes(), candidate.bytes.len());
                        let digest: [u8; 32] = Sha256::digest(&candidate.bytes).into();
                        assert_eq!(published.candidate_sha256(), &digest);
                        FirstOutcome::Published
                    }
                    Err(error) => FirstOutcome::Refused(
                        error.phase(),
                        error.publication(),
                        error.diagnostic().to_owned(),
                        error.compiler_fatal(),
                    ),
                };
                assert!(
                    observed_first
                        .lock()
                        .unwrap()
                        .replace(FirstObservation {
                            original,
                            candidate,
                            outcome,
                        })
                        .is_none()
                );
            },
        );
        assert_eq!(compiler_entries, 1);
        assert_eq!(method_calls, if repeat { 2 } else { 1 });
        assert_eq!(
            attempt.request().original_path(),
            original_path.to_str().unwrap()
        );
        assert_eq!(
            attempt.request().candidate_path(),
            candidate_path.to_str().unwrap()
        );
        assert_eq!(attempt.request().expected_original_sha256(), &expected);
        assert_eq!(attempt.request().registers(), registers);
        let error = attempt.result().err().expect("designated live failure");
        assert_eq!(error.compiler_fatal(), fatal_after_first);
        let first = first.lock().unwrap().take().unwrap();
        let original_after = FileObservation::read(&original_path);
        let candidate_after = FileObservation::read(&candidate_path);
        assert_eq!(candidate_after, first.candidate);
        original_after.same_identity(&original_before);
        match mode.as_str() {
            "repeat-success" | "postcallback-fatal-success" => {
                assert_eq!(first.outcome, FirstOutcome::Published);
                assert_eq!(error.phase(), FailurePhaseV1::Frontend);
                assert_eq!(
                    error.publication(),
                    CandidatePublicationStateV1::MayHaveCreatedCandidate
                );
                assert_eq!(
                    error.diagnostic(),
                    if fatal_after_first {
                        "rustc reported a fatal error after the promotion callback"
                    } else {
                        "source promotion received a repeated live callback"
                    }
                );
                assert_eq!(candidate_after.mode, 0o600);
                assert_eq!(original_after, original_before);
                assert_eq!(first.original, original_before);
                assert!(late.lock().unwrap().is_none());
            }
            "repeat-existing" | "postcallback-fatal-existing" => {
                assert_eq!(error.phase(), FailurePhaseV1::Publication);
                assert_eq!(
                    error.publication(),
                    CandidatePublicationStateV1::MayHaveCreatedCandidate
                );
                assert!(error.diagnostic().starts_with(
                    "candidate publication failed without replacing an existing entry: "
                ));
                assert_eq!(
                    first.outcome,
                    FirstOutcome::Refused(
                        error.phase(),
                        error.publication(),
                        error.diagnostic().to_owned(),
                        false,
                    )
                );
                assert_eq!(candidate_after, existing.unwrap());
                assert_eq!(original_after, original_before);
                assert_eq!(first.original, original_before);
                assert!(late.lock().unwrap().is_none());
            }
            "postpublication-original-change" => {
                assert_eq!(error.phase(), FailurePhaseV1::PostPublication);
                assert_eq!(
                    error.publication(),
                    CandidatePublicationStateV1::MayHaveCreatedCandidate
                );
                assert_eq!(
                    error.diagnostic(),
                    "source changed before candidate publication"
                );
                assert_eq!(
                    first.outcome,
                    FirstOutcome::Refused(
                        error.phase(),
                        error.publication(),
                        error.diagnostic().to_owned(),
                        false,
                    )
                );
                let (created, changed) = late.lock().unwrap().take().unwrap();
                assert_eq!(candidate_after, created);
                assert_eq!(candidate_after.mode, 0o600);
                assert_eq!(original_after, changed);
                assert_eq!(first.original, changed);
                assert_ne!(original_after.bytes, original_before.bytes);
            }
            _ => unreachable!(),
        }
        assert_eq!(
            fs::read(relative.join("original-loader.rs")).unwrap(),
            ORIGINAL_LOADER
        );
        assert_eq!(
            fs::read(relative.join("candidate-loader.rs")).unwrap(),
            CANDIDATE_LOADER
        );
        let mut names: Vec<_> = fs::read_dir(&relative)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                "candidate-loader.rs",
                "candidate.rs",
                "original-loader.rs",
                "original.rs"
            ]
        );
        assert!(
            fs::read_dir(directory.join("analysis-output"))
                .unwrap()
                .next()
                .is_none()
        );
        require_current_source();
        let report = serde_json::to_vec(&json!({
            "invocation":actual,"mode":mode,"actual_compiler_sessions":1,
            "actual_callback_entries":compiler_entries,"deliberate_callback_method_calls":method_calls,
            "phase":format!("{:?}",error.phase()),"publication":format!("{:?}",error.publication()),
            "diagnostic":error.diagnostic(),"compiler_fatal":error.compiler_fatal(),
            "original_before":original_before.summary(),"original_after":original_after.summary(),
            "candidate_after_first_method":first.candidate.summary(),
            "candidate_after":candidate_after.summary(),"exact_bytes_inode_mode_checked":true,
            "exact_source_directory_entries_checked":true,"sync_failure_injected":false,
            "normal_public_entry_used":false,"production_resume":false,"hardware_observed":false,
        })).unwrap();
        assert!(report.len() <= 64 * 1024);
        println!("\n{PREFIX}{}", std::str::from_utf8(&report).unwrap());
    }

    fn run_live_child(directory: &Path, case: &str, mode: &str) -> Value {
        let actual = record(directory, case);
        paths::write_new(
            &directory.join(case).join("live.invocation.json"),
            &serde_json::to_vec_pretty(&actual).unwrap(),
        );
        let mut child = Command::new(std::env::current_exe().unwrap());
        let stdout = checked(
            sanitized(&mut child)
                .current_dir(repository())
                .args(["--exact", CHILD, "--ignored", "--nocapture"])
                .env(ROUNDTRIP_INPUT, directory)
                .env(ROUNDTRIP_CASE, case)
                .env(MODE, mode)
                .env(CRATE_BINDING_ID_ENV_V1, &actual.crate_binding)
                .env(
                    CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
                    &actual.cargo_observation,
                )
                .env("CARGO_MANIFEST_DIR", fixture())
                .env("CARGO_PKG_NAME", PACKAGE)
                .env("CARGO_PKG_VERSION", "0.1.0")
                .env("CARGO_CRATE_NAME", CRATE_NAME),
            directory,
            &format!("live-{mode}"),
            None,
        );
        let text = std::str::from_utf8(&stdout).unwrap();
        let rows: Vec<_> = text
            .lines()
            .filter_map(|line| line.strip_prefix(PREFIX))
            .collect();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].len() <= 64 * 1024);
        assert_eq!(
            text.lines()
                .filter(|line| line.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;"))
                .count(),
            1
        );
        let value: Value = serde_json::from_str(rows[0]).unwrap();
        assert_eq!(value["invocation"], serde_json::to_value(actual).unwrap());
        assert_eq!(value["mode"], mode);
        if FATAL_CASES.contains(&(case, mode)) {
            assert_eq!(value["compiler_fatal"], true);
            let stderr =
                read_bounded(&directory.join(format!("live-{mode}.stderr")), 64 * 1024).unwrap();
            let diagnostic = std::str::from_utf8(&stderr).unwrap();
            assert_eq!(
                diagnostic.matches(LIVE_FATAL_DIAGNOSTIC).count(),
                1,
                "the actual rustc fatal diagnostic was emitted once"
            );
        }
        value
    }

    #[test]
    #[ignore = "isolated test-only live failure injection; fresh bounded output required"]
    fn actual_source_bitselect_live_failure_ladder() {
        let directory = PathBuf::from(std::env::var_os(OUTPUT).expect("fresh output directory"));
        assert!(directory.is_absolute());
        fs::create_dir(&directory).unwrap();
        let directory = directory.canonicalize().unwrap();
        let source_root = paths::create_root(&directory);
        require_current_source();
        prepare(&directory);
        let mut observations = Vec::with_capacity(3);
        for (case, mode) in CASES {
            fs::create_dir(directory.join(case)).unwrap();
            let source = source_root.join(case);
            fs::create_dir(&source).unwrap();
            paths::write_new(&source.join("original.rs"), FIXTURE_FILES[2].1);
            paths::write_new(&source.join("original-loader.rs"), ORIGINAL_LOADER);
            paths::write_new(&source.join("candidate-loader.rs"), CANDIDATE_LOADER);
            if mode == "repeat-existing" {
                paths::write_new(&source.join("candidate.rs"), EXISTING);
            }
            observations.push(run_live_child(&directory, case, mode));
        }
        assert_eq!(observations.len(), 3);
        assert_eq!(
            observations
                .iter()
                .map(|v| v["actual_compiler_sessions"].as_u64().unwrap())
                .sum::<u64>(),
            3
        );
        assert_eq!(
            observations
                .iter()
                .map(|v| v["actual_callback_entries"].as_u64().unwrap())
                .sum::<u64>(),
            3
        );
        assert_eq!(
            observations
                .iter()
                .map(|v| v["deliberate_callback_method_calls"].as_u64().unwrap())
                .sum::<u64>(),
            5
        );
        let report = serde_json::to_vec_pretty(&json!({
            "observations":observations,"actual_compiler_sessions":3,"actual_callback_entries":3,
            "deliberate_callback_method_calls":5,"normal_public_entry_used":false,
            "postpublication_case":"actual original-file edit and mandatory custody recheck",
            "sync_failure_injected":false,"production_resume":false,"hardware_observed":false,
        }))
        .unwrap();
        assert!(report.len() <= 256 * 1024);
        paths::write_new(&directory.join("live-failures-observation.json"), &report);
        require_current_source();
    }

    #[test]
    #[ignore = "actual rustc fatal after inner callback; separate fresh bounded output required"]
    fn actual_source_bitselect_post_callback_fatal_ladder() {
        let directory =
            PathBuf::from(std::env::var_os(FATAL_OUTPUT).expect("fresh output directory"));
        assert!(directory.is_absolute());
        fs::create_dir(&directory).unwrap();
        let directory = directory.canonicalize().unwrap();
        let source_root = paths::create_root(&directory);
        require_current_source();
        prepare(&directory);
        let mut observations = Vec::with_capacity(FATAL_CASES.len());
        for (case, mode) in FATAL_CASES {
            fs::create_dir(directory.join(case)).unwrap();
            let source = source_root.join(case);
            fs::create_dir(&source).unwrap();
            paths::write_new(&source.join("original.rs"), FIXTURE_FILES[2].1);
            paths::write_new(&source.join("original-loader.rs"), ORIGINAL_LOADER);
            paths::write_new(&source.join("candidate-loader.rs"), CANDIDATE_LOADER);
            if mode == "postcallback-fatal-existing" {
                paths::write_new(&source.join("candidate.rs"), EXISTING);
            }
            let observation = run_live_child(&directory, case, mode);
            assert_eq!(observation["actual_compiler_sessions"], 1);
            assert_eq!(observation["actual_callback_entries"], 1);
            assert_eq!(observation["deliberate_callback_method_calls"], 1);
            assert_eq!(observation["compiler_fatal"], true);
            assert_eq!(observation["publication"], "MayHaveCreatedCandidate");
            assert_eq!(
                observation["original_before"],
                observation["original_after"]
            );
            assert_eq!(
                observation["candidate_after_first_method"],
                observation["candidate_after"]
            );
            observations.push(observation);
        }
        assert_eq!(observations.len(), 2);
        let report = serde_json::to_vec_pretty(&json!({
            "observations":observations,"actual_compiler_sessions":2,"actual_callback_entries":2,
            "deliberate_callback_method_calls":2,"actual_rustc_fatal_diagnostics":2,
            "fatal_trigger":"test-only tcx.dcx().fatal after the completed inner promotion callback",
            "normal_public_entry_used":false,"compiler_fatal_flag_synthesized":false,
            "original_and_candidate_exact_custody_checked":true,
            "sync_failure_injected":false,"os_sync_failure_observed":false,
            "candidate_removed_or_rolled_back":false,
            "production_resume":false,"hardware_observed":false,
        }))
        .unwrap();
        assert!(report.len() <= 256 * 1024);
        paths::write_new(
            &directory.join("post-callback-fatal-observation.json"),
            &report,
        );
        require_current_source();
    }
}
