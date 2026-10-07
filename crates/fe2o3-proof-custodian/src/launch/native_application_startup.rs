//! Actual running-manager join and original pipe-ACK activation barrier.
use super::*;
use crate::RunningNativeApplicationManagerV1 as Manager;
use fe2o3_broker_authority_service::{
    ReceivedNativeApplicationV3 as Received, RootNativeApplicationSupervisorV3 as Supervisor,
};
use fe2o3_runtime_protocol::{
    NativeApplicationSessionMessageV1 as SessionMessage,
    NativeApplicationStartupRecordV1 as StartupRecord,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StartupPhase {
    Controller,
    Ack,
    Activate,
    Active,
    Failed,
}

/// Native manager custody through actual root Ready, original ACK+EOF and
/// controller activation. This owner is constructed only by a running sealed
/// manager and the consuming authenticated registration. It keeps the original
/// supervisor, manager, issuer and cleanup custody; no decoded session substitutes
/// for any of those owners. Active is not GPU settlement or permission to Drop:
/// after offer, dropping unresolved proof custody is fail-stop as in the launcher.
///
/// ```compile_fail
/// use fe2o3_proof_custodian::RootNativeApplicationStartupV1;
/// fn fake(bytes:&[u8]) {let _=RootNativeApplicationStartupV1::decode(bytes);}
/// ```
pub struct RootNativeApplicationStartupV1<'manager, 'root, 'registry, 'custody, 'work> {
    manager: &'manager Manager<'root, 'work>,
    supervisor: &'registry Supervisor<'custody, 'work>,
    controller: PendingNativeApplication<'root, 'work>,
    proof: Option<OwnedFd>,
    ack: Option<OwnedFd>,
    supervisor_control: Option<OwnedFd>,
    objects: [(u64, u64); 3],
    ready: Option<StartupRecord>,
    accepted: Option<StartupRecord>,
    ack_bytes: [u8; 209],
    ack_used: usize,
    phase: StartupPhase,
    attempts: usize,
    retained: usize,
}

/// Original post-startup application/controller custody. The one-shot supervisor
/// may exit, but the actual manager, issuer, capsule and original process owners
/// remain retained. No Active/ACK/Proved record releases these dependencies.
/// Drop before original application exit and exact CPU-domain retirement remains
/// fail-stop. Retirement does not establish GPU settlement.
pub struct RootNativeApplicationLifetimeV1<'manager, 'root, 'work> {
    manager: &'manager Manager<'root, 'work>,
    controller: PendingNativeApplication<'root, 'work>,
    retained: usize,
}

