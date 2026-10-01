//! No-spawn qualification adapters for the ordinary retained-account route.
//! Root owns fresh setup, independent rustc processes, all stream custody and
//! the comparison of complete baseline/retained target records.
use super::*;

const MODE_ENV: &str = "FE2O3_TEST_RETAINED_TARGET_ACCOUNT_MODE_V1";
const RECORD: &str = "retained-target-invocation.json";
const REPORT_CAP: usize = 4 * 1024 * 1024;
const FRAME: &str = "FE2O3_RETAINED_TARGET_ACCOUNT_V1 ";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Baseline,
    Retained,
}
fn parse_mode(text: &str) -> Result<Mode, String> {
    match text {
        "baseline" => Ok(Mode::Baseline),
        "retained" => Ok(Mode::Retained),
        _ => Err("unknown retained target-account case".into()),
    }
}
impl Mode {
    fn name(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::Retained => "retained",
        }
    }
}

fn actual_directory() -> PathBuf {
    let path = PathBuf::from(std::env::var_os(CHILD_ENV).expect("missing root-prepared directory"));
    assert_eq!(
        path.canonicalize().unwrap(),
        path,
        "root-prepared directory must be canonical"
    );
    assert!(path.is_dir());
    path
}
fn exact_fixture() -> [String; 3] {
    let checks: [(&str, &[u8]); 3] = [
        (
            "src/lib.rs",
            include_bytes!("../../tests/fixtures/assembly-authoring-v30/src/lib.rs"),
        ),
        (
            "Cargo.toml",
            include_bytes!("../../tests/fixtures/assembly-authoring-v30/Cargo.toml"),
        ),
        (
            "Cargo.lock",
            include_bytes!("../../tests/fixtures/assembly-authoring-v30/Cargo.lock"),
        ),
    ];
    checks.map(|(relative, expected)| {
        let actual = read_bounded(&fixture().join(relative), 1024 * 1024).unwrap();
        assert_eq!(actual, expected, "current selected fixture differs");
        Sha256::digest(actual)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    })
}
fn encode_report(value: &Value, cap: usize) -> Result<Vec<u8>, String> {
    // Serialization temporaries are not part of the source compiler account.
    // Each copied LLVM/KIR payload is independently capped by the observer.
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    if bytes.len() > cap {
        return Err("retained target report cap".into());
    }
    Ok(bytes)
}
fn require_no_output<T, E: std::fmt::Display>(
    entries: impl IntoIterator<Item = Result<T, E>>,
) -> Result<(), String> {
    for entry in entries {
        entry.map_err(|error| format!("analysis-output enumeration failed: {error}"))?;
        return Err("callback emitted an analysis-output entry".into());
    }
    Ok(())
}

fn write_new(path: &Path, bytes: &[u8]) {
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .custom_flags(0o2000000 | 0o400000)
        .open(path)
        .unwrap();
    output.write_all(bytes).unwrap();
    output.sync_all().unwrap();
}

#[test]
#[ignore = "root-owned bounded setup only; no subprocess is spawned"]
fn prepare_retained_target_invocation() {
    let directory = actual_directory();
    let source = exact_fixture();
    let (args, binding, observation) = invocation(&directory);
    assert_eq!(exact_fixture(), source);
    let record = json!({
        "schema": "fe2o3-retained-target-invocation-v1",
        "args": args, "crate_binding": binding, "cargo_observation": observation,
        "fixture_sha256": source,
    });
    write_new(
        &directory.join(RECORD),
        &encode_report(&record, 64 * 1024).unwrap(),
    );
}

