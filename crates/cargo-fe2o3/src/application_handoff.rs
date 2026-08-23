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
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use fe2o3_artifact_transaction::{
    DurableCurrentLinkPublicationLeaseV1, reacquire_current_hsaco_publication_lease_v1,
    reacquire_current_hsaco_publication_lease_v3,
};
use fe2o3_worker_v2_bundle::{
    MAX_WORKER_V2_ARTIFACT_DIRECTORY_ENTRIES_V1, MAX_WORKER_V2_LOAD_ENVELOPE_BYTES,
    MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V1, WORKER_V2_APPLICATION_ARTIFACT_DIR_FD_ENV_V1,
    WORKER_V2_APPLICATION_ENVELOPE_FD_ENV_V1, WORKER_V2_APPLICATION_HANDOFF_ACK_BYTES_V1,
    WORKER_V2_APPLICATION_HANDOFF_ACK_FD_ENV_V1, WORKER_V2_APPLICATION_HANDOFF_CHALLENGE_ENV_V1,
    WORKER_V2_APPLICATION_HANDOFF_COMMITMENT_ENV_V1, WORKER_V2_LOAD_ENVELOPE_NAME_PREFIX_V1,
    WORKER_V2_LOAD_ENVELOPE_NAME_SUFFIX_V1, WORKER_V3_APPLICATION_ARTIFACT_DIR_FD_ENV_V1,
    WORKER_V3_APPLICATION_ENVELOPE_FD_ENV_V1, WORKER_V3_APPLICATION_HANDOFF_ACK_BYTES_V1,
    WORKER_V3_APPLICATION_HANDOFF_ACK_FD_ENV_V1, WORKER_V3_APPLICATION_HANDOFF_CHALLENGE_ENV_V1,
    WORKER_V3_APPLICATION_HANDOFF_COMMITMENT_ENV_V1, WORKER_V3_APPLICATION_OCCURRENCE_ENV_V1,
    WorkerV2ApplicationHandoffAckV1, WorkerV2ApplicationHandoffChallengeV1,
    WorkerV2ApplicationHandoffExpectationV1, WorkerV2ApplicationIdentityV1, WorkerV2LoadEnvelopeV1,
    WorkerV3ApplicationHandoffAckV1, WorkerV3ApplicationHandoffChallengeV1,
    WorkerV3ApplicationHandoffExpectationV1, WorkerV3ApplicationIdentityV1,
    WorkerV3ApplicationInputOccurrenceV1, WorkerV3ApplicationOccurrenceV1,
    WorkerV3LoadEnvelopeIdentityV1, WorkerV3LoadEnvelopeWireV1, worker_v2_load_envelope_name_v1,
};
use rustix::fs::{AtFlags, FileType, Mode, OFlags, ResolveFlags, fstat, openat2, statat};

use crate::application_sandbox::{
    ApplicationSandboxGuard, PendingApplicationSandbox, install_application_profile,
    no_fork_application_filter,
};
use crate::generation;
use crate::project::{PinnedDirectory, is_synthetic_dot_entry};

pub(crate) const RUNNER_CONTEXT_VERSION: &str = "3";
#[cfg(feature = "worker-v2-fault-injection-test-only")]
pub(crate) const RUNNER_SHORT_TIMEOUT_TEST_CONTEXT_VERSION: &str = "3-test-short-timeouts";
#[cfg(feature = "worker-v2-fault-injection-test-only")]
pub(crate) const RUNNER_SCHEDULER_TOLERANT_TEST_CONTEXT_VERSION: &str = "3-test-scheduler-tolerant";
pub(crate) const RUNNER_EXPECTS_ENVELOPE: &str = "required";
pub(crate) const RUNNER_EXPECTS_NO_ENVELOPE: &str = "none";

const ENVELOPE_PREFIX: &[u8] = WORKER_V2_LOAD_ENVELOPE_NAME_PREFIX_V1.as_bytes();
const ENVELOPE_SUFFIX: &[u8] = WORKER_V2_LOAD_ENVELOPE_NAME_SUFFIX_V1.as_bytes();
const ENVELOPE_NAME_BYTES: usize = ENVELOPE_PREFIX.len() + 64 + ENVELOPE_SUFFIX.len();
const V3_ENVELOPE_PREFIX: &[u8] = b".fe2o3-worker-v3-load-readiness-v1-";
const V3_ENVELOPE_SUFFIX: &[u8] = b".envelope";
const V3_ENVELOPE_NAME_BYTES: usize = V3_ENVELOPE_PREFIX.len() + 64 + V3_ENVELOPE_SUFFIX.len();
const MAX_ENVELOPE_CANDIDATES: usize = 256;
const MAX_PENDING_APPLICATION_REAPS: usize = 8;
const REAPER_POLL_INTERVAL: Duration = Duration::from_millis(10);
// V3 performs durable recovery and semantic admission before acknowledging the transfer.
const WORKER_V3_PRODUCTION_ACK_TIMEOUT: Duration = Duration::from_secs(30);
#[cfg(any(test, feature = "worker-v2-fault-injection-test-only"))]
const TEST_ACK_READY_FD_ENV: &str = "FE2O3_INTERNAL_TEST_ACK_READY_FD";
#[cfg(any(test, feature = "worker-v2-fault-injection-test-only"))]
const TEST_ACK_STARTUP_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ApplicationTimeouts {
    ack: Duration,
    cleanup: Duration,
    #[cfg(any(test, feature = "worker-v2-fault-injection-test-only"))]
    wait_for_test_ready: bool,
}

impl ApplicationTimeouts {
    pub(crate) const PRODUCTION: Self = Self {
        ack: Duration::from_secs(5),
        cleanup: Duration::from_secs(2),
        #[cfg(any(test, feature = "worker-v2-fault-injection-test-only"))]
        wait_for_test_ready: false,
    };

    fn for_worker_v3(mut self) -> Self {
        if self == Self::PRODUCTION {
            self.ack = WORKER_V3_PRODUCTION_ACK_TIMEOUT;
        }
        self
    }

    #[cfg(any(test, feature = "worker-v2-fault-injection-test-only"))]
    pub(crate) const TEST_SHORT: Self = Self {
        ack: Duration::from_secs(2),
        cleanup: Duration::from_millis(500),
        wait_for_test_ready: true,
    };

    #[cfg(any(test, feature = "worker-v2-fault-injection-test-only"))]
    pub(crate) const TEST_SCHEDULER_TOLERANT: Self = Self {
        ack: Duration::from_secs(30),
        cleanup: Duration::from_secs(5),
        wait_for_test_ready: false,
    };
}

struct ReaperReservation {
    supervisor: Arc<ReaperSupervisor>,
    released: bool,
}

impl Drop for ReaperReservation {
    fn drop(&mut self) {
        if !self.released {
            self.supervisor.reserved.fetch_sub(1, Ordering::AcqRel);
            self.released = true;
        }
    }
}

struct ReapJob {
    child: Child,
    process_group: libc::pid_t,
    process_group_terminal: bool,
    sandbox: Option<ApplicationSandboxGuard>,
    _reservation: ReaperReservation,
    leader_status: Option<ExitStatus>,
    completion: Option<SyncSender<Result<ExitStatus, String>>>,
    completion_error: Option<String>,
    last_retryable_error: Option<String>,
    #[cfg(test)]
    test_hold: Option<Arc<AtomicBool>>,
    #[cfg(test)]
    test_completed: Option<Arc<AtomicBool>>,
    #[cfg(test)]
    test_retryable_error: Option<Arc<AtomicBool>>,
}

struct ReaperSupervisor {
    capacity: usize,
    reserved: AtomicUsize,
    worker_count: AtomicUsize,
    worker: Mutex<ReaperWorker>,
    jobs: Mutex<Vec<ReapJob>>,
    wake: Condvar,
    shutdown: AtomicBool,
    #[cfg(test)]
    fail_worker_start: AtomicBool,
    #[cfg(test)]
    panic_worker: AtomicBool,
}

#[derive(Clone, Debug)]
enum ReaperWorkerPhase {
    Unstarted,
    Running,
    Dead(String),
    Stopped,
}

struct ReaperWorker {
    phase: ReaperWorkerPhase,
    handle: Option<JoinHandle<()>>,
}

impl ReaperSupervisor {
    fn new(capacity: usize) -> Arc<Self> {
        Arc::new(Self {
            capacity,
            reserved: AtomicUsize::new(0),
            worker_count: AtomicUsize::new(0),
            worker: Mutex::new(ReaperWorker {
                phase: ReaperWorkerPhase::Unstarted,
                handle: None,
            }),
            jobs: Mutex::new(Vec::with_capacity(capacity)),
            wake: Condvar::new(),
            shutdown: AtomicBool::new(false),
            #[cfg(test)]
            fail_worker_start: AtomicBool::new(false),
            #[cfg(test)]
            panic_worker: AtomicBool::new(false),
        })
    }

