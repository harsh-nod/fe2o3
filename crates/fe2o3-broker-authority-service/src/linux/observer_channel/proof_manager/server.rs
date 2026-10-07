use super::*;

struct Bootstrap {
    approval: Rc<Approval>,
    local: Rc<ApprovedProcess>,
    listener: OwnedFd,
    endpoint: Option<Endpoint>,
    remote: Option<Rc<ApprovedProcess>>,
    welcome: Option<Frame>,
    deadline: Option<Instant>,
}
enum Phase {
    AwaitAttach {
        application: LiveClientPidfdIdentityV1,
        cargo: LiveClientPidfdIdentityV1,
    },
    Staged,
    Offered(ProofSession),
    Activating(ProofSession),
    Active(ProofSession),
}
struct Registration {
    operation: [u8; 32],
    binding: Binding,
    transcript: Option<Transcript>,
    deadline_wire: u64,
    deadline: Instant,
    phase: Phase,
}

/// One original registration received from the independently approved coordinator.
/// No public constructor, Clone, or arbitrary descriptor import exists.
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::ReceivedPublishedApplicationV1;
/// let _ = ReceivedPublishedApplicationV1 {};
/// ```
pub struct ReceivedPublishedApplicationV1 {
    binding: Binding,
    transcript: Transcript,
    deadline: Instant,
    application: LiveClientPidfdIdentityV1,
    cargo: LiveClientPidfdIdentityV1,
    peer: Endpoint,
    approval: Rc<Approval>,
    coordinator: Rc<ApprovedProcess>,
    manager: Rc<ApprovedProcess>,
    tid: u32,
}
impl ReceivedPublishedApplicationV1 {
    pub fn binding(&self) -> &Binding {
        &self.binding
    }
    pub fn transcript(&self) -> Transcript {
        self.transcript
    }
    pub fn startup_deadline(&self) -> Instant {
        self.deadline
    }
    pub fn revalidate(&self) -> Result<()> {
        require_deadline(self.deadline)?;
        if std::process::id() != self.manager.process.expected_client.pid
            || rustix::thread::gettid().as_raw_pid() as u32 != self.tid
        {
            return Err(invalid("received registration moved process/thread"));
        }
        self.approval.revalidate()?;
        self.coordinator.revalidate()?;
        self.manager.revalidate()?;
        self.application.validate_parent(&self.cargo)?;
        self.peer.revalidate()?;
        if self.peer.creator != self.cargo.expected_client.credentials() {
            return Err(invalid("received application peer creator differs"));
        }
        require_deadline(self.deadline)
    }
    /// Consumes authenticated staging custody into the fixed controller launcher.
    /// The callback receives the original application/Cargo pidfds and exact peer,
    /// in that order. It cannot mint another registered owner from those descriptors.
    pub fn stage<T>(
        self,
        launch: impl FnOnce(Binding, Transcript, Instant, [OwnedFd; 3]) -> io::Result<T>,
    ) -> io::Result<T> {
        self.revalidate().map_err(io::Error::other)?;
        launch(
            self.binding,
            self.transcript,
            self.deadline,
            [self.application.pidfd, self.cargo.pidfd, self.peer.peer],
        )
    }
}

/// Authenticated manager commands. Bytes in these commands are not proof evidence.
pub enum ProofManagerCommandV1 {
    Registered(Box<ReceivedPublishedApplicationV1>),
    Activate {
        operation: [u8; 32],
        session: [u8; 32],
    },
}

