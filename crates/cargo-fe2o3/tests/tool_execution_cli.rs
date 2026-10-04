#![cfg(unix)]

use std::env;
use std::fs;
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};
use std::process::{self, Command, Output};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    tool: PathBuf,
    target: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        loop {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let root =
                env::temp_dir().join(format!("cargo-fe2o3-tool-execution-{}-{id}", process::id()));
            match fs::create_dir(&root) {
                Ok(()) => {
                    let source = fixture_binary();
                    let tool = root.join("rocgdb");
                    let target = root.join("target-program");
                    copy_executable(source, &tool);
                    copy_executable(source, &target);
                    return Self { root, tool, target };
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("failed to create temporary directory: {error}"),
            }
        }
    }

    fn run(&self, mode: &str, behavior: &str, extra: &[&str]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
        command.args([mode, "--execute"]);
        if mode == "debug" {
            command.arg("--batch");
        }
        command.args([
            "--tool",
            self.tool.to_str().expect("UTF-8 tool path"),
            "--timeout-ms",
            "2000",
            "--stdout-limit",
            "65536",
            "--stderr-limit",
            "65536",
            "--",
            self.target.to_str().expect("UTF-8 target path"),
            &format!("--fe2o3-fixture={behavior}"),
        ]);
        command.args(extra).output().expect("execute cargo-fe2o3")
    }
}

fn fixture_binary() -> &'static Path {
    static FIXTURE: OnceLock<PathBuf> = OnceLock::new();
    FIXTURE
        .get_or_init(|| {
            let source =
                Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/g8_b_tool_fixture.rs");
            let output = Path::new(env!("CARGO_TARGET_TMPDIR"))
                .join(format!("g8-b-tool-fixture-{}", process::id()));
            let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
            let status = Command::new(rustc)
                .args(["--edition=2024", "-O"])
                .arg(&source)
                .arg("-o")
                .arg(&output)
                .status()
                .expect("compile adversarial tool fixture");
            assert!(status.success(), "fixture compilation failed");
            output
        })
        .as_path()
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn copy_executable(source: &Path, destination: &Path) {
    fs::copy(source, destination).expect("copy ELF fixture");
    fs::set_permissions(destination, fs::Permissions::from_mode(0o700))
        .expect("make fixture executable");
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("UTF-8 evidence")
}

#[test]
fn execution_clears_unapproved_environment_and_records_deterministically() {
    let fixture = Fixture::new();
    let run = || {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
        command
            .env("FE2O3_SECRET", "must-not-leak")
            .args([
                "debug",
                "--execute",
                "--batch",
                "--tool",
                fixture.tool.to_str().unwrap(),
                "--",
                fixture.target.to_str().unwrap(),
                "--fe2o3-fixture=environment",
            ])
            .output()
            .unwrap()
    };
    let first = run();
    let second = run();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(stdout(&first), stdout(&second));
    assert!(stdout(&first).contains("FE2O3_SECRET=absent"));
    assert!(stdout(&first).contains("authority: diagnostic-only"));
}

