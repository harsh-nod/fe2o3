#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

struct TemporaryBuild(PathBuf);

impl Drop for TemporaryBuild {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run<I, S>(program: &str, arguments: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new(program)
        .args(arguments)
        .output()
        .unwrap_or_else(|error| panic!("failed to run {program}: {error}"));
    assert!(
        output.status.success(),
        "{program} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

#[test]
fn static_preexec_launcher_cmake_suite() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("cargo-fe2o3 must remain below the repository root");
    let source = repository.join("tools/fe2o3-static-preexec-launcher");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must follow the Unix epoch")
        .as_nanos();
    let build = TemporaryBuild(std::env::temp_dir().join(format!(
        "fe2o3-static-preexec-cargo-{}-{nonce}",
        std::process::id()
    )));

    run(
        "cmake",
        [
            OsStr::new("-S"),
            source.as_os_str(),
            OsStr::new("-B"),
            build.0.as_os_str(),
            OsStr::new("-DCMAKE_BUILD_TYPE=Release"),
            OsStr::new("-DCMAKE_C_COMPILER=/usr/bin/cc"),
            OsStr::new("-DBUILD_TESTING=ON"),
        ],
    );
    run(
        "cmake",
        [
            OsStr::new("--build"),
            build.0.as_os_str(),
            OsStr::new("--parallel"),
            OsStr::new("4"),
        ],
    );
    run(
        "ctest",
        [
            OsStr::new("--test-dir"),
            build.0.as_os_str(),
            OsStr::new("--output-on-failure"),
            OsStr::new("-j"),
            OsStr::new("4"),
        ],
    );

    let launcher = build.0.join("fe2o3-static-preexec-launcher");
    let readelf = run(
        "/usr/bin/readelf",
        [OsStr::new("-lW"), OsStr::new("-dW"), launcher.as_os_str()],
    );
    let report = String::from_utf8(readelf.stdout).expect("readelf output is UTF-8");
    assert!(!report.contains("INTERP"));
    assert!(!report.contains("DYNAMIC"));
    assert!(!report.contains("(NEEDED)"));

    let undefined = run("/usr/bin/nm", [OsStr::new("-u"), launcher.as_os_str()]);
    assert!(
        undefined.stdout.is_empty(),
        "freestanding launcher has undefined symbols: {}",
        String::from_utf8_lossy(&undefined.stdout)
    );
    let symbols = run("/usr/bin/nm", [OsStr::new("-n"), launcher.as_os_str()]);
    let symbols = String::from_utf8(symbols.stdout).expect("nm output is UTF-8");
    assert!(symbols.lines().any(|line| line.ends_with(" T _start")));
    assert!(!symbols.contains("__libc"));
}
