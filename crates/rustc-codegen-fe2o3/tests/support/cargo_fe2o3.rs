use sha2::{Digest as _, Sha256};
use std::ffi::{OsStr, OsString};
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) const TEST_DRIVER_ENV: &str = "FE2O3_TEST_CARGO_FE2O3_BIN";
pub(crate) const TEST_DRIVER_SHA256_ENV: &str = "FE2O3_TEST_CARGO_FE2O3_SHA256";
const QUALIFICATION_DRIVER_BUILD_ARGUMENTS: [&str; 9] = [
    "build",
    "--locked",
    "-p",
    "cargo-fe2o3",
    "--bin",
    "cargo-fe2o3",
    "--features",
    "qualification-oracles-test-only",
    "--message-format=json-render-diagnostics",
];

#[derive(Debug)]
struct DriverIdentity {
    path: PathBuf,
    sha256: [u8; 32],
}

#[derive(Debug)]
struct CargoDriverBuild {
    package_id: String,
    source_path: PathBuf,
    target_root: PathBuf,
}

pub fn cargo_target_root(workspace: &Path) -> PathBuf {
    let declared = match std::env::var_os("CARGO_TARGET_DIR") {
        Some(path) if Path::new(&path).is_absolute() => PathBuf::from(path),
        Some(path) => workspace.join(path),
        None => workspace.join("target"),
    };
    let canonical = declared
        .canonicalize()
        .unwrap_or_else(|error| panic!("canonicalize Cargo target {declared:?}: {error}"));
    let metadata = canonical
        .symlink_metadata()
        .unwrap_or_else(|error| panic!("inspect Cargo target {canonical:?}: {error}"));
    assert!(
        metadata.file_type().is_dir()
            && !metadata.file_type().is_symlink()
            && metadata.uid() == unsafe { libc::geteuid() }
            && metadata.mode() & 0o077 == 0,
        "Cargo target must be an owner-held private directory: {canonical:?}"
    );
    canonical
}

fn is_dynamic_loader_environment_name(name: &OsStr) -> bool {
    let bytes = name.as_bytes();
    bytes.starts_with(b"LD_") || bytes.starts_with(b"DYLD_") || bytes == b"GLIBC_TUNABLES"
}

fn parse_sha256(value: &OsStr) -> Result<[u8; 32], String> {
    let value = value
        .to_str()
        .ok_or_else(|| format!("{TEST_DRIVER_SHA256_ENV} is not UTF-8"))?;
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!(
            "{TEST_DRIVER_SHA256_ENV} must be exactly 64 hexadecimal digits"
        ));
    }
    let mut digest = [0_u8; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|error| format!("invalid {TEST_DRIVER_SHA256_ENV}: {error}"))?;
    }
    Ok(digest)
}

fn sha256(path: &Path) -> Result<[u8; 32], String> {
    let bytes = fs::read(path).map_err(|error| format!("failed to read {path:?}: {error}"))?;
    Ok(Sha256::digest(bytes).into())
}

