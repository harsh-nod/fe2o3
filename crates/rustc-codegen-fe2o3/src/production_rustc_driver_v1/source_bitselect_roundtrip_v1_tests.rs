//! Task-owned source replacement and fresh admission; diagnostics never become owners.

use super::super::gfx942_inline_value_qualification_v30_tests::invocation_for_fixture_source;
use super::*;
use crate::collector::source_census_v1::bitselect_feasibility::retained::RetainedInput;
use std::io::Write as _;

#[path = "source_bitselect_headless_driver_v1_tests.rs"]
mod headless;
#[path = "source_local_order_driver_v1_tests.rs"]
mod local_order;
#[path = "source_bitselect_candidate_machine_driver_v1_tests.rs"]
mod machine;
#[path = "source_bitselect_roundtrip_paths_v1_tests.rs"]
mod paths;

const ROUNDTRIP_OUTPUT: &str = "FE2O3_TEST_SOURCE_BITSELECT_ROUNDTRIP_OUTPUT";
const ROUNDTRIP_INPUT: &str = "FE2O3_TEST_SOURCE_BITSELECT_ROUNDTRIP_INPUT";
const ROUNDTRIP_CASE: &str = "FE2O3_TEST_SOURCE_BITSELECT_ROUNDTRIP_CASE";
const ROUNDTRIP_PHASE: &str = "FE2O3_TEST_SOURCE_BITSELECT_ROUNDTRIP_PHASE";
const CHILD_TEST: &str = "production_rustc_driver_v1::source_bitselect_feasibility_v1_tests::roundtrip::actual_source_bitselect_roundtrip_child";
const REPORT_PREFIX: &str = "FE2O3_SOURCE_BITSELECT_ROUNDTRIP ";
const CASES: [&str; 8] = [
    "positive",
    "wrong-target",
    "wrong-launch",
    "stale-original",
    "replaced-original",
    "existing-candidate",
    "same-destination",
    "stale-candidate",
];
const ORIGINAL_LOADER: &[u8] =
    b"#![no_std]\n#[path = \"original.rs\"] mod source_bitselect_feasibility;\n";
const CANDIDATE_LOADER: &[u8] =
    b"#![no_std]\n#[path = \"candidate.rs\"] mod source_bitselect_feasibility;\n";
const EXISTING: &[u8] = b"// existing destination must remain unchanged\n";

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RoundtripInvocation {
    case: String,
    phase: String,
    source_directory: PathBuf,
    args: Vec<String>,
    crate_binding: String,
    cargo_observation: String,
    source_sha256: String,
    loader_sha256: String,
    fixture_sha256: [String; 4],
    artifacts_sha256: String,
    metadata_sha256: String,
}

fn derive_roundtrip(directory: &Path, case: &str, phase: &str) -> RoundtripInvocation {
    assert!(CASES.contains(&case));
    assert!(matches!(phase, "baseline" | "fresh"));
    let case_dir = paths::absolute_case(directory, case);
    let (source, loader) = if phase == "baseline" {
        ("original.rs", "original-loader.rs")
    } else {
        ("candidate.rs", "candidate-loader.rs")
    };
    paths::checked_source_file(&case_dir.join(source));
    paths::checked_source_file(&case_dir.join(loader));
    assert!(
        read_bounded(&case_dir.join(source), 128 * 1024)
            .unwrap()
            .len()
            > 0
    );
    let (args, crate_binding, cargo_observation) = invocation_for_fixture_source(
        directory,
        &fixture(),
        PACKAGE,
        CRATE_NAME,
        Some(FEATURES[0]),
        &case_dir.join(loader),
        if case == "wrong-target" {
            "gfx950"
        } else {
            "gfx942"
        },
    );
    RoundtripInvocation {
        case: case.into(),
        phase: phase.into(),
        source_directory: paths::relative_case(directory, case),
        args,
        crate_binding,
        cargo_observation,
        source_sha256: hash(&case_dir.join(source)),
        loader_sha256: hash(&case_dir.join(loader)),
        fixture_sha256: FIXTURE_FILES.map(|(path, _)| hash(&fixture().join(path))),
        artifacts_sha256: hash(&directory.join("dependencies.stdout")),
        metadata_sha256: hash(&directory.join("metadata.stdout")),
    }
}

