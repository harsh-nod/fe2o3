use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("canonical workspace");
        let declared = std::env::var_os("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .map(|path| {
                if path.is_absolute() {
                    path
                } else {
                    workspace.join(path)
                }
            })
            .unwrap_or_else(|| workspace.join("target"));
        let target = declared.canonicalize().expect("canonical Cargo target");
        let path = target.join(format!("example-artifact-root-cli-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).expect("create test directory");
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn inspect(root: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .current_dir(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .canonicalize()
                .expect("canonical workspace"),
        )
        .env("CARGO", "/definitely/not/an/admitted/cargo")
        .args(["examples", "check-artifacts", "fe2o3-fill"])
        .arg(root)
        .output()
        .expect("run artifact inspection")
}

#[test]
fn artifact_inspection_uses_only_the_explicit_admitted_root() {
    let directory = TestDirectory::new();
    let admitted = directory.0.join("admitted");
    let mismatched = directory.0.join("mismatched");
    fs::create_dir(&admitted).expect("create admitted root");
    fs::create_dir(&mismatched).expect("create mismatched root");
    fs::write(admitted.join("fill.hsaco"), b"fixture").expect("write admitted artifact");

    let accepted = inspect(&admitted);
    assert!(
        accepted.status.success(),
        "explicit artifact inspection consulted hostile Cargo or rejected its admitted root:\n{}",
        String::from_utf8_lossy(&accepted.stderr)
    );
    assert!(
        String::from_utf8_lossy(&accepted.stdout)
            .contains("example artifacts: fe2o3-fill: fill.hsaco")
    );

    let rejected = inspect(&mismatched);
    assert!(!rejected.status.success());
    let stderr = String::from_utf8_lossy(&rejected.stderr);
    assert!(
        stderr.contains(&format!(
            "expected artifact for package `fe2o3-fill` was not produced as a regular non-symlink file: {}",
            mismatched.join("fill.hsaco").display()
        )),
        "mismatched artifact root did not fail closed:\n{stderr}"
    );
    assert!(!stderr.contains("cargo metadata"));

    let noncanonical = admitted.join("..").join("admitted");
    let noncanonical_result = inspect(&noncanonical);
    assert!(!noncanonical_result.status.success());
    assert!(
        String::from_utf8_lossy(&noncanonical_result.stderr)
            .contains("artifact directory must already be canonical")
    );

    let linked_root = directory.0.join("linked-root");
    symlink(&admitted, &linked_root).expect("create hostile directory symlink");
    let linked_result = inspect(&linked_root);
    assert!(!linked_result.status.success());
    assert!(
        String::from_utf8_lossy(&linked_result.stderr)
            .contains("artifact directory must be a non-symlink directory")
    );

    let external = directory.0.join("external.hsaco");
    fs::write(&external, b"substituted").expect("write external artifact");
    fs::remove_file(admitted.join("fill.hsaco")).expect("remove admitted artifact");
    symlink(&external, admitted.join("fill.hsaco")).expect("create hostile artifact symlink");
    let symlinked = inspect(&admitted);
    assert!(!symlinked.status.success());
    assert!(
        String::from_utf8_lossy(&symlinked.stderr)
            .contains("was not produced as a regular non-symlink file"),
        "child symlink escaped the admitted directory capability:\n{}",
        String::from_utf8_lossy(&symlinked.stderr)
    );
}
