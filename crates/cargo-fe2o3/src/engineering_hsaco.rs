//! Explicitly non-authoritative compiler-to-HSACO engineering command.

use std::env;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fe2o3_hsaco_finalize::{
    ContentIdentityV1, EngineeringHsacoObservationV1, MAX_WORKER_EXECUTABLE_BYTES,
    MAX_WORKER_OUTPUT_BYTES, MAX_WORKER_RESPONSE_BYTES, PinnedWorkerV1, WorkerExecutionLimitsV1,
    WorkerInputKindV1, WorkerInputV1, WorkerMeasurementV1, WorkerOutputConstraintsV1,
    observe_engineering_hsaco_v1,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

const NAMESPACE: &str = "fe2o3-engineering-v1";
const MANIFEST_SCHEMA: &str = "EngineeringHsacoObservationV1";
const TARGET: &str = "gfx942:xnack-";
const CARGO_TARGET: &str = "amdgcn-amd-amdhsa";
const CODE_OBJECT_VERSION: u8 = 6;
const MAX_HANDOFF_BYTES: u64 = 64 * 1024 * 1024;
const MAX_TOOL_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_RUSTC_SYSROOT_OUTPUT_BYTES: u64 = 4096;
const CONTENT_ID_DOMAIN: &[u8] = b"FE2O3/ENGINEERING-HSACO-OBSERVATION-CONTENT/V1\0";

pub(crate) fn command(args: &[OsString]) -> ExitCode {
    match run(args) {
        Ok(path) => {
            println!("{}", path.display());
            eprintln!(
                "cargo-fe2o3: wrote a non-authoritative engineering observation; publication, load, and launch authority remain false"
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("cargo-fe2o3 engineering hsaco: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug)]
struct Options {
    crate_name: String,
    output_root: PathBuf,
    extractor: FileClaim,
    extractor_backend: FileClaim,
    worker: FileClaim,
    cargo: FileClaim,
    rustc: FileClaim,
    worker_build_identity: String,
    llvm_build_identity: String,
    providers: Vec<ProviderClaim>,
    timeout: Duration,
    max_output_bytes: u64,
    cargo_args: Vec<OsString>,
}

#[derive(Debug)]
struct FileClaim {
    path: PathBuf,
    sha256: [u8; 32],
}

#[derive(Debug)]
struct ProviderClaim {
    kind: WorkerInputKindV1,
    path: PathBuf,
    sha256: [u8; 32],
}

fn run(args: &[OsString]) -> Result<PathBuf, String> {
    reject_conflicting_environment()?;
    let current_dir = env::current_dir()
        .map_err(|error| format!("cannot identify invocation directory: {error}"))?;
    let options = parse(args, &current_dir)?;
    validate_fresh_output_root(&options.output_root)?;

    let scratch = ScratchDirectory::new()?;
    let cargo_bytes = read_claimed_file("Cargo", &options.cargo, MAX_TOOL_BYTES, true)?;
    let rustc_bytes = read_claimed_file("rustc", &options.rustc, MAX_TOOL_BYTES, true)?;
    let extractor_bytes = read_claimed_file("extractor", &options.extractor, MAX_TOOL_BYTES, true)?;
    let extractor_backend_bytes = read_claimed_file(
        "extractor backend",
        &options.extractor_backend,
        MAX_TOOL_BYTES,
        false,
    )?;
    let worker_bytes = read_claimed_file(
        "native worker",
        &options.worker,
        MAX_WORKER_EXECUTABLE_BYTES,
        true,
    )?;

    let pinned_extractor = scratch.path.join("fe2o3-rustc-extract");
    write_new_file(&pinned_extractor, &extractor_bytes, 0o500)?;
    let pinned_extractor_backend = scratch.path.join("librustc_codegen_fe2o3.so");
    write_new_file(&pinned_extractor_backend, &extractor_backend_bytes, 0o400)?;
    let handoff_path = scratch.path.join("compiler-handoff-v2");
    run_extraction(&options, &pinned_extractor, &handoff_path, &scratch.path)?;
    let handoff = read_bounded_regular_file(&handoff_path, MAX_HANDOFF_BYTES, false)?;

    let mut providers = Vec::new();
    let mut total_provider_bytes = 0_u64;
    for claim in &options.providers {
        let bytes = read_claimed_provider(claim)?;
        total_provider_bytes = total_provider_bytes
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| "engineering provider byte count overflowed".to_owned())?;
        if total_provider_bytes > fe2o3_hsaco_finalize::MAX_WORKER_TOTAL_INPUT_BYTES as u64 {
            return Err("engineering provider bytes exceed the worker protocol bound".to_owned());
        }
        providers.push(WorkerInputV1::new(claim.kind, bytes).map_err(|error| {
            format!("engineering provider does not satisfy the worker protocol: {error}")
        })?);
    }

    let worker_measurement = WorkerMeasurementV1::new(
        ContentIdentityV1::from_parts(options.worker.sha256, worker_bytes.len() as u64),
        &options.worker_build_identity,
        &options.llvm_build_identity,
    )
    .map_err(|error| format!("invalid native worker measurement: {error}"))?;
    let worker = PinnedWorkerV1::open(&options.worker.path, worker_measurement)
        .map_err(|error| format!("cannot pin native worker: {error}"))?;
    let limits = WorkerExecutionLimitsV1::new(
        options.timeout,
        MAX_WORKER_RESPONSE_BYTES,
        fe2o3_hsaco_finalize::DEFAULT_WORKER_STDERR_BYTES,
    )
    .map_err(|error| format!("invalid native worker limits: {error}"))?;
    let output_bound = WorkerOutputConstraintsV1::new(options.max_output_bytes)
        .map_err(|error| format!("invalid native worker output bound: {error}"))?;
    let observation =
        observe_engineering_hsaco_v1(&handoff, &worker, providers, output_bound, limits)
            .map_err(|error| error.to_string())?;

    let manifest = canonical_manifest(
        &options,
        &observation,
        &cargo_bytes,
        &rustc_bytes,
        &extractor_bytes,
        &extractor_backend_bytes,
    )?;
    publish_observation(&options.output_root, &manifest, observation.hsaco_bytes())
}

fn parse(args: &[OsString], current_dir: &Path) -> Result<Options, String> {
    if matches!(args, [argument] if argument == "--help" || argument == "-h") {
        return Err(usage().to_owned());
    }
    let mut crate_name = None;
    let mut output_root = None;
    let mut extractor = None;
    let mut extractor_sha256 = None;
    let mut extractor_backend = None;
    let mut extractor_backend_sha256 = None;
    let mut worker = None;
    let mut worker_sha256 = None;
    let mut cargo = None;
    let mut cargo_sha256 = None;
    let mut rustc = None;
    let mut rustc_sha256 = None;
    let mut worker_build_identity = None;
    let mut llvm_build_identity = None;
    let mut target = None;
    let mut code_object_version = None;
    let mut timeout_seconds = None;
    let mut max_output_bytes = None;
    let mut providers = Vec::new();
    let mut cargo_args = Vec::new();
    let mut args = args.iter();
    while let Some(argument) = args.next() {
        if argument == "--" {
            cargo_args.extend(args.cloned());
            break;
        }
        let argument = argument
            .to_str()
            .ok_or_else(|| "engineering options before `--` must be valid UTF-8".to_owned())?;
        if argument == "--help" || argument == "-h" {
            return Err(usage().to_owned());
        }
        let value = args
            .next()
            .ok_or_else(|| format!("{argument} requires a value"))?;
        match argument {
            "--crate" => set_once_string(&mut crate_name, value, argument)?,
            "--output-root" => set_once_path(&mut output_root, value, argument)?,
            "--extractor" => set_once_path(&mut extractor, value, argument)?,
            "--extractor-sha256" => set_once_digest(&mut extractor_sha256, value, argument)?,
            "--extractor-backend" => set_once_path(&mut extractor_backend, value, argument)?,
            "--extractor-backend-sha256" => {
                set_once_digest(&mut extractor_backend_sha256, value, argument)?
            }
            "--worker" => set_once_path(&mut worker, value, argument)?,
            "--worker-sha256" => set_once_digest(&mut worker_sha256, value, argument)?,
            "--cargo" => set_once_path(&mut cargo, value, argument)?,
            "--cargo-sha256" => set_once_digest(&mut cargo_sha256, value, argument)?,
            "--rustc" => set_once_path(&mut rustc, value, argument)?,
            "--rustc-sha256" => set_once_digest(&mut rustc_sha256, value, argument)?,
            "--worker-build-id" => set_once_string(&mut worker_build_identity, value, argument)?,
            "--llvm-build-id" => set_once_string(&mut llvm_build_identity, value, argument)?,
            "--target" => set_once_string(&mut target, value, argument)?,
            "--code-object-version" => set_once_string(&mut code_object_version, value, argument)?,
            "--timeout-seconds" => set_once_u64(&mut timeout_seconds, value, argument)?,
            "--max-output-bytes" => set_once_u64(&mut max_output_bytes, value, argument)?,
            "--provider" => providers.push(parse_provider(value, current_dir)?),
            _ => {
                return Err(format!(
                    "unknown engineering hsaco option {argument:?}\n{}",
                    usage()
                ));
            }
        }
    }

    let crate_name = crate_name.ok_or_else(|| "missing required --crate".to_owned())?;
    validate_crate_name(&crate_name)?;
    let output_root = absolute_path(
        current_dir,
        output_root.ok_or_else(|| "missing required --output-root".to_owned())?,
    );
    if output_root.file_name() != Some(OsStr::new(NAMESPACE)) {
        return Err(format!(
            "--output-root basename must be exactly {NAMESPACE}"
        ));
    }
    let target = target.ok_or_else(|| "missing required --target".to_owned())?;
    if target != TARGET {
        return Err(format!("--target must be exactly {TARGET}"));
    }
    let code_object_version =
        code_object_version.ok_or_else(|| "missing required --code-object-version".to_owned())?;
    if code_object_version != CODE_OBJECT_VERSION.to_string() {
        return Err("--code-object-version must be exactly 6".to_owned());
    }
    let timeout_seconds = timeout_seconds.unwrap_or(120);
    if timeout_seconds == 0 || timeout_seconds > fe2o3_hsaco_finalize::MAX_WORKER_TIMEOUT.as_secs()
    {
        return Err("--timeout-seconds must be in 1..=600".to_owned());
    }
    let max_output_bytes = max_output_bytes.unwrap_or(MAX_WORKER_OUTPUT_BYTES as u64);
    if max_output_bytes == 0 || max_output_bytes > MAX_WORKER_OUTPUT_BYTES as u64 {
        return Err(format!(
            "--max-output-bytes must be in 1..={MAX_WORKER_OUTPUT_BYTES}"
        ));
    }
    reject_cargo_override_args(&cargo_args)?;
    validate_build_identity(
        worker_build_identity
            .as_deref()
            .ok_or_else(|| "missing required --worker-build-id".to_owned())?,
        "--worker-build-id",
    )?;
    validate_build_identity(
        llvm_build_identity
            .as_deref()
            .ok_or_else(|| "missing required --llvm-build-id".to_owned())?,
        "--llvm-build-id",
    )?;

    Ok(Options {
        crate_name,
        output_root,
        extractor: required_file_claim(
            current_dir,
            extractor,
            extractor_sha256,
            "--extractor",
            "--extractor-sha256",
        )?,
        extractor_backend: required_file_claim(
            current_dir,
            extractor_backend,
            extractor_backend_sha256,
            "--extractor-backend",
            "--extractor-backend-sha256",
        )?,
        worker: required_file_claim(
            current_dir,
            worker,
            worker_sha256,
            "--worker",
            "--worker-sha256",
        )?,
        cargo: required_file_claim(
            current_dir,
            cargo,
            cargo_sha256,
            "--cargo",
            "--cargo-sha256",
        )?,
        rustc: required_file_claim(
            current_dir,
            rustc,
            rustc_sha256,
            "--rustc",
            "--rustc-sha256",
        )?,
        worker_build_identity: worker_build_identity.expect("validated required worker build ID"),
        llvm_build_identity: llvm_build_identity.expect("validated required LLVM build ID"),
        providers,
        timeout: Duration::from_secs(timeout_seconds),
        max_output_bytes,
        cargo_args,
    })
}

fn required_file_claim(
    current_dir: &Path,
    path: Option<PathBuf>,
    sha256: Option<[u8; 32]>,
    path_option: &str,
    digest_option: &str,
) -> Result<FileClaim, String> {
    Ok(FileClaim {
        path: absolute_path(
            current_dir,
            path.ok_or_else(|| format!("missing required {path_option}"))?,
        ),
        sha256: sha256.ok_or_else(|| format!("missing required {digest_option}"))?,
    })
}

fn parse_provider(value: &OsStr, current_dir: &Path) -> Result<ProviderClaim, String> {
    let value = value
        .to_str()
        .ok_or_else(|| "--provider must be valid UTF-8".to_owned())?;
    let mut fields = value.splitn(3, ':');
    let kind = match fields.next() {
        Some("llvm-bitcode") => WorkerInputKindV1::LlvmBitcode,
        Some("llvm-ir") => WorkerInputKindV1::LlvmTextIr,
        Some("amdgpu-relocatable") => WorkerInputKindV1::AmdGpuRelocatable,
        _ => {
            return Err(
                "--provider kind must be llvm-bitcode, llvm-ir, or amdgpu-relocatable".to_owned(),
            );
        }
    };
    let sha256 = parse_sha256(
        fields
            .next()
            .ok_or_else(|| "--provider is missing its SHA-256".to_owned())?,
        "--provider SHA-256",
    )?;
    let path = fields
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "--provider is missing its path".to_owned())?;
    Ok(ProviderClaim {
        kind,
        path: absolute_path(current_dir, PathBuf::from(path)),
        sha256,
    })
}

fn set_once_string(slot: &mut Option<String>, value: &OsStr, option: &str) -> Result<(), String> {
    let value = value
        .to_str()
        .ok_or_else(|| format!("{option} must be valid UTF-8"))?
        .to_owned();
    if slot.replace(value).is_some() {
        return Err(format!("{option} may be specified only once"));
    }
    Ok(())
}

fn set_once_path(slot: &mut Option<PathBuf>, value: &OsStr, option: &str) -> Result<(), String> {
    if slot.replace(PathBuf::from(value)).is_some() {
        return Err(format!("{option} may be specified only once"));
    }
    Ok(())
}

fn set_once_digest(slot: &mut Option<[u8; 32]>, value: &OsStr, option: &str) -> Result<(), String> {
    let value = value
        .to_str()
        .ok_or_else(|| format!("{option} must be valid UTF-8"))?;
    if slot.replace(parse_sha256(value, option)?).is_some() {
        return Err(format!("{option} may be specified only once"));
    }
    Ok(())
}

fn set_once_u64(slot: &mut Option<u64>, value: &OsStr, option: &str) -> Result<(), String> {
    let value = value
        .to_str()
        .ok_or_else(|| format!("{option} must be valid UTF-8"))?
        .parse::<u64>()
        .map_err(|_| format!("{option} must be an unsigned decimal integer"))?;
    if slot.replace(value).is_some() {
        return Err(format!("{option} may be specified only once"));
    }
    Ok(())
}

fn parse_sha256(value: &str, label: &str) -> Result<[u8; 32], String> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("{label} must be exactly 64 hexadecimal characters"));
    }
    let mut digest = [0_u8; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| format!("{label} is malformed"))?;
    }
    Ok(digest)
}

