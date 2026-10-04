//! Native root/socket/client custody, not an issuer or a second transport.
use super::{
    AdmissionErrorKindV1 as Kind, ExpectedClientProcessIdentityV1, LiveClientPidfdErrorV2,
    LiveClientPidfdIdentityV2 as Client, ObjectIdentityV1,
    checks::{self, CheckError},
    native, require_distinct_descriptors, require_distinct_pidfd_descriptor, require_root_identity,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{
    fmt,
    mem::size_of,
    os::fd::{AsFd, BorrowedFd, OwnedFd},
};

/// Unreserved growth over the complete consumed input charge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedServiceStorageV2(usize);
impl ProtectedServiceStorageV2 {
    /// Preserve input reservations and reserve this delta immediately on success.
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}
use ProtectedServiceStorageV2 as Storage;

/// Allocation-free refusal; diagnostic rendering is a caller operation.
#[derive(Debug)]
pub struct ProtectedServiceAdmissionErrorV2(Failure);
#[derive(Debug)]
enum Failure {
    Resource(Resource),
    Inspection(CheckError),
    Client(LiveClientPidfdErrorV2),
}
type Error = ProtectedServiceAdmissionErrorV2;
type Result<T> = std::result::Result<T, Error>;
impl Error {
    pub const fn kind(&self) -> Option<Kind> {
        match &self.0 {
            Failure::Resource(_) => None,
            Failure::Inspection(error) => Some(error.kind()),
            Failure::Client(error) => error.kind(),
        }
    }
    pub const fn errno(&self) -> Option<i32> {
        match &self.0 {
            Failure::Resource(_) => None,
            Failure::Inspection(error) => error.errno(),
            Failure::Client(error) => error.errno(),
        }
    }
    pub const fn resource(&self) -> Option<Resource> {
        match &self.0 {
            Failure::Resource(error) => Some(*error),
            Failure::Inspection(_) => None,
            Failure::Client(error) => error.resource(),
        }
    }
}
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self(Failure::Resource(error))
    }
}
impl From<CheckError> for Error {
    fn from(error: CheckError) -> Self {
        Self(Failure::Inspection(error))
    }
}
impl From<LiveClientPidfdErrorV2> for Error {
    fn from(error: LiveClientPidfdErrorV2) -> Self {
        Self(Failure::Client(error))
    }
}
impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Failure::Resource(error) => error.fmt(formatter),
            Failure::Inspection(error) => error.fmt(formatter),
            Failure::Client(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.0 {
            Failure::Resource(error) => Some(error),
            Failure::Inspection(error) => Some(error),
            Failure::Client(error) => Some(error),
        }
    }
}

/// Sealed, move-only custody of a protected root, rustc endpoint and live client.
///
/// Fresh admission retains the supplied native client owner, never a converted
/// V1 service admission. The root must remain a linked directory owned by the
/// service effective UID with exact mode 0700. All three roles are distinct and
/// CLOEXEC. The endpoint must be connected, unnamed AF_UNIX SOCK_SEQPACKET with
/// exact read-write/nonblocking status; SO_PEERCRED must match the retained
/// client's PID/UID/GID, with client and service UIDs distinct. Complete metadata
/// snapshots and the pidfd's target/start-time/liveness are rechecked on use.
///
/// Success observes continuity at a point in time. It cannot prove exclusive
/// endpoint ownership, future liveness or procfs mount provenance. Compatible
/// procfs is trusted. There is no signing, readiness, journal, packet exchange,
/// publication or GPU authority, and no public descriptor extraction/transfer.
///
/// Every call uses the original cumulative ledger. Input reservations remain
/// live until drop; success returns only additional owner/receipt growth, which
/// the caller must immediately reserve. Scopes restore entry storage on success,
/// refusal and unwind without refunding work or clearing denial/peak history.
/// Consuming refusal drops all three inputs even if entry funding is refused.
/// Logical charges cover declared Rust values/scratch and finite inspection
/// work, not allocator overhead, generated stacks, kernel objects, RSS, latency
/// or work done by the client. Formatting diagnostics is outside this contract.
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::ProtectedServiceAdmissionV2;
/// fn clone<T: Clone>() {}
/// clone::<ProtectedServiceAdmissionV2>();
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::ProtectedServiceAdmissionV2;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<ProtectedServiceAdmissionV2>();
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::ProtectedServiceAdmissionV2;
/// fn descriptor<T: std::os::fd::AsRawFd>() {}
/// descriptor::<ProtectedServiceAdmissionV2>();
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::ProtectedServiceAdmissionV2;
/// fn descriptor<T: std::os::fd::IntoRawFd>() {}
/// descriptor::<ProtectedServiceAdmissionV2>();
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::{ProtectedServiceAdmissionV1, ProtectedServiceAdmissionV2};
/// fn upgrade(old: ProtectedServiceAdmissionV1) -> ProtectedServiceAdmissionV2 { old.into() }
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::{ProtectedServiceAdmissionV2, ProtectedCompilerExecutionIssuerAdmissionV1};
/// fn issuer(service: ProtectedServiceAdmissionV2) -> ProtectedCompilerExecutionIssuerAdmissionV1 {
///     service.into()
/// }
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::ProtectedServiceAdmissionV2;
/// fn escape(service: ProtectedServiceAdmissionV2) { let _ = service.service_peer(); }
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::ProtectedServiceAdmissionV2;
/// fn escape(service: ProtectedServiceAdmissionV2) { let _ = service.service_root(); }
/// ```
pub struct ProtectedServiceAdmissionV2 {
    root: OwnedFd,
    peer: OwnedFd,
    live_client: Client,
    service_uid: u32,
    root_identity: ObjectIdentityV1,
    peer_identity: ObjectIdentityV1,
    #[cfg(test)]
    same_uid_fixture: bool,
}
type Service = ProtectedServiceAdmissionV2;