fn validate_private_driver(path: &Path, expected_sha256: [u8; 32]) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!(
            "{TEST_DRIVER_ENV} must name an absolute path: {path:?}"
        ));
    }
    if path.file_name() != Some(OsStr::new("cargo-fe2o3")) {
        return Err(format!(
            "{TEST_DRIVER_ENV} must name the cargo-fe2o3 binary: {path:?}"
        ));
    }
    let metadata = path
        .symlink_metadata()
        .map_err(|error| format!("failed to inspect {path:?}: {error}"))?;
    let owner = unsafe { libc::geteuid() };
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.uid() != owner
        || metadata.mode() & 0o7777 != 0o500
    {
        return Err(format!(
            "{TEST_DRIVER_ENV} must be an owner-held, non-symlink, mode-0500 regular file: {path:?}"
        ));
    }
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize {path:?}: {error}"))?;
    if canonical != path {
        return Err(format!(
            "{TEST_DRIVER_ENV} must already be canonical: {path:?}"
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| format!("{TEST_DRIVER_ENV} has no private parent: {path:?}"))?;
    let parent_metadata = parent
        .symlink_metadata()
        .map_err(|error| format!("failed to inspect driver root {parent:?}: {error}"))?;
    if !parent_metadata.file_type().is_dir()
        || parent_metadata.file_type().is_symlink()
        || parent_metadata.uid() != owner
        || parent_metadata.mode() & 0o7777 != 0o500
    {
        return Err(format!(
            "{TEST_DRIVER_ENV} parent must be an owner-held, non-symlink, mode-0500 directory: {parent:?}"
        ));
    }
    if sha256(path)? != expected_sha256 {
        return Err(format!(
            "{TEST_DRIVER_ENV} no longer matches {TEST_DRIVER_SHA256_ENV}"
        ));
    }
    Ok(())
}

fn immutable_copy(source: &Path, target_root: &Path, digest: [u8; 32]) -> Result<PathBuf, String> {
    let cache = target_root.join(".fe2o3-test-drivers");
    match fs::create_dir(&cache) {
        Ok(()) => fs::set_permissions(&cache, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("failed to protect {cache:?}: {error}"))?,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(format!("failed to create {cache:?}: {error}")),
    }
    let cache_metadata = cache
        .symlink_metadata()
        .map_err(|error| format!("failed to inspect {cache:?}: {error}"))?;
    if !cache_metadata.file_type().is_dir()
        || cache_metadata.file_type().is_symlink()
        || cache_metadata.uid() != unsafe { libc::geteuid() }
        || cache_metadata.mode() & 0o077 != 0
    {
        return Err(format!(
            "test-driver cache is not a private owned directory: {cache:?}"
        ));
    }

    let digest_hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let final_root = cache.join(&digest_hex);
    let final_binary = final_root.join("cargo-fe2o3");
    if final_root.exists() {
        validate_private_driver(&final_binary, digest)?;
        return Ok(final_binary);
    }

    static NONCE: AtomicU64 = AtomicU64::new(0);
    let temporary = cache.join(format!(
        ".tmp-{}-{}",
        std::process::id(),
        NONCE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&temporary)
        .map_err(|error| format!("failed to create driver staging root {temporary:?}: {error}"))?;
    fs::set_permissions(&temporary, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("failed to protect {temporary:?}: {error}"))?;
    let staged = temporary.join("cargo-fe2o3");
    let mut input = fs::File::open(source)
        .map_err(|error| format!("failed to open built driver {source:?}: {error}"))?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&staged)
        .map_err(|error| format!("failed to create driver copy {staged:?}: {error}"))?;
    std::io::copy(&mut input, &mut output)
        .map_err(|error| format!("failed to copy driver into {staged:?}: {error}"))?;
    output
        .flush()
        .and_then(|()| output.sync_all())
        .map_err(|error| format!("failed to sync driver copy {staged:?}: {error}"))?;
    fs::set_permissions(&staged, fs::Permissions::from_mode(0o500))
        .map_err(|error| format!("failed to seal {staged:?}: {error}"))?;
    fs::set_permissions(&temporary, fs::Permissions::from_mode(0o500))
        .map_err(|error| format!("failed to seal {temporary:?}: {error}"))?;
    match fs::rename(&temporary, &final_root) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            fs::set_permissions(&temporary, fs::Permissions::from_mode(0o700))
                .map_err(|chmod| format!("failed to reopen staging root {temporary:?}: {chmod}"))?;
            fs::remove_dir_all(&temporary).map_err(|remove| {
                format!("failed to remove staging root {temporary:?}: {remove}")
            })?;
        }
        Err(error) => return Err(format!("failed to publish private driver copy: {error}")),
    }
    validate_private_driver(&final_binary, digest)?;
    Ok(final_binary)
}

fn scrub_dynamic_loader_environment(command: &mut Command) {
    let names = std::env::vars_os()
        .map(|(name, _)| name)
        .chain(command.get_envs().map(|(name, _)| name.to_os_string()))
        .filter(|name| is_dynamic_loader_environment_name(name))
        .collect::<Vec<OsString>>();
    for name in names {
        command.env_remove(name);
    }
}

