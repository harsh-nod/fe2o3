#[test]
fn generic_handoff_accepts_exact_targets_and_legacy_remains_gfx942_only() {
    use super::validate_compiler_handoff_target;
    for target in ["gfx942:xnack-", "gfx950:xnack-"] {
        assert!(validate_compiler_handoff_target(target, None).is_ok());
        assert_eq!(
            validate_compiler_handoff_target(target, Some("gfx942:xnack-")).is_ok(),
            target == "gfx942:xnack-"
        );
    }
    for target in [
        "gfx942",
        "gfx950",
        "gfx950:xnack+",
        "gfx950:sramecc+:xnack-",
        "gfx951:xnack-",
    ] {
        assert!(validate_compiler_handoff_target(target, None).is_err());
    }
}

use super::*;

#[test]
fn production_driver_requires_one_canonical_overflow_policy() {
    require_canonical_overflow_checks_v1(&[
        "rustc".to_owned(),
        "--crate-name".to_owned(),
        "kernel".to_owned(),
        "-Coverflow-checks=on".to_owned(),
    ])
    .unwrap();

    for rejected in [
        vec!["rustc"],
        vec!["rustc", "-Coverflow-checks=off"],
        vec!["rustc", "-C", "overflow-checks=on"],
        vec![
            "rustc",
            "-Coverflow-checks=on",
            "--codegen=overflow-checks=on",
        ],
        vec!["rustc", "--", "-Coverflow-checks=on"],
    ] {
        let args = rejected.into_iter().map(str::to_owned).collect::<Vec<_>>();
        assert!(
            require_canonical_overflow_checks_v1(&args)
                .unwrap_err()
                .contains("requires exactly one canonical")
        );
    }
}

fn scratch() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "fe2o3-simulation-bundle-output-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&path).unwrap();
    path
}

#[test]
fn simulation_bundle_output_is_create_new_exact_and_private() {
    let root = scratch();
    let output = root.join("kernel.fe2sim");
    publish_new_simulation_bundle_v1(&output, b"exact-bundle").unwrap();
    assert_eq!(std::fs::read(&output).unwrap(), b"exact-bundle");
    assert!(publish_new_simulation_bundle_v1(&output, b"replacement").is_err());
    assert_eq!(std::fs::read(&output).unwrap(), b"exact-bundle");
    #[cfg(unix)]
    {
        use std::os::unix::fs::{PermissionsExt as _, symlink};
        assert_eq!(
            std::fs::metadata(&output).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let link = root.join("link.fe2sim");
        symlink(&output, &link).unwrap();
        assert!(publish_new_simulation_bundle_v1(&link, b"replacement").is_err());
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn simulation_bundle_output_rejects_empty_and_oversized_payloads() {
    let root = scratch();
    assert!(publish_new_simulation_bundle_v1(&root.join("empty"), b"").is_err());
    assert!(
        publish_new_simulation_bundle_v1(
            &root.join("oversized"),
            &vec![0; fe2o3_kernel_ir::MAX_SIMULATION_BUNDLE_BYTES_V1 + 1],
        )
        .is_err()
    );
    std::fs::remove_dir_all(root).unwrap();
}