impl<'root, 'work> Manager<'root, 'work> {
    /// Stage the genuine received registration on this actual running sealed
    /// manager. The original cleanup controller must already be funded; no new
    /// lifecycle/account or caller-selected deployment can enter the join.
    /// Returns conservative growth above the complete consumed Received charge.
    /// Existing manager/deployment/supervisor/issuer reservations stay prepaid.
    ///
    /// # Safety
    /// Retain the actual root cloning thread and original funded cleanup service
    /// through termination and unresolved domain cleanup. Preserve sole consuming
    /// waits, sealed-image/namespace custody and the proof launcher's fail-stop
    /// contract after any possibly delivered offer or activation. Never reset the
    /// original request or cleanup accounts while these owners remain live.
    #[allow(unsafe_code)]
    pub unsafe fn begin_received_registration<'manager, 'registry, 'custody>(
        &'manager self,
        received: Received<'root, 'registry, 'custody, 'work>,
        cleanup: &mut Cleanup,
        b: &mut Budget<'work>,
    ) -> io::Result<(
        RootNativeApplicationStartupV1<'manager, 'root, 'registry, 'custody, 'work>,
        usize,
    )>
    where
        'custody: 'root,
    {
        self.revalidate(b)?;
        let input = received.retained_storage();
        let floor = b.storage();
        let (value, growth) = received.stage(
            b,
            |compiler,
             supervisor,
             binding,
             transcript,
             deadline,
             application,
             cargo,
             proof,
             ack,
             supervisor_control,
             currentness,
             b| {
                self.revalidate(b)?;
                require(
                    std::ptr::eq(compiler, self.deployment().compiler()),
                    "native registration changed actual root deployment owner",
                )?;
                supervisor.revalidate(b).map_err(other)?;
                let (deployment, charge) = Deployment::open(b)?;
                b.reserve_storage(charge.additional_storage())
                    .map_err(other)?;
                require(
                    deployment.deployment().canonical_bytes()
                        == self.deployment().proof().deployment().canonical_bytes(),
                    "native proof deployment changed manager installation",
                )?;
                let objects = [
                    object(proof.as_fd())?,
                    object(ack.as_fd())?,
                    object(supervisor_control.as_fd())?,
                ];
                require(
                    objects[0] != objects[1]
                        && objects[0] != objects[2]
                        && objects[1] != objects[2],
                    "native startup roles alias",
                )?;
                check_ack(ack.as_fd(), &binding)?;
                // The wrapper retains the original root receive endpoint until ACK.
                // Only this exact duplicate enters the controller's closed FD table.
                let child_peer = rustix::io::fcntl_dupfd_cloexec(&proof, 0)?;
                b.reserve_storage(size_of::<OwnedFd>()).map_err(other)?;
                let (controller, charge) = PendingNativeApplication::begin(
                    deployment,
                    compiler,
                    currentness,
                    binding,
                    transcript,
                    application,
                    cargo,
                    child_peer,
                    deadline,
                    cleanup,
                    b,
                )?;
                b.reserve_storage(charge).map_err(other)?;
                let growth = charge
                    .checked_add(size_of::<
                        RootNativeApplicationStartupV1<'manager, 'root, 'registry, 'custody, 'work>,
                    >())
                    .and_then(|n| n.checked_add(8192))
                    .ok_or_else(|| io::Error::other("native startup accounting"))?;
                let retained = input
                    .checked_add(growth)
                    .ok_or_else(|| io::Error::other("native startup accounting"))?;
                let value = RootNativeApplicationStartupV1 {
                    manager: self,
                    supervisor,
                    controller,
                    proof: Some(proof),
                    ack: Some(ack),
                    supervisor_control: Some(supervisor_control),
                    objects,
                    ready: None,
                    accepted: None,
                    ack_bytes: [0; 209],
                    ack_used: 0,
                    phase: StartupPhase::Controller,
                    attempts: 0,
                    retained,
                };
                self.revalidate(b)?;
                Ok((value, growth))
            },
        )?;
        b.release_storage(
            b.storage()
                .checked_sub(floor)
                .ok_or_else(|| io::Error::other("native startup accounting"))?,
        )
        .map_err(other)?;
        Ok((value, growth))
    }
}

impl<'work> RootNativeApplicationStartupV1<'_, '_, '_, '_, 'work> {
    /// Complete conservative reservation, including unreleased consumed inputs.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }

