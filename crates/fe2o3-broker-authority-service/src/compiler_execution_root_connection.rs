//! Original-root association and the closed measured-issuer admission handshake.
//! Publication custody, retirement and the production attempt remain separate.
use crate::compiler_execution_root_exchange::{
    RootControlReplayErrorV3 as ReplayError, RootControlReplayWindowV3 as Replay,
};
use crate::{
    RootIssuerImageErrorV3 as ImageError, RootLaunchChannelErrorV3 as ChannelError,
    RootLaunchChannelV3 as Channel, retained_issuer_image_quota_v3,
    validate_retained_issuer_image_v3,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_ROOT_CONTROL_BYTES_V3 as BYTES,
    COMPILER_EXECUTION_ROOT_CONTROL_STORAGE_V3 as CODEC_SCRATCH,
    COMPILER_EXECUTION_ROOT_CONTROL_WORK_V3 as CODEC_WORK,
    COMPILER_EXECUTION_ROOT_GATE_STORAGE_V3 as GATE_SCRATCH,
    COMPILER_EXECUTION_ROOT_GATE_WORK_V3 as GATE_WORK,
    COMPILER_EXECUTION_SERVICE_READY_STORAGE_V3 as READY_SCRATCH,
    COMPILER_EXECUTION_SERVICE_READY_WORK_V3 as READY_WORK,
    CompilerExecutionIssuerPolicyV3 as Policy, CompilerExecutionRootControlBindingV3 as Binding,
    CompilerExecutionRootControlErrorV3 as ProtocolError,
    CompilerExecutionRootControlRecordV3 as Record,
    CompilerExecutionServiceLaunchManifestV3 as Manifest,
    CompilerExecutionServiceReadyErrorV3 as ReadyError, CompilerExecutionServiceReadyV3 as Ready,
    compiler_execution_root_gate_request_v3 as gate_request,
    validate_compiler_execution_root_gate_reply_v3 as validate_gate_reply,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use fe2o3_protected_service_profile::{
    ProtectedServiceCredentialProfileV1 as Credentials,
    ProtectedServiceNamespaceSetV2 as Namespaces, ProtectedServiceProfileErrorV2 as ProfileError,
    observations,
};
use fe2o3_protected_service_spawn::{
    launch_io,
    native_spawn::{
        ProtectedServiceSpawnErrorV2 as SpawnError, RootOwnedProtectedServiceChildV2 as PlainChild,
        RootOwnedRetainedServiceChildV2 as Child, RootTaskIdentityV2 as RootIdentity,
        RootTaskObservationV2 as Original,
    },
    require_exact_root_identity_v1,
};
use rustix::{event, io, process};
use std::{
    fmt,
    marker::PhantomData,
    mem::size_of,
    os::fd::OwnedFd,
    rc::Rc,
    time::{Duration, Instant},
};

const ENTRY: usize = 8;
const LOCAL_WORK: usize = ENTRY + 64 * 1088;
const FRAME: usize = 4 * size_of::<RootControlSessionV3<'static>>()
    + 4 * size_of::<RootConnectionV3<'static>>()
    + 8 * size_of::<RootConnectionErrorV3>()
    + 8192;
const FD_STORAGE: usize = size_of::<(OwnedFd, usize)>();
const PAUSE: event::Timespec = event::Timespec {
    tv_sec: 0,
    tv_nsec: 1_000_000,
};

/// Full unreserved retained charge, not a delta above consumed channel storage.
#[derive(Debug)]
pub struct RootConnectionStorageV3(usize);
impl RootConnectionStorageV3 {
    pub const fn additional_storage(&self) -> usize {
        self.0
    }
}

/// Move-only root-attempt association. It retains an actual trace allocation and
/// fresh epoch independently of replaceable issuer connections. It does NOT yet
/// own a publication occurrence or implement retirement/recovery.
/// Keep its full charge, original Work and Budget address through destruction.
/// No namespace, epoch, identity or authority is accepted from decoded input.
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::RootControlSessionV3 as S;
/// fn clone(s: S<'_>) { let _ = s.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::RootControlSessionV3 as S;
/// fn send<T: Send>() {} send::<S<'static>>();
/// ```
pub struct RootControlSessionV3<'work> {
    original: RootIdentity,
    namespaces: Namespaces,
    epoch: [u8; 32],
    ledger: Ledger,
    budget_address: usize,
    process: process::Pid,
    thread: process::Pid,
    retained: usize,
    _work: PhantomData<(&'work Work, Rc<()>)>,
}

impl<'work> RootControlSessionV3<'work> {
    pub const CREATE_WORK: usize = LOCAL_WORK + Original::IDENTITY_WORK + Namespaces::CAPTURE_WORK;
    pub const CREATE_SCRATCH: usize =
        FRAME + Original::IDENTITY_SCRATCH + Namespaces::CAPTURE_SCRATCH;
    pub const VALIDATE_WORK: usize =
        LOCAL_WORK + Original::IDENTITY_WORK + Namespaces::REVALIDATE_SELF_WORK;
    pub const VALIDATE_SCRATCH: usize =
        FRAME + Original::IDENTITY_SCRATCH + Namespaces::REVALIDATE_SELF_SCRATCH;

    pub fn create(
        original: &Original<'_, 'work>,
        b: &mut Budget<'work>,
    ) -> Result<(Self, RootConnectionStorageV3)> {
        b.with_prepaid_scope(original.retained_storage(), ENTRY, LOCAL_WORK, FRAME, |b| {
            require_root()?;
            let (identity, charge) = original.retain_identity(b)?;
            b.reserve_storage(charge.additional_storage())?;
            let (namespaces, charge) = Namespaces::capture_self(b)?;
            b.reserve_storage(charge.additional_storage())?;
            let retained = sum(&[
                size_of::<(Self, RootConnectionStorageV3)>(),
                identity.retained_storage(),
                namespaces.retained_storage(),
            ])?;
            Ok((
                Self {
                    original: identity,
                    namespaces,
                    epoch: nonce()?,
                    ledger: b.work_ledger_identity_v1(),
                    budget_address: b as *const Budget<'_> as usize,
                    process: process::getpid(),
                    thread: rustix::thread::gettid(),
                    retained,
                    _work: PhantomData,
                },
                RootConnectionStorageV3(retained),
            ))
        })
    }

    pub const fn retained_storage(&self) -> usize {
        self.retained
    }

    pub fn validate_original(&self, original: &Original<'_, '_>, b: &mut Budget<'_>) -> Result<()> {
        let floor = sum(&[self.retained, original.retained_storage()])?;
        b.with_prepaid_scope(floor, ENTRY, LOCAL_WORK, FRAME, |b| {
            self.check_account(b)?;
            let (identity, charge) = original.retain_identity(b)?;
            b.reserve_storage(charge.additional_storage())?;
            if !self.original.matches(&identity) {
                return Err(Error::Refused("different original root trace"));
            }
            self.namespaces.revalidate_self(b)?;
            Ok(())
        })
    }

    /// Run only after the actual private readiness frame AND writer EOF, before
    /// compiler resume. The caller supplies retained deployment policy/manifest
    /// and credentials, not claims received from the issuer. Original trace and
    /// actual issuer custody, namespaces, profile and running image are checked
    /// here. A fresh connection challenge is generated AFTER image validation,
    /// excluding queued pre-exec responses; only the measured issuer's closed
    /// post-Admission/post-readiness gate can answer it.
    ///
    /// The channel must have been created after compiler clone, staged exclusively
    /// to this issuer, with ALL parent/stage issuer aliases already closed. These
    /// launch-history obligations cannot be inferred from inert readiness bytes.
    /// Permission denial refuses, without a PID reopen or authority provider.
    /// Failure consumes the channel; caller must cancel the still-held compiler
    /// and issuer using their original funded cleanup owners. No occurrence is
    /// acquired or retired by this method. Returned storage is FULL unreserved.
    #[allow(clippy::too_many_arguments)]
    pub fn connect_after_readiness<T: Send + 'static>(
        &self,
        original: &Original<'_, 'work>,
        issuer: &Child<T>,
        policy: &Policy,
        manifest: &Manifest,
        credentials: Credentials,
        ready: &Ready,
        channel: Channel<'work>,
        timeout: Duration,
        b: &mut Budget<'work>,
    ) -> Result<(RootConnectionV3<'work>, RootConnectionStorageV3)> {
        let inputs = issuer.retained_storage().max(sum(&[
            policy.retained_storage(),
            manifest.retained_storage(),
        ])?);
        let floor = sum(&[
            self.retained,
            original.retained_storage(),
            inputs,
            ready.retained_storage(),
            channel.retained_storage(),
        ])?;
        b.with_prepaid_scope(floor, ENTRY, LOCAL_WORK, FRAME, |b| {
            self.validate_original(original, b)?;
            channel.validate_root_endpoint(b)?;
            if manifest.client().pid() != original.pid().as_raw_pid() as u32
                || !ready.matches_launch(issuer.pid().as_raw_pid() as u32, manifest, policy, b)?
            {
                return Err(Error::Refused(
                    "root control readiness or original compiler mismatch",
                ));
            }
            let deadline = launch_io::bounded_deadline(timeout).map_err(Error::Transport)?;
            let (pidfd, charge) = issuer.try_clone_pidfd(b)?;
            b.reserve_storage(charge.additional_storage())?;
            validate_peer(self, issuer, issuer.pid(), &pidfd, credentials, b)?;
            validate_retained_issuer_image_v3(issuer, policy, b)?;
            let (binding, charge) = Binding::new(policy, manifest, self.epoch, nonce()?, b)?;
            b.reserve_storage(charge.additional_storage())?;
            let (request, charge) = gate_request(&binding, b)?;
            b.reserve_storage(charge.additional_storage())?;
            let sender = launch_io::MessageSender::new(
                issuer.pid().as_raw_pid(),
                credentials.uid(),
                credentials.gid(),
            );
            let mut sent = false;
            let mut complete = false;
            for _ in 0..RootConnectionV3::MAX_HANDSHAKE_ATTEMPTS {
                b.with_prepaid_scope(floor, ENTRY, LOCAL_WORK, FRAME, |b| -> Result<()> {
                    check_deadline(deadline)?;
                    self.validate_original(original, b)?;
                    validate_peer(self, issuer, issuer.pid(), &pidfd, credentials, b)?;
                    if !sent {
                        sent = channel.send_packet(request.canonical_bytes(), b)?.is_some();
                    } else if let Some(bytes) = channel.receive_packet(sender, b)? {
                        b.reserve_storage(BYTES)?;
                        let (reply, charge) = Record::decode(&bytes, b)?;
                        b.reserve_storage(charge.additional_storage())?;
                        validate_gate_reply(&reply, &request, b)?;
                        complete = true;
                    }
                    Ok(())
                })?;
                if complete {
                    break;
                }
                // One bounded pause, no EINTR retry and no unmetered sleep loop.
                event::poll(&mut [], Some(&PAUSE))?;
            }
            if !complete {
                return Err(Error::Refused("root control handshake attempt limit"));
            }
            self.validate_original(original, b)?;
            validate_peer(self, issuer, issuer.pid(), &pidfd, credentials, b)?;
            validate_retained_issuer_image_v3(issuer, policy, b)?;
            check_deadline(deadline)?;
            let (replay, charge) = Replay::new(binding, b)?;
            b.reserve_storage(charge.additional_storage())?;
            let retained = sum(&[
                size_of::<(RootConnectionV3<'_>, RootConnectionStorageV3)>(),
                channel.retained_storage(),
                FD_STORAGE,
                request.retained_storage(),
                replay.retained_storage(),
            ])?;
            Ok((
                RootConnectionV3 {
                    channel,
                    replay,
                    request,
                    pidfd,
                    issuer: issuer.pid(),
                    credentials,
                    epoch: self.epoch,
                    retained,
                },
                RootConnectionStorageV3(retained),
            ))
        })
    }

    fn check_account(&self, b: &Budget<'_>) -> Result<()> {
        if self.ledger != b.work_ledger_identity_v1()
            || self.budget_address != b as *const Budget<'_> as usize
            || self.process != process::getpid()
            || self.thread != rustix::thread::gettid()
        {
            return Err(Resource::Accounting.into());
        }
        require_root()
    }
}

/// An actual challenge-completed issuer connection, not compiler occurrence,
/// receipt, proof or GPU authority. No arbitrary-FD/PID/receipt constructor exists.
/// Drop closes transport only; a future root-owned occurrence/tombstone must
/// remain outside this replaceable owner. Revalidate before every later use.
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::RootConnectionV3 as C;
/// fn fake(fd: std::os::fd::OwnedFd) { let _ = C::from_fd(fd); }
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::RootConnectionV3 as C;
/// fn send<T: Send>() {} send::<C<'static>>();
/// ```
pub struct RootConnectionV3<'work> {
    channel: Channel<'work>,
    replay: Replay<'work>,
    request: Record,
    pidfd: OwnedFd,
    issuer: process::Pid,
    credentials: Credentials,
    epoch: [u8; 32],
    retained: usize,
}
impl RootConnectionV3<'_> {
    pub const MAX_HANDSHAKE_ATTEMPTS: usize = launch_io::MAX_PHASE_ATTEMPTS;
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }

    pub fn validate<T: Send + 'static>(
        &self,
        root: &RootControlSessionV3<'_>,
        original: &Original<'_, '_>,
        issuer: &Child<T>,
        policy: &Policy,
        manifest: &Manifest,
        b: &mut Budget<'_>,
    ) -> Result<()> {
        let inputs = issuer.retained_storage().max(sum(&[
            policy.retained_storage(),
            manifest.retained_storage(),
        ])?);
        let floor = sum(&[
            self.retained,
            root.retained,
            original.retained_storage(),
            inputs,
        ])?;
        b.with_prepaid_scope(floor, ENTRY, LOCAL_WORK, FRAME, |b| {
            root.validate_original(original, b)?;
            if self.epoch != root.epoch || !self.request.matches_launch(policy, manifest, b)? {
                return Err(Error::Refused("root connection association changed"));
            }
            self.channel.validate_root_endpoint(b)?;
            validate_peer(root, issuer, self.issuer, &self.pidfd, self.credentials, b)?;
            validate_retained_issuer_image_v3(issuer, policy, b)?;
            Ok(())
        })
    }
}

