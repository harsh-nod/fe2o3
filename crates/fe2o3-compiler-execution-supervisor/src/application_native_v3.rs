//! Native application custody only; root observation and readiness are separate.
use super::{
    AcceptedCompilerExecutionHandoffV3 as Compiler, Budget, Client,
    ExpectedClientProcessIdentityV1, Frame, LiveClient,
    ProtectedIssuerHandoffErrorV3 as HandoffError, Resource, Snapshot, Supervisor, checks, context,
    pidfd_snapshot,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrStorageAccountIdentityV1 as StorageAccount,
    CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use fe2o3_runtime_protocol::{
    NATIVE_APPLICATION_REGISTRATION_BYTES_V1 as BYTES,
    NativeApplicationRegistrationBindingV1 as Binding,
    NativeApplicationRegistrationErrorV1 as BindingError, WorkerV3ApplicationInputOccurrenceV1,
};
use rustix::{
    fs::OFlags,
    net::{AddressFamily, SocketAddrUnix, SocketType},
};
use std::{
    error::Error,
    fmt,
    marker::PhantomData,
    mem::{align_of, size_of},
    os::fd::OwnedFd,
    time::{Duration, Instant},
};

#[path = "application_native_root_v3.rs"]
mod root_transfer;
#[path = "application_native_v3_io.rs"]
mod transport;
pub use root_transfer::{
    PendingNativeApplicationRootTransferV3, PublishedNativeApplicationRootTransferV3,
};
type Result<T> = std::result::Result<T, ProtectedNativeApplicationHandoffErrorV3>;
use ProtectedNativeApplicationHandoffErrorV3 as Failure;

/// Growth above the consumed control descriptor; reserve before retaining the owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedNativeApplicationHandoffStorageV3(usize);
impl ProtectedNativeApplicationHandoffStorageV3 {
    /// Additional retained bytes above the consumed-control reservation.
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}
use ProtectedNativeApplicationHandoffStorageV3 as Storage;

/// Original native compiler/application, Cargo parent and candidate proof-peer custody.
///
/// The actual V3 handoff is retained without any V1/V2 policy projection. The
/// native binding remains inert: root observation must still prove the reversed
/// proof addresses and exact application input occurrence. This owner grants no
/// readiness, proof, compiler, publication, or GPU authority. No public descriptor
/// extraction or conversion to the compiler-only accepted owner is available.
/// Keep all retained inputs prepaid and use the originating cumulative account.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::AcceptedNativeApplicationHandoffV3;
/// fn clone<T: Clone>() {} clone::<AcceptedNativeApplicationHandoffV3<'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::AcceptedNativeApplicationHandoffV3;
/// fn fd<T: std::os::fd::AsFd>() {} fd::<AcceptedNativeApplicationHandoffV3<'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{AcceptedNativeApplicationHandoffV3 as A,
///     AcceptedCompilerExecutionHandoffV3 as C};
/// fn downgrade(a: A<'_>) -> C { a.into() }
/// ```
/// The complete native service lifecycle stays within one owned-account view:
/// ```
/// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerSupervisorV3 as Supervisor,
///     AcceptedNativeApplicationHandoffV3 as Accepted};
/// use fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Account;
/// fn inspect(s: &Supervisor, fd: std::os::fd::OwnedFd, account: &mut Account) {
///     account.with_budget(|b| {
///         b.reserve_storage(Accepted::CONTROL_STORAGE).unwrap();
///         if let Ok((owner, growth)) = s.accept_native_application_handoff(fd, std::time::Duration::from_secs(1), b) {
///             b.reserve_storage(growth.additional_storage()).unwrap();
///             let _ = owner.revalidate(s, b);
///             let retained = owner.retained_storage();
///             drop(owner);
///             b.release_storage(retained).unwrap();
///         }
///     });
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::ProtectedIssuerSupervisorV3 as Supervisor;
/// use fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Account;
/// fn escape(s: &Supervisor, fd: std::os::fd::OwnedFd, account: &mut Account) {
///     let owner = account.with_budget(|b| s.accept_native_application_handoff(fd, std::time::Duration::from_secs(1), b));
///     drop(owner);
/// }
/// ```
pub struct AcceptedNativeApplicationHandoffV3<'work> {
    compiler: Compiler,
    binding: Binding,
    proof_peer: OwnedFd,
    parent: LiveClient,
    proof_snapshot: ProofSnapshot,
    ledger: Ledger,
    storage_account: Option<StorageAccount>,
    retained: usize,
    work: PhantomData<&'work Work>,
}
type Accepted<'work> = AcceptedNativeApplicationHandoffV3<'work>;

