use std::env;
use std::ffi::OsString;
use std::fs;
use std::os::unix::ffi::OsStringExt as _;
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};
use std::process::{self, Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    tool: PathBuf,
}

impl Fixture {
    fn new(exit_failure: bool) -> Self {
        loop {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let root = env::temp_dir().join(format!("cargo-fe2o3-profile-{}-{id}", process::id()));
            match fs::create_dir(&root) {
                Ok(()) => {
                    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
                    let tool = root.join("rocprofv3");
                    let behavior = if exit_failure {
                        "raise SystemExit(7)"
                    } else {
                        r#"
out = args[args.index("--output-directory") + 1]
os.makedirs(out, exist_ok=True)
with open(os.path.join(out, "capture_results.json"), "wb") as stream:
    stream.write(b'{"collector":"fixture"}')
target = args[args.index("--") + 1:]
raise SystemExit(subprocess.run(target, check=False).returncode)
"#
                    };
                    write_tool(&tool, behavior);
                    return Self { root, tool };
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("failed to create fixture: {error}"),
            }
        }
    }

    fn output(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    fn plan(&self, output: &Path, target_args: &[&str]) -> Output {
        self.plan_with_options(output, &[], target_args)
    }

    fn plan_with_options(
        &self,
        output: &Path,
        profile_options: &[&str],
        target_args: &[&str],
    ) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
        command.args(["profile", "--tool", self.tool.to_str().unwrap()]);
        command.args(profile_options);
        command.args(["--output-dir", output.to_str().unwrap(), "--", "/bin/true"]);
        command.args(target_args).output().unwrap()
    }

    fn replace_behavior(&self, behavior: &str) {
        write_tool(&self.tool, behavior);
    }
}

fn write_tool(path: &Path, behavior: &str) {
    fs::write(
        path,
        format!(
            r#"#!/usr/bin/env python3
# reviewed fixture surfaces: --kernel-trace --advanced-thread-trace
import os
import subprocess
import sys
import time
args = sys.argv[1:]
{behavior}
"#
        ),
    )
    .unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn authorization(output: &Output) -> String {
    let stdout = String::from_utf8(output.stdout.clone()).unwrap();
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("collection-authorization: "))
        .expect("plan authorization")
        .to_owned()
}

fn field(output: &Output, name: &str) -> String {
    let stdout = String::from_utf8(output.stdout.clone()).unwrap();
    stdout
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{name}: ")))
        .unwrap_or_else(|| panic!("missing {name}"))
        .to_owned()
}

fn collect(fixture: &Fixture, output: &Path, auth: &str, target_args: &[&str]) -> Output {
    collect_with_options(fixture, output, auth, &[], target_args)
}

fn collect_with_options(
    fixture: &Fixture,
    output: &Path,
    auth: &str,
    profile_options: &[&str],
    target_args: &[&str],
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
    command.args([
        "profile",
        "--collect",
        "--authorize-collection",
        auth,
        "--tool",
        fixture.tool.to_str().unwrap(),
    ]);
    command.args(profile_options);
    command.args(["--output-dir", output.to_str().unwrap(), "--", "/bin/true"]);
    command.args(target_args).output().unwrap()
}

#[test]
fn dry_run_is_inert_and_reports_capabilities_without_claiming_observation() {
    let fixture = Fixture::new(false);
    let output_directory = fixture.output("capture");
    let output = fixture.plan(&output_directory, &["argument with space"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("authority: plan-only"));
    assert!(stdout.contains("stateful-action: not-executed"));
    assert!(stdout.contains("dispatch-observability-origin: unavailable"));
    assert!(stdout.contains("collector-runtime-limitation:"));
    assert!(!output_directory.exists());
}

#[test]
fn exact_authorization_collects_without_a_shell_and_writes_a_bounded_manifest() {
    let fixture = Fixture::new(false);
    let output_directory = fixture.output("capture");
    let marker = fixture.output("injected");
    let payload = format!(";touch {}", marker.display());
    let plan = fixture.plan(&output_directory, &[&payload]);
    assert!(plan.status.success());
    let output = collect(
        &fixture,
        &output_directory,
        &authorization(&plan),
        &[&payload],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let evidence = String::from_utf8(output.stdout).unwrap();
    assert!(evidence.contains("outcome: collector-completed-artifacts-unvalidated"));
    assert!(evidence.contains("dispatch-observability-origin: unavailable"));
    assert!(!marker.exists());
    let manifest =
        fs::read_to_string(output_directory.join("fe2o3-profile-manifest-v1.txt")).unwrap();
    assert!(manifest.contains("schema: fe2o3-profile-artifact-manifest-v1"));
    assert!(manifest.contains("capture_results.json"));
    assert_eq!(
        fs::metadata(&output_directory)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
}

#[test]
fn authorization_is_bound_to_exact_target_argv_and_output_path() {
    let fixture = Fixture::new(false);
    let output_directory = fixture.output("capture");
    let plan = fixture.plan(&output_directory, &["first"]);
    assert!(plan.status.success());
    let output = collect(
        &fixture,
        &output_directory,
        &authorization(&plan),
        &["second"],
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("does not match this exact plan")
    );
    assert!(!output_directory.exists());
}

#[test]
fn semantic_configuration_excludes_output_routing_but_authorization_binds_it() {
    let fixture = Fixture::new(false);
    let first = fixture.plan(&fixture.output("first"), &[]);
    let second = fixture.plan(&fixture.output("second"), &[]);
    assert!(first.status.success() && second.status.success());
    assert_eq!(
        field(&first, "configuration-identity"),
        field(&second, "configuration-identity")
    );
    assert_ne!(authorization(&first), authorization(&second));
}

#[test]
fn collector_failure_cleans_only_the_owned_new_directory() {
    let fixture = Fixture::new(true);
    let output_directory = fixture.output("capture");
    let plan = fixture.plan(&output_directory, &[]);
    let output = collect(&fixture, &output_directory, &authorization(&plan), &[]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("output-cleanup: complete")
    );
    assert!(!output_directory.exists());
    assert!(fixture.root.exists());
}

#[test]
fn timeout_kills_the_collector_process_group_and_cleans_output() {
    let fixture = Fixture::new(false);
    let pid_file = fixture.output("descendant-pid");
    fixture.replace_behavior(&format!(
        r#"
child = subprocess.Popen(["/bin/sleep", "30"])
with open({:?}, "w", encoding="utf-8") as stream:
    stream.write(str(child.pid))
    stream.flush()
time.sleep(30)
"#,
        pid_file
    ));
    let output_directory = fixture.output("capture");
    let options = ["--timeout-ms", "75"];
    let plan = fixture.plan_with_options(&output_directory, &options, &[]);
    assert!(plan.status.success());
    let output = collect_with_options(
        &fixture,
        &output_directory,
        &authorization(&plan),
        &options,
        &[],
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("outcome: timeout")
    );
    assert!(!output_directory.exists());
    let pid = fs::read_to_string(&pid_file).unwrap();
    let process_path = PathBuf::from(format!("/proc/{}", pid.trim()));
    for _ in 0..100 {
        if !process_path.exists() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("collector descendant survived process-group timeout");
}

#[test]
fn output_and_storage_overflow_are_bounded_and_cleaned() {
    let fixture = Fixture::new(false);
    fixture.replace_behavior(
        r#"
sys.stdout.write("x" * 8192)
sys.stdout.flush()
time.sleep(30)
"#,
    );
    let output_directory = fixture.output("stdout-overflow");
    let options = ["--stdout-limit", "64", "--timeout-ms", "2000"];
    let plan = fixture.plan_with_options(&output_directory, &options, &[]);
    let output = collect_with_options(
        &fixture,
        &output_directory,
        &authorization(&plan),
        &options,
        &[],
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("outcome: output-overflow")
    );
    assert!(!output_directory.exists());

    fixture.replace_behavior(
        r#"
out = args[args.index("--output-directory") + 1]
with open(os.path.join(out, "too-large.json"), "wb") as stream:
    stream.write(b"x" * 1024)
"#,
    );
    let output_directory = fixture.output("storage-overflow");
    let options = ["--storage-limit", "128"];
    let plan = fixture.plan_with_options(&output_directory, &options, &[]);
    let output = collect_with_options(
        &fixture,
        &output_directory,
        &authorization(&plan),
        &options,
        &[],
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("storage limit")
    );
    assert!(!output_directory.exists());
}

#[test]
fn existing_symlink_output_and_symlink_tool_are_rejected() {
    let fixture = Fixture::new(false);
    let destination = fixture.output("destination");
    fs::create_dir(&destination).unwrap();
    let output_link = fixture.output("output-link");
    symlink(&destination, &output_link).unwrap();
    assert!(!fixture.plan(&output_link, &[]).status.success());

    let linked_directory = fixture.root.join("linked");
    fs::create_dir(&linked_directory).unwrap();
    let tool_link = linked_directory.join("rocprofv3");
    symlink(&fixture.tool, &tool_link).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .args([
            "profile",
            "--tool",
            tool_link.to_str().unwrap(),
            "--output-dir",
            fixture.output("capture").to_str().unwrap(),
            "--",
            "/bin/true",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn canonical_output_through_non_utf8_parent_is_rejected() {
    let fixture = Fixture::new(false);
    let non_utf8_parent = fixture
        .root
        .join(OsString::from_vec(b"non-utf8-\xff".to_vec()));
    fs::create_dir(&non_utf8_parent).unwrap();
    let utf8_alias = fixture.output("utf8-alias");
    symlink(&non_utf8_parent, &utf8_alias).unwrap();
    let requested = utf8_alias.join("capture");

    let output = fixture.plan(&requested, &[]);

    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("canonical --output-dir must be valid UTF-8")
    );
    assert!(!non_utf8_parent.join("capture").exists());
}

#[test]
fn duplicates_and_bounds_fail_before_any_output_creation() {
    let fixture = Fixture::new(false);
    let output_directory = fixture.output("capture");
    let prefixes = [
        vec!["--kind", "att", "--kind", "dispatch-json"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        vec!["--timeout-ms", "0"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        vec!["--storage-limit", "4294967297"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        vec!["--kir-sha256".to_owned(), "0".repeat(64)],
        vec![
            "--timeout-ms".to_owned(),
            "1".to_owned(),
            "--timeout-ms=2".to_owned(),
        ],
        vec![
            "--stdout-limit=1".to_owned(),
            "--stdout-limit".to_owned(),
            "2".to_owned(),
        ],
        vec![
            "--stderr-limit".to_owned(),
            "1".to_owned(),
            "--stderr-limit=2".to_owned(),
        ],
        vec![
            "--storage-limit=1".to_owned(),
            "--storage-limit=2".to_owned(),
        ],
        vec![
            format!("--tool={}", fixture.tool.display()),
            "--tool".to_owned(),
            fixture.tool.to_str().unwrap().to_owned(),
        ],
    ];
    for prefix in prefixes {
        let mut arguments = vec!["profile".to_owned()];
        arguments.extend(prefix);
        arguments.extend(
            [
                "--tool",
                fixture.tool.to_str().unwrap(),
                "--output-dir",
                output_directory.to_str().unwrap(),
                "--",
                "/bin/true",
            ]
            .into_iter()
            .map(str::to_owned),
        );
        let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
            .args(arguments)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(!output_directory.exists());
    }
}