    fn reserve(self: &Arc<Self>) -> Result<ReaperReservation, String> {
        self.ensure_worker()?;
        let mut current = self.reserved.load(Ordering::Acquire);
        loop {
            if current >= self.capacity {
                return Err(format!(
                    "application cleanup supervisor is saturated at {} pending handoffs",
                    self.capacity
                ));
            }
            match self.reserved.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    let reservation = ReaperReservation {
                        supervisor: Arc::clone(self),
                        released: false,
                    };
                    if let Err(error) = self.ensure_worker() {
                        drop(reservation);
                        return Err(error);
                    }
                    return Ok(reservation);
                }
                Err(observed) => current = observed,
            }
        }
    }

    fn ensure_worker(self: &Arc<Self>) -> Result<(), String> {
        let mut worker = self
            .worker
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match &worker.phase {
            ReaperWorkerPhase::Running
                if worker
                    .handle
                    .as_ref()
                    .is_some_and(|handle| !handle.is_finished()) =>
            {
                return Ok(());
            }
            ReaperWorkerPhase::Running => {
                worker.phase = ReaperWorkerPhase::Dead(
                    "application cleanup worker exited before lifecycle publication caught up"
                        .to_string(),
                );
            }
            ReaperWorkerPhase::Dead(error) => {
                return Err(format!(
                    "application cleanup supervisor is dead and fails closed: {error}"
                ));
            }
            ReaperWorkerPhase::Stopped => {
                return Err("application cleanup supervisor is stopped and fails closed".into());
            }
            ReaperWorkerPhase::Unstarted => {}
        }
        if let ReaperWorkerPhase::Dead(error) = &worker.phase {
            return Err(format!(
                "application cleanup supervisor is dead and fails closed: {error}"
            ));
        }
        #[cfg(test)]
        if self.fail_worker_start.load(Ordering::Acquire) {
            return Err("injected application cleanup worker startup failure".to_string());
        }
        let supervisor = Arc::clone(self);
        let spawned = thread::Builder::new()
            .name("fe2o3-bounded-application-reaper".into())
            .spawn(move || {
                let runner = Arc::clone(&supervisor);
                let result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runner.run()));
                supervisor.worker_exited(result);
            });
        match spawned {
            Ok(handle) => {
                // Publication happens only after spawn returned a live, retained JoinHandle.
                worker.handle = Some(handle);
                worker.phase = ReaperWorkerPhase::Running;
                self.worker_count.fetch_add(1, Ordering::Release);
                Ok(())
            }
            Err(error) => Err(format!(
                "failed to start bounded application cleanup supervisor: {error}"
            )),
        }
    }

    fn transfer(&self, job: ReapJob) {
        let worker = self
            .worker
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut jobs = self
            .jobs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        debug_assert!(jobs.len() < self.capacity);
        jobs.push(job);
        if matches!(
            worker.phase,
            ReaperWorkerPhase::Dead(_) | ReaperWorkerPhase::Stopped
        ) {
            let message = match &worker.phase {
                ReaperWorkerPhase::Dead(error) => error.clone(),
                ReaperWorkerPhase::Stopped => "cleanup worker stopped".to_string(),
                _ => unreachable!(),
            };
            if let Some(completion) = jobs.last_mut().and_then(|job| job.completion.take()) {
                let _ = completion.send(Err(format!(
                    "application cleanup ownership retained after worker failure: {message}"
                )));
            }
        }
        drop(jobs);
        drop(worker);
        self.wake.notify_one();
    }

    fn run(self: Arc<Self>) {
        loop {
            let mut jobs = self
                .jobs
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            while jobs.is_empty() {
                if self.shutdown.load(Ordering::Acquire) {
                    return;
                }
                jobs = self
                    .wake
                    .wait(jobs)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }
            drop(jobs);
            #[cfg(test)]
            if self.panic_worker.swap(false, Ordering::AcqRel) {
                panic!("injected application cleanup worker panic");
            }
            self.poll_jobs_once();
            thread::sleep(REAPER_POLL_INTERVAL);
        }
    }

    fn poll_jobs_once(&self) {
        let mut jobs = self
            .jobs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut completed_jobs = Vec::new();
        let mut index = 0;
        while index < jobs.len() {
            #[cfg(test)]
            if jobs[index]
                .test_hold
                .as_ref()
                .is_some_and(|hold| hold.load(Ordering::Acquire))
            {
                index += 1;
                continue;
            }
            match try_reap_job(&mut jobs[index]) {
                Ok(true) => {
                    completed_jobs.push(jobs.swap_remove(index));
                }
                Ok(false) => index += 1,
                Err(error) => {
                    if jobs[index].last_retryable_error.as_deref() != Some(&error) {
                        eprintln!(
                            "cargo-fe2o3 application cleanup supervisor retained a slot after a retryable error: {error}"
                        );
                        jobs[index].last_retryable_error = Some(error.clone());
                    }
                    if let Some(completion) = jobs[index].completion.take() {
                        let _ = completion.send(Err(format!(
                                "application cleanup remains pending with its fixed-capacity supervisor slot retained after retryable error: {error}"
                            )));
                    }
                    index += 1;
                }
            }
        }
        drop(jobs);
        for mut completed in completed_jobs {
            let result = match (completed.leader_status, completed.completion_error.take()) {
                (_, Some(error)) => Err(error),
                (Some(status), None) => Ok(status),
                (None, None) => {
                    Err("application cleanup completed without retaining leader status".to_string())
                }
            };
            if let Some(completion) = completed.completion.take() {
                let _ = completion.send(result);
            }
            #[cfg(test)]
            if let Some(observed) = completed.test_completed.as_ref() {
                observed.store(true, Ordering::Release);
            }
        }
    }

    fn worker_exited(&self, result: std::thread::Result<()>) {
        let failure = result.err().map(|payload| {
            payload
                .downcast_ref::<&str>()
                .map(|message| (*message).to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown panic payload".to_string())
        });
        let mut worker = self
            .worker
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        worker.phase = if let Some(error) = &failure {
            ReaperWorkerPhase::Dead(format!("cleanup worker panicked: {error}"))
        } else if self.shutdown.load(Ordering::Acquire) {
            ReaperWorkerPhase::Stopped
        } else {
            ReaperWorkerPhase::Dead("cleanup worker exited unexpectedly".to_string())
        };
        drop(worker);
        if let Some(error) = failure {
            let mut jobs = self
                .jobs
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            for job in &mut *jobs {
                if let Some(completion) = job.completion.take() {
                    let _ = completion.send(Err(format!(
                        "application cleanup ownership retained after cleanup worker panic: {error}"
                    )));
                }
            }
        }
        self.wake.notify_all();
    }

    fn has_pending(&self) -> bool {
        self.reserved.load(Ordering::Acquire) != 0
    }

    fn finish_process(self: &Arc<Self>) -> Result<(), String> {
        while self.has_pending() {
            let dead = {
                let worker = self
                    .worker
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                matches!(worker.phase, ReaperWorkerPhase::Dead(_))
            };
            if dead {
                // The dedicated supervisor process itself is the bounded fallback worker. It
                // admits no new jobs after worker death and retains ownership until they finish.
                self.poll_jobs_once();
            }
            thread::sleep(REAPER_POLL_INTERVAL);
        }
        self.shutdown.store(true, Ordering::Release);
        self.wake.notify_all();
        loop {
            let finished = {
                let worker = self
                    .worker
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                worker.handle.as_ref().is_none_or(JoinHandle::is_finished)
            };
            if finished {
                break;
            }
            thread::sleep(REAPER_POLL_INTERVAL);
        }
        let mut worker = self
            .worker
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(handle) = worker.handle.take() {
            handle
                .join()
                .map_err(|_| "application cleanup worker join observed a panic".to_string())?;
        }
        Ok(())
    }

    #[cfg(test)]
    fn stop_for_test(&self) {
        self.shutdown.store(true, Ordering::Release);
        self.wake.notify_all();
    }
}

fn application_reaper() -> &'static Arc<ReaperSupervisor> {
    static REAPER: OnceLock<Arc<ReaperSupervisor>> = OnceLock::new();
    REAPER.get_or_init(|| ReaperSupervisor::new(MAX_PENDING_APPLICATION_REAPS))
}

pub(crate) fn application_cleanup_is_pending() -> bool {
    application_reaper().has_pending()
}

pub(crate) fn finish_application_cleanup_supervisor() -> Result<(), String> {
    application_reaper().finish_process()
}

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
    envelope: ApplicationEnvelopeWireV1,
    artifact_directory_file: File,
    current_lease: Option<DurableCurrentLinkPublicationLeaseV1>,
}

enum ApplicationEnvelopeWireV1 {
    WorkerV2(Box<WorkerV2LoadEnvelopeV1>),
    WorkerV3(Box<WorkerV3LoadEnvelopeWireV1>),
}

impl ApplicationEnvelopeWireV1 {
    const fn schema_name(&self) -> &'static str {
        match self {
            Self::WorkerV2(_) => "Worker V2",
            Self::WorkerV3(_) => "Worker V3",
        }
    }

    fn encode_canonical(&self) -> Result<Vec<u8>, String> {
        match self {
            Self::WorkerV2(envelope) => Ok(envelope.to_bytes()),
            Self::WorkerV3(envelope) => envelope
                .encode_canonical()
                .map_err(|error| format!("failed to encode canonical Worker V3 envelope: {error}")),
        }
    }
}

impl<'directory> PinnedApplicationEnvelope<'directory> {
    pub(crate) fn discover(directory: &'directory PinnedDirectory) -> Result<Option<Self>, String> {
        directory.validate_path("Cargo application artifact directory")?;
        let names = envelope_names(directory)?;
        if names.is_empty() {
            return Ok(None);
        }

        validate_single_envelope_schema(&names)?;

        let mut current = None;
        let mut rejected = Vec::new();
        for name in names {
            let candidate = Self::open(directory, name)?;
            match candidate.retain_current_lease() {
                Ok(candidate) if current.is_none() => current = Some(candidate),
                Ok(_) => {
                    return Err(
                        "multiple canonical application envelopes claim the current publication"
                            .to_string(),
                    );
                }
                Err(error) => rejected.push(error),
            }
        }
        directory.validate_path("Cargo application artifact directory")?;
        current.map(Some).ok_or_else(|| {
            format!(
                "canonical application envelopes exist but none is current: {}",
                rejected
                    .first()
                    .map(String::as_str)
                    .unwrap_or("no candidate admitted")
            )
        })
    }