    /// One bounded startup step on the original cumulative account. Returns true
    /// only after actual ACK+EOF, closed root proof alias, controller Activated,
    /// and final accepted ACK forwarding. Refusal is terminal, never retryable.
    pub fn poll_startup(&mut self, b: &mut Budget<'work>) -> io::Result<bool> {
        let result = self.poll_inner(b);
        if result.is_err() {
            self.phase = StartupPhase::Failed;
        }
        result
    }
    fn poll_inner(&mut self, b: &mut Budget<'work>) -> io::Result<bool> {
        require(
            self.phase != StartupPhase::Failed,
            "native startup already refused",
        )?;
        if self.phase == StartupPhase::Active {
            self.revalidate(b)?;
            return Ok(true);
        }
        require(
            self.attempts < launch_io::MAX_PHASE_ATTEMPTS,
            "native startup attempt limit",
        )?;
        self.attempts += 1;
        require(
            Instant::now() < self.controller.deadline,
            "native startup deadline",
        )?;
        self.revalidate(b)?;
        match self.phase {
            StartupPhase::Controller => {
                if !self.controller.poll_ready(b)? {
                    return Ok(false);
                }
                let (pidfd, charge) = self.controller.offer_ready(b)?;
                b.reserve_storage(charge).map_err(other)?;
                self.revalidate(b)?;
                let (message, charge) =
                    SessionMessage::custodian_ready(self.controller.session(), b).map_err(other)?;
                b.reserve_storage(charge.additional_storage())
                    .map_err(other)?;
                let peer = self
                    .proof
                    .as_ref()
                    .ok_or_else(|| io::Error::other("native startup peer absent"))?;
                b.charge_work(launch_io::packet_receive_work(
                    message.canonical_bytes().len(),
                ))
                .map_err(other)?;
                let packet: &[u8; 144 + fe2o3_runtime_protocol::NATIVE_APPLICATION_PROOF_SESSION_BYTES_V1] =
                    message.canonical_bytes().try_into()
                        .map_err(|_| io::Error::other("native CustodianReady framing length"))?;
                require(
                    launch_io::send_packet_with_descriptor(peer.as_fd(), packet, pidfd.as_fd())
                        .map_err(transport_error)?
                        .is_some(),
                    "native CustodianReady send unavailable",
                )?;
                drop(pidfd);
                self.revalidate(b)?;
                let (ready, charge) = StartupRecord::ready(
                    self.controller.session(),
                    self.controller.currentness.gate_identity(),
                    b,
                )
                .map_err(other)?;
                b.reserve_storage(charge.additional_storage())
                    .map_err(other)?;
                b.charge_work(launch_io::packet_receive_work(208))
                    .map_err(other)?;
                require(
                    launch_io::send_packet(
                        self.proof.as_ref().unwrap().as_fd(),
                        ready.canonical_bytes(),
                    )
                    .map_err(transport_error)?
                    .is_some(),
                    "native CurrentnessReady send unavailable",
                )?;
                self.ready = Some(ready);
                self.phase = StartupPhase::Ack;
            }
            StartupPhase::Ack => {
                let fd = self
                    .ack
                    .as_ref()
                    .ok_or_else(|| io::Error::other("native ACK reader absent"))?;
                if read_ack_once(fd, &mut self.ack_bytes, &mut self.ack_used)? {
                    let (ack, charge) =
                        StartupRecord::decode(&self.ack_bytes[..208], b).map_err(other)?;
                    b.reserve_storage(charge.additional_storage())
                        .map_err(other)?;
                    ack.check_acknowledgment(
                        self.ready
                            .as_ref()
                            .ok_or_else(|| io::Error::other("native Ready absent before ACK"))?,
                        b,
                    )
                    .map_err(other)?;
                    self.revalidate(b)?;
                    self.accepted = Some(ack);
                    // No root receive alias may survive into irreversible Activate.
                    drop(self.ack.take());
                    drop(self.proof.take());
                    self.phase = StartupPhase::Activate;
                }
            }
            StartupPhase::Activate => {
                require(
                    self.proof.is_none() && self.ack.is_none() && self.accepted.is_some(),
                    "native Activate before actual ACK closure",
                )?;
                if !self.controller.poll_activate(b)? {
                    return Ok(false);
                }
                self.revalidate(b)?;
                let packet = self.accepted.as_ref().unwrap().canonical_bytes();
                b.charge_work(launch_io::packet_receive_work(packet.len()))
                    .map_err(other)?;
                require(
                    launch_io::send_packet(
                        self.supervisor_control
                            .as_ref()
                            .ok_or_else(|| io::Error::other("native startup control absent"))?
                            .as_fd(),
                        packet,
                    )
                    .map_err(transport_error)?
                    .is_some(),
                    "native accepted ACK forwarding unavailable",
                )?;
                drop(self.supervisor_control.take());
                self.phase = StartupPhase::Active;
                return Ok(true);
            }
            StartupPhase::Active | StartupPhase::Failed => unreachable!(),
        }
        Ok(false)
    }
    fn revalidate(&self, b: &mut Budget<'work>) -> io::Result<()> {
        b.charge_work(WORK).map_err(other)?;
        require(
            b.storage() >= self.retained,
            "native startup owner not prepaid",
        )?;
        self.manager.revalidate(b)?;
        self.supervisor.revalidate(b).map_err(other)?;
        self.controller.revalidate(b)?;
        for (slot, fd) in [&self.proof, &self.ack, &self.supervisor_control]
            .into_iter()
            .enumerate()
        {
            if let Some(fd) = fd {
                require(
                    object(fd.as_fd())? == self.objects[slot],
                    "native startup object changed",
                )?;
            }
        }
        if let Some(fd) = &self.ack {
            check_ack(fd.as_fd(), &self.controller.capsule.binding)?;
        }
        Ok(())
    }
}

