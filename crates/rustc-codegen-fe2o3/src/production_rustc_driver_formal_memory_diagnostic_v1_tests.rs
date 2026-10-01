//! Opted-in ordinary-source diagnostics, not a successful native compilation.
use super::*;
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::os::unix::fs::DirBuilderExt;

#[path = "../tests/production_scoped_tile_cpu_driver_v1/process.rs"]
mod process;

const REQUEST: &str = "FE2O3_TEST_FORMAL_DIAGNOSTIC_REQUEST_V1";
const DIRECTORY: &str = "FE2O3_TEST_FORMAL_DIAGNOSTIC_DIRECTORY_V1";
const CHILD: &str = concat!(
    "production_rustc_driver_v1::checked_output_source_v1_tests::",
    "formal_memory_diagnostic::formal_memory_diagnostic_child"
);
const BYTE_LIMIT: usize = 16 * 1024 * 1024;

struct Scratch(
    PathBuf,
    #[allow(dead_code)] crate::test_temp_dir::TestTempDir,
);

impl Scratch {
    fn new(label: &str) -> Self {
        let guard = crate::test_temp_dir::TestTempDir::create(label);
        Self(guard.path().to_owned(), guard)
    }
}

pub(super) fn bounded_output(
    command: &mut Command,
    directory: &Path,
    label: &str,
    timeout: u64,
) -> Result<std::process::Output, String> {
    process::Job::start(command, directory, label, timeout)
        .finish()
        .map(|capture| std::process::Output {
            status: capture.status,
            stdout: capture.stdout,
            stderr: capture.stderr,
        })
}

