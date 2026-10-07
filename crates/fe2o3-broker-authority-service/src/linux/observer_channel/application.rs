//! Application registration lifetime, independent of compiler issuer slots and GPU custody.

use super::*;
use crate::RetainedWorkerV3ApplicationObservationV1;
use fe2o3_runtime_protocol::{
    WorkerV3ApplicationRegistrationBindingV1, WorkerV3ApplicationSessionKindV1 as Kind,
    WorkerV3ApplicationSessionMessageV1 as Message,
    WorkerV3ApplicationSessionTranscriptV1 as Transcript,
};

const GATE_BYTES: usize = 72;
const GATE_MAGIC: &[u8; 8] = b"F3AOBS1\0";
const PUBLICATION_MAGIC: &[u8; 8] = b"F3APUB1\0";
pub(super) const MAX_APPLICATIONS: usize = 16;
const STARTUP_TIMEOUT: Duration = Duration::from_secs(120);

#[cfg(test)]
pub(super) mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ApplicationRoute {
    ObservationOnly,
    ProofCustodian,
}

enum State {
    AwaitHello,
    ChallengePending(Message),
    AwaitAccept(Transcript),
    GatePending(Transcript),
    AwaitPublication {
        transcript: Transcript,
        bytes: [u8; GATE_BYTES + 1],
        used: usize,
    },
    ReadyPending(Message),
    CustodianHandoffReady(Transcript),
    CustodianReadyOffered,
    Registered,
    Retired,
}

/// Private root owner: never holds a compiler peer, proof, or native invocation authority.
pub(super) struct ApplicationSession {
    pub(super) id: [u8; 32],
    pub(super) binding: WorkerV3ApplicationRegistrationBindingV1,
    application: LiveClientPidfdIdentityV1,
    parent: LiveClientPidfdIdentityV1,
    endpoint: Endpoint,
    writer: Option<OwnedFd>,
    publication: Option<OwnedFd>,
    observation: Option<RetainedWorkerV3ApplicationObservationV1>,
    state: State,
    deadline: Instant,
    containing: bool,
    issuer_bound: bool,
    route: ApplicationRoute,
}

impl ApplicationSession {
    pub(super) fn install(
        application: LiveClientPidfdIdentityV1,
        parent: LiveClientPidfdIdentityV1,
        peer: OwnedFd,
        binding: WorkerV3ApplicationRegistrationBindingV1,
        id: [u8; 32],
        route: ApplicationRoute,
    ) -> Result<(Self, [OwnedFd; 2])> {
        application.validate_parent(&parent)?;
        let endpoint = Endpoint::admit(peer)?;
        if endpoint.creator != parent.expected_client.credentials() || id == [0; 32] {
            return Err(invalid(
                "application proof creator or session identity differs",
            ));
        }
        let (reader, writer) = rustix::pipe::pipe_with(
            rustix::pipe::PipeFlags::CLOEXEC | rustix::pipe::PipeFlags::NONBLOCK,
        )?;
        let (publication, publication_writer) = rustix::pipe::pipe_with(
            rustix::pipe::PipeFlags::CLOEXEC | rustix::pipe::PipeFlags::NONBLOCK,
        )?;
        Ok((
            Self {
                id,
                binding,
                application,
                parent,
                endpoint,
                writer: Some(writer),
                publication: Some(publication),
                observation: None,
                state: State::AwaitHello,
                deadline: Instant::now() + STARTUP_TIMEOUT,
                containing: false,
                issuer_bound: false,
                route,
            },
            [reader, publication_writer],
        ))
    }

    pub(super) fn process_identity(&self) -> (u32, u64) {
        (
            self.application.expected_client.pid,
            self.application.start_time_ticks,
        )
    }

    pub(super) fn installed(&self) -> bool {
        !self.containing
            && !matches!(self.state, State::Retired)
            && (matches!(self.state, State::Registered) || Instant::now() < self.deadline)
    }

    pub(super) fn revalidate_installation(&self) -> Result<()> {
        if !self.installed() {
            return Err(invalid("application registration retired"));
        }
        self.application.validate_parent(&self.parent)?;
        self.endpoint.revalidate()?;
        Ok(())
    }

