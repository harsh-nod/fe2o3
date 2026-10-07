//! Original measured-supervisor registration ingress. No application Ready here.
use super::native_supervisor::RootNativeApplicationSupervisorV3 as Supervisor;
use crate::{
    ExpectedClientProcessIdentityV1 as Expected, LiveClientPidfdIdentityV2 as Client,
    RetainedNativeApplicationObservationV3 as Observation,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrStorageAccountIdentityV1 as Account,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use fe2o3_protected_service_spawn::launch_io as transport;
use fe2o3_runtime_protocol::{
    NATIVE_APPLICATION_ROOT_TRANSFER_BYTES_V1 as BYTES,
    NativeApplicationRegistrationBindingV1 as Binding, NativeApplicationRootTransferV1 as Transfer,
};
use rustix::{event, fs, io, net};
use std::{
    io as stdio,
    mem::size_of,
    os::fd::{AsFd, BorrowedFd, OwnedFd},
    time::{Duration, Instant},
};

#[path = "native_application_session.rs"]
mod session;
pub use session::ReceivedNativeApplicationV3;

type Result<T> = stdio::Result<T>;
fn other(error: impl std::error::Error + Send + Sync + 'static) -> stdio::Error {
    stdio::Error::other(error)
}
fn refused(reason: &'static str) -> stdio::Error {
    stdio::Error::other(reason)
}
fn transport_error(error: transport::Failure) -> stdio::Error {
    match error {
        transport::Failure::Io { source, .. } => source.into(),
        transport::Failure::InvalidTimeout => stdio::Error::new(
            stdio::ErrorKind::InvalidInput,
            "native registration invalid transport timeout",
        ),
        transport::Failure::Timeout(_) => stdio::Error::new(
            stdio::ErrorKind::TimedOut,
            "native registration transport timeout",
        ),
        transport::Failure::ChildExited(_) => stdio::Error::new(
            stdio::ErrorKind::BrokenPipe,
            "native registration transport closed",
        ),
        _ => stdio::Error::new(
            stdio::ErrorKind::InvalidData,
            "native registration transport framing",
        ),
    }
}

/// Consumed authenticated registration and independently inspected native inputs.
/// This is still pre-ACK custody: no Ready, currentness, proof, or launch authority.
/// Only the measured original supervisor's exact four-datagram transfer can
/// construct it. Partial transfer is terminal and closes every received right.
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::PendingRootNativeApplicationV3;
/// let _ = PendingRootNativeApplicationV3 {};
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::PendingRootNativeApplicationV3;
/// fn fd<T: std::os::fd::AsFd>() {} fd::<PendingRootNativeApplicationV3<'_, '_, '_>>();
/// ```
pub struct PendingRootNativeApplicationV3<'registry, 'custody, 'work> {
    supervisor: &'registry Supervisor<'custody, 'work>,
    observation: Observation<'work>,
    control: OwnedFd,
    service: OwnedFd,
    proof: OwnedFd,
    ack_reader: OwnedFd,
    snapshots: [(u64, u64); 4],
    nonce: [u8; 32],
    published_gate: Option<[u8; 32]>,
    publication_attempted: bool,
    deadline: Instant,
    ledger: Ledger,
    account: Option<Account>,
    thread: u32,
    retained: usize,
}
impl<'registry, 'custody, 'work> PendingRootNativeApplicationV3<'registry, 'custody, 'work> {
    pub const INPUT_STORAGE: usize = size_of::<OwnedFd>() + size_of::<usize>();
    pub const CONTROL_WORK: usize = 256 * 1024;
    pub const CONTROL_STORAGE: usize = 128 * 1024;

