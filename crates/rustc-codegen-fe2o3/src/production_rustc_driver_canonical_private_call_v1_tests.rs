// Genuine ordinary rustc source, consuming C14 before any target-bound stage.
use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionCanonicalAssertionCallKindV1 as CallKind,
    ProductionCanonicalPrivateOperationKindV1 as PrivateKind,
};
use std::mem::size_of;

const P_REQUEST_ENV: &str = "FE2O3_TEST_CANONICAL_PRIVATE_CALL_REQUEST_V1";
const P_ARGS_ENV: &str = "FE2O3_TEST_CANONICAL_PRIVATE_CALL_ARGS_V1";
const P_RESULT_ENV: &str = "FE2O3_TEST_CANONICAL_PRIVATE_CALL_RESULT_V1";
const P_ROUTE: &str = "canonical-private-call-history-v1";
const P_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::integer_identity_source::canonical_scalar_source::private_call_history::canonical_private_call_source_child";
const P_CFGS: [&str; 3] = [
    "fe2o3_private_call_shared",
    "fe2o3_private_call_cross_block",
    "fe2o3_private_call_normal",
];

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum PrivateCase {
    SharedPrivateOpt0,
    CrossBlockPrivateOpt0,
    NormalTypedCalls,
}
impl PrivateCase {
    fn cfg(self) -> &'static str {
        P_CFGS[match self {
            Self::SharedPrivateOpt0 => 0,
            Self::CrossBlockPrivateOpt0 => 1,
            Self::NormalTypedCalls => 2,
        }]
    }
    fn roots(self) -> &'static [&'static str] {
        match self {
            Self::SharedPrivateOpt0 => &["private_call_first", "private_call_second"],
            Self::CrossBlockPrivateOpt0 => &["private_call_cross_block"],
            Self::NormalTypedCalls => &["private_call_normal"],
        }
    }
    fn opt0(self) -> bool {
        self != Self::NormalTypedCalls
    }
}
include!("production_rustc_driver_canonical_private_call_protocol_v1_tests.rs");
include!("production_rustc_driver_canonical_private_call_observation_v1_tests.rs");