    pub(super) fn issuer_bound(&mut self) {
        self.issuer_bound = true;
    }

    // At most one expensive first observation is allowed per registry iteration.
    pub(super) fn step(
        &mut self,
        root: &LiveClientPidfdIdentityV1,
        observation_budget: &mut bool,
    ) -> Result<bool> {
        if matches!(self.state, State::Retired) {
            return Ok(false);
        }
        if pidfd_exited(&self.application.pidfd)? {
            self.retire_observation_only();
            return Ok(true);
        }
        if self.containing {
            return self.contain();
        }
        let result = self.advance(root, observation_budget);
        if result.is_err() {
            self.cancel();
        }
        result
    }

    pub(super) fn retired(&self) -> bool {
        matches!(self.state, State::Retired | State::CustodianReadyOffered)
    }

    fn advance(
        &mut self,
        root: &LiveClientPidfdIdentityV1,
        observation_budget: &mut bool,
    ) -> Result<bool> {
        if !matches!(self.state, State::Registered) {
            require_deadline(self.deadline)?;
        }
        if matches!(
            self.state,
            State::AwaitHello | State::AwaitAccept(_) | State::Registered
        ) {
            if !readable(&self.endpoint.peer)? {
                return Ok(false);
            }
            if matches!(self.state, State::AwaitHello) && !*observation_budget {
                return Ok(false);
            }
            let received = match self.endpoint.receive_bytes(&self.application) {
                Err(CompilerExecutionObserverErrorV1::Closed)
                    if matches!(self.state, State::Registered) =>
                {
                    // This owner has no proof or GPU custody. EOF only revokes registration;
                    // it does not establish settlement and must not kill an unloading app.
                    self.retire_observation_only();
                    return Ok(true);
                }
                result => result?,
            };
            let Some((bytes, rights)) = received else {
                return Ok(false);
            };
            let message = Message::decode(&bytes)
                .map_err(|_| invalid("invalid application session packet"))?;
            if !rights.is_empty() {
                return Err(invalid("application cannot send descriptors"));
            }
            match &self.state {
                State::AwaitHello => {
                    if message.kind() != Kind::Hello
                        || !message
                            .inputs()
                            .is_some_and(|value| value.matches_binding(&self.binding))
                    {
                        return Err(invalid(
                            "application Hello inputs differ from original registration",
                        ));
                    }
                    *observation_budget = false;
                    let observation =
                        RetainedWorkerV3ApplicationObservationV1::observe_registered_pre_ack(
                            self.application.try_clone()?,
                            self.parent.try_clone()?,
                            &self.binding,
                            self.endpoint.peer.as_fd(),
                        )
                        .map_err(|error| {
                            CompilerExecutionObserverErrorV1::ApplicationObservation(Box::new(
                                error,
                            ))
                        })?;
                    self.observation = Some(observation);
                    self.state = State::ChallengePending(
                        Message::challenge(self.binding.clone(), message.app_nonce(), self.id)
                            .map_err(|_| invalid("invalid application challenge transcript"))?,
                    );
                }
                State::AwaitAccept(transcript) => {
                    if message.kind() != Kind::Accept || message.transcript() != Some(*transcript) {
                        return Err(invalid("application Accept transcript mismatch or replay"));
                    }
                    self.revalidate_observation()?;
                    self.state = State::GatePending(*transcript);
                }
                _ => return Err(invalid("unexpected packet after application registration")),
            }
            return Ok(true);
        }
        root.validate_liveness()?;
        self.application.validate_liveness()?;
        match &self.state {
            State::ChallengePending(message) => {
                if !writable(&self.endpoint.peer)? {
                    return Ok(false);
                }
                self.revalidate_observation()?;
                if self
                    .endpoint
                    .send_bytes(message.canonical_bytes(), &[root.pidfd.as_fd()])?
                {
                    self.state =
                        State::AwaitAccept(message.transcript().expect("challenge transcript"));
                    return Ok(true);
                }
            }
            State::GatePending(transcript) => {
                // Do not let a fast application unload its endpoint before issuer binding.
                if !self.issuer_bound {
                    return Ok(false);
                }
                self.revalidate_observation()?;
                let bytes = gate_record(self.binding.identity().as_bytes(), &self.id);
                match rustix::io::write(self.writer.as_ref().expect("pending gate writer"), &bytes)
                {
                    Ok(GATE_BYTES) => {
                        self.writer = None;
                        self.state = State::AwaitPublication {
                            transcript: *transcript,
                            bytes: [0; GATE_BYTES + 1],
                            used: 0,
                        };
                        return Ok(true);
                    }
                    Ok(_) => return Err(invalid("partial application gate write")),
                    Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {}
                    Err(error) => return Err(error.into()),
                }
            }
            State::AwaitPublication { .. } => {
                self.revalidate_observation()?;
                let State::AwaitPublication {
                    transcript,
                    bytes,
                    used,
                } = &mut self.state
                else {
                    unreachable!()
                };
                match rustix::io::read(
                    self.publication
                        .as_ref()
                        .expect("pending publication reader"),
                    &mut bytes[*used..],
                ) {
                    Ok(0) => {
                        if *used != GATE_BYTES
                            || bytes[..GATE_BYTES]
                                != publication_record(self.binding.identity().as_bytes(), &self.id)
                        {
                            return Err(invalid(
                                "incomplete or mismatched application publication",
                            ));
                        }
                        self.publication = None;
                        self.state = match self.route {
                            ApplicationRoute::ObservationOnly => {
                                State::ReadyPending(Message::ready(*transcript))
                            }
                            ApplicationRoute::ProofCustodian => {
                                State::CustodianHandoffReady(*transcript)
                            }
                        };
                        return Ok(true);
                    }
                    Ok(count) => {
                        *used += count;
                        if *used > GATE_BYTES {
                            return Err(invalid("trailing application publication data"));
                        }
                        return Ok(true);
                    }
                    Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {}
                    Err(error) => return Err(error.into()),
                }
            }
            State::ReadyPending(message) => {
                if !writable(&self.endpoint.peer)? {
                    return Ok(false);
                }
                self.revalidate_observation()?;
                if self.endpoint.send_bytes(message.canonical_bytes(), &[])? {
                    // The gate is already committed/closed before the app may ACK and unload.
                    self.state = State::Registered;
                    return Ok(true);
                }
            }
            State::CustodianHandoffReady(_) => {
                // No receive or Ready here. Only the extracted custodian owner may continue.
                self.revalidate_observation()?;
            }
            _ => unreachable!("receive and retired states handled before send"),
        }
        Ok(false)
    }

