//! Capture outside the protected runner; replay only the existing in-process
//! rustc child inside it. Preparation is descriptive, never proof authority.
use super::*;
use crate::production_rustc_driver_v1::gfx942_inline_value_qualification_v30_tests::{
    checked, read_bounded,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::Component;

const PREPARE: &str = "FE2O3_TEST_CONDITIONAL_VECADD_PREPARE_V1";
const INPUTS: &str = "FE2O3_TEST_CONDITIONAL_VECADD_INPUTS_V1";
const RESULTS: &str = "FE2O3_TEST_CONDITIONAL_VECADD_RESULTS_V1";
const SCHEMA: &str = "fe2o3-test-conditional-vecadd-preparation-v1";
const TARGETS: [&str; 2] = ["gfx942", "gfx950"];
// Exact sorted r6 TOOL_PINS support subset, not a full sysroot closure.
// The compiler executable has its own separate stamp.
const REPLAY_SUPPORT_PATHS: [&str; 5] = [
    "lib/libLLVM-22-rust-1.96.0-nightly.so",
    "lib/libLLVM.so.22.1-rust-1.96.0-nightly",
    "lib/librustc_driver-7bb70639c3ace5a4.so",
    "lib/rustlib/src/rust/library/core/Cargo.toml",
    "lib/rustlib/x86_64-unknown-linux-gnu/lib/libstd-fa01d964e82d0da8.so",
];
const JSON_CAP: usize = 16 * 1024 * 1024;
const TEXT_CAP: usize = 1024 * 1024;
const ENTRIES_CAP: usize = 50_000;
const FILE_CAP: u64 = 2 * 1024 * 1024 * 1024;
const TREE_CAP: u64 = 8 * 1024 * 1024 * 1024;
type Check<T> = Result<T, String>;
type Environment = Vec<(String, Vec<u8>)>;

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct FileStamp {
    path: PathBuf,
    bytes: u64,
    sha256: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Invocation {
    target: String,
    fixture: corpus::Fixture,
    cwd: PathBuf,
    args_sha256: [u8; 32],
    environment_sha256: [u8; 32],
    request: Request,
    files: Vec<FileStamp>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Preparation {
    schema: String,
    workspace: PathBuf,
    source: Vec<(String, [u8; 32])>,
    executable: FileStamp,
    cargo: FileStamp,
    rustc: FileStamp,
    toolchain_manifest: FileStamp,
    sysroot: PathBuf,
    replay_support_files: Vec<FileStamp>,
    invocations: Vec<Invocation>,
    proof_executed: bool,
    qualification_credit: bool,
    grants_artifact_or_launch_authority: bool,
    hardware_observed: bool,
}

fn require(ok: bool, reason: &str) -> Check<()> {
    if ok { Ok(()) } else { Err(reason.into()) }
}

fn io(error: std::io::Error) -> String {
    error.to_string()
}

fn json<T: Serialize>(value: &T) -> Check<serde_json::Value> {
    serde_json::to_value(value).map_err(|e| e.to_string())
}

fn canonical_directory(path: &Path) -> Check<()> {
    require(
        path.is_absolute()
            && path
                .components()
                .all(|c| matches!(c, Component::RootDir | Component::Normal(_))),
        "directory must be absolute without dot components",
    )?;
    require(
        fs::symlink_metadata(path).map_err(io)?.is_dir()
            && path.canonicalize().map_err(io)? == path,
        "directory must be canonical and not a symlink",
    )
}

fn fresh_directory(path: &Path) -> Check<()> {
    require(path.is_absolute(), "fresh directory must be absolute")?;
    canonical_directory(path.parent().ok_or("directory has no parent")?)?;
    DirBuilder::new().mode(0o700).create(path).map_err(io)?;
    canonical_directory(path)
}

fn new_file(path: &Path, bytes: &[u8]) -> Check<()> {
    require(bytes.len() <= JSON_CAP, "new record exceeds bound")?;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(io)?
        .write_all(bytes)
        .map_err(io)
}

fn write_json(path: &Path, value: &impl Serialize) -> Check<()> {
    new_file(path, &serde_json::to_vec(value).map_err(|e| e.to_string())?)
}

fn file_stamp(path: &Path, recorded_path: PathBuf, remaining: &mut u64) -> Check<FileStamp> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(io)?;
    let metadata = file.metadata().map_err(io)?;
    require(
        metadata.is_file() && metadata.len() <= FILE_CAP,
        "unbounded/nonregular file",
    )?;
    *remaining = remaining
        .checked_sub(metadata.len())
        .ok_or("tree byte bound")?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut read = 0u64;
    loop {
        let n = file.read(&mut buffer).map_err(io)?;
        if n == 0 {
            break;
        }
        read = read.checked_add(n as u64).ok_or("file size overflow")?;
        require(read <= metadata.len(), "file grew during read")?;
        digest.update(&buffer[..n]);
    }
    require(
        read == metadata.len() && file.metadata().map_err(io)?.len() == read,
        "file size changed during read",
    )?;
    Ok(FileStamp {
        path: recorded_path,
        bytes: read,
        sha256: crate::encode_hex(&digest.finalize()),
    })
}

fn absolute_stamp(path: &Path) -> Check<FileStamp> {
    require(
        path.is_absolute() && path.canonicalize().map_err(io)? == path,
        "tool/source path is not canonical",
    )?;
    let mut remaining = TREE_CAP;
    file_stamp(path, path.to_owned(), &mut remaining)
}

fn exact_support_roster(files: &[FileStamp]) -> Check<()> {
    require(
        files
            .iter()
            .map(|file| file.path.to_str())
            .eq(REPLAY_SUPPORT_PATHS.map(Some)),
        "expected exact sorted replay support file roster",
    )
}

fn replay_support_files(sysroot: &Path) -> Check<Vec<FileStamp>> {
    canonical_directory(sysroot)?;
    let mut remaining = TREE_CAP;
    REPLAY_SUPPORT_PATHS
        .into_iter()
        .map(|relative| {
            let path = sysroot.join(relative);
            require(
                path.canonicalize().map_err(io)? == path,
                "replay support path is not canonical",
            )?;
            file_stamp(&path, relative.into(), &mut remaining)
        })
        .collect()
}

fn tree(root: &Path) -> Check<Vec<FileStamp>> {
    canonical_directory(root)?;
    let mut pending = vec![root.to_owned()];
    let mut result = Vec::new();
    let mut entries = 0usize;
    let mut remaining = TREE_CAP;
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).map_err(io)? {
            entries += 1;
            require(entries <= ENTRIES_CAP, "tree entry bound")?;
            let entry = entry.map_err(io)?;
            let kind = entry.file_type().map_err(io)?;
            let path = entry.path();
            require(path.as_os_str().len() <= 4096, "tree path bound")?;
            if kind.is_dir() {
                pending.push(path);
            } else {
                require(kind.is_file(), "symlink or special file in preparation")?;
                let relative = path
                    .strip_prefix(root)
                    .map_err(|e| e.to_string())?
                    .to_owned();
                result.push(file_stamp(&path, relative, &mut remaining)?);
            }
        }
    }
    result.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(result)
}

