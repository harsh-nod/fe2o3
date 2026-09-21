use crate::production_rustc_driver_checked_output_source_helpers_v1_tests::{
    artifact, clean_command, output,
};
use crate::test_temp_dir::TestTempDir;
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "host_core_setup_v1_tests.rs"]
mod tests;

const MIR_FLAGS: [&str; 9] = [
    "-Zalways-encode-mir",
    "-Zinline-mir=no",
    "-Zmir-opt-level=1",
    "-Zmir-enable-passes=-JumpThreading",
    "-Copt-level=0",
    "-Cdebug-assertions=on",
    "-Coverflow-checks=on",
    "-Cpanic=abort",
    "-Cdebuginfo=0",
];

struct HostCompiler {
    executable: OsString,
    sysroot: PathBuf,
    host: String,
}

fn field<'a>(verbose: &'a str, prefix: &str) -> &'a str {
    let values: Vec<_> = verbose
        .lines()
        .filter_map(|line| line.strip_prefix(prefix))
        .collect();
    assert_eq!(values.len(), 1, "one rustc identity field {prefix:?}");
    assert!(!values[0].is_empty(), "nonempty rustc identity field");
    values[0]
}

fn checked_host(verbose: &str) -> String {
    for (prefix, expected) in [
        ("release: ", env!("FE2O3_BUILD_RUSTC_RELEASE")),
        ("commit-hash: ", env!("FE2O3_BUILD_RUSTC_COMMIT")),
        ("LLVM version: ", env!("FE2O3_BUILD_RUSTC_LLVM")),
    ] {
        assert_eq!(field(verbose, prefix), expected, "selected rustc identity");
    }
    let host = field(verbose, "host: ");
    assert!(
        host.bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte)),
        "a literal rustc host target, not arguments"
    );
    host.to_owned()
}

fn compiler() -> HostCompiler {
    let executable = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let version = output(clean_command(&executable).arg("-vV"));
    let verbose = String::from_utf8(version.stdout).unwrap();
    let host = checked_host(&verbose);
    let sysroot = output(clean_command(&executable).args(["--print", "sysroot"]));
    let sysroot = String::from_utf8(sysroot.stdout).unwrap();
    assert_eq!(sysroot.trim().lines().count(), 1);
    let sysroot = PathBuf::from(sysroot.trim());
    assert!(sysroot.is_absolute() && sysroot.is_dir());
    let sysroot = sysroot.canonicalize().unwrap();
    println!("primitive From debug-core compiler: {verbose}");
    HostCompiler {
        executable,
        sysroot,
        host,
    }
}

fn build_command(workspace: &Path, target: &Path, compiler: &HostCompiler) -> Command {
    let mut command = clean_command(env!("CARGO"));
    command
        .current_dir(workspace)
        .args([
            "check",
            "--offline",
            "--locked",
            "-Zbuild-std=core",
            "--lib",
            "-p",
            "fe2o3-device",
            "--target",
            &compiler.host,
            "--message-format=json-render-diagnostics",
            "--target-dir",
        ])
        .arg(target)
        .env("RUSTC", &compiler.executable)
        .env("CARGO_ENCODED_RUSTFLAGS", MIR_FLAGS.join("\x1f"));
    command
}

fn owned_artifact(messages: &[serde_json::Value], name: &str, target: &Path) -> PathBuf {
    let selected = artifact(messages, name);
    assert!(selected.is_absolute() && selected.is_file());
    let selected = selected.canonicalize().unwrap();
    let target = target.canonicalize().unwrap();
    assert!(
        selected.starts_with(&target),
        "actual artifact outside owned target"
    );
    selected
}

fn path_text(path: &Path) -> &str {
    path.to_str().expect("UTF-8 compiler argument path")
}

fn arguments(
    directory: &Path,
    source: &Path,
    compiler: &HostCompiler,
    core: &Path,
    builtins: &Path,
) -> Vec<String> {
    let mut args = vec![
        compiler.executable.to_str().unwrap().to_owned(),
        "--crate-name=fe2o3_checked_primitive_from_fixture".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--emit=metadata".into(),
        "-Zunstable-options".into(),
        "--target".into(),
        compiler.host.clone(),
        "--sysroot".into(),
        path_text(&compiler.sysroot).into(),
        format!("--extern=noprelude:core={}", path_text(core)),
        format!(
            "--extern=noprelude:compiler_builtins={}",
            path_text(builtins)
        ),
    ];
    args.extend(MIR_FLAGS.into_iter().map(str::to_owned));
    let core_dependencies = core.parent().unwrap();
    args.push(format!("-Ldependency={}", path_text(core_dependencies)));
    if builtins.parent().unwrap() != core_dependencies {
        args.push(format!(
            "-Ldependency={}",
            path_text(builtins.parent().unwrap())
        ));
    }
    args.extend([
        "-o".into(),
        path_text(&directory.join("fixture.rmeta")).into(),
        path_text(source).into(),
    ]);
    args
}

pub(super) fn fixture_arguments(directory: &TestTempDir, source: &Path) -> Vec<String> {
    let compiler = compiler();
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let target = directory.path().join("debug-core");
    std::fs::create_dir(&target).unwrap();
    let mut command = build_command(&workspace, &target, &compiler);
    println!("primitive From debug-core producer: {command:?}");
    let built = output(&mut command);
    let messages: Vec<serde_json::Value> = built
        .stdout
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).unwrap())
        .collect();
    let core = owned_artifact(&messages, "core", &target);
    let builtins = owned_artifact(&messages, "compiler_builtins", &target);
    println!("primitive From debug-core actual artifacts: core={core:?} builtins={builtins:?}");
    arguments(directory.path(), source, &compiler, &core, &builtins)
}