fn refusal(case: &str, phase: &str) -> Option<(&'static str, bool)> {
    match (case, phase) {
        ("wrong-target", "baseline") => Some((
            "source-candidate requires authenticated gfx942 xnack-off wave64",
            false,
        )),
        ("wrong-launch", "baseline") => Some((
            "source-candidate requires required and maximum 64x1x1 bounds",
            false,
        )),
        ("stale-original", "baseline") => {
            Some(("source changed before candidate publication", false))
        }
        ("replaced-original", "baseline") => {
            Some(("source path changed before candidate publication", false))
        }
        ("existing-candidate" | "same-destination", "baseline") => Some((
            "candidate publication failed without replacing an existing entry: ",
            true,
        )),
        ("stale-candidate", "fresh") => Some((
            "source-candidate retained bytes differ from parsed compiler input",
            false,
        )),
        _ => None,
    }
}

struct CandidateCallbacks {
    case: String,
    phase: String,
    source_directory: PathBuf,
    source: Option<RetainedInput>,
    calls: usize,
    result: Option<Result<Value, String>>,
}

impl Callbacks for CandidateCallbacks {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        let input = self.source.take().expect("one move-only source descriptor");
        let result = super::super::transaction_in_active_session_v1(
            tcx,
            crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
        )
        .and_then(|transaction| {
            if self.phase == "fresh" {
                transaction.observe_fresh_source_bitselect_candidate(input)
            } else {
                let case = self.case.as_str();
                let original = self.source_directory.join("original.rs");
                let retired = self.source_directory.join("retired-original.rs");
                let destination = self.source_directory.join(if case == "same-destination" {
                    "original.rs"
                } else {
                    "candidate.rs"
                });
                transaction.publish_source_bitselect_candidate(
                    input,
                    destination.to_str().unwrap(),
                    || {
                        if case == "stale-original" {
                            let mut bytes = read_bounded(&original, 64 * 1024)?;
                            bytes.extend_from_slice(b"\n// changed after the exact owner join\n");
                            fs::write(&original, bytes).map_err(|e| e.to_string())?;
                        } else if case == "replaced-original" {
                            let bytes = read_bounded(&original, 64 * 1024)?;
                            assert!(!retired.try_exists().map_err(|e| e.to_string())?);
                            fs::rename(&original, &retired).map_err(|e| e.to_string())?;
                            let mut replacement = fs::OpenOptions::new()
                                .write(true)
                                .create_new(true)
                                .open(&original)
                                .map_err(|e| e.to_string())?;
                            replacement.write_all(&bytes).map_err(|e| e.to_string())?;
                        }
                        Ok(())
                    },
                )
            }
        });
        self.result = Some(result);
        Compilation::Stop
    }
}