fn empty_output(directory: &Path) -> Check<()> {
    canonical_directory(directory)?;
    require(
        fs::read_dir(directory).map_err(io)?.next().is_none(),
        "compiler output is not empty",
    )
}

fn dep_info_only(directory: &Path) -> Check<()> {
    require(
        tree(directory)?.iter().all(|file| {
            file.path.components().count() == 1
                && file
                    .path
                    .extension()
                    .is_some_and(|extension| extension == "d")
        }),
        "stopped compiler emitted a non-dep-info artifact",
    )
}

fn bounded_source_stamps() -> Check<Vec<(String, [u8; 32])>> {
    // Bound the existing parent's fixed source-stamp inputs before invoking it.
    for path in [
        MANIFEST.to_owned(),
        "Cargo.lock".into(),
        "examples/vecadd/Cargo.toml".into(),
        "examples/vecadd/src/lib.rs".into(),
        "examples/vecadd/src/vecadd_body.rs".into(),
        format!("{BASE}/src/conditional_vecadd_reference.rs"),
        format!("{BASE}/Cargo.toml"),
        format!("{BASE}/src/lib.rs"),
        format!("{BASE}/src/conditional_vecadd.rs"),
    ] {
        read_bounded(&workspace().join(path), TEXT_CAP)?;
    }
    Ok(source_stamps(Case::Annotated))
}