fn validate_crate_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 256
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(
            "--crate must be a nonempty rustc crate name containing only ASCII letters, digits, or `_`"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_build_identity(value: &str, option: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > fe2o3_hsaco_finalize::MAX_WORKER_TOOLCHAIN_ID_BYTES
        || !value.is_ascii()
        || value.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(format!(
            "{option} is empty, oversized, or contains control bytes"
        ));
    }
    Ok(())
}

fn reject_cargo_override_args(args: &[OsString]) -> Result<(), String> {
    for argument in args {
        let Some(argument) = argument.to_str() else {
            continue;
        };
        if argument == "--target"
            || argument.starts_with("--target=")
            || argument == "--target-dir"
            || argument.starts_with("--target-dir=")
            || argument == "--config"
            || argument.starts_with("--config=")
            || argument == "--release"
            || argument.starts_with("--release=")
            || argument == "-r"
            || argument == "--profile"
            || argument.starts_with("--profile=")
        {
            return Err(format!(
                "Cargo argument {argument:?} conflicts with the fixed engineering extraction profile"
            ));
        }
    }
    Ok(())
}

fn reject_conflicting_environment() -> Result<(), String> {
    for (name, value) in env::vars_os() {
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.starts_with("FE2O3_EXTRACT_")
            || matches!(
                name,
                "RUSTC_WRAPPER"
                    | "CARGO_BUILD_RUSTC_WRAPPER"
                    | "RUSTC_WORKSPACE_WRAPPER"
                    | "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER"
                    | "RUSTFLAGS"
                    | "CARGO_ENCODED_RUSTFLAGS"
            )
        {
            return Err(format!(
                "caller environment {name}={value:?} conflicts with explicit engineering extraction"
            ));
        }
    }
    Ok(())
}

