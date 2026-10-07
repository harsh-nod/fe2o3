//! Native-only application registration with separately pinned installed policy.
use super::{registration::require_connected, transport, *};
use fe2o3_compiler_closure_capability::{
    ProductionCompilerExecutionDeploymentErrorV3 as DeploymentError,
    ProductionCompilerExecutionDeploymentV3 as Deployment,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_process_identity::pidfd::{
    NativeReceivedProcessPidfdErrorV1 as PidfdError, NativeReceivedProcessPidfdV1 as NativePidfd,
};
use fe2o3_runtime_protocol::{
    NativeApplicationRegistrationBindingV1 as Binding,
    NativeApplicationRegistrationErrorV1 as ProtocolError,
    NativeApplicationRegistrationInputsV1 as Inputs, NativeApplicationSessionKindV1 as Kind,
    NativeApplicationSessionMessageV1 as Message,
    NativeApplicationSessionTranscriptV1 as Transcript, WorkerV3ApplicationInputOccurrenceV1,
};
use std::{
    mem::size_of,
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, NativeApplicationChannelErrorV1>;
mod currentness;
pub use currentness::NativeApplicationCurrentRecordV1;
mod custodian;
pub use custodian::{
    NativeCustodianCurrentRecordV1, RegisteredNativeApplicationCustodianV1,
    RetainedNativeApplicationProofV1,
};
const MAX_STARTUP: Duration = Duration::from_secs(120);
const IO_SCRATCH: usize = 128 * 1024;
const ATTEMPT_WORK: usize = 32 * 1024;

#[derive(Debug)]
pub enum NativeApplicationChannelErrorV1 {
    Transport(ApplicationProofChannelErrorV1),
    Protocol(ProtocolError),
    Deployment(DeploymentError),
    Client(crate::CompilerExecutionClientErrorV3),
    Association(fe2o3_runtime_protocol::NativeConditionalApplicationBindingErrorV1),
    Proof(fe2o3_runtime_protocol::NativeApplicationProofErrorV1),
    Startup(fe2o3_runtime_protocol::NativeApplicationStartupErrorV1),
    ProofProfile(fe2o3_compiler_closure_capability::ProductionNativeApplicationProofProfileErrorV1),
    Resource(Resource),
    Binding(&'static str),
}
impl From<ApplicationProofChannelErrorV1> for NativeApplicationChannelErrorV1 {
    fn from(error: ApplicationProofChannelErrorV1) -> Self {
        Self::Transport(error)
    }
}
impl From<Resource> for NativeApplicationChannelErrorV1 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl From<fe2o3_runtime_protocol::NativeApplicationStartupErrorV1>
    for NativeApplicationChannelErrorV1
{
    fn from(error: fe2o3_runtime_protocol::NativeApplicationStartupErrorV1) -> Self {
        Self::Startup(error)
    }
}
impl From<ProtocolError> for NativeApplicationChannelErrorV1 {
    fn from(error: ProtocolError) -> Self {
        Self::Protocol(error)
    }
}
impl From<DeploymentError> for NativeApplicationChannelErrorV1 {
    fn from(error: DeploymentError) -> Self {
        Self::Deployment(error)
    }
}
impl From<fe2o3_runtime_protocol::NativeApplicationProofErrorV1>
    for NativeApplicationChannelErrorV1
{
    fn from(error: fe2o3_runtime_protocol::NativeApplicationProofErrorV1) -> Self {
        Self::Proof(error)
    }
}
impl From<fe2o3_compiler_closure_capability::ProductionNativeApplicationProofProfileErrorV1>
    for NativeApplicationChannelErrorV1
{
    fn from(
        error: fe2o3_compiler_closure_capability::ProductionNativeApplicationProofProfileErrorV1,
    ) -> Self {
        Self::ProofProfile(error)
    }
}
impl fmt::Display for NativeApplicationChannelErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native application channel rejected: {self:?}")
    }
}
impl std::error::Error for NativeApplicationChannelErrorV1 {}

/// Unreserved growth over the consumed endpoint, inputs and deployment charges.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeApplicationChannelStorageV1(usize);
impl NativeApplicationChannelStorageV1 {
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}

/// Original native registration transport, root pidfd, binding and pinned V3 policy.
///
/// This owner cannot be reconstructed from a transcript, root credential sample,
/// caller policy or bare verified current-record response. Registration alone
/// grants no protected proof custody, current-record, load or launch authority.
/// Its root-liveness requirement applies to the ordinary Ready route only;
/// CustodianReady is a distinct transition and is rejected here.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_client::RegisteredNativeApplicationProofEndpointV1 as Native;
/// fn clone(value: Native) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_client::RegisteredNativeApplicationProofEndpointV1 as Native;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<Native>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_client::{RegisteredNativeApplicationProofEndpointV1 as Native,
///     RegisteredApplicationProofEndpointV1 as Legacy};
/// fn promote(value: Legacy) -> Native { value.into() }
/// ```
pub struct RegisteredNativeApplicationProofEndpointV1<'work> {
    endpoint: RetainedApplicationProofEndpointV1,
    root: NativePidfd<'work>,
    sender: CompilerExecutionClientProcessIdentityV1,
    transcript: Transcript,
    binding: Binding,
    deployment: Deployment<'work>,
}
impl fmt::Debug for RegisteredNativeApplicationProofEndpointV1<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegisteredNativeApplicationProofEndpointV1")
            .field("transcript", &self.transcript)
            .finish_non_exhaustive()
    }
}
impl<'work> RegisteredNativeApplicationProofEndpointV1<'work> {
    /// Exact prepaid endpoint input, not any proof or policy authority.
    pub const ENDPOINT_STORAGE: usize = size_of::<RetainedApplicationProofEndpointV1>();