fn cargo_driver_build(workspace: &Path) -> Result<CargoDriverBuild, String> {
    let mut command = Command::new(env!("CARGO"));
    command.current_dir(workspace).args([
        "metadata",
        "--locked",
        "--no-deps",
        "--format-version",
        "1",
    ]);
    scrub_dynamic_loader_environment(&mut command);
    let output = command
        .output()
        .map_err(|error| format!("failed to inspect cargo-fe2o3 package identity: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo-fe2o3 metadata failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("invalid Cargo metadata: {error}"))?;
    let target_root = metadata
        .get("target_directory")
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| "Cargo metadata has no target_directory".to_string())?
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize Cargo target: {error}"))?;
    if target_root != cargo_target_root(workspace) {
        return Err(
            "Cargo metadata target_directory disagrees with the admitted target".to_string(),
        );
    }
    let packages = metadata
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "Cargo metadata has no packages".to_string())?;
    let matches = packages
        .iter()
        .filter(|package| {
            package.get("name").and_then(serde_json::Value::as_str) == Some("cargo-fe2o3")
        })
        .collect::<Vec<_>>();
    let [package] = matches.as_slice() else {
        return Err(format!(
            "Cargo metadata reported {} cargo-fe2o3 packages; expected exactly one",
            matches.len()
        ));
    };
    let expected_manifest = workspace
        .join("crates/cargo-fe2o3/Cargo.toml")
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize cargo-fe2o3 manifest: {error}"))?;
    let manifest = package
        .get("manifest_path")
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| "cargo-fe2o3 metadata has no manifest_path".to_string())?
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize cargo-fe2o3 manifest: {error}"))?;
    if manifest != expected_manifest {
        return Err(format!(
            "cargo-fe2o3 package resolved outside the workspace: {manifest:?}"
        ));
    }
    let targets = package
        .get("targets")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "cargo-fe2o3 metadata has no targets".to_string())?;
    let targets = targets
        .iter()
        .filter(|target| {
            target.get("name").and_then(serde_json::Value::as_str) == Some("cargo-fe2o3")
                && target
                    .get("kind")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|kind| kind.len() == 1 && kind[0].as_str() == Some("bin"))
        })
        .collect::<Vec<_>>();
    let [target] = targets.as_slice() else {
        return Err(format!(
            "Cargo metadata reported {} cargo-fe2o3 binary targets; expected exactly one",
            targets.len()
        ));
    };
    let source_path = target
        .get("src_path")
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| "cargo-fe2o3 binary target has no src_path".to_string())?
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize cargo-fe2o3 source: {error}"))?;
    let expected_source = workspace
        .join("crates/cargo-fe2o3/src/main.rs")
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize cargo-fe2o3 source: {error}"))?;
    if source_path != expected_source {
        return Err(format!(
            "cargo-fe2o3 binary resolved to an unexpected source: {source_path:?}"
        ));
    }
    let package_id = package
        .get("id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "cargo-fe2o3 metadata has no package id".to_string())?
        .to_string();
    Ok(CargoDriverBuild {
        package_id,
        source_path,
        target_root,
    })
}

