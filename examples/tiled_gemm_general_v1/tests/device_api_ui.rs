use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-gemm-phase-ui-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&path).expect("create isolated phase UI directory");
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn phase_ui_target_dir(scratch: &Path, outer: Option<&Path>) -> PathBuf {
    outer
        .map(|root| root.join("tutorial-ui/fe2o3-tiled-gemm-general-v1/phase-order"))
        .unwrap_or_else(|| scratch.join("target"))
}

fn copy_phase_ui_fixtures(source: &Path, scratch: &Path) {
    for (kind, count) in [("pass", 1), ("fail", 16)] {
        let relative = Path::new("tests/ui").join(kind);
        let destination = scratch.join(&relative);
        std::fs::create_dir_all(&destination).expect("create phase UI fixture directory");
        let mut fixtures = std::fs::read_dir(source.join(&relative))
            .expect("read phase UI fixture inventory")
            .map(|entry| entry.expect("read phase UI fixture entry").path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
            .collect::<Vec<_>>();
        fixtures.sort();
        assert_eq!(fixtures.len(), count, "phase UI {kind} roster changed");
        for fixture in fixtures {
            std::fs::copy(
                &fixture,
                destination.join(fixture.file_name().expect("fixture file name")),
            )
            .expect("copy exact phase UI source");
            if kind == "fail" {
                let expected = fixture.with_extension("stderr");
                std::fs::copy(
                    &expected,
                    destination.join(expected.file_name().expect("snapshot file name")),
                )
                .expect("copy required phase UI diagnostic snapshot");
            }
        }
    }
}

#[test]
fn general_gemm_capability_enforces_linear_phase_order() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api = source
        .join("device-api")
        .canonicalize()
        .expect("canonical GEMM device API path");
    let scratch = Scratch::new();
    copy_phase_ui_fixtures(source, &scratch.0);
    // trybuild automatically depends on its enclosing package's library. Use a
    // bin-only driver so these Rust typestate tests do not rebuild the typed kernel.
    std::fs::write(
        scratch.0.join("Cargo.toml"),
        format!(
            "[package]\nname = \"fe2o3-gemm-phase-ui-driver\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[workspace]\n\n[dependencies]\ntrybuild = \"1\"\n\n[dev-dependencies]\nfe2o3-gemm-device-v1 = {{ path = {api:?} }}\n\n[[bin]]\nname = \"fe2o3-gemm-phase-ui-driver\"\npath = \"runner.rs\"\n",
        ),
    )
    .expect("write isolated phase UI manifest");
    std::fs::copy(source.join("Cargo.lock"), scratch.0.join("Cargo.lock"))
        .expect("retain resolved phase UI dependency versions");
    std::fs::write(
        scratch.0.join("runner.rs"),
        include_str!("device-api-ui/runner.rs"),
    )
    .expect("write isolated trybuild driver");
    // Resolve the outer path before nested Cargo changes directory.
    let outer = std::env::var_os("CARGO_TARGET_DIR")
        .map(|path| std::path::absolute(path).expect("resolve outer Cargo target directory"));
    let output = Command::new(env!("CARGO"))
        .current_dir(&scratch.0)
        .env("CARGO_MANIFEST_DIR", &scratch.0)
        .env(
            "CARGO_TARGET_DIR",
            phase_ui_target_dir(&scratch.0, outer.as_deref()),
        )
        .env("CARGO_NET_OFFLINE", "true")
        .env("TRYBUILD", "wip")
        // Retain the pinned compiler descriptors supplied by the host-test runner.
        .args([
            "run",
            "--offline",
            "--quiet",
            "--bin",
            "fe2o3-gemm-phase-ui-driver",
        ])
        .output()
        .expect("run isolated phase-order trybuild fixtures");
    assert!(
        output.status.success(),
        "phase-order UI failed ({}):\n{}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn phase_ui_cache_is_nested_and_distinct_from_other_ui_caches() {
    let outer = Path::new("/caller/target");
    let expected = outer.join("tutorial-ui/fe2o3-tiled-gemm-general-v1/phase-order");
    for scratch in [Path::new("/scratch/first"), Path::new("/scratch/second")] {
        assert_eq!(phase_ui_target_dir(scratch, Some(outer)), expected);
        assert_eq!(phase_ui_target_dir(scratch, None), scratch.join("target"));
    }
    for other in [
        outer.to_path_buf(),
        outer.join("tutorial-ui/fe2o3-tiled-gemm-general-v1/native"),
        outer.join("tutorial-ui/fe2o3-tiled-gemm-general-v1/amdgpu"),
    ] {
        assert_ne!(expected, other);
    }
}

#[test]
fn phase_ui_provisioning_retains_every_source_and_diagnostic_byte() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scratch = Scratch::new();
    copy_phase_ui_fixtures(source, &scratch.0);
    for (kind, count) in [("pass", 1), ("fail", 32)] {
        let relative = Path::new("tests/ui").join(kind);
        let entries = std::fs::read_dir(scratch.0.join(&relative))
            .expect("read copied fixture inventory")
            .map(|entry| entry.expect("read copied fixture entry"))
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), count);
        for entry in entries {
            assert_eq!(
                std::fs::read(entry.path()).expect("read copied fixture"),
                std::fs::read(source.join(&relative).join(entry.file_name()))
                    .expect("read original fixture")
            );
        }
    }
}