#[test]
fn target_arguments_never_become_debugger_or_shell_commands() {
    let fixture = Fixture::new();
    let marker = fixture.root.join("injected");
    let payload = format!(";touch {}", marker.display());
    let output = fixture.run("debug", "arguments", &["-ex", "shell false", &payload]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let evidence = stdout(&output);
    assert!(evidence.contains("PAYLOAD[0]=\\\"-ex\\\""));
    assert!(evidence.contains("PAYLOAD[1]=\\\"shell false\\\""));
    assert!(evidence.contains(&payload));
    assert!(!marker.exists());
}

#[test]
fn timeout_and_output_overflow_are_distinct_and_bounded() {
    let fixture = Fixture::new();
    let started = Instant::now();
    let mut timeout = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
    let timeout = timeout
        .args([
            "debug",
            "--execute",
            "--batch",
            "--tool",
            fixture.tool.to_str().unwrap(),
            "--timeout-ms=50",
            "--",
            fixture.target.to_str().unwrap(),
            "--fe2o3-fixture=timeout",
        ])
        .output()
        .unwrap();
    assert!(!timeout.status.success());
    assert!(stdout(&timeout).contains("outcome: timeout"));
    assert!(started.elapsed() < Duration::from_secs(2));

    let mut overflow = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
    let overflow = overflow
        .args([
            "debug",
            "--execute",
            "--batch",
            "--tool",
            fixture.tool.to_str().unwrap(),
            "--stdout-limit=1024",
            "--",
            fixture.target.to_str().unwrap(),
            "--fe2o3-fixture=output-overflow",
        ])
        .output()
        .unwrap();
    assert!(!overflow.status.success());
    assert!(stdout(&overflow).contains("outcome: output-overflow"));
    assert!(stdout(&overflow).contains("stdout-bytes: 1024"));

    let mut stderr_overflow = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
    let stderr_overflow = stderr_overflow
        .args([
            "debug",
            "--execute",
            "--batch",
            "--tool",
            fixture.tool.to_str().unwrap(),
            "--stderr-limit=1024",
            "--",
            fixture.target.to_str().unwrap(),
            "--fe2o3-fixture=stderr-overflow",
        ])
        .output()
        .unwrap();
    assert!(!stderr_overflow.status.success());
    assert!(stdout(&stderr_overflow).contains("outcome: output-overflow"));
    assert!(stdout(&stderr_overflow).contains("stderr-bytes: 1024"));
}

#[test]
fn descendants_cannot_hold_capture_pipes_open() {
    let fixture = Fixture::new();
    let started = Instant::now();
    let output = fixture.run("debug", "descendant", &[]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(stdout(&output).contains("outcome: diagnostic-run-completed"));
}

#[test]
fn tool_and_target_failures_have_distinct_evidence() {
    let fixture = Fixture::new();
    for (behavior, outcome) in [
        ("tool-exit", "tool-exit-failure"),
        ("tool-signal", "tool-signal"),
        ("target-exit", "target-exit-failure"),
        ("target-signal", "target-signal"),
    ] {
        let output = fixture.run("debug", behavior, &[]);
        assert!(
            !output.status.success(),
            "{behavior} unexpectedly succeeded"
        );
        assert!(
            stdout(&output).contains(&format!("outcome: {outcome}")),
            "{behavior}: {}",
            stdout(&output)
        );
    }
}

#[test]
fn sanitizer_reports_diagnostics_without_claiming_memory_or_race_safety() {
    let fixture = Fixture::new();
    let successful = fixture.run("sanitize", "success", &[]);
    assert!(successful.status.success());
    let evidence = stdout(&successful);
    assert!(evidence.contains("no-memory-fault-reported-not-a-safety-claim"));
    assert!(evidence.contains("coverage-race: unsupported"));
    assert!(evidence.contains("coverage-api: unsupported"));

    let finding = fixture.run("sanitize", "memory-diagnostic", &[]);
    assert!(!finding.status.success());
    assert!(stdout(&finding).contains("outcome: sanitizer-diagnostic-reported"));

    for coverage in ["race", "api", "unsupported"] {
        let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
            .args([
                "sanitize",
                "--execute",
                "--coverage",
                coverage,
                "--tool",
                fixture.tool.to_str().unwrap(),
                "--",
                fixture.target.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(!output.status.success());
    }
}

#[test]
fn symlink_and_path_replacement_are_rejected() {
    let fixture = Fixture::new();
    let link_directory = fixture.root.join("linked-tool");
    fs::create_dir(&link_directory).unwrap();
    let link = link_directory.join("rocgdb");
    symlink(&fixture.tool, &link).unwrap();
    let symlink_output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .args([
            "debug",
            "--execute",
            "--batch",
            "--tool",
            link.to_str().unwrap(),
            "--",
            fixture.target.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!symlink_output.status.success());

    let replaced = fixture.run("debug", "replace-tool", &[]);
    assert!(!replaced.status.success());
    assert!(stdout(&replaced).contains("outcome: tool-identity-changed"));

    let fixture = Fixture::new();
    let replaced = fixture.run("debug", "replace-target", &[]);
    assert!(!replaced.status.success());
    assert!(stdout(&replaced).contains("outcome: target-identity-changed"));
}

#[test]
fn execute_modes_and_bounds_fail_closed_before_spawn() {
    let fixture = Fixture::new();
    for args in [
        vec![
            "debug",
            "--execute",
            "--tool",
            fixture.tool.to_str().unwrap(),
            "--",
            fixture.target.to_str().unwrap(),
        ],
        vec![
            "debug",
            "--execute",
            "--batch",
            "--interactive",
            "--tool",
            fixture.tool.to_str().unwrap(),
            "--",
            fixture.target.to_str().unwrap(),
        ],
        vec![
            "debug",
            "--execute",
            "--batch",
            "--timeout-ms=0",
            "--tool",
            fixture.tool.to_str().unwrap(),
            "--",
            fixture.target.to_str().unwrap(),
        ],
        vec![
            "sanitize",
            "--execute",
            "--interactive",
            "--tool",
            fixture.tool.to_str().unwrap(),
            "--",
            fixture.target.to_str().unwrap(),
        ],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
    }
}

#[test]
#[ignore = "set FE2O3_RUN_ROCGDB_SMOKE=1 and request this ignored test explicitly"]
fn real_rocgdb_batch_smoke_is_explicitly_gated() {
    assert_eq!(env::var("FE2O3_RUN_ROCGDB_SMOKE").as_deref(), Ok("1"));
    let tool = [
        "/opt/rocm/bin/rocgdb-py_3.12",
        "/opt/rocm/bin/rocgdb-py_3.13",
    ]
    .into_iter()
    .find(|path| Path::new(path).is_file())
    .expect("reviewed native ROCgdb executable");
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .args([
            "debug",
            "--execute",
            "--batch",
            "--tool",
            tool,
            "--timeout-ms=30000",
            "--",
            "/bin/true",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
