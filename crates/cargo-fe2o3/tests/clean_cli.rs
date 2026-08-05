use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{self, Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TempProject {
    root: PathBuf,
    cwd: PathBuf,
}

impl TempProject {
    fn new() -> Self {
        let root = Self::empty_root();

        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"clean-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace]\n",
        )
        .expect("write standalone Cargo manifest");
        fs::create_dir(root.join("src")).expect("create source directory");
        fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("write source file");

        let cwd = root.join("src");
        Self { root, cwd }
    }

    fn virtual_workspace() -> Self {
        let root = Self::empty_root();
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"member\"]\nresolver = \"2\"\n",
        )
        .expect("write virtual workspace manifest");
        let member = root.join("member");
        fs::create_dir_all(member.join("src")).expect("create member source directory");
        fs::write(
            member.join("Cargo.toml"),
            "[package]\nname = \"clean-member\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .expect("write member manifest");
        fs::write(member.join("src/main.rs"), "fn main() {}\n").expect("write member source");

        Self {
            root,
            cwd: member.join("src"),
        }
    }

    fn empty_root() -> PathBuf {
        loop {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let path =
                env::temp_dir().join(format!("cargo-fe2o3-clean-cli-test-{}-{id}", process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return path,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("failed to create temporary project: {error}"),
            }
        }
    }

    fn generated(&self) -> PathBuf {
        self.root.join("target/fe2o3")
    }

    fn unrelated(&self) -> PathBuf {
        self.root.join("target/debug/unrelated")
    }

    fn populate_artifacts(&self) {
        fs::create_dir_all(self.generated()).expect("create generated directory");
        fs::create_dir_all(self.unrelated().parent().expect("unrelated parent"))
            .expect("create unrelated directory");
        fs::write(self.generated().join("kernel.hsaco"), b"generated")
            .expect("write generated artifact");
        fs::write(self.unrelated(), b"unrelated").expect("write unrelated artifact");
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
            .args(args)
            .current_dir(&self.cwd)
            .output()
            .expect("run cargo-fe2o3")
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn direct_invocation_dry_run_discovers_standalone_project() {
    let project = TempProject::new();
    project.populate_artifacts();

    let output = project.run(&["clean", "--dry-run"]);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stderr).expect("UTF-8 stderr"),
        format!(
            "cargo fe2o3 clean: would remove the opened artifact directory resolved from {}\n",
            project.generated().display()
        )
    );
    assert!(project.generated().join("kernel.hsaco").is_file());
    assert_eq!(
        fs::read(project.unrelated()).expect("read unrelated"),
        b"unrelated"
    );
}

#[test]
#[cfg(unix)]
fn cargo_prefixed_invocation_deletes_only_fe2o3_output() {
    let project = TempProject::new();
    project.populate_artifacts();

    let output = project.run(&["fe2o3", "clean"]);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stderr).expect("UTF-8 stderr"),
        format!(
            "cargo fe2o3 clean: removed the opened artifact directory originally resolved from {}\n",
            project.generated().display()
        )
    );
    assert!(!project.generated().exists());
    assert_eq!(
        fs::read(project.unrelated()).expect("read unrelated"),
        b"unrelated"
    );
}

#[test]
fn virtual_workspace_is_discovered_from_member_directory() {
    let project = TempProject::virtual_workspace();
    project.populate_artifacts();

    let output = project.run(&["clean", "--dry-run"]);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stderr).expect("UTF-8 stderr"),
        format!(
            "cargo fe2o3 clean: would remove the opened artifact directory resolved from {}\n",
            project.generated().display()
        )
    );
    assert!(project.generated().join("kernel.hsaco").is_file());
    assert_eq!(
        fs::read(project.unrelated()).expect("read unrelated"),
        b"unrelated"
    );
}

#[test]
fn direct_and_cargo_prefixed_help_are_equivalent() {
    let direct = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .arg("--help")
        .output()
        .expect("run direct help");
    let cargo_prefixed = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .args(["fe2o3", "--help"])
        .output()
        .expect("run Cargo-prefixed help");

    assert!(direct.status.success());
    assert_eq!(cargo_prefixed.status, direct.status);
    assert_eq!(cargo_prefixed.stdout, direct.stdout);
    assert_eq!(cargo_prefixed.stderr, direct.stderr);
}
