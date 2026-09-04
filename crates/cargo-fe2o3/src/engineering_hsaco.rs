//! Explicitly non-authoritative compiler-to-HSACO engineering command.

use std::env;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::process::{ExitCode, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fe2o3_hsaco_finalize::{
    ContentIdentityV1, EngineeringHsacoObservationV1, MAX_WORKER_EXECUTABLE_BYTES,
    MAX_WORKER_OUTPUT_BYTES, MAX_WORKER_RESPONSE_BYTES, PinnedWorkerV1, WorkerExecutionLimitsV1,
    WorkerInputKindV1, WorkerInputV1, WorkerMeasurementV1, WorkerOutputConstraintsV1,
    observe_engineering_hsaco_v1,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

mod execution;
mod support;

use execution::*;
use support::*;

const NAMESPACE: &str = "fe2o3-engineering-v1";
const MANIFEST_SCHEMA: &str = "EngineeringHsacoObservationV1";
const PROFILE: fe2o3_amd_target::ProductionAmdTargetProfileV1 =
    fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942;
const TARGET: &str = PROFILE.device_target();
const CARGO_TARGET: &str = PROFILE.rustc_target();
const CODE_OBJECT_VERSION: u8 = 6;
const MAX_HANDOFF_BYTES: u64 = 64 * 1024 * 1024;
const MAX_TOOL_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_MANIFEST_BYTES: usize = 1024 * 1024;
const MAX_CARGO_GIT_SOURCES: usize = 64;
const EXTRACTOR_CHILD_FD: std::os::fd::RawFd = 205;
const VENDOR_CHILD_FD: std::os::fd::RawFd = 206;
const HOST_LINKER_CHILD_FD: std::os::fd::RawFd = 207;
const HOST_LLD_CHILD_FD: std::os::fd::RawFd = 208;
const HOST_LLD_PROXY_CHILD_FD: std::os::fd::RawFd = 209;
const _: () = assert!(EXTRACTOR_CHILD_FD != VENDOR_CHILD_FD);
const _: () = assert!(EXTRACTOR_CHILD_FD != crate::RUSTC_LIBRARY_CHILD_FD);
const _: () = assert!(EXTRACTOR_CHILD_FD != crate::RUSTC_CHILD_FD);
const _: () = assert!(VENDOR_CHILD_FD != crate::RUSTC_LIBRARY_CHILD_FD);
const _: () = assert!(VENDOR_CHILD_FD != crate::RUSTC_CHILD_FD);
const _: () = assert!(HOST_LINKER_CHILD_FD != HOST_LLD_CHILD_FD);
const _: () = assert!(HOST_LINKER_CHILD_FD != EXTRACTOR_CHILD_FD);
const _: () = assert!(HOST_LINKER_CHILD_FD != VENDOR_CHILD_FD);
const _: () = assert!(HOST_LINKER_CHILD_FD != crate::RUSTC_LIBRARY_CHILD_FD);
const _: () = assert!(HOST_LINKER_CHILD_FD != crate::RUSTC_CHILD_FD);
const _: () = assert!(HOST_LLD_CHILD_FD != EXTRACTOR_CHILD_FD);
const _: () = assert!(HOST_LLD_CHILD_FD != VENDOR_CHILD_FD);
const _: () = assert!(HOST_LLD_CHILD_FD != crate::RUSTC_LIBRARY_CHILD_FD);
const _: () = assert!(HOST_LLD_CHILD_FD != crate::RUSTC_CHILD_FD);
const _: () = assert!(HOST_LLD_PROXY_CHILD_FD != HOST_LINKER_CHILD_FD);
const _: () = assert!(HOST_LLD_PROXY_CHILD_FD != HOST_LLD_CHILD_FD);
const _: () = assert!(HOST_LLD_PROXY_CHILD_FD != EXTRACTOR_CHILD_FD);
const _: () = assert!(HOST_LLD_PROXY_CHILD_FD != VENDOR_CHILD_FD);
const _: () = assert!(HOST_LLD_PROXY_CHILD_FD != crate::RUSTC_LIBRARY_CHILD_FD);
const _: () = assert!(HOST_LLD_PROXY_CHILD_FD != crate::RUSTC_CHILD_FD);
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
    host_linker: FileClaim,
    host_lld: FileClaim,
    host_lld_proxy: FileClaim,
    cargo_vendor: Option<PathBuf>,
    cargo_git_sources: Vec<CargoGitSource>,
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

#[derive(Debug, Serialize)]
struct CargoGitSource {
    url: String,
    rev: String,
}

fn run(args: &[OsString]) -> Result<PathBuf, String> {
    reject_conflicting_environment()?;
    let current_dir = env::current_dir()
        .map_err(|error| format!("cannot identify invocation directory: {error}"))?;
    let options = parse(args, &current_dir)?;
    validate_fresh_output_root(&options.output_root)?;

    let scratch = ScratchDirectory::new()?;
    let pinned_cargo = pin_claimed_executable("Cargo", &options.cargo)?;
    let pinned_rustc_source = pin_claimed_executable("rustc", &options.rustc)?;
    let rustc_lib_tree = crate::rustc_lib_tree::PinnedRustcLibTree::pin(
        crate::rustc_lib_tree_directory(&options.rustc.path)?,
    )?;
    let rustc_lib_tree_sha256 = *rustc_lib_tree.sha256();
    let pinned_rustc = crate::PinnedRustc {
        executable: pinned_rustc_source,
        lib_tree: crate::RustcLibTree::Authority(rustc_lib_tree),
    };
    let pinned_host_linker = pin_claimed_executable("host linker", &options.host_linker)?;
    let pinned_host_lld = pin_claimed_executable("host lld", &options.host_lld)?;
    let pinned_host_lld_proxy = pin_claimed_executable("host lld proxy", &options.host_lld_proxy)?;
    let cargo_vendor = options
        .cargo_vendor
        .as_ref()
        .map(|path| pin_vendor_tree(path))
        .transpose()?;
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

    let tool_directory = scratch.path.join("tool-images");
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&tool_directory)
        .map_err(|error| format!("cannot create private tool-image directory: {error}"))?;
    let pinned_extractor_path = tool_directory.join("fe2o3-rustc-extract");
    write_new_file(&pinned_extractor_path, &extractor_bytes, 0o500)?;
    let pinned_extractor_backend = tool_directory.join("librustc_codegen_fe2o3.so");
    write_new_file(&pinned_extractor_backend, &extractor_backend_bytes, 0o400)?;
    let pinned_extractor = crate::pinned_executable::PinnedExecutable::open(&pinned_extractor_path)
        .map_err(|error| format!("cannot pin captured extractor: {error}"))?
        .seal_executable_image()
        .map_err(|error| format!("cannot seal captured extractor: {error}"))?;
    if pinned_extractor.sha256() != &options.extractor.sha256 {
        return Err("sealed extractor differs from its declared identity".to_owned());
    }
    let captured_tool_tree = crate::rustc_lib_tree::PinnedRustcLibTree::pin(
        crate::project::PinnedDirectory::open_existing(
            tool_directory,
            "engineering captured tool-image directory",
        )?,
    )?;
    let handoff_path = scratch.path.join("compiler-handoff-v2");
    run_extraction(
        &options,
        &pinned_cargo,
        &pinned_rustc,
        &pinned_host_linker,
        &pinned_host_lld,
        &pinned_host_lld_proxy,
        cargo_vendor.as_ref(),
        &pinned_extractor,
        &handoff_path,
        &scratch.path,
    )?;
    captured_tool_tree.revalidate()?;
    if let Some(vendor) = cargo_vendor.as_ref() {
        vendor.revalidate()?;
    }
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
        ContentIdentityV1::from_parts(*pinned_cargo.sha256(), pinned_cargo.size()),
        ContentIdentityV1::from_parts(
            *pinned_rustc.executable.sha256(),
            pinned_rustc.executable.size(),
        ),
        ContentIdentityV1::from_parts(*pinned_host_linker.sha256(), pinned_host_linker.size()),
        ContentIdentityV1::from_parts(*pinned_host_lld.sha256(), pinned_host_lld.size()),
        ContentIdentityV1::from_parts(
            *pinned_host_lld_proxy.sha256(),
            pinned_host_lld_proxy.size(),
        ),
        rustc_lib_tree_sha256,
        cargo_vendor.as_ref().map(|vendor| *vendor.sha256()),
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
    let mut host_linker = None;
    let mut host_linker_sha256 = None;
    let mut host_lld = None;
    let mut host_lld_sha256 = None;
    let mut host_lld_proxy = None;
    let mut host_lld_proxy_sha256 = None;
    let mut cargo_vendor = None;
    let mut cargo_git_sources = Vec::new();
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
            "--host-linker" => set_once_path(&mut host_linker, value, argument)?,
            "--host-linker-sha256" => set_once_digest(&mut host_linker_sha256, value, argument)?,
            "--host-lld" => set_once_path(&mut host_lld, value, argument)?,
            "--host-lld-sha256" => set_once_digest(&mut host_lld_sha256, value, argument)?,
            "--host-lld-proxy" => set_once_path(&mut host_lld_proxy, value, argument)?,
            "--host-lld-proxy-sha256" => {
                set_once_digest(&mut host_lld_proxy_sha256, value, argument)?
            }
            "--cargo-vendor" => set_once_path(&mut cargo_vendor, value, argument)?,
            "--cargo-git-source" => cargo_git_sources.push(parse_cargo_git_source(value)?),
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
    if cargo_vendor.is_none() && !cargo_git_sources.is_empty() {
        return Err("--cargo-git-source requires --cargo-vendor".to_owned());
    }
    if cargo_git_sources.len() > MAX_CARGO_GIT_SOURCES {
        return Err(format!(
            "at most {MAX_CARGO_GIT_SOURCES} --cargo-git-source values are allowed"
        ));
    }
    for pair in cargo_git_sources.windows(2) {
        if (&pair[0].url, &pair[0].rev) >= (&pair[1].url, &pair[1].rev) {
            return Err(
                "--cargo-git-source values must be unique and strictly sorted by URL then revision"
                    .to_owned(),
            );
        }
    }
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
        host_linker: required_file_claim(
            current_dir,
            host_linker,
            host_linker_sha256,
            "--host-linker",
            "--host-linker-sha256",
        )?,
        host_lld: required_file_claim(
            current_dir,
            host_lld,
            host_lld_sha256,
            "--host-lld",
            "--host-lld-sha256",
        )?,
        host_lld_proxy: required_file_claim(
            current_dir,
            host_lld_proxy,
            host_lld_proxy_sha256,
            "--host-lld-proxy",
            "--host-lld-proxy-sha256",
        )?,
        cargo_vendor: cargo_vendor.map(|path| absolute_path(current_dir, path)),
        cargo_git_sources,
        worker_build_identity: worker_build_identity.expect("validated required worker build ID"),
        llvm_build_identity: llvm_build_identity.expect("validated required LLVM build ID"),
        providers,
        timeout: Duration::from_secs(timeout_seconds),
        max_output_bytes,
        cargo_args,
    })
}

