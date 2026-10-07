use super::*;

enum Phase {
    Begin,
    Attach,
    AwaitReady,
    PublishReady(ProofSession, LiveClientPidfdIdentityV1),
    Activate(ProofSession),
    AwaitActivated(ProofSession),
    Active(ProofSession),
}
struct Application {
    operation: [u8; 32],
    deadline: u64,
    handoff: Option<PublishedApplicationCustodianHandoffV1>,
    phase: Phase,
}
struct Bootstrap {
    approval: Rc<Approval>,
    local: Rc<ApprovedProcess>,
    transport: Option<(Endpoint, ExpectedClientProcessIdentityV1)>,
    client_nonce: [u8; 32],
    hello: Vec<u8>,
    sent: bool,
    deadline: Instant,
}

/// Fixed-manager connection owned by the installed root coordinator.
/// Only an original published registration can enter its bounded staging table.
/// This owner grants no proof lease, native invocation authority or settlement.
pub struct RootProofManagerClientV1 {
    channel: Option<Channel>,
    bootstrap: Option<Bootstrap>,
    path: Option<path::SocketPath>,
    applications: Vec<Application>,
    cursor: usize,
    poisoned: bool,
}
impl RootProofManagerClientV1 {
    #[cfg(test)]
    pub(in crate::linux::observer_channel) fn ready_published_for_qualification(&self) -> bool {
        self.applications.iter().any(|app| app.handoff.is_none())
    }

    /// Authenticates both installed process images and original self-pidfds.
    /// Startup is bounded; it never opens a pidfd using the numeric peer PID.
    pub fn connect() -> Result<Self> {
        let mut client = Self::begin_connect()?;
        while client.channel.is_none() {
            client.step()?;
            std::thread::sleep(Duration::from_millis(1));
        }
        Ok(client)
    }

    /// Starts a cooperative bootstrap. Subsequent steps perform at most one
    /// send/receive; unrelated compiler observers and termination signals remain live.
    pub fn begin_connect() -> Result<Self> {
        let deadline = Instant::now() + TIMEOUT;
        let approval = Rc::new(Approval::open()?);
        let local = Rc::new(approval.admit_process(current_identity()?, false)?);
        let client_nonce = nonce()?;
        let hello = Frame {
            kind: Kind::Hello,
            connection: client_nonce,
            operation: [0; 32],
            sequence: 0,
            deadline: 0,
            body: approval.deployment.identity().to_vec(),
        }
        .encode()?;
        Ok(Self {
            channel: None,
            bootstrap: Some(Bootstrap {
                approval,
                local,
                transport: None,
                client_nonce,
                hello,
                sent: false,
                deadline,
            }),
            path: None,
            applications: Vec::with_capacity(CAPACITY),
            cursor: 0,
            poisoned: false,
        })
    }
    fn poll_connect(&mut self) -> Result<bool> {
        let bootstrap = self.bootstrap.as_mut().expect("pending manager bootstrap");
        require_deadline(bootstrap.deadline)?;
        bootstrap.approval.revalidate()?;
        bootstrap.local.revalidate()?;
        if bootstrap.transport.is_none() {
            let Some((path, peer)) = path::SocketPath::connect()? else {
                return Ok(false);
            };
            let endpoint = Endpoint::admit_with_profile(peer, true)?;
            let expected = peer_identity(&endpoint)?;
            bootstrap.transport = Some((endpoint, expected));
            self.path = Some(path);
            return Ok(true);
        }
        let (endpoint, expected) = bootstrap.transport.as_ref().unwrap();
        if !bootstrap.sent {
            bootstrap.sent =
                endpoint.send_bytes(&bootstrap.hello, &[bootstrap.local.process.pidfd.as_fd()])?;
            return Ok(bootstrap.sent);
        }
        let Some((bytes, mut rights)) = endpoint.receive_from_credentials(*expected)? else {
            return Ok(false);
        };
        let welcome = Frame::decode(&bytes)?;
        if welcome.kind != Kind::Welcome
            || welcome.connection != bootstrap.client_nonce
            || welcome.body != bootstrap.approval.deployment.identity()
            || rights.len() != 1
        {
            return Err(invalid("manager bootstrap response mismatch"));
        }
        let remote = Rc::new(bootstrap.approval.admit_process(
            LiveClientPidfdIdentityV1::admit(rights.remove(0), *expected)?,
            true,
        )?);
        let connection = wire::connection_nonce(
            bootstrap.client_nonce,
            welcome.operation,
            bootstrap.approval.deployment.identity(),
        )?;
        require_deadline(bootstrap.deadline)?;
        let bootstrap = self.bootstrap.take().unwrap();
        let channel = Channel {
            approval: bootstrap.approval,
            local: bootstrap.local,
            remote,
            endpoint: bootstrap.transport.unwrap().0,
            connection,
            send_sequence: 1,
            receive_sequence: 1,
            tid: rustix::thread::gettid().as_raw_pid() as u32,
            _thread: PhantomData,
        };
        channel.revalidate()?;
        self.path.as_ref().unwrap().revalidate()?;
        require_deadline(bootstrap.deadline)?;
        self.channel = Some(channel);
        Ok(true)
    }

    pub fn has_capacity(&self) -> bool {
        !self.poisoned && self.applications.len() < CAPACITY
    }