fn run_extraction(
    options: &Options,
    extractor: &Path,
    handoff: &Path,
    scratch: &Path,
) -> Result<(), String> {
    let rustc_lib = rustc_sysroot_lib_dir(&options.rustc.path)?;
    let loader_path = env::join_paths([
        extractor
            .parent()
            .ok_or_else(|| "pinned extractor has no parent directory".to_owned())?,
        rustc_lib.as_path(),
    ])
    .map_err(|error| format!("cannot construct extraction loader path: {error}"))?;
    let mut command = Command::new(&options.cargo.path);
    command
        .arg("check")
        .arg("--locked")
        .arg("-Zbuild-std=core")
        .arg("--target")
        .arg(CARGO_TARGET)
        .arg("--target-dir")
        .arg(scratch.join("cargo-target"))
        .args(&options.cargo_args)
        .env("RUSTC", &options.rustc.path)
        .env("RUSTC_WRAPPER", "")
        .env("CARGO_BUILD_RUSTC_WRAPPER", "")
        .env("RUSTC_WORKSPACE_WRAPPER", extractor)
        .env("CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER", extractor)
        .env("LD_LIBRARY_PATH", loader_path)
        .env("FE2O3_HIP_SYS_DISABLE", "1")
        .env("FE2O3_HSA_RUNTIME_DISABLE", "1")
        .env("FE2O3_EXTRACT_CRATE_V1", &options.crate_name)
        .env("FE2O3_EXTRACT_GFX942_COMPILER_HANDOFF_PATH_V1", handoff)
        .env(
            "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            "-Zalways-encode-mir -Zinline-mir=no -Zmir-enable-passes=-JumpThreading -Copt-level=0 -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32",
        )
        .stdin(Stdio::null());
    for (name, _) in env::vars_os() {
        if name.to_str().is_some_and(|name| name.starts_with("FE2O3_")) {
            command.env_remove(name);
        }
    }
    command
        .env("FE2O3_HIP_SYS_DISABLE", "1")
        .env("FE2O3_HSA_RUNTIME_DISABLE", "1")
        .env("FE2O3_EXTRACT_CRATE_V1", &options.crate_name)
        .env("FE2O3_EXTRACT_GFX942_COMPILER_HANDOFF_PATH_V1", handoff);
    let status = command
        .status()
        .map_err(|error| format!("failed to execute Cargo engineering extraction: {error}"))?;
    if !status.success() {
        return Err(format!("Cargo engineering extraction failed with {status}"));
    }
    if !handoff.is_file() {
        return Err("Cargo succeeded without producing the compiler handoff".to_owned());
    }
    Ok(())
}