impl<'work> Accepted<'work> {
    /// Prepaid consumed control-descriptor charge, including receipt padding.
    pub const CONTROL_STORAGE: usize = Compiler::CONTROL_STORAGE;
    /// Fixed outer work; native decoding/process checks charge additional work.
    pub const WORK: usize = 8 + 256 * 1024;
    /// Logical outer scratch, excluding nested charges, kernel memory and RSS.
    pub const SCRATCH: usize =
        4 * size_of::<(Self, Storage)>() + 4 * BYTES + 2 * transport::ANCILLARY_BYTES + 4096;
    const OWNER_GROWTH: usize = size_of::<(
        ProofSnapshot,
        Ledger,
        Option<StorageAccount>,
        usize,
        Storage,
    )>() + align_of::<Self>();

    /// Exact inert native binding, not root observation or readiness authority.
    pub const fn binding(&self) -> &Binding {
        &self.binding
    }
    /// Full retained reservation, retired only after consuming or dropping this owner.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }

    /// Rechecks original native policy, live parentage and exact descriptor roles.
    /// A different ledger is rejected before any process or descriptor inspection.
    pub fn revalidate(&self, supervisor: &Supervisor, budget: &mut Budget<'work>) -> Result<()> {
        budget.charge_work(8)?;
        if self.ledger != budget.work_ledger_identity_v1()
            || self.storage_account != budget.storage_account_identity_v1()
        {
            return Err(Resource::Accounting.into());
        }
        let floor = self
            .retained
            .checked_add(supervisor.retained_storage())
            .ok_or(Resource::Arithmetic)?;
        budget.with_prepaid_scope(floor, 0, Self::WORK - 8, Self::SCRATCH, |budget| {
            self.check(supervisor, budget)
        })
    }

    fn check(&self, supervisor: &Supervisor, budget: &mut Budget<'work>) -> Result<()> {
        if self.ledger != budget.work_ledger_identity_v1()
            || self.storage_account != budget.storage_account_identity_v1()
        {
            return Err(Resource::Accounting.into());
        }
        self.compiler.check(supervisor, budget)?;
        if self.compiler.handoff.canonical_bytes()
            != self.binding.compiler_handoff().canonical_bytes()
            || proof_snapshot(&self.proof_peer, self.compiler.submitter(), &self.binding)?
                != self.proof_snapshot
        {
            return Err(HandoffError::DescriptorChanged.into());
        }
        let objects = [
            self.compiler.control_snapshot,
            self.compiler.service_snapshot,
            pidfd_snapshot(&self.compiler.client),
            self.proof_snapshot.object,
            pidfd_snapshot(&self.parent),
        ];
        distinct_roles(&objects)?;
        self.compiler
            .client
            .validate_parent(&self.parent, budget)
            .map_err(HandoffError::from)?;
        Ok(())
    }

    pub(crate) fn compiler(&self) -> &Compiler {
        &self.compiler
    }
}

