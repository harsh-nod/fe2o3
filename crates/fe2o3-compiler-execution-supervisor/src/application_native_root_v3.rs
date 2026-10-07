//! Consuming original application transfer to its independently authenticating root.
use super::*;
use fe2o3_protected_service_spawn::launch_io;
use fe2o3_runtime_protocol::NativeApplicationRootTransferV1 as Transfer;
use std::io;
use std::os::fd::{AsFd, BorrowedFd};

/// Only control custody remains after the original four owners were transferred
/// and every local alias was dropped. Only an authenticated original-root
/// currentness publication can forward application service readiness.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::PendingNativeApplicationRootTransferV3;
/// fn descriptor<T: std::os::fd::AsFd>() {} descriptor::<PendingNativeApplicationRootTransferV3<'_, '_>>();
/// ```
pub struct PendingNativeApplicationRootTransferV3<'supervisor, 'work> {
    supervisor: &'supervisor Supervisor,
    application_control: OwnedFd,
    root_control: OwnedFd,
    binding: Binding,
    nonce: [u8; 32],
    root: rustix::process::Pid,
    control_snapshot: Snapshot,
    root_snapshot: Snapshot,
    deadline: Instant,
    ledger: Ledger,
    storage_account: Option<StorageAccount>,
    retained: usize,
    work: PhantomData<&'work Work>,
}
type Pending<'a, 'w> = PendingNativeApplicationRootTransferV3<'a, 'w>;
fn error(e: impl std::error::Error + Send + Sync + 'static) -> io::Error {
    io::Error::other(e)
}
fn refuse(message: &'static str) -> io::Error {
    io::Error::other(message)
}
fn transport_error(error: launch_io::Failure) -> io::Error {
    match error {
        launch_io::Failure::Io { source, .. } => source.into(),
        launch_io::Failure::InvalidTimeout => io::Error::new(
            io::ErrorKind::InvalidInput,
            "native root transfer invalid transport timeout",
        ),
        launch_io::Failure::Timeout(_) => io::Error::new(
            io::ErrorKind::TimedOut,
            "native root transfer transport timeout",
        ),
        launch_io::Failure::ChildExited(_) => io::Error::new(
            io::ErrorKind::BrokenPipe,
            "native root transfer transport closed",
        ),
        _ => io::Error::new(
            io::ErrorKind::InvalidData,
            "native root transfer transport framing",
        ),
    }
}

