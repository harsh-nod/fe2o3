//! Feature-gated observation of one wrapper-owned compiler handoff for integration tests.
//!
//! This protocol exposes no production path and grants no compiler, worker, link, load, or launch
//! authority. The observer must consume the wrapper's existing one-shot attempt-scoped handoff;
//! this module never creates an attempt or copies its payload.

use std::fs::{self, File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use fe2o3_artifact_transaction::{
    BuildAttempt, CompilerModuleHandoffErrorV1, ProducerIdentity,
    consume_compiler_module_handoff_v1,
};
use rustix::fs::{RenameFlags, renameat_with};
use sha2::{Digest as _, Sha256};

const DIRECTORY_ENV: &str = "FE2O3_COMPILER_HANDOFF_OBSERVATION_DIRECTORY_TEST_ONLY_V1";
const CRATE_ENV: &str = "FE2O3_COMPILER_HANDOFF_OBSERVATION_CRATE_TEST_ONLY_V1";
const OBSERVATION_NAME: &str = "observation";
const ACK_NAME: &str = "ack";
const OBSERVATION_MAGIC: &[u8] = b"FE2O3-CARGO-WRAPPER-HANDOFF-OBSERVATION-V1\0";
const OBSERVATION_DOMAIN: &[u8] = b"FE2O3/CARGO-WRAPPER-HANDOFF-OBSERVATION/V1\0";
const PRODUCER_DOMAIN: &[u8] = b"FE2O3/CARGO-WRAPPER-HANDOFF-PRODUCER/V1\0";
const AUTHORITY_NONE: &[u8] = b"inert-one-shot-compiler-handoff-test-observation-no-authority";
const ACK_MAGIC: &[u8] = b"FE2O3-CARGO-WRAPPER-HANDOFF-OBSERVATION-ACK-V1\0";
const ACK_BYTES: usize = ACK_MAGIC.len() + 32 + 32;
const MAX_FIELD_BYTES: usize = 4096;
const ACK_TIMEOUT: Duration = Duration::from_secs(120);
const POLL_INTERVAL: Duration = Duration::from_millis(10);

pub(crate) struct Request {
    directory: PathBuf,
    crate_name: String,
    source_path: PathBuf,
    ordered_metadata: Vec<String>,
}

impl Request {
    pub(crate) fn for_compile(
        crate_name: &str,
        source_path: &Path,
        ordered_metadata: &[String],
    ) -> Result<Option<Self>, String> {
        let directory = std::env::var_os(DIRECTORY_ENV);
        let selected_crate = std::env::var_os(CRATE_ENV);
        let (Some(directory), Some(selected_crate)) = (directory, selected_crate) else {
            if std::env::var_os(DIRECTORY_ENV).is_some() || std::env::var_os(CRATE_ENV).is_some() {
                return Err(format!(
                    "test-only compiler-handoff observation requires both {DIRECTORY_ENV} and {CRATE_ENV}"
                ));
            }
            return Ok(None);
        };
        let selected_crate = selected_crate.to_str().ok_or_else(|| {
            format!("test-only compiler-handoff observation has non-UTF-8 {CRATE_ENV}")
        })?;
        if selected_crate != crate_name {
            return Ok(None);
        }
        let directory = validate_private_directory(Path::new(&directory))?;
        let source = source_path.to_str().ok_or_else(|| {
            "test-only compiler-handoff observation source path is not UTF-8".to_owned()
        })?;
        if crate_name.is_empty()
            || crate_name.len() > MAX_FIELD_BYTES
            || source.is_empty()
            || source.len() > MAX_FIELD_BYTES
            || ordered_metadata.is_empty()
            || ordered_metadata.len() > u16::MAX.into()
            || ordered_metadata
                .iter()
                .any(|value| value.is_empty() || value.len() > MAX_FIELD_BYTES)
        {
            return Err(
                "test-only compiler-handoff observation producer fields exceed their bounds"
                    .to_owned(),
            );
        }
        Ok(Some(Self {
            directory,
            crate_name: crate_name.to_owned(),
            source_path: source_path.to_owned(),
            ordered_metadata: ordered_metadata.to_vec(),
        }))
    }
}

pub(crate) fn publish_and_wait_for_consumption(
    request: &Request,
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<(), String> {
    validate_private_directory(&request.directory)?;
    let output_dir = fs::canonicalize(output_dir).map_err(|error| {
        format!("cannot resolve wrapper-owned compiler-handoff output directory: {error}")
    })?;
    let output = output_dir
        .to_str()
        .ok_or_else(|| "wrapper-owned compiler-handoff output directory is not UTF-8".to_owned())?;
    if output.is_empty() || output.len() > MAX_FIELD_BYTES {
        return Err("wrapper-owned compiler-handoff output directory exceeds its bound".to_owned());
    }

    let observation = encode_observation(
        attempt,
        &request.crate_name,
        request
            .source_path
            .to_str()
            .expect("validated observation source path is UTF-8"),
        output,
        &request.ordered_metadata,
    )?;
    let observation_sha256: [u8; 32] = Sha256::digest(&observation).into();
    write_private_file(&request.directory.join(OBSERVATION_NAME), &observation)?;
    wait_for_ack(&request.directory, observation_sha256)?;

    match consume_compiler_module_handoff_v1(output_dir.as_path(), producer, attempt) {
        Err(CompilerModuleHandoffErrorV1::AlreadyConsumed) => Ok(()),
        Ok(_) => Err(
            "test-only observer acknowledged without first consuming the exact compiler handoff"
                .to_owned(),
        ),
        Err(error) => Err(format!(
            "cannot verify test-only one-shot compiler-handoff consumption: {error}"
        )),
    }
}

fn encode_observation(
    attempt: BuildAttempt,
    crate_name: &str,
    source_path: &str,
    output_dir: &str,
    ordered_metadata: &[String],
) -> Result<Vec<u8>, String> {
    let attempt = attempt.to_env_value();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(OBSERVATION_MAGIC);
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(AUTHORITY_NONE);
    push_field(&mut bytes, attempt.as_bytes())?;
    push_field(&mut bytes, crate_name.as_bytes())?;
    push_field(&mut bytes, source_path.as_bytes())?;
    push_field(&mut bytes, output_dir.as_bytes())?;
    let metadata_count = u16::try_from(ordered_metadata.len())
        .map_err(|_| "test-only metadata observation count overflow".to_owned())?;
    bytes.extend_from_slice(&metadata_count.to_le_bytes());
    for value in ordered_metadata {
        push_field(&mut bytes, value.as_bytes())?;
    }
    bytes.extend_from_slice(&producer_binding(
        crate_name.as_bytes(),
        source_path.as_bytes(),
    ));
    let checksum = digest_parts(OBSERVATION_DOMAIN, &[&bytes]);
    bytes.extend_from_slice(&checksum);
    Ok(bytes)
}

fn producer_binding(crate_name: &[u8], source_path: &[u8]) -> [u8; 32] {
    digest_parts(PRODUCER_DOMAIN, &[crate_name, source_path])
}

fn digest_parts(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    for field in fields {
        digest.update((field.len() as u64).to_le_bytes());
        digest.update(field);
    }
    digest.finalize().into()
}

fn push_field(output: &mut Vec<u8>, field: &[u8]) -> Result<(), String> {
    if field.is_empty() || field.len() > MAX_FIELD_BYTES {
        return Err("test-only compiler-handoff observation field exceeds its bound".to_owned());
    }
    let length = u32::try_from(field.len())
        .map_err(|_| "test-only compiler-handoff observation field length overflow".to_owned())?;
    output.extend_from_slice(&length.to_le_bytes());
    output.extend_from_slice(field);
    Ok(())
}

fn wait_for_ack(directory: &Path, observation_sha256: [u8; 32]) -> Result<(), String> {
    let deadline = Instant::now() + ACK_TIMEOUT;
    let path = directory.join(ACK_NAME);
    loop {
        match read_private_file_if_complete(&path, ACK_BYTES)? {
            Some(bytes) => {
                if bytes[..ACK_MAGIC.len()] != *ACK_MAGIC
                    || bytes[ACK_MAGIC.len()..ACK_MAGIC.len() + 32] != observation_sha256
                    || bytes[ACK_MAGIC.len() + 32..].iter().all(|byte| *byte == 0)
                {
                    return Err("test-only compiler-handoff observation ack is invalid".to_owned());
                }
                return Ok(());
            }
            None if Instant::now() < deadline => thread::sleep(POLL_INTERVAL),
            None => {
                return Err(
                    "timed out waiting for test-only compiler-handoff observation ack".to_owned(),
                );
            }
        }
    }
}

fn write_private_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let directory = path
        .parent()
        .ok_or_else(|| "test-only observation path has no parent".to_owned())?;
    let name = path
        .file_name()
        .ok_or_else(|| "test-only observation path has no file name".to_owned())?;
    let temp_name = ".observation.tmp";
    let temp_path = directory.join(temp_name);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&temp_path)
        .map_err(|error| {
            format!(
                "cannot create test-only observation {}: {error}",
                temp_path.display()
            )
        })?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| {
            format!(
                "cannot persist test-only observation {}: {error}",
                temp_path.display()
            )
        })?;
    let directory_file = File::open(directory)
        .map_err(|error| format!("cannot pin test-only observation directory: {error}"))?;
    renameat_with(
        &directory_file,
        temp_name,
        &directory_file,
        name,
        RenameFlags::NOREPLACE,
    )
    .map_err(|error| format!("cannot publish test-only observation: {error}"))?;
    directory_file
        .sync_all()
        .map_err(|error| format!("cannot sync published test-only observation: {error}"))
}

