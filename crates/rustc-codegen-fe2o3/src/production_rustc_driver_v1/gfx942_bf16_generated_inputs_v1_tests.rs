//! Fresh task-package invocation derivation; no historical fixture authority.
use super::*;
use crate::production_rustc_driver_v1::gfx942_bf16_call_source_cpu_qualification_v1_tests::inputs as tree;
use crate::production_rustc_driver_v1::gfx942_inline_value_qualification_v30_tests::{
    invocation_for_fixture_source_with_dependencies, read_bounded,
};

pub(super) const CONFIG_CAP: usize = 32768;
pub(super) const RECORD_CAP: usize = 131072;
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Pin {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Spec {
    pub schema: String,
    pub cwd: String,
    pub package: String,
    pub directory: String,
    pub dependencies: String,
    pub package_name: String,
    pub crate_name: String,
    pub source: Pin,
    pub manifest: Pin,
    pub lock: Pin,
}
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    pub schema: String,
    pub spec: Spec,
    pub args: Vec<String>,
    pub crate_binding: String,
    pub cargo_observation: String,
    pub package_version: String,
    pub dependencies: tree::TreePin,
    pub metadata_sha256: String,
    pub artifacts_sha256: String,
    pub sysroot_sha256: String,
}
pub(super) fn lower_digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub(super) fn relative(text: &str) -> Result<&Path, &'static str> {
    let p = Path::new(text);
    if text.is_empty()
        || text.len() > 1024
        || p.is_absolute()
        || p.components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err("bounded task-relative path");
    }
    Ok(p)
}
pub(super) fn checked_path(cwd: &Path, text: &str, file: bool) -> Result<PathBuf, &'static str> {
    let rel = relative(text)?;
    let mut path = cwd.to_path_buf();
    let parts = rel.components().collect::<Vec<_>>();
    if parts.len() > 32 {
        return Err("task path depth");
    }
    for (i, part) in parts.iter().enumerate() {
        path.push(part.as_os_str());
        let metadata = std::fs::symlink_metadata(&path).map_err(|_| "task path metadata")?;
        if metadata.file_type().is_symlink()
            || if i + 1 == parts.len() && file {
                !metadata.is_file()
            } else {
                !metadata.is_dir()
            }
        {
            return Err("task path type or symlink");
        }
    }
    Ok(path)
}
pub(super) fn absent_output(cwd: &Path, text: &str) -> Result<PathBuf, &'static str> {
    let rel = relative(text)?;
    let parent = rel
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or("output parent")?;
    let parent = checked_path(cwd, parent.to_str().ok_or("output UTF-8")?, false)?;
    let path = parent.join(rel.file_name().ok_or("output basename")?);
    match std::fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(path),
        _ => Err("create-new output already exists or unavailable"),
    }
}
fn verify_pin(cwd: &Path, pin: &Pin) -> Result<(), String> {
    if pin.bytes == 0 || pin.bytes > 1024 * 1024 || !lower_digest(&pin.sha256) {
        return Err("task selected pin shape".into());
    }
    let path = checked_path(cwd, &pin.path, true).map_err(str::to_owned)?;
    let bytes = read_bounded(&path, 1024 * 1024).map_err(|e| format!("{e:?}"))?;
    if bytes.len() as u64 != pin.bytes || digest(&bytes) != pin.sha256 {
        return Err("task selected file changed".into());
    }
    Ok(())
}
pub(super) fn validate_spec(spec: &Spec) -> Result<PathBuf, String> {
    if spec.schema != "fe2o3-bf16-generated-invocation-spec-v1" {
        return Err("invocation spec schema".into());
    }
    let cwd = PathBuf::from(&spec.cwd);
    if !cwd.is_absolute()
        || spec.cwd.len() > 4096
        || cwd.canonicalize().map_err(|e| e.to_string())? != cwd
        || std::env::current_dir().map_err(|e| e.to_string())? != cwd
    {
        return Err("explicit actual task cwd".into());
    }
    checked_path(&cwd, &spec.package, false).map_err(str::to_owned)?;
    checked_path(&cwd, &spec.directory, false).map_err(str::to_owned)?;
    checked_path(&cwd, &spec.dependencies, false).map_err(str::to_owned)?;
    if spec.source.path != format!("{}/src/lib.rs", spec.package)
        || spec.manifest.path != format!("{}/Cargo.toml", spec.package)
        || spec.lock.path != format!("{}/Cargo.lock", spec.package)
        || spec.source.bytes > 65536
        || spec.package_name.is_empty()
        || spec.package_name.len() > 128
        || !spec
            .package_name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        || spec.crate_name.is_empty()
        || spec.crate_name.len() > 128
        || !spec
            .crate_name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err("actual package/source selection".into());
    }
    for pin in [&spec.source, &spec.manifest, &spec.lock] {
        verify_pin(&cwd, pin)?;
    }
    Ok(cwd)
}
pub(super) fn derive(spec: &Spec) -> Result<Record, String> {
    let cwd = validate_spec(spec)?;
    let directory = cwd.join(&spec.directory);
    let dependency_directory = cwd.join(&spec.dependencies);
    let metadata = read_bounded(&directory.join("metadata.stdout"), 16 * 1024 * 1024)
        .map_err(|e| format!("{e:?}"))?;
    let parsed: Value = serde_json::from_slice(&metadata).map_err(|e| e.to_string())?;
    let manifest = cwd.join(&spec.manifest.path);
    let source = cwd.join(&spec.source.path);
    let packages = parsed["packages"]
        .as_array()
        .ok_or("metadata package array")?;
    let selected = packages
        .iter()
        .filter(|p| p["manifest_path"].as_str() == manifest.to_str())
        .collect::<Vec<_>>();
    let [package] = selected.as_slice() else {
        return Err("exact actual metadata package".into());
    };
    if package["name"] != spec.package_name || package["version"] != "0.0.0" {
        return Err("metadata package identity/version".into());
    }
    let targets = package["targets"].as_array().ok_or("metadata targets")?;
    let selected = targets
        .iter()
        .filter(|t| t["kind"] == json!(["lib"]))
        .collect::<Vec<_>>();
    let [target] = selected.as_slice() else {
        return Err("one actual metadata lib target".into());
    };
    if target["name"] != spec.crate_name || target["src_path"].as_str() != source.to_str() {
        return Err("metadata selected source target".into());
    }
    // The actual direct invocation below has no --cfg feature. A --no-deps
    // metadata response may have null resolve; do not invent resolved features.
    if !parsed["resolve"].is_null() {
        let nodes = parsed["resolve"]["nodes"]
            .as_array()
            .ok_or("metadata resolve nodes")?;
        let selected = nodes
            .iter()
            .filter(|n| n["id"] == package["id"])
            .collect::<Vec<_>>();
        let [node] = selected.as_slice() else {
            return Err("actual root resolve node".into());
        };
        if node["features"] != json!([]) {
            return Err("unexpected enabled root features".into());
        }
    }
    let (args, crate_binding, cargo_observation) = invocation_for_fixture_source_with_dependencies(
        (&directory, &dependency_directory),
        &cwd.join(&spec.package),
        &spec.package_name,
        &spec.crate_name,
        None,
        &source,
        "gfx942",
    );
    if args.len() > 128
        || args.iter().any(|a| a.len() > 4096)
        || args.iter().map(String::len).sum::<usize>() > 32768
        || args.iter().any(|a| a == "--cfg" || a.starts_with("--cfg="))
    {
        return Err("bounded no-feature actual argv".into());
    }
    let artifacts = read_bounded(
        &dependency_directory.join("dependencies.stdout"),
        16 * 1024 * 1024,
    )
    .map_err(|e| format!("{e:?}"))?;
    let sysroot =
        read_bounded(&directory.join("sysroot.stdout"), 4096).map_err(|e| format!("{e:?}"))?;
    let dependencies = tree::dependency_snapshot(&dependency_directory).0;
    // Recheck source/manifest/lock after deriving actual argv and dependency pins.
    validate_spec(spec)?;
    Ok(Record {
        schema: "fe2o3-bf16-generated-invocation-v1".into(),
        spec: spec.clone(),
        args,
        crate_binding,
        cargo_observation,
        package_version: "0.0.0".into(),
        dependencies,
        metadata_sha256: digest(&metadata),
        artifacts_sha256: digest(&artifacts),
        sysroot_sha256: digest(&sysroot),
    })
}
pub(super) fn read_record(cwd: &Path, path: &str, sha256: &str) -> Result<Record, String> {
    if !lower_digest(sha256) {
        return Err("record digest".into());
    }
    let bytes = read_bounded(
        &checked_path(cwd, path, true).map_err(str::to_owned)?,
        RECORD_CAP,
    )
    .map_err(|e| format!("{e:?}"))?;
    if digest(&bytes) != sha256 {
        return Err("record bytes changed".into());
    }
    let record: Record = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if record.schema != "fe2o3-bf16-generated-invocation-v1" || derive(&record.spec)? != record {
        return Err("actual current invocation differs from prepared record".into());
    }
    Ok(record)
}
pub(super) fn environment(record: &Record) -> Result<(), String> {
    for (key, value) in [
        (
            reserved_fe2o3_symbols::CRATE_BINDING_ID_ENV_V1,
            record.crate_binding.as_str(),
        ),
        (
            fe2o3_rustc_invocation::CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
            record.cargo_observation.as_str(),
        ),
        ("CARGO_PKG_NAME", record.spec.package_name.as_str()),
        ("CARGO_PKG_VERSION", record.package_version.as_str()),
        ("CARGO_CRATE_NAME", record.spec.crate_name.as_str()),
    ] {
        if std::env::var(key).map_err(|_| "required actual invocation environment")? != value {
            return Err(format!("actual invocation environment differs: {key}"));
        }
    }
    if std::env::var_os("CARGO_MANIFEST_DIR").map(PathBuf::from)
        != Some(Path::new(&record.spec.cwd).join(&record.spec.package))
    {
        return Err("actual manifest directory environment".into());
    }
    super::super::require_canonical_overflow_checks_v1(&record.args)
}