struct PrivateCallbacks {
    request: PrivateRequest,
    count: usize,
    result: Option<Result<PrivateObservation, String>>,
}
impl Callbacks for PrivateCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.count += 1;
        self.result = Some(if self.count == 1 {
            private_observe(tcx, &self.request)
        } else {
            Err("duplicate compiler callback".into())
        });
        Compilation::Stop
    }
}
#[test]
#[ignore = "strict managed child; absent parent input fails"]
fn canonical_private_call_source_child() {
    let path = |key| PathBuf::from(env::var_os(key).expect("strict private/call parent paths"));
    let request: PrivateRequest = read_json(&path(P_REQUEST_ENV), INPUT_CAP).unwrap();
    let invocation: Invocation = read_json(&path(P_ARGS_ENV), INPUT_CAP).unwrap();
    let result_path = path(P_RESULT_ENV);
    assert!(!result_path.exists());
    let mut callbacks = PrivateCallbacks {
        request: request.clone(),
        count: 0,
        result: None,
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        private_check_request(&request, &invocation)?;
        rustc_driver::run_compiler(&invocation.executed, &mut callbacks);
        require(callbacks.count == 1, "one actual compiler callback")?;
        let row = callbacks
            .result
            .take()
            .ok_or("missing consuming callback")??;
        private_check_request(&request, &invocation)?;
        private_validate(&request, &row)?;
        Ok(row)
    }))
    .unwrap_or_else(|_| Err("compiler or private/call callback panicked".into()));
    let success = result.is_ok();
    let report = PrivateReport {
        request,
        callbacks: callbacks.count,
        result,
    };
    if success {
        private_audit_protocol(&report.request, &invocation, &report);
    }
    write_json(&result_path, &report, REPORT_CAP).unwrap();
    assert!(success, "strict canonical private/call history: {report:?}");
}
fn private_fixture(case: PrivateCase, target: Target) -> corpus::Fixture {
    let source = private_stamps().unwrap();
    let hex = |bytes: [u8; 32]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    corpus::Fixture {
        fixture_id: format!("canonical-private-call-{case:?}-{}", target.cpu()),
        target: target.cpu().into(),
        compiler_input: corpus::CompilerInput {
            package_manifest_sha256: hex(source[1].sha256),
            package_manifest: format!("{BASE}/Cargo.toml"),
            cargo_lock_path: "Cargo.lock".into(),
            cargo_lock_sha256: hex(source[0].sha256),
            source_paths: vec![
                format!("{BASE}/src/lib.rs"),
                format!("{BASE}/src/private_call_history.rs"),
                format!("{BASE}/src/canonical_assertion_source.rs"),
            ],
            source_closure_sha256: String::new(),
            cargo_target: corpus::CargoTarget {
                kind: "lib".into(),
                name: "fe2o3_production_extraction_fixture".into(),
                source_path: "src/lib.rs".into(),
            },
            default_features: false,
            // This existing feature suppresses default roots. Its own fixture
            // has no enabled case cfg; it is still included in exact custody.
            features: vec!["canonical-assertion-source".into()],
            kernel_symbols: case.roots().iter().map(|s| (*s).into()).collect(),
        },
    }
}
fn private_qualify(case: PrivateCase) {
    let scratch = crate::test_temp_dir::TestTempDir::create("canonical-private-call");
    for target in [Target::Gfx942, Target::Gfx950] {
        let directory = scratch.path().join(target.cpu());
        std::fs::create_dir(&directory).unwrap();
        let captured = corpus_cargo::capture(
            &workspace(),
            &private_fixture(case, target),
            &directory,
            &scratch.path().join("cargo-target"),
        )
        .unwrap();
        let invocation = Invocation {
            executed: private_executed(&captured.args, case).unwrap(),
            captured: captured.args,
        };
        let request = PrivateRequest {
            schema: 1,
            route: P_ROUTE.into(),
            run_id: format!("{}-{case:?}-{}", directory.display(), std::process::id()),
            case,
            target,
            captured: args_hash(&invocation.captured),
            executed: args_hash(&invocation.executed),
            cwd: captured.cwd.clone(),
            source: private_stamps().unwrap(),
        };
        let request_path = directory.join("request.json");
        let args_path = directory.join("invocation.json");
        let result_path = directory.join("report.json");
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
            .env(P_REQUEST_ENV, &request_path)
            .env(P_ARGS_ENV, &args_path)
            .env(P_RESULT_ENV, &result_path)
            .args(["--exact", P_CHILD, "--ignored", "--nocapture"]);
        let output = command.output().unwrap();
        let bytes = std::fs::File::open(&result_path).ok().map(|file| {
            let mut bytes = Vec::new();
            file.take(REPORT_CAP as u64 + 1)
                .read_to_end(&mut bytes)
                .unwrap();
            bytes
        });
        let observed = private_decode(output.status.code(), bytes.as_deref(), &request)
            .unwrap_or_else(|error| {
                panic!(
                    "private/call {case:?}/{}: {error}\n{}",
                    target.cpu(),
                    corpus_cargo::diagnostics(&output)
                )
            });
        assert_eq!(request.source, private_stamps().unwrap());
        println!(
            "CANONICAL_PRIVATE_CALL {} {case:?} callbacks=1 roots={} rounds={} functions={} stages={} calls={} memory={:?} pending=19",
            target.cpu(),
            observed.roots.len(),
            observed.rounds.len(),
            observed.checks.functions,
            observed.checks.stages,
            observed.checks.call_aliases,
            observed.checks.memory,
        );
    }
}
#[test]
#[ignore = "actual retained private arrays and shared typed calls, both profiles"]
fn ordinary_rust_canonical_private_shared_calls_both_profiles() {
    private_qualify(PrivateCase::SharedPrivateOpt0);
}
#[test]
#[ignore = "actual cross-block private source storage and typed calls, both profiles"]
fn ordinary_rust_canonical_private_cross_block_both_profiles() {
    private_qualify(PrivateCase::CrossBlockPrivateOpt0);
}
#[test]
#[ignore = "actual normal-MIR typed argument/result calls, both profiles"]
fn ordinary_rust_canonical_private_normal_typed_calls_both_profiles() {
    private_qualify(PrivateCase::NormalTypedCalls);
}
