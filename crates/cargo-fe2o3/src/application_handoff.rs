//! Descriptor-only Cargo-to-application handoff for canonical Worker V2 evidence.
//!
//! ACK bytes are child-visible protocol completion, not authentication or authority. The runner's
//! non-clone current-publication lease and pinned descriptors remain the authority-bearing state.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::mem::MaybeUninit;
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus};
use std::time::{Duration, Instant};

use fe2o3_artifact_transaction::{
    DurableCurrentLinkPublicationLeaseV1, reacquire_current_hsaco_publication_lease_v1,
};
use fe2o3_worker_v2_bundle::{
    MAX_WORKER_V2_LOAD_ENVELOPE_BYTES, WORKER_V2_APPLICATION_ARTIFACT_DIR_FD_ENV_V1,
    WORKER_V2_APPLICATION_ENVELOPE_FD_ENV_V1, WORKER_V2_APPLICATION_HANDOFF_ACK_BYTES_V1,
    WORKER_V2_APPLICATION_HANDOFF_ACK_FD_ENV_V1, WORKER_V2_APPLICATION_HANDOFF_CHALLENGE_ENV_V1,
    WORKER_V2_APPLICATION_HANDOFF_COMMITMENT_ENV_V1, WORKER_V2_LOAD_ENVELOPE_NAME_PREFIX_V1,
    WORKER_V2_LOAD_ENVELOPE_NAME_SUFFIX_V1, WorkerV2ApplicationHandoffAckV1,
    WorkerV2ApplicationHandoffChallengeV1, WorkerV2ApplicationHandoffExpectationV1,
    WorkerV2ApplicationIdentityV1, WorkerV2LoadEnvelopeV1, worker_v2_load_envelope_name_v1,
};
use rustix::fs::{AtFlags, FileType, Mode, OFlags, ResolveFlags, fstat, openat2, statat};

use crate::application_sandbox::{
    ApplicationSandboxGuard, PendingApplicationSandbox, install_application_profile,
    no_fork_application_filter,
};
use crate::generation;
use crate::project::PinnedDirectory;

pub(crate) const RUNNER_CONTEXT_VERSION: &str = "3";
pub(crate) const RUNNER_EXPECTS_ENVELOPE: &str = "required";
pub(crate) const RUNNER_EXPECTS_NO_ENVELOPE: &str = "none";

const ENVELOPE_PREFIX: &[u8] = WORKER_V2_LOAD_ENVELOPE_NAME_PREFIX_V1.as_bytes();
const ENVELOPE_SUFFIX: &[u8] = WORKER_V2_LOAD_ENVELOPE_NAME_SUFFIX_V1.as_bytes();
const ENVELOPE_NAME_BYTES: usize = ENVELOPE_PREFIX.len() + 64 + ENVELOPE_SUFFIX.len();
const MAX_ENVELOPE_CANDIDATES: usize = 256;
const ACK_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileSnapshot {
    device: u64,
    inode: u64,
    mode: u32,
    links: u64,
    size: i64,
    modified_seconds: i64,
    modified_nanoseconds: u64,
    changed_seconds: i64,
    changed_nanoseconds: u64,
}

impl FileSnapshot {
    const fn from_stat(stat: &rustix::fs::Stat) -> Self {
        Self {
            device: stat.st_dev,
            inode: stat.st_ino,
            mode: stat.st_mode,
            links: stat.st_nlink,
            size: stat.st_size,
            modified_seconds: stat.st_mtime,
            modified_nanoseconds: stat.st_mtime_nsec,
            changed_seconds: stat.st_ctime,
            changed_nanoseconds: stat.st_ctime_nsec,
        }
    }
}

pub(crate) fn open_expected_generation(
    path: PathBuf,
    expected_device: u64,
    expected_inode: u64,
) -> Result<PinnedDirectory, String> {
    let directory = PinnedDirectory::open_existing(path, "Cargo application artifact directory")?;
    if !directory.matches_identity(expected_device, expected_inode) {
        return Err("Cargo application artifact directory identity was substituted".to_string());
    }
    generation::validate_owned_artifact(&directory)?;
    let stat = fstat(directory.file()).map_err(|error| {
        format!("failed to inspect Cargo application artifact directory: {error}")
    })?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory
        || stat.st_uid != unsafe { libc::geteuid() }
        || stat.st_mode & 0o022 != 0
    {
        return Err(
            "Cargo application artifact directory is not controlled by the current owner"
                .to_string(),
        );
    }
    directory.validate_path("Cargo application artifact directory")?;
    Ok(directory)
}