pub(super) fn read_bounded(path: &Path) -> Result<Vec<u8>, String> {
    let mut file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    if !file
        .metadata()
        .map_err(|error| error.to_string())?
        .is_file()
    {
        return Err("diagnostic input is not a regular file".into());
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(BYTE_LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > BYTE_LIMIT {
        return Err("diagnostic input exceeds byte bound".into());
    }
    Ok(bytes)
}

struct LimitedJson(Vec<u8>);

impl Write for LimitedJson {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > BYTE_LIMIT.saturating_sub(self.0.len()) {
            return Err(std::io::Error::other("diagnostic JSON exceeds byte bound"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn fresh_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let mut bytes = LimitedJson(Vec::new());
    serde_json::to_writer(&mut bytes, value).map_err(|error| error.to_string())?;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut file| file.write_all(&bytes.0))
        .map_err(|error| error.to_string())
}

fn digest(bytes: &[u8]) -> String {
    crate::encode_hex(&Sha256::digest(bytes))
}

fn original_native_rustflags(target: &str) -> String {
    format!(
        "-Zalways-encode-mir -Ctarget-cpu={target} -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32"
    )
}

fn replay_preserves_arguments(
    original: &[String],
    replay: &[String],
    captured_sysroot: &str,
    output: &Path,
) -> bool {
    struct Options<'a> {
        stable: Vec<&'a str>,
        sysroot: Option<(usize, Vec<&'a str>, &'a str)>,
        output: Option<&'a str>,
    }
    fn parsed(args: &[String]) -> Option<Options<'_>> {
        let mut parsed = Options {
            stable: Vec::new(),
            sysroot: None,
            output: None,
        };
        let mut index = 0;
        while index < args.len() {
            let argument = args[index].as_str();
            let option = ["--out-dir", "--sysroot"].into_iter().find(|option| {
                argument == *option
                    || argument
                        .strip_prefix(*option)
                        .is_some_and(|rest| rest.starts_with('='))
            });
            let Some(option) = option else {
                parsed.stable.push(argument);
                index += 1;
                continue;
            };
            let separate = argument == option;
            let value = if separate {
                args.get(index + 1)?.as_str()
            } else {
                argument.strip_prefix(option)?.strip_prefix('=')?
            };
            if !Path::new(value).is_absolute() {
                return None;
            }
            let spelling = if separate {
                vec![argument, value]
            } else {
                vec![argument]
            };
            if option == "--sysroot" {
                if parsed
                    .sysroot
                    .replace((parsed.stable.len(), spelling, value))
                    .is_some()
                {
                    return None;
                }
            } else {
                if parsed.output.replace(value).is_some() {
                    return None;
                }
                parsed.stable.push(if separate {
                    "--out-dir <replay-output>"
                } else {
                    "--out-dir=<replay-output>"
                });
            }
            index += if separate { 2 } else { 1 };
        }
        parsed.output?;
        Some(parsed)
    }
    let (Some(left), Some(right)) = (parsed(original), parsed(replay)) else {
        return false;
    };
    if left.stable != right.stable || right.output.map(Path::new) != Some(output) {
        return false;
    }
    match (left.sysroot, right.sysroot) {
        (Some(left), Some(right)) => left == right && right.2 == captured_sysroot,
        (None, Some((position, _, value))) => {
            position == right.stable.len() && value == captured_sysroot
        }
        _ => false,
    }
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .canonicalize()
        .unwrap()
}

fn checkout_identity(workspace: &Path, directory: &Path, label: &str) -> Value {
    let output = bounded_output(
        clean_command("git")
            .current_dir(workspace)
            .args(["rev-parse", "HEAD", "HEAD^{tree}"]),
        directory,
        &format!("{label}-identity"),
        30,
    )
    .unwrap();
    assert!(output.status.success());
    let text = std::str::from_utf8(&output.stdout).unwrap();
    let values: Vec<_> = text.lines().collect();
    assert_eq!(values.len(), 2);
    assert!(
        values
            .iter()
            .all(|value| value.len() == 40 && value.bytes().all(|b| b.is_ascii_hexdigit()))
    );
    let status = bounded_output(
        clean_command("git").current_dir(workspace).args([
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
        ]),
        directory,
        &format!("{label}-status"),
        30,
    )
    .unwrap();
    assert!(
        status.status.success() && status.stdout.is_empty(),
        "diagnostic requires a clean source checkout"
    );
    let roster = bounded_output(
        clean_command("git").current_dir(workspace).args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ]),
        directory,
        &format!("{label}-source-roster"),
        30,
    )
    .unwrap();
    assert!(roster.status.success() && roster.stdout.last() == Some(&0));
    let paths: std::collections::BTreeSet<_> = roster.stdout[..roster.stdout.len() - 1]
        .split(|byte| *byte == 0)
        .collect();
    assert!(paths.len() <= 16_384);
    let mut fingerprint = Sha256::new();
    let mut total = 0_usize;
    for path in paths {
        let path = std::str::from_utf8(path).unwrap();
        assert!(!path.contains(['\n', '\\']) && !Path::new(path).is_absolute());
        let bytes = read_bounded(&workspace.join(path)).unwrap();
        total = total.checked_add(bytes.len()).unwrap();
        assert!(
            total <= 250 * 1024 * 1024,
            "diagnostic source-tree bound exceeded"
        );
        fingerprint.update(format!("{}  {path}\n", digest(&bytes)).as_bytes());
    }
    json!({
        "commit": values[0], "tree": values[1], "trackedAndUntrackedStatus": "clean",
        "sourceFingerprintSha256": crate::encode_hex(&fingerprint.finalize()),
        "fingerprintAlgorithm": "sha256 of byte-sorted unique git tracked/nonignored paths, each encoded as sha256(file) + two spaces + path + LF",
    })
}

