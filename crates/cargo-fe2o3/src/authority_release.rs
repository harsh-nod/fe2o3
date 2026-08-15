//! Protected self-launch boundary for authority-bearing cargo-fe2o3 requests.
//!
//! The public process pins its own executable and closure inputs, snapshots itself into a sealed
//! memfd, and supervises one exec of that image. The child accepts the release token only after
//! independently checking the parent process, executable backing objects and bytes, exact argv,
//! environment, cwd, descriptor manifest, compiler closure, and a fresh one-shot transcript.
//!
//! This module authenticates only launcher and handoff mechanics. It does not authenticate
//! compiler origin, proof validity, generated artifacts, memory safety, or GPU execution.

use std::env;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::{AsRawFd, BorrowedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, ExitStatus};
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::pinned_executable::PinnedExecutable;

pub(crate) const INTERNAL_CHILD_ARG: &str = "__fe2o3-authority-release-child-v1";

const RELEASE_ARG: &str = "release";
const PROBE_ARG: &str = "probe";
const CONTRACT_FD: RawFd = 187;
const CONTROL_FD: RawFd = 188;
const LAUNCHER_IMAGE_FD: RawFd = 189;
const CWD_FD: RawFd = 190;
const CONTRACT_MAGIC: &[u8; 8] = b"F2AURL1\0";
const CONTRACT_VERSION: u16 = 1;
const CONTRACT_HEADER_BYTES: usize = 24;
const CONTRACT_IDENTITY_BYTES: usize = 32;
const MAX_CONTRACT_BYTES: usize = 2 * 1024 * 1024;
const MAX_ARGUMENTS: usize = 512;
const MAX_ENVIRONMENT_ENTRIES: usize = 4096;
const MAX_FIELD_BYTES: usize = 1024 * 1024;
const MAX_PROC_STAT_BYTES: usize = 4096;
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
const CONTRACT_DOMAIN: &[u8] = b"FE2O3/PROTECTED-AUTHORITY-RELEASE-CONTRACT/V1\0";
const GRANT_DOMAIN: &[u8] = b"FE2O3/PROTECTED-AUTHORITY-RELEASE-GRANT/V1\0";
const ACCEPT_DOMAIN: &[u8] = b"FE2O3/PROTECTED-AUTHORITY-RELEASE-ACCEPT/V1\0";
const READY_MAGIC: &[u8; 8] = b"F2AURDY1";
const GRANT_MAGIC: &[u8; 8] = b"F2AUGRT1";
const ACCEPT_MAGIC: &[u8; 8] = b"F2AUACC1";
const READY_BYTES: usize = 8 + 32 + 32 + 4 + 8;
const GRANT_BYTES: usize = 8 + 32 + 32;
const ACCEPT_BYTES: usize = 8 + 32;
const REQUIRED_SEALS: rustix::fs::SealFlags = rustix::fs::SealFlags::WRITE
    .union(rustix::fs::SealFlags::GROW)
    .union(rustix::fs::SealFlags::SHRINK)
    .union(rustix::fs::SealFlags::SEAL);