pub(crate) struct PinnedApplicationEnvelope<'directory> {
    directory: &'directory PinnedDirectory,
    name: String,
    file: File,
    snapshot: FileSnapshot,
    exact_bytes: Vec<u8>,
    envelope: WorkerV2LoadEnvelopeV1,
    artifact_directory_file: File,
    current_lease: Option<DurableCurrentLinkPublicationLeaseV1>,
}

impl<'directory> PinnedApplicationEnvelope<'directory> {
    pub(crate) fn discover(directory: &'directory PinnedDirectory) -> Result<Option<Self>, String> {
        directory.validate_path("Cargo application artifact directory")?;
        let names = envelope_names(directory)?;
        if names.is_empty() {
            return Ok(None);
        }

        let mut current = None;
        let mut rejected = Vec::new();
        for name in names {
            let candidate = Self::open(directory, name)?;
            match candidate.retain_current_lease() {
                Ok(candidate) if current.is_none() => current = Some(candidate),
                Ok(_) => {
                    return Err(
                        "multiple canonical Worker V2 envelopes claim the current publication"
                            .to_string(),
                    );
                }
                Err(error) => rejected.push(error),
            }
        }
        directory.validate_path("Cargo application artifact directory")?;
        current.map(Some).ok_or_else(|| {
            format!(
                "canonical Worker V2 envelopes exist but none is current: {}",
                rejected
                    .first()
                    .map(String::as_str)
                    .unwrap_or("no candidate admitted")
            )
        })
    }

    fn open(directory: &'directory PinnedDirectory, name: String) -> Result<Self, String> {
        let descriptor = openat2(
            directory.file(),
            Path::new(&name),
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
            ResolveFlags::BENEATH
                | ResolveFlags::NO_SYMLINKS
                | ResolveFlags::NO_MAGICLINKS
                | ResolveFlags::NO_XDEV,
        )
        .map_err(|error| format!("failed to open canonical Worker V2 envelope {name}: {error}"))?;
        let flags = rustix::io::fcntl_getfd(&descriptor)
            .map_err(|error| format!("failed to inspect envelope descriptor flags: {error}"))?;
        let status = rustix::fs::fcntl_getfl(&descriptor)
            .map_err(|error| format!("failed to inspect envelope access mode: {error}"))?;
        if !flags.contains(rustix::io::FdFlags::CLOEXEC)
            || status & OFlags::ACCMODE != OFlags::RDONLY
        {
            return Err("canonical Worker V2 envelope descriptor is not read-only CLOEXEC".into());
        }
        let initial = fstat(&descriptor)
            .map_err(|error| format!("failed to inspect canonical Worker V2 envelope: {error}"))?;
        validate_envelope_stat(directory, &name, &initial)?;
        let snapshot = FileSnapshot::from_stat(&initial);
        let size = usize::try_from(initial.st_size).map_err(|_| {
            "canonical Worker V2 envelope has a negative or unrepresentable size".to_string()
        })?;
        if size == 0 || size > MAX_WORKER_V2_LOAD_ENVELOPE_BYTES {
            return Err(format!(
                "canonical Worker V2 envelope size {size} is outside 1..={MAX_WORKER_V2_LOAD_ENVELOPE_BYTES}"
            ));
        }
        let mut file = File::from(descriptor);
        let mut exact_bytes = Vec::with_capacity(size.saturating_add(1));
        Read::by_ref(&mut file)
            .take((MAX_WORKER_V2_LOAD_ENVELOPE_BYTES + 1) as u64)
            .read_to_end(&mut exact_bytes)
            .map_err(|error| format!("failed to read canonical Worker V2 envelope: {error}"))?;
        let final_stat = fstat(&file).map_err(|error| {
            format!("failed to re-inspect canonical Worker V2 envelope: {error}")
        })?;
        if FileSnapshot::from_stat(&final_stat) != snapshot || exact_bytes.len() != size {
            return Err("canonical Worker V2 envelope changed while it was read".to_string());
        }
        let envelope = WorkerV2LoadEnvelopeV1::from_bytes(&exact_bytes)
            .map_err(|error| format!("invalid canonical Worker V2 envelope {name}: {error}"))?;
        if envelope.to_bytes() != exact_bytes {
            return Err("Worker V2 envelope encoding is not canonical".to_string());
        }
        if name
            != worker_v2_load_envelope_name_v1(
                envelope.published_claim().receipt().publication_identity(),
            )
        {
            return Err("Worker V2 envelope filename does not bind its publication".to_string());
        }
        file.seek(SeekFrom::Start(0))
            .map_err(|error| format!("failed to rewind canonical Worker V2 envelope: {error}"))?;
        let artifact_directory_file = directory.try_clone_for_transfer()?;
        Ok(Self {
            directory,
            name,
            file,
            snapshot,
            exact_bytes,
            envelope,
            artifact_directory_file,
            current_lease: None,
        })
    }