fn read_private_file_if_complete(path: &Path, expected: usize) -> Result<Option<Vec<u8>>, String> {
    let mut file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "cannot open test-only observation ack {}: {error}",
                path.display()
            ));
        }
    };
    let metadata = file
        .metadata()
        .map_err(|error| format!("cannot inspect test-only observation ack: {error}"))?;
    if !metadata.is_file()
        || metadata.mode() & 0o777 != 0o600
        || metadata.nlink() != 1
        || metadata.uid() != unsafe { libc::geteuid() }
    {
        return Err("test-only compiler-handoff observation ack is not a private file".to_owned());
    }
    let length = usize::try_from(metadata.len())
        .map_err(|_| "test-only compiler-handoff observation ack length overflow".to_owned())?;
    if length < expected {
        return Ok(None);
    }
    if length != expected {
        return Err("test-only compiler-handoff observation ack length is invalid".to_owned());
    }
    let mut bytes = vec![0; expected];
    file.read_exact(&mut bytes)
        .map_err(|error| format!("cannot read test-only compiler-handoff ack: {error}"))?;
    Ok(Some(bytes))
}

fn validate_private_directory(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err("test-only compiler-handoff observation directory is not absolute".to_owned());
    }
    let canonical = fs::canonicalize(path).map_err(|error| {
        format!("cannot resolve test-only compiler-handoff observation directory: {error}")
    })?;
    if canonical != path {
        return Err("test-only compiler-handoff observation directory is not canonical".to_owned());
    }
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        format!("cannot inspect test-only compiler-handoff observation directory: {error}")
    })?;
    if !metadata.is_dir()
        || metadata.mode() & 0o777 != 0o700
        || metadata.uid() != unsafe { libc::geteuid() }
    {
        return Err(
            "test-only compiler-handoff observation directory is not private 0700".to_owned(),
        );
    }
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|error| format!("cannot sync test-only observation directory: {error}"))?;
    Ok(canonical)
}
