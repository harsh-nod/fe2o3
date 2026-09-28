//! Native V3 records over the existing protocol-neutral child-created endpoints.
//! The child-channel V1 suffix versions descriptor construction, not issuer policy.
//! No V1/V2 policy, manifest or readiness owner is admitted by this path.
use super::{
    CompilerExecutionHandoffErrorV1 as Transport,
    CompilerExecutionSupervisorCredentialsV1 as Credentials,
};
use crate::{
    CompilerExecutionChildChannelErrorV1 as ChildError,
    CompilerExecutionServiceLaunchV1 as ChildLaunch,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_STORAGE_V3 as MANIFEST_SCRATCH,
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_WORK_V3 as MANIFEST_WORK,
    COMPILER_EXECUTION_SERVICE_READY_BYTES_V3 as READY_BYTES,
    COMPILER_EXECUTION_SERVICE_READY_STORAGE_V3 as READY_SCRATCH,
    COMPILER_EXECUTION_SERVICE_READY_WORK_V3 as READY_WORK,
    COMPILER_EXECUTION_SUPERVISOR_HANDOFF_STORAGE_V3 as HANDOFF_SCRATCH,
    COMPILER_EXECUTION_SUPERVISOR_HANDOFF_WORK_V3 as HANDOFF_WORK,
    CompilerExecutionClientProfileV3 as Profile,
    CompilerExecutionServiceLaunchManifestErrorV3 as ManifestError,
    CompilerExecutionServiceLaunchManifestV3 as Manifest,
    CompilerExecutionServiceReadyErrorV3 as ReadyError, CompilerExecutionServiceReadyV3 as Ready,
    CompilerExecutionSupervisorHandoffErrorV3 as HandoffError,
    CompilerExecutionSupervisorHandoffV3 as Handoff,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use std::{
    error::Error,
    fmt,
    mem::size_of,
    os::fd::{AsFd, OwnedFd},
    time::Instant,
};

#[path = "native_supervisor_handoff_io.rs"]
mod io;
type Result<T> = std::result::Result<T, CompilerExecutionHandoffErrorV3>;
use CompilerExecutionHandoffErrorV3 as Failure;
use CompilerExecutionHandoffStorageV3 as Storage;
use CompilerExecutionSupervisorReadinessV3 as Received;
use PendingCompilerExecutionSupervisorV3 as Pending;

const LOCAL_WORK: usize = 8 + 128 * 1024;
const FRAME: usize = 32 * 1024;

/// Unreserved growth above the consumed input owner, not its complete charge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerExecutionHandoffStorageV3(usize);
impl Storage {
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}

// Private continuation: the public exchange retains the original mutable budget
// borrow until readiness and EOF. An address token alone cannot outlive that borrow.
struct PendingCompilerExecutionSupervisorV3 {
    control: OwnedFd,
    handoff: Handoff,
    profile: [u8; 32],
    deadline: Instant,
    ledger: Ledger,
}
impl Pending {
    /// Full inert child-launch input charge, including both descriptor owners.
    pub const CHILD_LAUNCH_STORAGE: usize = size_of::<ChildLaunch>();
    /// Complete successful transfer work on the original account.
    pub const TRANSFER_WORK: usize = LOCAL_WORK + MANIFEST_WORK + HANDOFF_WORK;
    /// Conservative maximum additional peak above complete borrowed inputs.
    pub const TRANSFER_SCRATCH: usize = FRAME + 2 * MANIFEST_SCRATCH + 2 * HANDOFF_SCRATCH;
    pub const READINESS_WORK: usize = LOCAL_WORK + MANIFEST_WORK + 2 * READY_WORK;
    pub const READINESS_SCRATCH: usize = FRAME + 2 * READY_SCRATCH + MANIFEST_SCRATCH;

    pub const fn manifest(&self) -> &Manifest {
        self.handoff.launch_manifest()
    }
    pub fn retained_storage(&self) -> usize {
        self.handoff.retained_storage() + size_of::<(Self, Storage)>() - size_of::<Handoff>()
    }

    fn finish_with(
        self,
        profile: &Profile,
        b: &mut Budget<'_>,
        transport: &mut impl ReadinessIo,
    ) -> Result<(Received, Storage)> {
        let inherited = self.retained_storage();
        let floor = sum(inherited, profile.retained_storage())?;
        b.with_prepaid_scope(floor, 8, LOCAL_WORK, FRAME, |b| {
            if self.ledger != b.work_ledger_identity_v1() {
                return Err(Resource::Accounting.into());
            }
            if self.profile != *profile.identity().as_bytes()
                || self.manifest().external_anchor_service() != profile.external_anchor_service()
                || !self.manifest().matches_policy(profile.policy(), b)?
            {
                return Err(Failure::Mismatch("handoff profile changed"));
            }
            super::require_deadline(self.deadline)?;
            transport.validate(&self.control, credentials(profile)?)?;
            let bytes = transport.readiness(&self.control, self.deadline)?;
            b.reserve_storage(READY_BYTES)?;
            let (ready, charge) = Ready::decode(&bytes, b)?;
            b.reserve_storage(charge.additional_storage())?;
            if !ready.matches_launch(ready.issuer_pid(), self.manifest(), profile.policy(), b)? {
                return Err(Failure::Mismatch(
                    "supervisor readiness differs from native launch",
                ));
            }
            transport.eof(&self.control, self.deadline)?;
            super::require_deadline(self.deadline)?;
            let received = Received {
                handoff: self.handoff,
                readiness: ready,
            };
            let growth = received
                .retained_storage()
                .checked_sub(inherited)
                .ok_or(Resource::Accounting)?;
            Ok((received, Storage(growth)))
        })
    }
}

// The production entrypoint fixes System; test effects exercise account/order only.
trait ReadinessIo {
    fn validate(&mut self, control: &OwnedFd, expected: Credentials) -> Result<()>;
    fn readiness(&mut self, control: &OwnedFd, deadline: Instant) -> Result<[u8; READY_BYTES]>;
    fn eof(&mut self, control: &OwnedFd, deadline: Instant) -> Result<()>;
}

impl ChildLaunch {
    /// Transfers genuine child descriptors and receives native V3 readiness/EOF.
    /// Exactly one connect/send/receive attempt per phase; EINTR, AGAIN, short
    /// writes and deadlines refuse rather than retry. One absolute deadline spans
    /// the complete exchange, at most the existing 120-second supervisor boundary.
    ///
    /// Reserve `CompilerExecutionSupervisorReadinessV3::CHILD_LAUNCH_STORAGE` and
    /// the full authenticated profile first. On success retain the consumed input
    /// reservation and add returned growth. On failure all consumed descriptors
    /// close and the input charge remains caller-owned. Entry storage is restored
    /// without refunding work or denial history. The same mutable account remains
    /// borrowed throughout; no public intermediate owner can change accounts.
    ///
    /// Child preparation/spawning and profile provenance are caller obligations.
    /// Logical quotas do not bound kernel buffers, syscall latency or RSS. No V1
    /// policy/manifest/readiness owner is converted or decoded, and observing
    /// readiness does not grant compiler, signing, publication or GPU authority.
    ///
    /// ```compile_fail
    /// use fe2o3_compiler_execution_client::CompilerExecutionServiceLaunchV1 as Launch;
    /// use fe2o3_compiler_execution_protocol::CompilerExecutionClientProfileV1 as Profile;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn mix(p: Launch, profile: &Profile, b: &mut Budget<'_>) {
    ///     p.handoff_to_supervisor_v3_until(profile, std::time::Instant::now(), b);
    /// }
    /// ```
    pub fn handoff_to_supervisor_v3_until(
        self,
        profile: &Profile,
        deadline: Instant,
        b: &mut Budget<'_>,
    ) -> Result<(Received, Storage)> {
        let floor = sum(Pending::CHILD_LAUNCH_STORAGE, profile.retained_storage())?;
        b.with_prepaid_scope(floor, 0, 0, 0, |b| {
            let (pending, charge) = transfer_with::<true>(self, profile, deadline, b, io::connect)?;
            b.reserve_storage(charge.additional_storage())?;
            let (received, _) = pending.finish_with(profile, b, &mut io::System)?;
            let growth = received
                .retained_storage()
                .checked_sub(Pending::CHILD_LAUNCH_STORAGE)
                .ok_or(Resource::Accounting)?;
            Ok((received, Storage(growth)))
        })
    }
}

fn transfer_with<const DISTINCT_UID: bool>(
    launch: ChildLaunch,
    profile: &Profile,
    deadline: Instant,
    b: &mut Budget<'_>,
    connect: impl FnOnce(Credentials, Instant) -> Result<OwnedFd>,
) -> Result<(Pending, Storage)> {
    let floor = sum(Pending::CHILD_LAUNCH_STORAGE, profile.retained_storage())?;
    b.with_prepaid_scope(floor, 8, LOCAL_WORK, FRAME, |b| {
        super::validate_boundary_deadline(deadline)?;
        let expected = credentials(profile)?;
        if DISTINCT_UID && launch.client().uid() == expected.uid() {
            return Err(Failure::Mismatch("client and supervisor UIDs must differ"));
        }
        launch.revalidate_for_supervisor_handoff()?;
        let (manifest, charge) = Manifest::new(
            launch.client(),
            profile.external_anchor_service(),
            profile.policy(),
            b,
        )?;
        b.reserve_storage(charge.additional_storage())?;
        let (handoff, growth) = Handoff::new(launch.submitter(), manifest, b)?;
        b.reserve_storage(growth.additional_storage())?;
        let control = connect(expected, deadline)?;
        // Enable after connect so Linux does not autobind an abstract local name.
        rustix::net::sockopt::set_socket_passcred(&control, true)?;
        let (peer, pidfd) = launch.into_descriptors();
        io::send(
            &control,
            handoff.canonical_bytes(),
            &[peer.as_fd(), pidfd.as_fd()],
            deadline,
        )?;
        let pending = Pending {
            control,
            handoff,
            profile: *profile.identity().as_bytes(),
            deadline,
            ledger: b.work_ledger_identity_v1(),
        };
        let growth = pending
            .retained_storage()
            .checked_sub(Pending::CHILD_LAUNCH_STORAGE)
            .ok_or(Resource::Accounting)?;
        Ok((pending, Storage(growth)))
    })
}

/// Move-only V3 public records observed on one authenticated control connection.
/// No descriptor remains, and no signing, compiler, publication or GPU authority
/// is granted. Provenance of the caller-supplied profile remains its obligation.
/// ```compile_fail
/// use fe2o3_compiler_execution_client::CompilerExecutionSupervisorReadinessV3 as R;
/// fn clone<T: Clone>() {} clone::<R>();
/// ```
pub struct CompilerExecutionSupervisorReadinessV3 {
    handoff: Handoff,
    readiness: Ready,
}
impl Received {
    /// Full inert child-launch input charge, including both descriptor owners.
    pub const CHILD_LAUNCH_STORAGE: usize = Pending::CHILD_LAUNCH_STORAGE;
    /// Complete successful transfer, readiness decode/match and EOF work.
    pub const HANDOFF_WORK: usize = Pending::TRANSFER_WORK + Pending::READINESS_WORK;
    /// Conservative additional peak above the full prepaid launch/profile inputs.
    pub const HANDOFF_SCRATCH: usize = Pending::TRANSFER_SCRATCH + Pending::READINESS_SCRATCH;
    pub const fn manifest(&self) -> &Manifest {
        self.handoff.launch_manifest()
    }
    pub const fn readiness(&self) -> &Ready {
        &self.readiness
    }
    pub fn retained_storage(&self) -> usize {
        self.handoff.retained_storage()
            + self.readiness.retained_storage()
            + size_of::<(Self, Storage)>()
            - size_of::<Handoff>()
            - size_of::<Ready>()
    }
}
fn credentials(profile: &Profile) -> Result<Credentials> {
    Ok(Credentials::new(
        profile.supervisor_uid(),
        profile.supervisor_gid(),
    )?)
}
fn sum(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).ok_or(Resource::Arithmetic.into())
}

#[derive(Debug)]
pub enum CompilerExecutionHandoffErrorV3 {
    Resource(Resource),
    Transport(Transport),
    Child(ChildError),
    Manifest(ManifestError),
    Handoff(HandoffError),
    Readiness(ReadyError),
    Io(rustix::io::Errno),
    Mismatch(&'static str),
}
macro_rules! causes {
    ($($ty:ty => $variant:ident),+ $(,)?) => {
        $(impl From<$ty> for Failure { fn from(e: $ty) -> Self { Self::$variant(e) } })+
        impl fmt::Display for Failure { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self { $(Self::$variant(e) => e.fmt(f),)+ Self::Mismatch(s) => f.write_str(s) }
        } }
        impl Error for Failure { fn source(&self) -> Option<&(dyn Error + 'static)> {
            match self { $(Self::$variant(e) => Some(e),)+ Self::Mismatch(_) => None }
        } }
    };
}
causes!(Resource=>Resource, Transport=>Transport, ChildError=>Child, ManifestError=>Manifest,
    HandoffError=>Handoff, ReadyError=>Readiness, rustix::io::Errno=>Io);

#[cfg(test)]
#[path = "native_supervisor_handoff_tests.rs"]
mod tests;