    fn retain_current_lease(mut self) -> Result<Self, String> {
        let lease = reacquire_current_hsaco_publication_lease_v1(
            &self.directory.child_path(),
            self.envelope.published_claim(),
        )
        .map_err(|error| format!("{}: {error}", self.name))?;
        self.current_lease = Some(lease);
        Ok(self)
    }

    pub(crate) fn validate_retained_currentness(&self) -> Result<(), String> {
        let lease = self.current_lease.as_ref().ok_or_else(|| {
            "application envelope has no retained current-publication lease".to_string()
        })?;
        let token = lease.acquire_current_token().map_err(|error| {
            format!("retained application publication is no longer current: {error}")
        })?;
        drop(token);
        Ok(())
    }

    fn revalidate(&mut self) -> Result<(), String> {
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|error| format!("failed to rewind inherited envelope: {error}"))?;
        let mut bytes = Vec::with_capacity(self.exact_bytes.len().saturating_add(1));
        Read::by_ref(&mut self.file)
            .take((MAX_WORKER_V2_LOAD_ENVELOPE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("failed to re-read inherited envelope: {error}"))?;
        let stat = fstat(&self.file)
            .map_err(|error| format!("failed to re-inspect inherited envelope: {error}"))?;
        validate_envelope_stat(self.directory, &self.name, &stat)?;
        if FileSnapshot::from_stat(&stat) != self.snapshot || bytes != self.exact_bytes {
            return Err("inherited Worker V2 envelope changed after validation".to_string());
        }
        let decoded = WorkerV2LoadEnvelopeV1::from_bytes(&bytes)
            .map_err(|error| format!("inherited Worker V2 envelope is no longer valid: {error}"))?;
        if decoded != self.envelope || decoded.to_bytes() != bytes {
            return Err("inherited Worker V2 envelope identity changed".to_string());
        }
        self.validate_retained_currentness()?;
        self.directory
            .validate_path("Cargo application artifact directory")?;
        self.file
            .seek(SeekFrom::Start(0))
            .map(|_| ())
            .map_err(|error| format!("failed to rewind inherited envelope for child: {error}"))
    }