fn rustc_sysroot_lib_dir(rustc: &Path) -> Result<PathBuf, String> {
    let mut child = Command::new(rustc)
        .args(["--print", "sysroot"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("cannot query rustc sysroot: {error}"))?;
    let mut bytes = Vec::new();
    child
        .stdout
        .take()
        .expect("piped rustc stdout")
        .take(MAX_RUSTC_SYSROOT_OUTPUT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read rustc sysroot: {error}"))?;
    let status = child
        .wait()
        .map_err(|error| format!("cannot wait for rustc sysroot query: {error}"))?;
    if !status.success() || bytes.is_empty() || bytes.len() as u64 > MAX_RUSTC_SYSROOT_OUTPUT_BYTES
    {
        return Err("rustc sysroot query failed or returned an invalid path".to_owned());
    }
    let sysroot = std::str::from_utf8(&bytes)
        .map_err(|_| "rustc sysroot must be valid UTF-8".to_owned())?
        .trim_end_matches(['\r', '\n']);
    if sysroot.is_empty() || sysroot.contains('\r') || sysroot.contains('\n') {
        return Err("rustc sysroot query returned a malformed path".to_owned());
    }
    let lib = Path::new(sysroot).join("lib");
    if !lib.is_dir() {
        return Err(format!(
            "rustc sysroot library directory `{}` is absent",
            lib.display()
        ));
    }
    Ok(lib)
}

fn read_claimed_provider(claim: &ProviderClaim) -> Result<Vec<u8>, String> {
    read_claimed_file(
        "provider",
        &FileClaim {
            path: claim.path.clone(),
            sha256: claim.sha256,
        },
        fe2o3_hsaco_finalize::MAX_WORKER_TOTAL_INPUT_BYTES as u64,
        false,
    )
}

fn read_claimed_file(
    label: &str,
    claim: &FileClaim,
    max_bytes: u64,
    executable: bool,
) -> Result<Vec<u8>, String> {
    require_canonical_absolute_path(&claim.path, label)?;
    let bytes = read_bounded_regular_file(&claim.path, max_bytes, executable)?;
    let actual: [u8; 32] = Sha256::digest(&bytes).into();
    if actual != claim.sha256 {
        return Err(format!(
            "{label} SHA-256 does not match the declared identity"
        ));
    }
    Ok(bytes)
}

fn read_bounded_regular_file(
    path: &Path,
    max_bytes: u64,
    executable: bool,
) -> Result<Vec<u8>, String> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    let mut file = options.open(path).map_err(|error| {
        format!(
            "cannot open `{}` without following a symlink: {error}",
            path.display()
        )
    })?;
    let before = file
        .metadata()
        .map_err(|error| format!("cannot inspect `{}`: {error}", path.display()))?;
    if !before.is_file() || before.len() == 0 || before.len() > max_bytes {
        return Err(format!(
            "`{}` is empty, oversized, or not a regular file",
            path.display()
        ));
    }
    if executable && before.permissions().mode() & 0o111 == 0 {
        return Err(format!("`{}` is not executable", path.display()));
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read `{}`: {error}", path.display()))?;
    if bytes.len() as u64 != before.len() || bytes.len() as u64 > max_bytes {
        return Err(format!(
            "`{}` changed length or exceeded its bound",
            path.display()
        ));
    }
    let after = file
        .metadata()
        .map_err(|error| format!("cannot re-inspect `{}`: {error}", path.display()))?;
    if metadata_identity(&before) != metadata_identity(&after) {
        return Err(format!(
            "`{}` changed while it was captured",
            path.display()
        ));
    }
    Ok(bytes)
}

fn metadata_identity(metadata: &fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    )
}

fn require_canonical_absolute_path(path: &Path, label: &str) -> Result<(), String> {
    if !path.is_absolute() || path.components().any(|part| part == Component::ParentDir) {
        return Err(format!("{label} path must be absolute and contain no `..`"));
    }
    let canonical = fs::canonicalize(path).map_err(|error| {
        format!(
            "cannot canonicalize {label} path `{}`: {error}",
            path.display()
        )
    })?;
    if canonical != path {
        return Err(format!(
            "{label} path must already be canonical and contain no symlinks"
        ));
    }
    Ok(())
}

fn validate_fresh_output_root(root: &Path) -> Result<(), String> {
    if !root.is_absolute() || root.components().any(|part| part == Component::ParentDir) {
        return Err("--output-root must be absolute and contain no `..`".to_owned());
    }
    if root.exists() || fs::symlink_metadata(root).is_ok() {
        return Err(format!(
            "engineering output root `{}` already exists",
            root.display()
        ));
    }
    let parent = root
        .parent()
        .ok_or_else(|| "engineering output root has no parent".to_owned())?;
    let canonical_parent = fs::canonicalize(parent)
        .map_err(|error| format!("cannot canonicalize engineering output parent: {error}"))?;
    if canonical_parent != parent || !parent.is_dir() {
        return Err("engineering output parent must be an existing canonical directory".to_owned());
    }
    Ok(())
}

#[derive(Serialize)]
struct Manifest<'a> {
    schema: &'static str,
    namespace: &'static str,
    authority: &'static str,
    artifact: &'static str,
    crate_name: &'a str,
    target: &'static str,
    code_object_version: u8,
    compiler_handoff: Identity,
    tools: Tools<'a>,
    providers: Vec<Provider>,
    options: FixedOptions,
    execution: Execution,
    hsaco: Hsaco<'a>,
    grants: Grants,
}

