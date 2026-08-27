use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard, OnceLock};

fn backend_test_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace")
}

#[test]
fn local_marker_adversary_clears_generic_frontend_compilation() {
    let _lock = backend_test_lock();
    let workspace = workspace();
    let output = Command::new(env!("CARGO"))
        .current_dir(&workspace)
        .args(["check", "--locked", "-p", "fe2o3-trusted-item-local-marker"])
        .output()
        .expect("check local-marker adversarial fixture");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "local-marker adversary failed before reaching the backend boundary:\n{stderr}"
    );
}

#[test]
fn renamed_genuine_fixture_uses_the_name_neutral_exact_fill_profile() {
    let fixture = include_str!("fixtures/renamed-genuine/src/main.rs");
    assert!(fixture.contains("pub fn renamed_genuine(mut output: DisjointSlice<f32>)"));
    assert!(fixture.contains("*value = 42.5;"));
    assert!(!fixture.contains("namespace ="));

    let codegen = include_str!("../src/kernel_ir_codegen.rs");
    assert!(
        !codegen.contains("renamed_genuine"),
        "terminal profile selection must not recognize the security fixture by name"
    );
}