    pub(crate) fn configure_child(
        &mut self,
        command: &mut Command,
        application: WorkerV2ApplicationIdentityV1,
    ) -> Result<PendingApplicationAck, String> {
        self.revalidate()?;
        let expectation = WorkerV2ApplicationHandoffExpectationV1::new(&self.envelope, application);
        let challenge = random_challenge()?;
        ensure_child_subreaper()?;
        let (ack_read, ack_write) = cloexec_pipe()?;
        let envelope_fd = self.file.as_raw_fd();
        let artifact_directory_fd = self.artifact_directory_file.as_raw_fd();
        let ack_fd = ack_write.as_raw_fd();
        let expected = self.snapshot;
        command
            .env(
                WORKER_V2_APPLICATION_ENVELOPE_FD_ENV_V1,
                envelope_fd.to_string(),
            )
            .env(
                WORKER_V2_APPLICATION_ARTIFACT_DIR_FD_ENV_V1,
                artifact_directory_fd.to_string(),
            )
            .env(
                WORKER_V2_APPLICATION_HANDOFF_COMMITMENT_ENV_V1,
                expectation.commitment().to_hex(),
            )
            .env(
                WORKER_V2_APPLICATION_HANDOFF_ACK_FD_ENV_V1,
                ack_fd.to_string(),
            )
            .env(
                WORKER_V2_APPLICATION_HANDOFF_CHALLENGE_ENV_V1,
                challenge.to_hex(),
            );
        let directory_stat = fstat(&self.artifact_directory_file)
            .map_err(|error| format!("failed to inspect inherited artifact directory: {error}"))?;
        let directory_device = directory_stat.st_dev;
        let directory_inode = directory_stat.st_ino;
        let seccomp_filter = no_fork_application_filter();
        let sandbox = PendingApplicationSandbox::start()?;
        let supervisor_socket = sandbox.child_socket_fd();
        // SAFETY: all three owning `File`s remain alive through spawn. The callback validates the
        // exact evidence and ACK descriptors before clearing only their child-side CLOEXEC flags.
        unsafe {
            command.pre_exec(move || {
                establish_fresh_application_session()?;
                if libc::syscall(
                    libc::SYS_close_range,
                    3_u32,
                    u32::MAX,
                    libc::CLOSE_RANGE_CLOEXEC,
                ) != 0
                {
                    return Err(io::Error::last_os_error());
                }
                let descriptor = BorrowedFd::borrow_raw(envelope_fd);
                let flags = rustix::io::fcntl_getfd(descriptor).map_err(io::Error::from)?;
                let status = rustix::fs::fcntl_getfl(descriptor).map_err(io::Error::from)?;
                let stat = fstat(descriptor).map_err(io::Error::from)?;
                if !flags.contains(rustix::io::FdFlags::CLOEXEC)
                    || status & OFlags::ACCMODE != OFlags::RDONLY
                    || FileSnapshot::from_stat(&stat) != expected
                {
                    return Err(io::Error::from_raw_os_error(libc::ESTALE));
                }
                let directory = BorrowedFd::borrow_raw(artifact_directory_fd);
                let directory_flags =
                    rustix::io::fcntl_getfd(directory).map_err(io::Error::from)?;
                let directory_status =
                    rustix::fs::fcntl_getfl(directory).map_err(io::Error::from)?;
                let current_directory = fstat(directory).map_err(io::Error::from)?;
                if !directory_flags.contains(rustix::io::FdFlags::CLOEXEC)
                    || directory_status & OFlags::ACCMODE != OFlags::RDONLY
                    || FileType::from_raw_mode(current_directory.st_mode) != FileType::Directory
                    || current_directory.st_dev != directory_device
                    || current_directory.st_ino != directory_inode
                {
                    return Err(io::Error::from_raw_os_error(libc::ESTALE));
                }
                let ack = BorrowedFd::borrow_raw(ack_fd);
                let ack_flags = rustix::io::fcntl_getfd(ack).map_err(io::Error::from)?;
                let ack_status = rustix::fs::fcntl_getfl(ack).map_err(io::Error::from)?;
                if !ack_flags.contains(rustix::io::FdFlags::CLOEXEC)
                    || ack_status & OFlags::ACCMODE != OFlags::WRONLY
                {
                    return Err(io::Error::from_raw_os_error(libc::ESTALE));
                }
                for inherited in [descriptor, directory, ack] {
                    rustix::io::fcntl_setfd(inherited, rustix::io::FdFlags::empty())
                        .map_err(io::Error::from)?;
                }
                install_application_profile(&seccomp_filter, supervisor_socket)?;
                Ok(())
            });
        }
        Ok(PendingApplicationAck {
            read: ack_read,
            parent_write: Some(ack_write),
            expectation,
            challenge,
            sandbox: Some(sandbox),
        })
    }
}

