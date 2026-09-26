// Strict ordinary-source assertion qualification before ranked/target stages.
use super::*;
use std::io::{Read, Write};

const REQUEST_ENV: &str = "FE2O3_TEST_CANONICAL_ASSERTION_REQUEST_V1";
const ARGS_ENV: &str = "FE2O3_TEST_CANONICAL_ASSERTION_ARGS_V1";
const RESULT_ENV: &str = "FE2O3_TEST_CANONICAL_ASSERTION_RESULT_V1";
const CHILD_NAME: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::integer_identity_source::canonical_assertion_source::canonical_assertion_source_child";
const INPUT_CAP: usize = 1024 * 1024;
const REPORT_CAP: usize = 8 * 1024 * 1024;
const CASE_CFGS: [&str; 5] = [
    "fe2o3_canonical_assertion_retained",
    "fe2o3_canonical_assertion_literal",
    "fe2o3_canonical_assertion_masked",
    "fe2o3_canonical_assertion_shared",
    "fe2o3_canonical_assertion_private",
];

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum AssertionCase {
    RetainedArithmeticOpt0,
    SignedLiteralShiftOpt0,
    MaskedShiftElidedOpt0,
    SharedScalarAliasesOpt0,
    UnitLocalPrivateAliasesOpt0,
}
impl AssertionCase {
    fn ordinal(self) -> usize {
        match self {
            Self::RetainedArithmeticOpt0 => 0,
            Self::SignedLiteralShiftOpt0 => 1,
            Self::MaskedShiftElidedOpt0 => 2,
            Self::SharedScalarAliasesOpt0 => 3,
            Self::UnitLocalPrivateAliasesOpt0 => 4,
        }
    }
    fn roots(self) -> &'static [&'static str] {
        match self {
            Self::RetainedArithmeticOpt0 => &["assertion_retained"],
            Self::SignedLiteralShiftOpt0 => &["assertion_literal"],
            Self::MaskedShiftElidedOpt0 => &["assertion_masked"],
            Self::SharedScalarAliasesOpt0 => &["assertion_shared_left", "assertion_shared_right"],
            Self::UnitLocalPrivateAliasesOpt0 => {
                &["assertion_private_left", "assertion_private_right"]
            }
        }
    }
}
include!("production_rustc_driver_canonical_assertion_protocol_v1_tests.rs");
include!("production_rustc_driver_canonical_assertion_observation_v1_tests.rs");