fn nul_records(bytes: &[u8]) -> Check<Vec<&[u8]>> {
    require(
        !bytes.is_empty() && bytes.len() <= TEXT_CAP && bytes.last() == Some(&0),
        "invalid bounded NUL capture",
    )?;
    let records: Vec<_> = bytes[..bytes.len() - 1].split(|b| *b == 0).collect();
    require(records.len() <= 4096, "capture record bound")?;
    Ok(records)
}

fn raw_arguments(bytes: &[u8]) -> Check<(PathBuf, Vec<String>)> {
    let records = nul_records(bytes)?;
    let (cwd, args) = records.split_first().ok_or("missing capture cwd")?;
    let cwd = PathBuf::from(OsString::from_vec(cwd.to_vec()));
    let args = args
        .iter()
        .map(|row| String::from_utf8(row.to_vec()).map_err(|e| e.to_string()))
        .collect::<Check<Vec<_>>>()?;
    require(
        !args.is_empty() && args.iter().all(|a| !a.is_empty()),
        "empty captured argument",
    )?;
    Ok((cwd, args))
}

fn raw_environment(bytes: &[u8]) -> Check<Environment> {
    let mut keys = BTreeSet::new();
    nul_records(bytes)?
        .into_iter()
        .map(|row| {
            let at = row
                .iter()
                .position(|b| *b == b'=')
                .ok_or("environment missing equals")?;
            let key = std::str::from_utf8(&row[..at])
                .map_err(|e| e.to_string())?
                .to_owned();
            require(
                !key.is_empty() && keys.insert(key.clone()),
                "duplicate/empty environment key",
            )?;
            Ok((key, row[at + 1..].to_vec()))
        })
        .collect()
}

fn option(args: &[String], name: &str) -> Check<Vec<String>> {
    let mut result = Vec::new();
    let mut index = 1;
    while index < args.len() {
        if args[index] == name {
            index += 1;
            let value = args.get(index).ok_or("missing compiler option value")?;
            require(!value.is_empty(), "empty compiler option")?;
            result.push(value.clone());
        } else if let Some(value) = args[index].strip_prefix(&format!("{name}=")) {
            require(!value.is_empty(), "empty compiler option")?;
            result.push(value.to_owned());
        }
        index += 1;
    }
    Ok(result)
}

fn codegen(args: &[String], name: &str) -> Check<Vec<String>> {
    let mut result = Vec::new();
    let mut index = 1;
    while index < args.len() {
        let value = if args[index] == "-C" {
            index += 1;
            Some(args.get(index).ok_or("missing codegen option")?.as_str())
        } else {
            args[index].strip_prefix("-C")
        };
        if let Some(value) = value.and_then(|v| v.strip_prefix(&format!("{name}="))) {
            result.push(value.to_owned());
        }
        index += 1;
    }
    Ok(result)
}

fn exact_selection(args: &[String], target: &str) -> Check<()> {
    require(TARGETS.contains(&target), "unknown target")?;
    let cfg = option(args, "--cfg")?;
    require(
        option(args, "--crate-name")? == ["fe2o3_production_extraction_fixture"]
            && option(args, "--crate-type")? == ["lib"]
            && option(args, "--target")? == ["amdgcn-amd-amdhsa"]
            && codegen(args, "target-cpu")? == [target]
            && codegen(args, "target-feature")? == ["-xnack,+wavefrontsize64,-wavefrontsize32"]
            && codegen(args, "overflow-checks")? == ["on"]
            && option(args, "--emit")? == ["dep-info,metadata"]
            && cfg == ["feature=\"conditional-vecadd\""],
        "not the exact annotated Vecadd target/flags/features",
    )?;
    require(
        !args.iter().any(|a| a.starts_with('@')),
        "response files are not captured argv",
    )?;
    closed_options(args)
}