pub(crate) fn terminate_application_group(child: &mut Child) -> Result<(), String> {
    let process_group = child.id() as libc::pid_t;
    let mut failures = Vec::new();
    let group_killed = match kill_process_group(process_group) {
        Ok(()) => true,
        Err(error) => {
            failures.push(error);
            false
        }
    };
    match child.kill() {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::InvalidInput => {}
        Err(error) => failures.push(format!("failed to kill application leader: {error}")),
    }
    if let Err(error) = child.wait() {
        failures.push(format!("failed to reap application leader: {error}"));
    }
    if group_killed && let Err(error) = reap_process_group(process_group) {
        failures.push(error);
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

pub(crate) fn wait_and_contain_application_group(child: &mut Child) -> Result<ExitStatus, String> {
    let process_group = child.id() as libc::pid_t;
    wait_for_leader_exit_without_reaping(process_group)?;

    let mut failures = Vec::new();
    if let Err(error) = kill_process_group(process_group) {
        failures.push(error);
    }
    let status = match child.wait() {
        Ok(status) => Some(status),
        Err(error) => {
            failures.push(format!("failed to reap pinned Cargo application: {error}"));
            None
        }
    };
    if let Err(error) = reap_process_group(process_group) {
        failures.push(error);
    }
    if failures.is_empty() {
        Ok(status.expect("successful child wait produced an exit status"))
    } else {
        Err(failures.join("; "))
    }
}

pub(crate) struct PendingApplicationAck {
    read: File,
    parent_write: Option<File>,
    expectation: WorkerV2ApplicationHandoffExpectationV1,
    challenge: WorkerV2ApplicationHandoffChallengeV1,
    sandbox: Option<PendingApplicationSandbox>,
}

impl PendingApplicationAck {
    pub(crate) fn await_after_spawn(
        mut self,
        child: &mut Child,
    ) -> Result<ApplicationSandboxGuard, String> {
        drop(self.parent_write.take());
        let sandbox = self
            .sandbox
            .take()
            .expect("pending acknowledgment owns its sandbox")
            .complete(child.id())?;
        let deadline = Instant::now() + ACK_TIMEOUT;
        let mut bytes = Vec::with_capacity(WORKER_V2_APPLICATION_HANDOFF_ACK_BYTES_V1 + 1);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("application handoff acknowledgment timed out".to_string());
            }
            poll_readable(self.read.as_raw_fd(), remaining)?;
            let mut chunk = [0_u8; 256];
            match self.read.read(&mut chunk) {
                Ok(0) => break,
                Ok(read) => {
                    bytes.extend_from_slice(&chunk[..read]);
                    if bytes.len() > WORKER_V2_APPLICATION_HANDOFF_ACK_BYTES_V1 {
                        return Err(
                            "application handoff acknowledgment has extra bytes".to_string()
                        );
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => {
                    return Err(format!(
                        "failed to read application handoff acknowledgment: {error}"
                    ));
                }
            }
            if child
                .try_wait()
                .map_err(|error| format!("failed to inspect application during handoff: {error}"))?
                .is_some()
                && bytes.is_empty()
            {
                continue;
            }
        }
        let ack = WorkerV2ApplicationHandoffAckV1::decode_canonical(&bytes)
            .map_err(|error| format!("invalid application handoff acknowledgment: {error}"))?;
        ack.validate(self.expectation, self.challenge)
            .map_err(|error| format!("rejected application handoff acknowledgment: {error}"))?;
        Ok(sandbox)
    }
}

fn cloexec_pipe() -> Result<(File, File), String> {
    let mut descriptors = [-1_i32; 2];
    // SAFETY: `descriptors` points to two writable integers and successful `pipe2` initializes both.
    if unsafe { libc::pipe2(descriptors.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK) } != 0 {
        return Err(format!(
            "failed to create application acknowledgment pipe: {}",
            io::Error::last_os_error()
        ));
    }
    // SAFETY: successful `pipe2` returned two newly owned descriptors.
    Ok(unsafe {
        (
            File::from_raw_fd(descriptors[0]),
            File::from_raw_fd(descriptors[1]),
        )
    })
}

fn random_challenge() -> Result<WorkerV2ApplicationHandoffChallengeV1, String> {
    let mut bytes = [0_u8; 32];
    let mut offset = 0;
    while offset != bytes.len() {
        // SAFETY: the remaining byte slice is valid writable storage for `getrandom`.
        let read = unsafe {
            libc::getrandom(bytes[offset..].as_mut_ptr().cast(), bytes.len() - offset, 0)
        };
        if read > 0 {
            offset += read as usize;
            continue;
        }
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::Interrupted {
            continue;
        }
        return Err(format!(
            "failed to generate application handoff challenge: {error}"
        ));
    }
    WorkerV2ApplicationHandoffChallengeV1::from_bytes(bytes)
        .map_err(|error| format!("invalid application handoff challenge: {error}"))
}

fn poll_readable(descriptor: RawFd, timeout: Duration) -> Result<(), String> {
    let millis = timeout.as_millis().min(i32::MAX as u128) as i32;
    let mut pollfd = libc::pollfd {
        fd: descriptor,
        events: libc::POLLIN | libc::POLLHUP | libc::POLLERR,
        revents: 0,
    };
    loop {
        // SAFETY: `pollfd` is one valid poll descriptor record for the duration of the call.
        let result = unsafe { libc::poll(&mut pollfd, 1, millis) };
        if result > 0 {
            return Ok(());
        }
        if result == 0 {
            return Err("application handoff acknowledgment timed out".to_string());
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(format!(
                "failed to wait for application handoff acknowledgment: {error}"
            ));
        }
    }
}

fn ensure_child_subreaper() -> Result<(), String> {
    // SAFETY: `prctl` receives the documented scalar argument for this process attribute.
    if unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) } != 0 {
        return Err(format!(
            "failed to make the application runner a child subreaper: {}",
            io::Error::last_os_error()
        ));
    }
    Ok(())
}