impl Supervisor {
    /// Consumes exactly one native binding packet and four ordered rights:
    /// service peer, application pidfd, Cargo-side proof peer, Cargo parent pidfd.
    ///
    /// One poll/receive shares a <=120-second absolute deadline with admission.
    /// All nested decoding and native process checks charge the original budget;
    /// consuming failures close every descriptor without refunding work/denials.
    pub fn accept_native_application_handoff<'work>(
        &self,
        control: OwnedFd,
        timeout: Duration,
        budget: &mut Budget<'work>,
    ) -> Result<(Accepted<'work>, Storage)> {
        budget.charge_work(8)?;
        let floor = self
            .retained_storage()
            .checked_add(Accepted::CONTROL_STORAGE)
            .ok_or(Resource::Arithmetic)?;
        budget.with_prepaid_scope(floor, 0, Accepted::WORK - 8, Accepted::SCRATCH, |budget| {
            if timeout.is_zero() || timeout > Duration::from_secs(120) {
                return Err(HandoffError::InvalidTimeout.into());
            }
            let deadline = Instant::now()
                .checked_add(timeout)
                .ok_or(HandoffError::DeadlineOverflow)?;
            self.revalidate(budget).map_err(HandoffError::from)?;
            checks::control_shape(&control).map_err(HandoffError::from)?;
            let submitter = checks::control_peer(&control).map_err(HandoffError::from)?;
            if submitter.uid() == self.credentials().uid() {
                return Err(HandoffError::ClientAndSupervisorUidMatch.into());
            }
            let control_snapshot = checks::snapshot(&control).map_err(HandoffError::from)?;
            let (payload, [service_peer, pidfd, proof_peer, parent_pidfd]) =
                transport::receive(&control, submitter, deadline)?;
            budget.reserve_storage(2 * Compiler::CONTROL_STORAGE + 2 * LiveClient::FD_STORAGE)?;
            let (binding, charge) = Binding::decode(&payload, budget)?;
            budget.reserve_storage(charge.additional_storage())?;
            let (handoff, charge) =
                Frame::decode(binding.compiler_handoff().canonical_bytes(), budget)
                    .map_err(HandoffError::from)?;
            budget.reserve_storage(charge.additional_storage())?;
            context(&handoff, self, budget)?;
            if handoff.submitter() != submitter {
                return Err(HandoffError::SubmitterCredentialsMismatch.into());
            }
            let expected = handoff.launch_manifest().client();
            let service_snapshot = checks::snapshot(&service_peer).map_err(HandoffError::from)?;
            let client_snapshot = checks::snapshot(&pidfd).map_err(HandoffError::from)?;
            checks::distinct(control_snapshot, service_snapshot, client_snapshot)
                .map_err(HandoffError::from)?;
            checks::service_peer(&service_peer, expected).map_err(HandoffError::from)?;
            let expected = ExpectedClientProcessIdentityV1::new(
                expected.pid(),
                expected.uid(),
                expected.gid(),
            )
            .expect("canonical native client PID is nonzero");
            let (client, charge) =
                LiveClient::admit(pidfd, expected, budget).map_err(HandoffError::from)?;
            budget.reserve_storage(charge.additional_storage())?;
            let expected_parent = ExpectedClientProcessIdentityV1::new(
                submitter.pid(),
                submitter.uid(),
                submitter.gid(),
            )
            .expect("canonical native submitter PID is nonzero");
            let (parent, charge) = LiveClient::admit(parent_pidfd, expected_parent, budget)
                .map_err(HandoffError::from)?;
            budget.reserve_storage(charge.additional_storage())?;
            let compiler_retained = (2 * Compiler::CONTROL_STORAGE)
                .checked_add(handoff.retained_storage())
                .and_then(|n| n.checked_add(client.retained_storage()))
                .and_then(|n| n.checked_add(Compiler::OWNER_GROWTH))
                .ok_or(Resource::Arithmetic)?;
            let compiler = Compiler {
                control,
                handoff,
                service_peer,
                client,
                control_snapshot,
                service_snapshot,
                pidfd_snapshot: client_snapshot,
                retained: compiler_retained,
            };
            let retained = compiler_retained
                .checked_add(binding.retained_storage())
                .and_then(|n| n.checked_add(Compiler::CONTROL_STORAGE))
                .and_then(|n| n.checked_add(parent.retained_storage()))
                .and_then(|n| n.checked_add(Accepted::OWNER_GROWTH))
                .ok_or(Resource::Arithmetic)?;
            let accepted = Accepted {
                proof_snapshot: proof_snapshot(&proof_peer, submitter, &binding)?,
                compiler,
                binding,
                proof_peer,
                parent,
                ledger: budget.work_ledger_identity_v1(),
                storage_account: budget.storage_account_identity_v1(),
                retained,
                work: PhantomData,
            };
            budget.reserve_storage(Compiler::OWNER_GROWTH + Accepted::OWNER_GROWTH)?;
            accepted.check(self, budget)?;
            if Instant::now() >= deadline {
                return Err(HandoffError::Timeout.into());
            }
            Ok((accepted, Storage(retained - Accepted::CONTROL_STORAGE)))
        })
    }
}