fn actual_fixture(workspace: &Path) -> (corpus::Fixture, Value) {
    let bytes = read_bounded(&workspace.join("config/tutorial-kernel-manifest-v1.json")).unwrap();
    let manifest: Value = serde_json::from_slice(&bytes).unwrap();
    let matches: Vec<_> = manifest["compilerFixtures"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["fixtureId"] == "gfx942-workgroup-collectives")
        .collect();
    let [row] = matches.as_slice() else {
        panic!("one actual manifest configuration is required");
    };
    let fixture: corpus::Fixture = serde_json::from_value((*row).clone()).unwrap();
    assert_eq!(fixture.target, "gfx942");
    assert_eq!(
        fixture.compiler_input.package_manifest,
        "examples/workgroup_sync_v1/Cargo.toml"
    );
    assert_eq!(fixture.compiler_input.features, ["lds-kernel"]);
    assert!(!fixture.compiler_input.default_features);
    let mut inputs = Vec::new();
    for (path, expected) in [
        (
            &fixture.compiler_input.package_manifest,
            &fixture.compiler_input.package_manifest_sha256,
        ),
        (
            &fixture.compiler_input.cargo_lock_path,
            &fixture.compiler_input.cargo_lock_sha256,
        ),
    ] {
        let actual = workspace.join(path).canonicalize().unwrap();
        assert!(actual.starts_with(workspace));
        let bytes = read_bounded(&actual).unwrap();
        assert_eq!(digest(&bytes), *expected, "manifest input pin: {path}");
        inputs.push(json!({"path": path, "sha256": expected, "byteLength": bytes.len()}));
    }
    for path in &fixture.compiler_input.source_paths {
        let actual = workspace.join(path).canonicalize().unwrap();
        assert!(actual.starts_with(workspace));
        let bytes = read_bounded(&actual).unwrap();
        inputs.push(json!({"path": path, "sha256": digest(&bytes), "byteLength": bytes.len()}));
    }
    (
        fixture,
        json!({
            "manifestSha256": digest(&bytes), "physicalInputs": inputs,
            "closureSelection": "manifest declaration only; actual rustc source ownership is recorded separately",
        }),
    )
}

#[derive(Serialize, Deserialize)]
struct Request {
    args: Vec<String>,
    directory: PathBuf,
}

struct DiagnosticCallbacks {
    directory: PathBuf,
    result: Option<Result<Value, SourceFailure>>,
}

impl Callbacks for DiagnosticCallbacks {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )
            .map_err(|error| SourceFailure::new(SourceStage::SourceCollection, error))?;
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(
                usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT).unwrap(),
            );
            let mut budget = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(
                &mut work,
                crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
            );
            transaction
                .observe_pre_formal_memory_v1(&self.directory, &mut budget)
                .map_err(|error| {
                    SourceFailure::new(SourceStage::RankedChecks, format!("{error:?}"))
                })
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "subprocess helper; only the opted-in ordinary-source diagnostic supplies its request"]
fn formal_memory_diagnostic_child() {
    let Some(path) = env::var_os(REQUEST) else {
        return;
    };
    let request: Request =
        serde_json::from_slice(&read_bounded(Path::new(&path)).unwrap()).unwrap();
    assert!(request.directory.is_absolute() && request.directory.is_dir());
    let mut callbacks = DiagnosticCallbacks {
        directory: request.directory.clone(),
        result: None,
    };
    let completed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        rustc_driver::run_compiler(&request.args, &mut callbacks);
    }));
    let result = callbacks.result.unwrap_or_else(|| {
        Err(SourceFailure::new(
            SourceStage::Rustc,
            if completed.is_err() {
                "actual compiler panicked before diagnostic completion"
            } else {
                "actual-source callback did not run"
            },
        ))
    });
    let output = request.directory.join("formal-result.json");
    if let Err(error) = fresh_json(&output, &result) {
        let fallback = match &result {
            Ok(report) => json!({
                "diagnosticLimitation": error,
                "canonicalOwner": report["canonicalOwner"],
                "formalAdmission": report["formalAdmission"],
                "optimized": report["optimized"],
            }),
            Err(failure) => json!({"diagnosticLimitation": error, "compilerFailure": failure}),
        };
        fresh_json(
            &request.directory.join("formal-result-limited.json"),
            &fallback,
        )
        .unwrap();
        panic!("complete diagnostic could not be retained: {error}");
    }
    assert!(
        completed.is_ok(),
        "compiler panicked; actual callback result retained"
    );
}