struct TargetCallbacks {
    mode: Mode,
    result: Option<Result<Value, String>>,
}
impl Callbacks for TargetCallbacks {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        assert!(self.result.is_none(), "one actual frontend callback");
        self.result = Some((|| {
            let transaction = super::super::transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let mut target_account =
                crate::production_target_account::new().map_err(|e| e.to_string())?;
            target_account.with_budget(|budget| {
                transaction.observe_ordinary_target_account_for_test_v1(
                    self.mode == Mode::Retained,
                    budget,
                )
            })
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "one fresh root-supervised rustc process for each baseline/retained case"]
fn actual_retained_target_account() {
    let directory = actual_directory();
    let mode = parse_mode(&std::env::var(MODE_ENV).expect("missing exact case")).unwrap();
    let source = exact_fixture();
    let retained: Value =
        serde_json::from_slice(&read_bounded(&directory.join(RECORD), 64 * 1024).unwrap()).unwrap();
    let (args, binding, observation) = invocation(&directory);
    assert_eq!(
        retained,
        json!({
            "schema": "fe2o3-retained-target-invocation-v1",
            "args": args, "crate_binding": binding, "cargo_observation": observation,
            "fixture_sha256": source,
        })
    );
    assert_eq!(std::env::var(CRATE_BINDING_ID_ENV_V1).unwrap(), binding);
    assert_eq!(
        std::env::var(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2).unwrap(),
        observation
    );
    assert_eq!(
        std::env::var("CARGO_MANIFEST_DIR").unwrap(),
        fixture().canonicalize().unwrap().to_str().unwrap()
    );
    assert_eq!(
        std::env::var("CARGO_PKG_NAME").unwrap(),
        "fe2o3-assembly-authoring-v30-fixture"
    );
    assert_eq!(std::env::var("CARGO_PKG_VERSION").unwrap(), "0.0.0");
    assert_eq!(std::env::var("CARGO_PRIMARY_PACKAGE").unwrap(), "1");
    assert_eq!(std::env::var("CARGO_CRATE_NAME").unwrap(), CRATE_NAME);
    let path = directory.join(format!("retained-target-{}.json", mode.name()));
    assert!(
        !path.try_exists().unwrap(),
        "report is create-new, never a retry slot"
    );
    eprintln!(
        "fe2o3-retained-target-account-start-v1 case={}",
        mode.name()
    );
    let mut callbacks = TargetCallbacks { mode, result: None };
    super::super::require_canonical_overflow_checks_v1(&args).unwrap();
    rustc_driver::run_compiler(&args, &mut callbacks);
    assert_eq!(exact_fixture(), source);
    require_no_output(fs::read_dir(directory.join("analysis-output")).unwrap()).unwrap();
    let result = callbacks
        .result
        .expect("actual source callback was not reached");
    let accepted = result.is_ok();
    let (observed, failure) = match result {
        Ok(value) => (Some(value), None),
        Err(error) => (None, Some(error.chars().take(4096).collect::<String>())),
    };
    let record = json!({
        "schema": "fe2o3-retained-target-account-observation-v1",
        "case": mode.name(), "accepted": accepted, "fixture_sha256": source,
        "observation": observed, "failure": failure,
        "hardware_observed": false, "normal_bf16_admission_changed": false,
        "whole_memory_envelope": false,
    });
    let bytes = encode_report(&record, REPORT_CAP).unwrap();
    write_new(&path, &bytes);
    println!("\n{FRAME}{}", std::str::from_utf8(&bytes).unwrap());
    assert!(
        accepted,
        "ordinary target route refused; retained report is not success"
    );
}

#[test]
fn qualification_cases_are_closed_and_do_not_select_a_new_target_policy() {
    assert_eq!(parse_mode("baseline").unwrap(), Mode::Baseline);
    assert_eq!(parse_mode("retained").unwrap(), Mode::Retained);
    for value in ["", "Retained", "bf16", "retained ", "baseline\n", "gfx950"] {
        assert!(parse_mode(value).is_err());
    }
}
#[test]
fn report_cap_counts_exact_utf8_bytes_without_rounding() {
    let value = json!({"x": "λ", "counter": "18446744073709551615"});
    let bytes = serde_json::to_vec(&value).unwrap();
    assert_eq!(encode_report(&value, bytes.len()).unwrap(), bytes);
    assert!(encode_report(&value, bytes.len() - 1).is_err());
}

#[test]
fn no_output_postflight_refuses_entries_and_enumeration_errors() {
    assert!(require_no_output::<(), &str>([]).is_ok());
    assert!(require_no_output([Ok::<_, &str>(())]).is_err());
    assert!(
        require_no_output::<(), _>([Err("inert enumeration error")])
            .unwrap_err()
            .contains("enumeration failed")
    );
}
