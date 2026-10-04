//! Diagnostic coverage only; storage remains outside admitted simulation profiles.
use super::*;

#[test]
fn storage_profile_refusal_keeps_an_exact_preflight_error_document() {
    let error = SimulationPreflightErrorV1::StorageProfileNotAdmitted;
    assert_eq!(
        preflight_kind(&error),
        ErrorKind::PreflightStorageProfileNotAdmitted
    );
    let failure = Failure::preflight(error);
    assert!(failure.1.is_none());
    assert_eq!(
        serde_json::to_value(error_document_view(&failure)).unwrap(),
        serde_json::json!({
            "schema": "fe2o3-simulation-error-v1",
            "status": "error",
            "stage": "preflight",
            "kind": "preflight_storage_profile_not_admitted",
            "message": "module-owned storage layouts require a separate simulation profile"
        })
    );
}

#[test]
fn inert_storage_refusal_keeps_the_exact_unsupported_site() {
    assert_eq!(
        unsupported_code(&UnsupportedFeatureV1::InertStorage),
        UnsupportedFeatureCode::InertStorage
    );
    let failure = Failure::unsupported(
        1,
        &[UnsupportedSimulationSiteV1 {
            function: FunctionId::new("storage_kernel"),
            block: Some(fe2o3_kernel_ir::BlockId(3)),
            operation: Some(7),
            feature: UnsupportedFeatureV1::InertStorage,
        }],
    );
    assert_eq!(
        serde_json::to_value(error_document_view(&failure)).unwrap(),
        serde_json::json!({
            "schema": "fe2o3-simulation-error-v1",
            "status": "error",
            "stage": "preflight",
            "kind": "preflight_unsupported",
            "message": "selected kernel has 1 unsupported reachable site occurrence(s)",
            "unsupported": {
                "total": 1,
                "emitted": 1,
                "truncated": false,
                "sites": [{
                    "function": "storage_kernel",
                    "function_bytes": 14,
                    "function_truncated": false,
                    "block": 3,
                    "operation": 7,
                    "feature": "inert_storage"
                }]
            }
        })
    );
}

#[test]
fn storage_refusal_codes_do_not_relabel_legacy_diagnostics() {
    for (error, expected) in [
        (
            SimulationPreflightErrorV1::AllocationFailure,
            "preflight_allocation_failure",
        ),
        (
            SimulationPreflightErrorV1::PhysicalEntrySymbolicDebugUnavailableV20,
            "preflight_physical_entry_symbolic_debug_unavailable_v20",
        ),
        (
            SimulationPreflightErrorV1::PhysicalGlobalCopyPendingDebugUnavailableV21,
            "preflight_physical_global_copy_pending_debug_unavailable_v21",
        ),
    ] {
        let failure = Failure::preflight(error);
        let document = serde_json::to_value(error_document_view(&failure)).unwrap();
        assert_eq!(document["status"], "error");
        assert_eq!(document["stage"], "preflight");
        assert_eq!(document["kind"], expected);
        assert!(document.get("unsupported").is_none());
    }
    for (feature, expected) in [
        (UnsupportedFeatureV1::InertV12Carrier, "inert_v12_carrier"),
        (
            UnsupportedFeatureV1::InertExecutionV15,
            "inert_execution_v15",
        ),
        (UnsupportedFeatureV1::UnsupportedType, "unsupported_type"),
        (UnsupportedFeatureV1::NonScalarMemory, "non_scalar_memory"),
    ] {
        assert_eq!(
            serde_json::to_value(unsupported_code(&feature)).unwrap(),
            expected
        );
    }
}