impl<'work> Accepted<'work> {
    /// Sends four exact one-right native frames on the original root-created
    /// channel. One nonce and <=120-second deadline bind the entire sequence.
    /// Consuming failure closes all local owners; no partial-success owner exists.
    /// The returned growth is unreserved above the consumed accepted/control
    /// charges. All cumulative work remains on the original account.
    pub fn transfer_to_root<'supervisor>(
        self,
        supervisor: &'supervisor Supervisor,
        root_control: OwnedFd,
        timeout: Duration,
        b: &mut Budget<'work>,
    ) -> io::Result<(Pending<'supervisor, 'work>, usize)> {
        b.charge_work(Self::WORK).map_err(error)?;
        if self.ledger != b.work_ledger_identity_v1()
            || self.storage_account != b.storage_account_identity_v1()
        {
            return Err(refuse("native root transfer account changed"));
        }
        let input = self
            .retained
            .checked_add(Self::CONTROL_STORAGE)
            .ok_or_else(|| refuse("native root transfer accounting"))?;
        let floor = b.storage();
        if floor < input + supervisor.retained_storage() {
            return Err(refuse("native root transfer inputs not prepaid"));
        }
        b.reserve_storage(Self::SCRATCH).map_err(error)?;
        if timeout.is_zero() || timeout > Duration::from_secs(120) {
            return Err(refuse("native root transfer timeout"));
        }
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or_else(|| refuse("native root transfer deadline"))?;
        self.check(supervisor, b).map_err(error)?;
        let root = rustix::process::getppid()
            .ok_or_else(|| refuse("native root transfer parent absent"))?;
        root_shape(root_control.as_fd(), root)?;
        let root_snapshot =
            checks::snapshot(&root_control).map_err(|e| error(HandoffError::from(e)))?;
        // The original four roles stay unchanged. The separate exact reader
        // transfer is followed by Cargo's typed post-drop closure frame.
        let cargo = self.binding.compiler_handoff().submitter();
        let sender = launch_io::MessageSender::new(cargo.pid() as i32, cargo.uid(), cargo.gid());
        poll(self.compiler.control.as_fd(), deadline)?;
        b.charge_work(2 * launch_io::packet_receive_work(40))
            .map_err(error)?;
        b.reserve_storage(
            2 * (launch_io::packet_receive_scratch(40) + 40) + LiveClient::FD_STORAGE,
        )
        .map_err(error)?;
        let (packet, ack_reader) = launch_io::receive_authenticated_descriptor::<40>(
            self.compiler.control.as_fd(),
            sender,
        )
        .map_err(transport_error)?
        .ok_or_else(|| refuse("native ACK reader transfer unavailable"))?;
        let binding_id = *self.binding.identity().as_bytes();
        if packet != Transfer::ack_reader_packet(binding_id, b).map_err(error)? {
            return Err(refuse("native ACK reader registration changed"));
        }
        validate_ack_reader(ack_reader.as_fd(), &self.binding)?;
        poll(self.compiler.control.as_fd(), deadline)?;
        let closed =
            launch_io::receive_authenticated_packet::<40>(self.compiler.control.as_fd(), sender)
                .map_err(transport_error)?
                .ok_or_else(|| refuse("native ACK reader closure unavailable"))?;
        if closed != Transfer::ack_reader_closed_packet(binding_id, b).map_err(error)? {
            return Err(refuse("native ACK reader closure changed"));
        }
        self.check(supervisor, b).map_err(error)?;
        poll(root_control.as_fd(), deadline)?;
        b.charge_work(launch_io::packet_receive_work(40))
            .map_err(error)?;
        b.reserve_storage(launch_io::packet_receive_scratch(40) + 40)
            .map_err(error)?;
        let packet = launch_io::receive_authenticated_packet::<40>(
            root_control.as_fd(),
            launch_io::MessageSender::new(root.as_raw_pid(), 0, 0),
        )
        .map_err(transport_error)?
        .ok_or_else(|| refuse("native root nonce unavailable"))?;
        let nonce = Transfer::decode_nonce(&packet, b).map_err(error)?;
        let (app, charge) = self
            .compiler
            .client
            .try_clone_for_transfer(b)
            .map_err(error)?;
        b.reserve_storage(charge.additional_storage())
            .map_err(error)?;
        let (cargo, charge) = self.parent.try_clone_for_transfer(b).map_err(error)?;
        b.reserve_storage(charge.additional_storage())
            .map_err(error)?;
        let descriptors = [
            self.compiler.service_peer.as_fd(),
            app.as_fd(),
            self.proof_peer.as_fd(),
            cargo.as_fd(),
        ];
        for (slot, fd) in descriptors.into_iter().enumerate() {
            self.check(supervisor, b).map_err(error)?;
            root_shape(root_control.as_fd(), root)?;
            remaining(deadline)?;
            let packet = Transfer::new(&self.binding, slot as u8, nonce, b).map_err(error)?;
            b.reserve_storage(Transfer::STORAGE).map_err(error)?;
            b.charge_work(launch_io::packet_receive_work(
                packet.canonical_bytes().len(),
            ))
            .map_err(error)?;
            if launch_io::send_packet_with_descriptor(
                root_control.as_fd(),
                packet.canonical_bytes(),
                fd,
            )
            .map_err(transport_error)?
            .is_none()
            {
                return Err(refuse("native root transfer send unavailable"));
            }
        }
        validate_ack_reader(ack_reader.as_fd(), &self.binding)?;
        let ack_packet = Transfer::root_ack_reader_packet(nonce, binding_id, b).map_err(error)?;
        b.charge_work(launch_io::packet_receive_work(72))
            .map_err(error)?;
        if launch_io::send_packet_with_descriptor(
            root_control.as_fd(),
            &ack_packet,
            ack_reader.as_fd(),
        )
        .map_err(transport_error)?
        .is_none()
        {
            return Err(refuse("native root ACK reader unavailable"));
        }
        drop(ack_reader);
        self.check(supervisor, b).map_err(error)?;
        root_shape(root_control.as_fd(), root)?;
        remaining(deadline)?;
        // Drop all service/application/Cargo/proof aliases before returning the
        // control-only owner. A root may not start proof activation before this
        // transfer's separate publication completion protocol is acknowledged.
        drop(app);
        drop(cargo);
        let Accepted {
            compiler,
            binding,
            proof_peer,
            parent,
            ..
        } = self;
        let Compiler {
            control: application_control,
            service_peer,
            client,
            control_snapshot,
            ..
        } = compiler;
        drop(service_peer);
        drop(client);
        drop(proof_peer);
        drop(parent);
        let retained = size_of::<Pending<'supervisor, 'work>>()
            .checked_add(binding.retained_storage())
            .ok_or_else(|| refuse("native root transfer retained overflow"))?
            .max(input);
        let pending = Pending {
            supervisor,
            application_control,
            root_control,
            binding,
            nonce,
            root,
            control_snapshot,
            root_snapshot,
            deadline,
            ledger: b.work_ledger_identity_v1(),
            storage_account: b.storage_account_identity_v1(),
            retained,
            work: PhantomData,
        };
        pending.revalidate_inner(b)?;
        let complete =
            Transfer::completion_packet(nonce, *pending.binding.identity().as_bytes(), b)
                .map_err(error)?;
        b.charge_work(launch_io::packet_receive_work(complete.len()))
            .map_err(error)?;
        if launch_io::send_packet(pending.root_control.as_fd(), &complete)
            .map_err(transport_error)?
            .is_none()
        {
            return Err(refuse("native root transfer completion unavailable"));
        }
        pending.revalidate_inner(b)?;
        b.release_storage(
            b.storage()
                .checked_sub(floor)
                .ok_or_else(|| refuse("native root transfer accounting"))?,
        )
        .map_err(error)?;
        // Do not refund consumed input reservations here. Keeping a conservative
        // larger input charge preserves failure/unwind and caller scope custody.
        Ok((pending, retained.saturating_sub(input)))
    }
}
impl<'supervisor, 'work> Pending<'supervisor, 'work> {
    /// Consume the original root publication, including its issuer pidfd, and
    /// forward exactly one native ServiceReady to Cargo without EOF. The
    /// control stays owned until the root accepts the original pipe ACK.
    /// This is not proof readiness or startup ACK. Failure consumes the control
    /// owners and cannot retry publication. Keep the entire consumed reservation
    /// until the returned inert readiness record is dropped.
    pub fn publish_currentness_from_root(
        self,
        b: &mut Budget<'work>,
    ) -> io::Result<(
        PublishedNativeApplicationRootTransferV3<'supervisor, 'work>,
        usize,
    )> {
        let floor = b.storage();
        let input = self.retained;
        self.revalidate(b)?;
        b.charge_work(launch_io::packet_receive_work(224) + launch_io::packet_receive_work(104))
            .map_err(error)?;
        b.reserve_storage(launch_io::packet_receive_scratch(224) + 224 + LiveClient::FD_STORAGE)
            .map_err(error)?;
        poll(self.root_control.as_fd(), self.deadline)?;
        self.revalidate(b)?;
        let (packet, pidfd) = launch_io::receive_authenticated_descriptor::<224>(
            self.root_control.as_fd(),
            launch_io::MessageSender::new(self.root.as_raw_pid(), 0, 0),
        )
        .map_err(transport_error)?
        .ok_or_else(|| refuse("native root publication unavailable"))?;
        let (ready, gate, charge) = Transfer::decode_publication(
            &packet,
            self.nonce,
            *self.binding.identity().as_bytes(),
            b,
        )
        .map_err(error)?;
        b.reserve_storage(charge).map_err(error)?;
        let manifest = self.binding.compiler_handoff().launch_manifest();
        if !ready
            .matches_launch(ready.issuer_pid(), manifest, self.supervisor.policy(), b)
            .map_err(error)?
            || [
                self.root.as_raw_pid() as u32,
                rustix::process::getpid().as_raw_pid() as u32,
                manifest.client().pid(),
                self.binding.compiler_handoff().submitter().pid(),
                self.supervisor.external_anchor_process().pid(),
            ]
            .contains(&ready.issuer_pid())
        {
            return Err(refuse(
                "native root publication issuer or manifest mismatch",
            ));
        }
        let credentials = self.supervisor.credentials();
        let expected = ExpectedClientProcessIdentityV1::new(
            ready.issuer_pid(),
            credentials.uid(),
            credentials.gid(),
        )
        .map_err(error)?;
        let (issuer, charge) = LiveClient::admit(pidfd, expected, b).map_err(error)?;
        b.reserve_storage(charge.additional_storage())
            .map_err(error)?;
        self.revalidate(b)?;
        issuer.validate_liveness(b).map_err(error)?;
        checks::control_shape(&self.application_control)
            .map_err(|e| error(HandoffError::from(e)))?;
        if checks::control_peer(&self.application_control)
            .map_err(|e| error(HandoffError::from(e)))?
            != self.binding.compiler_handoff().submitter()
        {
            return Err(refuse("native publication Cargo control peer changed"));
        }
        let bytes = ready.canonical_bytes();
        if rustix::net::send(
            &self.application_control,
            bytes,
            rustix::net::SendFlags::DONTWAIT | rustix::net::SendFlags::NOSIGNAL,
        )? != bytes.len()
        {
            return Err(refuse("native publication partial Cargo readiness"));
        }
        issuer.validate_liveness(b).map_err(error)?;
        self.revalidate(b)?;
        let completion = Transfer::publication_completion(
            self.nonce,
            *self.binding.identity().as_bytes(),
            gate,
            b,
        )
        .map_err(error)?;
        self.revalidate(b)?;
        if launch_io::send_packet(self.root_control.as_fd(), &completion)
            .map_err(transport_error)?
            .is_none()
        {
            return Err(refuse("native publication completion unavailable"));
        }
        issuer.validate_liveness(b).map_err(error)?;
        remaining(self.deadline)?;
        let retained = self
            .retained
            .checked_add(issuer.retained_storage())
            .and_then(|n| {
                n.checked_add(size_of::<
                    PublishedNativeApplicationRootTransferV3<'supervisor, 'work>,
                >())
            })
            .ok_or_else(|| refuse("native publication accounting"))?;
        b.release_storage(
            b.storage()
                .checked_sub(floor)
                .ok_or_else(|| refuse("native publication accounting"))?,
        )
        .map_err(error)?;
        Ok((
            PublishedNativeApplicationRootTransferV3 {
                pending: self,
                issuer,
                gate,
                retained,
            },
            retained - input,
        ))
    }
    /// Exact inert registration retained after descriptor alias transfer.
    pub const fn binding(&self) -> &Binding {
        &self.binding
    }
    /// Full conservative charge, including unreleased consumed input storage.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    /// This pre-ACK control owner never grants application readiness.
    pub const fn grants_readiness(&self) -> bool {
        false
    }
    /// Checks original supervisor/account/root-parent/control continuity within
    /// the same aggregate deadline; does not reopen transferred descriptor roles.
    pub fn revalidate(&self, b: &mut Budget<'work>) -> io::Result<()> {
        b.charge_work(Accepted::WORK).map_err(error)?;
        if b.storage() < self.retained {
            return Err(refuse("native root transfer owner not prepaid"));
        }
        b.reserve_storage(Accepted::SCRATCH).map_err(error)?;
        self.revalidate_inner(b)?;
        b.release_storage(Accepted::SCRATCH).map_err(error)
    }
    fn revalidate_inner(&self, b: &mut Budget<'work>) -> io::Result<()> {
        if self.ledger != b.work_ledger_identity_v1()
            || self.storage_account != b.storage_account_identity_v1()
        {
            return Err(refuse("native root transfer account changed"));
        }
        self.supervisor.revalidate(b).map_err(error)?;
        root_shape(self.root_control.as_fd(), self.root)?;
        if checks::snapshot(&self.application_control).map_err(|e| error(HandoffError::from(e)))?
            != self.control_snapshot
            || checks::snapshot(&self.root_control).map_err(|e| error(HandoffError::from(e)))?
                != self.root_snapshot
        {
            return Err(refuse("native root transfer control changed"));
        }
        remaining(self.deadline)
    }
}