    pub fn retained_storage(&self) -> Result<usize> {
        sum(
            size_of::<(Self, NativeApplicationChannelStorageV1)>(),
            sum(
                self.binding
                    .retained_storage()
                    .checked_sub(size_of::<Binding>())
                    .ok_or(Resource::Arithmetic)?,
                self.deployment
                    .retained_storage()?
                    .checked_sub(size_of::<Deployment<'work>>())
                    .ok_or(Resource::Arithmetic)?,
            )?,
        )
    }
    pub fn registration(&self) -> &Binding {
        &self.binding
    }
    pub const fn transcript(&self) -> Transcript {
        self.transcript
    }
    pub fn deployment(&self) -> &Deployment<'work> {
        &self.deployment
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
    pub const fn authenticates_protected_currentness(&self) -> bool {
        false
    }

    pub fn revalidate(&self, budget: &mut Budget<'work>) -> Result<()> {
        budget.charge_work(ATTEMPT_WORK)?;
        if budget.storage() < self.retained_storage()? {
            return Err(Resource::Accounting.into());
        }
        self.revalidate_inner(budget)
    }
    fn revalidate_inner(&self, budget: &mut Budget<'work>) -> Result<()> {
        self.deployment.revalidate(budget)?;
        self.endpoint.revalidate()?;
        require_connected(&self.endpoint)?;
        self.root.revalidate(budget).map_err(pidfd_error)?;
        if self.sender.pid() != self.root.pid() || self.sender.uid() != 0 || self.sender.gid() != 0
        {
            return Err(NativeApplicationChannelErrorV1::Binding(
                "original root sender",
            ));
        }
        check_binding(&self.endpoint, &self.binding, &self.deployment)?;
        self.endpoint.revalidate()?;
        require_connected(&self.endpoint)?;
        Ok(())
    }
}

impl RetainedApplicationProofEndpointV1 {
    /// Completes the native Ready route before application startup ACK.
    ///
    /// Prepay the exact endpoint, native inputs and original deployment. Failure
    /// consumes all three owners, closes descriptors and retains operation charges;
    /// it cannot retry legacy registration. Successful output returns only growth.
    /// All packet retries charge the original ledger under one absolute deadline.
    pub fn register_native_pre_ack<'work>(
        self,
        inputs: Inputs,
        deployment: Deployment<'work>,
        deadline: Instant,
        budget: &mut Budget<'work>,
    ) -> Result<(
        RegisteredNativeApplicationProofEndpointV1<'work>,
        NativeApplicationChannelStorageV1,
    )> {
        let accepted = self.begin_native_registration(inputs, deployment, deadline, budget)?;
        let (ready, ready_raw) = receive(&accepted.endpoint, &[&accepted.root], deadline, budget)?;
        if ready.kind() != Kind::Ready
            || ready_raw.sender != accepted.sender
            || ready.transcript() != Some(accepted.transcript)
        {
            return Err(NativeApplicationChannelErrorV1::Binding(
                "native Ready sender or phase",
            ));
        }
        let value = RegisteredNativeApplicationProofEndpointV1 {
            endpoint: accepted.endpoint,
            root: accepted.root,
            sender: accepted.sender,
            transcript: accepted.transcript,
            binding: accepted.binding,
            deployment: accepted.deployment,
        };
        value.revalidate_inner(budget)?;
        transport::check_deadline(deadline)?;
        let additional = value
            .retained_storage()?
            .checked_sub(accepted.inherited)
            .ok_or(Resource::Arithmetic)?;
        drop((ready, ready_raw));
        finish_native_operation(accepted.floor, budget)?;
        Ok((value, NativeApplicationChannelStorageV1(additional)))
    }