#[test]
#[ignore = "requires pinned nightly rust-src and real AMD source capture; never launches a GPU"]
fn ordinary_lds_source_retains_owner_bound_execution_discharge() {
    let directory = env::var_os(DIRECTORY)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            env::temp_dir().join(format!(
                "fe2o3-formal-diagnostic-{}-{nonce}",
                std::process::id()
            ))
        });
    assert!(directory.is_absolute());
    let directory = directory
        .parent()
        .unwrap()
        .canonicalize()
        .unwrap()
        .join(directory.file_name().expect("named evidence directory"));
    let workspace = workspace();
    assert!(
        !directory.starts_with(&workspace),
        "diagnostic evidence must remain outside the source checkout"
    );
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .unwrap();
    eprintln!(
        "formal diagnostic evidence retained at {}",
        directory.display()
    );
    let identity = checkout_identity(&workspace, &directory, "before");
    let (fixture, inputs) = actual_fixture(&workspace);
    let rustflags = original_native_rustflags(&fixture.target);
    let captured = corpus_cargo::capture_with_exact_rustflags(
        &workspace,
        &fixture,
        &directory,
        &directory.join("dependencies"),
        &rustflags,
    )
    .unwrap_or_else(|error| {
        panic!(
            "actual Cargo capture: {error:?}; evidence: {}",
            directory.display()
        )
    });
    assert!(!directory.join("cargo-root.env").exists());
    assert!(replay_preserves_arguments(
        &captured.original_args,
        &captured.args,
        &captured.explicit_sysroot,
        &directory.join("compiler-output"),
    ));
    assert!(
        !captured
            .original_args
            .iter()
            .any(|argument| argument.contains("overflow-checks"))
    );
    assert_eq!(
        captured
            .environment
            .iter()
            .find(|(key, _)| key == "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS")
            .map(|(_, value)| value),
        Some(&OsString::from(&rustflags)),
    );
    fresh_json(&directory.join("source-invocation.json"), &json!({
        "schema": "fe2o3-test-formal-source-invocation-v1",
        "checkout": identity, "fixture": fixture, "inputs": inputs,
        "requestedTargetRustflags": rustflags,
        "originalCargoRustcArguments": captured.original_args,
        "replayedRustcArguments": captured.args, "compilerCwd": captured.cwd,
        "capturedCompilerSysroot": captured.explicit_sysroot,
        "actualCfg": captured.cfg,
        "replayChanges": ["explicit captured compiler sysroot when originally implicit", "isolated output directory"],
        "rawEnvironmentRetained": false, "nativeCompilationPassed": false,
    })).unwrap();
    let request_path = directory.join("request.json");
    fresh_json(
        &request_path,
        &Request {
            args: captured.args,
            directory: directory.clone(),
        },
    )
    .unwrap();
    let mut command = Command::new(env::current_exe().unwrap());
    command
        .env_clear()
        .envs(captured.environment)
        .current_dir(&captured.cwd)
        .args([
            "--ignored",
            "--exact",
            CHILD,
            "--test-threads=1",
            "--nocapture",
        ])
        .env(REQUEST, &request_path);
    let output = bounded_output(&mut command, &directory, "actual-rustc", 600).unwrap();
    let unexpected_artifacts: Vec<_> = std::fs::read_dir(directory.join("compiler-output"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| !path.is_file() || path.extension().is_none_or(|extension| extension != "d"))
        .collect();
    let final_identity = checkout_identity(&workspace, &directory, "after");
    assert_eq!(
        identity, final_identity,
        "source identity changed during capture"
    );
    let (_, final_inputs) = actual_fixture(&workspace);
    assert_eq!(
        inputs, final_inputs,
        "physical fixture bytes changed during capture"
    );
    let result: Result<Value, SourceFailure> =
        serde_json::from_slice(&read_bounded(&directory.join("formal-result.json")).unwrap())
            .unwrap();
    assert!(
        output.status.success(),
        "actual rustc child: {}",
        corpus_cargo::diagnostics(&output)
    );
    assert!(
        unexpected_artifacts.is_empty(),
        "diagnostic emitted native/compiler artifacts: {unexpected_artifacts:?}"
    );
    let report =
        result.unwrap_or_else(|error| panic!("earlier genuine compiler refusal: {error:?}"));
    assert_eq!(
        report["formalAdmission"]["status"], "admitted",
        "the exact singleton proof must discharge the retained raw conflict"
    );
    let execution = &report["formalAdmission"]["execution"];
    assert_eq!(execution["observationGrantsAuthority"], false);
    let [kernel] = execution["kernels"].as_array().unwrap().as_slice() else {
        panic!("exactly one actual kernel execution report is required");
    };
    let [conflict] = kernel["rawConflicts"].as_array().unwrap().as_slice() else {
        panic!("the original self-conflict must remain visible");
    };
    let [discharge] = kernel["executionDischarges"].as_array().unwrap().as_slice() else {
        panic!("the exact raw conflict requires one explicit discharge");
    };
    assert_eq!(discharge["conflictOrdinal"], 0);
    for field in ["left", "right", "allocationParameter"] {
        assert_eq!(discharge[field], conflict[field], "wrong discharge {field}");
    }
    assert_eq!(conflict["left"], conflict["right"]);
    assert_eq!(conflict["ownershipStatus"], "unique-live-owner");
    assert_eq!(kernel["kernel"], report["kernels"][0]);
    assert_eq!(kernel["entry"], conflict["uniqueOwner"]["function"]);
    assert_eq!(discharge["leftWitness"], discharge["rightWitness"]);
    assert_eq!(discharge["leftWitness"]["invocation"], 0);
    assert_eq!(discharge["leftWitness"]["path"]["kind"], "trueEdge");
    assert!(kernel["rankedDischargedReasonCount"].as_u64().unwrap() > 0);
    assert!(kernel["compilerDischargedReasonCount"].as_u64().unwrap() > 0);
    assert_eq!(report["formalAdmission"]["nativeLowering"], "not attempted");
    assert_eq!(
        report["optimized"]["status"],
        "unavailable_not_executed_by_diagnostic"
    );
    assert_eq!(
        report["canonicalOwner"]["capture"]["path"],
        "pre-formal-owner.kir"
    );
    let kir = read_bounded(&directory.join("pre-formal-owner.kir")).unwrap();
    assert_eq!(report["canonicalOwner"]["bytesSha256"], digest(&kir));
    assert_eq!(
        report["kernels"],
        json!(fixture.compiler_input.kernel_symbols)
    );
    assert_eq!(report["functionLayouts"]["status"], "exact");
    assert!(
        report["operations"]
            .as_array()
            .is_some_and(|rows| !rows.is_empty())
    );
    assert!(report["correspondence"].is_array());
    assert!(!report["liveSource"]["files"].as_array().unwrap().is_empty());
    // Source-map gaps and non-unique conflict ownership remain visible; the
    // diagnostic does not replace them with a guessed source line.
}