fn distinct_roles(objects: &[Snapshot; 5]) -> Result<()> {
    for (index, object) in objects.iter().enumerate() {
        if objects[..index]
            .iter()
            .enumerate()
            .any(|(other_index, other)|
            // Some kernels share anon-inode metadata across pidfds; native exact
            // target/start-time and parentage checks distinguish these two roles.
            !(index == 4 && other_index == 2) && (other.0, other.1) == (object.0, object.1))
        {
            return Err(HandoffError::DescriptorAlias.into());
        }
    }
    Ok(())
}

#[derive(Debug, Eq, PartialEq)]
struct ProofSnapshot {
    object: Snapshot,
    local: SocketAddrUnix,
    remote: SocketAddrUnix,
}

fn proof_snapshot(peer: &OwnedFd, parent: Client, binding: &Binding) -> Result<ProofSnapshot> {
    let snapshot = proof_socket_snapshot(peer, parent)?;
    let object = snapshot.object;
    let input = WorkerV3ApplicationInputOccurrenceV1::from_linux_descriptor_v1(
        4, object.0, object.1, object.2,
    )
    .map_err(|_| Failure::ProofPeer("invalid object"))?;
    if binding.occurrence().inputs().get(3) == Some(&input) {
        return Err(Failure::ProofPeer(
            "received app endpoint instead of counterpart",
        ));
    }
    Ok(snapshot)
}

fn proof_socket_snapshot(peer: &OwnedFd, parent: Client) -> Result<ProofSnapshot> {
    let flags = rustix::fs::fcntl_getfl(peer)?;
    if rustix::io::fcntl_getfd(peer)? != rustix::io::FdFlags::CLOEXEC
        || flags & OFlags::ACCMODE != OFlags::RDWR
        || !flags.contains(OFlags::NONBLOCK)
        || flags.intersects(OFlags::PATH | OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT)
        || rustix::net::sockopt::socket_domain(peer)? != AddressFamily::UNIX
        || rustix::net::sockopt::socket_type(peer)? != SocketType::SEQPACKET
        || rustix::net::sockopt::socket_acceptconn(peer)?
        || !rustix::net::sockopt::socket_passcred(peer)?
        || checks::control_peer(peer).map_err(HandoffError::from)? != parent
    {
        return Err(Failure::ProofPeer("shape, flags or Cargo creator mismatch"));
    }
    let local = SocketAddrUnix::try_from(rustix::net::getsockname(peer)?)?;
    let remote = SocketAddrUnix::try_from(
        rustix::net::getpeername(peer)?.ok_or(Failure::ProofPeer("disconnected peer"))?,
    )?;
    if local.abstract_name().is_none() || remote.abstract_name().is_none() || local == remote {
        return Err(Failure::ProofPeer("expected distinct abstract addresses"));
    }
    let object = checks::snapshot(peer).map_err(HandoffError::from)?;
    Ok(ProofSnapshot {
        object,
        local,
        remote,
    })
}

impl fmt::Debug for Accepted<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AcceptedNativeApplicationHandoffV3")
            .field("binding", &self.binding.identity())
            .field("authority", &"none")
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
#[non_exhaustive]
/// Fixed native application admission failure, never a legacy fallback.
pub enum ProtectedNativeApplicationHandoffErrorV3 {
    /// The original work/storage account refused the operation.
    Resource(Resource),
    /// Native compiler, process or descriptor custody did not validate.
    Handoff(HandoffError),
    /// The exact native registration record was invalid.
    Binding(BindingError),
    /// The proof endpoint's shape or occurrence did not match its role.
    ProofPeer(&'static str),
}
impl From<Resource> for Failure {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl From<HandoffError> for Failure {
    fn from(e: HandoffError) -> Self {
        Self::Handoff(e)
    }
}
impl From<BindingError> for Failure {
    fn from(e: BindingError) -> Self {
        Self::Binding(e)
    }
}
impl From<rustix::io::Errno> for Failure {
    fn from(e: rustix::io::Errno) -> Self {
        Self::Handoff(HandoffError::Io(e))
    }
}
impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Handoff(e) => e.fmt(f),
            Self::Binding(e) => e.fmt(f),
            Self::ProofPeer(s) => f.write_str(s),
        }
    }
}
impl Error for Failure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Handoff(e) => Some(e),
            Self::Binding(e) => Some(e),
            Self::ProofPeer(_) => None,
        }
    }
}

#[cfg(test)]
#[path = "application_native_v3_tests.rs"]
mod tests;