fn driver_artifact_path(
    record: &serde_json::Value,
    expected: &CargoDriverBuild,
) -> Result<Option<PathBuf>, String> {
    if record.get("reason").and_then(serde_json::Value::as_str) != Some("compiler-artifact")
        || record
            .pointer("/target/name")
            .and_then(serde_json::Value::as_str)
            != Some("cargo-fe2o3")
    {
        return Ok(None);
    }
    let exact_array = |pointer: &str, expected_value: &str| {
        record
            .pointer(pointer)
            .and_then(serde_json::Value::as_array)
            .is_some_and(|values| values.len() == 1 && values[0].as_str() == Some(expected_value))
    };
    let source = record
        .pointer("/target/src_path")
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from);
    let profile = record.get("profile");
    if record.get("package_id").and_then(serde_json::Value::as_str)
        != Some(expected.package_id.as_str())
        || !exact_array("/target/kind", "bin")
        || !exact_array("/target/crate_types", "bin")
        || source.as_deref() != Some(expected.source_path.as_path())
        || profile
            .and_then(|value| value.get("test"))
            .and_then(serde_json::Value::as_bool)
            != Some(false)
        || profile
            .and_then(|value| value.get("opt_level"))
            .and_then(serde_json::Value::as_str)
            != Some("0")
    {
        return Err("Cargo reported a cargo-fe2o3 artifact with mismatched package, source, target, or profile identity".to_string());
    }
    let executable = record
        .get("executable")
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| "Cargo reported cargo-fe2o3 without an executable".to_string())?;
    let canonical = executable.canonicalize().map_err(|error| {
        format!("failed to canonicalize Cargo executable {executable:?}: {error}")
    })?;
    if canonical != executable || !canonical.starts_with(&expected.target_root) {
        return Err(format!(
            "Cargo reported cargo-fe2o3 outside the admitted target: {executable:?}"
        ));
    }
    Ok(Some(canonical))
}

fn unique_driver_artifact(
    records: &[serde_json::Value],
    expected: &CargoDriverBuild,
) -> Result<PathBuf, String> {
    let mut executables = Vec::new();
    for record in records {
        if let Some(path) = driver_artifact_path(record, expected)? {
            executables.push(path);
        }
    }
    let [built] = executables.as_slice() else {
        return Err(format!(
            "Cargo reported {} cargo-fe2o3 executable artifacts; expected exactly one",
            executables.len()
        ));
    };
    Ok(built.clone())
}

