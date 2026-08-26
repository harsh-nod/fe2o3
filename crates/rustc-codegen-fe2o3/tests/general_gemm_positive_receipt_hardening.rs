use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[path = "support/cargo_fe2o3.rs"]
mod cargo_fe2o3;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace")
}

fn cargo_target_directory(workspace: &Path) -> PathBuf {
    let Some(configured) = std::env::var_os("CARGO_TARGET_DIR") else {
        return workspace.join("target");
    };
    let configured = PathBuf::from(configured);
    if configured.is_absolute() {
        configured
    } else {
        workspace.join(configured)
    }
}

fn managed_build(workspace: &Path, manifest: &Path, bin: &str, artifacts: &Path) -> Output {
    cargo_fe2o3::qualification_command(workspace)
        .current_dir(workspace)
        .args(["build", "--locked", "--manifest-path"])
        .arg(manifest)
        .args(["--release", "--bin", bin])
        .env(
            "FE2O3_BACKEND",
            cargo_target_directory(workspace).join("debug/librustc_codegen_fe2o3.so"),
        )
        .env("CARGO_TARGET_DIR", cargo_target_directory(workspace))
        .env("FE2O3_TARGET", "gfx942:xnack-")
        .env("FE2O3_QUALIFICATION_ORACLE_V1", "kernel-ir-v1")
        .env("FE2O3_HSACO_DIR", artifacts)
        .output()
        .expect("run managed hostile positive-receipt build")
}

fn contains_regular_file(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if metadata.is_file() {
        return true;
    }
    if !metadata.is_dir() {
        return false;
    }
    std::fs::read_dir(path)
        .expect("read artifact directory")
        .any(|entry| contains_regular_file(&entry.expect("read artifact entry").path()))
}

#[test]
fn hostile_positive_sources_cannot_mint_frontend_correspondence() {
    let workspace = workspace();
    let fixture =
        workspace.join("crates/rustc-codegen-fe2o3/tests/fixtures/general-gemm-semantic-frontend");
    let built = Command::new(env!("CARGO"))
        .current_dir(&workspace)
        .args([
            "build",
            "--locked",
            "-p",
            "rustc-codegen-fe2o3",
            "--features",
            "qualification-oracles-test-only",
        ])
        .output()
        .expect("build codegen backend");
    assert!(
        built.status.success(),
        "backend build failed:\n{}",
        String::from_utf8_lossy(&built.stderr)
    );

    for (bin, source, expected) in [
        (
            "positive-early-return",
            "positive_early_return.rs",
            "the canonical acquire before any normal return",
        ),
        (
            "positive-split-loader-provenance",
            "positive_split_loader_provenance.rs",
            "failed: load derives from slice and index",
        ),
        (
            "positive-store-backedge",
            "positive_store_backedge.rs",
            "the canonical store has no backedge to acquire or phase entry",
        ),
    ] {
        let source = std::fs::read_to_string(fixture.join("src/bin").join(source))
            .expect("read hostile positive source");
        assert!(source.contains("#![forbid(unsafe_code)]"));
        assert!(!source.contains("unsafe {"));

        let artifacts = cargo_target_directory(&workspace)
            .join("general-gemm-positive-receipt-hardening")
            .join(bin);
        let _ = std::fs::remove_dir_all(&artifacts);
        let rejected = managed_build(&workspace, &fixture.join("Cargo.toml"), bin, &artifacts);
        let stderr = String::from_utf8_lossy(&rejected.stderr);
        assert!(
            !rejected.status.success()
                && stderr.contains("general GEMM authenticated MIR import failed:")
                && stderr.contains("general GEMM semantic fact is Unknown/Unproved:")
                && stderr.contains(expected),
            "hostile positive source `{bin}` missed its exact fail-closed importer rejection:\n{stderr}"
        );
        assert!(
            !stderr.contains("reached verified symbolic semantic template")
                && !stderr.contains("mutation-oracle baseline passed")
                && !stderr.contains("artifact authority was issued")
                && !contains_regular_file(&artifacts),
            "hostile positive source `{bin}` acquired correspondence or left artifacts:\n{stderr}"
        );
    }
}
