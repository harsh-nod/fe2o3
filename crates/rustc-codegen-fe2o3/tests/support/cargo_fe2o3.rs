use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

fn cargo_target_root(workspace: &Path) -> PathBuf {
    match std::env::var_os("CARGO_TARGET_DIR") {
        Some(path) if Path::new(&path).is_absolute() => PathBuf::from(path),
        Some(path) => workspace.join(path),
        None => workspace.join("target"),
    }
}

fn scrub_dynamic_loader_environment(command: &mut Command) {
    for (name, _) in std::env::vars_os() {
        let bytes = name.as_bytes();
        if bytes.starts_with(b"LD_") || bytes.starts_with(b"DYLD_") || bytes == b"GLIBC_TUNABLES" {
            command.env_remove(name);
        }
    }
}

fn binary(workspace: &Path) -> &'static Path {
    static BINARY: OnceLock<PathBuf> = OnceLock::new();
    BINARY
        .get_or_init(|| {
            let mut command = Command::new(env!("CARGO"));
            command.current_dir(workspace).args([
                "build",
                "--locked",
                "-p",
                "cargo-fe2o3",
                "--bin",
                "cargo-fe2o3",
            ]);
            scrub_dynamic_loader_environment(&mut command);
            let output = command.output().expect("build cargo-fe2o3 test binary");
            assert!(
                output.status.success(),
                "cargo-fe2o3 build failed:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
            cargo_target_root(workspace).join("debug/cargo-fe2o3")
        })
        .as_path()
}

pub fn non_production_command(workspace: &Path) -> Command {
    let mut command = Command::new(binary(workspace));
    command.env(
        "FE2O3_NON_PRODUCTION_UNPROTECTED_AUTHORITY_VALIDATION_V1",
        "1",
    );
    scrub_dynamic_loader_environment(&mut command);
    command
}
