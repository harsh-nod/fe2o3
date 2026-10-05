//! Closed root-side bootstrap custody, not proof-helper or execution admission.
//! The child retains the entire backing through aggregate cleanup. Root IDs,
//! mapping readback, a READY record and a live pidfd are not deployment provenance.
use crate::{
    compiler_invocation_backing::CompilerInvocationBacking as Compiler,
    native_launch::{self as native, Channels, CompilerExecutionLaunchErrorV2 as NativeError},
    native_v3::root_intake::Receiver,
    proof_helper_backing::{
        ProofHelperBacking as Backing, ProofHelperBackingError as BackingError,
    },
};
use fe2o3_compiler_execution_protocol::{
    PROOF_EXECUTOR_BOOTSTRAP_BYTES_V1 as WIRE, ProofExecutorBootstrapErrorV1 as RecordError,
    ProofExecutorBootstrapKindV1 as Kind, ProofExecutorBootstrapRecordV1 as Record,
    ProofExecutorBootstrapStorageV1 as RecordStorage,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_protected_service_profile::{
    ProtectedServiceCredentialProfileV1 as Credentials, observations,
};
use fe2o3_protected_service_spawn::{
    ProtectedServiceCleanupServiceV2 as Cleanup, ProtectedServiceDescriptorBindingV1 as Binding,
    RetainedResourceAccessErrorV2 as AccessError,
    cleanup_bridge::CleanupPollV1 as CleanupPoll,
    launch_io,
    native_spawn::{
        ProtectedServiceSpawnErrorV2 as SpawnError,
        RootOwnedRetainedServiceChildV2 as RetainedChild, StagedProtectedServiceExecV2 as Stage,
    },
};
use fe2o3_protected_static_executable::{
    ProtectedStaticExecutableMeasurementV1 as Measurement, ProtectedStaticExecutableV2 as Image,
};
use rustix::{io::FdFlags, net, process::Pid};
use std::{
    fmt,
    fs::File,
    mem::size_of,
    os::fd::{AsFd, BorrowedFd, OwnedFd},
    sync::{Arc, Mutex, MutexGuard, TryLockError},
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, ProofHelperLaunchError>;
type Child = RetainedChild<Payload>;

// Published in the original cleanup slot before clone. In particular, failure
// before READY cannot retire the original received rights with a live helper.
pub(crate) struct Payload {
    backing: Backing,
    _received: Arc<Receiver>,
}

impl Payload {
    pub(crate) fn storage(backing: usize) -> Result<usize> {
        native::sum(&[
            backing,
            Receiver::TRANSPORT_STORAGE,
            size_of::<Self>() - size_of::<Backing>(),
        ])
        .map_err(Into::into)
    }
}
const BOOTSTRAP_FD: i32 = 3;
const BINDING_STORAGE: usize = size_of::<[Binding<'static>; 1]>();
const ENTRY_WORK: usize = 8;
// Local descriptor/scalar work only. Existing image/backing, stage, child,
// transport, record and cleanup APIs debit every nested operation separately.
pub(crate) const LOCAL_WORK: usize = ENTRY_WORK + 192 * 1088;
pub(crate) const FRAME: usize = 4 * size_of::<(ManagedProofHelper, usize)>()
    + 4 * size_of::<Stage>()
    + 8 * size_of::<ProofHelperLaunchError>()
    + 8 * size_of::<rustix::fs::Stat>()
    + 4 * size_of::<(Record, RecordStorage)>()
    + 4 * WIRE
    + BINDING_STORAGE
    + launch_io::ATTEMPT_SCRATCH
    + 8192;

#[derive(Debug)]
pub(crate) enum ProofHelperLaunchError {
    Resource(Resource),
    Backing(BackingError),
    Native(NativeError),
    Record(RecordError),
    Inventory(crate::native_runtime_inventory::Error),
    Descriptor(crate::native_runtime_descriptors::Error),
    Runtime(crate::native_runtime_guard::Error),
    Invalid(&'static str),
}
macro_rules! errors {
    ($($ty:ty => $variant:ident),+ $(,)?) => {$(
        impl From<$ty> for ProofHelperLaunchError {
            fn from(e: $ty) -> Self { Self::$variant(e) }
        }
    )+};
}
errors!(Resource => Resource, BackingError => Backing, NativeError => Native, RecordError => Record);
errors!(crate::native_runtime_inventory::Error => Inventory);
errors!(crate::native_runtime_descriptors::Error => Descriptor);
errors!(crate::native_runtime_guard::Error => Runtime);
impl From<SpawnError> for ProofHelperLaunchError {
    fn from(e: SpawnError) -> Self {
        Self::Native(NativeError::Spawn(e))
    }
}
impl From<AccessError> for ProofHelperLaunchError {
    fn from(e: AccessError) -> Self {
        Self::Native(NativeError::Retained(e))
    }
}
impl From<launch_io::Failure> for ProofHelperLaunchError {
    fn from(e: launch_io::Failure) -> Self {
        Self::Native(NativeError::Transport(e))
    }
}
impl From<launch_io::Error<ProofHelperLaunchError>> for ProofHelperLaunchError {
    fn from(e: launch_io::Error<Self>) -> Self {
        match e {
            launch_io::Error::Failure(e) => e.into(),
            launch_io::Error::Observer(e) => e,
        }
    }
}
impl fmt::Display for ProofHelperLaunchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Backing(e) => e.fmt(f),
            Self::Native(e) => e.fmt(f),
            Self::Record(e) => e.fmt(f),
            Self::Inventory(e) => e.fmt(f),
            Self::Descriptor(e) => e.fmt(f),
            Self::Runtime(e) => e.fmt(f),
            Self::Invalid(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for ProofHelperLaunchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Backing(e) => Some(e),
            Self::Native(e) => Some(e),
            Self::Record(e) => Some(e),
            Self::Runtime(e) => Some(e),
            Self::Inventory(e) => Some(e),
            Self::Descriptor(e) => Some(e),
            Self::Invalid(_) => None,
        }
    }
}

/// Complete native child custody; no proof RPC, receipt or public authority API.
/// Child drops first, so its prepaid cancellation/defer precedes bootstrap close.
/// Pending/Quarantined cancellation retains the backing in the ORIGINAL pool.
pub(crate) struct ManagedProofHelper {
    child: Mutex<HelperChild>,
    bootstrap: OwnedFd,
    session: [u8; 32],
    runtime: [u8; 32],
    retained: usize,
}

#[derive(Debug, Eq, PartialEq)]
enum Phase {
    Ready,
    Finishing,
    Closed,
}
impl Phase {
    fn require_ready(&self) -> Result<()> {
        match self {
            Self::Ready => Ok(()),
            Self::Finishing | Self::Closed => Err(ProofHelperLaunchError::Invalid(
                "proof helper lifecycle is closed",
            )),
        }
    }
    fn begin_finish(&mut self) -> Result<()> {
        self.require_ready()?;
        *self = Self::Finishing;
        Ok(())
    }
    fn close(&mut self) {
        *self = Self::Closed;
    }
}

struct HelperChild {
    child: Child,
    phase: Phase,
}
impl HelperChild {
    fn cancel(&mut self) -> CleanupPoll {
        self.phase.close();
        // Native cancellation caches its disposition and preserves the original
        // slot on Pending/Quarantined. Keep the typed backing even after Reaped.
        self.child.cancel()
    }
}

struct FinishAttempt<'a>(&'a mut HelperChild);
impl FinishAttempt<'_> {
    fn child(&self) -> &Child {
        &self.0.child
    }
    fn cancel(&mut self) -> CleanupPoll {
        self.0.cancel()
    }
}
impl Drop for FinishAttempt<'_> {
    fn drop(&mut self) {
        let _ = self.0.cancel();
    }
}

