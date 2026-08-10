use std::env;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::path::PathBuf;
use std::process::ExitCode;

use fe2o3_artifact_transaction::{
    DurableCurrentLinkPublicationLeaseV1, reacquire_current_hsaco_publication_lease_v1,
};
use fe2o3_worker_v2_bundle::{
    MAX_WORKER_V2_LOAD_ENVELOPE_BYTES, WORKER_V2_APPLICATION_ARTIFACT_DIR_FD_ENV_V1,
    WORKER_V2_APPLICATION_ENVELOPE_FD_ENV_V1, WORKER_V2_APPLICATION_HANDOFF_ACK_FD_ENV_V1,
    WORKER_V2_APPLICATION_HANDOFF_CHALLENGE_ENV_V1,
    WORKER_V2_APPLICATION_HANDOFF_COMMITMENT_ENV_V1, WorkerV2ApplicationHandoffChallengeV1,
    WorkerV2ApplicationHandoffCommitmentV1, WorkerV2ApplicationHandoffExpectationV1,
    WorkerV2ApplicationIdentityV1, WorkerV2LoadEnvelopeV1,
};
use sha2::{Digest, Sha256};

struct ValidatedHandoff {
    report: serde_json::Value,
    _envelope: Option<File>,
    _artifact_directory: Option<File>,
    _current_lease: Option<DurableCurrentLinkPublicationLeaseV1>,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("runner fixture: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    let Some(report) = args.first().map(PathBuf::from) else {
        return Err("runner fixture requires a report path".to_string());
    };
    let leaked_environment = env::vars_os()
        .filter_map(|(name, _)| {
            is_build_control(&name).then(|| name.to_string_lossy().into_owned())
        })
        .collect::<Vec<_>>();
    let payload = args
        .get(1)
        .map(|value| hex(os_bytes(value)))
        .unwrap_or_default();
    let handoff = validate_handoff()?;
    let probe_fd_open = env::var("RUNNER_FIXTURE_PROBE_FD")
        .ok()
        .and_then(|value| value.parse::<i32>().ok())
        .is_some_and(|descriptor| {
            fs::symlink_metadata(format!("/proc/self/fd/{descriptor}")).is_ok()
        });
    let record = serde_json::json!({
        "artifact_fd_open": fs::symlink_metadata("/proc/self/fd/197").is_ok(),
        "backend_fd_open": fs::symlink_metadata("/proc/self/fd/198").is_ok(),
        "leaked_environment": leaked_environment,
        "handoff": &handoff.report,
        "payload_hex": payload,
        "probe_fd_open": probe_fd_open,
        "preserved_environment_hex": env::var_os("RUNNER_CHAIN_ENV")
            .map(|value| hex(os_bytes(&value))),
    });
    fs::write(
        report,
        serde_json::to_vec(&record).map_err(|error| format!("encode report: {error}"))?,
    )
    .map_err(|error| format!("write report: {error}"))?;
    // The descriptor-derived current-publication lease and evidence descriptors remain owned by
    // the application until all handoff-dependent application work above has completed.
    drop(handoff);
    Ok(())
}

