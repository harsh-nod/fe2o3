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
            "fe2o3-vecadd-reference-negative-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(path.join("src")).expect("create semantic-negative scratch");
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace")
}

fn provisioned_extractor(workspace: &Path) -> PathBuf {
    for variable in ["FE2O3_RUSTC_EXTRACT_BIN", "FE2O3_RUSTC_EXTRACTOR"] {
        if let Some(value) = std::env::var_os(variable) {
            let path = PathBuf::from(value);
            assert!(path.is_file(), "{variable} does not name a file: {path:?}");
            return path;
        }
    }

    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("target"));
    let candidate = target.join("debug/fe2o3-rustc-extract");
    assert!(
        candidate.is_file(),
        "mandatory vecadd semantic proof requires the repository-provisioned extractor at {candidate:?}",
    );
    candidate
}

fn extractor_library_path(extractor: &Path) -> std::ffi::OsString {
    let extractor_directory = extractor
        .parent()
        .expect("extractor has a parent directory");
    let (runtime, dependencies) = if extractor_directory
        .file_name()
        .is_some_and(|name| name == "deps")
    {
        (
            extractor_directory
                .parent()
                .expect("deps has a parent directory")
                .to_owned(),
            extractor_directory.to_owned(),
        )
    } else {
        (
            extractor_directory.to_owned(),
            extractor_directory.join("deps"),
        )
    };
    assert!(
        runtime.is_dir(),
        "extractor runtime directory is absent: {runtime:?}"
    );
    assert!(
        dependencies.is_dir(),
        "extractor dependency directory is absent: {dependencies:?}"
    );

    let sysroot = Command::new("rustup")
        .args(["run", "nightly-2026-04-03", "rustc", "--print", "sysroot"])
        .output()
        .expect("query pinned nightly sysroot");
    assert!(
        sysroot.status.success(),
        "pinned nightly sysroot query failed"
    );
    let sysroot = PathBuf::from(
        std::str::from_utf8(&sysroot.stdout)
            .expect("sysroot is UTF-8")
            .trim(),
    );

    let mut paths = vec![runtime, dependencies, sysroot.join("lib")];
    if let Some(inherited) = std::env::var_os("LD_LIBRARY_PATH") {
        paths.extend(std::env::split_paths(&inherited));
    }
    std::env::join_paths(paths).expect("join extractor runtime library path")
}

#[test]
fn gpu_x_plus_one_mutation_is_rejected_by_the_authenticated_cpu_reference() {
    let workspace = workspace();
    let extractor = provisioned_extractor(&workspace);
    let extractor_library_path = extractor_library_path(&extractor);

    let scratch = Scratch::new();
    let source = include_str!("../src/lib.rs");
    let mutated = source.replacen("$lhs + $rhs", "$lhs + 1.0_f32", 1);
    assert_ne!(mutated, source, "vecadd GPU add mutation was not applied");
    assert!(mutated.contains("reference = vecadd_cpu_reference"));

    std::fs::write(
        scratch.0.join("Cargo.toml"),
        format!(
            "[package]\nname = \"fe2o3-vecadd-semantic-negative\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[workspace]\n\n[dependencies]\nfe2o3-device = {{ path = {:?} }}\n\n[target.'cfg(not(target_arch = \"amdgpu\"))'.dependencies]\nfe2o3-host = {{ path = {:?} }}\n",
            workspace.join("crates/fe2o3-device"),
            workspace.join("crates/fe2o3-host"),
        ),
    )
    .expect("write semantic-negative manifest");
    std::fs::write(scratch.0.join("src/lib.rs"), mutated).expect("write mutated vecadd source");
    std::fs::write(
        scratch.0.join("src/vecadd_body.rs"),
        include_str!("../src/vecadd_body.rs"),
    )
    .expect("write shared vecadd body");

    let output = Command::new("rustup")
        .args(["run", "nightly-2026-04-03", "cargo"])
        .current_dir(&workspace)
        .env(
            "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2",
            "55".repeat(32),
        )
        .env("FE2O3_CRATE_BINDING_ID_V1", "77".repeat(32))
        .env("FE2O3_EXTRACT_RANKED_MEMORY_V1", "1")
        .env(
            "FE2O3_EXTRACT_CRATE_V1",
            "fe2o3_vecadd_semantic_negative",
        )
        .env("RUSTC_WORKSPACE_WRAPPER", extractor)
        .env("LD_LIBRARY_PATH", extractor_library_path)
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env(
            "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32",
        )
        .args([
            "check",
            "--offline",
            "-Zbuild-std=core",
            "--manifest-path",
        ])
        .arg(scratch.0.join("Cargo.toml"))
        .args(["--target", "amdgcn-amd-amdhsa", "--target-dir"])
        .arg(workspace.join("target"))
        .output()
        .expect("run semantic-negative compiler extraction");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "mutated GPU graph was accepted");
    assert!(
        stderr.contains("source-to-proof V2 effect mismatch") && stderr.contains("RHS mismatch"),
        "GPU x+1 mutation did not fail at semantic equivalence:\n{stderr}",
    );
}
