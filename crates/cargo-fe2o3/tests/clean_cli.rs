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
        mark_owned(&self.generated());
        fs::create_dir_all(self.unrelated().parent().expect("unrelated parent"))
            .expect("create unrelated directory");
        fs::write(self.generated().join("kernel.hsaco"), b"generated")
            .expect("write generated artifact");
        fs::write(self.unrelated(), b"unrelated").expect("write unrelated artifact");
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().expect("run cargo-fe2o3")
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
        command
            .args(args)
            .current_dir(&self.cwd)
            .env_remove("CARGO_TARGET_DIR");
        command
    }
}

fn mark_owned(directory: &std::path::Path) {
    let marker = directory.join(".fe2o3-owned-v1");
    let mut bytes = b"fe2o3-owned-v1\0".to_vec();
    bytes.extend_from_slice(&[0x5a; 16]);
    fs::write(&marker, bytes).expect("write artifact deletion guard");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&marker, fs::Permissions::from_mode(0o600))
            .expect("make artifact deletion guard private");
    }
}

fn mark_target_root_owned(directory: &std::path::Path) {
    let marker = directory.join(".fe2o3-target-root-owned-v1");
    let mut bytes = b"fe2o3-target-root-owned-v1\0".to_vec();
    bytes.extend_from_slice(&[0x6b; 16]);
    fs::write(&marker, bytes).expect("write target-root guard");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&marker, fs::Permissions::from_mode(0o600))
            .expect("make target-root guard private");
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

#[test]
#[cfg(unix)]
fn clean_never_removes_the_complete_selected_target() {
    let project = TempProject::new();
    project.populate_artifacts();

    let output = project.run(&["clean"]);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(project.root.join("target").is_dir());
    assert!(!project.generated().exists());
    assert_eq!(
        fs::read(project.unrelated()).expect("read unrelated output"),
        b"unrelated"
    );
    assert!(project.root.join("Cargo.toml").is_file());
}

#[test]
fn dry_run_preserves_every_target_output() {
    let project = TempProject::new();
    project.populate_artifacts();

    let output = project.run(&["clean", "--dry-run"]);

    assert!(output.status.success());
    assert!(project.generated().join("kernel.hsaco").is_file());
    assert_eq!(
        fs::read(project.unrelated()).expect("read unrelated"),
        b"unrelated"
    );
}

#[test]
#[cfg(unix)]
fn explicit_custom_target_scopes_cleanup_to_fe2o3_artifacts() {
    let project = TempProject::new();
    let custom = project.root.join("custom-target");
    let generated = custom.join("fe2o3");
    let host = custom.join("debug/host");
    fs::create_dir_all(&generated).expect("create generated output");
    mark_owned(&generated);
    fs::create_dir_all(host.parent().expect("host parent")).expect("create host output");
    fs::write(generated.join("kernel.hsaco"), b"generated").expect("write generated output");
    fs::write(&host, b"host").expect("write host output");

    let custom_string = custom.to_str().expect("UTF-8 temporary path");
    let scoped = project.run(&["clean", "--target-dir", custom_string]);
    assert!(
        scoped.status.success(),
        "{}",
        String::from_utf8_lossy(&scoped.stderr)
    );
    assert!(!generated.exists());
    assert_eq!(fs::read(&host).expect("read host output"), b"host");

    assert!(custom.is_dir());
    assert_eq!(fs::read(&host).expect("read host output"), b"host");
}

#[test]
#[cfg(unix)]
fn inherited_cargo_target_dir_is_cleaned_without_touching_workspace_target() {
    let project = TempProject::new();
    project.populate_artifacts();
    let custom = project.root.join("environment-target");
    fs::create_dir_all(custom.join("fe2o3")).expect("create custom generated output");
    mark_owned(&custom.join("fe2o3"));
    fs::write(custom.join("fe2o3/kernel.hsaco"), b"custom").expect("write custom output");

    let output = project
        .command(&["clean"])
        .env("CARGO_TARGET_DIR", &custom)
        .output()
        .expect("run clean with CARGO_TARGET_DIR");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!custom.join("fe2o3").exists());
    assert!(project.generated().join("kernel.hsaco").is_file());
}

#[test]
fn invocation_config_controls_metadata_and_cleanup_target_consistently() {
    let project = TempProject::new();
    project.populate_artifacts();
    let configured = project.root.join("configured-target");
    fs::create_dir_all(configured.join("fe2o3")).expect("create configured artifacts");
    mark_owned(&configured.join("fe2o3"));
    fs::write(configured.join("fe2o3/configured.hsaco"), b"configured")
        .expect("write configured artifact");
    let config = format!("build.target-dir=\"{}\"", configured.display());

    let output = project.run(&["clean", "--dry-run", "--config", &config]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains(&configured.display().to_string()));
    assert!(configured.join("fe2o3/configured.hsaco").is_file());
    assert!(project.generated().join("kernel.hsaco").is_file());
}