fn closed_options(args: &[String]) -> Check<()> {
    let mut index = 1;
    while index < args.len() {
        let arg = args[index].as_str();
        if arg.starts_with("-C") || arg.starts_with("-Z") {
            let family = &arg[..2];
            let value = if arg.len() == 2 {
                index += 1;
                args.get(index).ok_or("missing compiler switch")?.as_str()
            } else {
                &arg[2..]
            };
            let key = value.split('=').next().unwrap();
            require(
                if family == "-Z" {
                    matches!(value, "always-encode-mir" | "unstable-options")
                } else {
                    matches!(
                        key,
                        "metadata"
                            | "extra-filename"
                            | "opt-level"
                            | "embed-bitcode"
                            | "debug-assertions"
                            | "overflow-checks"
                            | "strip"
                            | "target-cpu"
                            | "target-feature"
                            | "panic"
                            | "debuginfo"
                            | "codegen-units"
                    )
                },
                "uncaptured compiler side-effect option",
            )?;
        } else if arg.starts_with('-') {
            let (name, joined) = arg.split_once('=').map_or((arg, false), |(k, _)| (k, true));
            require(
                matches!(
                    name,
                    "--crate-name"
                        | "--edition"
                        | "--error-format"
                        | "--json"
                        | "--crate-type"
                        | "--emit"
                        | "--cfg"
                        | "--check-cfg"
                        | "--out-dir"
                        | "--target"
                        | "--extern"
                        | "--sysroot"
                        | "--cap-lints"
                        | "-L"
                ),
                "unknown compiler option",
            )?;
            if !joined {
                index += 1;
                require(
                    args.get(index).is_some_and(|v| !v.is_empty()),
                    "missing compiler switch",
                )?;
            }
        }
        index += 1;
    }
    Ok(())
}

fn normalized_arguments(raw: &[String], sysroot: &Path, output: &Path) -> Check<Vec<String>> {
    require(
        option(raw, "--out-dir")?.len() == 1,
        "ambiguous raw output directory",
    )?;
    let mut args = raw.to_vec();
    match option(raw, "--sysroot")?.as_slice() {
        [] => args.extend([
            "--sysroot".into(),
            sysroot.to_str().ok_or("sysroot UTF-8")?.into(),
        ]),
        [path] if Path::new(path) == sysroot => {}
        _ => return Err("captured sysroot mismatch".into()),
    }
    for index in 1..args.len() {
        if args[index] == "--out-dir" {
            args[index + 1] = output.to_str().ok_or("output UTF-8")?.into();
            return Ok(args);
        }
        if args[index].starts_with("--out-dir=") {
            args[index] = format!("--out-dir={}", output.display());
            return Ok(args);
        }
    }
    Err("missing output directory".into())
}

fn keep_environment(key: &str) -> bool {
    if key.contains("RUSTFLAGS")
        || key.contains("WRAPPER")
        || key.ends_with("MAKEFLAGS")
        || matches!(
            key,
            "MFLAGS" | "RUSTC" | "CARGO_BUILD_RUSTC" | "CARGO_BUILD_TARGET"
        )
    {
        return false;
    }
    key.starts_with("CARGO_")
        || matches!(
            key,
            "HOME"
                | "PATH"
                | "CARGO_HOME"
                | "RUSTUP_HOME"
                | "RUSTUP_TOOLCHAIN"
                | "LD_LIBRARY_PATH"
                | "FE2O3_HIP_SYS_DISABLE"
                | "FE2O3_HSA_RUNTIME_DISABLE"
        )
}

fn replay_arguments(
    captured: &[u8],
    request: &Request,
    sysroot: &Path,
    output: &Path,
) -> Check<(Vec<u8>, Request)> {
    require(
        <[u8; 32]>::from(Sha256::digest(captured)) == request.args_sha256,
        "replay input arguments changed",
    )?;
    let args: Vec<String> = serde_json::from_slice(captured).map_err(|e| e.to_string())?;
    // rustc writes dep-info before after_analysis. Keep the prepared inputs
    // read-only and bind the child's request to its isolated output directory.
    let args = serde_json::to_vec(&normalized_arguments(&args, sysroot, output)?)
        .map_err(|e| e.to_string())?;
    let request = Request {
        case: request.case,
        stage: request.stage,
        source: request.source.clone(),
        args_sha256: Sha256::digest(&args).into(),
    };
    Ok((args, request))
}