    fn begin_native_registration<'work>(
        self,
        inputs: Inputs,
        deployment: Deployment<'work>,
        deadline: Instant,
        budget: &mut Budget<'work>,
    ) -> Result<NativeAccepted<'work>> {
        let inherited = sum(
            RegisteredNativeApplicationProofEndpointV1::ENDPOINT_STORAGE,
            sum(inputs.retained_storage(), deployment.retained_storage()?)?,
        )?;
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        budget.charge_work(ATTEMPT_WORK)?;
        if floor < inherited {
            return Err(Resource::Accounting.into());
        }
        budget.reserve_storage(IO_SCRATCH)?;
        transport::check_deadline(deadline)?;
        if deadline.saturating_duration_since(Instant::now()) > MAX_STARTUP {
            return Err(NativeApplicationChannelErrorV1::Binding("startup deadline"));
        }
        deployment.revalidate(budget)?;
        self.revalidate()?;
        if rustix::process::geteuid().as_raw() == 0 {
            return Err(NativeApplicationChannelErrorV1::Binding(
                "unprivileged application",
            ));
        }
        let app_nonce = fresh_nonce(deadline, budget)?;
        let (hello, charge) = Message::hello(&inputs, app_nonce, budget)?;
        budget.reserve_storage(charge.additional_storage())?;
        send(&self, &[], &hello, deadline, budget)?;
        let (challenge, mut raw) = receive(&self, &[], deadline, budget)?;
        if challenge.kind() != Kind::Challenge
            || challenge.app_nonce() != app_nonce
            || raw.sender.uid() != 0
            || raw.sender.gid() != 0
            || raw.sender.pid() == self.application_pid
        {
            return Err(NativeApplicationChannelErrorV1::Binding(
                "native Challenge sender or phase",
            ));
        }
        let (binding, charge) = challenge.decode_registration(budget)?;
        budget.reserve_storage(charge.additional_storage())?;
        if binding.inputs().canonical_bytes() != inputs.canonical_bytes() {
            return Err(NativeApplicationChannelErrorV1::Binding(
                "native input occurrence",
            ));
        }
        check_binding(&self, &binding, &deployment)?;
        let (root, charge) = NativePidfd::admit_received(
            raw.rights
                .pop()
                .ok_or(NativeApplicationChannelErrorV1::Binding("root pidfd"))?,
            raw.sender.pid(),
            budget,
        )
        .map_err(pidfd_error)?;
        budget.reserve_storage(charge.additional_storage())?;
        let transcript = challenge
            .transcript()
            .ok_or(NativeApplicationChannelErrorV1::Binding(
                "native transcript",
            ))?;
        let (accept, charge) = Message::accept(transcript, budget)?;
        budget.reserve_storage(charge.additional_storage())?;
        send(&self, &[&root], &accept, deadline, budget)?;
        let value = NativeAccepted {
            endpoint: self,
            root,
            sender: raw.sender,
            transcript,
            binding,
            deployment,
            floor,
            inherited,
        };
        transport::check_deadline(deadline)?;
        if budget.work_ledger_identity_v1() != ledger {
            return Err(Resource::Accounting.into());
        }
        drop((hello, challenge, raw, accept, inputs));
        Ok(value)
    }
}

struct NativeAccepted<'work> {
    endpoint: RetainedApplicationProofEndpointV1,
    root: NativePidfd<'work>,
    sender: CompilerExecutionClientProcessIdentityV1,
    transcript: Transcript,
    binding: Binding,
    deployment: Deployment<'work>,
    floor: usize,
    inherited: usize,
}
fn finish_native_operation(floor: usize, budget: &mut Budget<'_>) -> Result<()> {
    budget.release_storage(
        budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)?,
    )?;
    Ok(())
}