#[derive(Serialize)]
struct Identity {
    sha256: String,
    byte_len: u64,
}

#[derive(Serialize)]
struct Tools<'a> {
    cargo: Identity,
    rustc: Identity,
    extractor: Identity,
    extractor_backend: Identity,
    worker: Worker<'a>,
}

#[derive(Serialize)]
struct Worker<'a> {
    executable: Identity,
    worker_build_identity: &'a str,
    llvm_build_identity: &'a str,
}

#[derive(Serialize)]
struct Provider {
    kind: &'static str,
    identity: Identity,
}

#[derive(Serialize)]
struct FixedOptions {
    optimization: &'static str,
    strip_debug: bool,
    verify_each: bool,
    timeout_seconds: u64,
    maximum_output_bytes: u64,
}

#[derive(Serialize)]
struct Execution {
    bootstrap_request: Identity,
    bootstrap_response: Identity,
    replay_request: Identity,
    replay_response: Identity,
    exact_output_replay: bool,
}

#[derive(Serialize)]
struct Hsaco<'a> {
    identity: Identity,
    canonical_descriptor_sha256: String,
    kernel_names: &'a [String],
}

#[derive(Serialize)]
struct Grants {
    publication: bool,
    load: bool,
    launch: bool,
}

fn canonical_manifest(
    options: &Options,
    observation: &EngineeringHsacoObservationV1,
    cargo: &[u8],
    rustc: &[u8],
    extractor: &[u8],
    extractor_backend: &[u8],
) -> Result<Vec<u8>, String> {
    let providers = observation
        .providers()
        .iter()
        .map(|provider| Provider {
            kind: provider_kind(provider.kind()),
            identity: identity(provider.identity()),
        })
        .collect();
    let worker = observation.worker_measurement();
    let manifest = Manifest {
        schema: MANIFEST_SCHEMA,
        namespace: NAMESPACE,
        authority: observation.authority(),
        artifact: "observation.hsaco",
        crate_name: &options.crate_name,
        target: TARGET,
        code_object_version: CODE_OBJECT_VERSION,
        compiler_handoff: identity(observation.handoff_identity()),
        tools: Tools {
            cargo: identity(ContentIdentityV1::calculate(cargo)),
            rustc: identity(ContentIdentityV1::calculate(rustc)),
            extractor: identity(ContentIdentityV1::calculate(extractor)),
            extractor_backend: identity(ContentIdentityV1::calculate(extractor_backend)),
            worker: Worker {
                executable: identity(worker.executable()),
                worker_build_identity: worker.worker_build_identity(),
                llvm_build_identity: worker.llvm_build_identity(),
            },
        },
        providers,
        options: FixedOptions {
            optimization: "O2",
            strip_debug: true,
            verify_each: true,
            timeout_seconds: options.timeout.as_secs(),
            maximum_output_bytes: options.max_output_bytes,
        },
        execution: Execution {
            bootstrap_request: identity(observation.bootstrap_request_identity()),
            bootstrap_response: identity(observation.bootstrap_response_identity()),
            replay_request: identity(observation.replay_request_identity()),
            replay_response: identity(observation.replay_response_identity()),
            exact_output_replay: true,
        },
        hsaco: Hsaco {
            identity: identity(observation.finalized_hsaco_identity()),
            canonical_descriptor_sha256: hex(observation.canonical_descriptor_digest()),
            kernel_names: observation.kernel_names(),
        },
        grants: Grants {
            publication: observation.grants_publication_authority(),
            load: observation.grants_load_authority(),
            launch: observation.grants_launch_authority(),
        },
    };
    let mut bytes = serde_json::to_vec(&manifest)
        .map_err(|error| format!("cannot encode engineering observation: {error}"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn identity(identity: ContentIdentityV1) -> Identity {
    Identity {
        sha256: hex(identity.sha256()),
        byte_len: identity.byte_len(),
    }
}

const fn provider_kind(kind: WorkerInputKindV1) -> &'static str {
    match kind {
        WorkerInputKindV1::LlvmBitcode => "llvm-bitcode",
        WorkerInputKindV1::AmdGpuRelocatable => "amdgpu-relocatable",
        WorkerInputKindV1::LlvmTextIr => "llvm-ir",
    }
}

fn publish_observation(root: &Path, manifest: &[u8], hsaco: &[u8]) -> Result<PathBuf, String> {
    validate_fresh_output_root(root)?;
    let content_id = observation_content_id(manifest, hsaco);
    let content_dir = root.join(&content_id);
    let result = (|| {
        fs::DirBuilder::new()
            .mode(0o700)
            .create(root)
            .map_err(|error| format!("cannot create engineering namespace: {error}"))?;
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&content_dir)
            .map_err(|error| format!("cannot create engineering content directory: {error}"))?;
        write_new_file(&content_dir.join("observation.hsaco"), hsaco, 0o600)?;
        write_new_file(&content_dir.join("observation.json"), manifest, 0o600)?;
        File::open(&content_dir)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| format!("cannot sync engineering content directory: {error}"))?;
        File::open(root)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| format!("cannot sync engineering namespace: {error}"))?;
        Ok(content_dir.clone())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(root);
    }
    result
}

