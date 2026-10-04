//! Separate application admission; no root observation or compiler-only downgrade.

use super::*;
use fe2o3_runtime_protocol::{
    WORKER_V3_APPLICATION_REGISTRATION_BYTES_V1, WorkerV3ApplicationInputOccurrenceV1,
    WorkerV3ApplicationRegistrationBindingV1, WorkerV3ApplicationRegistrationErrorV1,
};
use rustix::fs::OFlags;

/// Failure admitting the dedicated application transport, without any root authority.
#[derive(Debug)]
pub enum ProtectedApplicationHandoffErrorV1 {
    /// An existing compiler/control/process admission check failed.
    Handoff(ProtectedIssuerHandoffErrorV1),
    /// The exact application binding could not be decoded.
    Binding(WorkerV3ApplicationRegistrationErrorV1),
    /// The candidate proof peer's shape, creator, or continuity is invalid.
    ProofPeer(&'static str),
}

impl fmt::Display for ProtectedApplicationHandoffErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Handoff(error) => write!(f, "application handoff: {error}"),
            Self::Binding(error) => write!(f, "application binding: {error}"),
            Self::ProofPeer(reason) => write!(f, "application proof peer: {reason}"),
        }
    }
}

impl Error for ProtectedApplicationHandoffErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Handoff(error) => Some(error),
            Self::Binding(error) => Some(error),
            Self::ProofPeer(_) => None,
        }
    }
}

impl From<ProtectedIssuerHandoffErrorV1> for ProtectedApplicationHandoffErrorV1 {
    fn from(error: ProtectedIssuerHandoffErrorV1) -> Self {
        Self::Handoff(error)
    }
}

impl From<rustix::io::Errno> for ProtectedApplicationHandoffErrorV1 {
    fn from(error: rustix::io::Errno) -> Self {
        Self::Handoff(ProtectedIssuerHandoffErrorV1::Io(error.into()))
    }
}

/// Original application/Cargo process custody and a candidate Cargo-side proof peer.
///
/// This is not root observation, readiness, proof ownership, or execution authority. It cannot
/// be converted to an ordinary accepted compiler handoff or passed to ordinary issuer launch.
/// Root observation must still authenticate the reversed addresses and exact slot-4 object;
/// Cargo creator credentials alone cannot exclude another pair created by the same Cargo.
///
/// ```compile_fail
/// fn cloneable<T: Clone>() {}
/// cloneable::<fe2o3_compiler_execution_supervisor::AcceptedApplicationHandoffV1>();
/// ```
/// ```compile_fail
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<fe2o3_compiler_execution_supervisor::AcceptedApplicationHandoffV1>();
/// ```
/// ```compile_fail
/// fn downgrade(value: fe2o3_compiler_execution_supervisor::AcceptedApplicationHandoffV1)
///     -> fe2o3_compiler_execution_supervisor::AcceptedCompilerExecutionHandoffV1 {
///     value.into()
/// }
/// ```
pub struct AcceptedApplicationHandoffV1 {
    compiler: AcceptedCompilerExecutionHandoffV1,
    binding: WorkerV3ApplicationRegistrationBindingV1,
    proof_peer: OwnedFd,
    parent_pidfd: OwnedFd,
    live_parent: LiveClientPidfdIdentityV1,
    proof_snapshot: ProofSnapshot,
    parent_snapshot: DescriptorSnapshotV1,
}

impl fmt::Debug for AcceptedApplicationHandoffV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AcceptedApplicationHandoffV1")
            .field("binding", &self.binding.identity())
            .field("authority", &"none")
            .finish_non_exhaustive()
    }
}

impl AcceptedApplicationHandoffV1 {
    /// Returns descriptive binding data, not root observation or application readiness.
    pub const fn binding(&self) -> &WorkerV3ApplicationRegistrationBindingV1 {
        &self.binding
    }

