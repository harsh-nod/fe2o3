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
    os::fd::{AsFd, OwnedFd},
    time::{Duration, Instant},
};

const EXECUTION_TIMEOUT: Duration = Duration::from_secs(300);
const CONTROL_TIMEOUT: Duration = Duration::from_secs(30);

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
        let config = wire::seal(self.config.canonical_bytes())?;
        let request_file = wire::seal(&request.encode())?;
        let envelope = wire::seal(envelope)?;
        let payload = wire::seal(payload)?;
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
        let bindings = [
            (config.as_fd(), 3),
            (request_file.as_fd(), 4),
            (envelope.as_fd(), 5),
            (payload.as_fd(), 6),
            (child_end.as_fd(), 7),
        ]
        .map(|(fd, slot)| ProtectedServiceDescriptorBindingV1::new(fd, slot).map_err(other))
        .into_iter()
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
        let mut contained = ContainedChild::new(Scope::create()?);
        self.revalidate()?;
        contained.install(staged.spawn(credentials).map_err(other)?);
        drop((staged, child_end, ready_write, gate_read, status_write));
        wire::wait(ready_read.as_fd(), rustix::event::PollFlags::IN, deadline)?;
        let mut byte = [0];
        require(
            rustix::io::read(&ready_read, &mut byte)? == 1
                && byte[0] == PROTECTED_SERVICE_PROFILE_READY_V1,
            "controller failed pre-exec profile",
        )?;
        validate_proof_controller_process_v1(credentials, contained.child().pid())
            .map_err(other)?;
        contained.attach()?;
        self.revalidate()?;
        require(
            rustix::io::write(&gate_write, &[PROTECTED_SERVICE_GATE_RELEASE_V1])? == 1,
            "controller gate write",
        )?;
        drop(gate_write);
        wire::wait(status_read.as_fd(), rustix::event::PollFlags::IN, deadline)?;
        require(
            rustix::io::read(&status_read, &mut byte)? == 0,
            "controller exec failed",
        )?;
        let pid = contained.child().pid().as_raw_pid() as u32;
        let mut controller = RootManagedProofControllerV1 {
            contained,
            deployment: self,
            control: parent,
            nonce: request.nonce,
            pid,
            deadline,
            poisoned: false,
        };
        let (kind, body) = controller.receive()?;
        require(
            kind == wire::READY && body == controller.deployment.config.identity(),
            "controller resources not ready",
        )?;
        controller.revalidate()?;
        Ok(controller)
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
        self.revalidate()?;
        let credentials = self.deployment.config.credentials()?;
        let result = wire::receive(
            self.control.as_fd(),
            (
                self.child_pid() as i32,
                credentials.uid(),
                credentials.gid(),
            ),
            self.nonce,
            self.deadline,
        );
        match result {
            Ok((wire::REJECTED, detail)) => {
                self.poisoned = true;
                Err(io::Error::other(format!(
                    "proof controller rejected: {}",
                    String::from_utf8_lossy(&detail)
                )))
            }
            Ok(packet) => {
                self.revalidate()?;
                Ok(packet)
            }
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
    pub fn prove(mut self) -> io::Result<RootRetainedConditionalFillProofV1> {
        self.send(wire::START)?;
        let (kind, subject) = self.receive()?;
        require(
            kind == wire::PROVED && !subject.is_empty(),
            "missing retained proof subject",
        )?;
        Ok(RootRetainedConditionalFillProofV1 {
            controller: self,
            subject,
        })
    }
    /// Contains the entire proof process tree. This says nothing about GPU settlement.
    /// On error this borrowed owner keeps its child/scope and deployment custody.
    pub fn cancel(&mut self) -> io::Result<()> {
        self.poisoned = true;
        self.contained.stop()
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
        require(
            kind == wire::RETAINED && body == self.subject,
            "retained proof subject changed",
        )
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