/// Original two-phase Cargo control after currentness service publication. No
/// proof activation or ACK can be inferred until the original root returns the
/// exact accepted native startup record. Dropping this owner closes both controls.
pub struct PublishedNativeApplicationRootTransferV3<'supervisor, 'work> {
    pending: Pending<'supervisor, 'work>,
    issuer: LiveClient,
    gate: [u8; 32],
    retained: usize,
}
impl<'work> PublishedNativeApplicationRootTransferV3<'_, 'work> {
    /// Full conservative retained charge, including consumed pending controls.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    /// Receive original-root accepted ACK, forward once, then close Cargo control
    /// for EOF. The root must have read the actual original ACK pipe plus EOF;
    /// this method only authenticates that original root transport and binding.
    pub fn finish_startup(self, b: &mut Budget<'work>) -> io::Result<()> {
        use fe2o3_runtime_protocol::{
            NativeApplicationStartupKindV1 as Kind, NativeApplicationStartupRecordV1 as Record,
        };
        if b.storage() < self.retained {
            return Err(refuse("native published owner not prepaid"));
        }
        self.pending.revalidate(b)?;
        self.issuer.validate_liveness(b).map_err(error)?;
        b.charge_work(launch_io::packet_receive_work(208))
            .map_err(error)?;
        b.reserve_storage(launch_io::packet_receive_scratch(208) + 208)
            .map_err(error)?;
        poll(self.pending.root_control.as_fd(), self.pending.deadline)?;
        let bytes = launch_io::receive_authenticated_packet::<208>(
            self.pending.root_control.as_fd(),
            launch_io::MessageSender::new(self.pending.root.as_raw_pid(), 0, 0),
        )
        .map_err(transport_error)?
        .ok_or_else(|| refuse("native root accepted ACK unavailable"))?;
        let (record, charge) = Record::decode(&bytes, b).map_err(error)?;
        b.reserve_storage(charge.additional_storage())
            .map_err(error)?;
        if record.kind() != Kind::Acknowledgment
            || record.currentness_gate_identity() != self.gate
            || record.transcript().binding() != *self.pending.binding.identity().as_bytes()
        {
            return Err(refuse("native root accepted ACK changed registration"));
        }
        self.pending.revalidate(b)?;
        self.issuer.validate_liveness(b).map_err(error)?;
        if rustix::net::send(
            &self.pending.application_control,
            &bytes,
            rustix::net::SendFlags::DONTWAIT | rustix::net::SendFlags::NOSIGNAL,
        )? != bytes.len()
        {
            return Err(refuse("native accepted ACK partial Cargo send"));
        }
        remaining(self.pending.deadline)?;
        drop(self);
        Ok(())
    }
}