    fn revalidate_observation(&self) -> Result<()> {
        self.observation
            .as_ref()
            .ok_or_else(|| invalid("application observation missing"))?
            .revalidate_registered_counterpart(self.endpoint.peer.as_fd())
            .map_err(|error| {
                CompilerExecutionObserverErrorV1::ApplicationObservation(Box::new(error))
            })
    }

    pub(super) fn custodian_handoff_ready(&self) -> bool {
        !self.containing
            && self.route == ApplicationRoute::ProofCustodian
            && self.issuer_bound
            && self.writer.is_none()
            && self.publication.is_none()
            && matches!(self.state, State::CustodianHandoffReady(_))
    }

    pub(super) fn cancel(&mut self) {
        if self.retired() {
            return;
        }
        self.containing = true;
        self.writer = None;
        self.publication = None;
        self.endpoint.close();
        let _ = self.contain();
    }

    fn contain(&mut self) -> Result<bool> {
        if !pidfd_exited(&self.application.pidfd)? {
            match rustix::process::pidfd_send_signal(
                &self.application.pidfd,
                rustix::process::Signal::KILL,
            ) {
                Ok(()) | Err(rustix::io::Errno::SRCH) => {}
                Err(error) => return Err(error.into()),
            }
        }
        if pidfd_exited(&self.application.pidfd)? {
            self.retire_observation_only();
            return Ok(true);
        }
        Ok(false)
    }