fn observation_content_id(manifest: &[u8], hsaco: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(CONTENT_ID_DOMAIN);
    hasher.update((manifest.len() as u64).to_le_bytes());
    hasher.update(manifest);
    hasher.update((hsaco.len() as u64).to_le_bytes());
    hasher.update(hsaco);
    hex(&hasher.finalize())
}

fn write_new_file(path: &Path, bytes: &[u8], mode: u32) -> Result<(), String> {
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create_new(true)
        .mode(mode)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    let mut file = options
        .open(path)
        .map_err(|error| format!("cannot create fresh `{}`: {error}", path.display()))?;
    let result = file.write_all(bytes).and_then(|()| file.sync_all());
    if let Err(error) = result {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(format!("cannot publish `{}`: {error}", path.display()));
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

fn absolute_path(current_dir: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        current_dir.join(path)
    }
}

struct ScratchDirectory {
    path: PathBuf,
}

impl ScratchDirectory {
    fn new() -> Result<Self, String> {
        let base = env::temp_dir();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "system clock is before the Unix epoch".to_owned())?
            .as_nanos();
        for attempt in 0_u32..32 {
            let path = base.join(format!(
                "fe2o3-engineering-hsaco-{}-{nonce}-{attempt}",
                std::process::id()
            ));
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(format!(
                        "cannot create private engineering scratch: {error}"
                    ));
                }
            }
        }
        Err("cannot allocate a fresh engineering scratch directory".to_owned())
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