fn validate_ack_reader(fd: BorrowedFd<'_>, binding: &Binding) -> io::Result<()> {
    let stat = rustix::fs::fstat(fd)?;
    let flags = rustix::fs::fcntl_getfl(fd)?;
    let occurrence =
        fe2o3_runtime_protocol::WorkerV3ApplicationInputOccurrenceV1::from_linux_descriptor_v1(
            3,
            stat.st_dev,
            stat.st_ino,
            stat.st_mode,
        )
        .map_err(error)?;
    if rustix::fs::FileType::from_raw_mode(stat.st_mode) != rustix::fs::FileType::Fifo
        || flags & rustix::fs::OFlags::ACCMODE != rustix::fs::OFlags::RDONLY
        || !flags.contains(rustix::fs::OFlags::NONBLOCK)
        || !rustix::io::fcntl_getfd(fd)?.contains(rustix::io::FdFlags::CLOEXEC)
        || binding.inputs().occurrence().inputs().get(2) != Some(&occurrence)
    {
        return Err(refuse(
            "native ACK reader differs from observed original pipe",
        ));
    }
    Ok(())
}
fn remaining(deadline: Instant) -> io::Result<()> {
    if deadline
        .checked_duration_since(Instant::now())
        .is_none_or(|d| d.is_zero() || d > Duration::from_secs(120))
    {
        return Err(refuse("native root transfer deadline expired"));
    }
    Ok(())
}
fn poll(fd: BorrowedFd<'_>, deadline: Instant) -> io::Result<()> {
    remaining(deadline)?;
    let timeout =
        rustix::event::Timespec::try_from(deadline.saturating_duration_since(Instant::now()))
            .map_err(error)?;
    let mut fds = [rustix::event::PollFd::new(
        &fd,
        rustix::event::PollFlags::IN,
    )];
    if rustix::event::poll(&mut fds, Some(&timeout))? == 0
        || !fds[0].revents().contains(rustix::event::PollFlags::IN)
        || fds[0]
            .revents()
            .intersects(rustix::event::PollFlags::ERR | rustix::event::PollFlags::NVAL)
    {
        return Err(refuse("native root transfer poll unavailable"));
    }
    Ok(())
}
fn root_shape(fd: BorrowedFd<'_>, root: rustix::process::Pid) -> io::Result<()> {
    if rustix::process::getppid() != Some(root)
        || rustix::io::fcntl_getfd(fd)? != rustix::io::FdFlags::CLOEXEC
        || rustix::fs::fcntl_getfl(fd)? != (OFlags::RDWR | OFlags::NONBLOCK)
        || rustix::net::sockopt::socket_domain(fd)? != AddressFamily::UNIX
        || rustix::net::sockopt::socket_type(fd)? != SocketType::SEQPACKET
        || !rustix::net::sockopt::socket_passcred(fd)?
    {
        return Err(refuse("native root transfer channel shape"));
    }
    let peer = rustix::net::sockopt::socket_peercred(fd)?;
    if peer.pid != root || peer.uid.as_raw() != 0 || peer.gid.as_raw() != 0 {
        return Err(refuse("native root transfer original root creator"));
    }
    if !pair_address(rustix::net::getsockname(fd)?)
        || !rustix::net::getpeername(fd)?.is_some_and(pair_address)
    {
        return Err(refuse(
            "native root transfer requires original unnamed pair",
        ));
    }
    Ok(())
}
fn pair_address(address: rustix::net::SocketAddrAny) -> bool {
    if address == SocketAddrUnix::new_unnamed().into() {
        return true;
    }
    let Ok(address) = SocketAddrUnix::try_from(address) else {
        return false;
    };
    address.abstract_name().is_some_and(|name| {
        name.len() == 5
            && name
                .iter()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
    })
}