fn establish_fresh_application_session() -> io::Result<()> {
    // SAFETY: this runs in the single-threaded post-fork child before exec. A successful `setsid`
    // makes the child both session and process-group leader, so processes from the runner's session
    // cannot join the group later. Any failure is returned through `Command::spawn`.
    let process = unsafe { libc::getpid() };
    let session = unsafe { libc::setsid() };
    if session < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: these scalar identity queries have no memory preconditions.
    let current_session = unsafe { libc::getsid(0) };
    let process_group = unsafe { libc::getpgrp() };
    if session != process || current_session != process || process_group != process {
        return Err(io::Error::from_raw_os_error(libc::ESTALE));
    }
    Ok(())
}

fn kill_process_group(process_group: libc::pid_t) -> Result<(), String> {
    // SAFETY: a negative PID addresses the application process group in its dedicated session. A
    // live, unreaped session leader prevents its PID and process-group identity from being reused.
    if unsafe { libc::kill(-process_group, libc::SIGKILL) } == 0 {
        return Ok(());
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(format!("failed to kill application process group: {error}"))
    }
}

fn wait_for_leader_exit_without_reaping(leader: libc::pid_t) -> Result<(), String> {
    loop {
        let mut information = MaybeUninit::<libc::siginfo_t>::zeroed();
        // SAFETY: `information` is writable, P_PID selects the owned child, and WNOWAIT leaves
        // the exited leader waitable so its PID/PGID cannot be recycled before group signaling.
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                leader as libc::id_t,
                information.as_mut_ptr(),
                libc::WEXITED | libc::WNOWAIT,
            )
        };
        if result == 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::Interrupted {
            continue;
        }
        return Err(format!(
            "failed to observe application leader exit without reaping it: {error}"
        ));
    }
}

fn reap_process_group(process_group: libc::pid_t) -> Result<(), String> {
    loop {
        let mut status = 0;
        // SAFETY: `status` is writable and the negative PID selects children in the application
        // process group. Subreaper ownership makes orphaned descendants waitable by this runner.
        let result = unsafe { libc::waitpid(-process_group, &mut status, 0) };
        if result > 0 {
            continue;
        }
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::Interrupted {
            continue;
        }
        if error.raw_os_error() == Some(libc::ECHILD) {
            return Ok(());
        }
        return Err(format!(
            "failed to reap application process-group descendants: {error}"
        ));
    }
}