fn normalized_environment(raw: Environment, args: &[String]) -> Check<Environment> {
    let actual: Vec<_> = args.iter().map(OsString::from).collect();
    let RustcInvocationV2::Compile(compile) =
        classify_rustc_invocation_v2(&actual).map_err(|e| e.to_string())?
    else {
        return Err("not a source compile".into());
    };
    let metadata = ordered_rustc_codegen_metadata_v1(compile).map_err(|e| e.to_string())?;
    require(
        !metadata.is_empty(),
        "missing actual Cargo codegen metadata",
    )?;
    let mut rows: BTreeMap<_, _> = raw
        .into_iter()
        .filter(|(key, _)| keep_environment(key))
        .collect();
    rows.insert(
        CRATE_BINDING_ID_ENV_V1.into(),
        derive_crate_binding_id_v1(compile.crate_name(), metadata.iter().map(String::as_str))
            .to_hex()
            .into_bytes(),
    );
    rows.insert(
        CARGO_METADATA_BUILD_OBSERVATION_ENV_V2.into(),
        derive_cargo_metadata_build_observation_v2(&metadata)
            .to_hex()
            .into_bytes(),
    );
    Ok(rows.into_iter().collect())
}

fn parse_preparation(bytes: &[u8]) -> Check<Preparation> {
    require(bytes.len() <= JSON_CAP, "preparation JSON bound")?;
    let record: Preparation = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    // Also reject unknown nested fields in the existing shared Fixture schema.
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    require(json(&record)? == value, "noncanonical preparation fields")?;
    require(
        record.schema == SCHEMA
            && !record.proof_executed
            && !record.qualification_credit
            && !record.grants_artifact_or_launch_authority
            && !record.hardware_observed,
        "wrong preparation schema or authority claim",
    )?;
    require(
        record.invocations.len() == TARGETS.len()
            && record
                .invocations
                .iter()
                .map(|i| i.target.as_str())
                .eq(TARGETS),
        "expected exact ordered gfx942/gfx950 selections",
    )?;
    require(
        record.source.len() == 9
            && record
                .invocations
                .iter()
                .all(|i| i.files.len() <= ENTRIES_CAP),
        "preparation roster bound",
    )?;
    exact_support_roster(&record.replay_support_files)?;
    for invocation in &record.invocations {
        require(
            invocation.request.case == Case::Annotated
                && invocation.request.stage == Stage::Consuming
                && invocation.request.source == record.source
                && invocation.request.args_sha256 == invocation.args_sha256,
            "child request mismatch",
        )?;
    }
    Ok(record)
}