    fn open(directory: &'directory PinnedDirectory, name: String) -> Result<Self, String> {
        let schema = if is_canonical_v2_envelope_name(name.as_bytes()) {
            "Worker V2"
        } else if is_canonical_v3_envelope_name(name.as_bytes()) {
            "Worker V3"
        } else {
            return Err("application envelope name has no admitted schema".to_string());
        };
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
        .map_err(|error| format!("failed to open canonical {schema} envelope {name}: {error}"))?;
        let flags = rustix::io::fcntl_getfd(&descriptor)
            .map_err(|error| format!("failed to inspect envelope descriptor flags: {error}"))?;
        let status = rustix::fs::fcntl_getfl(&descriptor)
            .map_err(|error| format!("failed to inspect envelope access mode: {error}"))?;
        if !flags.contains(rustix::io::FdFlags::CLOEXEC)
            || status & OFlags::ACCMODE != OFlags::RDONLY
        {
            return Err(format!(
                "canonical {schema} envelope descriptor is not read-only CLOEXEC"
            ));
        }
        let initial = fstat(&descriptor)
            .map_err(|error| format!("failed to inspect canonical {schema} envelope: {error}"))?;
        validate_envelope_stat(directory, &name, &initial)?;
        let snapshot = FileSnapshot::from_stat(&initial);
        let size = usize::try_from(initial.st_size).map_err(|_| {
            format!("canonical {schema} envelope has a negative or unrepresentable size")
        })?;
        let maximum = if schema == "Worker V2" {
            MAX_WORKER_V2_LOAD_ENVELOPE_BYTES
        } else {
            MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V1
        };
        if size == 0 || size > maximum {
            return Err(format!(
                "canonical {schema} envelope size {size} is outside 1..={maximum}"
            ));
        }
        let mut file = File::from(descriptor);
        let mut exact_bytes = Vec::with_capacity(size.saturating_add(1));
        Read::by_ref(&mut file)
            .take((maximum + 1) as u64)
            .read_to_end(&mut exact_bytes)
            .map_err(|error| format!("failed to read canonical Worker V2 envelope: {error}"))?;
        let final_stat = fstat(&file).map_err(|error| {
            format!("failed to re-inspect canonical Worker V2 envelope: {error}")
        })?;
        if FileSnapshot::from_stat(&final_stat) != snapshot || exact_bytes.len() != size {
            return Err(format!(
                "canonical {schema} envelope changed while it was read"
            ));
        }
        let envelope = if schema == "Worker V2" {
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
            ApplicationEnvelopeWireV1::WorkerV2(Box::new(envelope))
        } else {
            let envelope = WorkerV3LoadEnvelopeWireV1::decode_canonical(&exact_bytes)
                .map_err(|error| format!("invalid canonical Worker V3 envelope {name}: {error}"))?;
            if envelope
                .encode_canonical()
                .map_err(|error| format!("failed to re-encode Worker V3 envelope: {error}"))?
                != exact_bytes
            {
                return Err("Worker V3 envelope encoding is not canonical".to_string());
            }
            ApplicationEnvelopeWireV1::WorkerV3(Box::new(envelope))
        };
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
        let lease = match &self.envelope {
            ApplicationEnvelopeWireV1::WorkerV2(envelope) => {
                reacquire_current_hsaco_publication_lease_v1(
                    &self.directory.child_path(),
                    envelope.published_claim(),
                )
                .map_err(|error| format!("{}: {error}", self.name))?
            }
            ApplicationEnvelopeWireV1::WorkerV3(envelope) => {
                reacquire_current_hsaco_publication_lease_v3(
                    &self.directory.child_path(),
                    envelope.published_claim(),
                )
                .map_err(|error| format!("{}: {error}", self.name))?
            }
        };
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
            .take((self.exact_bytes.len() + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("failed to re-read inherited envelope: {error}"))?;
        let stat = fstat(&self.file)
            .map_err(|error| format!("failed to re-inspect inherited envelope: {error}"))?;
        validate_envelope_stat(self.directory, &self.name, &stat)?;
        if FileSnapshot::from_stat(&stat) != self.snapshot || bytes != self.exact_bytes {
            return Err(format!(
                "inherited {} envelope changed after validation",
                self.envelope.schema_name()
            ));
        }
        if self.envelope.encode_canonical()? != bytes {
            return Err(format!(
                "inherited {} envelope identity changed",
                self.envelope.schema_name()
            ));
        }
        self.validate_retained_currentness()?;
        self.directory
            .validate_path("Cargo application artifact directory")?;
        self.file
            .seek(SeekFrom::Start(0))
            .map(|_| ())
            .map_err(|error| format!("failed to rewind inherited envelope for child: {error}"))
    }

    pub(crate) fn configure_child_with_timeouts(
        &mut self,
        command: &mut Command,
        application_v2: WorkerV2ApplicationIdentityV1,
        application_v3: WorkerV3ApplicationIdentityV1,
        timeouts: ApplicationTimeouts,
    ) -> Result<PendingApplicationAck, String> {
        self.revalidate()?;
        let reaper = application_reaper().reserve()?;
        ensure_child_subreaper()?;
        let (ack_read, ack_write) = cloexec_pipe()?;
        #[cfg(any(test, feature = "worker-v2-fault-injection-test-only"))]
        let (test_ready_read, test_ready_write) = if timeouts.wait_for_test_ready {
            let (read, write) = cloexec_pipe()?;
            command.env(TEST_ACK_READY_FD_ENV, write.as_raw_fd().to_string());
            (Some(read), Some(write))
        } else {
            command.env_remove(TEST_ACK_READY_FD_ENV);
            (None, None)
        };
        let envelope_fd = self.file.as_raw_fd();
        let artifact_directory_fd = self.artifact_directory_file.as_raw_fd();
        let ack_fd = ack_write.as_raw_fd();
        #[cfg(any(test, feature = "worker-v2-fault-injection-test-only"))]
        let test_ready_fd = test_ready_write.as_ref().map(AsRawFd::as_raw_fd);
        let expected = self.snapshot;
        let directory_stat = fstat(&self.artifact_directory_file)
            .map_err(|error| format!("failed to inspect inherited artifact directory: {error}"))?;
        let ack_stat = fstat(&ack_write)
            .map_err(|error| format!("failed to inspect application acknowledgment: {error}"))?;
        let protocol = match &self.envelope {
            ApplicationEnvelopeWireV1::WorkerV2(envelope) => {
                let expectation =
                    WorkerV2ApplicationHandoffExpectationV1::new(envelope, application_v2);
                let challenge = random_v2_challenge()?;
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
                ApplicationHandoffExpectationV1::WorkerV2 {
                    expectation,
                    challenge,
                }
            }
            ApplicationEnvelopeWireV1::WorkerV3(_) => {
                let envelope = WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(&self.exact_bytes)
                    .map_err(|error| format!("failed to identify exact V3 envelope: {error}"))?;
                let envelope_stat = fstat(&self.file)
                    .map_err(|error| format!("failed to inspect inherited V3 envelope: {error}"))?;
                let inputs = [
                    descriptor_occurrence(1, &envelope_stat)?,
                    descriptor_occurrence(2, &directory_stat)?,
                    descriptor_occurrence(3, &ack_stat)?,
                ];
                let occurrence = WorkerV3ApplicationOccurrenceV1::new(
                    application_v3,
                    random_identity_bytes()?,
                    &inputs,
                )
                .map_err(|error| format!("failed to bind V3 application occurrence: {error}"))?;
                let expectation =
                    WorkerV3ApplicationHandoffExpectationV1::new(envelope, &occurrence);
                let challenge =
                    WorkerV3ApplicationHandoffChallengeV1::from_bytes(random_identity_bytes()?)
                        .map_err(|error| format!("invalid V3 application challenge: {error}"))?;
                command
                    .env(
                        WORKER_V3_APPLICATION_ENVELOPE_FD_ENV_V1,
                        envelope_fd.to_string(),
                    )
                    .env(
                        WORKER_V3_APPLICATION_ARTIFACT_DIR_FD_ENV_V1,
                        artifact_directory_fd.to_string(),
                    )
                    .env(
                        WORKER_V3_APPLICATION_HANDOFF_ACK_FD_ENV_V1,
                        ack_fd.to_string(),
                    )
                    .env(
                        WORKER_V3_APPLICATION_OCCURRENCE_ENV_V1,
                        encode_lower_hex(&occurrence.encode_canonical().map_err(|error| {
                            format!("failed to encode V3 application occurrence: {error}")
                        })?),
                    )
                    .env(
                        WORKER_V3_APPLICATION_HANDOFF_COMMITMENT_ENV_V1,
                        encode_lower_hex(
                            &expectation
                                .commitment()
                                .encode_canonical()
                                .map_err(|error| {
                                    format!("failed to encode V3 commitment: {error}")
                                })?,
                        ),
                    )
                    .env(
                        WORKER_V3_APPLICATION_HANDOFF_CHALLENGE_ENV_V1,
                        encode_lower_hex(
                            &challenge.encode_canonical().map_err(|error| {
                                format!("failed to encode V3 challenge: {error}")
                            })?,
                        ),
                    );
                ApplicationHandoffExpectationV1::WorkerV3 {
                    expectation,
                    challenge,
                }
            }
        };
        let timeouts = if matches!(protocol, ApplicationHandoffExpectationV1::WorkerV3 { .. }) {
            timeouts.for_worker_v3()
        } else {
            timeouts
        };
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
                crate::application_exec::protect_all_nonstdio_descriptors()?;
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
                #[cfg(any(test, feature = "worker-v2-fault-injection-test-only"))]
                if let Some(test_ready_fd) = test_ready_fd {
                    let test_ready = BorrowedFd::borrow_raw(test_ready_fd);
                    let flags = rustix::io::fcntl_getfd(test_ready).map_err(io::Error::from)?;
                    let status = rustix::fs::fcntl_getfl(test_ready).map_err(io::Error::from)?;
                    if !flags.contains(rustix::io::FdFlags::CLOEXEC)
                        || status & OFlags::ACCMODE != OFlags::WRONLY
                    {
                        return Err(io::Error::from_raw_os_error(libc::ESTALE));
                    }
                    crate::application_exec::expose_descriptor(test_ready_fd)?;
                }
                for inherited in [envelope_fd, artifact_directory_fd, ack_fd] {
                    crate::application_exec::expose_descriptor(inherited)?;
                }
                install_application_profile(&seccomp_filter, supervisor_socket)?;
                Ok(())
            });
        }
        Ok(PendingApplicationAck {
            read: ack_read,
            parent_write: Some(ack_write),
            protocol,
            sandbox: Some(sandbox),
            reaper: Some(reaper),
            timeouts,
            #[cfg(any(test, feature = "worker-v2-fault-injection-test-only"))]
            test_ready_read,
            #[cfg(any(test, feature = "worker-v2-fault-injection-test-only"))]
            test_ready_parent_write: test_ready_write,
        })
    }
}

pub(crate) fn terminate_application_group(
    mut child: Child,
    cleanup: ApplicationCleanup,
) -> Result<ExitStatus, String> {
    let process_group = child.id() as libc::pid_t;
    let mut failures = Vec::new();
    if let Err(error) = kill_process_group(process_group) {
        failures.push(error);
    }
    match child.kill() {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::InvalidInput => {}
        Err(error) => failures.push(format!("failed to kill application leader: {error}")),
    }
    let status = match transfer_application_cleanup(child, process_group, cleanup) {
        Ok(status) => Some(status),
        Err(error) => {
            failures.push(error);
            None
        }
    };
    if failures.is_empty() {
        Ok(status.expect("successful application termination retained its exit status"))
    } else {
        Err(failures.join("; "))
    }
}

pub(crate) fn wait_and_contain_application_group(
    child: Child,
    cleanup: ApplicationCleanup,
) -> Result<ExitStatus, String> {
    let process_group = child.id() as libc::pid_t;
    if let Err(error) = wait_for_leader_exit_without_reaping(process_group) {
        return match terminate_application_group(child, cleanup) {
            Ok(_) => Err(error),
            Err(containment) => Err(format!(
                "{error}; application containment failed: {containment}"
            )),
        };
    }

    let mut failures = Vec::new();
    if let Err(error) = kill_process_group(process_group) {
        failures.push(error);
    }
    let status = match transfer_application_cleanup(child, process_group, cleanup) {
        Ok(status) => Some(status),
        Err(error) => {
            failures.push(error);
            None
        }
    };
    if failures.is_empty() {
        Ok(status.expect("successful child wait produced an exit status"))
    } else {
        Err(failures.join("; "))
    }
}

pub(crate) struct ApplicationCleanup {
    reaper: ReaperReservation,
    sandbox: Option<ApplicationSandboxGuard>,
    timeout: Duration,
    #[cfg(test)]
    test_hold: Option<Arc<AtomicBool>>,
}

pub(crate) struct ApplicationHandoffGuard {
    cleanup: Option<ApplicationCleanup>,
}

impl ApplicationHandoffGuard {
    pub(crate) fn into_cleanup(mut self) -> ApplicationCleanup {
        self.cleanup
            .take()
            .expect("active application handoff owns cleanup state")
    }
}

pub(crate) struct ApplicationHandoffFailure {
    message: String,
    cleanup: Option<ApplicationCleanup>,
}