fn check_binding(
    endpoint: &RetainedApplicationProofEndpointV1,
    binding: &Binding,
    deployment: &Deployment<'_>,
) -> Result<()> {
    let handoff = binding.compiler_handoff();
    let (device, inode, mode) = endpoint.snapshot.object;
    let input =
        WorkerV3ApplicationInputOccurrenceV1::from_linux_descriptor_v1(4, device, inode, mode)
            .map_err(|_| NativeApplicationChannelErrorV1::Binding("native endpoint occurrence"))?;
    if handoff.launch_manifest().client() != current_creator(endpoint.application_pid)?
        || handoff.submitter() != endpoint.snapshot.creator
        || binding.descriptors().as_array()[3] != endpoint.peer.as_raw_fd()
        || binding.occurrence().inputs().get(3) != Some(&input)
        || handoff.launch_manifest().policy_identity() != deployment.policy().identity()
    {
        return Err(NativeApplicationChannelErrorV1::Binding(
            "native application, Cargo or pinned policy",
        ));
    }
    Ok(())
}
fn sum(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).ok_or(Resource::Arithmetic.into())
}

fn fresh_nonce(deadline: Instant, budget: &mut Budget<'_>) -> Result<[u8; 32]> {
    loop {
        budget.charge_work(64)?;
        transport::check_deadline(deadline)?;
        let mut nonce = [0; 32];
        // SAFETY: fixed initialized output; NONBLOCK preserves the shared deadline.
        let count =
            unsafe { libc::getrandom(nonce.as_mut_ptr().cast(), nonce.len(), libc::GRND_NONBLOCK) };
        if count == -1 {
            let error = rustix::io::Errno::from_io_error(&io::Error::last_os_error())
                .unwrap_or(rustix::io::Errno::IO);
            if error == rustix::io::Errno::INTR {
                continue;
            }
            return Err(ApplicationProofChannelErrorV1::Io(error).into());
        }
        if count != nonce.len() as isize || nonce == [0; 32] {
            return Err(NativeApplicationChannelErrorV1::Binding(
                "fresh native nonce",
            ));
        }
        return Ok(nonce);
    }
}

fn pidfd_error(error: PidfdError) -> ApplicationProofChannelErrorV1 {
    match error {
        PidfdError::Observation(error) => ApplicationProofChannelErrorV1::Process(error),
        PidfdError::Resource(error) => ApplicationProofChannelErrorV1::Resource(error),
    }
}

fn revalidate_processes<'work>(
    processes: &[&NativePidfd<'work>],
    budget: &mut Budget<'work>,
) -> std::result::Result<(), ApplicationProofChannelErrorV1> {
    for process in processes {
        process.revalidate(budget).map_err(pidfd_error)?;
    }
    Ok(())
}