struct AssertionCallbacks {
    request: AssertionRequest,
    count: usize,
    result: Option<Result<Observation, String>>,
}
impl Callbacks for AssertionCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.count += 1;
        self.result = Some(if self.count == 1 {
            observe(tcx, &self.request)
        } else {
            Err("duplicate compiler callback".into())
        });
        Compilation::Stop
    }
}
#[test]
#[ignore = "strict consuming subprocess helper; missing parent request is failure"]
fn canonical_assertion_source_child() {
    let path = |key| {
        PathBuf::from(env::var_os(key).expect("strict assertion child requires parent paths"))
    };
    let request: AssertionRequest = read_json(&path(REQUEST_ENV), INPUT_CAP).unwrap();
    let invocation: Invocation = read_json(&path(ARGS_ENV), INPUT_CAP).unwrap();
    let result_path = path(RESULT_ENV);
    assert!(!result_path.exists());
    let mut callbacks = AssertionCallbacks {
        request: request.clone(),
        count: 0,
        result: None,
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        check_request(&request, &invocation)?;
        rustc_driver::run_compiler(&invocation.executed, &mut callbacks);
        require(callbacks.count == 1, "exactly one actual compiler callback")?;
        let row = callbacks.result.take().ok_or("missing callback")??;
        check_request(&request, &invocation)?;
        validate(&request, &row)?;
        Ok(row)
    }))
    .unwrap_or_else(|_| Err("compiler or callback panicked".into()));
    let success = result.is_ok();
    let report = Report {
        request,
        callbacks: callbacks.count,
        result,
    };
    if success {
        audit_successful_protocol(&report.request, &invocation, &report);
    }
    write_json(&result_path, &report, REPORT_CAP).unwrap();
    assert!(success, "strict canonical assertion source: {report:?}");
}
fn fixture(case: AssertionCase, target: Target) -> corpus::Fixture {
    let source = stamps().unwrap();
    let hex = |bytes: [u8; 32]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    corpus::Fixture {
        fixture_id: format!("canonical-assertion-{case:?}-{}", target.cpu()),
        target: target.cpu().into(),
        compiler_input: corpus::CompilerInput {
            package_manifest_sha256: hex(source[1].sha256),
            package_manifest: format!("{BASE}/Cargo.toml"),
            cargo_lock_path: "Cargo.lock".into(),
            cargo_lock_sha256: hex(source[0].sha256),
            source_paths: vec![
                format!("{BASE}/src/lib.rs"),
                format!("{BASE}/src/canonical_assertion_source.rs"),
            ],
            source_closure_sha256: String::new(),
            cargo_target: corpus::CargoTarget {
                kind: "lib".into(),
                name: "fe2o3_production_extraction_fixture".into(),
                source_path: "src/lib.rs".into(),
            },
            default_features: false,
            features: vec!["canonical-assertion-source".into()],
            kernel_symbols: case.roots().iter().map(|name| (*name).into()).collect(),
        },
    }
}
fn qualify(case: AssertionCase) {
    let scratch = crate::test_temp_dir::TestTempDir::create("canonical-assertion-source");
    for target in [Target::Gfx942, Target::Gfx950] {
        let directory = scratch.path().join(target.cpu());
        std::fs::create_dir(&directory).unwrap();
        let captured = corpus_cargo::capture(
            &workspace(),
            &fixture(case, target),
            &directory,
            &scratch.path().join("cargo-target"),
        )
        .unwrap();
        let invocation = Invocation {
            executed: executed(&captured.args, case).unwrap(),
            captured: captured.args,
        };
        let request = AssertionRequest {
            schema: 1,
            run_id: format!("{}-{case:?}-{}", directory.display(), std::process::id()),
            case,
            target,
            captured: args_hash(&invocation.captured),
            executed: args_hash(&invocation.executed),
            cwd: captured.cwd.clone(),
            source: stamps().unwrap(),
        };
        let (request_path, args_path, result_path) = (
            directory.join("request.json"),
            directory.join("invocation.json"),
            directory.join("report.json"),
        );
        write_json(&request_path, &request, INPUT_CAP).unwrap();
        write_json(&args_path, &invocation, INPUT_CAP).unwrap();
        let mut command = Command::new(env::current_exe().unwrap());
        command
            .env_clear()
            .envs(captured.environment.iter().cloned())
            .current_dir(&captured.cwd);
        for (key, _) in &captured.environment {
            if key.to_string_lossy().starts_with("FE2O3_TEST_") {
                command.env_remove(key);
            }
        }
        command
            .env_remove("RUSTC_WRAPPER")
            .env_remove("RUSTC_WORKSPACE_WRAPPER");
        progress::clear_inherited_jobserver(&mut command);
        census::configure(&mut command, None);
        simulation::configure_child(&mut command, None);
        command
            .env(REQUEST_ENV, &request_path)
            .env(ARGS_ENV, &args_path)
            .env(RESULT_ENV, &result_path)
            .args(["--exact", CHILD_NAME, "--ignored", "--nocapture"]);
        let output = command.output().unwrap();
        let bytes = std::fs::File::open(&result_path).ok().map(|file| {
            let mut bytes = Vec::new();
            file.take(REPORT_CAP as u64 + 1)
                .read_to_end(&mut bytes)
                .unwrap();
            bytes
        });
        let observed =
            decode(output.status.code(), bytes.as_deref(), &request).unwrap_or_else(|e| {
                panic!(
                    "assertion {case:?}/{}: {e}\n{}",
                    target.cpu(),
                    corpus_cargo::diagnostics(&output)
                )
            });
        assert_eq!(request.source, stamps().unwrap());
        println!(
            "CANONICAL_ASSERTION_SOURCE {} {case:?} callbacks=1 roots={} assertions={} definitions={} pending=19",
            target.cpu(),
            observed.roots.len(),
            observed.rows.assertions.len(),
            observed.rows.definitions.len()
        );
    }
}
#[test]
#[ignore = "strict actual rustc original-owner assertions, both collection profiles"]
fn ordinary_rust_canonical_assertion_retained_both_profiles() {
    qualify(AssertionCase::RetainedArithmeticOpt0);
}

#[test]
#[ignore = "strict actual rustc original-owner assertions, both collection profiles"]
fn ordinary_rust_canonical_assertion_literal_shift_both_profiles() {
    qualify(AssertionCase::SignedLiteralShiftOpt0);
}

#[test]
#[ignore = "strict actual rustc original-owner assertions, both collection profiles"]
fn ordinary_rust_canonical_assertion_masked_elision_both_profiles() {
    qualify(AssertionCase::MaskedShiftElidedOpt0);
}

#[test]
#[ignore = "strict actual rustc original-owner assertions, both collection profiles"]
fn ordinary_rust_canonical_assertion_shared_helper_aliases_both_profiles() {
    qualify(AssertionCase::SharedScalarAliasesOpt0);
}

#[test]
#[ignore = "strict actual rustc original-owner assertions, both collection profiles"]
fn ordinary_rust_canonical_assertion_unitlocal_private_aliases_both_profiles() {
    qualify(AssertionCase::UnitLocalPrivateAliasesOpt0);
}

#[path = "production_rustc_driver_canonical_assertion_history_source_v1_tests.rs"]
mod canonical_assertion_history;
