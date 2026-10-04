//! Executes the production startup assembly without libc or a protected runtime.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use std::{
    fs,
    os::unix::{fs::DirBuilderExt, process::CommandExt},
    path::PathBuf,
    process::Command,
};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("fe2o3-secure-start-{}", std::process::id()));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }

    fn build(&self, entry: &str) -> PathBuf {
        let source = self.0.join("start.S");
        let executable = self.0.join(entry);
        let startup = format!(
            include_str!("../src/secure_start_x86_64.S"),
            invocation = "descriptor_invocation",
            max_argv0 = fe2o3_protected_service_profile::observations::MAX_DESCRIPTOR_ARGV0_BYTES,
        );
        fs::write(&source, startup + include_str!("secure_start_probe.S")).unwrap();
        let output = Command::new("cc")
            .args(["-nostdlib", "-static", "-no-pie"])
            .arg(format!("-Wl,-e,{entry}"))
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        executable
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for name in ["start.S", "fe2o3_secure_start_v1", "_start"] {
            let _ = fs::remove_file(self.0.join(name));
        }
        let _ = fs::remove_dir(&self.0);
    }
}

#[test]
fn actual_entry_checks_bounded_invocation_and_keeps_confinement() {
    let fixture = Fixture::new();
    let executable = fixture.build("fe2o3_secure_start_v1");
    let maximum = fe2o3_protected_service_profile::observations::MAX_DESCRIPTOR_ARGV0_BYTES;
    for (name, argument, environment, expected) in [
        ("service".to_owned(), None, false, 0),
        ("a".repeat(maximum - 1), None, false, 0),
        ("a".repeat(maximum), None, false, 126),
        (String::new(), None, false, 126),
        ("service".to_owned(), Some("extra"), false, 126),
        ("service".to_owned(), None, true, 126),
    ] {
        let mut command = Command::new(&executable);
        command.arg0(&name).env_clear();
        if let Some(argument) = argument {
            command.arg(argument);
        }
        if environment {
            command.env("POISON", "1");
        }
        assert_eq!(command.status().unwrap().code(), Some(expected));
    }
    let bypass = fixture.build("_start");
    assert_eq!(
        Command::new(bypass).env_clear().status().unwrap().code(),
        Some(127)
    );
}
