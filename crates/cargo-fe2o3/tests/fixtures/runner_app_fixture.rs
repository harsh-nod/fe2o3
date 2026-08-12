use std::env;
use std::ffi::{CString, OsStr, OsString};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::path::PathBuf;
use std::process::ExitCode;
use std::thread;
use std::time::Duration;

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

const TEST_ACK_READY_FD_ENV: &str = "FE2O3_INTERNAL_TEST_ACK_READY_FD";

struct ValidatedHandoff {
    report: serde_json::Value,
    _envelope: Option<File>,
    _artifact_directory: Option<File>,
    _current_lease: Option<DurableCurrentLinkPublicationLeaseV1>,
}

#[derive(Default)]
struct FixtureControls {
    probe_fd: Option<i32>,
    ignore_handoff: bool,
    reuse_handoff_fd: bool,
    reuse_artifact_directory_fd: bool,
    substitute_commitment: bool,
    public_ack_without_reacquire: bool,
    seccomp_escape_marker: Option<OsString>,
    exec_replacement_images: Option<(OsString, OsString)>,
    premature_close_ack: bool,
    extra_ack_byte: bool,
    stall_before_ack: Option<OsString>,
}

impl FixtureControls {
    fn parse(arguments: &[OsString]) -> Result<Self, String> {
        let mut controls = Self::default();
        let mut index = 2;
        while let Some(argument) = arguments.get(index) {
            let flag = argument
                .to_str()
                .ok_or_else(|| "fixture control flag is not UTF-8".to_string())?;
            match flag {
                "--fe2o3-test-probe-fd" => {
                    let value = arguments
                        .get(index + 1)
                        .and_then(|value| value.to_str())
                        .and_then(|value| value.parse::<i32>().ok())
                        .filter(|descriptor| *descriptor >= 3)
                        .ok_or_else(|| "fixture probe descriptor is invalid".to_string())?;
                    controls.probe_fd = Some(value);
                    index += 2;
                }
                "--fe2o3-test-ignore-handoff" => controls.ignore_handoff = true,
                "--fe2o3-test-reuse-handoff-fd" => controls.reuse_handoff_fd = true,
                "--fe2o3-test-reuse-artifact-dir-fd" => {
                    controls.reuse_artifact_directory_fd = true;
                }
                "--fe2o3-test-substitute-commitment" => {
                    controls.substitute_commitment = true;
                }
                "--fe2o3-test-public-ack-without-reacquire" => {
                    controls.public_ack_without_reacquire = true;
                }
                "--fe2o3-test-seccomp-process-probe" => {
                    controls.seccomp_escape_marker = Some(
                        arguments
                            .get(index + 1)
                            .ok_or_else(|| "seccomp probe requires an escape marker".to_string())?
                            .clone(),
                    );
                    index += 2;
                }
                "--fe2o3-test-exec-replacement-probe" => {
                    let static_image = arguments
                        .get(index + 1)
                        .ok_or_else(|| "exec probe requires a static image".to_string())?
                        .clone();
                    let dynamic_image = arguments
                        .get(index + 2)
                        .ok_or_else(|| "exec probe requires a dynamic image".to_string())?
                        .clone();
                    controls.exec_replacement_images = Some((static_image, dynamic_image));
                    index += 3;
                }
                "--fe2o3-test-premature-close-ack" => controls.premature_close_ack = true,
                "--fe2o3-test-extra-ack-byte" => controls.extra_ack_byte = true,
                "--fe2o3-test-stall-before-ack" => {
                    controls.stall_before_ack = Some(
                        arguments
                            .get(index + 1)
                            .ok_or_else(|| "stall probe requires a ready marker".to_string())?
                            .clone(),
                    );
                    index += 2;
                }
                _ => return Err(format!("unknown runner fixture control {argument:?}")),
            }
            if !matches!(
                flag,
                "--fe2o3-test-probe-fd"
                    | "--fe2o3-test-seccomp-process-probe"
                    | "--fe2o3-test-exec-replacement-probe"
                    | "--fe2o3-test-stall-before-ack"
            ) {
                index += 1;
            }
        }
        Ok(controls)
    }
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
    let controls = FixtureControls::parse(&args)?;
    let leaked_environment = env::vars_os()
        .filter_map(|(name, _)| {
            is_build_control(&name).then(|| name.to_string_lossy().into_owned())
        })
        .collect::<Vec<_>>();
    let payload = args
        .get(1)
        .map(|value| hex(os_bytes(value)))
        .unwrap_or_default();
    let unexpected_environment = env::vars_os()
        .filter_map(|(name, _)| (!is_handoff_environment(&name)).then_some(name))
        .map(|name| name.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    let handoff = validate_handoff(&controls)?;
    let probe_fd_open = controls.probe_fd.is_some_and(|descriptor| {
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
        "unexpected_environment": unexpected_environment,
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

fn validate_handoff(controls: &FixtureControls) -> Result<ValidatedHandoff, String> {
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
    if controls.ignore_handoff {
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

    if controls.reuse_handoff_fd {
        replace_descriptor(envelope_fd, "envelope")?;
    }
    if controls.reuse_artifact_directory_fd {
        replace_descriptor(artifact_directory_fd, "artifact directory")?;
    }
    if controls.substitute_commitment {
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
    let child =
        fs::read("/proc/self/exe").map_err(|error| format!("read child executable: {error}"))?;
    let expectation = WorkerV2ApplicationHandoffExpectationV1::new(
        &envelope,
        WorkerV2ApplicationIdentityV1::from_sealed_static_elf_v1(&child)
            .map_err(|error| format!("bind sealed-static child executable: {error}"))?,
    );
    let supplied_commitment = WorkerV2ApplicationHandoffCommitmentV1::from_hex(&commitment)
        .map_err(|error| format!("decode application handoff commitment: {error}"))?;
    if supplied_commitment != expectation.commitment() {
        return Err(
            "application handoff commitment does not match the descriptor and child".into(),
        );
    }

    if controls.public_ack_without_reacquire {
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
    if let Some(marker) = &controls.stall_before_ack {
        fs::write(marker, std::process::id().to_string())
            .map_err(|error| format!("write stalled-ACK ready marker: {error}"))?;
        if let Some(descriptor) = env::var_os(TEST_ACK_READY_FD_ENV) {
            let descriptor = descriptor
                .to_str()
                .and_then(|value| value.parse::<i32>().ok())
                .filter(|descriptor| *descriptor >= 3)
                .ok_or_else(|| "test ACK readiness descriptor is invalid".to_string())?;
            // SAFETY: the feature-gated internal runner context transfers ownership of this
            // write-only descriptor exclusively to the static test fixture.
            let mut ready = unsafe { File::from_raw_fd(descriptor) };
            validate_descriptor(&ready, rustix::fs::OFlags::WRONLY, "test ACK readiness")?;
            ready
                .write_all(&[1])
                .map_err(|error| format!("write test ACK readiness: {error}"))?;
        }
        thread::sleep(Duration::from_secs(30));
    }
    let process_creation = controls
        .seccomp_escape_marker
        .as_deref()
        .map(seccomp_process_probe)
        .transpose()?;
    let exec_replacement = controls
        .exec_replacement_images
        .as_ref()
        .map(|(static_image, dynamic_image)| exec_replacement_probe(static_image, dynamic_image))
        .transpose()?;

    if controls.premature_close_ack {
        drop(ack_file);
    } else {
        let acknowledgment = expectation.acknowledgment(challenge).encode_canonical();
        ack_file
            .write_all(&acknowledgment)
            .map_err(|error| format!("write application handoff acknowledgment: {error}"))?;
        if controls.extra_ack_byte {
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
            "process_creation": process_creation,
            "exec_replacement": exec_replacement,
            "read_only": true,
        }),
        _envelope: Some(envelope_file),
        _artifact_directory: Some(artifact_directory_file),
        _current_lease: Some(current_lease),
    })
}

fn seccomp_process_probe(escape_marker: &OsStr) -> Result<serde_json::Value, String> {
    let escape_marker = CString::new(os_bytes(escape_marker))
        .map_err(|_| "seccomp escape marker path contains NUL".to_string())?;
    let no_args = [0_usize; 6];
    let clone_args = [libc::SIGCHLD as usize, 0, 0, 0, 0, 0];
    let clone3_args = [0_usize, 0, 0, 0, 0, 0];
    let setns_args = [usize::MAX, 0, 0, 0, 0, 0];
    let io_uring_args = [1_usize, 0, 0, 0, 0, 0];
    blocked_process_creation("fork", libc::SYS_fork, no_args)?;
    blocked_process_creation("vfork", libc::SYS_vfork, no_args)?;
    blocked_process_creation("clone", libc::SYS_clone, clone_args)?;
    blocked_noncreation("clone3", libc::SYS_clone3, clone3_args)?;
    blocked_noncreation("unshare", libc::SYS_unshare, no_args)?;
    blocked_noncreation("setns", libc::SYS_setns, setns_args)?;
    blocked_noncreation("setsid", libc::SYS_setsid, no_args)?;
    blocked_noncreation("io_uring_setup", libc::SYS_io_uring_setup, io_uring_args)?;
    blocked_noncreation("io_uring_enter", libc::SYS_io_uring_enter, no_args)?;
    blocked_noncreation("io_uring_register", libc::SYS_io_uring_register, no_args)?;
    blocked_double_fork_setsid_escape(&escape_marker)?;
    Ok(serde_json::json!({
        "clone": "EPERM",
        "clone3": "EPERM",
        "double_fork_setsid": "EPERM",
        "fork": "EPERM",
        "io_uring": "EPERM",
        "setns": "EPERM",
        "setsid": "EPERM",
        "unshare": "EPERM",
        "vfork": "EPERM",
    }))
}

fn exec_replacement_probe(
    static_image: &OsStr,
    dynamic_image: &OsStr,
) -> Result<serde_json::Value, String> {
    for (kind, image) in [("static", static_image), ("dynamic", dynamic_image)] {
        let image = CString::new(os_bytes(image))
            .map_err(|_| format!("{kind} exec replacement path contains NUL"))?;
        blocked_exec_replacement(&format!("{kind} execve"), libc::SYS_execve, &image)?;
        blocked_exec_replacement(&format!("{kind} execveat"), libc::SYS_execveat, &image)?;
    }
    Ok(serde_json::json!({
        "dynamic_execve": "EPERM",
        "dynamic_execveat": "EPERM",
        "static_execve": "EPERM",
        "static_execveat": "EPERM",
    }))
}

fn blocked_exec_replacement(
    name: &str,
    syscall: libc::c_long,
    image: &CString,
) -> Result<(), String> {
    let arguments = [image.as_ptr(), std::ptr::null()];
    let environment = [std::ptr::null::<libc::c_char>()];
    let syscall_arguments = if syscall == libc::SYS_execve {
        [
            image.as_ptr() as usize,
            arguments.as_ptr() as usize,
            environment.as_ptr() as usize,
            0,
            0,
            0,
        ]
    } else {
        [
            libc::AT_FDCWD as isize as usize,
            image.as_ptr() as usize,
            arguments.as_ptr() as usize,
            environment.as_ptr() as usize,
            0,
            0,
        ]
    };
    if raw_syscall(syscall, syscall_arguments) >= 0 {
        return Err(format!("seccomp unexpectedly allowed {name}"));
    }
    expect_eperm(name)
}

fn blocked_process_creation(
    name: &str,
    number: libc::c_long,
    arguments: [usize; 6],
) -> Result<(), String> {
    let result = raw_syscall(number, arguments);
    if result == 0 {
        // SAFETY: this is the unexpected child path; `_exit` is async-signal-safe and prevents it
        // from returning into Rust or retaining inherited evidence descriptors.
        unsafe { libc::_exit(210) };
    }
    if result > 0 {
        raw_wait(result as libc::pid_t);
        return Err(format!("seccomp unexpectedly allowed {name}"));
    }
    expect_eperm(name)
}

fn blocked_noncreation(
    name: &str,
    number: libc::c_long,
    arguments: [usize; 6],
) -> Result<(), String> {
    if raw_syscall(number, arguments) >= 0 {
        return Err(format!("seccomp unexpectedly allowed {name}"));
    }
    expect_eperm(name)
}

fn blocked_double_fork_setsid_escape(escape_marker: &CString) -> Result<(), String> {
    let result = raw_syscall(libc::SYS_fork, [0; 6]);
    if result == 0 {
        if raw_syscall(libc::SYS_setsid, [0; 6]) < 0 {
            // SAFETY: this unexpected child cannot escape its inherited process group.
            unsafe { libc::_exit(213) };
        }
        let second = raw_syscall(libc::SYS_fork, [0; 6]);
        if second == 0 {
            // SAFETY: the path was converted before fork; these libc calls are async-signal-safe.
            let marker = unsafe {
                libc::open(
                    escape_marker.as_ptr(),
                    libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
                    0o600,
                )
            };
            if marker >= 0 {
                // SAFETY: the marker descriptor is owned by this unexpected grandchild.
                unsafe {
                    libc::write(marker, b"escaped".as_ptr().cast(), 7);
                    libc::close(marker);
                }
            }
            // SAFETY: an unexpectedly created grandchild exits immediately without Rust cleanup.
            unsafe { libc::_exit(212) };
        }
        if second > 0 {
            raw_wait(second as libc::pid_t);
        }
        // SAFETY: the unexpected first child has completed the exact escape sequence probe.
        unsafe { libc::_exit(211) };
    }
    if result > 0 {
        raw_wait(result as libc::pid_t);
        return Err("seccomp unexpectedly allowed double-fork+setsid stage one".to_string());
    }
    expect_eperm("double-fork+setsid")
}

fn raw_syscall(number: libc::c_long, arguments: [usize; 6]) -> libc::c_long {
    // SAFETY: all probes use the Linux syscall ABI. Seccomp rejects them before the kernel reads
    // argument pointers; unexpected child paths use only raw syscalls and `_exit`.
    unsafe {
        libc::syscall(
            number,
            arguments[0],
            arguments[1],
            arguments[2],
            arguments[3],
            arguments[4],
            arguments[5],
        )
    }
}

fn raw_wait(pid: libc::pid_t) {
    let mut status = 0;
    loop {
        // SAFETY: `status` is writable and `pid` came from an unexpected successful creation.
        let result = unsafe { libc::waitpid(pid, &mut status, 0) };
        if result >= 0 || std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted
        {
            break;
        }
    }
}

fn expect_eperm(name: &str) -> Result<(), String> {
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::EPERM) {
        Ok(())
    } else {
        Err(format!(
            "seccomp returned {error} instead of EPERM for {name}"
        ))
    }
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
    bytes.starts_with(b"FE2O3_") && !is_handoff_environment(name)
        || matches!(
            bytes,
            b"RUSTFLAGS"
                | b"CARGO_ENCODED_RUSTFLAGS"
                | b"RUSTC_WRAPPER"
                | b"RUSTC_WORKSPACE_WRAPPER"
        )
}

fn is_handoff_environment(name: &OsStr) -> bool {
    [
        WORKER_V2_APPLICATION_ENVELOPE_FD_ENV_V1,
        WORKER_V2_APPLICATION_ARTIFACT_DIR_FD_ENV_V1,
        WORKER_V2_APPLICATION_HANDOFF_COMMITMENT_ENV_V1,
        WORKER_V2_APPLICATION_HANDOFF_ACK_FD_ENV_V1,
        WORKER_V2_APPLICATION_HANDOFF_CHALLENGE_ENV_V1,
        TEST_ACK_READY_FD_ENV,
    ]
    .iter()
    .any(|allowed| name == OsStr::new(allowed))
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