    /// One original-account, <=120-second aggregate transfer. The unfiltered root
    /// application service owns this call; a filtered compiler coordinator refuses.
    /// Reserve returned growth above INPUT_STORAGE before retaining the owner.
    pub fn receive(
        supervisor: &'registry Supervisor<'custody, 'work>,
        control: OwnedFd,
        timeout: Duration,
        b: &mut Budget<'work>,
    ) -> Result<(Self, usize)> {
        b.charge_work(Self::CONTROL_WORK).map_err(other)?;
        let floor = b.storage();
        if floor < Self::INPUT_STORAGE + supervisor.retained_storage() {
            return Err(refused("native registration inputs are not prepaid"));
        }
        b.reserve_storage(Self::CONTROL_STORAGE).map_err(other)?;
        if timeout.is_zero() || timeout > Duration::from_secs(120) {
            return Err(refused("native registration timeout"));
        }
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or_else(|| refused("native registration deadline"))?;
        supervisor.revalidate(b).map_err(other)?;
        unfiltered_root(supervisor.expected_root())?;
        channel(control.as_fd(), supervisor.expected_root())?;
        let mut nonce = [0; 32];
        let count = rustix::rand::getrandom(&mut nonce, rustix::rand::GetRandomFlags::empty())?;
        if count != nonce.len() || nonce == [0; 32] {
            return Err(refused("native registration entropy"));
        }
        let packet = Transfer::nonce_packet(nonce, b).map_err(other)?;
        if transport::send_packet(control.as_fd(), &packet)
            .map_err(transport_error)?
            .is_none()
        {
            return Err(refused("native registration nonce send unavailable"));
        }
        let expected = supervisor.expected_supervisor();
        let sender = transport::MessageSender::new(expected.pid as i32, expected.uid, expected.gid);
        let mut rights: [Option<OwnedFd>; 4] = [None, None, None, None];
        let mut binding: Option<Binding> = None;
        for slot in 0..4 {
            wait_readable(control.as_fd(), deadline)?;
            supervisor.revalidate(b).map_err(other)?;
            channel(control.as_fd(), supervisor.expected_root())?;
            b.charge_work(transport::packet_receive_work(BYTES))
                .map_err(other)?;
            b.reserve_storage(
                transport::packet_receive_scratch(BYTES) + BYTES + Client::FD_STORAGE,
            )
            .map_err(other)?;
            let (bytes, fd) =
                transport::receive_authenticated_descriptor::<BYTES>(control.as_fd(), sender)
                    .map_err(transport_error)?
                    .ok_or_else(|| refused("native registration receive unavailable"))?;
            rights[slot] = Some(fd);
            let frame = Transfer::decode(&bytes, slot as u8, nonce, b).map_err(other)?;
            b.reserve_storage(Transfer::STORAGE).map_err(other)?;
            if let Some(binding) = &binding {
                if frame.binding_bytes() != binding.canonical_bytes() {
                    return Err(refused("native registration packet binding changed"));
                }
            } else {
                let (value, charge) = Binding::decode(frame.binding_bytes(), b).map_err(other)?;
                b.reserve_storage(charge.additional_storage())
                    .map_err(other)?;
                binding = Some(value);
            }
            remaining(deadline)?;
        }
        let [service, application, proof, parent] =
            rights.map(|fd| fd.expect("four completed packets"));
        let binding = binding.expect("first packet decoded");
        wait_readable(control.as_fd(), deadline)?;
        supervisor.revalidate(b).map_err(other)?;
        b.charge_work(transport::packet_receive_work(72))
            .map_err(other)?;
        b.reserve_storage(transport::packet_receive_scratch(72) + 72 + Client::FD_STORAGE)
            .map_err(other)?;
        let (ack_packet, ack_reader) =
            transport::receive_authenticated_descriptor::<72>(control.as_fd(), sender)
                .map_err(transport_error)?
                .ok_or_else(|| refused("native original ACK reader unavailable"))?;
        if ack_packet
            != Transfer::root_ack_reader_packet(nonce, *binding.identity().as_bytes(), b)
                .map_err(other)?
        {
            return Err(refused("native root ACK reader frame changed"));
        }
        validate_ack_reader(ack_reader.as_fd(), &binding)?;
        // Only the measured sender's post-drop completion closes the alias
        // transfer. Four received descriptors alone are not a completed owner.
        wait_readable(control.as_fd(), deadline)?;
        supervisor.revalidate(b).map_err(other)?;
        channel(control.as_fd(), supervisor.expected_root())?;
        b.charge_work(transport::packet_receive_work(72))
            .map_err(other)?;
        b.reserve_storage(transport::packet_receive_scratch(72) + 72)
            .map_err(other)?;
        let complete = transport::receive_authenticated_packet::<72>(control.as_fd(), sender)
            .map_err(transport_error)?
            .ok_or_else(|| refused("native registration completion unavailable"))?;
        let expected_complete =
            Transfer::completion_packet(nonce, *binding.identity().as_bytes(), b).map_err(other)?;
        if complete != expected_complete {
            return Err(refused("native registration completion changed"));
        }
        let manifest = binding.compiler_handoff().launch_manifest();
        if manifest.policy_identity() != supervisor.deployment().policy().identity()
            || manifest.external_anchor_service()
                != supervisor.deployment().profile().external_anchor_service()
        {
            return Err(refused(
                "native registration installed policy or anchor mismatch",
            ));
        }
        let app = manifest.client();
        let cargo = binding.compiler_handoff().submitter();
        let objects = [
            snapshot(control.as_fd())?,
            snapshot(service.as_fd())?,
            snapshot(application.as_fd())?,
            snapshot(proof.as_fd())?,
            snapshot(parent.as_fd())?,
            snapshot(ack_reader.as_fd())?,
        ];
        distinct(&objects)?;
        let (application, charge) = Client::admit(
            application,
            Expected {
                pid: app.pid(),
                uid: app.uid(),
                gid: app.gid(),
            },
            b,
        )
        .map_err(other)?;
        b.reserve_storage(charge.additional_storage())
            .map_err(other)?;
        let (parent, charge) = Client::admit(
            parent,
            Expected {
                pid: cargo.pid(),
                uid: cargo.uid(),
                gid: cargo.gid(),
            },
            b,
        )
        .map_err(other)?;
        b.reserve_storage(charge.additional_storage())
            .map_err(other)?;
        application.validate_parent(&parent, b).map_err(other)?;
        service_peer(service.as_fd(), application.expected_client())?;
        let (observation, charge) =
            Observation::observe_pre_ack(application, parent, binding, proof.as_fd(), b)
                .map_err(other)?;
        b.reserve_storage(charge.additional_storage())
            .map_err(other)?;
        let retained = size_of::<Self>()
            .checked_add(observation.retained_storage())
            .ok_or_else(|| refused("native registration retained overflow"))?;
        let value = Self {
            supervisor,
            observation,
            control,
            service,
            proof,
            ack_reader,
            snapshots: [objects[0], objects[1], objects[3], objects[5]],
            nonce,
            published_gate: None,
            publication_attempted: false,
            deadline,
            ledger: b.work_ledger_identity_v1(),
            account: b.storage_account_identity_v1(),
            thread: rustix::thread::gettid().as_raw_pid() as u32,
            retained,
        };
        value.check(b)?;
        let growth = retained
            .checked_sub(Self::INPUT_STORAGE)
            .ok_or_else(|| refused("native registration retained underflow"))?;
        b.release_storage(
            b.storage()
                .checked_sub(floor)
                .ok_or_else(|| refused("native registration accounting"))?,
        )
        .map_err(other)?;
        Ok((value, growth))
    }
    pub const fn binding(&self) -> &Binding {
        self.observation.binding()
    }
    pub(crate) const fn deployment(
        &self,
    ) -> &'custody fe2o3_compiler_closure_capability::RootProductionCompilerExecutionDeploymentV3<'work>
    {
        self.supervisor.deployment()
    }
    pub(crate) const fn root_client(&self) -> &Client {
        self.supervisor.root_client()
    }
    pub(crate) const fn application(&self) -> &Client {
        self.observation.application()
    }
    pub(crate) const fn parent(&self) -> &Client {
        self.observation.parent()
    }
    pub(crate) const fn transfer_nonce(&self) -> [u8; 32] {
        self.nonce
    }
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub const fn startup_deadline(&self) -> Instant {
        self.deadline
    }
    pub const fn authenticates_currentness(&self) -> bool {
        false
    }
    pub(crate) fn publish_currentness(
        &mut self,
        ready: &fe2o3_compiler_execution_protocol::CompilerExecutionServiceReadyV3,
        issuer: BorrowedFd<'_>,
        gate: [u8; 32],
        b: &mut Budget<'work>,
    ) -> Result<()> {
        self.revalidate(b)?;
        if self.publication_attempted {
            return Err(refused("native service already published"));
        }
        // A partially sent or refused publication can never be retried.
        self.publication_attempted = true;
        let packet = Transfer::publication_packet(
            ready,
            self.nonce,
            *self.binding().identity().as_bytes(),
            gate,
            b,
        )
        .map_err(other)?;
        b.reserve_storage(224 + transport::packet_receive_scratch(104) + 104)
            .map_err(other)?;
        b.charge_work(transport::packet_receive_work(224) + transport::packet_receive_work(104))
            .map_err(other)?;
        if transport::send_packet_with_descriptor(self.control.as_fd(), &packet, issuer)
            .map_err(transport_error)?
            .is_none()
        {
            return Err(refused("native root publication unavailable"));
        }
        wait_readable(self.control.as_fd(), self.deadline)?;
        self.revalidate(b)?;
        let expected = self.supervisor.expected_supervisor();
        let packet = transport::receive_authenticated_packet::<104>(
            self.control.as_fd(),
            transport::MessageSender::new(expected.pid as i32, expected.uid, expected.gid),
        )
        .map_err(transport_error)?
        .ok_or_else(|| refused("native publication completion unavailable"))?;
        if packet
            != Transfer::publication_completion(
                self.nonce,
                *self.binding().identity().as_bytes(),
                gate,
                b,
            )
            .map_err(other)?
        {
            return Err(refused("native publication completion changed"));
        }
        self.revalidate(b)?;
        self.published_gate = Some(gate);
        Ok(())
    }
    /// Borrows only this authenticated registration's original launch inputs.
    /// The callback cannot return a borrowed descriptor. It must account for any
    /// duplicates it stages, retain original cleanup custody, and close aliases
    /// before completing its root connection. No input replacement is accepted.
    /// This is not a constructor for a compiler trace or an admitted issuer.
    pub fn with_currentness_launch_inputs<R>(
        &self,
        b: &mut Budget<'work>,
        launch: impl for<'fd> FnOnce(
            fe2o3_compiler_execution_protocol::CompilerExecutionClientProcessIdentityV1,
            BorrowedFd<'fd>,
            BorrowedFd<'fd>,
            &mut Budget<'work>,
        ) -> Result<R>,
    ) -> Result<R> {
        self.revalidate(b)?;
        let value = launch(
            self.binding().compiler_handoff().launch_manifest().client(),
            self.service.as_fd(),
            self.observation.application().pidfd(),
            b,
        )?;
        self.revalidate(b)?;
        Ok(value)
    }
    pub fn revalidate(&self, b: &mut Budget<'work>) -> Result<()> {
        b.charge_work(Self::CONTROL_WORK).map_err(other)?;
        if b.storage() < self.retained {
            return Err(refused("native registration owner is not prepaid"));
        }
        b.reserve_storage(Self::CONTROL_STORAGE).map_err(other)?;
        self.check(b)?;
        b.release_storage(Self::CONTROL_STORAGE).map_err(other)
    }
    fn check(&self, b: &mut Budget<'work>) -> Result<()> {
        if self.ledger != b.work_ledger_identity_v1()
            || self.account != b.storage_account_identity_v1()
            || self.thread != rustix::thread::gettid().as_raw_pid() as u32
        {
            return Err(refused("native registration account or thread changed"));
        }
        remaining(self.deadline)?;
        self.supervisor.revalidate(b).map_err(other)?;
        unfiltered_root(self.supervisor.expected_root())?;
        channel(self.control.as_fd(), self.supervisor.expected_root())?;
        service_peer(
            self.service.as_fd(),
            self.observation.application().expected_client(),
        )?;
        if [
            snapshot(self.control.as_fd())?,
            snapshot(self.service.as_fd())?,
            snapshot(self.proof.as_fd())?,
            snapshot(self.ack_reader.as_fd())?,
        ] != self.snapshots
        {
            return Err(refused("native registration descriptor changed"));
        }
        self.observation
            .revalidate(self.proof.as_fd(), b)
            .map_err(other)?;
        validate_ack_reader(self.ack_reader.as_fd(), self.binding())?;
        remaining(self.deadline)
    }
}