fn validate_peer<T: Send + 'static>(
    root: &RootControlSessionV3<'_>,
    child: &Child<T>,
    expected: process::Pid,
    pidfd: &OwnedFd,
    credentials: Credentials,
    b: &mut Budget<'_>,
) -> Result<()> {
    b.with_prepaid_scope(
        sum(&[root.retained, child.retained_storage(), FD_STORAGE])?,
        ENTRY,
        LOCAL_WORK,
        FRAME,
        |b| {
            if child.pid() != expected || !child.is_live(b)? {
                return Err(Error::Refused("root issuer custody changed or exited"));
            }
            if io::fcntl_getfd(pidfd)? != io::FdFlags::CLOEXEC {
                return Err(Error::Refused("root issuer pidfd flags changed"));
            }
            let mut poll = [event::PollFd::new(pidfd, event::PollFlags::IN)];
            if event::poll(
                &mut poll,
                Some(&event::Timespec {
                    tv_sec: 0,
                    tv_nsec: 0,
                }),
            )? != 0
                || !poll[0].revents().is_empty()
            {
                return Err(Error::Refused("original issuer pidfd is terminal"));
            }
            root.namespaces.revalidate_process(expected, b)?;
            b.with_prepaid_scope(
                root.retained,
                ENTRY,
                ENTRY + observations::PROCESS_VALIDATE_WORK,
                observations::PROCESS_VALIDATE_SCRATCH,
                |_| -> Result<()> { Ok(observations::validate_process(credentials, expected)?) },
            )?;
            if !child.is_live(b)? {
                return Err(Error::Refused(
                    "root issuer exited during profile validation",
                ));
            }
            Ok(())
        },
    )
}