impl ManagedProofHelper {
    pub(crate) const ENVELOPE: usize =
        size_of::<(Self, usize)>() - size_of::<Child>() - size_of::<OwnedFd>();

    pub(crate) const fn retained_storage(&self) -> usize {
        self.retained
    }

    #[cfg(test)]
    pub(crate) fn pid_for_test(&self) -> Pid {
        let child = self.lock_child().unwrap();
        child.phase.require_ready().unwrap();
        child.child.pid()
    }

    /// Scoped access to the fixed compiler owner while this lifecycle is ready.
    /// The callback receives the SAME Budget and funds its own work and outputs.
    /// No backing reference may escape; reserve returned growth after this call.
    /// Reentry or concurrent access refuses without waiting for this mutex.
    pub(crate) fn with_compiler<R, E>(
        &self,
        b: &mut Budget<'_>,
        operation: impl FnOnce(&Compiler, &mut Budget<'_>) -> std::result::Result<R, E>,
    ) -> std::result::Result<R, E>
    where
        E: From<ProofHelperLaunchError>,
    {
        lifecycle_scope(self.retained, b, |b| {
            let child = self.lock_child()?;
            child.phase.require_ready()?;
            child.child.with_resources(b, |backing, b| -> Result<_> {
                let credentials = validate_backing(&backing.backing, self.runtime, b)?;
                validate_process(&child.child, credentials, b)?;
                if !child.child.is_live(b)? {
                    return Err(
                        launch_io::Failure::ChildExited("proof helper backing access").into(),
                    );
                }
                Ok(operation(backing.backing.compiler(), b))
            })
        })
        .map_err(E::from)?
    }

    /// Borrow the same compiler for a stopped-task checkpoint. Complete content
    /// validation belongs at capture and ownership transitions; the controller
    /// must retain immutable backing and inspect its original inventory objects.
    /// This checks helper lifecycle/custody, not compiler or device admission.
    pub(crate) fn with_compiler_checkpoint<R, E>(
        &self,
        b: &mut Budget<'_>,
        operation: impl FnOnce(&Compiler, &mut Budget<'_>) -> std::result::Result<R, E>,
    ) -> std::result::Result<R, E>
    where
        E: From<ProofHelperLaunchError>,
    {
        lifecycle_scope(self.retained, b, |b| {
            let child = self.lock_child()?;
            child.phase.require_ready()?;
            child.child.with_resources(b, |backing, b| -> Result<_> {
                if backing.backing.runtime_identity() != self.runtime {
                    return Err(ProofHelperLaunchError::Invalid(
                        "checkpoint helper runtime association changed",
                    ));
                }
                validate_process(&child.child, backing.backing.credentials(), b)?;
                if !child.child.is_live(b)? {
                    return Err(launch_io::Failure::ChildExited("proof helper checkpoint").into());
                }
                Ok(operation(backing.backing.compiler(), b))
            })
        })
        .map_err(E::from)?
    }

    fn lock_child(&self) -> Result<MutexGuard<'_, HelperChild>> {
        match self.child.try_lock() {
            Ok(child) => Ok(child),
            Err(TryLockError::WouldBlock) => Err(ProofHelperLaunchError::Invalid(
                "proof helper lifecycle is busy",
            )),
            Err(TryLockError::Poisoned(_)) => Err(AccessError::Poisoned.into()),
        }
    }

    /// One prepaid cancellation step, callable through retained shared custody.
    /// Repeated calls return the native cached disposition, never another step.
    /// Poison recovery is ONLY for cancellation; it cannot reopen backing access.
    pub(crate) fn cancel(&self) -> Result<CleanupPoll> {
        let mut child = match self.child.try_lock() {
            Ok(child) => child,
            Err(TryLockError::Poisoned(error)) => error.into_inner(),
            Err(TryLockError::WouldBlock) => {
                return Err(ProofHelperLaunchError::Invalid(
                    "proof helper lifecycle is busy",
                ));
            }
        };
        Ok(child.cancel())
    }

    /// One-use handshake followed by one exclusive, prepaid cancellation step.
    /// Once started, refusal or unwind also cancels and permanently closes access.
    /// Only Reaped proves both terminal consuming wait and aggregate domain cleanup.
    /// It does NOT prove a graceful exit or status zero. Pending/Quarantined keep
    /// the complete unresolved record/backing in the existing pool. This view also
    /// keeps its full backing and reservation until its enclosing owner drops it.
    /// The shared EOF scheduler may conservatively refuse a concurrent terminal
    /// race; no terminal observation is substituted for actual bootstrap EOF.
    pub(crate) fn finish(&self, timeout: Duration, b: &mut Budget<'_>) -> Result<CleanupPoll> {
        let mut child = self.lock_child()?;
        child.phase.begin_finish()?;
        // Install before budget/deadline admission: the old consuming finish also
        // cancelled on those refusals. No backing/pool lock survives into Drop.
        let mut attempt = FinishAttempt(&mut child);
        lifecycle_scope(self.retained, b, |b| {
            let deadline = launch_io::bounded_deadline(timeout)?;
            let child = attempt.child();
            let credentials = child.with_resources(b, |backing, b| {
                validate_backing(&backing.backing, self.runtime, b)
            })?;
            validate_process(child, credentials, b)?;
            send_record(
                child,
                self.bootstrap.as_fd(),
                Kind::Finish,
                self.session,
                self.runtime,
                b,
                deadline,
            )?;
            receive_record(
                child,
                self.bootstrap.as_fd(),
                credentials,
                Kind::Finished,
                self.session,
                self.runtime,
                b,
                deadline,
            )?;
            launch_io::await_exec_eof(
                self.bootstrap.as_fd(),
                &mut Observer { child, budget: b },
                deadline,
            )?;
            ensure_deadline(deadline, "proof helper terminal handshake")?;
            Ok(attempt.cancel())
        })
    }
}

impl Drop for ManagedProofHelper {
    fn drop(&mut self) {
        // Exclusive destruction needs no lock and cannot lose poisoned custody.
        let child = self.child.get_mut().unwrap_or_else(|e| e.into_inner());
        let _ = child.cancel();
    }
}

fn lifecycle_scope<T>(
    retained: usize,
    b: &mut Budget<'_>,
    operation: impl FnOnce(&mut Budget<'_>) -> Result<T>,
) -> Result<T> {
    b.with_prepaid_scope(retained, ENTRY_WORK, LOCAL_WORK, FRAME, operation)
}

fn validate_backing(
    backing: &Backing,
    runtime: [u8; 32],
    b: &mut Budget<'_>,
) -> Result<Credentials> {
    backing.revalidate(b)?;
    if backing.runtime_identity() != runtime {
        return Err(ProofHelperLaunchError::Invalid(
            "proof helper runtime association changed",
        ));
    }
    Ok(backing.credentials())
}

/// Launches only the helper selected and sealed by the consumed actual backing.
/// Keep its original FULL request reservation and reserve returned growth before
/// retaining the result. Nested operations use this same budget/address/ledger.
/// Cleanup must be the original funded native pool; spawn reserves its full
/// backing payload and fresh namespace/domain custody before cloning. No fresh
/// resource account is created here. All refusal/unwind paths keep native cleanup.
///
/// # Safety
/// The caller must independently establish host-root deployment provenance, the
/// actual helper/peer deployment-role binding and a dedicated creator thread that
/// remains alive until aggregate cleanup. V2 approval supplies helper credentials,
/// but numeric root and configuration do not establish deployment provenance. Retain
/// an outside whole-domain custodian, exclusive child wait ownership and the
/// original pool/controller through unresolved cleanup. Exclude concurrent FD,
/// credential, signal, namespace/map, cgroup and approved backing mutations.
/// All Stage::spawn_retaining_in_fresh_user_namespace obligations apply: no child
/// cgroup controls/delegation/escape, caller-funded consuming input Drop, and no
/// competing reapers. Do not move the root into the child's containment domain.
/// A successful return is bootstrap custody ONLY, never proof/process admission.
#[allow(unsafe_code)]
pub(crate) unsafe fn launch(
    backing: Backing,
    received: Arc<Receiver>,
    session: [u8; 32],
    peer: Credentials,
    timeout: Duration,
    cleanup: &mut Cleanup,
    b: &mut Budget<'_>,
) -> Result<(ManagedProofHelper, usize)> {
    let input = Payload::storage(backing.retained_storage())?;
    b.with_prepaid_scope(input, ENTRY_WORK, LOCAL_WORK, FRAME, |b| {
        let deadline = launch_io::bounded_deadline(timeout)?;
        if session == [0; 32] {
            return Err(ProofHelperLaunchError::Invalid("zero proof helper session"));
        }
        backing.revalidate(b)?;
        let runtime = backing.runtime_identity();
        if runtime == [0; 32] {
            return Err(ProofHelperLaunchError::Invalid("zero proof helper runtime"));
        }
        let credentials = backing.credentials();
        b.reserve_storage(Channels::STORAGE)?;
        let channels = helper_channels()?;
        let (stage, stage_charge) = stage_backing(&backing, &channels, b)?;
        b.reserve_storage(stage_charge)?;
        validate_stage(&backing, &stage, &channels, b)?;
        ensure_deadline(deadline, "proof helper staging")?;
        // SAFETY: the fixed FD3 table and sealed image passed final contextual
        // validation. The whole actual backing is prepublished in the original
        // funded pool before clone. Caller supplies the documented deployment,
        // creator, outside-custodian, role binding and mutation/wait exclusions.
        let (child, charge) = unsafe {
            stage.spawn_retaining_in_fresh_user_namespace(
                credentials,
                peer,
                Payload {
                    backing,
                    _received: received,
                },
                input,
                cleanup,
                b,
            )
        }?;
        let mut child = child;
        b.reserve_storage(charge.additional_storage())?;
        launch_io::await_profile_ready(
            channels.profile_reader.as_fd(),
            channels.exec_reader.as_fd(),
            &mut Observer {
                child: &child,
                budget: b,
            },
            deadline,
        )?;
        child.with_resources(b, |backing, b| {
            validate_stage(&backing.backing, &stage, &channels, b)
        })?;
        validate_process(&child, credentials, b)?;
        // Keep the parent's gate reader until after write, even if the child died.
        launch_io::release_child(
            channels.gate_writer.as_fd(),
            &mut Observer {
                child: &child,
                budget: b,
            },
            deadline,
        )?;
        drop(stage);
        b.release_storage(stage_charge)?;
        let Channels {
            root,
            child: child_end,
            exec_reader,
            exec_writer,
            profile_reader,
            profile_writer,
            gate_reader,
            gate_writer,
        } = channels;
        drop(child_end);
        drop(exec_writer);
        drop(profile_writer);
        drop(profile_reader);
        drop(gate_reader);
        drop(gate_writer);
        b.release_storage(6 * native::FILE_STORAGE)?;
        // The persistent FD3 socket cannot stand in for CLOEXEC stage reporting.
        launch_io::await_exec_eof(
            exec_reader.as_fd(),
            &mut Observer {
                child: &child,
                budget: b,
            },
            deadline,
        )?;
        drop(exec_reader);
        b.release_storage(native::FILE_STORAGE)?;
        send_record(
            &child,
            root.as_fd(),
            Kind::Initial,
            session,
            runtime,
            b,
            deadline,
        )?;
        receive_record(
            &child,
            root.as_fd(),
            credentials,
            Kind::Ready,
            session,
            runtime,
            b,
            deadline,
        )?;
        child.with_resources(b, |backing, b| {
            validate_backing(&backing.backing, runtime, b)
        })?;
        validate_process(&child, credentials, b)?;
        if !child.is_live(b)? {
            return Err(launch_io::Failure::ChildExited("proof helper ready").into());
        }
        ensure_deadline(deadline, "proof helper final validation")?;
        // SAFETY: independent CLOEXEC exec EOF precedes exact child-credentialed
        // READY/session/runtime. Final real profile/liveness/backing checks passed;
        // all parent stage/status writer aliases are closed. Only this owner waits.
        unsafe {
            child.confirm_exec(b)?;
        }
        ensure_deadline(deadline, "proof helper confirmed exec")?;
        let retained = native::sum(&[
            child.retained_storage(),
            native::FILE_STORAGE,
            ManagedProofHelper::ENVELOPE,
        ])?;
        let growth = retained.checked_sub(input).ok_or(Resource::Accounting)?;
        b.reserve_storage(ManagedProofHelper::ENVELOPE)?;
        Ok((
            ManagedProofHelper {
                child: Mutex::new(HelperChild {
                    child,
                    phase: Phase::Ready,
                }),
                bootstrap: root,
                session,
                runtime,
                retained,
            },
            growth,
        ))
    })
}

// The shared Channels constructor enables root reception. The helper independently
// requires child reception to be enabled before clone, not after its first send.
fn helper_channels() -> Result<Channels> {
    let channels = Channels::new()?;
    net::sockopt::set_socket_passcred(&channels.child, true)
        .map_err(|e| native::io("enable helper bootstrap credentials", e))?;
    Ok(channels)
}

fn source_storage(measurement: Measurement) -> Result<usize> {
    native::sum(&[
        Image::file_storage(measurement).map_err(NativeError::Executable)?,
        4 * native::FILE_STORAGE,
        BINDING_STORAGE,
    ])
    .map_err(Into::into)
}

#[allow(unsafe_code)]
fn stage_backing(
    backing: &Backing,
    channels: &Channels,
    b: &mut Budget<'_>,
) -> Result<(Stage, usize)> {
    let input = native::sum(&[backing.retained_storage(), Channels::STORAGE])?;
    b.with_prepaid_scope(input, ENTRY_WORK, LOCAL_WORK, FRAME, |b| {
        let (image, charge) = backing.try_clone_for_exec(b)?;
        b.reserve_storage(charge.additional_storage())?;
        let bindings = [Binding::new(channels.child.as_fd(), BOOTSTRAP_FD)
            .map_err(|_| ProofHelperLaunchError::Invalid("invalid proof helper FD3 binding"))?];
        b.reserve_storage(BINDING_STORAGE)?;
        // SAFETY: these concrete sources include the FULL sealed executable,
        // bootstrap/profile/gate/status FDs and binding array. Their owners remain
        // charged through final staged validation; no caller-supplied source table.
        let (stage, charge) = unsafe {
            Stage::stage(
                &image,
                &bindings,
                channels.profile_writer.as_fd(),
                channels.gate_reader.as_fd(),
                channels.exec_writer.as_fd(),
                source_storage(backing.measurement())?,
                b,
            )
        }?;
        b.reserve_storage(charge.additional_storage())?;
        validate_stage(backing, &stage, channels, b)?;
        Ok((stage, charge.additional_storage()))
    })
}

fn validate_stage(
    backing: &Backing,
    stage: &Stage,
    channels: &Channels,
    b: &mut Budget<'_>,
) -> Result<()> {
    backing.revalidate_exec_clone(stage.executable(), b)?;
    let bootstrap = stage
        .binding(BOOTSTRAP_FD)
        .ok_or(ProofHelperLaunchError::Invalid("missing proof helper FD3"))?;
    validate_bootstrap(bootstrap, channels)
}

fn validate_bootstrap(staged: &File, channels: &Channels) -> Result<()> {
    let original = rustix::fs::fstat(&channels.child)
        .map_err(|e| native::io("inspect helper bootstrap", e))?;
    let actual =
        rustix::fs::fstat(staged).map_err(|e| native::io("inspect staged helper bootstrap", e))?;
    if (original.st_dev, original.st_ino, original.st_mode)
        != (actual.st_dev, actual.st_ino, actual.st_mode)
    {
        return Err(ProofHelperLaunchError::Invalid(
            "proof helper bootstrap object changed",
        ));
    }
    for fd in [
        channels.root.as_fd(),
        channels.child.as_fd(),
        staged.as_fd(),
    ] {
        let flags = rustix::fs::fcntl_getfl(fd)
            .map_err(|e| native::io("inspect helper bootstrap flags", e))?;
        let forbidden = rustix::fs::OFlags::APPEND
            | rustix::fs::OFlags::ASYNC
            | rustix::fs::OFlags::DIRECT
            | rustix::fs::OFlags::PATH;
        if rustix::io::fcntl_getfd(fd)
            .map_err(|e| native::io("inspect helper bootstrap FD flags", e))?
            != FdFlags::CLOEXEC
            || flags & rustix::fs::OFlags::ACCMODE != rustix::fs::OFlags::RDWR
            || !flags.contains(rustix::fs::OFlags::NONBLOCK)
            || flags.intersects(forbidden)
            || !net::sockopt::socket_passcred(fd)
                .map_err(|e| native::io("inspect helper bootstrap credentials", e))?
            || net::sockopt::socket_type(fd)
                .map_err(|e| native::io("inspect helper bootstrap type", e))?
                != net::SocketType::SEQPACKET
        {
            return Err(ProofHelperLaunchError::Invalid(
                "proof helper bootstrap flags changed",
            ));
        }
    }
    Ok(())
}

fn validate_process(child: &Child, credentials: Credentials, b: &mut Budget<'_>) -> Result<()> {
    b.with_prepaid_scope(
        child.retained_storage(),
        0,
        observations::PROCESS_VALIDATE_WORK,
        observations::PROCESS_VALIDATE_SCRATCH,
        |_| {
            observations::validate_process(credentials, child.pid())
                .map_err(NativeError::Observation)?;
            Ok(())
        },
    )
}

struct Observer<'a, 'w> {
    child: &'a Child,
    budget: &'a mut Budget<'w>,
}
impl launch_io::Observer for Observer<'_, '_> {
    type Error = ProofHelperLaunchError;
    fn before_attempt(&mut self, boundary: launch_io::Boundary) -> Result<()> {
        Ok(self.budget.charge_work(boundary.work())?)
    }
    fn is_live(&mut self) -> Result<bool> {
        Ok(self.child.is_live(self.budget)?)
    }
}

fn ensure_deadline(deadline: Instant, phase: &'static str) -> Result<()> {
    if Instant::now() >= deadline {
        return Err(launch_io::Failure::Timeout(phase).into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn send_record(
    child: &Child,
    bootstrap: BorrowedFd<'_>,
    kind: Kind,
    session: [u8; 32],
    runtime: [u8; 32],
    b: &mut Budget<'_>,
    deadline: Instant,
) -> Result<()> {
    let (record, charge) = Record::new(
        kind,
        native::pid_u32(rustix::process::getpid())?,
        session,
        runtime,
        b,
    )?;
    b.reserve_storage(charge.additional_storage())?;
    launch_io::send_ready(
        bootstrap,
        record.canonical_bytes(),
        &mut Observer { child, budget: b },
        deadline,
    )?;
    drop(record);
    b.release_storage(charge.additional_storage())?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn receive_record(
    child: &Child,
    bootstrap: BorrowedFd<'_>,
    credentials: Credentials,
    kind: Kind,
    session: [u8; 32],
    runtime: [u8; 32],
    b: &mut Budget<'_>,
    deadline: Instant,
) -> Result<()> {
    b.reserve_storage(WIRE)?;
    let sender = launch_io::MessageSender::new(
        child.pid().as_raw_pid(),
        credentials.uid(),
        credentials.gid(),
    );
    let (bytes, unexpected) = launch_io::receive_ready_from::<WIRE, false, _>(
        bootstrap,
        sender,
        &mut Observer { child, budget: b },
        deadline,
    )?;
    if unexpected.is_some() {
        return Err(ProofHelperLaunchError::Invalid(
            "proof helper sent a descriptor",
        ));
    }
    check_record(&bytes, kind, child.pid(), session, runtime, b)?;
    b.release_storage(WIRE)?;
    Ok(())
}

fn check_record(
    bytes: &[u8; WIRE],
    kind: Kind,
    pid: Pid,
    session: [u8; 32],
    runtime: [u8; 32],
    b: &mut Budget<'_>,
) -> Result<()> {
    let (record, charge) = Record::decode(bytes, b)?;
    b.reserve_storage(charge.additional_storage())?;
    let matches = record.matches_association(kind, native::pid_u32(pid)?, session, runtime, b)?;
    drop(record);
    b.release_storage(charge.additional_storage())?;
    if !matches {
        return Err(ProofHelperLaunchError::Invalid(
            "proof helper bootstrap association mismatch",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "proof_helper_launch_tests.rs"]
mod tests;
