use super::*;

fn scratch() -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "fe2o3-diagnostic-kir-v17-output-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::create_dir(&path).unwrap();
    path
}

#[test]
fn raw_program_output_is_exact_create_new_private_and_not_a_bundle() {
    // This is deliberately a publication-mechanics test, NOT canonical/source
    // admission. Actual-source ordinary export qualification is separate.
    let root = scratch();
    let output = root.join("program.kir");
    let bytes = b"publication-mechanics-only";
    publish_new_diagnostic_kir_v17(&output, bytes).unwrap();
    assert_eq!(std::fs::read(&output).unwrap(), bytes);
    let error = publish_new_diagnostic_kir_v17(&output, b"replacement").unwrap_err();
    assert!(error.contains("diagnostic canonical KIR V17"));
    assert!(!error.contains("simulation bundle"));
    assert_eq!(std::fs::read(&output).unwrap(), bytes);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{PermissionsExt as _, symlink};
        assert_eq!(
            std::fs::metadata(&output).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let link = root.join("link.kir");
        symlink(&output, &link).unwrap();
        assert!(publish_new_diagnostic_kir_v17(&link, b"replacement").is_err());
        assert_eq!(std::fs::read(&output).unwrap(), bytes);
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn raw_program_output_rejects_empty_or_oversized_before_creation() {
    let root = scratch();
    let empty = root.join("empty.kir");
    let oversized = root.join("oversized.kir");
    for (path, bytes) in [
        (&empty, Vec::new()),
        (
            &oversized,
            vec![0; fe2o3_kernel_ir::MAX_MODULE_BYTES_V1 + 1],
        ),
    ] {
        let error = publish_new_diagnostic_kir_v17(path, &bytes).unwrap_err();
        assert!(error.contains("empty or oversized diagnostic canonical KIR V17"));
        assert!(!path.exists());
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn program_export_uses_one_private_source_owner_and_no_production_fallback() {
    // Source census supplements, and does not substitute for, actual callbacks.
    let source = include_str!("../production_pipeline/ordered_program_diagnostic_v32.rs");
    let stages = [
        ".import_semantic_mir()",
        "ProductionCompilation::construct_semantic_middle_end",
        "ProductionCompilation::construct_semantic_ssa",
        "validate_production_v1_semantic_ownership_evidence",
        "source_launch_roster_for_ranked_inputs_v1",
        "try_materialize_with_budget",
    ];
    let mut prior = 0;
    for stage in stages {
        assert_eq!(source.matches(stage).count(), 1, "{stage}");
        let position = source.find(stage).unwrap();
        assert!(position > prior, "{stage}");
        prior = position;
    }
    assert!(source.contains("SemanticMirWireVersionV1::V32"));
    assert!(source.contains("ProductionOrderedProgramPreRankedKirOwnerV17"));
    for unavailable in [
        ".verify_general_kernel_checks(",
        ".lower_production_target(",
        ".publish_worker_handoff(",
        ".into_simulation_bundle_",
        "ProductionOrderedRegionPreRankedKirOwnerV16",
        "from_module",
    ] {
        assert!(!source.contains(unavailable), "{unavailable}");
    }
    let driver = include_str!("ordered_program_diagnostic_export_v17.rs");
    assert_eq!(driver.matches(".observe_ordered_program_v32()?").count(), 1);
    assert!(driver.contains("DebugSourceCaptureRequestV2::Disabled"));
    assert!(driver.contains("executable.canonical().canonical_bytes()"));
    assert!(!driver.contains("from_module"));
    assert!(!driver.contains("observe_ordered_region_v31"));
}