fn validate_ack_reader(fd: BorrowedFd<'_>, binding: &Binding) -> Result<()> {
    let stat = fs::fstat(fd)?;
    let flags = fs::fcntl_getfl(fd)?;
    let occurrence =
        fe2o3_runtime_protocol::WorkerV3ApplicationInputOccurrenceV1::from_linux_descriptor_v1(
            3,
            stat.st_dev,
            stat.st_ino,
            stat.st_mode,
        )
        .map_err(other)?;
    if fs::FileType::from_raw_mode(stat.st_mode) != fs::FileType::Fifo
        || flags & fs::OFlags::ACCMODE != fs::OFlags::RDONLY
        || !flags.contains(fs::OFlags::NONBLOCK)
        || !io::fcntl_getfd(fd)?.contains(io::FdFlags::CLOEXEC)
        || binding.inputs().occurrence().inputs().get(2) != Some(&occurrence)
    {
        return Err(refused(
            "native ACK reader differs from original observed pipe",
        ));
    }
    Ok(())
}
fn remaining(deadline: Instant) -> Result<()> {
    if deadline
        .checked_duration_since(Instant::now())
        .is_none_or(|d| d.is_zero() || d > Duration::from_secs(120))
    {
        return Err(refused("native registration deadline expired"));
    }
    Ok(())
}
fn wait_readable(fd: BorrowedFd<'_>, deadline: Instant) -> Result<()> {
    remaining(deadline)?;
    let timeout = event::Timespec::try_from(deadline.saturating_duration_since(Instant::now()))
        .map_err(other)?;
    let mut descriptors = [event::PollFd::new(&fd, event::PollFlags::IN)];
    if event::poll(&mut descriptors, Some(&timeout))? == 0
        || !descriptors[0].revents().contains(event::PollFlags::IN)
        || descriptors[0]
            .revents()
            .intersects(event::PollFlags::ERR | event::PollFlags::NVAL)
    {
        return Err(refused("native registration poll unavailable"));
    }
    Ok(())
}
fn snapshot(fd: BorrowedFd<'_>) -> Result<(u64, u64)> {
    let stat = fs::fstat(fd)?;
    Ok((stat.st_dev, stat.st_ino))
}
fn distinct(values: &[(u64, u64)]) -> Result<()> {
    for (index, value) in values.iter().enumerate() {
        if values[..index].contains(value) {
            return Err(refused("native registration descriptor roles alias"));
        }
    }
    Ok(())
}
fn channel(fd: BorrowedFd<'_>, root: Expected) -> Result<()> {
    if io::fcntl_getfd(fd)? != io::FdFlags::CLOEXEC
        || fs::fcntl_getfl(fd)? != (fs::OFlags::RDWR | fs::OFlags::NONBLOCK)
        || net::sockopt::socket_domain(fd)? != net::AddressFamily::UNIX
        || net::sockopt::socket_type(fd)? != net::SocketType::SEQPACKET
        || !net::sockopt::socket_passcred(fd)?
        || !crate::compiler_execution_root_channel::socketpair_address(net::getsockname(fd)?)
        || !net::getpeername(fd)?
            .is_some_and(crate::compiler_execution_root_channel::socketpair_address)
    {
        return Err(refused("native registration channel shape"));
    }
    let peer = net::sockopt::socket_peercred(fd)?;
    if peer.pid.as_raw_pid() as u32 != root.pid
        || peer.uid.as_raw() != root.uid
        || peer.gid.as_raw() != root.gid
    {
        return Err(refused("native registration root creator"));
    }
    Ok(())
}
fn service_peer(fd: BorrowedFd<'_>, parent: Expected) -> Result<()> {
    if io::fcntl_getfd(fd)? != io::FdFlags::CLOEXEC
        || fs::fcntl_getfl(fd)? != (fs::OFlags::RDWR | fs::OFlags::NONBLOCK)
        || net::sockopt::socket_domain(fd)? != net::AddressFamily::UNIX
        || net::sockopt::socket_type(fd)? != net::SocketType::SEQPACKET
    {
        return Err(refused("native service endpoint shape"));
    }
    let peer = net::sockopt::socket_peercred(fd)?;
    if peer.pid.as_raw_pid() as u32 != parent.pid
        || peer.uid.as_raw() != parent.uid
        || peer.gid.as_raw() != parent.gid
    {
        return Err(refused("native service endpoint creator"));
    }
    Ok(())
}
#[allow(unsafe_code)]
fn unfiltered_root(root: Expected) -> Result<()> {
    // A query only, never a filter change. Filtered compiler coordinators cannot
    // become the independent application/proof manager by calling this API.
    let mode = unsafe { libc::prctl(libc::PR_GET_SECCOMP, 0, 0, 0, 0) };
    if mode < 0 {
        return Err(stdio::Error::last_os_error());
    }
    if mode != 0 || root.pid != std::process::id() || root.uid != 0 || root.gid != 0 {
        return Err(refused("native application root must be unfiltered"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn alias_and_aggregate_deadline_checks_fail_closed() {
        assert!(distinct(&[(1, 2), (1, 3), (2, 2)]).is_ok());
        assert!(distinct(&[(1, 2), (1, 3), (1, 2)]).is_err());
        assert!(remaining(Instant::now()).is_err());
        assert!(remaining(Instant::now() + Duration::from_secs(121)).is_err());
        assert!(remaining(Instant::now() + Duration::from_secs(1)).is_ok());
    }
    #[test]
    fn transport_conversion_preserves_errno_and_failure_categories() {
        assert_eq!(
            transport_error(transport::Failure::Io {
                operation: "test",
                source: io::Errno::PERM
            })
            .raw_os_error(),
            Some(libc::EPERM)
        );
        assert_eq!(
            transport_error(transport::Failure::Timeout("test")).kind(),
            stdio::ErrorKind::TimedOut
        );
        assert_eq!(
            transport_error(transport::Failure::MalformedReadyTransfer).kind(),
            stdio::ErrorKind::InvalidData
        );
    }
    #[test]
    fn no_caller_root_identity_override_is_accepted() {
        assert!(
            unfiltered_root(Expected {
                pid: std::process::id(),
                uid: 1,
                gid: 0
            })
            .is_err()
        );
    }
}