fn send<'work>(
    endpoint: &RetainedApplicationProofEndpointV1,
    processes: &[&NativePidfd<'work>],
    message: &Message,
    deadline: Instant,
    budget: &mut Budget<'work>,
) -> Result<()> {
    transport::send_with_attempt(
        endpoint,
        &[],
        message.canonical_bytes(),
        &[],
        deadline,
        &mut || {
            budget
                .charge_work(ATTEMPT_WORK)
                .map_err(ApplicationProofChannelErrorV1::Resource)?;
            revalidate_processes(processes, budget)
        },
    )?;
    revalidate_processes(processes, budget)?;
    transport::check_deadline(deadline)?;
    Ok(())
}
fn receive<'work>(
    endpoint: &RetainedApplicationProofEndpointV1,
    processes: &[&NativePidfd<'work>],
    deadline: Instant,
    budget: &mut Budget<'work>,
) -> Result<(Message, transport::Received)> {
    let received = transport::receive_with_attempt(
        endpoint,
        &[],
        transport::ReceiveProfile::NativeRegistration,
        deadline,
        &mut || {
            budget
                .charge_work(ATTEMPT_WORK)
                .map_err(ApplicationProofChannelErrorV1::Resource)?;
            revalidate_processes(processes, budget)
        },
    )?;
    revalidate_processes(processes, budget)?;
    transport::check_deadline(deadline)?;
    let (message, charge) = Message::decode(&received.bytes, budget)?;
    budget.reserve_storage(charge.additional_storage())?;
    if received.rights.len() != message.rights() {
        return Err(NativeApplicationChannelErrorV1::Binding(
            "native ancillary rights",
        ));
    }
    Ok((message, received))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

    #[test]
    fn native_attempt_denial_precedes_socket_validation_or_io() {
        let prepared = PreparedApplicationProofChannelV1::prepare().unwrap();
        // Transport-only fixture: not an admitted inherited or registered endpoint.
        // Its deliberate wrong parent must not be inspected after work refusal.
        let endpoint = RetainedApplicationProofEndpointV1 {
            snapshot: EndpointSnapshot::inspect(&prepared.child).unwrap(),
            peer: prepared.child,
            application_pid: std::process::id(),
        };
        let mut work = Work::new(0);
        let mut budget = Budget::new(&mut work, 4096);
        budget.reserve_storage(73).unwrap();
        let deadline = Instant::now() + Duration::from_secs(1);
        let send =
            transport::send_with_attempt(&endpoint, &[], b"native", &[], deadline, &mut || {
                budget
                    .charge_work(1)
                    .map_err(ApplicationProofChannelErrorV1::Resource)
            });
        assert!(matches!(
            send,
            Err(ApplicationProofChannelErrorV1::Resource(_))
        ));
        let receive = transport::receive_with_attempt(
            &endpoint,
            &[],
            transport::ReceiveProfile::NativeRegistration,
            deadline,
            &mut || {
                budget
                    .charge_work(1)
                    .map_err(ApplicationProofChannelErrorV1::Resource)
            },
        );
        assert!(matches!(
            receive,
            Err(ApplicationProofChannelErrorV1::Resource(_))
        ));
        assert_eq!(budget.storage(), 73);
        assert!(budget.failed_work().is_some());
        let mut byte = [0_u8; 1];
        assert_eq!(
            rustix::net::recv(
                &prepared.peer.peer,
                &mut byte,
                rustix::net::RecvFlags::DONTWAIT
            ),
            Err(rustix::io::Errno::AGAIN)
        );
    }

    #[test]
    fn native_pidfd_account_rejection_precedes_endpoint_validation_and_receive() {
        use std::os::fd::FromRawFd;
        let prepared = PreparedApplicationProofChannelV1::prepare().unwrap();
        // Transport-only wrong-parent endpoint, never production-admitted.
        let endpoint = RetainedApplicationProofEndpointV1 {
            snapshot: EndpointSnapshot::inspect(&prepared.child).unwrap(),
            peer: prepared.child,
            application_pid: std::process::id(),
        };
        // SAFETY: pidfd_open returns a new, exclusively owned descriptor.
        let raw = unsafe { libc::syscall(libc::SYS_pidfd_open, std::process::id(), 0) };
        assert!(raw >= 0);
        // SAFETY: only this owner assumes the successful syscall's descriptor.
        let fd = unsafe { OwnedFd::from_raw_fd(raw as i32) };
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        let (owner, charge) =
            NativePidfd::admit_received(fd, std::process::id(), &mut budget).unwrap();
        budget.reserve_storage(charge.additional_storage()).unwrap();
        let mut foreign_work = Work::new(1_000_000);
        let mut foreign = Budget::new(&mut foreign_work, 1_000_000);
        foreign
            .reserve_storage(charge.additional_storage())
            .unwrap();
        assert!(matches!(
            receive(
                &endpoint,
                &[&owner],
                Instant::now() + Duration::from_secs(1),
                &mut foreign
            ),
            Err(NativeApplicationChannelErrorV1::Transport(
                ApplicationProofChannelErrorV1::Resource(Resource::Accounting)
            ))
        ));
        assert_eq!(foreign.work(), ATTEMPT_WORK);
        assert_eq!(foreign.storage(), charge.additional_storage());
        owner.revalidate(&mut budget).unwrap();
    }

    #[test]
    fn fresh_native_nonce_charges_each_attempt_and_never_uses_a_caller_nonce() {
        let mut work = Work::new(0);
        let mut budget = Budget::new(&mut work, 0);
        assert!(matches!(
            fresh_nonce(Instant::now() + Duration::from_secs(1), &mut budget),
            Err(NativeApplicationChannelErrorV1::Resource(_))
        ));
        assert!(budget.failed_work().is_some());
        let mut work = Work::new(128);
        let mut budget = Budget::new(&mut work, 0);
        let first = fresh_nonce(Instant::now() + Duration::from_secs(1), &mut budget).unwrap();
        let second = fresh_nonce(Instant::now() + Duration::from_secs(1), &mut budget).unwrap();
        assert_ne!(first, [0; 32]);
        assert_ne!(first, second);
        assert_eq!(budget.work(), 128);
    }
}
