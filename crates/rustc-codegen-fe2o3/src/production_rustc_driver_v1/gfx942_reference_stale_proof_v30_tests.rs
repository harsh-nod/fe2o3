//! Actual-source stale-receipt pair; selected only in isolated protected children.
//! Feature selection changes real Rust MIR and crate metadata, not source file bytes.
use super::*;
use crate::production_reference_effect_join_v2::source_proof_freshness_v1 as observation;

const ORIGINAL_ENV: &str = "FE2O3_TEST_ISA_REFERENCE_ORIGINAL_V30";
const ORIGINAL_HASH_ENV: &str = "FE2O3_TEST_ISA_REFERENCE_ORIGINAL_SHA256_V30";
const PREFIX: &str = "FE2O3_ASSEMBLY_REFERENCE_FRESHNESS_V30 ";
const ORIGINAL_SCHEMA: &str = "fe2o3-test-source-proof-original-v1";

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Original {
    schema: String,
    invocation: PreparedInvocation,
    receipt: observation::Transport,
}

fn current_record(directory: &Path, feature: &str) -> PreparedInvocation {
    assert_compiled_source_is_current();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            &read_bounded(&directory.join("preparation.json"), 64 * 1024).unwrap(),
        )
        .unwrap(),
        preparation_record(directory),
        "current bounded preparation must be independently rederived",
    );
    let actual = derive_record(directory, feature);
    let retained: PreparedInvocation = serde_json::from_slice(
        &read_bounded(
            &directory.join(format!("{feature}.invocation.json")),
            64 * 1024,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(actual, retained);
    actual
}

fn original_receipt(
    directory: &Path,
    feature: &str,
) -> (Option<observation::Transport>, Option<String>) {
    let path = std::env::var_os(ORIGINAL_ENV);
    let expected_hash = std::env::var_os(ORIGINAL_HASH_ENV);
    if feature == FEATURES[0] {
        assert!(
            path.is_none() && expected_hash.is_none(),
            "positive proof must be freshly produced, never replayed"
        );
        return (None, None);
    }
    let path = PathBuf::from(path.expect("negative needs the exact retained positive receipt"));
    assert!(path.is_absolute());
    let expected_hash = expected_hash.unwrap().into_string().unwrap();
    assert!(
        expected_hash.len() == 64
            && expected_hash
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
    let bytes = read_bounded(&path, 64 * 1024).unwrap();
    let actual_hash = super::super::lower_hex_v1(&Sha256::digest(&bytes));
    assert_eq!(
        actual_hash, expected_hash,
        "original receipt transport pin changed"
    );
    let original: Original = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(original.schema, ORIGINAL_SCHEMA);
    assert_eq!(
        original.invocation,
        current_record(directory, FEATURES[0]),
        "old receipt must name the independently rederived original invocation"
    );
    original.receipt.validate().unwrap();
    (Some(original.receipt), Some(actual_hash))
}

struct FreshnessCallbacks {
    calls: usize,
    original: Option<observation::Transport>,
    result: Option<(Result<(), String>, observation::Observation)>,
}
impl Callbacks for FreshnessCallbacks {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        assert_eq!(self.calls, 1, "exactly one compiler callback is required");
        // rustc may run its callback on a compiler-owned thread. Install TLS
        // HERE around the actual source join, never around run_compiler.
        self.result = Some(observation::observe(self.original.take(), || {
            super::super::extract_ranked_memory_in_active_session_v1(tcx, None)
        }));
        Compilation::Stop
    }
}

#[test]
#[ignore = "isolated actual-rustc proof freshness child; requires fresh preparation and protected runtime"]
fn actual_source_reference_freshness_child() {
    let directory = PathBuf::from(std::env::var_os(CHILD_ENV).unwrap());
    assert!(directory.is_absolute());
    let feature = std::env::var(FEATURE_ENV).unwrap();
    checked_feature(&feature).unwrap();
    let actual = current_record(&directory, &feature);
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
    let (original, original_hash) = original_receipt(&directory, &feature);
    let mut callbacks = FreshnessCallbacks {
        calls: 0,
        original,
        result: None,
    };
    rustc_driver::run_compiler(&actual.args, &mut callbacks);
    assert_eq!(callbacks.calls, 1);
    let (result, mut observed) = callbacks
        .result
        .expect("actual source callback not reached");
    let stage = expected_stage(&feature, &result).unwrap();
    assert_eq!(observed.request_count, 1);
    assert!(observed.binding.is_some());
    let captured = if feature == FEATURES[0] {
        assert_eq!(observed.normal_import_count, 1);
        assert_eq!(observed.stale_error, None);
        assert_eq!(observed.original_reimport_count, 0);
        assert!(!observed.normalized_obligation_changed);
        Some(Original {
            schema: ORIGINAL_SCHEMA.into(),
            invocation: current_record(&directory, FEATURES[0]),
            receipt: observed.original_receipt.take().unwrap(),
        })
    } else {
        assert_eq!(observed.normal_import_count, 0);
        assert!(observed.original_receipt.is_none());
        assert_eq!(observed.stale_error, Some("StaleSafeReferenceIdentity"));
        assert_eq!(observed.rejected_import_count, 0);
        assert_eq!(observed.original_reimport_count, 1);
        assert!(observed.normalized_obligation_changed && observed.kernel_mir_changed);
        None
    };
    assert_compiled_source_is_current();
    assert!(
        !fs::read_dir(directory.join("analysis-output"))
            .unwrap()
            .any(|entry| entry.is_ok()),
        "test must stop before artifact emission"
    );
    let value = json!({
        "schema": "fe2o3-test-source-proof-freshness-observation-v1",
        "feature": feature,
        "stage": stage,
        "invocation": actual,
        "diagnostic": result.unwrap_err(),
        "observed": observed,
        "original": captured,
        "original_transport_sha256": original_hash,
        "actual_rustc_callback": true,
        "source_file_bytes_unchanged": true,
        "semantic_change_is_feature_selected": true,
        "proof_subject_kind": "mir",
        "source_hash_is_proof_subject": false,
        "source_admission_complete": false,
        "v17_proof_support": false,
        "grants_artifact_or_launch_authority": false,
        "hardware_observed": false,
    });
    let encoded = serde_json::to_string(&value).unwrap();
    assert!(encoded.len() <= 64 * 1024);
    println!("\n{PREFIX}{encoded}");
}

#[test]
fn source_proof_original_transport_refuses_unknown_fields() {
    assert!(serde_json::from_str::<Original>("{}").is_err());
    assert!(
        serde_json::from_str::<Original>(
            r#"{"schema":"x","invocation":{},"receipt":{},"authority":true}"#,
        )
        .is_err()
    );
}
