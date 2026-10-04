use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[test]
fn real_rustc_version_query_passes_through_wrapper_mode() {
    let output = Command::new(env!("CARGO_BIN_EXE_fe2o3-rustc-wrapper"))
        .arg(rustc_path())
        .arg("-vV")
        .output()
        .expect("run cargo-fe2o3 wrapper");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("rustc "));
}

#[test]
fn real_cargo_probe_shape_passes_through_wrapper_mode() {
    let output = Command::new(env!("CARGO_BIN_EXE_fe2o3-rustc-wrapper"))
        .arg(rustc_path())
        .args([
            "-",
            "--crate-name",
            "___",
            "--print=file-names",
            "--crate-type=bin",
            "--crate-type=rlib",
        ])
        .stdin(Stdio::null())
        .output()
        .expect("run cargo-fe2o3 wrapper");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!output.stdout.is_empty());
}

#[test]
fn real_rustc_treats_managed_options_after_terminator_as_input_paths() {
    let root = env::temp_dir().join(format!(
        "cargo-fe2o3-rustc-terminator-{}-{}",
        std::process::id(),
        unique_id()
    ));
    fs::create_dir(&root).expect("create rustc terminator fixture");
    let source = root.join("main.rs");
    fs::write(&source, "fn main() {}\n").expect("write rustc terminator fixture");

    let output = Command::new(rustc_path())
        .arg("--")
        .arg(&source)
        .arg("-Zcodegen-backend=/tmp/fe2o3-terminator-probe.so")
        .output()
        .expect("run real rustc terminator probe");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(
        stderr.contains("multiple input filenames provided")
            && stderr.contains("-Zcodegen-backend=/tmp/fe2o3-terminator-probe.so"),
        "real rustc no longer demonstrated terminator semantics: {stderr}"
    );
    fs::remove_dir_all(root).expect("remove rustc terminator fixture");
}

#[cfg(unix)]
#[test]
fn compile_shapes_fail_before_the_rustc_process_is_spawned() {
    for extra in [
        &[][..],
        &["--print=native-static-libs"][..],
        &["--print=link-args"][..],
        &["-Zunpretty=expanded"][..],
        &["-Zno-analysis"][..],
        &["-Zno-codegen"][..],
    ] {
        assert_source_shape_does_not_spawn("src/lib.rs", extra);
    }
    assert_source_shape_does_not_spawn("src/kernel.input", &["--print=file-names"]);
    assert_source_shape_does_not_spawn("-", &["--print=file-names"]);
}

#[cfg(unix)]
#[test]
fn unsafe_passthrough_options_fail_before_rustc_is_spawned() {
    for arguments in [
        vec!["-Zcodegen-backend=/tmp/untrusted.so", "--print=sysroot"],
        vec!["-Zcodegen-backend=/tmp/untrusted.so", "--help"],
        vec!["--verbose"],
    ] {
        let (root, rustc) = fake_rustc("exit 0");
        let sentinel = root.join("spawned");
        fs::write(
            &rustc,
            format!("#!/bin/sh\ntouch '{}'\nexit 0\n", sentinel.display()),
        )
        .unwrap();
        make_executable(&rustc);

        let status = Command::new(env!("CARGO_BIN_EXE_fe2o3-rustc-wrapper"))
            .arg(&rustc)
            .args(arguments)
            .stderr(Stdio::null())
            .status()
            .expect("run wrapper");
        assert!(!status.success());
        assert!(!sentinel.exists(), "unsafe passthrough spawned rustc");
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(unix)]
fn assert_source_shape_does_not_spawn(source: &str, extra: &[&str]) {
    let (root, rustc) = fake_rustc("exit 0");
    let sentinel = root.join("spawned");
    fs::write(
        &rustc,
        format!("#!/bin/sh\ntouch '{}'\nexit 0\n", sentinel.display()),
    )
    .unwrap();
    make_executable(&rustc);

    let mut command = Command::new(env!("CARGO_BIN_EXE_fe2o3-rustc-wrapper"));
    command
        .arg(&rustc)
        .args(["--crate-name", "kernel", source])
        .args(extra)
        .stdout(Stdio::null());
    let output = command.output().expect("run cargo-fe2o3 wrapper");

    assert!(!output.status.success());
    assert!(
        !sentinel.exists(),
        "source path {source:?} with {extra:?} spawned rustc before pinning"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not activated") || stderr.contains("invalid rustc wrapper invocation"),
        "{stderr}"
    );
    fs::remove_dir_all(root).expect("remove test directory");
}

#[cfg(unix)]
#[test]
fn passthrough_preserves_nonzero_exit_status() {
    let (root, rustc) = fake_rustc("exit 23");
    let status = Command::new(env!("CARGO_BIN_EXE_fe2o3-rustc-wrapper"))
        .arg(&rustc)
        .arg("--version")
        .status()
        .expect("run wrapper");
    assert_eq!(status.code(), Some(23));
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn passthrough_preserves_signal_status() {
    let (root, rustc) = fake_rustc("kill -TERM $$");
    let status = Command::new(env!("CARGO_BIN_EXE_fe2o3-rustc-wrapper"))
        .arg(&rustc)
        .arg("--version")
        .status()
        .expect("run wrapper");
    assert_eq!(status.code(), Some(143));
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn passthrough_preserves_a_non_utf8_executable_path() {
    use std::os::unix::ffi::OsStringExt;

    let (root, rustc) = fake_rustc("exit 0");
    let native_rustc = root.join(std::ffi::OsString::from_vec(b"rustc-\xff".to_vec()));
    fs::rename(rustc, &native_rustc).unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_fe2o3-rustc-wrapper"))
        .arg(&native_rustc)
        .arg("--version")
        .status()
        .expect("run wrapper");
    assert!(status.success());
    fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn passthrough_retries_a_transiently_busy_executable() {
    use std::fs::OpenOptions;

    let (root, rustc) = fake_rustc("exit 0");
    let writer = OpenOptions::new().write(true).open(&rustc).unwrap();
    let release = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(20));
        drop(writer);
    });

    let output = Command::new(env!("CARGO_BIN_EXE_fe2o3-rustc-wrapper"))
        .arg(&rustc)
        .arg("--version")
        .output()
        .expect("run wrapper");
    release.join().unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn passthrough_fails_after_bounded_retries_for_a_busy_executable() {
    use std::fs::OpenOptions;

    let (root, rustc) = fake_rustc("exit 0");
    let writer = OpenOptions::new().write(true).open(&rustc).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_fe2o3-rustc-wrapper"))
        .arg(&rustc)
        .arg("--version")
        .output()
        .expect("run wrapper");
    drop(writer);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("os error 26"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}

fn rustc_path() -> PathBuf {
    env::var_os("RUSTC")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("rustc"))
}

fn unique_id() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_nanos()
}

#[cfg(unix)]
fn fake_rustc(body: &str) -> (PathBuf, PathBuf) {
    let root = env::temp_dir().join(format!(
        "cargo-fe2o3-wrapper-cli-{}-{}",
        std::process::id(),
        unique_id()
    ));
    fs::create_dir(&root).expect("create test directory");
    let rustc = root.join("fake-rustc");
    fs::write(&rustc, format!("#!/bin/sh\n{body}\n")).expect("write fake rustc");
    make_executable(&rustc);
    (root, rustc)
}

#[cfg(unix)]
fn make_executable(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions).unwrap();
}