const fn usage() -> &'static str {
    "usage: cargo fe2o3 engineering hsaco --crate <rustc-crate-name> --output-root </fresh/fe2o3-engineering-v1> --target gfx942:xnack- --code-object-version 6 --extractor <absolute-path> --extractor-sha256 <hex> --extractor-backend <absolute-path> --extractor-backend-sha256 <hex> --worker <absolute-path> --worker-sha256 <hex> --worker-build-id <id> --llvm-build-id <id> --cargo <absolute-path> --cargo-sha256 <hex> --rustc <absolute-path> --rustc-sha256 <hex> [--provider <llvm-bitcode|llvm-ir|amdgpu-relocatable>:<sha256>:<absolute-path>] [--timeout-seconds <1..600>] [--max-output-bytes <bytes>] -- [Cargo package/feature args]"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_args(root: &Path) -> Vec<OsString> {
        let digest = "11".repeat(32);
        [
            "--crate".into(),
            "aggregate_device".into(),
            "--output-root".into(),
            root.join(NAMESPACE).into_os_string(),
            "--target".into(),
            TARGET.into(),
            "--code-object-version".into(),
            "6".into(),
            "--extractor".into(),
            "/tools/extractor".into(),
            "--extractor-sha256".into(),
            digest.clone().into(),
            "--extractor-backend".into(),
            "/tools/librustc_codegen_fe2o3.so".into(),
            "--extractor-backend-sha256".into(),
            digest.clone().into(),
            "--worker".into(),
            "/tools/worker".into(),
            "--worker-sha256".into(),
            digest.clone().into(),
            "--worker-build-id".into(),
            "worker-build".into(),
            "--llvm-build-id".into(),
            "llvm-build".into(),
            "--cargo".into(),
            "/tools/cargo".into(),
            "--cargo-sha256".into(),
            digest.clone().into(),
            "--rustc".into(),
            "/tools/rustc".into(),
            "--rustc-sha256".into(),
            digest.into(),
            "--".into(),
            "--manifest-path".into(),
            "/source/Cargo.toml".into(),
            "--lib".into(),
        ]
        .to_vec()
    }

    #[test]
    fn parses_only_the_fixed_target_cov_and_namespace() {
        let root = env::temp_dir();
        let options = parse(&base_args(&root), &root).unwrap();
        assert_eq!(options.crate_name, "aggregate_device");
        assert_eq!(options.output_root, root.join(NAMESPACE));
        assert_eq!(options.max_output_bytes, MAX_WORKER_OUTPUT_BYTES as u64);

        let mut wrong_target = base_args(&root);
        let index = wrong_target
            .iter()
            .position(|value| value == "--target")
            .unwrap()
            + 1;
        wrong_target[index] = "gfx950:xnack-".into();
        assert!(
            parse(&wrong_target, &root)
                .unwrap_err()
                .contains("gfx942:xnack-")
        );

        let mut wrong_cov = base_args(&root);
        let index = wrong_cov
            .iter()
            .position(|value| value == "--code-object-version")
            .unwrap()
            + 1;
        wrong_cov[index] = "5".into();
        assert!(parse(&wrong_cov, &root).unwrap_err().contains("exactly 6"));

        let mut wrong_namespace = base_args(&root);
        let index = wrong_namespace
            .iter()
            .position(|value| value == "--output-root")
            .unwrap()
            + 1;
        wrong_namespace[index] = root.join("fe2o3").into_os_string();
        assert!(
            parse(&wrong_namespace, &root)
                .unwrap_err()
                .contains(NAMESPACE)
        );
    }

    #[test]
    fn rejects_hostile_identities_providers_limits_and_cargo_overrides() {
        let root = env::temp_dir();
        let mut bad_digest = base_args(&root);
        let index = bad_digest
            .iter()
            .position(|value| value == "--worker-sha256")
            .unwrap()
            + 1;
        bad_digest[index] = "00".into();
        assert!(parse(&bad_digest, &root).is_err());

        let mut bad_provider = base_args(&root);
        bad_provider.splice(
            bad_provider.len() - 4..bad_provider.len() - 4,
            ["--provider".into(), "object:not-a-digest:/tmp/p".into()],
        );
        assert!(parse(&bad_provider, &root).is_err());

        let mut bad_timeout = base_args(&root);
        bad_timeout.splice(
            bad_timeout.len() - 4..bad_timeout.len() - 4,
            ["--timeout-seconds".into(), "601".into()],
        );
        assert!(parse(&bad_timeout, &root).is_err());

        let mut bad_output = base_args(&root);
        bad_output.splice(
            bad_output.len() - 4..bad_output.len() - 4,
            ["--max-output-bytes".into(), "0".into()],
        );
        assert!(parse(&bad_output, &root).is_err());

        let mut override_target = base_args(&root);
        override_target.push("--target=gfx950".into());
        assert!(parse(&override_target, &root).is_err());
    }

    #[test]
    fn output_namespace_is_fresh_and_never_uses_production_names() {
        let parent = env::temp_dir().join(format!(
            "fe2o3-engineering-output-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&parent);
        fs::create_dir(&parent).unwrap();
        let root = parent.join(NAMESPACE);
        let hsaco = b"inert-test-hsaco";
        let manifest = b"{\"authority\":\"none\"}\n";
        let published = publish_observation(&root, manifest, hsaco).unwrap();
        assert_eq!(
            fs::read(published.join("observation.hsaco")).unwrap(),
            hsaco
        );
        assert_eq!(
            fs::read(published.join("observation.json")).unwrap(),
            manifest
        );
        assert!(!root.join("CURRENT").exists());
        assert!(!root.join(".fe2o3-owned-v1").exists());
        assert!(publish_observation(&root, manifest, hsaco).is_err());
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn source_has_no_production_or_supervisor_adoption_path() {
        let source = include_str!("engineering_hsaco.rs");
        for forbidden in [
            concat!("compiler_execution", "_client"),
            concat!("authority_", "release"),
            concat!("PublishedProtected", "WorkerV3HsacoV1"),
            concat!("/run/fe2o3/", "compiler-execution-supervisor.sock"),
        ] {
            assert!(
                !source.contains(forbidden),
                "forbidden production surface: {forbidden}"
            );
        }
        assert!(source.contains("authority: observation.authority()"));
        assert!(source.contains("publication: observation.grants_publication_authority()"));
    }

    #[test]
    fn content_namespace_binds_manifest_and_hsaco() {
        let first = observation_content_id(b"manifest", b"hsaco");
        assert_eq!(first, observation_content_id(b"manifest", b"hsaco"));
        assert_ne!(first, observation_content_id(b"manifest-2", b"hsaco"));
        assert_ne!(first, observation_content_id(b"manifest", b"hsaco-2"));
    }
}
