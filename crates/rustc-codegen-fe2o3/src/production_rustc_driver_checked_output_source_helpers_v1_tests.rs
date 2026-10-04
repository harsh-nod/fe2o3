use std::env;
use std::path::PathBuf;
use std::process::Command;

pub(crate) fn clean_command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut command = Command::new(program);
    command.env_clear();
    for key in [
        "HOME",
        "PATH",
        "CARGO_HOME",
        "RUSTUP_HOME",
        "RUSTUP_TOOLCHAIN",
        "LD_LIBRARY_PATH",
        "TMPDIR",
    ] {
        if let Some(value) = env::var_os(key) {
            command.env(key, value);
        }
    }
    command
        .env("CARGO_BUILD_JOBS", "1")
        .env("CARGO_INCREMENTAL", "0")
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_PROFILE_RELEASE_DEBUG", "0")
        .env("FE2O3_HIP_SYS_DISABLE", "1")
        .env("FE2O3_HSA_RUNTIME_DISABLE", "1");
    command
}

pub(crate) fn output(command: &mut Command) -> std::process::Output {
    let result = command
        .output()
        .expect("execute source qualification command");
    assert!(
        result.status.success(),
        "{command:?}\n{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    result
}

pub(crate) fn artifact(messages: &[serde_json::Value], name: &str) -> PathBuf {
    let artifacts: Vec<_> = messages
        .iter()
        .filter(|m| m["reason"] == "compiler-artifact" && m["target"]["name"] == name)
        .collect();
    assert_eq!(
        artifacts.len(),
        1,
        "one actual Cargo artifact for {name}: {artifacts:?}"
    );
    let filenames = artifacts[0]["filenames"]
        .as_array()
        .expect("Cargo artifact filenames");
    // build-std can emit both for one artifact. This callback needs metadata;
    // use the standalone metadata when present, otherwise its library container.
    for extension in ["rmeta", "rlib"] {
        let matches: Vec<_> = filenames
            .iter()
            .filter_map(|p| p.as_str())
            .map(PathBuf::from)
            .filter(|p| p.extension().is_some_and(|ext| ext == extension))
            .collect();
        assert!(
            matches.len() <= 1,
            "ambiguous {extension} for {name}: {matches:?}"
        );
        if let Some(path) = matches.into_iter().next() {
            return path;
        }
    }
    panic!("missing metadata for actual Cargo artifact {name}");
}