    fn retire_observation_only(&mut self) {
        self.writer = None;
        self.publication = None;
        self.observation = None;
        self.containing = false;
        self.state = State::Retired;
    }
}

/// Original authenticated application registration, after exact publication and before Ready.
///
/// Only the root registry can produce this owner. It retains the original observation,
/// process pidfds and exact proof counterpart, not a reconstruction from descriptive bytes.
/// Extraction separates it from compiler issuer cleanup. Dropping it before the future
/// custodian handoff contains the exact application; it is not proof or GPU authority.
/// No Ready, activation, raw descriptor export, or settlement API is provided here.
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::PublishedApplicationCustodianHandoffV1;
/// fn cloneable<T: Clone>() {}
/// cloneable::<PublishedApplicationCustodianHandoffV1>();
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::PublishedApplicationCustodianHandoffV1;
/// use std::os::fd::AsFd;
/// fn export(value: PublishedApplicationCustodianHandoffV1) { let _ = value.as_fd(); }
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::PublishedApplicationCustodianHandoffV1;
/// let _ = PublishedApplicationCustodianHandoffV1 {};
/// ```
pub struct PublishedApplicationCustodianHandoffV1 {
    session: ApplicationSession,
}

impl PublishedApplicationCustodianHandoffV1 {
    pub(super) fn process_rights(&self) -> [BorrowedFd<'_>; 2] {
        [
            self.session.application.pidfd.as_fd(),
            self.session.parent.pidfd.as_fd(),
        ]
    }

    pub(super) fn proof_peer(&self) -> BorrowedFd<'_> {
        self.session.endpoint.peer.as_fd()
    }

    // Only the authenticated manager client may publish this one-way transition.
    pub(super) fn publish_ready(
        &mut self,
        session: &fe2o3_runtime_protocol::WorkerV3ApplicationProofSessionV1,
        controller: &LiveClientPidfdIdentityV1,
    ) -> Result<bool> {
        self.revalidate()?;
        controller.validate_liveness()?;
        let identity = controller.expected_client;
        if session.transcript() != self.transcript()
            || session.controller() != (identity.pid, identity.uid, identity.gid)
        {
            return Err(invalid(
                "custodian Ready does not match original registration",
            ));
        }
        let transcript = self.transcript();
        let message = Message::custodian_ready(session.clone());
        require_deadline(self.session.deadline)?;
        // Any error is conservatively possibly delivered. Drop must plain-close, not
        // shutdown the shared peer or kill the application after this point.
        self.session.state = State::CustodianReadyOffered;
        if !self
            .session
            .endpoint
            .send_bytes(message.canonical_bytes(), &[controller.pidfd.as_fd()])?
        {
            self.session.state = State::CustodianHandoffReady(transcript);
            return Ok(false);
        }
        Ok(true)
    }

    pub(super) fn process_identity(&self) -> (u32, u64) {
        self.session.process_identity()
    }

    pub(super) fn take_next(sessions: &mut Vec<ApplicationSession>) -> Option<Self> {
        let index = sessions
            .iter()
            .position(ApplicationSession::custodian_handoff_ready)?;
        Some(Self {
            session: sessions.remove(index),
        })
    }

    pub fn binding(&self) -> &WorkerV3ApplicationRegistrationBindingV1 {
        &self.session.binding
    }

    pub fn transcript(&self) -> Transcript {
        let State::CustodianHandoffReady(transcript) = self.session.state else {
            unreachable!("only published custodian sessions can be extracted")
        };
        transcript
    }

    /// Original absolute registration deadline. Extraction never grants a new startup window.
    pub fn startup_deadline(&self) -> Instant {
        self.session.deadline
    }

    /// Rechecks the original occurrence and counterpart, without reading the proof channel.
    /// The historical ACK descriptor is deliberately not reopened after first observation.
    pub fn revalidate(&self) -> Result<()> {
        if !self.session.custodian_handoff_ready() {
            return Err(invalid("custodian application handoff is not pending"));
        }
        require_deadline(self.session.deadline)?;
        self.session.revalidate_installation()?;
        self.session.revalidate_observation()?;
        require_deadline(self.session.deadline)
    }
}