/// Single-coordinator, fixed-path root manager ingress with bounded registration accounting.
/// A lost connection is terminal; it never authorizes cancelling an offered controller.
pub struct RootProofManagerServerV1 {
    path: path::SocketPath,
    bootstrap: Option<Bootstrap>,
    channel: Option<Channel>,
    registrations: Vec<Registration>,
    poisoned: bool,
    application_deployment: [u8; 32],
}
impl RootProofManagerServerV1 {
    pub fn listen() -> Result<Self> {
        let approval = Rc::new(Approval::open()?);
        let local = Rc::new(approval.admit_process(current_identity()?, true)?);
        let application_deployment = approval.deployment.application_deployment();
        let (path, listener) = path::SocketPath::listen()?;
        Ok(Self {
            path,
            bootstrap: Some(Bootstrap {
                approval,
                local,
                listener,
                endpoint: None,
                remote: None,
                welcome: None,
                deadline: None,
            }),
            channel: None,
            registrations: Vec::with_capacity(CAPACITY),
            poisoned: false,
            application_deployment,
        })
    }
    pub fn application_deployment(&self) -> [u8; 32] {
        self.application_deployment
    }
    /// Plain-closes only the manager control endpoint. Physical controller custody
    /// remains with its independent owner; this is not a cancellation or settlement.
    pub fn disconnect(&mut self) {
        self.poisoned = true;
        self.channel = None;
        self.bootstrap = None;
    }
    /// Polls one bootstrap or command step. No reconnect or registration replay is admitted.
    pub fn poll(&mut self) -> Result<Option<ProofManagerCommandV1>> {
        if self.poisoned {
            return Err(CompilerExecutionObserverErrorV1::Poisoned);
        }
        let result = self.poll_inner();
        if result.is_err() {
            self.disconnect();
        }
        result
    }
    fn poll_inner(&mut self) -> Result<Option<ProofManagerCommandV1>> {
        self.path.revalidate()?;
        if self.channel.is_none() {
            self.poll_bootstrap()?;
            return Ok(None);
        }
        let channel = self.channel.as_mut().unwrap();
        // All pre-offer operations retain their original absolute deadline.
        for registration in &self.registrations {
            if matches!(
                registration.phase,
                Phase::AwaitAttach { .. } | Phase::Staged
            ) {
                require_deadline(registration.deadline)?;
            }
        }
        let Some((frame, mut rights)) = channel.receive()? else {
            return Ok(None);
        };
        if frame.kind == Kind::Begin {
            if self.registrations.len() == CAPACITY
                || self
                    .registrations
                    .iter()
                    .any(|r| r.operation == frame.operation)
            {
                return Err(invalid("duplicate or excess manager registration"));
            }
            let binding = Binding::decode(&frame.body)
                .map_err(|_| invalid("manager registration binding"))?;
            let deadline = wire::import_deadline(frame.deadline)?;
            let app = binding.compiler_handoff().launch_manifest().client();
            let parent = binding.compiler_handoff().submitter();
            if app.uid() == 0
                || parent.uid() == 0
                || app.gid() == 0
                || parent.gid() == 0
                || self.registrations.iter().any(|r| {
                    r.binding
                        .compiler_handoff()
                        .launch_manifest()
                        .client()
                        .pid()
                        == app.pid()
                })
            {
                return Err(invalid(
                    "application role overlaps manager or prior registration",
                ));
            }
            let application = LiveClientPidfdIdentityV1::admit(
                rights.remove(0),
                ExpectedClientProcessIdentityV1::new(app.pid(), app.uid(), app.gid())?,
            )?;
            let cargo = LiveClientPidfdIdentityV1::admit(
                rights.remove(0),
                ExpectedClientProcessIdentityV1::new(parent.pid(), parent.uid(), parent.gid())?,
            )?;
            application.validate_parent(&cargo)?;
            self.registrations.push(Registration {
                operation: frame.operation,
                binding,
                deadline,
                deadline_wire: frame.deadline,
                transcript: None,
                phase: Phase::AwaitAttach { application, cargo },
            });
            return Ok(None);
        }
        let registration = self
            .registrations
            .iter_mut()
            .find(|r| r.operation == frame.operation)
            .ok_or_else(|| invalid("unknown manager registration command"))?;
        match frame.kind {
            Kind::Attach => {
                if !matches!(registration.phase, Phase::AwaitAttach { .. })
                    || frame.deadline != registration.deadline_wire
                    || frame.body[32..] != frame.operation
                {
                    return Err(invalid("manager attachment replay or deadline changed"));
                }
                let transcript = Transcript::new(
                    frame.body[..32].try_into().unwrap(),
                    frame.operation,
                    *registration.binding.identity().as_bytes(),
                )
                .map_err(|_| invalid("manager attachment transcript"))?;
                let peer = Endpoint::admit(rights.remove(0))?;
                let Phase::AwaitAttach { application, cargo } =
                    std::mem::replace(&mut registration.phase, Phase::Staged)
                else {
                    unreachable!()
                };
                let received = ReceivedPublishedApplicationV1 {
                    binding: registration.binding.clone(),
                    transcript,
                    deadline: registration.deadline,
                    application,
                    cargo,
                    peer,
                    approval: Rc::clone(&channel.approval),
                    coordinator: Rc::clone(&channel.remote),
                    manager: Rc::clone(&channel.local),
                    tid: channel.tid,
                };
                received.revalidate()?;
                registration.transcript = Some(transcript);
                Ok(Some(ProofManagerCommandV1::Registered(Box::new(received))))
            }
            Kind::Activate => {
                let Phase::Offered(session) = &registration.phase else {
                    return Err(invalid("manager Activate before Ready or replay"));
                };
                if frame.body != session.identity() {
                    return Err(invalid("manager activation session mismatch"));
                }
                let identity = session.identity();
                registration.phase = Phase::Activating(session.clone());
                Ok(Some(ProofManagerCommandV1::Activate {
                    operation: frame.operation,
                    session: identity,
                }))
            }
            _ => Err(invalid("unexpected manager command phase")),
        }
    }
    fn poll_bootstrap(&mut self) -> Result<()> {
        let bootstrap = self.bootstrap.as_mut().expect("bootstrap owner");
        bootstrap.approval.revalidate()?;
        bootstrap.local.revalidate()?;
        if let Some(deadline) = bootstrap.deadline {
            require_deadline(deadline)?;
        }
        if bootstrap.endpoint.is_none() {
            let peer = match rustix::net::accept_with(
                &bootstrap.listener,
                SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            ) {
                Ok(peer) => peer,
                Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => return Ok(()),
                Err(error) => return Err(error.into()),
            };
            rustix::net::sockopt::set_socket_passcred(&peer, true)?;
            bootstrap.endpoint = Some(Endpoint::admit_with_profile(peer, true)?);
            bootstrap.deadline = Some(Instant::now() + TIMEOUT);
            return Ok(());
        }
        let endpoint = bootstrap.endpoint.as_ref().unwrap();
        if bootstrap.welcome.is_none() {
            let expected = peer_identity(endpoint)?;
            let Some((bytes, mut rights)) = endpoint.receive_from_credentials(expected)? else {
                return Ok(());
            };
            let hello = Frame::decode(&bytes)?;
            if hello.kind != Kind::Hello
                || hello.body != bootstrap.approval.deployment.identity()
                || rights.len() != 1
            {
                return Err(invalid("manager bootstrap Hello mismatch"));
            }
            bootstrap.remote = Some(Rc::new(bootstrap.approval.admit_process(
                LiveClientPidfdIdentityV1::admit(rights.remove(0), expected)?,
                false,
            )?));
            let manager_nonce = nonce()?;
            wire::connection_nonce(
                hello.connection,
                manager_nonce,
                bootstrap.approval.deployment.identity(),
            )?;
            bootstrap.welcome = Some(Frame {
                kind: Kind::Welcome,
                connection: hello.connection,
                operation: manager_nonce,
                sequence: 0,
                deadline: 0,
                body: bootstrap.approval.deployment.identity().to_vec(),
            });
            return Ok(());
        }
        bootstrap.remote.as_ref().unwrap().revalidate()?;
        let welcome = bootstrap.welcome.as_ref().unwrap();
        if !endpoint.send_bytes(&welcome.encode()?, &[bootstrap.local.process.pidfd.as_fd()])? {
            return Ok(());
        }
        let connection = wire::connection_nonce(
            welcome.connection,
            welcome.operation,
            bootstrap.approval.deployment.identity(),
        )?;
        let bootstrap = self.bootstrap.take().unwrap();
        self.channel = Some(Channel {
            approval: bootstrap.approval,
            local: bootstrap.local,
            remote: bootstrap.remote.unwrap(),
            endpoint: bootstrap.endpoint.unwrap(),
            connection,
            send_sequence: 1,
            receive_sequence: 1,
            tid: rustix::thread::gettid().as_raw_pid() as u32,
            _thread: PhantomData,
        });
        Ok(())
    }
    /// The physical controller owner must already have entered sticky ReadyOffered custody.
    /// False permits retry of this exact offer, never cancellation or replacement.
    pub fn offer_ready(
        &mut self,
        session: &ProofSession,
        controller_pidfd: BorrowedFd<'_>,
    ) -> Result<bool> {
        if self.poisoned {
            return Err(CompilerExecutionObserverErrorV1::Poisoned);
        }
        let result = self.offer_ready_inner(session, controller_pidfd);
        if result.is_err() {
            self.disconnect();
        }
        result
    }
    fn offer_ready_inner(
        &mut self,
        session: &ProofSession,
        controller_pidfd: BorrowedFd<'_>,
    ) -> Result<bool> {
        self.path.revalidate()?;
        let channel = self
            .channel
            .as_mut()
            .ok_or_else(|| invalid("manager bootstrap incomplete"))?;
        let registration = self
            .registrations
            .iter_mut()
            .find(|r| r.operation == session.transcript().root_nonce())
            .ok_or_else(|| invalid("unknown manager Ready"))?;
        if !matches!(registration.phase, Phase::Staged)
            || registration.transcript != Some(session.transcript())
            || session.deployment() != channel.approval.deployment.application_deployment()
        {
            return Err(invalid(
                "manager Ready changed registered identity or phase",
            ));
        }
        require_deadline(registration.deadline)?;
        let (pid, uid, gid) = session.controller();
        let controller = LiveClientPidfdIdentityV1::admit(
            rustix::io::fcntl_dupfd_cloexec(controller_pidfd, 0)?,
            ExpectedClientProcessIdentityV1::new(pid, uid, gid)?,
        )?;
        controller.validate_parent(&channel.local.process)?;
        if !channel.send(
            Kind::Ready,
            registration.operation,
            0,
            session.canonical_bytes(),
            &[controller_pidfd],
        )? {
            return Ok(false);
        }
        registration.phase = Phase::Offered(session.clone());
        Ok(true)
    }
    pub fn acknowledge_activation(&mut self, session: &ProofSession) -> Result<bool> {
        if self.poisoned {
            return Err(CompilerExecutionObserverErrorV1::Poisoned);
        }
        let result = self.acknowledge_activation_inner(session);
        if result.is_err() {
            self.disconnect();
        }
        result
    }
    fn acknowledge_activation_inner(&mut self, session: &ProofSession) -> Result<bool> {
        self.path.revalidate()?;
        let channel = self
            .channel
            .as_mut()
            .ok_or_else(|| invalid("manager bootstrap incomplete"))?;
        let registration = self
            .registrations
            .iter_mut()
            .find(|r| r.operation == session.transcript().root_nonce())
            .ok_or_else(|| invalid("unknown manager activation acknowledgment"))?;
        if !matches!(&registration.phase,Phase::Activating(expected) if expected==session) {
            return Err(invalid("manager activation acknowledgment phase"));
        }
        if !channel.send(
            Kind::Activated,
            registration.operation,
            0,
            &session.identity(),
            &[],
        )? {
            return Ok(false);
        }
        registration.phase = Phase::Active(session.clone());
        Ok(true)
    }
    pub fn activated_count(&self) -> usize {
        self.registrations.iter().filter(|r| matches!(&r.phase,Phase::Active(session) if session.transcript().root_nonce()==r.operation)).count()
    }
}