fn validate_handoff() -> Result<ValidatedHandoff, String> {
    let names = [
        WORKER_V2_APPLICATION_ENVELOPE_FD_ENV_V1,
        WORKER_V2_APPLICATION_ARTIFACT_DIR_FD_ENV_V1,
        WORKER_V2_APPLICATION_HANDOFF_COMMITMENT_ENV_V1,
        WORKER_V2_APPLICATION_HANDOFF_ACK_FD_ENV_V1,
        WORKER_V2_APPLICATION_HANDOFF_CHALLENGE_ENV_V1,
    ];
    let values = names.map(env::var_os);
    if values.iter().all(Option::is_none) {
        return Ok(ValidatedHandoff {
            report: serde_json::Value::Null,
            _envelope: None,
            _artifact_directory: None,
            _current_lease: None,
        });
    }
    if values.iter().any(Option::is_none) {
        return Err("application received an incomplete handoff environment".to_string());
    }
    if env::var_os("RUNNER_FIXTURE_FORK_RETAIN_HANDOFF").is_some() {
        fork_descriptor_retaining_descendant()?;
        return Ok(ValidatedHandoff {
            report: serde_json::json!({"descendant_retained_handoff": true}),
            _envelope: None,
            _artifact_directory: None,
            _current_lease: None,
        });
    }
    if env::var_os("RUNNER_FIXTURE_IGNORE_HANDOFF").is_some() {
        return Ok(ValidatedHandoff {
            report: serde_json::json!({"ignored": true}),
            _envelope: None,
            _artifact_directory: None,
            _current_lease: None,
        });
    }

    let [
        envelope_fd,
        artifact_directory_fd,
        commitment,
        ack_fd,
        challenge,
    ] = values.map(|value| {
        value
            .expect("presence checked")
            .into_string()
            .map_err(|_| "application handoff environment is not UTF-8".to_string())
    });
    let envelope_fd = parse_fd(envelope_fd?, "envelope")?;
    let artifact_directory_fd = parse_fd(artifact_directory_fd?, "artifact directory")?;
    let ack_fd = parse_fd(ack_fd?, "acknowledgment")?;
    let challenge = WorkerV2ApplicationHandoffChallengeV1::from_hex(&challenge?)
        .map_err(|error| format!("decode application handoff challenge: {error}"))?;
    let mut commitment = commitment?;

    if env::var_os("RUNNER_FIXTURE_REUSE_HANDOFF_FD").is_some() {
        replace_descriptor(envelope_fd, "envelope")?;
    }
    if env::var_os("RUNNER_FIXTURE_REUSE_ARTIFACT_DIR_FD").is_some() {
        replace_descriptor(artifact_directory_fd, "artifact directory")?;
    }
    if env::var_os("RUNNER_FIXTURE_SUBSTITUTE_COMMITMENT").is_some() {
        let replacement = if commitment.starts_with('0') {
            "1"
        } else {
            "0"
        };
        commitment.replace_range(..1, replacement);
    }

    // SAFETY: the runner explicitly transfers ownership of these three descriptors. The ACK
    // descriptor is closed immediately after one response; evidence descriptors are retained.
    let mut envelope_file = unsafe { File::from_raw_fd(envelope_fd) };
    // SAFETY: ownership is transferred by the same complete handoff environment.
    let artifact_directory_file = unsafe { File::from_raw_fd(artifact_directory_fd) };
    // SAFETY: ownership is transferred by the same complete handoff environment.
    let mut ack_file = unsafe { File::from_raw_fd(ack_fd) };
    validate_descriptor(&envelope_file, rustix::fs::OFlags::RDONLY, "envelope")?;
    validate_descriptor(
        &artifact_directory_file,
        rustix::fs::OFlags::RDONLY,
        "artifact directory",
    )?;
    validate_descriptor(&ack_file, rustix::fs::OFlags::WRONLY, "acknowledgment")?;
    let directory_stat = rustix::fs::fstat(&artifact_directory_file)
        .map_err(|error| format!("inspect inherited artifact directory: {error}"))?;
    if rustix::fs::FileType::from_raw_mode(directory_stat.st_mode)
        != rustix::fs::FileType::Directory
    {
        return Err("inherited artifact directory descriptor is not a directory".to_string());
    }

    let mut bytes = Vec::new();
    Read::by_ref(&mut envelope_file)
        .take((MAX_WORKER_V2_LOAD_ENVELOPE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read inherited envelope: {error}"))?;
    let envelope = WorkerV2LoadEnvelopeV1::from_bytes(&bytes)
        .map_err(|error| format!("decode inherited envelope: {error}"))?;
    if envelope.to_bytes() != bytes {
        return Err("inherited envelope is not canonical".to_string());
    }
    let child_sha256: [u8; 32] = Sha256::digest(
        fs::read("/proc/self/exe").map_err(|error| format!("read child executable: {error}"))?,
    )
    .into();
    let expectation = WorkerV2ApplicationHandoffExpectationV1::new(
        &envelope,
        WorkerV2ApplicationIdentityV1::from_bytes(child_sha256),
    );
    let supplied_commitment = WorkerV2ApplicationHandoffCommitmentV1::from_hex(&commitment)
        .map_err(|error| format!("decode application handoff commitment: {error}"))?;
    if supplied_commitment != expectation.commitment() {
        return Err(
            "application handoff commitment does not match the descriptor and child".into(),
        );
    }

    if env::var_os("RUNNER_FIXTURE_PUBLIC_ACK_WITHOUT_REACQUIRE").is_some() {
        ack_file
            .write_all(&expectation.acknowledgment(challenge).encode_canonical())
            .map_err(|error| format!("write public protocol acknowledgment: {error}"))?;
        drop(ack_file);
        return Ok(ValidatedHandoff {
            report: serde_json::json!({
                "acknowledged": true,
                "child_reacquired_currentness": false,
                "commitment": supplied_commitment.to_hex(),
            }),
            _envelope: Some(envelope_file),
            _artifact_directory: Some(artifact_directory_file),
            _current_lease: None,
        });
    }

    let descriptor_path = PathBuf::from(format!("/proc/self/fd/{artifact_directory_fd}"));
    let current_lease =
        reacquire_current_hsaco_publication_lease_v1(&descriptor_path, envelope.published_claim())
            .map_err(|error| format!("reacquire descriptor-only current publication: {error}"))?;
    let current_token = current_lease
        .acquire_current_token()
        .map_err(|error| format!("retain current publication through acknowledgment: {error}"))?;

    if env::var_os("RUNNER_FIXTURE_PREMATURE_CLOSE_ACK").is_some() {
        drop(ack_file);
    } else {
        let acknowledgment = expectation.acknowledgment(challenge).encode_canonical();
        ack_file
            .write_all(&acknowledgment)
            .map_err(|error| format!("write application handoff acknowledgment: {error}"))?;
        if env::var_os("RUNNER_FIXTURE_EXTRA_ACK_BYTE").is_some() {
            ack_file
                .write_all(&[0])
                .map_err(|error| format!("write extra acknowledgment byte: {error}"))?;
        }
        ack_file
            .flush()
            .map_err(|error| format!("flush application handoff acknowledgment: {error}"))?;
        drop(ack_file);
    }
    drop(current_token);

    Ok(ValidatedHandoff {
        report: serde_json::json!({
            "acknowledged": true,
            "artifact_directory_descriptor": artifact_directory_fd,
            "artifact_directory_read_only": true,
            "child_reacquired_currentness": true,
            "commitment": supplied_commitment.to_hex(),
            "descriptor": envelope_fd,
            "envelope_identity": hex(&envelope.identity().as_bytes()),
            "read_only": true,
        }),
        _envelope: Some(envelope_file),
        _artifact_directory: Some(artifact_directory_file),
        _current_lease: Some(current_lease),
    })
}

fn fork_descriptor_retaining_descendant() -> Result<(), String> {
    let pid_file = env::var_os("RUNNER_FIXTURE_DESCENDANT_PID_FILE")
        .map(PathBuf::from)
        .ok_or_else(|| "descriptor-retention probe requires a descendant PID file".to_string())?;
    // SAFETY: the child executes only async-signal-safe libc operations and never returns into
    // Rust. It deliberately retains all inherited descriptors until runner containment kills it.
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        return Err(format!(
            "fork descriptor-retaining descendant: {}",
            std::io::Error::last_os_error()
        ));
    }
    if pid == 0 {
        loop {
            // SAFETY: `pause` has no memory preconditions and waits for process-group containment.
            unsafe { libc::pause() };
        }
    }
    fs::write(pid_file, pid.to_string())
        .map_err(|error| format!("write descriptor-retaining descendant PID: {error}"))
}

fn parse_fd(value: String, purpose: &str) -> Result<i32, String> {
    value
        .parse::<i32>()
        .ok()
        .filter(|descriptor| *descriptor >= 3)
        .ok_or_else(|| format!("application received an invalid {purpose} descriptor"))
}

fn replace_descriptor(descriptor: i32, purpose: &str) -> Result<(), String> {
    // SAFETY: this negative test deliberately closes an inherited descriptor before reusing its
    // number, proving that a descriptor number is not accepted as an identity.
    if unsafe { libc::close(descriptor) } != 0 {
        return Err(format!(
            "failed to close {purpose} descriptor for reuse probe: {}",
            std::io::Error::last_os_error()
        ));
    }
    let replacement =
        File::open("/dev/null").map_err(|error| format!("open descriptor replacement: {error}"))?;
    if replacement.as_raw_fd() == descriptor {
        std::mem::forget(replacement);
        return Ok(());
    }
    // SAFETY: both descriptor numbers are valid here and `dup2` atomically installs the probe.
    if unsafe { libc::dup2(replacement.as_raw_fd(), descriptor) } != descriptor {
        return Err(format!(
            "failed to install {purpose} descriptor replacement: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

fn validate_descriptor(
    descriptor: &File,
    expected_access: rustix::fs::OFlags,
    purpose: &str,
) -> Result<(), String> {
    let descriptor_flags = rustix::io::fcntl_getfd(descriptor)
        .map_err(|error| format!("inspect inherited {purpose} descriptor flags: {error}"))?;
    let status_flags = rustix::fs::fcntl_getfl(descriptor)
        .map_err(|error| format!("inspect inherited {purpose} descriptor access: {error}"))?;
    if descriptor_flags.contains(rustix::io::FdFlags::CLOEXEC)
        || status_flags & rustix::fs::OFlags::ACCMODE != expected_access
    {
        return Err(format!("inherited {purpose} descriptor has invalid flags"));
    }
    Ok(())
}

fn is_build_control(name: &OsStr) -> bool {
    let bytes = os_bytes(name);
    let handoff_control = [
        WORKER_V2_APPLICATION_ENVELOPE_FD_ENV_V1,
        WORKER_V2_APPLICATION_ARTIFACT_DIR_FD_ENV_V1,
        WORKER_V2_APPLICATION_HANDOFF_COMMITMENT_ENV_V1,
        WORKER_V2_APPLICATION_HANDOFF_ACK_FD_ENV_V1,
        WORKER_V2_APPLICATION_HANDOFF_CHALLENGE_ENV_V1,
    ]
    .iter()
    .any(|allowed| name == OsStr::new(allowed));
    bytes.starts_with(b"FE2O3_") && !handoff_control
        || matches!(
            bytes,
            b"RUSTFLAGS"
                | b"CARGO_ENCODED_RUSTFLAGS"
                | b"RUSTC_WRAPPER"
                | b"RUSTC_WORKSPACE_WRAPPER"
        )
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(unix)]
fn os_bytes(value: &OsStr) -> &[u8] {
    use std::os::unix::ffi::OsStrExt;
    value.as_bytes()
}

#[cfg(not(unix))]
fn os_bytes(value: &OsStr) -> &[u8] {
    value.to_str().expect("UTF-8 value off Unix").as_bytes()
}