    /// Rechecks original process parentage, exact descriptors, and proof endpoint continuity.
    pub fn revalidate(
        &self,
        supervisor: &ProtectedIssuerSupervisorV1,
    ) -> Result<(), ProtectedApplicationHandoffErrorV1> {
        self.compiler.revalidate(supervisor)?;
        validate_control_shape(&self.compiler.control)?;
        if &self.compiler.handoff != self.binding.compiler_handoff() {
            return Err(ProtectedIssuerHandoffErrorV1::DescriptorChanged.into());
        }
        if rustix::io::fcntl_getfd(&self.parent_pidfd)? != rustix::io::FdFlags::CLOEXEC
            || rustix::io::fcntl_getfd(&self.compiler.client_pidfd)? != rustix::io::FdFlags::CLOEXEC
            || descriptor_snapshot(&self.parent_pidfd)? != self.parent_snapshot
            || proof_snapshot(&self.proof_peer, self.compiler.submitter(), &self.binding)?
                != self.proof_snapshot
        {
            return Err(ProtectedIssuerHandoffErrorV1::DescriptorChanged.into());
        }
        let objects = [
            self.compiler.control_snapshot,
            self.compiler.service_peer_snapshot,
            self.compiler.client_pidfd_snapshot,
            self.proof_snapshot.object,
            self.parent_snapshot,
        ];
        for (index, value) in objects.iter().enumerate() {
            // Older anon-inode pidfds can share metadata. Exact target admission above and
            // parentage below distinguish the two process roles independently of fstat.
            if objects[..index]
                .iter()
                .enumerate()
                .any(|(other_index, other)| {
                    !(index == 4 && other_index == 2)
                        && (other.device, other.inode) == (value.device, value.inode)
                })
            {
                return Err(ProtectedIssuerHandoffErrorV1::DescriptorAlias.into());
            }
        }
        self.compiler
            .live_client
            .validate_parent(&self.live_parent)
            .map_err(ProtectedIssuerHandoffErrorV1::Pidfd)?;
        Ok(())
    }
}

impl ProtectedIssuerSupervisorV1 {
    /// Admits only the exact four-right application profile on a dedicated control connection.
    ///
    /// Production session dispatch is deliberately separate until root application readiness
    /// can be enforced. Calling the compiler-only receiver on this packet fails closed.
    pub fn accept_application_handoff(
        &self,
        control: OwnedFd,
        timeout: Duration,
    ) -> Result<AcceptedApplicationHandoffV1, ProtectedApplicationHandoffErrorV1> {
        self.accept_application_handoff_inner::<true>(control, timeout)
    }

    pub(crate) fn accept_application_handoff_inner<const REQUIRE_DISTINCT_UID: bool>(
        &self,
        control: OwnedFd,
        timeout: Duration,
    ) -> Result<AcceptedApplicationHandoffV1, ProtectedApplicationHandoffErrorV1> {
        if timeout.is_zero() || timeout > Duration::from_secs(120) {
            return Err(ProtectedIssuerHandoffErrorV1::InvalidTimeout.into());
        }
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(ProtectedIssuerHandoffErrorV1::DeadlineOverflow)?;
        self.revalidate()
            .map_err(ProtectedIssuerHandoffErrorV1::Supervisor)?;
        validate_control_shape(&control)?;
        let submitter = control_peer_identity(&control)?;
        if REQUIRE_DISTINCT_UID && submitter.uid() == self.credentials().uid() {
            return Err(ProtectedIssuerHandoffErrorV1::ClientAndSupervisorUidMatch.into());
        }
        let control_snapshot = descriptor_snapshot(&control)?;
        let (payload, [service_peer, client_pidfd, proof_peer, parent_pidfd]) =
            receive(&control, deadline)?;
        let binding = WorkerV3ApplicationRegistrationBindingV1::decode(&payload)
            .map_err(ProtectedApplicationHandoffErrorV1::Binding)?;
        let compiler = self.admit_received_handoff::<REQUIRE_DISTINCT_UID>(
            control,
            binding.compiler_handoff().clone(),
            service_peer,
            client_pidfd,
            control_snapshot,
        )?;
        let parent_snapshot = descriptor_snapshot(&parent_pidfd)?;
        let parent = compiler.submitter();
        let expected =
            ExpectedClientProcessIdentityV1::new(parent.pid(), parent.uid(), parent.gid())
                .map_err(ProtectedIssuerHandoffErrorV1::Pidfd)?;
        let live_parent = LiveClientPidfdIdentityV1::admit(
            rustix::io::fcntl_dupfd_cloexec(&parent_pidfd, 3)?,
            expected,
        )
        .map_err(ProtectedIssuerHandoffErrorV1::Pidfd)?;
        let accepted = AcceptedApplicationHandoffV1 {
            proof_snapshot: proof_snapshot(&proof_peer, parent, &binding)?,
            parent_snapshot,
            compiler,
            binding,
            proof_peer,
            parent_pidfd,
            live_parent,
        };
        accepted.revalidate(self)?;
        if Instant::now() >= deadline {
            return Err(ProtectedIssuerHandoffErrorV1::Timeout.into());
        }
        Ok(accepted)
    }
}