impl ApplicationHandoffFailure {
    pub(crate) fn into_parts(mut self) -> (String, ApplicationCleanup) {
        (
            self.message,
            self.cleanup
                .take()
                .expect("application handoff failure owns cleanup state"),
        )
    }
}

pub(crate) struct PendingApplicationAck {
    read: File,
    parent_write: Option<File>,
    protocol: ApplicationHandoffExpectationV1,
    sandbox: Option<PendingApplicationSandbox>,
    reaper: Option<ReaperReservation>,
    timeouts: ApplicationTimeouts,
    #[cfg(any(test, feature = "worker-v2-fault-injection-test-only"))]
    test_ready_read: Option<File>,
    #[cfg(any(test, feature = "worker-v2-fault-injection-test-only"))]
    test_ready_parent_write: Option<File>,
}

#[derive(Clone, Copy)]
enum ApplicationHandoffExpectationV1 {
    WorkerV2 {
        expectation: WorkerV2ApplicationHandoffExpectationV1,
        challenge: WorkerV2ApplicationHandoffChallengeV1,
    },
    WorkerV3 {
        expectation: WorkerV3ApplicationHandoffExpectationV1,
        challenge: WorkerV3ApplicationHandoffChallengeV1,
    },
}

impl ApplicationHandoffExpectationV1 {
    const fn maximum_ack_bytes(self) -> usize {
        match self {
            Self::WorkerV2 { .. } => WORKER_V2_APPLICATION_HANDOFF_ACK_BYTES_V1,
            Self::WorkerV3 { .. } => WORKER_V3_APPLICATION_HANDOFF_ACK_BYTES_V1,
        }
    }

    fn validate_ack(self, bytes: &[u8]) -> Result<(), String> {
        match self {
            Self::WorkerV2 {
                expectation,
                challenge,
            } => WorkerV2ApplicationHandoffAckV1::decode_canonical(bytes)
                .map_err(|error| format!("invalid Worker V2 application acknowledgment: {error}"))?
                .validate(expectation, challenge)
                .map_err(|error| format!("rejected Worker V2 application acknowledgment: {error}")),
            Self::WorkerV3 {
                expectation,
                challenge,
            } => WorkerV3ApplicationHandoffAckV1::decode_canonical(bytes)
                .map_err(|error| format!("invalid Worker V3 application acknowledgment: {error}"))?
                .validate(expectation, challenge)
                .map_err(|error| format!("rejected Worker V3 application acknowledgment: {error}")),
        }
    }
}

impl PendingApplicationAck {
    pub(crate) fn await_after_spawn(
        mut self,
        child: &mut Child,
    ) -> Result<ApplicationHandoffGuard, ApplicationHandoffFailure> {
        drop(self.parent_write.take());
        #[cfg(any(test, feature = "worker-v2-fault-injection-test-only"))]
        drop(self.test_ready_parent_write.take());
        let sandbox = match self
            .sandbox
            .take()
            .expect("pending acknowledgment owns its sandbox")
            .complete(child.id())
        {
            Ok(sandbox) => sandbox,
            Err(failure) => {
                let (message, sandbox) = failure.into_parts();
                return Err(self.failure(message, Some(sandbox)));
            }
        };
        #[cfg(any(test, feature = "worker-v2-fault-injection-test-only"))]
        if let Some(mut ready) = self.test_ready_read.take()
            && let Err(message) = read_test_ack_ready(&mut ready)
        {
            return Err(self.failure(message, Some(sandbox)));
        }
        let result = read_application_handoff_ack(
            &mut self.read,
            child,
            self.timeouts.ack,
            self.protocol.maximum_ack_bytes(),
        )
        .and_then(|bytes| self.protocol.validate_ack(&bytes));
        match result {
            Ok(()) => Ok(ApplicationHandoffGuard {
                cleanup: Some(self.cleanup(Some(sandbox))),
            }),
            Err(message) => Err(self.failure(message, Some(sandbox))),
        }
    }

    fn cleanup(&mut self, sandbox: Option<ApplicationSandboxGuard>) -> ApplicationCleanup {
        ApplicationCleanup {
            reaper: self
                .reaper
                .take()
                .expect("pending acknowledgment owns a reaper reservation"),
            sandbox,
            timeout: self.timeouts.cleanup,
            #[cfg(test)]
            test_hold: None,
        }
    }

    fn failure(
        &mut self,
        message: String,
        sandbox: Option<ApplicationSandboxGuard>,
    ) -> ApplicationHandoffFailure {
        ApplicationHandoffFailure {
            message,
            cleanup: Some(self.cleanup(sandbox)),
        }
    }
}