fn discover_built_driver(workspace: &Path) -> Result<DriverIdentity, String> {
    let expected = cargo_driver_build(workspace)?;
    let mut command = Command::new(env!("CARGO"));
    command
        .current_dir(workspace)
        .args(QUALIFICATION_DRIVER_BUILD_ARGUMENTS);
    scrub_dynamic_loader_environment(&mut command);
    let output = command
        .output()
        .map_err(|error| format!("failed to build cargo-fe2o3 test driver: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo-fe2o3 build failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let mut records = Vec::new();
    for line in output.stdout.split(|byte| *byte == b'\n') {
        if line.is_empty() {
            continue;
        }
        let record: serde_json::Value = serde_json::from_slice(line)
            .map_err(|error| format!("invalid Cargo JSON artifact record: {error}"))?;
        records.push(record);
    }
    let built = unique_driver_artifact(&records, &expected)?;
    let digest = sha256(&built)?;
    let path = immutable_copy(&built, &cargo_target_root(workspace), digest)?;
    Ok(DriverIdentity {
        path,
        sha256: digest,
    })
}

fn configured_driver() -> Result<DriverIdentity, String> {
    match (
        std::env::var_os(TEST_DRIVER_ENV),
        std::env::var_os(TEST_DRIVER_SHA256_ENV),
    ) {
        (Some(path), Some(digest)) => Ok(DriverIdentity {
            path: PathBuf::from(path),
            sha256: parse_sha256(&digest)?,
        }),
        (None, None) => Err("test driver is not configured".to_string()),
        _ => Err(format!(
            "{TEST_DRIVER_ENV} and {TEST_DRIVER_SHA256_ENV} must be supplied together"
        )),
    }
}

fn driver(workspace: &Path) -> &'static DriverIdentity {
    static DRIVER: OnceLock<DriverIdentity> = OnceLock::new();
    let identity = DRIVER.get_or_init(|| {
        configured_driver()
            .or_else(|error| {
                if error == "test driver is not configured" {
                    discover_built_driver(workspace)
                } else {
                    Err(error)
                }
            })
            .unwrap_or_else(|error| panic!("{error}"))
    });
    validate_private_driver(&identity.path, identity.sha256)
        .unwrap_or_else(|error| panic!("{error}"));
    identity
}

pub(crate) fn binary(workspace: &Path) -> &'static Path {
    driver(workspace).path.as_path()
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

#[cfg(test)]
mod tests {
    use super::{
        CargoDriverBuild, QUALIFICATION_DRIVER_BUILD_ARGUMENTS, driver_artifact_path,
        is_dynamic_loader_environment_name, scrub_dynamic_loader_environment,
        unique_driver_artifact,
    };
    use std::ffi::OsStr;
    use std::path::PathBuf;
    use std::process::Command;

    #[test]
    fn direct_driver_commands_remove_only_dynamic_loader_controls() {
        let mut command = Command::new("/bin/true");
        command
            .env("LD_FE2O3_HOSTILE", "injected")
            .env("DYLD_FE2O3_HOSTILE", "injected")
            .env("GLIBC_TUNABLES", "glibc.malloc.check=1")
            .env("FE2O3_PRESERVED", "present");
        scrub_dynamic_loader_environment(&mut command);
        assert!(
            command
                .get_envs()
                .all(|(name, value)| !is_dynamic_loader_environment_name(name) || value.is_none())
        );
        assert_eq!(
            command
                .get_envs()
                .find_map(|(name, value)| (name == OsStr::new("FE2O3_PRESERVED")).then_some(value)),
            Some(Some(OsStr::new("present")))
        );
    }

    #[test]
    fn fallback_driver_is_explicitly_qualification_enabled() {
        assert_eq!(
            QUALIFICATION_DRIVER_BUILD_ARGUMENTS,
            [
                "build",
                "--locked",
                "-p",
                "cargo-fe2o3",
                "--bin",
                "cargo-fe2o3",
                "--features",
                "qualification-oracles-test-only",
                "--message-format=json-render-diagnostics",
            ]
        );
    }

    #[test]
    fn cargo_json_driver_capture_rejects_identity_and_containment_substitution() {
        let executable = std::env::current_exe()
            .expect("current test executable")
            .canonicalize()
            .expect("canonical test executable");
        let expected = CargoDriverBuild {
            package_id: "path+file:///workspace/crates/cargo-fe2o3#0.1.0".to_string(),
            source_path: PathBuf::from("/workspace/crates/cargo-fe2o3/src/main.rs"),
            target_root: executable
                .parent()
                .expect("executable parent")
                .to_path_buf(),
        };
        let record = serde_json::json!({
            "reason": "compiler-artifact",
            "package_id": expected.package_id.clone(),
            "target": {
                "name": "cargo-fe2o3",
                "kind": ["bin"],
                "crate_types": ["bin"],
                "src_path": expected.source_path.clone(),
            },
            "profile": {"test": false, "opt_level": "0"},
            "executable": executable,
        });

        let mut wrong_package = record.clone();
        wrong_package["package_id"] = serde_json::json!("hostile#0.1.0");
        assert!(driver_artifact_path(&wrong_package, &expected).is_err());
        let mut wrong_source = record.clone();
        wrong_source["target"]["src_path"] = serde_json::json!("/hostile/main.rs");
        assert!(driver_artifact_path(&wrong_source, &expected).is_err());
        let mut wrong_profile = record.clone();
        wrong_profile["profile"]["test"] = serde_json::json!(true);
        assert!(driver_artifact_path(&wrong_profile, &expected).is_err());
        let mut wrong_opt_level = record.clone();
        wrong_opt_level["profile"]["opt_level"] = serde_json::json!("3");
        assert!(driver_artifact_path(&wrong_opt_level, &expected).is_err());
        let mut wrong_kind = record.clone();
        wrong_kind["target"]["kind"] = serde_json::json!(["lib"]);
        assert!(driver_artifact_path(&wrong_kind, &expected).is_err());
        let mut wrong_crate_type = record.clone();
        wrong_crate_type["target"]["crate_types"] = serde_json::json!(["lib"]);
        assert!(driver_artifact_path(&wrong_crate_type, &expected).is_err());
        assert!(driver_artifact_path(&record, &expected).is_ok());
        assert!(unique_driver_artifact(&[record.clone(), record.clone()], &expected).is_err());
        let mut outside = record;
        outside["executable"] = serde_json::json!("/hostile/cargo-fe2o3");
        assert!(driver_artifact_path(&outside, &expected).is_err());
    }
}