impl Drop for ApplicationSession {
    fn drop(&mut self) {
        if self.retired() {
            return;
        }
        self.cancel();
        while !self.retired() {
            let _ = self.contain();
            if !self.retired() {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
}

fn gate_record(binding: &[u8; 32], session: &[u8; 32]) -> [u8; GATE_BYTES] {
    let mut bytes = [0; GATE_BYTES];
    bytes[..8].copy_from_slice(GATE_MAGIC);
    bytes[8..40].copy_from_slice(binding);
    bytes[40..72].copy_from_slice(session);
    bytes
}

fn publication_record(binding: &[u8; 32], session: &[u8; 32]) -> [u8; GATE_BYTES] {
    let mut bytes = gate_record(binding, session);
    bytes[..8].copy_from_slice(PUBLICATION_MAGIC);
    bytes
}

/// Private reverse gate: only the registered supervisor owns this writer, never the issuer.
pub(super) struct ApplicationPublication {
    writer: OwnedFd,
    root: LiveClientPidfdIdentityV1,
    object: ObjectIdentityV1,
    expected: [u8; GATE_BYTES],
}

impl ApplicationPublication {
    pub(super) fn admit(
        writer: OwnedFd,
        root: LiveClientPidfdIdentityV1,
        binding: [u8; 32],
        session: [u8; 32],
        observation: ObjectIdentityV1,
    ) -> Result<Self> {
        let object = ObjectIdentityV1::inspect(
            &writer,
            AdmissionErrorKindV1::InspectPeer,
            "application publication",
        )?;
        if object == observation || binding == [0; 32] || session == [0; 32] {
            return Err(invalid("aliased or unbound application publication gate"));
        }
        let value = Self {
            writer,
            root,
            object,
            expected: publication_record(&binding, &session),
        };
        value.revalidate()?;
        Ok(value)
    }

    pub(super) fn revalidate(&self) -> Result<()> {
        self.root.validate_liveness()?;
        let flags = rustix::fs::fcntl_getfl(&self.writer)?;
        if rustix::io::fcntl_getfd(&self.writer)? != rustix::io::FdFlags::CLOEXEC
            || flags & OFlags::ACCMODE != OFlags::WRONLY
            || !flags.contains(OFlags::NONBLOCK)
            || flags.intersects(OFlags::PATH | OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT)
            || self.object.mode & libc::S_IFMT != libc::S_IFIFO
            || self.object.uid != self.root.expected_client.uid
            || ObjectIdentityV1::inspect(
                &self.writer,
                AdmissionErrorKindV1::InspectPeer,
                "application publication",
            )? != self.object
        {
            return Err(invalid("root application publication gate changed"));
        }
        Ok(())
    }

    pub(super) fn publish(self, deadline: Instant) -> Result<()> {
        loop {
            require_deadline(deadline)?;
            self.revalidate()?;
            require_deadline(deadline)?;
            match rustix::io::write(&self.writer, &self.expected) {
                // Closing this writer commits release. No fallible checks may follow the write:
                // after EOF the app may legitimately finish and close its issuer connection.
                Ok(GATE_BYTES) => return Ok(()),
                Ok(_) => return Err(invalid("partial application publication write")),
                Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
}

fn readable(fd: &OwnedFd) -> Result<bool> {
    ready(fd, libc::POLLIN)
}
fn writable(fd: &OwnedFd) -> Result<bool> {
    ready(fd, libc::POLLOUT)
}

fn ready(fd: &OwnedFd, events: i16) -> Result<bool> {
    let mut event = libc::pollfd {
        fd: fd.as_raw_fd(),
        events,
        revents: 0,
    };
    // SAFETY: one initialized pollfd remains borrowed for this nonblocking observation.
    if unsafe { libc::poll(&mut event, 1, 0) } < 0 {
        let error = io::Error::last_os_error();
        return if error.kind() == io::ErrorKind::Interrupted {
            Ok(false)
        } else {
            Err(error.into())
        };
    }
    if event.revents & libc::POLLNVAL != 0 {
        return Err(invalid("invalid application transport descriptor"));
    }
    Ok(event.revents & (events | libc::POLLERR | libc::POLLHUP) != 0)
}

/// Original root-created observation gate, separate from Cargo ACK and issuer readiness.
/// It can only be obtained through authenticated registry attachment and grants no proof authority.
///
/// ```compile_fail
/// fn cloneable<T: Clone>() {}
/// cloneable::<fe2o3_broker_authority_service::PendingApplicationObservationGateV1>();
/// ```
pub struct PendingApplicationObservationGateV1 {
    reader: OwnedFd,
    root: LiveClientPidfdIdentityV1,
    pub(super) object: ObjectIdentityV1,
    expected: [u8; GATE_BYTES],
}

impl PendingApplicationObservationGateV1 {
    pub(super) fn admit(
        reader: OwnedFd,
        root: LiveClientPidfdIdentityV1,
        binding: [u8; 32],
        session: [u8; 32],
    ) -> Result<Self> {
        let value = Self {
            object: ObjectIdentityV1::inspect(
                &reader,
                AdmissionErrorKindV1::InspectPeer,
                "application gate",
            )?,
            reader,
            root,
            expected: gate_record(&binding, &session),
        };
        if binding == [0; 32] || session == [0; 32] {
            return Err(invalid("empty application gate binding"));
        }
        value.revalidate()?;
        Ok(value)
    }

    pub(super) fn revalidate(&self) -> Result<()> {
        self.root.validate_liveness()?;
        let flags = rustix::fs::fcntl_getfl(&self.reader)?;
        if rustix::io::fcntl_getfd(&self.reader)? != rustix::io::FdFlags::CLOEXEC
            || flags & OFlags::ACCMODE != OFlags::RDONLY
            || !flags.contains(OFlags::NONBLOCK)
            || flags.intersects(OFlags::PATH | OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT)
            || self.object.mode & libc::S_IFMT != libc::S_IFIFO
            || self.object.uid != self.root.expected_client.uid
            || ObjectIdentityV1::inspect(
                &self.reader,
                AdmissionErrorKindV1::InspectPeer,
                "application gate",
            )? != self.object
        {
            return Err(invalid("root application observation gate changed"));
        }
        Ok(())
    }

    /// Waits outside the registry mutex for the exact observation record and terminal EOF.
    /// Ordinary compiler readiness is insufficient to complete this operation.
    pub fn await_observation(self, deadline: Instant) -> Result<()> {
        let mut bytes = [0; GATE_BYTES + 1];
        let mut count = 0;
        loop {
            require_deadline(deadline)?;
            self.revalidate()?;
            match rustix::io::read(&self.reader, &mut bytes[count..]) {
                Ok(0) => {
                    if count != GATE_BYTES || bytes[..GATE_BYTES] != self.expected {
                        return Err(invalid(
                            "incomplete or mismatched application observation gate",
                        ));
                    }
                    self.revalidate()?;
                    require_deadline(deadline)?;
                    return Ok(());
                }
                Ok(length) => {
                    count += length;
                    if count > GATE_BYTES {
                        return Err(invalid("trailing application gate data"));
                    }
                }
                Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                    let mut events = [
                        libc::pollfd {
                            fd: self.reader.as_raw_fd(),
                            events: libc::POLLIN,
                            revents: 0,
                        },
                        libc::pollfd {
                            fd: self.root.pidfd.as_raw_fd(),
                            events: libc::POLLIN,
                            revents: 0,
                        },
                    ];
                    let remaining = deadline
                        .saturating_duration_since(Instant::now())
                        .as_millis()
                        .clamp(1, 100) as i32;
                    // SAFETY: both initialized pollfds borrow owned descriptors for this call.
                    if unsafe {
                        libc::poll(events.as_mut_ptr(), events.len() as libc::nfds_t, remaining)
                    } < 0
                    {
                        let error = io::Error::last_os_error();
                        if error.kind() != io::ErrorKind::Interrupted {
                            return Err(error.into());
                        }
                    }
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
}
