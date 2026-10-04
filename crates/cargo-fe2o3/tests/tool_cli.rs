use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TempTool {
    root: PathBuf,
    executable: PathBuf,
}

impl TempTool {
    fn new() -> Self {
        loop {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let root = env::temp_dir().join(format!("cargo-fe2o3-tool-cli-{}-{id}", process::id()));
            match fs::create_dir(&root) {
                Ok(()) => {
                    let executable = root.join("rocgdb");
                    fs::write(&executable, b"never executed\n").expect("write fake ROCgdb");
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt as _;
                        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700))
                            .expect("make fake ROCgdb executable");
                    }
                    return Self { root, executable };
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("failed to create temporary tool: {error}"),
            }
        }
    }
}

impl Drop for TempTool {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn run(mode: &str, tool: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .args([
            mode,
            "--tool",
            tool.to_str().expect("UTF-8 tool path"),
            "--",
            "./kernel with space",
            "--length=4",
        ])
        .output()
        .expect("run tool plan command")
}

#[test]
fn sanitize_and_debug_print_plans_without_executing_the_tool() {
    let tool = TempTool::new();
    for (mode, backend) in [
        ("sanitize", "backend: rocgdb-precise-memory"),
        ("debug", "backend: rocgdb-interactive"),
    ] {
        let output = run(mode, &tool.executable);
        assert!(
            output.status.success(),
            "{mode} stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).expect("UTF-8 plan");
        assert!(stdout.contains("authority: plan-only"));
        assert!(stdout.contains(backend));
        assert!(stdout.contains("\"./kernel with space\""));
        assert!(stdout.contains("\"--length=4\""));
        assert_eq!(
            fs::read(&tool.executable).expect("read untouched fake ROCgdb"),
            b"never executed\n"
        );
    }
}

#[test]
fn unavailable_explicit_tool_is_diagnostic() {
    let missing = env::temp_dir().join(format!("cargo-fe2o3-missing-{}/rocgdb", process::id()));
    let output = run("sanitize", &missing);
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 diagnostic");
    assert!(stderr.contains("ROCgdb tool is unavailable or not executable"));
    assert!(!stderr.contains("panicked"));
}

#[test]
fn malformed_cli_is_rejected_before_discovery() {
    for args in [
        vec!["sanitize", "program"],
        vec!["debug", "--"],
        vec!["sanitize", "--unknown", "--", "program"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
            .args(args)
            .output()
            .expect("run malformed command");
        assert!(!output.status.success());
    }
}