fn read_application_handoff_ack(
    read: &mut File,
    child: &Child,
    timeout: Duration,
    maximum_bytes: usize,
) -> Result<Vec<u8>, String> {
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or_else(|| "application handoff acknowledgment deadline overflowed".to_string())?;
    let mut bytes = Vec::with_capacity(maximum_bytes + 1);
    loop {
        poll_readable(read.as_raw_fd(), deadline)?;
        let mut chunk = [0_u8; 256];
        match read.read(&mut chunk) {
            Ok(0) => break,
            Ok(count) => {
                bytes.extend_from_slice(&chunk[..count]);
                if bytes.len() > maximum_bytes {
                    return Err("application handoff acknowledgment has extra bytes".to_string());
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
        let _ =
            observe_leader_exit_without_reaping_until(child.id() as libc::pid_t, Some(deadline))?;
    }
    Ok(bytes)
}

#[cfg(any(test, feature = "worker-v2-fault-injection-test-only"))]
fn read_test_ack_ready(read: &mut File) -> Result<(), String> {
    let deadline = Instant::now()
        .checked_add(TEST_ACK_STARTUP_TIMEOUT)
        .ok_or_else(|| "test ACK startup deadline overflowed".to_string())?;
    loop {
        poll_readable(read.as_raw_fd(), deadline).map_err(|error| {
            format!("test ACK readiness failed before ACK timing began: {error}")
        })?;
        let mut byte = [0_u8; 1];
        match read.read(&mut byte) {
            Ok(1) if byte == [1] => return Ok(()),
            Ok(0) => {
                return Err("test ACK readiness descriptor closed before readiness".to_string());
            }
            Ok(_) => return Err("test ACK readiness descriptor returned invalid data".to_string()),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(format!("failed to read test ACK readiness: {error}")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LeaderExitObservation {
    Running,
    Exited,
}

#[cfg(test)]
fn observe_leader_exit_without_reaping(
    leader: libc::pid_t,
) -> Result<LeaderExitObservation, String> {
    observe_leader_exit_without_reaping_until(leader, None)
}

fn observe_leader_exit_without_reaping_until(
    leader: libc::pid_t,
    deadline: Option<Instant>,
) -> Result<LeaderExitObservation, String> {
    observe_leader_exit_without_reaping_with(leader, deadline, |information| {
        // SAFETY: `information` is writable, P_PID selects the owned child, WNOHANG bounds the
        // observation, and WNOWAIT retains an exited leader until containment has signaled its
        // dedicated process group.
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                leader as libc::id_t,
                information.as_mut_ptr(),
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    })
}

fn observe_leader_exit_without_reaping_with(
    leader: libc::pid_t,
    deadline: Option<Instant>,
    mut wait: impl FnMut(&mut MaybeUninit<libc::siginfo_t>) -> io::Result<()>,
) -> Result<LeaderExitObservation, String> {
    loop {
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return Err("application handoff acknowledgment timed out".to_string());
        }
        let mut information = MaybeUninit::<libc::siginfo_t>::zeroed();
        match wait(&mut information) {
            Ok(()) => {
                // SAFETY: a successful waitid-style operation initialized the siginfo record.
                let observed = unsafe { information.assume_init().si_pid() };
                return match observed {
                    0 => Ok(LeaderExitObservation::Running),
                    observed if observed == leader => Ok(LeaderExitObservation::Exited),
                    observed => Err(format!(
                        "application handoff observed unexpected child {observed} instead of {leader}"
                    )),
                };
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => {
                return Err(format!(
                    "failed to inspect application during handoff without reaping it: {error} (errno {:?})",
                    error.raw_os_error()
                ));
            }
        }
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

fn random_identity_bytes() -> Result<[u8; 32], String> {
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
    Ok(bytes)
}

fn random_v2_challenge() -> Result<WorkerV2ApplicationHandoffChallengeV1, String> {
    WorkerV2ApplicationHandoffChallengeV1::from_bytes(random_identity_bytes()?)
        .map_err(|error| format!("invalid application handoff challenge: {error}"))
}

fn descriptor_occurrence(
    slot: u16,
    stat: &rustix::fs::Stat,
) -> Result<WorkerV3ApplicationInputOccurrenceV1, String> {
    WorkerV3ApplicationInputOccurrenceV1::from_linux_descriptor_v1(
        slot,
        stat.st_dev,
        stat.st_ino,
        stat.st_mode,
    )
    .map_err(|error| format!("failed to identify application descriptor slot {slot}: {error}"))
}

fn encode_lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn poll_readable(descriptor: RawFd, deadline: Instant) -> Result<(), String> {
    poll_readable_with(deadline, Instant::now, |millis| {
        poll_descriptor(descriptor, millis)
    })
}

fn poll_readable_with(
    deadline: Instant,
    mut now: impl FnMut() -> Instant,
    mut poll: impl FnMut(i32) -> io::Result<i32>,
) -> Result<(), String> {
    loop {
        let remaining = deadline.saturating_duration_since(now());
        if remaining.is_zero() {
            return Err("application handoff acknowledgment timed out".to_string());
        }
        let millis = duration_to_poll_millis(remaining);
        match poll(millis) {
            Ok(result) if result > 0 => {
                if deadline.saturating_duration_since(now()).is_zero() {
                    return Err("application handoff acknowledgment timed out".to_string());
                }
                return Ok(());
            }
            Ok(_) => return Err("application handoff acknowledgment timed out".to_string()),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => {
                return Err(format!(
                    "failed to wait for application handoff acknowledgment: {error}"
                ));
            }
        }
    }
}

fn duration_to_poll_millis(duration: Duration) -> i32 {
    let whole_millis = duration.as_millis();
    let rounded_millis = if duration.subsec_nanos().is_multiple_of(1_000_000) {
        whole_millis
    } else {
        whole_millis.saturating_add(1)
    };
    rounded_millis.clamp(1, i32::MAX as u128) as i32
}

fn poll_descriptor(descriptor: RawFd, millis: i32) -> io::Result<i32> {
    let mut pollfd = libc::pollfd {
        fd: descriptor,
        events: libc::POLLIN | libc::POLLHUP | libc::POLLERR,
        revents: 0,
    };
    // SAFETY: `pollfd` is one valid poll descriptor record for the duration of the call.
    let result = unsafe { libc::poll(&mut pollfd, 1, millis) };
    if result >= 0 {
        Ok(result)
    } else {
        Err(io::Error::last_os_error())
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

pub(crate) fn wait_for_application_exit_without_reaping(child: &Child) -> Result<(), String> {
    wait_for_leader_exit_without_reaping(child.id() as libc::pid_t)
}

fn transfer_application_cleanup(
    child: Child,
    process_group: libc::pid_t,
    mut cleanup: ApplicationCleanup,
) -> Result<ExitStatus, String> {
    let supervisor = Arc::clone(&cleanup.reaper.supervisor);
    let timeout = cleanup.timeout;
    let (completion, completed) = mpsc::sync_channel(1);
    supervisor.transfer(ReapJob {
        child,
        process_group,
        process_group_terminal: false,
        sandbox: cleanup.sandbox.take(),
        _reservation: cleanup.reaper,
        leader_status: None,
        completion: Some(completion),
        completion_error: None,
        last_retryable_error: None,
        #[cfg(test)]
        test_hold: cleanup.test_hold.take(),
        #[cfg(test)]
        test_completed: None,
        #[cfg(test)]
        test_retryable_error: None,
    });
    match completed.recv_timeout(timeout) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => Err(format!(
            "application cleanup remains pending in the fixed-capacity supervisor after {} ms",
            timeout.as_millis()
        )),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(
            "application cleanup supervisor lost its completion channel while retaining the child"
                .to_string(),
        ),
    }
}

fn try_reap_job(job: &mut ReapJob) -> Result<bool, String> {
    try_reap_job_with(job, kill_process_group, reap_process_group_nonblocking)
}

fn try_reap_job_with(
    job: &mut ReapJob,
    mut signal_group: impl FnMut(libc::pid_t) -> Result<(), String>,
    mut reap_group: impl FnMut(libc::pid_t) -> Result<bool, String>,
) -> Result<bool, String> {
    #[cfg(test)]
    if job
        .test_retryable_error
        .as_ref()
        .is_some_and(|fail| fail.load(Ordering::Acquire))
    {
        return Err("injected retryable nonblocking reap failure".to_string());
    }
    if !job.process_group_terminal {
        signal_group(job.process_group)?;
        if job.leader_status.is_none() {
            match job.child.try_wait() {
                Ok(Some(status)) => job.leader_status = Some(status),
                Ok(None) => return Ok(false),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => return Ok(false),
                Err(error) => {
                    return Err(format!(
                        "failed to nonblockingly reap application leader: {error}"
                    ));
                }
            }
        }
        if !reap_group(job.process_group)? {
            return Ok(false);
        }
        // ECHILD is terminal for this owned group after the leader has been reaped. From this
        // point the numeric PGID can be recycled, so sandbox shutdown must never signal or wait
        // on it again.
        job.process_group_terminal = true;
    }
    let Some(sandbox) = job.sandbox.as_mut() else {
        return Ok(true);
    };
    sandbox.request_shutdown();
    match sandbox.try_finish() {
        Ok(false) => Ok(false),
        Ok(true) => {
            job.sandbox.take();
            Ok(true)
        }
        Err(error) => {
            job.sandbox.take();
            job.completion_error = Some(format!(
                "application process reaped but seccomp supervisor cleanup failed: {error}"
            ));
            Ok(true)
        }
    }
}

fn reap_process_group_nonblocking(process_group: libc::pid_t) -> Result<bool, String> {
    reap_process_group_nonblocking_with(|| {
        let mut status = 0;
        // SAFETY: `status` is writable, the negative PID selects adopted children in the dedicated
        // application process group, and WNOHANG guarantees this supervisor never blocks.
        let result = unsafe { libc::waitpid(-process_group, &mut status, libc::WNOHANG) };
        if result >= 0 {
            Ok(result)
        } else {
            Err(io::Error::last_os_error())
        }
    })
}

fn reap_process_group_nonblocking_with(
    mut wait: impl FnMut() -> io::Result<libc::pid_t>,
) -> Result<bool, String> {
    const MAX_REAPS_PER_POLL: usize = 64;
    let mut reaped = 0;
    loop {
        match wait() {
            Ok(result) if result > 0 => {
                reaped += 1;
                if reaped == MAX_REAPS_PER_POLL {
                    return Ok(false);
                }
            }
            Ok(0) => return Ok(false),
            Ok(_) => {
                return Err(
                    "nonblocking application process-group reap returned an invalid PID"
                        .to_string(),
                );
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) if error.raw_os_error() == Some(libc::ECHILD) => return Ok(true),
            Err(error) => {
                return Err(format!(
                    "failed to nonblockingly reap application process-group descendants: {error}"
                ));
            }
        }
    }
}

fn envelope_names(directory: &PinnedDirectory) -> Result<Vec<String>, String> {
    let scan = directory.try_clone_for_transfer()?;
    let mut entries = rustix::fs::Dir::read_from(&scan)
        .map_err(|error| format!("failed to scan artifact directory: {error}"))?;
    collect_envelope_names(|visit| {
        for entry in &mut entries {
            let entry = entry.map_err(|error| format!("failed to read artifact entry: {error}"))?;
            visit(entry.file_name().to_bytes())?;
        }
        Ok(())
    })
}

fn collect_envelope_names(
    scan: impl FnOnce(&mut dyn FnMut(&[u8]) -> Result<(), String>) -> Result<(), String>,
) -> Result<Vec<String>, String> {
    let mut total_entries = 0_usize;
    let mut names = Vec::new();
    scan(&mut |bytes| {
        if is_synthetic_dot_entry(bytes) {
            return Ok(());
        }
        total_entries = total_entries.checked_add(1).ok_or_else(|| {
            "artifact directory entry count overflowed its scan bound".to_string()
        })?;
        if total_entries > MAX_WORKER_V2_ARTIFACT_DIRECTORY_ENTRIES_V1 {
            return Err(format!(
                "artifact directory exceeds {MAX_WORKER_V2_ARTIFACT_DIRECTORY_ENTRIES_V1} visible entries"
            ));
        }
        let canonical = if bytes.starts_with(ENVELOPE_PREFIX) {
            if !is_canonical_v2_envelope_name(bytes) {
                return Err("malformed Worker V2 envelope publication name".to_string());
            }
            true
        } else if bytes.starts_with(V3_ENVELOPE_PREFIX) && bytes.ends_with(V3_ENVELOPE_SUFFIX) {
            if !is_canonical_v3_envelope_name(bytes) {
                return Err("malformed Worker V3 envelope publication name".to_string());
            }
            true
        } else {
            false
        };
        if !canonical {
            return Ok(());
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
        Ok(())
    })?;
    names.sort_unstable();
    Ok(names)
}

fn is_canonical_v2_envelope_name(bytes: &[u8]) -> bool {
    bytes.len() == ENVELOPE_NAME_BYTES
        && bytes.starts_with(ENVELOPE_PREFIX)
        && bytes.ends_with(ENVELOPE_SUFFIX)
        && bytes[ENVELOPE_PREFIX.len()..ENVELOPE_PREFIX.len() + 64]
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn is_canonical_v3_envelope_name(bytes: &[u8]) -> bool {
    bytes.len() == V3_ENVELOPE_NAME_BYTES
        && bytes.starts_with(V3_ENVELOPE_PREFIX)
        && bytes.ends_with(V3_ENVELOPE_SUFFIX)
        && bytes[V3_ENVELOPE_PREFIX.len()..V3_ENVELOPE_PREFIX.len() + 64]
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn validate_single_envelope_schema(names: &[String]) -> Result<(), String> {
    let v2_names = names
        .iter()
        .filter(|name| is_canonical_v2_envelope_name(name.as_bytes()))
        .count();
    if v2_names != 0 && v2_names != names.len() {
        Err("Worker V2 and Worker V3 application envelopes cannot coexist".to_string())
    } else {
        Ok(())
    }
}

fn validate_envelope_stat(
    directory: &PinnedDirectory,
    name: &str,
    opened: &rustix::fs::Stat,
) -> Result<(), String> {
    let linked = statat(directory.file(), name, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
        format!("failed to inspect linked application envelope {name}: {error}")
    })?;
    if FileType::from_raw_mode(opened.st_mode) != FileType::RegularFile
        || opened.st_dev != linked.st_dev
        || opened.st_ino != linked.st_ino
        || opened.st_nlink != 1
        || opened.st_uid != unsafe { libc::geteuid() }
        || opened.st_mode & 0o077 != 0
    {
        return Err(format!("refusing unsafe application envelope {name}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::process::ExitStatusExt;

    const INVALID_ACK_CHILD_ENV: &str = "FE2O3_INVALID_ACK_RUST_FIXTURE_CHILD";
    const INVALID_ACK_FD_ENV: &str = "FE2O3_INVALID_ACK_RUST_FIXTURE_FD";
    const INVALID_ACK_PID_FILE_ENV: &str = "FE2O3_INVALID_ACK_RUST_FIXTURE_PID_FILE";
    const INVALID_ACK_TEST_NAME: &str = "application_handoff::tests::invalid_ack_early_exit_retains_leader_until_bounded_group_cleanup";
    const MIN_HIGH_ACK_FD: RawFd = 64;
    const REPEATED_SIGNAL_CHILD_ENV: &str = "FE2O3_ACK_POLL_REPEATED_SIGNAL_CHILD";

    unsafe extern "C" fn acknowledge_test_signal(_: libc::c_int) {}

    struct SignalActionGuard {
        signal: libc::c_int,
        previous: libc::sigaction,
    }

    impl SignalActionGuard {
        fn install(signal: libc::c_int) -> Self {
            // SAFETY: zero is a valid initial representation before all required sigaction fields
            // are initialized below.
            let mut action = unsafe { std::mem::zeroed::<libc::sigaction>() };
            action.sa_sigaction = acknowledge_test_signal as *const () as usize;
            action.sa_flags = 0;
            // SAFETY: `sa_mask` is writable and belongs to the local action record.
            assert_eq!(unsafe { libc::sigemptyset(&mut action.sa_mask) }, 0);
            let mut previous = MaybeUninit::<libc::sigaction>::uninit();
            // SAFETY: both action records are valid for this isolated test process.
            assert_eq!(
                unsafe { libc::sigaction(signal, &action, previous.as_mut_ptr()) },
                0,
                "install signal action: {}",
                io::Error::last_os_error()
            );
            // SAFETY: successful sigaction initialized the previous action record.
            let previous = unsafe { previous.assume_init() };
            Self { signal, previous }
        }
    }

    impl Drop for SignalActionGuard {
        fn drop(&mut self) {
            // SAFETY: `previous` came from a successful sigaction for this signal.
            let result =
                unsafe { libc::sigaction(self.signal, &self.previous, std::ptr::null_mut()) };
            assert_eq!(
                result,
                0,
                "restore signal action: {}",
                io::Error::last_os_error()
            );
        }
    }

    fn fresh_session_command(program: impl AsRef<std::ffi::OsStr>) -> Command {
        let mut command = Command::new(program);
        // SAFETY: the callback performs only child-side session syscalls before exec.
        unsafe {
            command.pre_exec(establish_fresh_application_session);
        }
        command
    }

    fn test_cleanup() -> ApplicationCleanup {
        ApplicationCleanup {
            reaper: application_reaper().reserve().unwrap(),
            sandbox: None,
            timeout: Duration::from_secs(2),
            test_hold: None,
        }
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

    fn await_nonreaping_exit(child: &Child) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match observe_leader_exit_without_reaping(child.id() as libc::pid_t).unwrap() {
                LeaderExitObservation::Exited => return,
                LeaderExitObservation::Running if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(1));
                }
                LeaderExitObservation::Running => panic!("application leader did not exit"),
            }
        }
    }

    fn canonical_envelope_name(fill: u8) -> Vec<u8> {
        assert!(fill.is_ascii_hexdigit() && !fill.is_ascii_uppercase());
        let mut name = Vec::with_capacity(ENVELOPE_NAME_BYTES);
        name.extend_from_slice(ENVELOPE_PREFIX);
        name.extend(std::iter::repeat_n(fill, 64));
        name.extend_from_slice(ENVELOPE_SUFFIX);
        name
    }

    fn canonical_v3_envelope_name(fill: u8) -> Vec<u8> {
        assert!(fill.is_ascii_hexdigit() && !fill.is_ascii_uppercase());
        let mut name = Vec::with_capacity(V3_ENVELOPE_NAME_BYTES);
        name.extend_from_slice(V3_ENVELOPE_PREFIX);
        name.extend(std::iter::repeat_n(fill, 64));
        name.extend_from_slice(V3_ENVELOPE_SUFFIX);
        name
    }

    #[test]
    fn artifact_scan_accepts_exact_total_entry_bound() {
        let mut visited = 0_usize;
        let names = collect_envelope_names(|visit| {
            visit(b".")?;
            visit(b"..")?;
            for _ in 0..MAX_WORKER_V2_ARTIFACT_DIRECTORY_ENTRIES_V1 {
                visited += 1;
                visit(b"unrelated")?;
            }
            Ok(())
        })
        .unwrap();

        assert_eq!(visited, MAX_WORKER_V2_ARTIFACT_DIRECTORY_ENTRIES_V1);
        assert!(names.is_empty());
    }

    #[test]
    fn artifact_scan_rejects_limit_plus_one_unrelated_entry_early() {
        let mut visited = 0_usize;
        let error = collect_envelope_names(|visit| {
            visit(b".")?;
            visit(b"..")?;
            for _ in 0..MAX_WORKER_V2_ARTIFACT_DIRECTORY_ENTRIES_V1 + 100 {
                visited += 1;
                visit(b"unrelated")?;
            }
            panic!("scan continued after the first over-limit entry");
        })
        .unwrap_err();

        assert_eq!(visited, MAX_WORKER_V2_ARTIFACT_DIRECTORY_ENTRIES_V1 + 1);
        assert_eq!(
            error,
            format!(
                "artifact directory exceeds {MAX_WORKER_V2_ARTIFACT_DIRECTORY_ENTRIES_V1} visible entries"
            )
        );
    }

    #[test]
    fn artifact_scan_counts_mixed_entries_and_sorts_canonical_candidates() {
        let first = canonical_envelope_name(b'0');
        let last = canonical_envelope_name(b'f');
        let names = collect_envelope_names(|visit| {
            for _ in 0..MAX_WORKER_V2_ARTIFACT_DIRECTORY_ENTRIES_V1 - 3 {
                visit(b"unrelated")?;
            }
            visit(&last)?;
            visit(b"also-unrelated")?;
            visit(&first)
        })
        .unwrap();

        assert_eq!(
            names,
            [
                String::from_utf8(first).unwrap(),
                String::from_utf8(last).unwrap(),
            ]
        );
    }

    #[test]
    fn artifact_scan_finds_v3_envelopes_and_ignores_v3_readiness_siblings() {
        let envelope = canonical_v3_envelope_name(b'a');
        let claim = format!(
            "{}{}.claim",
            std::str::from_utf8(V3_ENVELOPE_PREFIX).unwrap(),
            "b".repeat(64)
        );
        let receipt = format!(
            "{}{}.receipt",
            std::str::from_utf8(V3_ENVELOPE_PREFIX).unwrap(),
            "c".repeat(64)
        );
        let names = collect_envelope_names(|visit| {
            visit(claim.as_bytes())?;
            visit(&envelope)?;
            visit(receipt.as_bytes())
        })
        .unwrap();
        assert_eq!(names, [String::from_utf8(envelope).unwrap()]);
        validate_single_envelope_schema(&names).unwrap();
    }

    #[test]
    fn application_handoff_rejects_v2_v3_envelope_coexistence() {
        let names = [
            String::from_utf8(canonical_envelope_name(b'a')).unwrap(),
            String::from_utf8(canonical_v3_envelope_name(b'b')).unwrap(),
        ];
        assert_eq!(
            validate_single_envelope_schema(&names),
            Err("Worker V2 and Worker V3 application envelopes cannot coexist".to_string())
        );
    }

    #[test]
    fn artifact_scan_preserves_deterministic_duplicate_candidates() {
        let duplicate = canonical_envelope_name(b'a');
        let names = collect_envelope_names(|visit| {
            visit(&duplicate)?;
            visit(&duplicate)
        })
        .unwrap();

        let duplicate = String::from_utf8(duplicate).unwrap();
        assert_eq!(names, [duplicate.clone(), duplicate]);
    }

    #[test]
    fn artifact_scan_fails_closed_on_entry_error() {
        let error = collect_envelope_names(|visit| {
            visit(b"unrelated")?;
            Err("failed to read artifact entry: injected EIO".to_string())
        })
        .unwrap_err();

        assert_eq!(error, "failed to read artifact entry: injected EIO");
    }

    #[test]
    fn artifact_scan_does_not_retain_huge_unrelated_names() {
        let hostile = vec![b'x'; 1024 * 1024];
        let names = collect_envelope_names(|visit| visit(&hostile)).unwrap();
        assert!(names.is_empty());
    }

    fn spawn_paused_outsider() -> libc::pid_t {
        // SAFETY: the child branch performs only the async-signal-safe `pause` syscall.
        let outsider = unsafe { libc::fork() };
        assert!(
            outsider >= 0,
            "fork outsider: {}",
            io::Error::last_os_error()
        );
        if outsider == 0 {
            unsafe {
                loop {
                    libc::pause();
                }
            }
        }
        outsider
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
        let application = application.spawn().unwrap();
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

        let containment = terminate_application_group(application, test_cleanup());
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
        assert_eq!(containment.unwrap().signal(), Some(libc::SIGKILL));
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
        assert!(reap_process_group_nonblocking(leader).unwrap());
    }

    fn run_invalid_ack_child_fixture() {
        let ack_fd = std::env::var(INVALID_ACK_FD_ENV)
            .unwrap()
            .parse::<RawFd>()
            .unwrap();
        assert!(ack_fd >= MIN_HIGH_ACK_FD);
        // SAFETY: the environment names the exact descriptor transferred by the parent pre-exec
        // callback. A successful exec proves the callback cleared CLOEXEC on that descriptor.
        let flags = unsafe { libc::fcntl(ack_fd, libc::F_GETFD) };
        assert!(flags >= 0, "inherited ACK descriptor is closed");
        assert_eq!(flags & libc::FD_CLOEXEC, 0);
        // SAFETY: this exec'd fixture owns the inherited ACK descriptor and closes it after the
        // single deliberately invalid byte.
        let mut ack = unsafe { File::from_raw_fd(ack_fd) };
        ack.write_all(b"x").unwrap();
        drop(ack);

        // SAFETY: the descendant branch performs only the async-signal-safe `pause` syscall. The
        // parent fixture exits immediately; the outer subreaper owns bounded group cleanup.
        let descendant = unsafe { libc::fork() };
        assert!(
            descendant >= 0,
            "fork invalid-ACK descendant: {}",
            io::Error::last_os_error()
        );
        if descendant == 0 {
            unsafe {
                loop {
                    libc::pause();
                }
            }
        }
        std::fs::write(
            std::env::var_os(INVALID_ACK_PID_FILE_ENV).unwrap(),
            descendant.to_string(),
        )
        .unwrap();
    }

    #[test]
    fn invalid_ack_early_exit_retains_leader_until_bounded_group_cleanup() {
        if std::env::var_os(INVALID_ACK_CHILD_ENV).is_some() {
            run_invalid_ack_child_fixture();
            return;
        }

        ensure_child_subreaper().unwrap();
        let pid_file = std::env::temp_dir().join(format!(
            "cargo-fe2o3-invalid-ack-descendant-{}",
            std::process::id()
        ));
        let (mut ack_read, original_ack_write) = cloexec_pipe().unwrap();
        let ack_write = File::from(
            rustix::io::fcntl_dupfd_cloexec(&original_ack_write, MIN_HIGH_ACK_FD).unwrap(),
        );
        drop(original_ack_write);
        let ack_fd = ack_write.as_raw_fd();
        assert!(ack_fd >= MIN_HIGH_ACK_FD);
        assert!(
            rustix::io::fcntl_getfd(&ack_write)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
        let mut command = fresh_session_command(std::env::current_exe().unwrap());
        // SAFETY: the callback changes only the inherited ACK descriptor before exec. Session
        // establishment was registered first and both callbacks run in the single-threaded child.
        unsafe {
            command.pre_exec(move || {
                if libc::fcntl(ack_fd, libc::F_SETFD, 0) != 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        command
            .arg("--exact")
            .arg(INVALID_ACK_TEST_NAME)
            .arg("--nocapture")
            .env(INVALID_ACK_CHILD_ENV, "1")
            .env(INVALID_ACK_FD_ENV, ack_fd.to_string())
            .env(INVALID_ACK_PID_FILE_ENV, &pid_file);
        let application = command.spawn().unwrap();
        drop(ack_write);

        await_nonreaping_exit(&application);
        let bytes = read_application_handoff_ack(
            &mut ack_read,
            &application,
            Duration::from_secs(1),
            WORKER_V2_APPLICATION_HANDOFF_ACK_BYTES_V1,
        )
        .unwrap();
        assert_eq!(bytes, b"x");
        assert!(WorkerV2ApplicationHandoffAckV1::decode_canonical(&bytes).is_err());
        assert_eq!(
            observe_leader_exit_without_reaping(application.id() as libc::pid_t).unwrap(),
            LeaderExitObservation::Exited
        );
        assert_eq!(
            observe_leader_exit_without_reaping(application.id() as libc::pid_t).unwrap(),
            LeaderExitObservation::Exited
        );

        let descendant = std::fs::read_to_string(&pid_file)
            .unwrap()
            .trim()
            .parse::<libc::pid_t>()
            .unwrap();
        let _ = std::fs::remove_file(&pid_file);
        assert!(process_exists(descendant));
        let outsider = spawn_paused_outsider();
        // SAFETY: both calls only query live process/session identities.
        let outsider_session = unsafe { libc::getsid(outsider) };
        let runner_session = unsafe { libc::getsid(0) };
        let started = Instant::now();
        let containment = terminate_application_group(application, test_cleanup());
        let elapsed = started.elapsed();
        let descendant_survived = process_exists(descendant);
        let outsider_survived = process_exists(outsider);
        // SAFETY: the outsider is this test's live child and is reaped immediately below.
        unsafe { libc::kill(outsider, libc::SIGKILL) };
        wait_for_raw_child(outsider);

        assert!(containment.is_ok(), "{containment:?}");
        assert!(elapsed < Duration::from_secs(2), "cleanup took {elapsed:?}");
        assert!(
            !descendant_survived,
            "application descendant escaped cleanup"
        );
        assert_eq!(outsider_session, runner_session);
        assert!(outsider_survived, "cleanup killed an unrelated outsider");
    }

    #[test]
    fn ack_exit_observation_retries_eintr_and_echild_fails_closed() {
        let mut command = fresh_session_command("/bin/true");
        let child = command.spawn().unwrap();
        let leader = child.id() as libc::pid_t;
        wait_for_leader_exit_without_reaping(leader).unwrap();

        let mut attempts = 0;
        let observation = observe_leader_exit_without_reaping_with(leader, None, |information| {
            attempts += 1;
            if attempts == 1 {
                return Err(io::Error::from(io::ErrorKind::Interrupted));
            }
            // SAFETY: the writable record and flags match the production waitid observation.
            let result = unsafe {
                libc::waitid(
                    libc::P_PID,
                    leader as libc::id_t,
                    information.as_mut_ptr(),
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            };
            if result == 0 {
                Ok(())
            } else {
                Err(io::Error::last_os_error())
            }
        })
        .unwrap();
        assert_eq!(attempts, 2);
        assert_eq!(observation, LeaderExitObservation::Exited);
        terminate_application_group(child, test_cleanup()).unwrap();

        let error = observe_leader_exit_without_reaping(leader).unwrap_err();
        assert!(error.contains("without reaping"), "{error}");
        assert!(
            error.contains(&format!("errno Some({})", libc::ECHILD)),
            "{error}"
        );
    }

    #[test]
    fn ack_poll_recomputes_the_deadline_after_every_interruption() {
        let origin = Instant::now();
        let elapsed = std::cell::Cell::new(Duration::ZERO);
        let attempts = std::cell::Cell::new(0_u32);
        let requested = std::cell::RefCell::new(Vec::new());
        let result = poll_readable_with(
            origin + Duration::from_millis(10),
            || origin + elapsed.get(),
            |millis| {
                requested.borrow_mut().push(millis);
                attempts.set(attempts.get() + 1);
                elapsed.set(elapsed.get() + Duration::from_millis(1));
                Err(io::Error::from(io::ErrorKind::Interrupted))
            },
        );

        assert_eq!(
            result.unwrap_err(),
            "application handoff acknowledgment timed out"
        );
        assert_eq!(attempts.get(), 10);
        assert_eq!(&*requested.borrow(), &[10, 9, 8, 7, 6, 5, 4, 3, 2, 1]);
    }

    #[test]
    fn ack_poll_rounds_submillisecond_deadlines_up_without_overflow() {
        assert_eq!(duration_to_poll_millis(Duration::from_nanos(1)), 1);
        assert_eq!(duration_to_poll_millis(Duration::from_millis(1)), 1);
        assert_eq!(duration_to_poll_millis(Duration::from_micros(1_001)), 2);
        assert_eq!(duration_to_poll_millis(Duration::MAX), i32::MAX);
    }

    #[test]
    fn repeated_signals_cannot_extend_the_ack_poll_deadline() {
        if std::env::var_os(REPEATED_SIGNAL_CHILD_ENV).is_some() {
            run_repeated_signal_deadline_probe();
            return;
        }

        let output = Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("application_handoff::tests::repeated_signals_cannot_extend_the_ack_poll_deadline")
            .arg("--nocapture")
            .env(REPEATED_SIGNAL_CHILD_ENV, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "isolated repeated-signal probe failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn run_repeated_signal_deadline_probe() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

        let _signal_action = SignalActionGuard::install(libc::SIGUSR2);
        let (read, _write) = cloexec_pipe().unwrap();
        // SAFETY: pthread_self returns the identity of this test thread for targeted delivery.
        let target = unsafe { libc::pthread_self() };
        let stop = Arc::new(AtomicBool::new(false));
        let sent = Arc::new(AtomicUsize::new(0));
        let sender_stop = Arc::clone(&stop);
        let sender_sent = Arc::clone(&sent);
        let sender = std::thread::spawn(move || {
            for _ in 0..2_000 {
                if sender_stop.load(Ordering::Acquire) {
                    return Ok(());
                }
                // SAFETY: `target` identifies the live parent test thread until this sender joins.
                let result = unsafe { libc::pthread_kill(target, libc::SIGUSR2) };
                if result != 0 {
                    return Err(result);
                }
                sender_sent.fetch_add(1, Ordering::Relaxed);
                std::thread::sleep(Duration::from_millis(1));
            }
            Ok(())
        });

        let interrupted = std::cell::Cell::new(0_usize);
        let started = Instant::now();
        let deadline = started.checked_add(Duration::from_millis(75)).unwrap();
        let result = poll_readable_with(deadline, Instant::now, |millis| {
            let result = poll_descriptor(read.as_raw_fd(), millis);
            if matches!(&result, Err(error) if error.kind() == io::ErrorKind::Interrupted) {
                interrupted.set(interrupted.get() + 1);
            }
            result
        });
        let elapsed = started.elapsed();
        stop.store(true, Ordering::Release);
        assert_eq!(sender.join().unwrap(), Ok(()));

        assert_eq!(
            result.unwrap_err(),
            "application handoff acknowledgment timed out"
        );
        assert!(sent.load(Ordering::Relaxed) >= 10);
        assert!(interrupted.get() >= 10);
        // The sender runs for more than two seconds if polling resets its timeout. The broad
        // one-second bound avoids depending on scheduler-scale timing while detecting that bug.
        assert!(elapsed < Duration::from_secs(1), "poll took {elapsed:?}");
    }

    #[test]
    fn early_exit_without_ack_remains_waitable_until_cleanup() {
        let (mut ack_read, ack_write) = cloexec_pipe().unwrap();
        let mut command = fresh_session_command("/bin/true");
        let child = command.spawn().unwrap();
        drop(ack_write);
        assert!(
            read_application_handoff_ack(
                &mut ack_read,
                &child,
                Duration::from_secs(1),
                WORKER_V2_APPLICATION_HANDOFF_ACK_BYTES_V1,
            )
            .unwrap()
            .is_empty()
        );
        await_nonreaping_exit(&child);
        assert_eq!(
            observe_leader_exit_without_reaping(child.id() as libc::pid_t).unwrap(),
            LeaderExitObservation::Exited
        );
        let started = Instant::now();
        terminate_application_group(child, test_cleanup()).unwrap();
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn successful_wait_contains_before_reaping_leader() {
        for _ in 0..100 {
            let mut command = fresh_session_command("/bin/true");
            let child = command.spawn().unwrap();
            assert!(
                wait_and_contain_application_group(child, test_cleanup())
                    .unwrap()
                    .success()
            );
        }
    }

    #[test]
    fn successful_wait_observation_error_still_transfers_and_reaps() {
        let mut command = fresh_session_command("/bin/true");
        let mut child = command.spawn().unwrap();
        assert!(child.wait().unwrap().success());
        let error = wait_and_contain_application_group(child, test_cleanup()).unwrap_err();
        assert!(error.contains("observe application leader"), "{error}");
        assert!(!error.contains("cleanup remains pending"), "{error}");
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
        let child = command.spawn().unwrap();
        let status = wait_and_contain_application_group(child, test_cleanup()).unwrap();
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

    #[test]
    fn nonblocking_group_reap_retries_eintr_and_reports_pending_and_complete() {
        let mut attempts = 0;
        let pending = reap_process_group_nonblocking_with(|| {
            attempts += 1;
            match attempts {
                1 => Err(io::Error::from(io::ErrorKind::Interrupted)),
                2 => Ok(41),
                3 => Ok(0),
                _ => unreachable!(),
            }
        })
        .unwrap();
        assert!(!pending);
        assert_eq!(attempts, 3);

        let complete =
            reap_process_group_nonblocking_with(|| Err(io::Error::from_raw_os_error(libc::ECHILD)))
                .unwrap();
        assert!(complete);
    }

    #[test]
    fn bounded_supervisor_saturates_recovers_and_uses_one_worker() {
        let supervisor = ReaperSupervisor::new(2);
        let hold = Arc::new(AtomicBool::new(true));
        let mut completed = Vec::new();
        let mut pids = Vec::new();

        for _ in 0..2 {
            let reservation = supervisor.reserve().unwrap();
            let child = fresh_session_command("/bin/sleep")
                .arg("30")
                .spawn()
                .unwrap();
            let process_group = child.id() as libc::pid_t;
            let observed = Arc::new(AtomicBool::new(false));
            completed.push(Arc::clone(&observed));
            pids.push(process_group);
            supervisor.transfer(ReapJob {
                child,
                process_group,
                process_group_terminal: false,
                sandbox: None,
                _reservation: reservation,
                leader_status: None,
                completion: None,
                completion_error: None,
                last_retryable_error: None,
                test_hold: Some(Arc::clone(&hold)),
                test_completed: Some(observed),
                test_retryable_error: None,
            });
        }

        let saturation = match supervisor.reserve() {
            Ok(_) => panic!("supervisor admitted work beyond its fixed capacity"),
            Err(error) => error,
        };
        assert!(saturation.contains("saturated"));
        assert_eq!(supervisor.worker_count.load(Ordering::Acquire), 1);
        assert_eq!(supervisor.jobs.lock().unwrap().len(), 2);
        for pid in &pids {
            assert!(process_exists(*pid));
        }

        hold.store(false, Ordering::Release);
        supervisor.wake.notify_all();
        let deadline = Instant::now() + Duration::from_secs(5);
        while completed
            .iter()
            .any(|observed| !observed.load(Ordering::Acquire))
            && Instant::now() < deadline
        {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(
            completed
                .iter()
                .all(|observed| observed.load(Ordering::Acquire)),
            "supervisor did not eventually reap every retained child"
        );
        assert_eq!(supervisor.reserved.load(Ordering::Acquire), 0);
        for pid in pids {
            assert!(!process_exists(pid));
        }
        drop(supervisor.reserve().unwrap());
        assert_eq!(supervisor.worker_count.load(Ordering::Acquire), 1);
        supervisor.stop_for_test();
    }

    #[test]
    fn cleanup_pending_returns_within_injected_bound_and_eventually_reaps() {
        let supervisor = ReaperSupervisor::new(1);
        let hold = Arc::new(AtomicBool::new(true));
        let reservation = supervisor.reserve().unwrap();
        let child = fresh_session_command("/bin/sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let process_group = child.id() as libc::pid_t;
        let timeout = Duration::from_millis(80);
        let started = Instant::now();
        let error = transfer_application_cleanup(
            child,
            process_group,
            ApplicationCleanup {
                reaper: reservation,
                sandbox: None,
                timeout,
                test_hold: Some(Arc::clone(&hold)),
            },
        )
        .unwrap_err();
        let elapsed = started.elapsed();
        assert!(error.contains("cleanup remains pending"), "{error}");
        assert!(
            elapsed >= timeout,
            "cleanup returned too early: {elapsed:?}"
        );
        assert!(
            elapsed < Duration::from_secs(2),
            "cleanup exceeded its broad scheduler bound: {elapsed:?}"
        );
        let saturation = match supervisor.reserve() {
            Ok(_) => panic!("supervisor admitted work beyond its fixed capacity"),
            Err(error) => error,
        };
        assert!(saturation.contains("saturated"));
        assert!(process_exists(process_group));

        hold.store(false, Ordering::Release);
        supervisor.wake.notify_all();
        let deadline = Instant::now() + Duration::from_secs(5);
        while supervisor.reserved.load(Ordering::Acquire) != 0 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(supervisor.reserved.load(Ordering::Acquire), 0);
        assert!(!process_exists(process_group));
        assert_eq!(supervisor.worker_count.load(Ordering::Acquire), 1);
        supervisor.stop_for_test();
    }

    #[test]
    fn stalled_sandbox_shutdown_does_not_block_unrelated_reaps() {
        let supervisor = ReaperSupervisor::new(2);
        let release = Arc::new(AtomicBool::new(false));
        let stalled_completed = Arc::new(AtomicBool::new(false));
        let unrelated_completed = Arc::new(AtomicBool::new(false));

        for (sandbox, completed) in [
            (
                Some(ApplicationSandboxGuard::test_stalled_guard(Arc::clone(
                    &release,
                ))),
                Arc::clone(&stalled_completed),
            ),
            (None, Arc::clone(&unrelated_completed)),
        ] {
            let reservation = supervisor.reserve().unwrap();
            let child = fresh_session_command("/bin/true").spawn().unwrap();
            let process_group = child.id() as libc::pid_t;
            supervisor.transfer(ReapJob {
                child,
                process_group,
                process_group_terminal: false,
                sandbox,
                _reservation: reservation,
                leader_status: None,
                completion: None,
                completion_error: None,
                last_retryable_error: None,
                test_hold: None,
                test_completed: Some(completed),
                test_retryable_error: None,
            });
        }

        let deadline = Instant::now() + Duration::from_secs(5);
        while !unrelated_completed.load(Ordering::Acquire) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(unrelated_completed.load(Ordering::Acquire));
        assert!(!stalled_completed.load(Ordering::Acquire));
        assert_eq!(supervisor.reserved.load(Ordering::Acquire), 1);
        assert_eq!(supervisor.worker_count.load(Ordering::Acquire), 1);

        release.store(true, Ordering::Release);
        supervisor.wake.notify_all();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !stalled_completed.load(Ordering::Acquire) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(stalled_completed.load(Ordering::Acquire));
        assert_eq!(supervisor.reserved.load(Ordering::Acquire), 0);
        supervisor.stop_for_test();
    }

    #[test]
    fn terminal_process_group_is_never_reused_for_sandbox_only_polling() {
        let supervisor = ReaperSupervisor::new(1);
        let reservation = supervisor.reserve().unwrap();
        let child = fresh_session_command("/bin/true").spawn().unwrap();
        let process_group = child.id() as libc::pid_t;
        wait_for_leader_exit_without_reaping(process_group).unwrap();
        let release = Arc::new(AtomicBool::new(false));
        let mut job = ReapJob {
            child,
            process_group,
            process_group_terminal: false,
            sandbox: Some(ApplicationSandboxGuard::test_stalled_guard(Arc::clone(
                &release,
            ))),
            _reservation: reservation,
            leader_status: None,
            completion: None,
            completion_error: None,
            last_retryable_error: None,
            test_hold: None,
            test_completed: None,
            test_retryable_error: None,
        };
        let signals = std::cell::Cell::new(0_u32);

        assert!(
            !try_reap_job_with(
                &mut job,
                |_| {
                    signals.set(signals.get() + 1);
                    Ok(())
                },
                |_| Ok(true),
            )
            .unwrap()
        );
        assert!(job.process_group_terminal);
        assert_eq!(signals.get(), 1);

        // Treat the numeric identity as if the kernel had already assigned it to an unrelated
        // process. Sandbox-only retries must not touch either signal or wait operations.
        for simulated_reuse in 0..100 {
            assert!(
                !try_reap_job_with(
                    &mut job,
                    |_| panic!("terminal/reused PGID was signalled at reuse {simulated_reuse}"),
                    |_| panic!("terminal/reused PGID was waited on at reuse {simulated_reuse}"),
                )
                .unwrap()
            );
        }
        assert_eq!(signals.get(), 1);

        release.store(true, Ordering::Release);
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut completed = false;
        while !completed && Instant::now() < deadline {
            completed = try_reap_job_with(
                &mut job,
                |_| panic!("terminal/reused PGID was signalled during final sandbox shutdown"),
                |_| panic!("terminal/reused PGID was waited on during final sandbox shutdown"),
            )
            .unwrap();
            if !completed {
                thread::sleep(Duration::from_millis(5));
            }
        }
        assert!(completed, "sandbox cleanup did not finish after release");
        assert!(job.process_group_terminal);
        supervisor.stop_for_test();
    }

    #[test]
    fn retryable_reap_error_is_observable_and_retains_capacity_until_recovery() {
        let supervisor = ReaperSupervisor::new(1);
        let fail = Arc::new(AtomicBool::new(true));
        let completed = Arc::new(AtomicBool::new(false));
        let reservation = supervisor.reserve().unwrap();
        let child = fresh_session_command("/bin/sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let process_group = child.id() as libc::pid_t;
        let (completion, result) = mpsc::sync_channel(1);
        supervisor.transfer(ReapJob {
            child,
            process_group,
            process_group_terminal: false,
            sandbox: None,
            _reservation: reservation,
            leader_status: None,
            completion: Some(completion),
            completion_error: None,
            last_retryable_error: None,
            test_hold: None,
            test_completed: Some(Arc::clone(&completed)),
            test_retryable_error: Some(Arc::clone(&fail)),
        });

        let error = result
            .recv_timeout(Duration::from_secs(2))
            .expect("retryable reap error was not reported")
            .unwrap_err();
        assert!(error.contains("retained"), "{error}");
        assert!(error.contains("retryable"), "{error}");
        let saturation = match supervisor.reserve() {
            Ok(_) => panic!("retryable cleanup error released its reserved slot"),
            Err(error) => error,
        };
        assert!(saturation.contains("saturated"));
        assert!(process_exists(process_group));

        fail.store(false, Ordering::Release);
        supervisor.wake.notify_all();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !completed.load(Ordering::Acquire) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(completed.load(Ordering::Acquire));
        assert_eq!(supervisor.reserved.load(Ordering::Acquire), 0);
        assert!(!process_exists(process_group));
        supervisor.stop_for_test();
    }

    #[test]
    fn worker_start_failure_never_publishes_a_reservation() {
        let supervisor = ReaperSupervisor::new(1);
        supervisor.fail_worker_start.store(true, Ordering::Release);
        for attempt in 0..100 {
            let error = match supervisor.reserve() {
                Ok(_) => panic!("reservation {attempt} succeeded without a cleanup worker"),
                Err(error) => error,
            };
            assert!(
                error.contains("startup failure"),
                "attempt={attempt}: {error}"
            );
        }
        assert_eq!(supervisor.reserved.load(Ordering::Acquire), 0);
        assert_eq!(supervisor.worker_count.load(Ordering::Acquire), 0);
        assert!(supervisor.jobs.lock().unwrap().is_empty());
    }

    #[test]
    fn worker_panic_is_observable_fails_closed_and_uses_process_fallback() {
        let supervisor = ReaperSupervisor::new(1);
        let reservation = supervisor.reserve().unwrap();
        let child = fresh_session_command("/bin/sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let process_group = child.id() as libc::pid_t;
        let (completion, result) = mpsc::sync_channel(1);
        supervisor.panic_worker.store(true, Ordering::Release);
        supervisor.transfer(ReapJob {
            child,
            process_group,
            process_group_terminal: false,
            sandbox: None,
            _reservation: reservation,
            leader_status: None,
            completion: Some(completion),
            completion_error: None,
            last_retryable_error: None,
            test_hold: None,
            test_completed: None,
            test_retryable_error: None,
        });

        let error = result
            .recv_timeout(Duration::from_secs(2))
            .expect("worker panic was not reported")
            .unwrap_err();
        assert!(error.contains("worker panic"), "{error}");
        let admission = match supervisor.reserve() {
            Ok(_) => panic!("dead cleanup worker admitted new ownership"),
            Err(error) => error,
        };
        assert!(admission.contains("fails closed"), "{admission}");

        supervisor.finish_process().unwrap();
        assert_eq!(supervisor.reserved.load(Ordering::Acquire), 0);
        assert!(!process_exists(process_group));
    }

    #[test]
    fn short_test_timeouts_are_internal_and_distinct_from_production() {
        assert!(ApplicationTimeouts::TEST_SHORT.ack < ApplicationTimeouts::PRODUCTION.ack);
        assert!(ApplicationTimeouts::TEST_SHORT.cleanup < ApplicationTimeouts::PRODUCTION.cleanup);
        assert!(
            ApplicationTimeouts::TEST_SCHEDULER_TOLERANT.ack > ApplicationTimeouts::PRODUCTION.ack
        );
        assert!(
            ApplicationTimeouts::TEST_SCHEDULER_TOLERANT.cleanup
                > ApplicationTimeouts::PRODUCTION.cleanup
        );
        assert_eq!(
            ApplicationTimeouts::PRODUCTION.for_worker_v3().ack,
            WORKER_V3_PRODUCTION_ACK_TIMEOUT
        );
        assert_eq!(
            ApplicationTimeouts::TEST_SHORT.for_worker_v3().ack,
            ApplicationTimeouts::TEST_SHORT.ack
        );
        assert_eq!(
            ApplicationTimeouts::TEST_SCHEDULER_TOLERANT
                .for_worker_v3()
                .ack,
            ApplicationTimeouts::TEST_SCHEDULER_TOLERANT.ack
        );
    }
}