fn checked_invocation(root: &Path, record: &Preparation, invocation: &Invocation) -> Check<()> {
    let directory = root.join(&invocation.target);
    canonical_directory(&directory)?;
    require(
        invocation.files == tree(&directory)?,
        "changed captured inputs or dependencies",
    )?;
    empty_output(&directory.join("compiler-output"))?;
    let args_bytes = read_bounded(&directory.join("args.json"), TEXT_CAP)?;
    let environment_bytes = read_bounded(&directory.join("environment.json"), TEXT_CAP)?;
    require(
        invocation.args_sha256 == <[u8; 32]>::from(Sha256::digest(&args_bytes))
            && invocation.environment_sha256
                == <[u8; 32]>::from(Sha256::digest(&environment_bytes)),
        "normalized invocation digest mismatch",
    )?;
    let args: Vec<String> = serde_json::from_slice(&args_bytes).map_err(|e| e.to_string())?;
    let environment: Environment =
        serde_json::from_slice(&environment_bytes).map_err(|e| e.to_string())?;
    let (cwd, raw) = raw_arguments(&read_bounded(&directory.join("cargo-root.argv"), TEXT_CAP)?)?;
    require(
        cwd == invocation.cwd && cwd == record.workspace,
        "captured cwd mismatch",
    )?;
    require(
        args == normalized_arguments(&raw, &record.sysroot, &directory.join("compiler-output"))?,
        "normalized argv differs from actual Cargo capture",
    )?;
    exact_selection(&args, &invocation.target)?;
    require(
        raw.first().map(Path::new) == Some(record.rustc.path.as_path()),
        "captured compiler differs",
    )?;
    let actual: Vec<_> = args.iter().map(OsString::from).collect();
    let RustcInvocationV2::Compile(compile) =
        classify_rustc_invocation_v2(&actual).map_err(|e| e.to_string())?
    else {
        return Err("not a source compile".into());
    };
    require(
        cwd.join(compile.source_path()).canonicalize().map_err(io)?
            == record.workspace.join(BASE).join("src/lib.rs"),
        "captured source path mismatch",
    )?;
    let raw_env = raw_environment(&read_bounded(&directory.join("cargo-root.env"), TEXT_CAP)?)?;
    require(
        environment == normalized_environment(raw_env, &args)?,
        "normalized environment mismatch",
    )?;
    let package_dir = record.workspace.join(BASE);
    require(
        environment
            .iter()
            .find(|(key, _)| key == "CARGO_MANIFEST_DIR")
            .map(|(_, v)| v.as_slice())
            == Some(package_dir.as_os_str().as_bytes()),
        "captured package environment mismatch",
    )?;
    require(
        json(&invocation.fixture)? == json(&fixture(Case::Annotated, &invocation.target))?,
        "fixture differs from exact shared annotated source selection",
    )?;
    let dependencies = directory.join("dependencies");
    canonical_directory(&dependencies)?;
    let mut externs = BTreeSet::new();
    for value in option(&args, "--extern")? {
        let (name, file) = value
            .split_once('=')
            .ok_or("extern without exact artifact")?;
        require(externs.insert(name.to_owned()), "duplicate extern")?;
        let file = PathBuf::from(file);
        require(
            file.is_absolute()
                && file.starts_with(&dependencies)
                && file.canonicalize().map_err(io)? == file
                && file.is_file(),
            "extern escaped captured dependencies",
        )?;
    }
    require(!externs.is_empty(), "no captured dependency artifacts")?;
    for value in option(&args, "-L")? {
        let (_, path) = value.split_once('=').ok_or("untyped search path")?;
        let path = Path::new(path);
        require(
            path.starts_with(&dependencies),
            "compiler search path escaped dependencies",
        )?;
        canonical_directory(path)?;
    }
    Ok(())
}

fn checked_preparation(root: &Path) -> Check<Preparation> {
    canonical_directory(root)?;
    let record = parse_preparation(&read_bounded(&root.join("preparation.json"), JSON_CAP)?)?;
    require(record.workspace == workspace(), "workspace mismatch")?;
    require(
        !root.starts_with(&record.workspace),
        "preparation must be outside sources",
    )?;
    require(
        record.source == bounded_source_stamps()?,
        "stale source preparation",
    )?;
    require(
        record.executable.path == env::current_exe().map_err(io)?.canonicalize().map_err(io)?
            && record.executable == absolute_stamp(&record.executable.path)?,
        "replay executable changed",
    )?;
    require(
        record.cargo.path == Path::new(env!("CARGO")).canonicalize().map_err(io)?
            && record.cargo == absolute_stamp(&record.cargo.path)?,
        "Cargo tool provenance changed",
    )?;
    canonical_directory(&record.sysroot)?;
    require(
        record
            .sysroot
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.starts_with("nightly-2026-04-03-")),
        "not the pinned compiler sysroot",
    )?;
    require(
        record.rustc.path == record.sysroot.join("bin/rustc")
            && record.rustc == absolute_stamp(&record.rustc.path)?,
        "compiler provenance changed",
    )?;
    require(
        record.toolchain_manifest == absolute_stamp(&record.workspace.join("rust-toolchain.toml"))?,
        "toolchain manifest changed",
    )?;
    require(
        record.replay_support_files == replay_support_files(&record.sysroot)?,
        "named replay support files changed",
    )?;
    for invocation in &record.invocations {
        checked_invocation(root, &record, invocation)?;
    }
    require(
        record.invocations[0].args_sha256 != record.invocations[1].args_sha256,
        "target invocations are identical",
    )?;
    Ok(record)
}