fn parse_cargo_git_source(value: &OsStr) -> Result<CargoGitSource, String> {
    let value = value
        .to_str()
        .ok_or_else(|| "--cargo-git-source must be valid UTF-8".to_owned())?;
    let (url, rev) = value
        .rsplit_once('@')
        .ok_or_else(|| "--cargo-git-source must have the form https://URL@40-hex-rev".to_owned())?;
    if !url.starts_with("https://")
        || url.len() > 1024
        || url.bytes().any(|byte| {
            byte.is_ascii_control() || matches!(byte, b'"' | b'\\' | b'?' | b'#' | b'@')
        })
    {
        return Err("--cargo-git-source URL is not a canonical safe HTTPS URL".to_owned());
    }
    if rev.len() != 40
        || !rev
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(
            "--cargo-git-source revision must be 40 lowercase hexadecimal bytes".to_owned(),
        );
    }
    Ok(CargoGitSource {
        url: url.to_owned(),
        rev: rev.to_owned(),
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
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(format!(
            "{label} must be exactly 64 lowercase hexadecimal characters"
        ));
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
    let mut index = 0;
    let mut manifests = 0;
    while index < args.len() {
        let argument = args[index]
            .to_str()
            .ok_or_else(|| "Cargo selection arguments must be valid UTF-8".to_owned())?;
        let takes_value = matches!(
            argument,
            "--manifest-path" | "--package" | "-p" | "--features" | "-F"
        );
        if takes_value {
            if argument == "--manifest-path" {
                manifests += 1;
            }
            let value = args
                .get(index + 1)
                .and_then(|value| value.to_str())
                .filter(|value| !value.is_empty() && !value.starts_with('-'))
                .ok_or_else(|| format!("Cargo argument {argument:?} requires one plain value"))?;
            if argument == "--manifest-path" {
                validate_manifest_path(Path::new(value))?;
            }
            index += 2;
            continue;
        }
        if let Some(value) = argument.strip_prefix("--manifest-path=") {
            manifests += 1;
            validate_manifest_path(Path::new(value))?;
        } else if argument.starts_with("--package=")
            || argument.starts_with("--features=")
            || matches!(
                argument,
                "--all-features" | "--no-default-features" | "--lib"
            )
        {
            if argument.ends_with('=') {
                return Err(format!("Cargo argument {argument:?} has an empty value"));
            }
        } else {
            return Err(format!(
                "Cargo argument {argument:?} is outside the fixed engineering package/feature selection grammar"
            ));
        }
        index += 1;
    }
    if manifests != 1 {
        return Err("engineering extraction requires exactly one --manifest-path".to_owned());
    }
    Ok(())
}

fn validate_manifest_path(path: &Path) -> Result<(), String> {
    require_canonical_absolute_path(path, "Cargo manifest")?;
    if path.file_name() != Some(OsStr::new("Cargo.toml")) || !path.is_file() {
        return Err("--manifest-path must name an existing canonical Cargo.toml".to_owned());
    }
    Ok(())
}

fn reject_conflicting_environment() -> Result<(), String> {
    for (name, value) in env::vars_os() {
        if conflicting_environment_name(&name) {
            return Err(format!(
                "caller environment {name:?}={value:?} conflicts with explicit engineering extraction"
            ));
        }
    }
    Ok(())
}

fn conflicting_environment_name(name: &OsStr) -> bool {
    if crate::is_dynamic_loader_injection_environment_name(name) {
        return true;
    }
    let Some(name) = name.to_str() else {
        return false;
    };
    name.starts_with("FE2O3_EXTRACT_")
        || name.starts_with("CARGO_TARGET_")
        || name.starts_with("CARGO_PROFILE_")
        || matches!(
            name,
            "RUSTC"
                | "CARGO_BUILD_RUSTC"
                | "RUSTC_WRAPPER"
                | "CARGO_BUILD_RUSTC_WRAPPER"
                | "RUSTC_WORKSPACE_WRAPPER"
                | "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER"
                | "RUSTDOC"
                | "CARGO_BUILD_RUSTDOC"
                | "RUSTFLAGS"
                | "CARGO_ENCODED_RUSTFLAGS"
                | "RUSTDOCFLAGS"
                | "CARGO_ENCODED_RUSTDOCFLAGS"
                | "CARGO_BUILD_TARGET"
                | "CARGO_TARGET_DIR"
                | "CARGO_INCREMENTAL"
                | "RUSTC_BOOTSTRAP"
                | "RUSTUP_TOOLCHAIN"
                | "RUSTUP_HOME"
        )
}

const fn usage() -> &'static str {
    "usage: cargo fe2o3 engineering hsaco --crate <rustc-crate-name> --output-root </fresh/fe2o3-engineering-v1> --target gfx942:xnack- --code-object-version 6 --extractor <absolute-path> --extractor-sha256 <hex> --extractor-backend <absolute-path> --extractor-backend-sha256 <hex> --worker <absolute-path> --worker-sha256 <hex> --worker-build-id <id> --llvm-build-id <id> --cargo <absolute-path> --cargo-sha256 <hex> --rustc <absolute-path> --rustc-sha256 <hex> --host-linker <absolute-clang-path> --host-linker-sha256 <hex> --host-lld <absolute-lld-path> --host-lld-sha256 <hex> --host-lld-proxy <absolute-proxy-path> --host-lld-proxy-sha256 <hex> [--cargo-vendor <absolute-directory> [--cargo-git-source <https://URL@40-hex-rev>]...] [--provider <llvm-bitcode|llvm-ir|amdgpu-relocatable>:<sha256>:<absolute-path>] [--timeout-seconds <1..600>] [--max-output-bytes <bytes>] -- [Cargo package/feature args]"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_args(root: &Path) -> Vec<OsString> {
        let digest = "11".repeat(32);
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
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
            digest.clone().into(),
            "--host-linker".into(),
            "/tools/clang".into(),
            "--host-linker-sha256".into(),
            digest.clone().into(),
            "--host-lld".into(),
            "/tools/lld".into(),
            "--host-lld-sha256".into(),
            digest.clone().into(),
            "--host-lld-proxy".into(),
            "/tools/lld-proxy".into(),
            "--host-lld-proxy-sha256".into(),
            digest.into(),
            "--".into(),
            "--manifest-path".into(),
            manifest.into_os_string(),
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
        assert_eq!(options.host_linker.path, Path::new("/tools/clang"));
        assert_eq!(options.host_lld.path, Path::new("/tools/lld"));
        assert_eq!(options.host_lld_proxy.path, Path::new("/tools/lld-proxy"));

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

        let mut uppercase_digest = base_args(&root);
        let index = uppercase_digest
            .iter()
            .position(|value| value == "--host-linker-sha256")
            .unwrap()
            + 1;
        uppercase_digest[index] = "AA".repeat(32).into();
        assert!(parse(&uppercase_digest, &root).is_err());

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

        for hostile in ["-rFfeature", "@/tmp/response", "--", "-Zconfig-include"] {
            let mut args = base_args(&root);
            args.push(hostile.into());
            assert!(parse(&args, &root).is_err(), "accepted {hostile}");
        }

        let mut source_without_vendor = base_args(&root);
        source_without_vendor.splice(
            source_without_vendor.len() - 3..source_without_vendor.len() - 3,
            [
                "--cargo-git-source".into(),
                format!(
                    "https://github.com/example/dependency.git@{}",
                    "a".repeat(40)
                )
                .into(),
            ],
        );
        assert!(parse(&source_without_vendor, &root).is_err());
    }

    #[test]
    fn rejects_loader_and_cargo_selection_environment() {
        for name in [
            "LD_PRELOAD",
            "LD_AUDIT",
            "LD_LIBRARY_PATH",
            "DYLD_INSERT_LIBRARIES",
            "GLIBC_TUNABLES",
            "RUSTC",
            "RUSTDOC",
            "CARGO_BUILD_TARGET",
            "CARGO_TARGET_AMDGCN_AMD_AMDHSA_LINKER",
            "CARGO_PROFILE_DEV_OPT_LEVEL",
            "RUSTC_BOOTSTRAP",
        ] {
            assert!(
                conflicting_environment_name(OsStr::new(name)),
                "accepted {name}"
            );
        }
        assert!(!conflicting_environment_name(OsStr::new("LANG")));
    }

    #[test]
    fn vendor_configuration_is_closed_and_injection_resistant() {
        let source = parse_cargo_git_source(OsStr::new(&format!(
            "https://github.com/example/dependency.git@{}",
            "a".repeat(40)
        )))
        .unwrap();
        let config = cargo_vendor_config(&[source]);
        assert!(config.contains("replace-with = \"vendored-sources\""));
        assert!(config.contains("directory = \"/proc/self/fd/206\""));
        for hostile in [
            "https://example.com/x?branch=main@0123456789012345678901234567890123456789",
            "https://example.com/x\"@0123456789012345678901234567890123456789",
            "ssh://example.com/x@0123456789012345678901234567890123456789",
        ] {
            assert!(parse_cargo_git_source(OsStr::new(hostile)).is_err());
        }

        let root = env::temp_dir();
        let insert = base_args(&root)
            .iter()
            .position(|argument| argument == "--")
            .unwrap();
        for sources in [
            [
                "https://example.com/z.git@0123456789012345678901234567890123456789",
                "https://example.com/a.git@0123456789012345678901234567890123456789",
            ],
            [
                "https://example.com/a.git@0123456789012345678901234567890123456789",
                "https://example.com/a.git@0123456789012345678901234567890123456789",
            ],
        ] {
            let mut args = base_args(&root);
            args.splice(
                insert..insert,
                [
                    "--cargo-vendor".into(),
                    root.clone().into_os_string(),
                    "--cargo-git-source".into(),
                    sources[0].into(),
                    "--cargo-git-source".into(),
                    sources[1].into(),
                ],
            );
            assert!(parse(&args, &root).is_err());
        }

        let mut too_many = base_args(&root);
        let mut inserted = vec!["--cargo-vendor".into(), root.clone().into_os_string()];
        for index in 0..=MAX_CARGO_GIT_SOURCES {
            inserted.push("--cargo-git-source".into());
            inserted.push(
                format!(
                    "https://example.com/{index:03}.git@0123456789012345678901234567890123456789"
                )
                .into(),
            );
        }
        too_many.splice(insert..insert, inserted);
        assert!(parse(&too_many, &root).is_err());
    }

    #[test]
    fn claimed_host_linker_path_swap_cannot_change_executed_bytes() {
        let root = env::temp_dir().join(format!(
            "fe2o3-engineering-executable-swap-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir(&root).unwrap();
        let executable = root.join("cargo");
        fs::copy("/bin/true", &executable).unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let bytes = fs::read(&executable).unwrap();
        let claim = FileClaim {
            path: fs::canonicalize(&executable).unwrap(),
            sha256: Sha256::digest(&bytes).into(),
        };
        let pinned = pin_claimed_executable("test tool", &claim).unwrap();
        fs::rename(&executable, root.join("original")).unwrap();
        fs::copy("/bin/false", &executable).unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let output = pinned.command().unwrap().output().unwrap();
        assert!(output.status.success());
        assert!(output.stdout.is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn substituted_scratch_path_is_never_removed() {
        let scratch = ScratchDirectory::new().unwrap();
        let selected = scratch.path.clone();
        let parked = selected.with_extension("parked");
        fs::rename(&selected, &parked).unwrap();
        fs::create_dir(&selected).unwrap();
        fs::write(selected.join("foreign"), b"leave-me").unwrap();
        drop(scratch);
        assert_eq!(fs::read(selected.join("foreign")).unwrap(), b"leave-me");
        fs::remove_dir_all(selected).unwrap();
        fs::remove_dir_all(parked).unwrap();
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
    fn publication_failure_retains_partial_output_without_cleanup() {
        let parent_path = env::temp_dir().join(format!(
            "fe2o3-engineering-failure-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&parent_path);
        fs::create_dir(&parent_path).unwrap();
        let root = parent_path.join(NAMESPACE);
        let manifest = b"{\"authority\":\"none\"}\n";
        let hsaco = b"inert-test-hsaco";
        let content_id = observation_content_id(manifest, hsaco);
        let error = publish_observation_inner(&root, manifest, hsaco, true).unwrap_err();
        assert!(error.contains("partial output was retained"));
        let content = root.join(content_id);
        assert_eq!(fs::read(content.join("observation.hsaco")).unwrap(), hsaco);
        assert!(!content.join("observation.json").exists());
        fs::remove_dir_all(parent_path).unwrap();
    }

    #[test]
    fn source_has_no_production_or_supervisor_adoption_path() {
        let source = concat!(
            include_str!("engineering_hsaco.rs"),
            include_str!("engineering_hsaco/execution.rs"),
            include_str!("engineering_hsaco/support.rs"),
        );
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