#[test]
fn diagnostic_replay_preserves_flags_and_rejects_other_argument_changes() {
    let accepted = |original: &[String], replay: &[String]| {
        replay_preserves_arguments(original, replay, "/actual-sysroot", Path::new("/isolated"))
    };
    for cpu in ["gfx942", "gfx950"] {
        let flags = original_native_rustflags(cpu);
        assert!(!flags.contains("overflow-checks"));
        let original: Vec<_> = ["rustc", "--out-dir", "/old", "source.rs"]
            .into_iter()
            .map(str::to_owned)
            .chain(flags.split_whitespace().map(str::to_owned))
            .collect();
        let mut replay = original.clone();
        replay[2] = "/isolated".into();
        replay.extend(["--sysroot".into(), "/actual-sysroot".into()]);
        assert!(accepted(&original, &replay));
        replay.push("-Coverflow-checks=on".into());
        assert!(!accepted(&original, &replay));
        replay.pop();
        replay[3] = "other-source.rs".into();
        assert!(!accepted(&original, &replay));
    }
}

#[test]
fn diagnostic_replay_requires_exact_sysroot_and_output_option_census() {
    let strings = |args: &[&str]| args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
    for original in [
        strings(&[
            "rustc",
            "--sysroot",
            "/actual-sysroot",
            "--out-dir",
            "/old",
            "source.rs",
        ]),
        strings(&[
            "rustc",
            "--sysroot=/actual-sysroot",
            "--out-dir=/old",
            "source.rs",
        ]),
    ] {
        let valid: Vec<_> = original
            .iter()
            .map(|arg| match arg.as_str() {
                "/old" => "/isolated".into(),
                "--out-dir=/old" => "--out-dir=/isolated".into(),
                _ => arg.clone(),
            })
            .collect();
        let accepted = |replay: &[String]| {
            replay_preserves_arguments(&original, replay, "/actual-sysroot", Path::new("/isolated"))
        };
        assert!(accepted(&valid));
        assert!(!replay_preserves_arguments(
            &original,
            &valid,
            "/wrong-captured-sysroot",
            Path::new("/isolated")
        ));
        let changed: Vec<_> = valid
            .iter()
            .map(|arg| arg.replace("/actual-sysroot", "/replacement"))
            .collect();
        assert!(!accepted(&changed));
        let removed: Vec<_> = valid
            .iter()
            .filter(|arg| !arg.starts_with("--sysroot") && arg.as_str() != "/actual-sysroot")
            .cloned()
            .collect();
        assert!(!accepted(&removed));
        let no_output: Vec<_> = valid
            .iter()
            .filter(|arg| !arg.starts_with("--out-dir") && arg.as_str() != "/isolated")
            .cloned()
            .collect();
        assert!(!accepted(&no_output));
        for extra in [
            vec!["--sysroot=/actual-sysroot"],
            vec!["--sysroot", "/actual-sysroot"],
            vec!["--out-dir=/isolated"],
            vec!["--out-dir", "/isolated"],
            vec!["--sysroot"],
            vec!["--out-dir"],
        ] {
            let mut duplicate = valid.clone();
            duplicate.extend(strings(&extra));
            assert!(!accepted(&duplicate));
        }
    }
    let original = strings(&["rustc", "--out-dir", "/old", "source.rs"]);
    for malformed in [
        vec!["rustc", "--out-dir", "/isolated", "source.rs", "--sysroot"],
        vec![
            "rustc",
            "--out-dir",
            "--sysroot",
            "/actual-sysroot",
            "source.rs",
        ],
        vec![
            "rustc",
            "--out-dir=",
            "source.rs",
            "--sysroot=/actual-sysroot",
        ],
        vec![
            "rustc",
            "--out-dir",
            "/wrong",
            "source.rs",
            "--sysroot=/actual-sysroot",
        ],
        vec!["rustc", "--out-dir", "/isolated", "source.rs", "--sysroot="],
        vec![
            "rustc",
            "--out-dir",
            "/isolated",
            "source.rs",
            "--sysroot=/wrong",
        ],
    ] {
        assert!(!replay_preserves_arguments(
            &original,
            &strings(&malformed),
            "/actual-sysroot",
            Path::new("/isolated")
        ));
    }
}

#[test]
fn diagnostic_json_is_bounded_and_create_new() {
    let scratch = Scratch::new("formal-diagnostic-json");
    let path = scratch.0.join("report.json");
    fresh_json(&path, &json!({"first": true})).unwrap();
    assert!(fresh_json(&path, &json!({"second": true})).is_err());
    assert_eq!(
        serde_json::from_slice::<Value>(&read_bounded(&path).unwrap()).unwrap(),
        json!({"first": true})
    );
    let mut bounded = LimitedJson(Vec::new());
    assert!(bounded.write(&vec![0; BYTE_LIMIT + 1]).is_err());
    assert!(bounded.0.is_empty());
}