const RELEASE_ENVIRONMENT_ALLOWLIST: &[&str] = &[
    "CARGO",
    "FE2O3_AUTHORITY_BACKEND_SHA256_V1",
    "FE2O3_AUTHORITY_CARGO_SHA256_V1",
    "FE2O3_AUTHORITY_RUSTC_PATH_V1",
    "FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1",
    "FE2O3_AUTHORITY_RUSTC_SHA256_V1",
    "FE2O3_BACKEND",
    "FE2O3_CODEGEN_PIPELINE",
    "FE2O3_TARGET",
    "FE2O3_WORKER_V2_CONFIG_V2",
    "LANG",
    "LC_ALL",
    "TZ",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ObjectIdentity {
    device: u64,
    inode: u64,
    mode: u32,
    size: u64,
}

impl ObjectIdentity {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
            size: metadata.len(),
        }
    }

    fn from_fd(fd: RawFd, label: &str) -> Result<Self, String> {
        // SAFETY: the caller supplies a live descriptor and the borrow is limited to fstat.
        let stat = unsafe { rustix::fs::fstat(BorrowedFd::borrow_raw(fd)) }
            .map_err(|error| format!("cannot inspect {label} descriptor {fd}: {error}"))?;
        Ok(Self {
            device: stat.st_dev,
            inode: stat.st_ino,
            mode: stat.st_mode,
            size: u64::try_from(stat.st_size)
                .map_err(|_| format!("{label} descriptor has a negative size"))?,
        })
    }

    fn encode(self, output: &mut Vec<u8>) {
        output.extend_from_slice(&self.device.to_le_bytes());
        output.extend_from_slice(&self.inode.to_le_bytes());
        output.extend_from_slice(&self.mode.to_le_bytes());
        output.extend_from_slice(&0_u32.to_le_bytes());
        output.extend_from_slice(&self.size.to_le_bytes());
    }

    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, String> {
        let device = decoder.u64()?;
        let inode = decoder.u64()?;
        let mode = decoder.u32()?;
        if decoder.u32()? != 0 {
            return Err("release contract object reserved field is nonzero".to_owned());
        }
        let size = decoder.u64()?;
        if inode == 0 {
            return Err("release contract object inode is zero".to_owned());
        }
        Ok(Self {
            device,
            inode,
            mode,
            size,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ImageIdentity {
    object: ObjectIdentity,
    sha256: [u8; 32],
}

impl ImageIdentity {
    fn encode(self, output: &mut Vec<u8>) {
        self.object.encode(output);
        output.extend_from_slice(&self.sha256);
    }

    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, String> {
        let object = ObjectIdentity::decode(decoder)?;
        let sha256 = decoder.array()?;
        require_nonzero(sha256, "release contract image")?;
        Ok(Self { object, sha256 })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CompilerClosureObservation {
    cargo: [u8; 32],
    rustc: [u8; 32],
    runtime_tree: [u8; 32],
    runtime_object: ObjectIdentity,
    backend: [u8; 32],
    closure: [u8; 32],
}

impl CompilerClosureObservation {
    fn encode(self, output: &mut Vec<u8>) {
        output.extend_from_slice(&self.cargo);
        output.extend_from_slice(&self.rustc);
        output.extend_from_slice(&self.runtime_tree);
        self.runtime_object.encode(output);
        output.extend_from_slice(&self.backend);
        output.extend_from_slice(&self.closure);
    }

    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, String> {
        let value = Self {
            cargo: decoder.array()?,
            rustc: decoder.array()?,
            runtime_tree: decoder.array()?,
            runtime_object: ObjectIdentity::decode(decoder)?,
            backend: decoder.array()?,
            closure: decoder.array()?,
        };
        for (label, digest) in [
            ("Cargo", value.cargo),
            ("rustc", value.rustc),
            ("runtime tree", value.runtime_tree),
            ("backend", value.backend),
            ("compiler closure", value.closure),
        ] {
            require_nonzero(digest, label)?;
        }
        if crate::compiler_toolchain::compiler_closure_sha256_v1(
            &value.cargo,
            &value.rustc,
            &value.runtime_tree,
            &value.backend,
        ) != value.closure
        {
            return Err("release contract compiler closure is not canonical".to_owned());
        }
        Ok(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReleaseContract {
    attempt: [u8; 32],
    parent_uid: u32,
    parent_pid: u32,
    parent_start_ticks: u64,
    launcher: ImageIdentity,
    child: ImageIdentity,
    cwd: ObjectIdentity,
    descriptors: [ObjectIdentity; 7],
    compiler: CompilerClosureObservation,
    argv: Vec<Vec<u8>>,
    environment: Vec<(Vec<u8>, Vec<u8>)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ChildObservation {
    parent_uid: u32,
    parent_pid: u32,
    parent_start_ticks: u64,
    launcher: ImageIdentity,
    child: ImageIdentity,
    cwd: ObjectIdentity,
    descriptors: [ObjectIdentity; 7],
    compiler: CompilerClosureObservation,
    argv: Vec<Vec<u8>>,
    environment: Vec<(Vec<u8>, Vec<u8>)>,
}

impl ReleaseContract {
    fn encode(&self) -> Result<Vec<u8>, String> {
        validate_fields(&self.argv, &self.environment)?;
        let mut body = Vec::new();
        body.extend_from_slice(&self.attempt);
        body.extend_from_slice(&self.parent_uid.to_le_bytes());
        body.extend_from_slice(&self.parent_pid.to_le_bytes());
        body.extend_from_slice(&self.parent_start_ticks.to_le_bytes());
        self.launcher.encode(&mut body);
        self.child.encode(&mut body);
        self.cwd.encode(&mut body);
        for descriptor in self.descriptors {
            descriptor.encode(&mut body);
        }
        self.compiler.encode(&mut body);
        encode_fields(&mut body, &self.argv)?;
        body.extend_from_slice(
            &u32::try_from(self.environment.len())
                .map_err(|_| "release environment count overflowed".to_owned())?
                .to_le_bytes(),
        );
        for (name, value) in &self.environment {
            encode_field(&mut body, name)?;
            encode_field(&mut body, value)?;
        }

        let total = CONTRACT_HEADER_BYTES
            .checked_add(body.len())
            .and_then(|value| value.checked_add(CONTRACT_IDENTITY_BYTES))
            .filter(|value| *value <= MAX_CONTRACT_BYTES)
            .ok_or_else(|| "release contract exceeds its byte bound".to_owned())?;
        let mut encoded = Vec::with_capacity(total);
        encoded.extend_from_slice(CONTRACT_MAGIC);
        encoded.extend_from_slice(&CONTRACT_VERSION.to_le_bytes());
        encoded.extend_from_slice(&(CONTRACT_HEADER_BYTES as u16).to_le_bytes());
        encoded.extend_from_slice(
            &u32::try_from(total)
                .map_err(|_| "release contract length overflowed".to_owned())?
                .to_le_bytes(),
        );
        encoded.extend_from_slice(&0_u64.to_le_bytes());
        encoded.extend_from_slice(&body);
        let identity = contract_identity(&encoded);
        encoded.extend_from_slice(&identity);
        Ok(encoded)
    }

    fn decode(encoded: &[u8]) -> Result<(Self, [u8; 32]), String> {
        if encoded.len() < CONTRACT_HEADER_BYTES + CONTRACT_IDENTITY_BYTES
            || encoded.len() > MAX_CONTRACT_BYTES
        {
            return Err("release contract has an invalid encoded length".to_owned());
        }
        if &encoded[..8] != CONTRACT_MAGIC {
            return Err("release contract magic differs".to_owned());
        }
        if u16::from_le_bytes(encoded[8..10].try_into().expect("fixed slice")) != CONTRACT_VERSION
            || usize::from(u16::from_le_bytes(
                encoded[10..12].try_into().expect("fixed slice"),
            )) != CONTRACT_HEADER_BYTES
        {
            return Err("release contract version/header differs".to_owned());
        }
        let declared = usize::try_from(u32::from_le_bytes(
            encoded[12..16].try_into().expect("fixed slice"),
        ))
        .map_err(|_| "release contract length is not representable".to_owned())?;
        if declared != encoded.len() || encoded[16..24] != [0; 8] {
            return Err("release contract length/reserved header differs".to_owned());
        }
        let identity_offset = encoded.len() - CONTRACT_IDENTITY_BYTES;
        let expected = contract_identity(&encoded[..identity_offset]);
        let declared_identity: [u8; 32] = encoded[identity_offset..]
            .try_into()
            .expect("identity suffix has fixed length");
        if expected != declared_identity {
            return Err("release contract identity differs".to_owned());
        }

        let mut decoder = Decoder::new(&encoded[CONTRACT_HEADER_BYTES..identity_offset]);
        let attempt = decoder.array()?;
        require_nonzero(attempt, "release attempt")?;
        let parent_uid = decoder.u32()?;
        let parent_pid = decoder.u32()?;
        let parent_start_ticks = decoder.u64()?;
        if parent_pid == 0 || parent_start_ticks == 0 {
            return Err("release contract parent process identity is zero".to_owned());
        }
        let launcher = ImageIdentity::decode(&mut decoder)?;
        let child = ImageIdentity::decode(&mut decoder)?;
        let cwd = ObjectIdentity::decode(&mut decoder)?;
        let mut descriptors = [cwd; 7];
        for descriptor in &mut descriptors {
            *descriptor = ObjectIdentity::decode(&mut decoder)?;
        }
        let compiler = CompilerClosureObservation::decode(&mut decoder)?;
        let argv = decoder.fields(MAX_ARGUMENTS)?;
        let environment_count = usize::try_from(decoder.u32()?)
            .map_err(|_| "release environment count is not representable".to_owned())?;
        if environment_count > MAX_ENVIRONMENT_ENTRIES {
            return Err("release environment has too many entries".to_owned());
        }
        let mut environment = Vec::with_capacity(environment_count);
        for _ in 0..environment_count {
            environment.push((decoder.field()?, decoder.field()?));
        }
        decoder.finish()?;
        validate_fields(&argv, &environment)?;
        Ok((
            Self {
                attempt,
                parent_uid,
                parent_pid,
                parent_start_ticks,
                launcher,
                child,
                cwd,
                descriptors,
                compiler,
                argv,
                environment,
            },
            expected,
        ))
    }
}

pub(crate) struct ProtectedReleaseAdmission {
    attempt: [u8; 32],
    contract_identity: [u8; 32],
    control: UnixStream,
}

impl ProtectedReleaseAdmission {
    pub(crate) const fn attempt(&self) -> &[u8; 32] {
        &self.attempt
    }

    pub(crate) const fn contract_identity(&self) -> &[u8; 32] {
        &self.contract_identity
    }
}

impl Drop for ProtectedReleaseAdmission {
    fn drop(&mut self) {
        let _ = self.control.shutdown(std::net::Shutdown::Both);
    }
}

pub(crate) fn command(args: &[OsString]) -> ExitCode {
    if args.first().and_then(|value| value.to_str()) != Some(RELEASE_ARG) {
        eprintln!("cargo fe2o3 authority requires: authority release <build|run|probe> [args]");
        return ExitCode::FAILURE;
    }
    let child_args = &args[1..];
    if !matches!(
        child_args.first().and_then(|value| value.to_str()),
        Some("build" | "run" | PROBE_ARG)
    ) {
        eprintln!("cargo fe2o3 authority release requires build, run, or probe");
        return ExitCode::FAILURE;
    }
    let admission_checks = crate::reject_dynamic_loader_environment()
        .and_then(|()| crate::reject_preexisting_compiler_environment())
        .and_then(|()| crate::reject_authority_environment_overrides(&child_args[1..]));
    if let Err(error) = admission_checks {
        eprintln!("cargo fe2o3 authority release: {error}");
        return ExitCode::FAILURE;
    }
    if env::var_os(crate::NON_PRODUCTION_AUTHORITY_VALIDATION_ENV).is_some() {
        eprintln!(
            "cargo fe2o3 authority release rejects the non-production validation escape hatch"
        );
        return ExitCode::FAILURE;
    }
    if let Err(error) = validate_release_environment() {
        eprintln!("cargo fe2o3 authority release: {error}");
        return ExitCode::FAILURE;
    }
    match launch(child_args) {
        Ok(status) => ExitCode::from(exit_code(status)),
        Err(error) => {
            eprintln!("cargo fe2o3 authority release: {error}");
            ExitCode::FAILURE
        }
    }
}

pub(crate) fn run_child(args: &[OsString]) -> ExitCode {
    let admission = match admit_child(args) {
        Ok(admission) => admission,
        Err(error) => {
            eprintln!("cargo fe2o3 authority release child: {error}");
            return ExitCode::FAILURE;
        }
    };
    if args.first().and_then(|value| value.to_str()) == Some(PROBE_ARG) {
        println!(
            "FE2O3_PROTECTED_AUTHORITY_RELEASE_V1_OK attempt={} contract={} runtime_authority=none gpu_authority=none",
            hex(admission.attempt()),
            hex(admission.contract_identity())
        );
        return ExitCode::SUCCESS;
    }
    crate::cargo_with_protected_release(args, admission)
}

fn launch(args: &[OsString]) -> Result<ExitStatus, String> {
    if !crate::authority_sensitive_request_selected() {
        return Err(
            "authority release requires an authority-bearing pipeline selection".to_owned(),
        );
    }
    reject_reserved_descriptors(&[0, 1, 2])?;
    let compiler = observe_compiler_closure()?;
    let environment = current_environment()?;
    let argv = planned_child_argv(args)?;
    let attempt = random_identity()?;
    let parent_pid = std::process::id();
    let parent_uid = unsafe { libc::geteuid() };
    let parent_start_ticks = process_start_time_ticks(parent_pid)?;

    let (launcher_file, launcher) = pin_process_image(parent_pid)?;
    let current_path = env::current_exe()
        .map_err(|error| format!("cannot resolve cargo-fe2o3 release executable: {error}"))?;
    let current = PinnedExecutable::open(&current_path)
        .map_err(|error| format!("cannot pin cargo-fe2o3 release executable: {error}"))?;
    if current.object_identity()
        != fe2o3_process_identity::LinuxObjectIdentityV3::from_linux_stat(
            launcher.object.device,
            launcher.object.inode,
            launcher.object.mode,
        )
        || current.sha256() != &launcher.sha256
    {
        return Err("current executable path does not name the running launcher image".to_owned());
    }
    let child_image = current
        .seal_executable_image()
        .map_err(|error| format!("cannot seal cargo-fe2o3 release image: {error}"))?;
    let child_file = child_image
        .try_clone_for_transfer()
        .map_err(|error| format!("cannot retain sealed child image: {error}"))?;
    let child_identity = image_identity(&child_file, *child_image.sha256())?;
    if child_identity.sha256 != launcher.sha256 {
        return Err("sealed child bytes differ from the running launcher".to_owned());
    }

    let cwd_file = open_current_directory()?;
    let cwd = ObjectIdentity::from_metadata(
        &cwd_file
            .metadata()
            .map_err(|error| format!("cannot inspect release cwd: {error}"))?,
    );
    let stdio = [
        ObjectIdentity::from_fd(0, "stdin")?,
        ObjectIdentity::from_fd(1, "stdout")?,
        ObjectIdentity::from_fd(2, "stderr")?,
    ];
    let (mut parent_control, child_control) = UnixStream::pair()
        .map_err(|error| format!("cannot create release control socket: {error}"))?;
    configure_timeouts(&parent_control)?;

    let contract_file = create_contract_file()?;
    let contract_object = ObjectIdentity::from_fd(contract_file.as_raw_fd(), "release contract")?;
    let child_control_object =
        ObjectIdentity::from_fd(child_control.as_raw_fd(), "release child control")?;
    let launcher_object = ObjectIdentity::from_metadata(
        &launcher_file
            .metadata()
            .map_err(|error| format!("cannot inspect retained launcher image: {error}"))?,
    );
    let descriptors = [
        stdio[0],
        stdio[1],
        stdio[2],
        contract_object,
        child_control_object,
        launcher_object,
        cwd,
    ];
    let mut contract = ReleaseContract {
        attempt,
        parent_uid,
        parent_pid,
        parent_start_ticks,
        launcher,
        child: child_identity,
        cwd,
        descriptors,
        compiler,
        argv,
        environment: environment_bytes(&environment),
    };
    let provisional = contract.encode()?;
    contract.descriptors[3].size = provisional.len() as u64;
    let encoded = contract.encode()?;
    if encoded.len() != provisional.len() {
        return Err("release contract size did not converge".to_owned());
    }
    write_and_seal_contract(&contract_file, &encoded)?;
    let (_, contract_identity) = ReleaseContract::decode(&encoded)?;

    let mut command = child_image
        .command()
        .map_err(|error| format!("cannot prepare sealed release child: {error}"))?;
    command
        .as_command_mut()
        .arg0(OsStr::from_bytes(
            fe2o3_build_authority::PROTECTED_AUTHORITY_ARGV0_V1,
        ))
        .arg(INTERNAL_CHILD_ARG)
        .args(args)
        .env_clear()
        .envs(environment.iter().cloned());
    install_child_boundary(
        command.as_command_mut(),
        &contract_file,
        &child_control,
        &launcher_file,
        &cwd_file,
        &contract.descriptors[3..],
    )?;
    let mut child = command
        .as_command_mut()
        .spawn()
        .map_err(|error| format!("cannot exec sealed release child: {error}"))?;
    drop(child_control);

    let handshake = parent_handshake(
        &mut parent_control,
        &mut child,
        &contract,
        contract_identity,
    );
    if let Err(error) = handshake {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    let status = child
        .wait()
        .map_err(|error| format!("cannot wait for protected release child: {error}"))?;
    if process_start_time_ticks(parent_pid)? != parent_start_ticks {
        return Err("release launcher process identity changed while supervising child".to_owned());
    }
    Ok(status)
}

fn admit_child(args: &[OsString]) -> Result<ProtectedReleaseAdmission, String> {
    let encoded = read_sealed_contract(CONTRACT_FD)?;
    let (contract, contract_identity) = ReleaseContract::decode(&encoded)?;
    validate_child_state(&contract, args)?;
    let mut control = take_control_socket(CONTROL_FD)?;
    configure_timeouts(&control)?;
    authenticate_parent(&contract, &control)?;
    let pid = std::process::id();
    let start_ticks = process_start_time_ticks(pid)?;

    let mut ready = Vec::with_capacity(READY_BYTES);
    ready.extend_from_slice(READY_MAGIC);
    ready.extend_from_slice(&contract.attempt);
    ready.extend_from_slice(&contract_identity);
    ready.extend_from_slice(&pid.to_le_bytes());
    ready.extend_from_slice(&start_ticks.to_le_bytes());
    control
        .write_all(&ready)
        .map_err(|error| format!("cannot send release readiness: {error}"))?;

    let mut grant = [0_u8; GRANT_BYTES];
    control
        .read_exact(&mut grant)
        .map_err(|error| format!("cannot receive release grant: {error}"))?;
    if &grant[..8] != GRANT_MAGIC || grant[8..40] != contract.attempt {
        return Err("release grant frame does not match the exact attempt".to_owned());
    }
    let expected_grant = grant_identity(&contract, &contract_identity, pid, start_ticks);
    if grant[40..] != expected_grant {
        return Err("release grant identity differs".to_owned());
    }
    let accept = accept_identity(&contract.attempt, &expected_grant);
    let mut frame = Vec::with_capacity(ACCEPT_BYTES);
    frame.extend_from_slice(ACCEPT_MAGIC);
    frame.extend_from_slice(&accept);
    control
        .write_all(&frame)
        .map_err(|error| format!("cannot acknowledge release grant: {error}"))?;

    close_admission_descriptors()?;
    Ok(ProtectedReleaseAdmission {
        attempt: contract.attempt,
        contract_identity,
        control,
    })
}

fn parent_handshake(
    control: &mut UnixStream,
    child: &mut std::process::Child,
    contract: &ReleaseContract,
    contract_identity: [u8; 32],
) -> Result<(), String> {
    let mut ready = [0_u8; READY_BYTES];
    control
        .read_exact(&mut ready)
        .map_err(|error| format!("protected release child did not become ready: {error}"))?;
    if &ready[..8] != READY_MAGIC
        || ready[8..40] != contract.attempt
        || ready[40..72] != contract_identity
    {
        return Err("protected release readiness frame differs".to_owned());
    }
    let pid = u32::from_le_bytes(ready[72..76].try_into().expect("fixed slice"));
    let start_ticks = u64::from_le_bytes(ready[76..84].try_into().expect("fixed slice"));
    if pid != child.id() || process_start_time_ticks(pid)? != start_ticks {
        return Err("protected release child process identity differs".to_owned());
    }
    let (_, observed_child) = pin_process_image(pid)?;
    if observed_child != contract.child {
        return Err("protected release child executable image differs".to_owned());
    }
    let grant = grant_identity(contract, &contract_identity, pid, start_ticks);
    let mut frame = Vec::with_capacity(GRANT_BYTES);
    frame.extend_from_slice(GRANT_MAGIC);
    frame.extend_from_slice(&contract.attempt);
    frame.extend_from_slice(&grant);
    control
        .write_all(&frame)
        .map_err(|error| format!("cannot send protected release grant: {error}"))?;
    let mut accept = [0_u8; ACCEPT_BYTES];
    control
        .read_exact(&mut accept)
        .map_err(|error| format!("protected release child did not accept grant: {error}"))?;
    if &accept[..8] != ACCEPT_MAGIC || accept[8..] != accept_identity(&contract.attempt, &grant) {
        return Err("protected release acceptance identity differs".to_owned());
    }
    Ok(())
}

fn validate_child_state(contract: &ReleaseContract, args: &[OsString]) -> Result<(), String> {
    validate_release_environment()?;
    let parent_pid = u32::try_from(unsafe { libc::getppid() })
        .map_err(|_| "release child parent PID is negative".to_owned())?;
    let (_, launcher) = pin_process_image(parent_pid)?;
    let (_, child) = pin_process_image(std::process::id())?;
    let cwd = ObjectIdentity::from_metadata(
        &fs::metadata(".").map_err(|error| format!("cannot inspect release child cwd: {error}"))?,
    );
    reject_reserved_descriptors(&[0, 1, 2, CONTRACT_FD, CONTROL_FD, LAUNCHER_IMAGE_FD, CWD_FD])?;
    let descriptors = [
        ObjectIdentity::from_fd(0, "release stdin")?,
        ObjectIdentity::from_fd(1, "release stdout")?,
        ObjectIdentity::from_fd(2, "release stderr")?,
        ObjectIdentity::from_fd(CONTRACT_FD, "release contract")?,
        ObjectIdentity::from_fd(CONTROL_FD, "release control")?,
        ObjectIdentity::from_fd(LAUNCHER_IMAGE_FD, "release launcher image")?,
        ObjectIdentity::from_fd(CWD_FD, "release cwd")?,
    ];
    let launcher_file = clone_fd(LAUNCHER_IMAGE_FD, "launcher image")?;
    if image_identity(&launcher_file, contract.launcher.sha256)? != launcher {
        return Err("retained launcher image differs from the sealed contract".to_owned());
    }
    validate_child_observation(
        contract,
        &ChildObservation {
            parent_uid: unsafe { libc::geteuid() },
            parent_pid,
            parent_start_ticks: process_start_time_ticks(parent_pid)?,
            launcher,
            child,
            cwd,
            descriptors,
            compiler: observe_compiler_closure()?,
            argv: observed_child_argv(args)?,
            environment: environment_bytes(&current_environment()?),
        },
    )
}

fn validate_child_observation(
    contract: &ReleaseContract,
    observed: &ChildObservation,
) -> Result<(), String> {
    if observed.parent_uid != contract.parent_uid
        || observed.parent_pid != contract.parent_pid
        || observed.parent_start_ticks != contract.parent_start_ticks
    {
        return Err("release child does not have the exact admitted launcher parent".to_owned());
    }
    if observed.launcher != contract.launcher {
        return Err("release launcher executable image/backing object differs".to_owned());
    }
    if observed.child != contract.child {
        return Err("release child executable image/backing object differs".to_owned());
    }
    if observed.cwd != contract.cwd {
        return Err("release child cwd differs from the retained object".to_owned());
    }
    if observed.descriptors != contract.descriptors {
        return Err("release child descriptor manifest differs".to_owned());
    }
    if observed.compiler != contract.compiler {
        return Err("release compiler closure/runtime tree drifted across exec".to_owned());
    }
    if observed.argv != contract.argv {
        return Err("release child argv differs from the sealed contract".to_owned());
    }
    if observed.environment != contract.environment {
        return Err("release child environment differs from the sealed contract".to_owned());
    }
    Ok(())
}

fn authenticate_parent(contract: &ReleaseContract, control: &UnixStream) -> Result<(), String> {
    let credentials = rustix::net::sockopt::socket_peercred(control)
        .map_err(|error| format!("cannot inspect release launcher peer: {error}"))?;
    if credentials.uid.as_raw() != contract.parent_uid
        || u32::try_from(credentials.pid.as_raw_nonzero().get()).ok() != Some(contract.parent_pid)
    {
        return Err("release control peer does not match the admitted launcher".to_owned());
    }
    Ok(())
}

fn observe_compiler_closure() -> Result<CompilerClosureObservation, String> {
    let cargo_expected = crate::authority_cargo_sha256_from_environment()?;
    let rustc_expected = crate::authority_rustc_sha256_from_environment()?;
    let runtime_expected = crate::authority_rustc_runtime_sha256_from_environment()?;
    let backend_expected = crate::authority_backend_sha256_from_environment()?;
    let cargo = canonical_file_from_env("CARGO")?;
    let rustc = canonical_file_from_env(crate::AUTHORITY_RUSTC_PATH_ENV)?;
    let backend = canonical_file_from_env(crate::BACKEND_ENV)?;
    let cargo_observed = bounded_regular_file_sha256(&cargo, "authority Cargo")?;
    let rustc_observed = bounded_regular_file_sha256(&rustc, "authority rustc")?;
    let backend_observed = bounded_regular_file_sha256(&backend, "authority backend")?;
    for (label, observed, expected) in [
        ("Cargo", cargo_observed, cargo_expected),
        ("rustc", rustc_observed, rustc_expected),
        ("backend", backend_observed, backend_expected),
    ] {
        if observed != expected {
            return Err(format!(
                "authority {label} differs from its declared digest"
            ));
        }
    }
    let runtime_directory = crate::rustc_lib_tree_directory(&rustc)?;
    let runtime_object = ObjectIdentity::from_metadata(
        &runtime_directory
            .file()
            .metadata()
            .map_err(|error| format!("cannot inspect authority rustc runtime tree: {error}"))?,
    );
    let runtime = crate::rustc_lib_tree::PinnedRustcLibTree::pin(runtime_directory)?;
    if runtime.sha256() != &runtime_expected {
        return Err("authority rustc runtime tree differs from its declared digest".to_owned());
    }
    runtime.revalidate()?;
    let closure = crate::compiler_toolchain::compiler_closure_sha256_v1(
        &cargo_observed,
        &rustc_observed,
        &runtime_expected,
        &backend_observed,
    );
    Ok(CompilerClosureObservation {
        cargo: cargo_observed,
        rustc: rustc_observed,
        runtime_tree: runtime_expected,
        runtime_object,
        backend: backend_observed,
        closure,
    })
}

fn canonical_file_from_env(name: &str) -> Result<PathBuf, String> {
    let declared = env::var_os(name).ok_or_else(|| format!("authority release requires {name}"))?;
    let path = PathBuf::from(declared);
    if !path.is_absolute() {
        return Err(format!("authority release requires {name} to be absolute"));
    }
    let canonical = fs::canonicalize(&path)
        .map_err(|error| format!("cannot canonicalize authority {name}: {error}"))?;
    if canonical != path {
        return Err(format!("authority release rejects aliased {name} path"));
    }
    Ok(path)
}

fn bounded_regular_file_sha256(path: &Path, label: &str) -> Result<[u8; 32], String> {
    let mut file = File::open(path).map_err(|error| format!("cannot open {label}: {error}"))?;
    let before = file
        .metadata()
        .map_err(|error| format!("cannot inspect {label}: {error}"))?;
    if !before.is_file() || before.len() == 0 || before.len() > 512 * 1024 * 1024 {
        return Err(format!("{label} is not a bounded nonempty regular file"));
    }
    let snapshot = metadata_snapshot(&before);
    let mut digest = Sha256::new();
    let mut copied = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("cannot hash {label}: {error}"))?;
        if read == 0 {
            break;
        }
        copied = copied
            .checked_add(read as u64)
            .ok_or_else(|| format!("{label} byte count overflowed"))?;
        digest.update(&buffer[..read]);
    }
    let after = file
        .metadata()
        .map_err(|error| format!("cannot re-inspect {label}: {error}"))?;
    if copied != before.len() || metadata_snapshot(&after) != snapshot {
        return Err(format!("{label} changed while it was measured"));
    }
    Ok(digest.finalize().into())
}

fn metadata_snapshot(metadata: &fs::Metadata) -> (u64, u64, u32, u64, i64, i64, i64, i64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.mode(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    )
}

fn current_environment() -> Result<Vec<(OsString, OsString)>, String> {
    let mut values = env::vars_os().collect::<Vec<_>>();
    if values.len() > MAX_ENVIRONMENT_ENTRIES {
        return Err("release environment has too many entries".to_owned());
    }
    values.sort_by(|left, right| left.0.as_bytes().cmp(right.0.as_bytes()));
    for pair in values.windows(2) {
        if pair[0].0.as_bytes() == pair[1].0.as_bytes() {
            return Err("release environment contains a duplicate name".to_owned());
        }
    }
    Ok(values)
}

fn validate_release_environment() -> Result<(), String> {
    for (name, value) in env::vars_os() {
        let Some(name_text) = name.to_str() else {
            return Err("authority release rejects a non-UTF-8 environment name".to_owned());
        };
        if !RELEASE_ENVIRONMENT_ALLOWLIST.contains(&name_text) {
            return Err(format!(
                "authority release rejects unexpected inherited environment {name:?}={value:?}"
            ));
        }
    }
    for (name, expected) in [("LANG", "C"), ("LC_ALL", "C"), ("TZ", "UTC")] {
        if env::var_os(name).as_deref() != Some(OsStr::new(expected)) {
            return Err(format!(
                "authority release requires exact environment {name}={expected}"
            ));
        }
    }
    Ok(())
}

fn environment_bytes(values: &[(OsString, OsString)]) -> Vec<(Vec<u8>, Vec<u8>)> {
    values
        .iter()
        .map(|(name, value)| (name.as_bytes().to_vec(), value.as_bytes().to_vec()))
        .collect()
}

fn planned_child_argv(args: &[OsString]) -> Result<Vec<Vec<u8>>, String> {
    child_argv_with(
        OsStr::from_bytes(fe2o3_build_authority::PROTECTED_AUTHORITY_ARGV0_V1),
        args,
    )
}

fn observed_child_argv(args: &[OsString]) -> Result<Vec<Vec<u8>>, String> {
    let argv0 = env::args_os()
        .next()
        .ok_or_else(|| "release child has no argv[0]".to_owned())?;
    child_argv_with(&argv0, args)
}

fn child_argv_with(argv0: &OsStr, args: &[OsString]) -> Result<Vec<Vec<u8>>, String> {
    let mut argv = Vec::with_capacity(args.len() + 2);
    argv.push(argv0.as_bytes().to_vec());
    argv.push(INTERNAL_CHILD_ARG.as_bytes().to_vec());
    argv.extend(args.iter().map(|value| value.as_bytes().to_vec()));
    if argv.len() > MAX_ARGUMENTS || argv.iter().any(|value| value.is_empty()) {
        return Err("release child argv has an invalid count or empty argument".to_owned());
    }
    if argv.iter().any(|value| value.contains(&0)) {
        return Err("release child argv contains an interior NUL".to_owned());
    }
    Ok(argv)
}

fn open_current_directory() -> Result<File, String> {
    rustix::fs::open(
        ".",
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(File::from)
    .map_err(|error| format!("cannot retain release cwd: {error}"))
}

fn create_contract_file() -> Result<File, String> {
    let file = rustix::fs::memfd_create(
        "fe2o3-authority-release-contract-v1",
        rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
    )
    .map(File::from)
    .map_err(|error| format!("cannot allocate release contract image: {error}"))?;
    file.set_permissions(fs::Permissions::from_mode(0o400))
        .map_err(|error| format!("cannot make release contract read-only: {error}"))?;
    Ok(file)
}

fn write_and_seal_contract(file: &File, bytes: &[u8]) -> Result<(), String> {
    let mut writer = file
        .try_clone()
        .map_err(|error| format!("cannot clone release contract: {error}"))?;
    writer
        .write_all(bytes)
        .and_then(|()| writer.flush())
        .map_err(|error| format!("cannot write release contract: {error}"))?;
    rustix::fs::fcntl_add_seals(
        file,
        rustix::fs::SealFlags::WRITE | rustix::fs::SealFlags::GROW | rustix::fs::SealFlags::SHRINK,
    )
    .and_then(|()| rustix::fs::fcntl_add_seals(file, rustix::fs::SealFlags::SEAL))
    .map_err(|error| format!("cannot seal release contract: {error}"))?;
    if rustix::fs::fcntl_get_seals(file)
        .map_err(|error| format!("cannot inspect release contract seals: {error}"))?
        != REQUIRED_SEALS
    {
        return Err("release contract has unexpected seals".to_owned());
    }
    Ok(())
}

fn install_child_boundary(
    command: &mut Command,
    contract: &File,
    control: &UnixStream,
    launcher: &File,
    cwd: &File,
    expected: &[ObjectIdentity],
) -> Result<(), String> {
    for fd in [CONTRACT_FD, CONTROL_FD, LAUNCHER_IMAGE_FD, CWD_FD] {
        if fs::metadata(format!("/proc/self/fd/{fd}")).is_ok() {
            return Err(format!(
                "reserved release descriptor {fd} is already in use"
            ));
        }
    }
    let sources = [
        contract.as_raw_fd(),
        control.as_raw_fd(),
        launcher.as_raw_fd(),
        cwd.as_raw_fd(),
    ];
    let expected: [ObjectIdentity; 4] = expected
        .try_into()
        .map_err(|_| "release descriptor manifest has the wrong count".to_owned())?;
    for ((source, identity), label) in
        sources
            .into_iter()
            .zip(expected)
            .zip(["contract", "control", "launcher image", "cwd"])
    {
        let observed = ObjectIdentity::from_fd(source, label)?;
        if observed != identity {
            return Err(format!(
                "release {label} source descriptor differs from its manifest: expected {identity:?}, observed {observed:?}"
            ));
        }
    }
    // SAFETY: all source files remain borrowed through spawn; the callback performs only
    // descriptor operations and fchdir before exec.
    unsafe {
        command.pre_exec(move || {
            for ((source, target), identity) in sources
                .into_iter()
                .zip([CONTRACT_FD, CONTROL_FD, LAUNCHER_IMAGE_FD, CWD_FD])
                .zip(expected)
            {
                if source != target && libc::dup3(source, target, 0) != target {
                    return Err(std::io::Error::last_os_error());
                }
                let stat = rustix::fs::fstat(BorrowedFd::borrow_raw(target))
                    .map_err(std::io::Error::from)?;
                if stat.st_dev != identity.device
                    || stat.st_ino != identity.inode
                    || stat.st_mode != identity.mode
                    || u64::try_from(stat.st_size).ok() != Some(identity.size)
                {
                    return Err(std::io::Error::from_raw_os_error(
                        rustix::io::Errno::STALE.raw_os_error(),
                    ));
                }
                rustix::io::fcntl_setfd(
                    BorrowedFd::borrow_raw(target),
                    rustix::io::FdFlags::empty(),
                )
                .map_err(std::io::Error::from)?;
            }
            if libc::fchdir(CWD_FD) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    Ok(())
}

fn read_sealed_contract(fd: RawFd) -> Result<Vec<u8>, String> {
    let mut file = clone_fd(fd, "release contract")?;
    if rustix::fs::fcntl_get_seals(&file)
        .map_err(|error| format!("cannot inspect release contract seals: {error}"))?
        != REQUIRED_SEALS
    {
        return Err("release contract descriptor is not fully sealed".to_owned());
    }
    let metadata = file
        .metadata()
        .map_err(|error| format!("cannot inspect release contract: {error}"))?;
    let length = usize::try_from(metadata.len())
        .ok()
        .filter(|length| *length <= MAX_CONTRACT_BYTES)
        .ok_or_else(|| "release contract length exceeds its bound".to_owned())?;
    file.seek(SeekFrom::Start(0))
        .map_err(|error| format!("cannot rewind release contract: {error}"))?;
    let mut bytes = Vec::with_capacity(length);
    file.read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read release contract: {error}"))?;
    if bytes.len() != length {
        return Err("release contract changed length while reading".to_owned());
    }
    Ok(bytes)
}

fn take_control_socket(fd: RawFd) -> Result<UnixStream, String> {
    let cloned = rustix::io::fcntl_dupfd_cloexec(
        // SAFETY: admission owns a live fixed control descriptor at this point.
        unsafe { BorrowedFd::borrow_raw(fd) },
        3,
    )
    .map_err(|error| format!("cannot clone release control socket: {error}"))?;
    Ok(UnixStream::from(cloned))
}

fn clone_fd(fd: RawFd, label: &str) -> Result<File, String> {
    let cloned = rustix::io::fcntl_dupfd_cloexec(
        // SAFETY: callers use this only after checking their fixed descriptor manifest.
        unsafe { BorrowedFd::borrow_raw(fd) },
        3,
    )
    .map_err(|error| format!("cannot clone {label} descriptor: {error}"))?;
    Ok(File::from(cloned))
}

fn close_admission_descriptors() -> Result<(), String> {
    // SAFETY: admission has validated the complete fixed descriptor manifest immediately before
    // this transition, and each descriptor is still owned by this process.
    unsafe {
        for fd in [CONTRACT_FD, CONTROL_FD, LAUNCHER_IMAGE_FD, CWD_FD] {
            rustix::io::close(fd);
        }
    }
    Ok(())
}

fn pin_process_image(pid: u32) -> Result<(File, ImageIdentity), String> {
    let path = PathBuf::from(format!("/proc/{pid}/exe"));
    let initial_start = process_start_time_ticks(pid)?;
    let file = File::open(&path)
        .map_err(|error| format!("cannot open process image {}: {error}", path.display()))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("cannot inspect process image {}: {error}", path.display()))?;
    let pinned = PinnedExecutable::from_transferred_file(
        file.try_clone()
            .map_err(|error| format!("cannot clone process image: {error}"))?,
        path,
    )
    .map_err(|error| format!("cannot pin process image: {error}"))?;
    if process_start_time_ticks(pid)? != initial_start {
        return Err("process identity changed while its image was pinned".to_owned());
    }
    Ok((
        file,
        ImageIdentity {
            object: ObjectIdentity::from_metadata(&metadata),
            sha256: *pinned.sha256(),
        },
    ))
}

fn image_identity(file: &File, expected_sha256: [u8; 32]) -> Result<ImageIdentity, String> {
    let pinned = PinnedExecutable::from_transferred_file(
        file.try_clone()
            .map_err(|error| format!("cannot clone retained image: {error}"))?,
        PathBuf::from("<retained release image>"),
    )
    .map_err(|error| format!("cannot pin retained release image: {error}"))?;
    if pinned.sha256() != &expected_sha256 {
        return Err("retained release image bytes differ".to_owned());
    }
    Ok(ImageIdentity {
        object: ObjectIdentity::from_metadata(
            &file
                .metadata()
                .map_err(|error| format!("cannot inspect retained release image: {error}"))?,
        ),
        sha256: expected_sha256,
    })
}

fn process_start_time_ticks(pid: u32) -> Result<u64, String> {
    let path = PathBuf::from(format!("/proc/{pid}/stat"));
    let bytes = fs::read(&path)
        .map_err(|error| format!("cannot read release process {}: {error}", path.display()))?;
    if bytes.is_empty() || bytes.len() > MAX_PROC_STAT_BYTES {
        return Err("release process stat has an invalid length".to_owned());
    }
    let close = bytes
        .iter()
        .rposition(|byte| *byte == b')')
        .ok_or_else(|| "release process stat has no command terminator".to_owned())?;
    let recorded = bytes[..close]
        .split(|byte| *byte == b' ')
        .next()
        .and_then(|field| std::str::from_utf8(field).ok())
        .and_then(|field| field.parse::<u32>().ok());
    if recorded != Some(pid) {
        return Err("release process stat PID differs".to_owned());
    }
    bytes[close + 1..]
        .split(u8::is_ascii_whitespace)
        .filter(|field| !field.is_empty())
        .nth(19)
        .and_then(|field| std::str::from_utf8(field).ok())
        .and_then(|field| field.parse::<u64>().ok())
        .filter(|value| *value != 0)
        .ok_or_else(|| "release process stat has no valid start time".to_owned())
}

fn reject_reserved_descriptors(allowed: &[RawFd]) -> Result<(), String> {
    let directory = fs::read_dir("/proc/self/fd")
        .map_err(|error| format!("cannot enumerate release descriptors: {error}"))?;
    let mut unexpected = Vec::new();
    for entry in directory {
        let entry = entry.map_err(|error| format!("cannot inspect release descriptor: {error}"))?;
        let Some(fd) = entry
            .file_name()
            .to_str()
            .and_then(|value| value.parse::<RawFd>().ok())
        else {
            return Err("release descriptor name is not canonical".to_owned());
        };
        if allowed.contains(&fd) {
            continue;
        }
        let target = fs::read_link(entry.path())
            .map_err(|error| format!("cannot resolve release descriptor {fd}: {error}"))?;
        if target == format!("/proc/{}/fd", std::process::id()) {
            continue;
        }
        unexpected.push(fd);
    }
    if unexpected.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "authority release rejects unexpected inherited descriptors {unexpected:?}"
        ))
    }
}

fn configure_timeouts(stream: &UnixStream) -> Result<(), String> {
    stream
        .set_read_timeout(Some(HANDSHAKE_TIMEOUT))
        .and_then(|()| stream.set_write_timeout(Some(HANDSHAKE_TIMEOUT)))
        .map_err(|error| format!("cannot bound release handshake: {error}"))
}

fn random_identity() -> Result<[u8; 32], String> {
    let mut value = [0_u8; 32];
    File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut value))
        .map_err(|error| format!("cannot obtain fresh release attempt: {error}"))?;
    require_nonzero(value, "release attempt")?;
    Ok(value)
}

fn contract_identity(bytes: &[u8]) -> [u8; 32] {
    domain_hash(CONTRACT_DOMAIN, &[bytes])
}

fn grant_identity(
    contract: &ReleaseContract,
    contract_identity: &[u8; 32],
    child_pid: u32,
    child_start: u64,
) -> [u8; 32] {
    domain_hash(
        GRANT_DOMAIN,
        &[
            &contract.attempt,
            contract_identity,
            &contract.parent_pid.to_le_bytes(),
            &contract.parent_start_ticks.to_le_bytes(),
            &child_pid.to_le_bytes(),
            &child_start.to_le_bytes(),
            &contract.launcher.sha256,
            &contract.child.sha256,
            &contract.compiler.closure,
        ],
    )
}

fn accept_identity(attempt: &[u8; 32], grant: &[u8; 32]) -> [u8; 32] {
    domain_hash(ACCEPT_DOMAIN, &[attempt, grant])
}

fn domain_hash(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update((domain.len() as u64).to_le_bytes());
    digest.update(domain);
    for field in fields {
        digest.update((field.len() as u64).to_le_bytes());
        digest.update(field);
    }
    digest.finalize().into()
}

fn require_nonzero(value: [u8; 32], label: &str) -> Result<(), String> {
    if value == [0; 32] {
        Err(format!("{label} identity is zero"))
    } else {
        Ok(())
    }
}

fn validate_fields(argv: &[Vec<u8>], environment: &[(Vec<u8>, Vec<u8>)]) -> Result<(), String> {
    if argv.len() < 3 || argv.len() > MAX_ARGUMENTS {
        return Err("release contract argv has an invalid count".to_owned());
    }
    if argv[0] != fe2o3_build_authority::PROTECTED_AUTHORITY_ARGV0_V1
        || argv[1] != INTERNAL_CHILD_ARG.as_bytes()
    {
        return Err("release contract argv prefix differs".to_owned());
    }
    if environment.len() > MAX_ENVIRONMENT_ENTRIES {
        return Err("release contract environment has too many entries".to_owned());
    }
    let mut previous: Option<&[u8]> = None;
    for (name, value) in environment {
        if name.is_empty()
            || name.contains(&b'=')
            || name.contains(&0)
            || value.contains(&0)
            || name.len() > MAX_FIELD_BYTES
            || value.len() > MAX_FIELD_BYTES
        {
            return Err("release contract environment contains an invalid field".to_owned());
        }
        if previous.is_some_and(|previous| previous >= name.as_slice()) {
            return Err("release contract environment is not strictly sorted".to_owned());
        }
        previous = Some(name);
    }
    Ok(())
}

fn encode_fields(output: &mut Vec<u8>, fields: &[Vec<u8>]) -> Result<(), String> {
    output.extend_from_slice(
        &u32::try_from(fields.len())
            .map_err(|_| "release field count overflowed".to_owned())?
            .to_le_bytes(),
    );
    for field in fields {
        encode_field(output, field)?;
    }
    Ok(())
}

fn encode_field(output: &mut Vec<u8>, field: &[u8]) -> Result<(), String> {
    if field.len() > MAX_FIELD_BYTES {
        return Err("release contract field exceeds its byte bound".to_owned());
    }
    output.extend_from_slice(
        &u32::try_from(field.len())
            .map_err(|_| "release field length overflowed".to_owned())?
            .to_le_bytes(),
    );
    output.extend_from_slice(field);
    Ok(())
}

struct Decoder<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Decoder<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], String> {
        let end = self
            .cursor
            .checked_add(length)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| "release contract is truncated".to_owned())?;
        let value = &self.bytes[self.cursor..end];
        self.cursor = end;
        Ok(value)
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("fixed slice"),
        ))
    }

    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().expect("fixed slice"),
        ))
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        Ok(self.take(N)?.try_into().expect("fixed slice"))
    }

    fn field(&mut self) -> Result<Vec<u8>, String> {
        let length = usize::try_from(self.u32()?)
            .map_err(|_| "release field length is not representable".to_owned())?;
        if length > MAX_FIELD_BYTES {
            return Err("release field exceeds its byte bound".to_owned());
        }
        Ok(self.take(length)?.to_vec())
    }

    fn fields(&mut self, maximum: usize) -> Result<Vec<Vec<u8>>, String> {
        let count = usize::try_from(self.u32()?)
            .map_err(|_| "release field count is not representable".to_owned())?;
        if count > maximum {
            return Err("release contract has too many fields".to_owned());
        }
        (0..count).map(|_| self.field()).collect()
    }

    fn finish(self) -> Result<(), String> {
        if self.cursor == self.bytes.len() {
            Ok(())
        } else {
            Err("release contract has trailing bytes".to_owned())
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn exit_code(status: ExitStatus) -> u8 {
    status
        .code()
        .and_then(|code| u8::try_from(code).ok())
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object(seed: u64, mode: u32) -> ObjectIdentity {
        ObjectIdentity {
            device: seed,
            inode: seed + 1,
            mode,
            size: seed + 2,
        }
    }

    fn contract() -> ReleaseContract {
        let compiler = CompilerClosureObservation {
            cargo: [1; 32],
            rustc: [2; 32],
            runtime_tree: [3; 32],
            runtime_object: object(30, libc::S_IFDIR | 0o500),
            backend: [4; 32],
            closure: crate::compiler_toolchain::compiler_closure_sha256_v1(
                &[1; 32], &[2; 32], &[3; 32], &[4; 32],
            ),
        };
        ReleaseContract {
            attempt: [5; 32],
            parent_uid: 1000,
            parent_pid: 123,
            parent_start_ticks: 456,
            launcher: ImageIdentity {
                object: object(1, libc::S_IFREG | 0o500),
                sha256: [6; 32],
            },
            child: ImageIdentity {
                object: object(10, libc::S_IFREG | 0o500),
                sha256: [6; 32],
            },
            cwd: object(20, libc::S_IFDIR | 0o500),
            descriptors: [
                object(40, libc::S_IFIFO | 0o600),
                object(50, libc::S_IFIFO | 0o600),
                object(60, libc::S_IFIFO | 0o600),
                object(70, libc::S_IFREG | 0o400),
                object(80, libc::S_IFSOCK | 0o600),
                object(1, libc::S_IFREG | 0o500),
                object(20, libc::S_IFDIR | 0o500),
            ],
            compiler,
            argv: vec![
                fe2o3_build_authority::PROTECTED_AUTHORITY_ARGV0_V1.to_vec(),
                INTERNAL_CHILD_ARG.as_bytes().to_vec(),
                b"probe".to_vec(),
            ],
            environment: vec![(b"LANG".to_vec(), b"C".to_vec())],
        }
    }

    fn observation(contract: &ReleaseContract) -> ChildObservation {
        ChildObservation {
            parent_uid: contract.parent_uid,
            parent_pid: contract.parent_pid,
            parent_start_ticks: contract.parent_start_ticks,
            launcher: contract.launcher,
            child: contract.child,
            cwd: contract.cwd,
            descriptors: contract.descriptors,
            compiler: contract.compiler,
            argv: contract.argv.clone(),
            environment: contract.environment.clone(),
        }
    }

    #[test]
    fn contract_round_trips_canonically() {
        let expected = contract();
        let encoded = expected.encode().unwrap();
        let (decoded, identity) = ReleaseContract::decode(&encoded).unwrap();
        assert_eq!(decoded, expected);
        assert_eq!(identity, contract_identity(&encoded[..encoded.len() - 32]));
        assert_eq!(decoded.encode().unwrap(), encoded);
    }

    #[test]
    fn every_contract_byte_is_identity_bound() {
        let encoded = contract().encode().unwrap();
        for index in 0..encoded.len() {
            let mut changed = encoded.clone();
            changed[index] ^= 1;
            assert!(ReleaseContract::decode(&changed).is_err(), "byte {index}");
        }
    }

    #[test]
    fn aliases_environment_drift_and_closure_drift_change_identity() {
        let baseline = contract().encode().unwrap();
        let mut changed = contract();
        changed.environment.push((b"TZ".to_vec(), b"UTC".to_vec()));
        assert_ne!(baseline, changed.encode().unwrap());

        let mut changed = contract();
        changed.compiler.runtime_tree[0] ^= 1;
        changed.compiler.closure = crate::compiler_toolchain::compiler_closure_sha256_v1(
            &changed.compiler.cargo,
            &changed.compiler.rustc,
            &changed.compiler.runtime_tree,
            &changed.compiler.backend,
        );
        assert_ne!(baseline, changed.encode().unwrap());

        let mut changed = contract();
        changed.launcher.object.inode += 1;
        assert_ne!(baseline, changed.encode().unwrap());
    }

    #[test]
    fn transcript_is_bound_to_attempt_process_and_contract() {
        let contract = contract();
        let encoded = contract.encode().unwrap();
        let identity = contract_identity(&encoded[..encoded.len() - 32]);
        let grant = grant_identity(&contract, &identity, 789, 987);
        assert_ne!(grant, grant_identity(&contract, &identity, 790, 987));
        let mut replay = contract.clone();
        replay.attempt[0] ^= 1;
        assert_ne!(grant, grant_identity(&replay, &identity, 789, 987));
        assert_ne!(
            accept_identity(&contract.attempt, &grant),
            accept_identity(&replay.attempt, &grant)
        );
    }

    #[test]
    fn launcher_substitution_and_backing_replacement_are_rejected() {
        let contract = contract();
        let mut substitute = observation(&contract);
        substitute.launcher.sha256[0] ^= 1;
        assert!(
            validate_child_observation(&contract, &substitute)
                .unwrap_err()
                .contains("launcher executable image")
        );

        let mut replacement = observation(&contract);
        replacement.launcher.object.inode += 1;
        assert!(
            validate_child_observation(&contract, &replacement)
                .unwrap_err()
                .contains("backing object")
        );

        let mut child_replacement = observation(&contract);
        child_replacement.child.object.inode += 1;
        assert!(
            validate_child_observation(&contract, &child_replacement)
                .unwrap_err()
                .contains("child executable image")
        );
    }

    #[test]
    fn descriptor_substitution_reuse_and_cwd_drift_are_rejected() {
        let contract = contract();
        let mut descriptor = observation(&contract);
        descriptor.descriptors[4].inode += 1;
        assert!(
            validate_child_observation(&contract, &descriptor)
                .unwrap_err()
                .contains("descriptor manifest")
        );

        let mut reused_pid = observation(&contract);
        reused_pid.parent_start_ticks += 1;
        assert!(
            validate_child_observation(&contract, &reused_pid)
                .unwrap_err()
                .contains("exact admitted launcher parent")
        );

        let mut cwd = observation(&contract);
        cwd.cwd.inode += 1;
        assert!(
            validate_child_observation(&contract, &cwd)
                .unwrap_err()
                .contains("cwd")
        );
    }

    #[test]
    fn argv_environment_and_closure_drift_are_rejected() {
        let contract = contract();
        let mut argv = observation(&contract);
        argv.argv.push(b"--hostile".to_vec());
        assert!(
            validate_child_observation(&contract, &argv)
                .unwrap_err()
                .contains("argv")
        );

        let mut argv0 = observation(&contract);
        argv0.argv[0] = b"/tmp/cargo-fe2o3-alias".to_vec();
        assert!(
            validate_child_observation(&contract, &argv0)
                .unwrap_err()
                .contains("argv")
        );

        let mut environment = observation(&contract);
        environment.environment[0].1 = b"hostile".to_vec();
        assert!(
            validate_child_observation(&contract, &environment)
                .unwrap_err()
                .contains("environment")
        );

        let mut closure = observation(&contract);
        closure.compiler.runtime_object.inode += 1;
        assert!(
            validate_child_observation(&contract, &closure)
                .unwrap_err()
                .contains("closure/runtime tree")
        );
    }
}