#[test]
#[ignore = "genuine offline Cargo dependency capture; primary serialized build guard only"]
fn prepare_actual_shared_body_vecadd_inputs() {
    let root = PathBuf::from(env::var_os(PREPARE).expect("fresh absolute Vecadd preparation root"));
    assert!(
        !root.starts_with(workspace()),
        "preparation outside source tree"
    );
    fresh_directory(&root).unwrap();
    let source = bounded_source_stamps().unwrap();
    let mut invocations = Vec::new();
    let mut sysroot = None;
    for target in TARGETS {
        let directory = root.join(target);
        fresh_directory(&directory).unwrap();
        let selected = fixture(Case::Annotated, target);
        let captured = corpus_cargo::capture(
            &workspace(),
            &selected,
            &directory,
            &directory.join("dependencies"),
        )
        .unwrap();
        let args_bytes = serde_json::to_vec(&captured.args).unwrap();
        exact_selection(&captured.args, target).unwrap();
        let roots = option(&captured.args, "--sysroot").unwrap();
        let [path] = roots.as_slice() else {
            panic!("one actual compiler sysroot")
        };
        let path = PathBuf::from(path);
        if let Some(previous) = &sysroot {
            assert_eq!(previous, &path);
        }
        sysroot = Some(path);
        let raw_env =
            raw_environment(&read_bounded(&directory.join("cargo-root.env"), TEXT_CAP).unwrap())
                .unwrap();
        let environment = normalized_environment(raw_env, &captured.args).unwrap();
        let captured_env = captured
            .environment
            .into_iter()
            .map(|(k, v)| {
                (
                    k.into_string().expect("ASCII Cargo environment key"),
                    v.into_vec(),
                )
            })
            .collect();
        assert_eq!(
            environment,
            normalized_environment(captured_env, &captured.args).unwrap()
        );
        let environment_bytes = serde_json::to_vec(&environment).unwrap();
        new_file(&directory.join("args.json"), &args_bytes).unwrap();
        new_file(&directory.join("environment.json"), &environment_bytes).unwrap();
        new_file(
            &directory.join("cargo-diagnostics.txt"),
            captured.cargo_diagnostics.as_bytes(),
        )
        .unwrap();
        let args_sha256 = Sha256::digest(&args_bytes).into();
        invocations.push(Invocation {
            target: target.into(),
            fixture: selected,
            cwd: captured.cwd,
            args_sha256,
            environment_sha256: Sha256::digest(&environment_bytes).into(),
            request: Request {
                case: Case::Annotated,
                stage: Stage::Consuming,
                source: source.clone(),
                args_sha256,
            },
            files: tree(&directory).unwrap(),
        });
    }
    let sysroot = sysroot.unwrap();
    let record = Preparation {
        schema: SCHEMA.into(),
        workspace: workspace(),
        source,
        executable: absolute_stamp(&env::current_exe().unwrap().canonicalize().unwrap()).unwrap(),
        cargo: absolute_stamp(&Path::new(env!("CARGO")).canonicalize().unwrap()).unwrap(),
        rustc: absolute_stamp(&sysroot.join("bin/rustc")).unwrap(),
        toolchain_manifest: absolute_stamp(&workspace().join("rust-toolchain.toml")).unwrap(),
        replay_support_files: replay_support_files(&sysroot).unwrap(),
        sysroot,
        invocations,
        proof_executed: false,
        qualification_credit: false,
        grants_artifact_or_launch_authority: false,
        hardware_observed: false,
    };
    assert_eq!(record.source, bounded_source_stamps().unwrap());
    write_json(&root.join("preparation.json"), &record).unwrap();
    checked_preparation(&root).unwrap();
    eprintln!("VECADD PREPARED INPUTS: {}", root.display());
}