fn require_root() -> Result<()> {
    require_exact_root_identity_v1()
        .map_err(|_| Error::Refused("root connection requires exact root identity"))
}
fn nonce() -> Result<[u8; 32]> {
    let mut bytes = [0; 32];
    if rustix::rand::getrandom(&mut bytes, rustix::rand::GetRandomFlags::NONBLOCK)? != bytes.len()
        || bytes == [0; 32]
    {
        return Err(Error::Refused("root control randomness unavailable"));
    }
    Ok(bytes)
}
fn check_deadline(deadline: Instant) -> Result<()> {
    if Instant::now() >= deadline {
        return Err(Error::Refused("root control handshake deadline"));
    }
    Ok(())
}
fn sum(values: &[usize]) -> Result<usize> {
    values.iter().try_fold(0usize, |a, &b| {
        a.checked_add(b).ok_or(Resource::Arithmetic.into())
    })
}

#[derive(Debug)]
pub enum RootConnectionErrorV3 {
    Resource(Resource),
    Channel(ChannelError),
    Image(ImageError),
    Spawn(SpawnError),
    Profile(ProfileError),
    Observation(observations::Error),
    Protocol(ProtocolError),
    Ready(ReadyError),
    Io(io::Errno),
    Transport(launch_io::Failure),
    Refused(&'static str),
}
use RootConnectionErrorV3 as Error;
type Result<T> = std::result::Result<T, Error>;
macro_rules! errors {
    ($($ty:ty => $variant:ident),* $(,)?) => { $(impl From<$ty> for Error {
        fn from(e: $ty) -> Self { Self::$variant(e) }
    })* };
}
errors!(Resource => Resource, ChannelError => Channel, ImageError => Image, SpawnError => Spawn,
    ProfileError => Profile, observations::Error => Observation, ProtocolError => Protocol,
    ReadyError => Ready, io::Errno => Io);
impl From<ReplayError> for Error {
    fn from(e: ReplayError) -> Self {
        match e {
            ReplayError::Resource(e) => Self::Resource(e),
            _ => Self::Refused("root control replay initialization"),
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Channel(e) => e.fmt(f),
            Self::Image(e) => e.fmt(f),
            Self::Spawn(e) => e.fmt(f),
            Self::Profile(e) => e.fmt(f),
            Self::Observation(e) => e.fmt(f),
            Self::Protocol(e) => e.fmt(f),
            Self::Ready(e) => e.fmt(f),
            Self::Io(e) => e.fmt(f),
            Self::Transport(e) => write!(f, "root control transport: {e:?}"),
            Self::Refused(e) => f.write_str(e),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Channel(e) => Some(e),
            Self::Image(e) => Some(e),
            Self::Spawn(e) => Some(e),
            Self::Profile(e) => Some(e),
            Self::Observation(e) => Some(e),
            Self::Protocol(e) => Some(e),
            Self::Ready(e) => Some(e),
            Self::Io(e) => Some(e),
            Self::Transport(_) | Self::Refused(_) => None,
        }
    }
}

#[path = "compiler_execution_root_connection_quota.rs"]
mod quota;
pub use quota::RootConnectionQuotaV3;