impl<'manager, 'root, 'work> RootNativeApplicationStartupV1<'manager, 'root, '_, '_, 'work> {
    /// Consumes actual completed startup, not caller-provided readiness bytes.
    /// Keep the complete original reservation; no new work/account is created.
    pub fn into_lifetime(
        self,
    ) -> io::Result<RootNativeApplicationLifetimeV1<'manager, 'root, 'work>> {
        require(
            self.phase == StartupPhase::Active
                && self.proof.is_none()
                && self.ack.is_none()
                && self.supervisor_control.is_none()
                && self.accepted.is_some(),
            "native lifetime before completed original startup",
        )?;
        Ok(RootNativeApplicationLifetimeV1 {
            manager: self.manager,
            controller: self.controller,
            retained: self.retained,
        })
    }
}

impl<'work> RootNativeApplicationLifetimeV1<'_, '_, 'work> {
    /// Unchanged conservative reservation, including consumed startup records.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }

    /// One finite same-account observation. True means original application exit
    /// and exact CPU proof-domain retirement, never GPU settlement. Pump the
    /// original cleanup service between calls. While the application remains
    /// live, actual controller retention probes and their original deadlines
    /// continue. No error counts as terminal; all unresolved dependencies remain.
    pub fn poll(&mut self, b: &mut Budget<'work>) -> io::Result<bool> {
        require(b.storage() >= self.retained, "native lifetime not prepaid")?;
        self.manager.revalidate(b)?;
        if self.controller.poll_terminal_cleanup(b)? {
            return Ok(true);
        }
        if !self.controller.application_terminal {
            if let Err(error) = self.controller.poll_probe(b) {
                // Liveness may change after the first original-pidfd poll. An
                // independent exact terminal observation, never the error, may
                // start CPU-domain cleanup. Retain the original refusal otherwise.
                let cleanup = self.controller.poll_terminal_cleanup(b);
                return after_probe_failure(error, cleanup, self.controller.application_terminal);
            }
        }
        Ok(false)
    }
}

fn after_probe_failure(
    error: io::Error,
    cleanup: io::Result<bool>,
    application_terminal: bool,
) -> io::Result<bool> {
    match cleanup {
        Ok(retired) if application_terminal => Ok(retired),
        _ => Err(error),
    }
}