#[test]
#[ignore = "prepared consuming source replay; protected runtime admission required, no Cargo or GPU"]
fn actual_prepared_shared_body_vecadd_retains_formula_then_requires_conditional_finalizer() {
    let root = PathBuf::from(env::var_os(INPUTS).expect("prepared Vecadd inputs"));
    let results = PathBuf::from(env::var_os(RESULTS).expect("fresh Vecadd results root"));
    let before = checked_preparation(&root).unwrap();
    assert!(
        !results.starts_with(&root)
            && !root.starts_with(&results)
            && !results.starts_with(&before.workspace),
        "disjoint input/source/result roots required"
    );
    fresh_directory(&results).unwrap();
    let mut reports = Vec::new();
    for invocation in &before.invocations {
        let input = root.join(&invocation.target);
        empty_output(&input.join("compiler-output")).unwrap();
        let output = results.join(&invocation.target);
        fresh_directory(&output).unwrap();
        fresh_directory(&output.join("tmp")).unwrap();
        let compiler_output = output.join("compiler-output");
        fresh_directory(&compiler_output).unwrap();
        let (args, request) = replay_arguments(
            &read_bounded(&input.join("args.json"), TEXT_CAP).unwrap(),
            &invocation.request,
            &before.sysroot,
            &compiler_output,
        )
        .unwrap();
        new_file(&output.join("args.json"), &args).unwrap();
        let environment: Environment = serde_json::from_slice(
            &read_bounded(&input.join("environment.json"), TEXT_CAP).unwrap(),
        )
        .unwrap();
        let mut command = Command::new(env::current_exe().unwrap());
        command
            .env_clear()
            .envs(
                environment
                    .into_iter()
                    .map(|(key, value)| (key, OsString::from_vec(value))),
            )
            .current_dir(&invocation.cwd)
            .env("TMPDIR", output.join("tmp"))
            .env(CHILD_ARGS, output.join("args.json"))
            .env(CHILD_RESULT, output.join("Consuming.json"))
            .env(REQUEST, serde_json::to_string(&request).unwrap())
            .args([
                "--exact",
                CHILD,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ]);
        progress::clear_inherited_jobserver(&mut command);
        write_json(&output.join("request.json"), &request).unwrap();
        let child = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            checked(&mut command, &output, "Consuming", None)
        }));
        empty_output(&input.join("compiler-output")).unwrap();
        dep_info_only(&compiler_output).unwrap();
        assert_eq!(
            args,
            read_bounded(&output.join("args.json"), TEXT_CAP).unwrap()
        );
        assert_eq!(
            invocation.files,
            tree(&input).unwrap(),
            "child changed prepared inputs"
        );
        assert_eq!(before.source, bounded_source_stamps().unwrap());
        let stdout = child.unwrap_or_else(|panic| std::panic::resume_unwind(panic));
        assert!(
            std::str::from_utf8(&stdout)
                .unwrap()
                .contains("1 passed; 0 failed; 0 ignored"),
            "exactly one actual Vecadd child required"
        );
        let response: Result<Report, String> = serde_json::from_slice(
            &read_bounded(&output.join("Consuming.json"), TEXT_CAP).unwrap(),
        )
        .unwrap();
        let response = response.unwrap();
        assert_eq!(
            (response.case, response.stage),
            (Case::Annotated, Stage::Consuming)
        );
        assert!(
            !response.default_manifest_selection
                && !response.qualification_credit
                && !response.grants_artifact_or_launch_authority
        );
        assert_eq!(
            response.detail["conditional_formula"]["proof_retained_after_callback"],
            true
        );
        assert!(
            response.detail["boundary"]
                .as_str()
                .unwrap()
                .contains("FE2O3-COND-FINALIZER-001")
        );
        empty_output(&input.join("compiler-output")).unwrap();
        assert_eq!(before.source, bounded_source_stamps().unwrap());
        reports.push(serde_json::json!({"target": invocation.target, "report": response}));
    }
    assert_eq!(
        json(&before).unwrap(),
        json(&checked_preparation(&root).unwrap()).unwrap(),
        "prepared inputs changed during protected replay"
    );
    write_json(&results.join("report.json"), &serde_json::json!({
        "schema": "fe2o3-test-conditional-vecadd-prepared-results-v1",
        "preparation_sha256": crate::encode_hex(&Sha256::digest(read_bounded(&root.join("preparation.json"), JSON_CAP).unwrap())),
        "targets": reports, "actual_rustc_callback": true, "qualification_credit": false,
        "default_manifest_selection": false, "grants_artifact_or_launch_authority": false,
        "hardware_observed": false, "source_admission_complete": false,
    })).unwrap();
}

#[path = "production_rustc_driver_conditional_vecadd_prepared_parsing_v1_tests.rs"]
mod parsing_tests;