#[test]
fn manifest_path_is_honored_and_package_cleanup_is_rejected() {
    let project = TempProject::virtual_workspace();
    project.populate_artifacts();
    let manifest = project.root.join("member/Cargo.toml");
    let manifest = manifest.to_str().expect("UTF-8 temporary manifest");

    let dry_run = project.run(&["clean", "--dry-run", "--manifest-path", manifest]);
    assert!(
        dry_run.status.success(),
        "{}",
        String::from_utf8_lossy(&dry_run.stderr)
    );

    let rejected = project.run(&["clean", "--package", "clean-member"]);
    assert!(!rejected.status.success());
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("ambiguous argument"),
        "{}",
        String::from_utf8_lossy(&rejected.stderr)
    );
    assert!(project.generated().join("kernel.hsaco").is_file());
}

#[test]
#[cfg(unix)]
fn clean_refuses_a_symlinked_explicit_target() {
    use std::os::unix::fs::symlink;

    let project = TempProject::new();
    let outside = project.root.join("outside-target");
    let selected = project.root.join("selected-target");
    fs::create_dir_all(outside.join("fe2o3")).expect("create outside target");
    fs::write(outside.join("keep"), b"outside").expect("write outside sentinel");
    symlink(&outside, &selected).expect("create target symlink");
    let selected = selected.to_str().expect("UTF-8 temporary target");

    let output = project.run(&["clean", "--target-dir", selected]);

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("symlink"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read(outside.join("keep")).expect("outside sentinel remains"),
        b"outside"
    );
}

#[test]
fn missing_split_path_never_consumes_dry_run() {
    let project = TempProject::new();
    project.populate_artifacts();

    for option in ["--target-dir", "--manifest-path"] {
        let output = project.run(&["clean", option, "--dry-run"]);
        assert!(
            !output.status.success(),
            "accepted missing value for {option}"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("requires a non-empty path"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(project.root.join("Cargo.toml").is_file());
        assert!(project.generated().is_dir());
    }
}

#[test]
fn generated_clean_rejects_unowned_directory_even_in_dry_run() {
    let project = TempProject::new();
    fs::create_dir_all(project.generated()).expect("create unowned output");
    fs::write(project.generated().join("keep"), b"unowned").expect("write sentinel");

    for args in [vec!["clean"], vec!["clean", "--dry-run"]] {
        let output = project.run(&args);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("unowned"));
        assert_eq!(
            fs::read(project.generated().join("keep")).expect("sentinel remains"),
            b"unowned"
        );
    }
}

#[test]
fn clean_never_grants_parent_deletion_for_root_or_project_targets() {
    let project = TempProject::new();
    let candidates = [
        PathBuf::from("/"),
        project.root.clone(),
        project.root.parent().expect("project parent").to_path_buf(),
    ];

    for target in candidates {
        let output = project.run(&[
            "clean",
            "--dry-run",
            "--target-dir",
            target.to_str().expect("UTF-8 temporary target"),
        ]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(project.root.join("Cargo.toml").is_file());
    }
}

#[test]
fn stale_root_guards_in_project_descendants_never_authorize_parent_deletion() {
    let project = TempProject::new();
    for relative in ["src", "src/nested", "arbitrary/project/descendant"] {
        let target = project.root.join(relative);
        let generated = target.join("fe2o3");
        fs::create_dir_all(&generated).expect("create descendant artifact directory");
        fs::write(target.join("keep"), relative).expect("write descendant sentinel");
        mark_owned(&generated);
        mark_target_root_owned(&target);

        let output = project.run(&[
            "clean",
            "--target-dir",
            target.to_str().expect("UTF-8 target"),
        ]);
        assert!(
            output.status.success(),
            "{}: {}",
            relative,
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(target.is_dir(), "removed project descendant {relative}");
        assert_eq!(
            fs::read_to_string(target.join("keep")).expect("read descendant sentinel"),
            relative
        );
        assert!(!generated.exists());
    }
}

#[test]
fn all_target_is_rejected_without_removing_any_output() {
    let project = TempProject::new();
    project.populate_artifacts();

    let output = project.run(&["clean", "--all-target"]);

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("use an explicit `cargo clean`"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(project.generated().join("kernel.hsaco").is_file());
    assert_eq!(
        fs::read(project.unrelated()).expect("read unrelated output"),
        b"unrelated"
    );
}