impl Service {
    const RETAINED: usize = size_of::<(Self, Storage)>();
    /// Full logical charge for the incoming root and peer, including padding.
    pub const FD_PAIR_STORAGE: usize = 2 * size_of::<(OwnedFd, Storage)>();
    /// Both FD owners plus the complete retained native client charge.
    pub const INPUT_STORAGE: usize = Self::FD_PAIR_STORAGE + Client::RETAINED;

    // Admission snapshots add two fstats to the common continuity schedule.
    // Fewer than 32 outer syscalls, fixed-size comparisons and three terminal
    // closes fit the shared 512 weighted-call allowance. Each of the two nested
    // liveness operations separately prepays its two target/start-time pairs,
    // including every bounded procfs fallback attempt, on the SAME ledger.
    const OUTER_WORK: usize = native::operation_work(0);
    const OUTER_STORAGE: usize = native::operation_storage(Self::RETAINED, Self::FD_PAIR_STORAGE);
    /// Success allowance: outer inspection and two native client revalidations.
    /// Refusal charges only scopes entered, retaining any accepted work.
    pub const ADMISSION_WORK: usize = Self::OUTER_WORK + 2 * Client::REVALIDATION_WORK;
    pub const REVALIDATION_WORK: usize = Self::ADMISSION_WORK;
    /// Maximum additional logical scratch, including one nested client frame.
    /// Client revalidations are sequential, not two simultaneous record buffers.
    pub const IO_STORAGE: usize = Self::OUTER_STORAGE + Client::IO_STORAGE;

    /// Consumes prepaid owners; does not reset or replace the caller's ledger.
    /// Expected credentials are independently joined to SO_PEERCRED here.
    pub fn admit(
        root: OwnedFd,
        peer: OwnedFd,
        live_client: Client,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        Self::admit_inner::<true>(root, peer, live_client, budget)
    }

    fn admit_inner<const DISTINCT: bool>(
        root: OwnedFd,
        peer: OwnedFd,
        live_client: Client,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        #[cfg(not(test))]
        const {
            assert!(DISTINCT);
        }
        Self::scope(budget, Self::INPUT_STORAGE, |budget| {
            let admission = Self {
                root_identity: checks::inspect_object(&root, Kind::InspectRoot, "root")?,
                peer_identity: checks::inspect_object(&peer, Kind::InspectPeer, "peer")?,
                root,
                peer,
                live_client,
                service_uid: rustix::process::geteuid().as_raw(),
                #[cfg(test)]
                same_uid_fixture: !DISTINCT,
            };
            admission.inspect::<DISTINCT>(Kind::PeerCredentialsMismatch, budget)?;
            Ok((admission, Storage(Self::RETAINED - Self::INPUT_STORAGE)))
        })
    }

    /// Revalidates all retained roles and snapshots, bracketing credential
    /// inspection with native pidfd liveness checks. No descriptor is duplicated.
    pub fn validate_continuity(&self, budget: &mut Budget<'_>) -> Result<()> {
        Self::scope(budget, Self::RETAINED, |budget| {
            #[cfg(test)]
            if self.same_uid_fixture {
                return self.inspect::<false>(Kind::PeerCredentialsChanged, budget);
            }
            self.inspect::<true>(Kind::PeerCredentialsChanged, budget)
        })
    }

    fn inspect<const DISTINCT: bool>(
        &self,
        credential_kind: Kind,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        self.inspect_with::<DISTINCT>(credential_kind, budget, || {})
    }