fn envelope_names(directory: &PinnedDirectory) -> Result<Vec<String>, String> {
    let scan = directory.try_clone_for_transfer()?;
    let mut entries = rustix::fs::Dir::read_from(&scan)
        .map_err(|error| format!("failed to scan artifact directory: {error}"))?;
    let mut names = Vec::new();
    for entry in &mut entries {
        let entry = entry.map_err(|error| format!("failed to read artifact entry: {error}"))?;
        let bytes = entry.file_name().to_bytes();
        if !bytes.starts_with(ENVELOPE_PREFIX) {
            continue;
        }
        if !is_canonical_envelope_name(bytes) {
            return Err("malformed Worker V2 envelope publication name".to_string());
        }
        if names.len() == MAX_ENVELOPE_CANDIDATES {
            return Err(format!(
                "artifact directory exceeds {MAX_ENVELOPE_CANDIDATES} canonical envelope candidates"
            ));
        }
        names.push(
            std::str::from_utf8(bytes)
                .expect("canonical envelope names are ASCII")
                .to_string(),
        );
    }
    names.sort_unstable();
    Ok(names)
}

fn is_canonical_envelope_name(bytes: &[u8]) -> bool {
    bytes.len() == ENVELOPE_NAME_BYTES
        && bytes.starts_with(ENVELOPE_PREFIX)
        && bytes.ends_with(ENVELOPE_SUFFIX)
        && bytes[ENVELOPE_PREFIX.len()..ENVELOPE_PREFIX.len() + 64]
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn validate_envelope_stat(
    directory: &PinnedDirectory,
    name: &str,
    opened: &rustix::fs::Stat,
) -> Result<(), String> {
    let linked = statat(directory.file(), name, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|error| format!("failed to inspect linked Worker V2 envelope {name}: {error}"))?;
    if FileType::from_raw_mode(opened.st_mode) != FileType::RegularFile
        || opened.st_dev != linked.st_dev
        || opened.st_ino != linked.st_ino
        || opened.st_nlink != 1
        || opened.st_uid != unsafe { libc::geteuid() }
        || opened.st_mode & 0o077 != 0
    {
        return Err(format!("refusing unsafe Worker V2 envelope {name}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;

    fn fresh_session_command(program: &str) -> Command {
        let mut command = Command::new(program);
        // SAFETY: the callback performs only child-side session syscalls before exec.
        unsafe {
            command.pre_exec(establish_fresh_application_session);
        }
        command
    }

    fn wait_for_raw_child(child: libc::pid_t) {
        let mut status = 0;
        loop {
            // SAFETY: `status` is writable and `child` was returned by `fork` in this process.
            let result = unsafe { libc::waitpid(child, &mut status, 0) };
            if result == child {
                return;
            }
            assert_eq!(result, -1);
            assert_eq!(
                io::Error::last_os_error().kind(),
                io::ErrorKind::Interrupted
            );
        }
    }

    fn process_exists(process: libc::pid_t) -> bool {
        // SAFETY: signal zero performs an existence/permission check without delivering a signal.
        unsafe { libc::kill(process, 0) == 0 }
    }

    #[test]
    fn session_setup_fails_closed_for_a_process_group_leader() {
        let mut command = Command::new("/bin/true");
        // SAFETY: the callback deliberately creates the forbidden precondition, then verifies that
        // the production setup returns EPERM through `spawn` instead of launching without isolation.
        unsafe {
            command.pre_exec(|| {
                if libc::setpgid(0, 0) != 0 {
                    return Err(io::Error::last_os_error());
                }
                establish_fresh_application_session()
            });
        }
        let error = command.spawn().unwrap_err();
        assert_eq!(error.raw_os_error(), Some(libc::EPERM));
    }

    #[test]
    fn unrelated_same_session_process_cannot_join_or_be_killed() {
        let mut application = fresh_session_command("/bin/sleep");
        application.arg("30");
        let mut application = application.spawn().unwrap();
        let leader = application.id() as libc::pid_t;
        // SAFETY: the spawned child is live and its pre-exec callback completed before `spawn`.
        assert_eq!(unsafe { libc::getsid(leader) }, leader);
        assert_eq!(unsafe { libc::getpgid(leader) }, leader);

        let mut report = [-1_i32; 2];
        // SAFETY: `report` points to two writable descriptor slots.
        assert_eq!(
            unsafe { libc::pipe2(report.as_mut_ptr(), libc::O_CLOEXEC) },
            0
        );
        // SAFETY: the child branch uses only async-signal-safe syscalls and never returns to Rust.
        let outsider = unsafe { libc::fork() };
        assert!(
            outsider >= 0,
            "fork outsider: {}",
            io::Error::last_os_error()
        );
        if outsider == 0 {
            unsafe {
                libc::close(report[0]);
                let joined = libc::setpgid(0, leader);
                let error = *libc::__errno_location();
                let values = [joined, error];
                let _ = libc::write(
                    report[1],
                    values.as_ptr().cast(),
                    std::mem::size_of_val(&values),
                );
                loop {
                    libc::pause();
                }
            }
        }

        // SAFETY: each branch owns the descriptor it closes; the remaining read end becomes `File`.
        unsafe { libc::close(report[1]) };
        let mut report = unsafe { File::from_raw_fd(report[0]) };
        let mut bytes = [0_u8; std::mem::size_of::<[i32; 2]>()];
        report.read_exact(&mut bytes).unwrap();
        let joined = i32::from_ne_bytes(bytes[..4].try_into().unwrap());
        let join_error = i32::from_ne_bytes(bytes[4..].try_into().unwrap());
        // SAFETY: `outsider` remains live and inherited the test runner's session.
        let outsider_session = unsafe { libc::getsid(outsider) };
        let runner_session = unsafe { libc::getsid(0) };

        let containment = terminate_application_group(&mut application);
        let outsider_survived = process_exists(outsider);
        // SAFETY: the outsider is this test's child and is intentionally paused until cleanup.
        unsafe { libc::kill(outsider, libc::SIGKILL) };
        wait_for_raw_child(outsider);

        assert_eq!(joined, -1);
        assert_eq!(join_error, libc::EPERM);
        assert_eq!(outsider_session, runner_session);
        assert_ne!(outsider_session, leader);
        assert!(containment.is_ok(), "{containment:?}");
        assert!(
            outsider_survived,
            "application containment killed the outsider"
        );
        assert_eq!(application.wait().unwrap().signal(), Some(libc::SIGKILL));
    }

    #[test]
    fn exit_observation_keeps_process_group_leader_unreaped() {
        let mut command = fresh_session_command("/bin/true");
        let mut child = command.spawn().unwrap();
        let leader = child.id() as libc::pid_t;

        wait_for_leader_exit_without_reaping(leader).unwrap();
        // SAFETY: signal zero only checks the dedicated group while its zombie leader is retained.
        assert_eq!(unsafe { libc::kill(-leader, 0) }, 0);
        kill_process_group(leader).unwrap();
        assert!(child.wait().unwrap().success());
        reap_process_group(leader).unwrap();
    }

    #[test]
    fn successful_wait_contains_before_reaping_leader() {
        for _ in 0..32 {
            let mut command = fresh_session_command("/bin/true");
            let mut child = command.spawn().unwrap();
            assert!(
                wait_and_contain_application_group(&mut child)
                    .unwrap()
                    .success()
            );
        }
    }

    #[test]
    fn successful_leader_exit_still_contains_session_descendants() {
        ensure_child_subreaper().unwrap();
        let pid_file = std::env::temp_dir().join(format!(
            "cargo-fe2o3-session-descendant-{}",
            std::process::id()
        ));
        let mut command = fresh_session_command("/bin/sh");
        command
            .arg("-c")
            .arg("sleep 30 & echo $! > \"$1\"")
            .arg("fe2o3-descendant-probe")
            .arg(&pid_file);
        let mut child = command.spawn().unwrap();
        let status = wait_and_contain_application_group(&mut child).unwrap();
        let descendant = std::fs::read_to_string(&pid_file)
            .unwrap()
            .trim()
            .parse::<libc::pid_t>()
            .unwrap();
        let _ = std::fs::remove_file(pid_file);

        assert!(status.success());
        assert!(
            !process_exists(descendant),
            "session descendant survived cleanup"
        );
    }
}