fn object(fd: std::os::fd::BorrowedFd<'_>) -> io::Result<(u64, u64)> {
    let stat = rustix::fs::fstat(fd)?;
    Ok((stat.st_dev, stat.st_ino))
}
fn read_ack_once(fd: &OwnedFd, bytes: &mut [u8; 209], used: &mut usize) -> io::Result<bool> {
    require(*used <= 208, "native ACK trailing bytes")?;
    match rustix::io::read(fd, &mut bytes[*used..]) {
        Err(rustix::io::Errno::AGAIN) => Ok(false),
        Err(error) => Err(error.into()),
        Ok(0) => {
            require(*used == 208, "native ACK closed before exact frame")?;
            Ok(true)
        }
        Ok(count) => {
            *used = used
                .checked_add(count)
                .ok_or_else(|| io::Error::other("native ACK accounting"))?;
            require(*used <= 208, "native ACK trailing bytes")?;
            Ok(false)
        }
    }
}
fn check_ack(fd: std::os::fd::BorrowedFd<'_>, binding: &Binding) -> io::Result<()> {
    let stat = rustix::fs::fstat(fd)?;
    let flags = rustix::fs::fcntl_getfl(fd)?;
    let occurrence =
        fe2o3_runtime_protocol::WorkerV3ApplicationInputOccurrenceV1::from_linux_descriptor_v1(
            3,
            stat.st_dev,
            stat.st_ino,
            stat.st_mode,
        )
        .map_err(other)?;
    require(
        rustix::fs::FileType::from_raw_mode(stat.st_mode) == rustix::fs::FileType::Fifo
            && flags & rustix::fs::OFlags::ACCMODE == rustix::fs::OFlags::RDONLY
            && flags.contains(rustix::fs::OFlags::NONBLOCK)
            && rustix::io::fcntl_getfd(fd)?.contains(rustix::io::FdFlags::CLOEXEC)
            && binding.inputs().occurrence().inputs().get(2) == Some(&occurrence),
        "native startup reader differs from original ACK pipe",
    )
}
fn transport_error(error: launch_io::Failure) -> io::Error {
    match error {
        launch_io::Failure::Io { source, .. } => source.into(),
        launch_io::Failure::Timeout(_) => {
            io::Error::new(io::ErrorKind::TimedOut, "native startup transport timeout")
        }
        launch_io::Failure::ChildExited(_) => {
            io::Error::new(io::ErrorKind::BrokenPipe, "native startup child exited")
        }
        launch_io::Failure::InvalidTimeout => {
            io::Error::new(io::ErrorKind::InvalidInput, "native startup timeout")
        }
        _ => io::Error::new(
            io::ErrorKind::InvalidData,
            "native startup transport framing",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn probe_error_never_substitutes_for_original_terminal_observation() {
        for terminal in [false, true] {
            for retired in [false, true] {
                let result = after_probe_failure(
                    io::Error::from_raw_os_error(libc::EIO),
                    Ok(retired),
                    terminal,
                );
                if terminal {
                    assert_eq!(result.unwrap(), retired);
                } else {
                    assert_eq!(result.unwrap_err().raw_os_error(), Some(libc::EIO));
                }
            }
            let result = after_probe_failure(
                io::Error::from_raw_os_error(libc::EIO),
                Err(io::Error::from_raw_os_error(libc::EINTR)),
                terminal,
            );
            assert_eq!(result.unwrap_err().raw_os_error(), Some(libc::EIO));
        }
    }
    #[test]
    fn actual_pipe_requires_exact_frame_then_writer_eof() {
        crate::eof_test_process::isolated(
            module_path!(),
            "actual_pipe_requires_exact_frame_then_writer_eof",
            || {
                let (reader, writer) = rustix::pipe::pipe_with(
                    rustix::pipe::PipeFlags::CLOEXEC | rustix::pipe::PipeFlags::NONBLOCK,
                )
                .unwrap();
                let mut bytes = [0; 209];
                let mut used = 0;
                assert!(!read_ack_once(&reader, &mut bytes, &mut used).unwrap());
                assert_eq!(rustix::io::write(&writer, &[7; 208]).unwrap(), 208);
                assert!(!read_ack_once(&reader, &mut bytes, &mut used).unwrap());
                assert_eq!(used, 208);
                assert!(
                    !read_ack_once(&reader, &mut bytes, &mut used).unwrap(),
                    "live writer is not EOF"
                );
                drop(writer);
                assert!(read_ack_once(&reader, &mut bytes, &mut used).unwrap());
                assert_eq!(&bytes[..208], &[7; 208]);
            },
        );
    }
    #[test]
    fn actual_pipe_rejects_partial_eof_and_trailing_bytes() {
        crate::eof_test_process::isolated(
            module_path!(),
            "actual_pipe_rejects_partial_eof_and_trailing_bytes",
            || {
                for length in [0, 1, 207, 209] {
                    let (reader, writer) = rustix::pipe::pipe_with(
                        rustix::pipe::PipeFlags::CLOEXEC | rustix::pipe::PipeFlags::NONBLOCK,
                    )
                    .unwrap();
                    if length > 0 {
                        assert_eq!(
                            rustix::io::write(&writer, &[7; 209][..length]).unwrap(),
                            length
                        );
                    }
                    drop(writer);
                    let mut bytes = [0; 209];
                    let mut used = 0;
                    let first = read_ack_once(&reader, &mut bytes, &mut used);
                    if length == 0 || length == 209 {
                        assert!(first.is_err());
                    } else {
                        assert!(!first.unwrap());
                        assert!(read_ack_once(&reader, &mut bytes, &mut used).is_err());
                    }
                }
            },
        );
    }
}