#[test]
#[ignore = "isolated actual rustc child; use actual_source_bitselect_candidate_roundtrip_ladder"]
fn actual_source_bitselect_roundtrip_child() {
    let directory =
        PathBuf::from(std::env::var_os(ROUNDTRIP_INPUT).expect("preparation directory"));
    let case = std::env::var(ROUNDTRIP_CASE).expect("case");
    let phase = std::env::var(ROUNDTRIP_PHASE).expect("phase");
    require_current_source();
    let actual = derive_roundtrip(&directory, &case, &phase);
    let retained: RoundtripInvocation = serde_json::from_slice(
        &read_bounded(
            &directory
                .join(&case)
                .join(format!("{phase}.invocation.json")),
            64 * 1024,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(actual, retained, "stale direct invocation preparation");
    assert_eq!(
        std::env::current_dir().unwrap(),
        repository().canonicalize().unwrap()
    );
    let source_directory = paths::relative_case(&directory, &case);
    assert_eq!(actual.source_directory, source_directory);
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
    super::super::require_canonical_overflow_checks_v1(&actual.args).unwrap();
    let source_path = source_directory.join(if phase == "fresh" {
        "candidate.rs"
    } else {
        "original.rs"
    });
    let source = RetainedInput::open(source_path.to_str().unwrap(), phase == "fresh").unwrap();
    if case == "stale-candidate" && phase == "fresh" {
        // Real changed bytes must fail the retained same-callback source hash.
        fs::OpenOptions::new()
            .append(true)
            .open(source_directory.join("candidate.rs"))
            .unwrap()
            .write_all(b"\n// modified after publication and retention, before fresh rustc\n")
            .unwrap();
    }
    let mut callbacks = CandidateCallbacks {
        case: case.clone(),
        phase: phase.clone(),
        source_directory: source_directory.clone(),
        source: Some(source),
        calls: 0,
        result: None,
    };
    rustc_driver::run_compiler(&actual.args, &mut callbacks);
    assert_eq!(
        callbacks.calls, 1,
        "real candidate callback must execute exactly once"
    );
    let result = callbacks
        .result
        .expect("candidate callback did not reach the boundary");
    let observation = if let Some((expected, prefix)) = refusal(&case, &phase) {
        let diagnostic = result.expect_err("negative must refuse at its exact boundary");
        if case == "replaced-original" && phase == "baseline" {
            // rename may also update the retained inode's ctime. Both exact
            // recheck errors are the same pre-publication currentness boundary;
            // no later compiler or arbitrary I/O failure is accepted.
            assert!(
                diagnostic == expected
                    || diagnostic == "source changed before candidate publication",
                "{diagnostic}"
            );
            assert_eq!(
                hash(&source_directory.join("retired-original.rs")),
                actual.source_sha256
            );
        } else if prefix {
            assert!(diagnostic.starts_with(expected), "{diagnostic}");
        } else {
            assert_eq!(
                diagnostic, expected,
                "later compiler failure is not source-boundary evidence"
            );
        }
        json!({"stage":"actual_candidate_boundary_refused","diagnostic":diagnostic})
    } else {
        let observed = result.unwrap();
        if phase == "baseline" {
            assert_eq!(observed["stage"], "same_session_source_candidate_published");
            assert_eq!(observed["baseline"]["kernel_ir_version"], "V8");
            assert_eq!(observed["candidate_written"], true);
        } else {
            assert_eq!(
                observed["stage"],
                "fresh_actual_source_mir32_kir17_candidate"
            );
            assert_eq!(observed["kernel_ir_version"], "V17");
            assert_eq!(observed["semantic_version"], "V32");
            assert_eq!(observed["fresh_frontend_admitted"], true);
            assert_eq!(observed["ranked_checks"], false);
            assert_eq!(observed["boolean_oracle_cases"], 128);
        }
        observed
    };
    if phase == "baseline" {
        if case != "stale-original" {
            assert_eq!(
                hash(&source_directory.join("original.rs")),
                actual.source_sha256
            );
        }
        if case == "existing-candidate" {
            assert_eq!(
                read_bounded(&source_directory.join("candidate.rs"), 64 * 1024).unwrap(),
                EXISTING
            );
        } else if refusal(&case, &phase).is_some() {
            assert!(!source_directory.join("candidate.rs").exists());
        }
    }
    require_current_source();
    assert!(
        fs::read_dir(directory.join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
    let report = serde_json::to_vec(&json!({
        "invocation":actual,"observation":observation,"actual_rustc_callback":true,
        "production_resume":false,"grants_artifact_or_launch_authority":false,
    }))
    .unwrap();
    assert!(report.len() <= 64 * 1024);
    println!("\n{REPORT_PREFIX}{}", std::str::from_utf8(&report).unwrap());
}

fn run_child(directory: &Path, case: &str, phase: &str) -> Value {
    let record = derive_roundtrip(directory, case, phase);
    paths::write_new(
        &directory
            .join(case)
            .join(format!("{phase}.invocation.json")),
        &serde_json::to_vec_pretty(&record).unwrap(),
    );
    let mut child = Command::new(std::env::current_exe().unwrap());
    let stdout = checked(
        sanitized(&mut child)
            .current_dir(repository())
            .args(["--exact", CHILD_TEST, "--ignored", "--nocapture"])
            .env(ROUNDTRIP_INPUT, directory)
            .env(ROUNDTRIP_CASE, case)
            .env(ROUNDTRIP_PHASE, phase)
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
        &format!("{case}-{phase}"),
        None,
    );
    let stdout = std::str::from_utf8(&stdout).unwrap();
    let lines = stdout
        .lines()
        .filter_map(|line| line.strip_prefix(REPORT_PREFIX))
        .collect::<Vec<_>>();
    assert_eq!(lines.len(), 1);
    assert!(lines[0].len() <= 64 * 1024);
    assert_eq!(
        stdout
            .lines()
            .filter(|line| line.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;"))
            .count(),
        1
    );
    let report: Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(report["invocation"], serde_json::to_value(record).unwrap());
    report
}

fn prepare(directory: &Path) {
    let rustc_path = PathBuf::from(std::env::var_os("RUSTC").expect("absolute pinned RUSTC"));
    assert!(rustc_path.is_absolute());
    let mut rustc = Command::new(rustc_path);
    let sysroot = checked(
        sanitized(&mut rustc).args(["--print", "sysroot"]),
        directory,
        "sysroot",
        None,
    );
    let sysroot = PathBuf::from(std::str::from_utf8(&sysroot).unwrap().trim_end());
    assert!(
        sysroot
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("nightly-2026-04-03-")
    );
    let mut metadata = Command::new(sysroot.join("bin/cargo"));
    checked(
        sanitized(&mut metadata)
            .args([
                "metadata",
                "--locked",
                "--offline",
                "--no-deps",
                "--format-version=1",
                "--manifest-path",
            ])
            .arg(fixture().join("Cargo.toml")),
        directory,
        "metadata",
        None,
    );
    let target = directory.join("dependencies");
    let mut cargo = Command::new(sysroot.join("bin/cargo"));
    checked(sanitized(&mut cargo).current_dir(repository()).args([
        "check","--release","--locked","--offline","-Zbuild-std=core","-p","fe2o3-device",
        "--target","amdgcn-amd-amdhsa","--message-format=json","--manifest-path",
    ]).arg(fixture().join("Cargo.toml")).arg("--target-dir").arg(&target)
        .env("RUSTC",sysroot.join("bin/rustc"))
        .env("CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32 -Coverflow-checks=on"),
        directory,"dependencies",Some(&target));
    fs::create_dir(directory.join("analysis-output")).unwrap();
}

#[test]
#[ignore = "pinned actual callbacks; serialized Cargo, fresh absolute output directory"]
fn actual_source_bitselect_candidate_roundtrip_ladder() {
    let directory =
        PathBuf::from(std::env::var_os(ROUNDTRIP_OUTPUT).expect("fresh output directory"));
    assert!(directory.is_absolute());
    fs::create_dir(&directory).unwrap();
    let directory = directory.canonicalize().unwrap();
    let source_root = paths::create_root(&directory);
    require_current_source();
    prepare(&directory);
    let mut observations = Vec::new();
    for case in CASES {
        fs::create_dir(directory.join(case)).unwrap();
        let case_dir = source_root.join(case);
        fs::create_dir(&case_dir).unwrap();
        let mut original = String::from_utf8(FIXTURE_FILES[2].1.to_vec()).unwrap();
        if case == "wrong-launch" {
            // Equal bounds pass the source macro, then fail our narrower profile.
            let launch = "required = [64, 1, 1], max = [64, 1, 1]";
            assert!(original.contains(launch));
            original = original.replace(launch, "required = [128, 1, 1], max = [128, 1, 1]");
        }
        assert!(original.len() <= 64 * 1024);
        paths::write_new(&case_dir.join("original.rs"), original.as_bytes());
        paths::write_new(&case_dir.join("original-loader.rs"), ORIGINAL_LOADER);
        paths::write_new(&case_dir.join("candidate-loader.rs"), CANDIDATE_LOADER);
        if case == "existing-candidate" {
            paths::write_new(&case_dir.join("candidate.rs"), EXISTING);
        }
        let baseline = run_child(&directory, case, "baseline");
        let fresh = if matches!(case, "positive" | "stale-candidate") {
            Some(run_child(&directory, case, "fresh"))
        } else {
            None
        };
        if case == "positive" {
            let fresh = fresh.as_ref().unwrap();
            assert_eq!(
                baseline["observation"]["candidate_sha256"],
                fresh["observation"]["candidate_sha256"]
            );
            assert_eq!(
                baseline["observation"]["descriptors"],
                fresh["observation"]["descriptors"]
            );
            assert_eq!(
                read_bounded(&case_dir.join("original.rs"), 64 * 1024).unwrap(),
                original.as_bytes()
            );
        }
        observations.push(json!({"case":case,"baseline":baseline,"fresh":fresh}));
    }
    require_current_source();
    let (source_files, source_bytes) = paths::footprint(&directory);
    let bytes = serde_json::to_vec_pretty(&json!({
        "observations":observations,"actual_rustc_callbacks":true,
        "source_directory":paths::relative_root(&directory),
        "source_files":source_files,"source_bytes":source_bytes,
        "source_file_limit":paths::MAX_SOURCE_FILES,"source_byte_limit":paths::MAX_SOURCE_BYTES,
        "baseline_owner_version":"V8","candidate_owner_version":"V17",
        "candidate_stage":"fresh_frontend_pre_ranked_diagnostic",
        "production_resume":false,"functional_proof":false,"hardware_observed":false,
        "grants_artifact_or_launch_authority":false,
    }))
    .unwrap();
    assert!(bytes.len() <= 512 * 1024);
    paths::write_new(&directory.join("observation.json"), &bytes);
    eprintln!(
        "source candidate roundtrip observations: {}",
        directory.display()
    );
}