#[derive(Debug, Eq, PartialEq)]
struct ProofSnapshot {
    object: DescriptorSnapshotV1,
    local: SocketAddrUnix,
    remote: SocketAddrUnix,
}

fn proof_snapshot(
    peer: &OwnedFd,
    parent: CompilerExecutionClientProcessIdentityV1,
    binding: &WorkerV3ApplicationRegistrationBindingV1,
) -> Result<ProofSnapshot, ProtectedApplicationHandoffErrorV1> {
    let flags = rustix::fs::fcntl_getfl(peer)?;
    if rustix::io::fcntl_getfd(peer)? != rustix::io::FdFlags::CLOEXEC
        || flags & OFlags::ACCMODE != OFlags::RDWR
        || !flags.contains(OFlags::NONBLOCK)
        || flags.intersects(OFlags::PATH | OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT)
        || rustix::net::sockopt::socket_domain(peer)? != AddressFamily::UNIX
        || rustix::net::sockopt::socket_type(peer)? != SocketType::SEQPACKET
        || rustix::net::sockopt::socket_acceptconn(peer)?
        || !rustix::net::sockopt::socket_passcred(peer)?
        || control_peer_identity(peer)? != parent
    {
        return Err(ProtectedApplicationHandoffErrorV1::ProofPeer(
            "shape, flags or Cargo creator mismatch",
        ));
    }
    let local = SocketAddrUnix::try_from(rustix::net::getsockname(peer)?)?;
    let remote = SocketAddrUnix::try_from(rustix::net::getpeername(peer)?.ok_or(
        ProtectedApplicationHandoffErrorV1::ProofPeer("disconnected peer"),
    )?)?;
    if local.abstract_name().is_none() || remote.abstract_name().is_none() || local == remote {
        return Err(ProtectedApplicationHandoffErrorV1::ProofPeer(
            "expected distinct abstract addresses",
        ));
    }
    let object = descriptor_snapshot(peer)?;
    let input = WorkerV3ApplicationInputOccurrenceV1::from_linux_descriptor_v1(
        4,
        object.device,
        object.inode,
        object.mode,
    )
    .map_err(|_| ProtectedApplicationHandoffErrorV1::ProofPeer("invalid object"))?;
    if binding.occurrence().inputs().get(3) == Some(&input) {
        return Err(ProtectedApplicationHandoffErrorV1::ProofPeer(
            "received app endpoint instead of counterpart",
        ));
    }
    Ok(ProofSnapshot {
        object,
        local,
        remote,
    })
}

fn receive(
    control: &OwnedFd,
    deadline: Instant,
) -> Result<
    (
        [u8; WORKER_V3_APPLICATION_REGISTRATION_BYTES_V1],
        [OwnedFd; 4],
    ),
    ProtectedIssuerHandoffErrorV1,
> {
    loop {
        wait_readable(control, deadline)?;
        let mut payload = [0; WORKER_V3_APPLICATION_REGISTRATION_BYTES_V1];
        let mut vectors = [IoSliceMut::new(&mut payload)];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(5))];
        let mut ancillary = RecvAncillaryBuffer::new(&mut space);
        let received = match recvmsg(
            control,
            &mut vectors,
            &mut ancillary,
            RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
        ) {
            Ok(received) => received,
            Err(rustix::io::Errno::INTR | rustix::io::Errno::AGAIN) => continue,
            Err(error) => return Err(ProtectedIssuerHandoffErrorV1::Io(error.into())),
        };
        let mut rights = Vec::with_capacity(5);
        let mut invalid_ancillary = false;
        for message in ancillary.drain() {
            match message {
                RecvAncillaryMessage::ScmRights(received) => rights.extend(received),
                _ => invalid_ancillary = true,
            }
        }
        if received.bytes != payload.len()
            || received
                .flags
                .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
            || invalid_ancillary
            || rights.len() != 4
        {
            return Err(ProtectedIssuerHandoffErrorV1::MalformedTransfer);
        }
        return Ok((payload, rights.try_into().expect("exact four rights")));
    }
}