    // Production supplies only the empty closure above. Private tests inject
    // an exit at the credential/liveness boundary without timing a race.
    fn inspect_with<const DISTINCT: bool>(
        &self,
        credential_kind: Kind,
        budget: &mut Budget<'_>,
        after_credentials: impl FnOnce(),
    ) -> Result<()> {
        self.check_service_uid()?;
        self.check_descriptor_flags()?;
        self.live_client.validate_liveness(budget)?;
        self.check_root()?;
        let peer = checks::inspect_object(&self.peer, Kind::InspectPeer, "peer")?;
        require_distinct_descriptors(self.root_identity, peer)?;
        require_distinct_pidfd_descriptor(
            self.root_identity,
            peer,
            self.live_client.descriptor_snapshot(),
        )?;
        if checks::validate_peer_shape(&self.peer)? != self.peer_identity {
            return Err(CheckError::new(
                Kind::PeerIdentityChanged,
                "retained service peer changed",
            )
            .into());
        }
        checks::validate_external_anchor_peer_status(&self.peer)?;
        if DISTINCT && self.expected_client().uid() == self.service_uid {
            return Err(CheckError::new(
                Kind::SameUidClient,
                "client UID equals protected service UID",
            )
            .into());
        }
        if checks::inspect_peer_credentials(&self.peer)? != self.expected_client().credentials() {
            return Err(CheckError::new(
                credential_kind,
                "service peer SO_PEERCRED differs from the retained client identity",
            )
            .into());
        }
        after_credentials();
        // Retained snapshots, rather than newly admitted replacements, must
        // survive the credential observation and the final native liveness check.
        if checks::inspect_object(&self.peer, Kind::InspectPeer, "peer")? != self.peer_identity {
            return Err(CheckError::new(
                Kind::PeerIdentityChanged,
                "service peer changed during credential inspection",
            )
            .into());
        }
        self.check_root()?;
        self.check_descriptor_flags()?;
        checks::validate_external_anchor_peer_status(&self.peer)?;
        self.live_client.validate_liveness(budget)?;
        self.check_service_uid()?;
        Ok(())
    }

    fn check_root(&self) -> std::result::Result<(), CheckError> {
        let identity = checks::inspect_object(&self.root, Kind::InspectRoot, "root")?;
        require_root_identity(identity, self.service_uid)?;
        if identity != self.root_identity {
            return Err(CheckError::new(
                Kind::RootIdentityChanged,
                "retained service root metadata changed",
            ));
        }
        Ok(())
    }

    fn check_service_uid(&self) -> std::result::Result<(), CheckError> {
        if rustix::process::geteuid().as_raw() != self.service_uid {
            return Err(CheckError::new(
                Kind::ServiceIdentityChanged,
                "protected service effective UID changed",
            ));
        }
        Ok(())
    }

    fn check_descriptor_flags(&self) -> std::result::Result<(), CheckError> {
        checks::require_close_on_exec(&self.service_root(), Kind::RootCloseOnExec, "service root")?;
        checks::require_close_on_exec(&self.service_peer(), Kind::PeerCloseOnExec, "service peer")?;
        checks::require_close_on_exec(
            &self.client_pidfd(),
            Kind::ClientPidfdCloseOnExec,
            "client pidfd",
        )
    }

    /// Connection-time credentials checked against the retained socket; not
    /// a fresh liveness or credential observation. Revalidate on each use.
    pub const fn expected_client(&self) -> ExpectedClientProcessIdentityV1 {
        self.live_client.expected_client()
    }

    /// Retire this full logical charge only after this owner has been dropped.
    pub const fn retained_storage(&self) -> usize {
        Self::RETAINED
    }

    // Broker consumers must fund and validate continuity at their use boundary.
    // These borrows convey no V1 service, issuer, signing or publication owner.
    pub(crate) fn service_root(&self) -> BorrowedFd<'_> {
        self.root.as_fd()
    }
    pub(crate) fn service_peer(&self) -> BorrowedFd<'_> {
        self.peer.as_fd()
    }
    pub(crate) fn client_pidfd(&self) -> BorrowedFd<'_> {
        self.live_client.pidfd()
    }
    pub(crate) const fn client_process_identity(&self) -> (u32, u64) {
        self.live_client.process_identity()
    }

    fn scope<T>(
        budget: &mut Budget<'_>,
        floor: usize,
        operation: impl FnOnce(&mut Budget<'_>) -> Result<T>,
    ) -> Result<T> {
        budget.with_prepaid_scope(
            floor,
            native::ENTRY_WORK,
            Self::OUTER_WORK,
            Self::OUTER_STORAGE,
            operation,
        )
    }
}

impl fmt::Debug for Service {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProtectedServiceAdmissionV2")
            .field("authority", &"none")
            .field("service_uid", &self.service_uid)
            .field("expected_client", &self.expected_client())
            .field("client_process_identity", &self.client_process_identity())
            .finish_non_exhaustive()
    }
}

const _: () = {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 as Ledger;
    assert!(Service::RETAINED >= Service::INPUT_STORAGE);
    assert!(Service::FD_PAIR_STORAGE >= size_of::<(OwnedFd, OwnedFd, Storage)>());
    // Native procfs scratch is accounted by the nested client's own frame. The
    // outer schedule only retains fixed snapshots, syscall/error and scope data.
    assert!(
        8 * size_of::<Error>()
            + 128 * size_of::<usize>()
            + size_of::<Ledger>()
            + 4 * size_of::<Result<(Service, Storage)>>()
            + 4 * size_of::<std::thread::Result<Result<()>>>()
            <= Service::OUTER_STORAGE
    );
    assert!(
        8 * size_of::<rustix::fs::Stat>()
            + 4 * size_of::<libc::sockaddr_un>()
            + 8 * size_of::<ObjectIdentityV1>()
            <= 4096
    );
};

#[cfg(test)]
#[path = "service_native_tests.rs"]
mod tests;
