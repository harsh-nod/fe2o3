use crate::{
    cgroup::{ContainedChild, Scope},
    deployment::ProductionProofCustodianDeploymentV1,
    other, require, wire,
};
use fe2o3_kernel_descriptor::KernelId;
use fe2o3_protected_service_profile::validate_proof_controller_process_v1;
use fe2o3_protected_service_spawn::{
    PROTECTED_SERVICE_GATE_RELEASE_V1, PROTECTED_SERVICE_PROFILE_READY_V1,
    ProtectedServiceDescriptorBindingV1, StagedProofControllerExecV1,
};
use std::{
    io,
    os::fd::{AsFd, BorrowedFd, OwnedFd},
    time::{Duration, Instant},
};

const EXECUTION_TIMEOUT: Duration = Duration::from_secs(300);
const CONTROL_TIMEOUT: Duration = Duration::from_secs(30);

mod application;
pub use application::{
    PendingRootApplicationProofControllerV1, RootStagedApplicationProofControllerV1,
};

fn check_deadline(deadline: Instant) -> io::Result<()> {
    if Instant::now() < deadline {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "proof-controller deadline",
        ))
    }
}

fn try_read_byte(fd: &OwnedFd) -> io::Result<Option<(usize, u8)>> {
    let mut byte = [0];
    match rustix::io::read(fd, &mut byte) {
        Ok(n) => Ok(Some((n, byte[0]))),
        Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum LaunchPhase {
    Profile,
    Gate,
    Exec,
    Resources,
    Ready,
}

struct ExpectedReady {
    bytes: [u8; fe2o3_runtime_protocol::WORKER_V3_APPLICATION_PROOF_SESSION_BYTES_V1],
    len: usize,
}
impl ExpectedReady {
    fn new(bytes: &[u8]) -> Self {
        let mut value = Self {
            bytes: [0; fe2o3_runtime_protocol::WORKER_V3_APPLICATION_PROOF_SESSION_BYTES_V1],
            len: bytes.len(),
        };
        value.bytes[..bytes.len()].copy_from_slice(bytes);
        value
    }
    fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

/// Borrowed polling keeps original custody on every error. Call `poll_cancel`
/// until complete before discarding a failed reactor session. Drop is a blocking
/// containment backstop, not a reactor operation. Admission and bounded filesystem
/// reads remain synchronous; polling never waits for child progress.
///
/// ```compile_fail
/// use fe2o3_proof_custodian::PendingRootProofControllerLaunchV1;
/// fn sendable<T: Send>() {}
/// sendable::<PendingRootProofControllerLaunchV1>();
/// ```
pub struct PendingRootProofControllerLaunchV1 {
    controller: Option<RootManagedProofControllerV1>,
    ready_read: OwnedFd,
    gate_write: Option<OwnedFd>,
    status_read: OwnedFd,
    phase: LaunchPhase,
    expected_ready: ExpectedReady,
}

#[cfg(test)]
#[path = "qualification.rs"]
mod qualification;

/// Root-owned fixed controller with approved installed inputs and whole-tree containment.
///
/// This root control endpoint is not the application's proof peer. No application
/// registration or GPU settlement authority can be constructed from this owner.
/// A separately deployed root manager must contain its own death with whole-cgroup
/// shutdown; the inherited compiler coordinator cannot launch this unfiltered role.
///
/// ```compile_fail
/// use fe2o3_proof_custodian::RootManagedProofControllerV1;
/// fn cloneable<T: Clone>() {}
/// cloneable::<RootManagedProofControllerV1>();
/// ```
pub struct RootManagedProofControllerV1 {
    // Drop contains the child before any original deployment owner is released.
    contained: ContainedChild,
    deployment: ProductionProofCustodianDeploymentV1,
    control: wire::ControlEndpoint,
    nonce: [u8; 32],
    pid: u32,
    deadline: Instant,
    poisoned: bool,
}

impl ProductionProofCustodianDeploymentV1 {
    /// Seals bounded immutable compiler inputs, enters a fresh cgroup before exec,
    /// and waits for the fixed child to open the pinned analyzer and Verus runtime.
    ///
    /// Inputs are untrusted evidence. No publication lock, compiler audit, signing
    /// key or GPU descriptor is transferred. One absolute deadline covers startup
    /// and proof execution. Aggregate-empty polling is bounded; direct-child reap
    /// must complete before releasing custody and may wait indefinitely.
    pub fn launch(
        self,
        envelope: &[u8],
        payload: &[u8],
        kernel: KernelId,
    ) -> io::Result<RootManagedProofControllerV1> {
        let mut pending = self.begin_launch(envelope, payload, kernel)?;
        while !pending.poll()? {
            pending.wait_for_progress()?;
        }
        pending
            .take_ready()?
            .ok_or_else(|| io::Error::other("controller not ready"))
    }

    /// Stages and spawns the fixed child without waiting for profile/exec/resource
    /// readiness. Use the returned owner on the originating root thread only.
    pub fn begin_launch(
        self,
        envelope: &[u8],
        payload: &[u8],
        kernel: KernelId,
    ) -> io::Result<PendingRootProofControllerLaunchV1> {
        self.revalidate()?;
        let deadline = Instant::now() + EXECUTION_TIMEOUT;
        let request = wire::Request {
            nonce: wire::nonce()?,
            parent: std::process::id() as i32,
            kernel: *kernel.as_bytes(),
            envelope: (wire::digest(envelope), envelope.len() as u64),
            payload: (wire::digest(payload), payload.len() as u64),
        };
        wire::Request::decode(&request.encode())?;
        let request_file = wire::seal(&request.encode())?;
        let envelope = wire::seal(envelope)?;
        let payload = wire::seal(payload)?;
        self.begin_descriptor_launch(
            request.nonce,
            request_file.as_fd(),
            &[envelope.as_fd(), payload.as_fd()],
            deadline,
        )
    }
    fn begin_descriptor_launch(
        self,
        nonce: [u8; 32],
        request: BorrowedFd<'_>,
        inputs: &[BorrowedFd<'_>],
        deadline: Instant,
    ) -> io::Result<PendingRootProofControllerLaunchV1> {
        self.revalidate()?;
        let config = wire::seal(self.config.canonical_bytes())?;
        let image = self.executable.try_clone_for_exec().map_err(other)?;
        let (parent, child_end) = wire::control_pair()?;
        let parent = wire::ControlEndpoint::admit(parent)?;
        let (ready_read, ready_write) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC)?;
        let (gate_read, gate_write) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC)?;
        let (status_read, status_write) = rustix::net::socketpair(
            rustix::net::AddressFamily::UNIX,
            rustix::net::SocketType::SEQPACKET,
            rustix::net::SocketFlags::CLOEXEC,
            None,
        )?;
        // Only parent endpoints are nonblocking; the gated child must still wait.
        for fd in [&ready_read, &gate_write, &status_read] {
            let flags = rustix::fs::fcntl_getfl(fd)?;
            rustix::fs::fcntl_setfl(fd, flags | rustix::fs::OFlags::NONBLOCK)?;
        }
        let bindings = [config.as_fd(), request]
            .into_iter()
            .chain(inputs.iter().copied())
            .chain([child_end.as_fd()])
            .zip(3..)
            .map(|(fd, slot)| ProtectedServiceDescriptorBindingV1::new(fd, slot).map_err(other))
            .collect::<io::Result<Vec<_>>>()?;
        let staged = StagedProofControllerExecV1::new(
            &image,
            &bindings,
            ready_write.as_fd(),
            gate_read.as_fd(),
            status_write.as_fd(),
        )
        .map_err(other)?;
        let credentials = self.config.credentials()?;
        let expected_ready = ExpectedReady::new(&self.config.identity());
        let mut contained = ContainedChild::new(Scope::create()?);
        self.revalidate()?;
        contained.install(staged.spawn(credentials).map_err(other)?);
        drop((staged, child_end, ready_write, gate_read, status_write));
        let pid = contained.child().pid().as_raw_pid() as u32;
        let controller = RootManagedProofControllerV1 {
            contained,
            deployment: self,
            control: parent,
            nonce,
            pid,
            deadline,
            poisoned: false,
        };
        Ok(PendingRootProofControllerLaunchV1 {
            controller: Some(controller),
            ready_read,
            gate_write: Some(gate_write),
            status_read,
            phase: LaunchPhase::Profile,
            expected_ready,
        })
    }
}

impl PendingRootProofControllerLaunchV1 {
    /// Advances at most one phase. False also covers EINTR and backpressure.
    /// Errors poison the session but do not drop, reap or relinquish its owners.
    pub fn poll(&mut self) -> io::Result<bool> {
        let result = self.poll_inner();
        if result.is_err()
            && let Some(controller) = &mut self.controller
        {
            controller.poisoned = true;
        }
        result
    }
    fn poll_inner(&mut self) -> io::Result<bool> {
        let controller = self
            .controller
            .as_mut()
            .ok_or_else(|| io::Error::other("controller already taken"))?;
        require(!controller.poisoned, "controller channel poisoned")?;
        check_deadline(controller.deadline)?;
        controller.deployment.revalidate()?;
        controller.control.revalidate()?;
        match self.phase {
            LaunchPhase::Profile => {
                let Some((n, byte)) = try_read_byte(&self.ready_read)? else {
                    return Ok(false);
                };
                require(
                    n == 1 && byte == PROTECTED_SERVICE_PROFILE_READY_V1,
                    "controller failed pre-exec profile",
                )?;
                controller.revalidate()?;
                controller.contained.attach()?;
                self.phase = LaunchPhase::Gate;
            }
            LaunchPhase::Gate => {
                controller.revalidate()?;
                match rustix::io::write(
                    self.gate_write.as_ref().unwrap(),
                    &[PROTECTED_SERVICE_GATE_RELEASE_V1],
                ) {
                    Ok(1) => (),
                    Ok(_) => return Err(io::Error::other("controller gate write")),
                    Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => return Ok(false),
                    Err(error) => return Err(error.into()),
                }
                self.gate_write = None;
                self.phase = LaunchPhase::Exec;
            }
            LaunchPhase::Exec => {
                let Some((n, _)) = try_read_byte(&self.status_read)? else {
                    return Ok(false);
                };
                require(n == 0, "controller exec failed")?;
                self.phase = LaunchPhase::Resources;
            }
            LaunchPhase::Resources => {
                let Some((kind, body)) = controller.try_receive()? else {
                    return Ok(false);
                };
                require(
                    kind == wire::READY && body == self.expected_ready.as_bytes(),
                    "controller resources not ready",
                )?;
                self.phase = LaunchPhase::Ready;
            }
            LaunchPhase::Ready => controller.revalidate()?,
        }
        Ok(self.phase == LaunchPhase::Ready)
    }
    /// Moves the original ready owner out once. Not-ready and error paths retain
    /// it here; callers can continue polling or cancel without blocking cleanup.
    pub fn take_ready(&mut self) -> io::Result<Option<RootManagedProofControllerV1>> {
        let controller = self
            .controller
            .as_mut()
            .ok_or_else(|| io::Error::other("controller already taken"))?;
        require(!controller.poisoned, "controller channel poisoned")?;
        if self.phase != LaunchPhase::Ready {
            return Ok(None);
        }
        if let Err(error) =
            check_deadline(controller.deadline).and_then(|()| controller.revalidate())
        {
            controller.poisoned = true;
            return Err(error);
        }
        Ok(self.controller.take())
    }
    /// True requires original-child reap and aggregate-empty scope removal.
    pub fn poll_cancel(&mut self) -> io::Result<bool> {
        match &mut self.controller {
            Some(controller) => controller.poll_cancel(),
            None => Err(io::Error::other("controller already taken")),
        }
    }
    fn wait_for_progress(&self) -> io::Result<()> {
        use rustix::event::PollFlags;
        let controller = self.controller.as_ref().unwrap();
        let (fd, event) = match self.phase {
            LaunchPhase::Profile => (self.ready_read.as_fd(), PollFlags::IN),
            LaunchPhase::Gate => (self.gate_write.as_ref().unwrap().as_fd(), PollFlags::OUT),
            LaunchPhase::Exec => (self.status_read.as_fd(), PollFlags::IN),
            LaunchPhase::Resources => (controller.control.as_fd(), PollFlags::IN),
            LaunchPhase::Ready => return Ok(()),
        };
        wire::wait(fd, event, controller.deadline)
    }
}

impl RootManagedProofControllerV1 {
    pub fn deployment_identity(&self) -> [u8; 32] {
        self.deployment.config.identity()
    }
    pub fn child_pid(&self) -> u32 {
        self.pid
    }
    /// Duplicates the original process handle; it conveys neither reaping nor proof authority.
    pub fn try_clone_pidfd(&self) -> io::Result<OwnedFd> {
        self.revalidate()?;
        self.contained.child().try_clone_pidfd().map_err(other)
    }
    pub fn revalidate(&self) -> io::Result<()> {
        require(!self.poisoned, "controller channel poisoned")?;
        self.deployment.revalidate()?;
        require(
            self.contained.child().is_live().map_err(other)?,
            "controller exited",
        )?;
        validate_proof_controller_process_v1(
            self.deployment.config.credentials()?,
            self.contained.child().pid(),
        )
        .map_err(other)?;
        self.control.revalidate()
    }
    fn receive(&mut self) -> io::Result<(u8, Vec<u8>)> {
        let result = (|| loop {
            self.revalidate()?;
            wire::wait(
                self.control.as_fd(),
                rustix::event::PollFlags::IN,
                self.deadline,
            )?;
            if let Some(packet) = self.try_receive()? {
                return Ok(packet);
            }
        })();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    fn try_receive(&mut self) -> io::Result<Option<(u8, Vec<u8>)>> {
        self.revalidate()?;
        let credentials = self.deployment.config.credentials()?;
        let result = wire::try_receive(
            self.control.as_fd(),
            (
                self.child_pid() as i32,
                credentials.uid(),
                credentials.gid(),
            ),
            self.nonce,
        );
        match result {
            Ok(Some((wire::REJECTED, detail))) => {
                self.poisoned = true;
                Err(io::Error::other(format!(
                    "proof controller rejected: {}",
                    String::from_utf8_lossy(&detail)
                )))
            }
            Ok(packet) => match self.revalidate() {
                Ok(()) => Ok(packet),
                Err(error) => {
                    self.poisoned = true;
                    Err(error)
                }
            },
            Err(error) => {
                self.poisoned = true;
                Err(error)
            }
        }
    }
    fn send(&mut self, kind: u8) -> io::Result<()> {
        self.revalidate()?;
        let result = wire::send(self.control.as_fd(), self.nonce, kind, &[], self.deadline);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    /// Executes the one supplied closed fill and retains the same controller/proof owner.
    /// It does not authenticate an application or consume FD195 compiler currentness.
    pub fn prove(self) -> io::Result<RootRetainedConditionalFillProofV1> {
        let mut pending = self.begin_proof();
        while !pending.poll()? {
            pending.wait_for_progress()?;
        }
        pending
            .take_ready()?
            .ok_or_else(|| io::Error::other("proof not ready"))
    }
    /// Begins a borrowed-polling operation without sending Start or waiting.
    pub fn begin_proof(self) -> PendingRootConditionalFillProofV1 {
        PendingRootConditionalFillProofV1 {
            controller: Some(self),
            started: false,
            subject: None,
        }
    }
    /// Contains the entire proof process tree. This says nothing about GPU settlement.
    /// On error this borrowed owner keeps its child/scope and deployment custody.
    pub fn cancel(&mut self) -> io::Result<()> {
        self.poisoned = true;
        self.contained.stop()
    }
    /// One cancellation step; false preserves all remaining custody. Only true
    /// establishes direct-child reap and aggregate-empty original scope removal.
    /// The direct child may remain pending indefinitely; no GPU settlement follows.
    /// An error is terminal for polling and must not be treated as successful cleanup.
    pub fn poll_cancel(&mut self) -> io::Result<bool> {
        self.poisoned = true;
        self.contained.poll_stop()
    }
}

/// Original controller custody while proof execution progresses. Like pending
/// launch, errors retain ownership and Drop is a blocking containment backstop.
///
/// ```compile_fail
/// use fe2o3_proof_custodian::PendingRootConditionalFillProofV1;
/// fn sendable<T: Send>() {}
/// sendable::<PendingRootConditionalFillProofV1>();
/// ```
pub struct PendingRootConditionalFillProofV1 {
    controller: Option<RootManagedProofControllerV1>,
    started: bool,
    subject: Option<Vec<u8>>,
}
impl PendingRootConditionalFillProofV1 {
    /// One nonblocking send or receive attempt. Completion is authenticated by
    /// the original live controller and does not create application authority.
    pub fn poll(&mut self) -> io::Result<bool> {
        let result = self.poll_inner();
        if result.is_err()
            && let Some(controller) = &mut self.controller
        {
            controller.poisoned = true;
        }
        result
    }
    fn poll_inner(&mut self) -> io::Result<bool> {
        let controller = self
            .controller
            .as_mut()
            .ok_or_else(|| io::Error::other("proof already taken"))?;
        controller.revalidate()?;
        // A genuinely completed proof is retained beyond its execution deadline.
        if self.subject.is_some() {
            return Ok(true);
        }
        check_deadline(controller.deadline)?;
        if !self.started {
            self.started = wire::try_send(
                controller.control.as_fd(),
                controller.nonce,
                wire::START,
                &[],
            )?;
            controller.revalidate()?;
            return Ok(false);
        }
        let Some((kind, subject)) = controller.try_receive()? else {
            return Ok(false);
        };
        require(
            kind == wire::PROVED && !subject.is_empty(),
            "missing retained proof subject",
        )?;
        self.subject = Some(subject);
        Ok(true)
    }
    /// Moves the original proof/controller owner out once, retaining it here on
    /// error or before readiness. Subject bytes alone never construct this owner.
    pub fn take_ready(&mut self) -> io::Result<Option<RootRetainedConditionalFillProofV1>> {
        let controller = self
            .controller
            .as_mut()
            .ok_or_else(|| io::Error::other("proof already taken"))?;
        if let Err(error) = controller.revalidate() {
            controller.poisoned = true;
            return Err(error);
        }
        let Some(subject) = self.subject.take() else {
            return Ok(None);
        };
        Ok(Some(RootRetainedConditionalFillProofV1 {
            controller: self.controller.take().unwrap(),
            subject,
        }))
    }
    pub fn poll_cancel(&mut self) -> io::Result<bool> {
        match &mut self.controller {
            Some(controller) => controller.poll_cancel(),
            None => Err(io::Error::other("proof already taken")),
        }
    }
    fn wait_for_progress(&self) -> io::Result<()> {
        let controller = self.controller.as_ref().unwrap();
        let event = if self.started {
            rustix::event::PollFlags::IN
        } else {
            rustix::event::PollFlags::OUT
        };
        wire::wait(controller.control.as_fd(), event, controller.deadline)
    }
}

/// Root-side controller custody after actual proof execution, not an application lease.
/// No receipt import or unconditional executable conversion is available.
pub struct RootRetainedConditionalFillProofV1 {
    controller: RootManagedProofControllerV1,
    subject: Vec<u8>,
}
impl RootRetainedConditionalFillProofV1 {
    /// Matching bytes only. The original proof remains alive in the fixed controller.
    pub fn subject_bytes(&self) -> &[u8] {
        &self.subject
    }
    pub fn child_pid(&self) -> u32 {
        self.controller.child_pid()
    }
    pub fn probe(&mut self) -> io::Result<()> {
        self.controller.deadline = Instant::now() + CONTROL_TIMEOUT;
        self.controller.send(wire::PROBE)?;
        let (kind, body) = self.controller.receive()?;
        let result = require(
            kind == wire::RETAINED && body == self.subject,
            "retained proof subject changed",
        );
        if result.is_err() {
            self.controller.poisoned = true;
        }
        result
    }
    /// Releases this root-only proof session and verifies whole-tree cleanup.
    /// Applications cannot call this as native settlement: no application lease exists here.
    pub fn release(mut self) -> io::Result<()> {
        self.controller.deadline = Instant::now() + CONTROL_TIMEOUT;
        self.controller.send(wire::RELEASE)?;
        let credentials = self.controller.deployment.config.credentials()?;
        let (kind, body) = wire::receive(
            self.controller.control.as_fd(),
            (
                self.controller.child_pid() as i32,
                credentials.uid(),
                credentials.gid(),
            ),
            self.controller.nonce,
            self.controller.deadline,
        )?;
        require(
            kind == wire::RELEASED && body.is_empty(),
            "controller release acknowledgment mismatch",
        )?;
        self.controller.cancel()
    }
}