    /// Consumes registration custody; a rejected pre-Ready owner contains its application.
    pub fn stage(&mut self, handoff: PublishedApplicationCustodianHandoffV1) -> Result<()> {
        if !self.has_capacity() {
            return Err(invalid("proof-manager registration capacity exhausted"));
        }
        if let Some(channel) = &self.channel {
            channel.revalidate()?;
        } else {
            let bootstrap = self.bootstrap.as_ref().expect("pending manager bootstrap");
            require_deadline(bootstrap.deadline)?;
            bootstrap.approval.revalidate()?;
            bootstrap.local.revalidate()?;
        }
        if let Some(path) = &self.path {
            path.revalidate()?;
        }
        handoff.revalidate()?;
        let operation = handoff.transcript().root_nonce();
        if self
            .applications
            .iter()
            .any(|app| app.operation == operation)
        {
            return Err(invalid("duplicate proof-manager registration"));
        }
        let deadline = wire::export_deadline(handoff.startup_deadline())?;
        self.applications.push(Application {
            operation,
            deadline,
            handoff: Some(handoff),
            phase: Phase::Begin,
        });
        Ok(())
    }

    /// Progresses one received packet and at most one round-robin application transition.
    /// Errors terminally poison the routing connection; the independent manager retains
    /// every offered controller. No post-Ready error is interpreted as settlement.
    pub fn step(&mut self) -> Result<bool> {
        if self.poisoned {
            return Err(CompilerExecutionObserverErrorV1::Poisoned);
        }
        let result = self.step_inner();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    fn step_inner(&mut self) -> Result<bool> {
        if let Some(path) = &self.path {
            path.revalidate()?;
        }
        let Some(channel) = &mut self.channel else {
            for app in &self.applications {
                require_deadline(app.handoff.as_ref().unwrap().startup_deadline())?;
            }
            return self.poll_connect();
        };
        channel.revalidate()?;
        let mut progressed = false;
        if let Some((frame, mut rights)) = channel.receive()? {
            let app = self
                .applications
                .iter_mut()
                .find(|a| a.operation == frame.operation)
                .ok_or_else(|| invalid("unknown proof-manager reply"))?;
            match (&app.phase, frame.kind) {
                (Phase::AwaitReady, Kind::Ready) => {
                    let session = ProofSession::decode(&frame.body)
                        .map_err(|_| invalid("invalid manager proof session"))?;
                    let handoff = app.handoff.as_ref().expect("pre-Ready custody");
                    handoff.revalidate()?;
                    if session.transcript() != handoff.transcript()
                        || session.deployment()
                            != channel.approval.deployment.application_deployment()
                    {
                        return Err(invalid("manager session deployment or transcript mismatch"));
                    }
                    let (pid, uid, gid) = session.controller();
                    let controller = LiveClientPidfdIdentityV1::admit(
                        rights.remove(0),
                        ExpectedClientProcessIdentityV1::new(pid, uid, gid)?,
                    )?;
                    controller.validate_parent(&channel.remote.process)?;
                    app.phase = Phase::PublishReady(session, controller);
                }
                (Phase::AwaitActivated(session), Kind::Activated)
                    if frame.body == session.identity() =>
                {
                    app.phase = Phase::Active(session.clone());
                }
                (_, Kind::Rejected) => {
                    return Err(invalid("proof manager rejected application startup"));
                }
                _ => return Err(invalid("out-of-phase proof-manager reply")),
            }
            progressed = true;
        }
        if self.applications.is_empty() {
            return Ok(progressed);
        }
        let index = self.cursor % self.applications.len();
        self.cursor = (index + 1) % self.applications.len();
        let app = &mut self.applications[index];
        if let Some(handoff) = &app.handoff {
            handoff.revalidate()?;
        }
        match &app.phase {
            Phase::Begin => {
                let handoff = app.handoff.as_ref().unwrap();
                if channel.send(
                    Kind::Begin,
                    app.operation,
                    app.deadline,
                    handoff.binding().canonical_bytes(),
                    &handoff.process_rights(),
                )? {
                    app.phase = Phase::Attach;
                    progressed = true;
                }
            }
            Phase::Attach => {
                let handoff = app.handoff.as_ref().unwrap();
                let mut body = handoff.transcript().app_nonce().to_vec();
                body.extend_from_slice(&handoff.transcript().root_nonce());
                if channel.send(
                    Kind::Attach,
                    app.operation,
                    app.deadline,
                    &body,
                    &[handoff.proof_peer()],
                )? {
                    app.phase = Phase::AwaitReady;
                    progressed = true;
                }
            }
            Phase::PublishReady(session, controller) => {
                let published = app
                    .handoff
                    .as_mut()
                    .unwrap()
                    .publish_ready(session, controller)?;
                if published {
                    // Plain Drop closes the coordinator alias. No receive/check follows Ready.
                    app.handoff = None;
                    app.phase = Phase::Activate(session.clone());
                    progressed = true;
                }
            }
            Phase::Activate(session) => {
                let sent =
                    channel.send(Kind::Activate, app.operation, 0, &session.identity(), &[])?;
                if sent {
                    app.phase = Phase::AwaitActivated(session.clone());
                    progressed = true;
                }
            }
            _ => {}
        }
        Ok(progressed)
    }

    /// Diagnostic count only; activation is not proof success or GPU execution.
    pub fn activated_count(&self) -> usize {
        self.applications.iter().filter(|app| matches!(&app.phase, Phase::Active(session) if session.transcript().root_nonce() == app.operation)).count()
    }
}
