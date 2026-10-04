//! Compiler-generated proof execution in the original, unfiltered protected Cargo broker.
//!
//! The transport carries source and process output, never a caller-supplied proof or receipt.
//! Only the compiler-private unsafe admission boundary can turn original delegated descriptors
//! into a runtime lease usable by the existing verifier-owned generated-proof producers.

use std::{
    fs::File,
    io,
    os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd},
    path::Path,
    process::{Child, Command},
    sync::{
        Arc, Mutex, MutexGuard, TryLockError,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use fe2o3_artifact_transaction::{BUILD_ATTEMPT_ENV_V1, BuildAttempt};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_closure_capability::RustcInvocationCapabilityV1;
use fe2o3_process_identity::pidfd::{inspect_pidfd_target, require_pidfd_not_pollable};
use fe2o3_rustc_invocation::{InvocationDigestV3, RustcInvocationDescriptorV3};

use crate::functional_refinement_runtime_v1::{
    FunctionalRefinementRuntimeProcessOutputV1, GeneratedVerusExecutionProfileV1,
};
use crate::retained_functional_refinement_runtime_v1::{
    RetainedGeneratedVerusRuntimeBackendV1, open_retained_generated_verus_runtime_v1,
};
use crate::{CanonicalGeneratedVerusProofInputV3, FunctionalRefinementVerusRuntimeLeaseV1};

mod process;
mod transport;
use process::Peer;
use transport::{
    Credentials, Endpoint, FRAME_TIMEOUT, Frame, MAX_OUTPUT, check_deadline, current_credentials,
    nonce,
};

type Result<T> = io::Result<T>;
fn invalid(message: &'static str) -> io::Error {
    io::Error::other(message)
}
fn require(condition: bool, message: &'static str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(invalid(message))
    }
}

/// Separate from FD195's compiler-currentness protocol and the sealed invocation at FD199.
pub const COMPILER_PROOF_ENDPOINT_CHILD_FD_V1: i32 = 224;
/// Original protected Cargo broker process handle; not a numeric PID reopening route.
pub const COMPILER_PROOF_BROKER_CHILD_FD_V1: i32 = 225;
// Leaves the fixed proof slots below all private duplicates, while remaining compatible with
// the protected launcher's minimum RLIMIT_NOFILE of 243 (fixed authority slots are 240..242).
const DESCRIPTOR_FLOOR: i32 = 226;
const _: () = assert!(
    COMPILER_PROOF_ENDPOINT_CHILD_FD_V1 > 223
        && COMPILER_PROOF_ENDPOINT_CHILD_FD_V1 < COMPILER_PROOF_BROKER_CHILD_FD_V1
        && COMPILER_PROOF_BROKER_CHILD_FD_V1 < DESCRIPTOR_FLOOR
);
const EXECUTE_BODY: usize = 120;

fn invocation_identity(invocation: &RustcInvocationDescriptorV3) -> Result<[u8; 32]> {
    InvocationDigestV3::calculate(invocation)
        .map(|value| value.into_bytes())
        .map_err(io::Error::other)
}

fn context(invocation: [u8; 32], runtime: [u8; 32]) -> Vec<u8> {
    [invocation.as_slice(), runtime.as_slice()].concat()
}

fn lock_until<T>(
    mutex: &Mutex<T>,
    deadline: Instant,
    validate: impl Fn() -> Result<()>,
) -> Result<MutexGuard<'_, T>> {
    loop {
        check_deadline(deadline)?;
        validate()?;
        match mutex.try_lock() {
            Ok(guard) => return Ok(guard),
            Err(TryLockError::Poisoned(_)) => {
                return Err(invalid("compiler-proof owner lock poisoned"));
            }
            Err(TryLockError::WouldBlock) => std::thread::sleep(
                deadline
                    .saturating_duration_since(Instant::now())
                    .min(Duration::from_millis(10)),
            ),
        }
    }
}

struct BrokerInner {
    closure: CompilerClosureV2,
    original: OwnedFd,
    peer: Peer,
    runtime: Mutex<RetainedGeneratedVerusRuntimeBackendV1>,
    runtime_identity: [u8; 32],
    stopped: AtomicBool,
}

impl BrokerInner {
    fn revalidate(&self) -> Result<()> {
        require(
            !self.stopped.load(Ordering::Acquire),
            "compiler-proof broker stopped",
        )?;
        require(
            current_credentials() == self.peer.credentials,
            "compiler-proof broker owner changed",
        )?;
        self.peer.revalidate()
    }
}

/// Concrete broker-side executor. Its public operations never return proof/process output.
///
/// The protected Cargo launcher must retain its release admission while this owner serves.
/// An arbitrary caller opening a server gains no compiler, receipt or launch authority.
pub struct CompilerProofBrokerV1 {
    inner: Arc<BrokerInner>,
}

impl CompilerProofBrokerV1 {
    /// Opens the pinned runtime before installing the Cargo descendant exec filter.
    pub fn open(closure: CompilerClosureV2, root: impl AsRef<Path>) -> Result<Self> {
        crate::authenticated_verus_execution_v2::validate_controller_security_v2()
            .map_err(io::Error::other)?;
        let original = rustix::process::pidfd_open(
            rustix::process::getpid(),
            rustix::process::PidfdFlags::empty(),
        )?;
        let peer = Peer::admit(
            rustix::io::fcntl_dupfd_cloexec(&original, DESCRIPTOR_FLOOR)?,
            current_credentials(),
            closure.cargo_fe2o3_binding_wrapper_sha256(),
        )?;
        let runtime =
            open_retained_generated_verus_runtime_v1(root.as_ref()).map_err(io::Error::other)?;
        let runtime_identity = runtime.identity();
        let value = Self {
            inner: Arc::new(BrokerInner {
                closure,
                original,
                peer,
                runtime: Mutex::new(runtime),
                runtime_identity,
                stopped: AtomicBool::new(false),
            }),
        };
        value.inner.revalidate()?;
        Ok(value)
    }

    /// Checks the original wrapper process and prepares one distinct endpoint.
    /// The Cargo caller must retain authenticated exec-permit custody and bind `attempt` to its
    /// build session; matching process/image records alone do not establish that provenance.
    pub fn prepare(
        &self,
        original_wrapper: OwnedFd,
        wrapper: Credentials,
        original_start_time: u64,
        attempt: BuildAttempt,
        deadline: Instant,
    ) -> Result<(PendingCompilerProofServerV1, CompilerProofBootstrapV1)> {
        check_deadline(deadline)?;
        self.inner.revalidate()?;
        require(
            wrapper.0 != std::process::id()
                && (wrapper.1, wrapper.2)
                    == (self.inner.peer.credentials.1, self.inner.peer.credentials.2),
            "compiler-proof wrapper role mismatch",
        )?;
        let wrapper = Peer::admit(
            original_wrapper,
            wrapper,
            self.inner.closure.cargo_fe2o3_binding_wrapper_sha256(),
        )?;
        wrapper.require_start_time(original_start_time)?;
        let session = nonce()?;
        let (server, client) = transport::pair()?;
        let original = rustix::io::fcntl_dupfd_cloexec(&self.inner.original, DESCRIPTOR_FLOOR)?;
        require_pidfd_not_pollable(&original).map_err(io::Error::other)?;
        self.inner.revalidate()?;
        check_deadline(deadline)?;
        Ok((
            PendingCompilerProofServerV1 {
                inner: Arc::clone(&self.inner),
                wrapper,
                endpoint: Endpoint::admit(server)?,
                session,
                attempt,
                deadline,
            },
            CompilerProofBootstrapV1 {
                session,
                endpoint: client,
                original,
            },
        ))
    }

    /// Stops admission and wakes bounded session polling. Running proof trees observe cancellation.
    pub fn stop(&self) {
        self.inner.stopped.store(true, Ordering::Release);
    }
}

impl Drop for CompilerProofBrokerV1 {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Inert original transport inputs, transferred only after the existing capability response.
pub struct CompilerProofBootstrapV1 {
    session: [u8; 32],
    endpoint: OwnedFd,
    original: OwnedFd,
}
impl CompilerProofBootstrapV1 {
    pub const fn session(&self) -> [u8; 32] {
        self.session
    }
    pub fn into_descriptors(self) -> [OwnedFd; 2] {
        [self.endpoint, self.original]
    }
}

/// Wrapper-owned delegation, not a proof executor or source-to-proof API.
///
/// ```compile_fail
/// fn reuse(delegation: fe2o3_verifier::PendingCompilerProofDelegationV1) {
///     let _ = delegation.spawn(std::process::Command::new("rustc"));
///     let _ = delegation.spawn(std::process::Command::new("rustc"));
/// }
/// ```
pub struct PendingCompilerProofDelegationV1 {
    endpoint: Endpoint,
    broker: Peer,
    original: OwnedFd,
    session: [u8; 32],
    owner: Credentials,
}

impl PendingCompilerProofDelegationV1 {
    /// Checks descriptors from the separately authenticated protected broker preparation response.
    /// This owner cannot create a runtime lease or admit proof output.
    pub fn from_authenticated_transfer(
        session: [u8; 32],
        descriptors: [OwnedFd; 2],
        closure: &CompilerClosureV2,
    ) -> Result<Self> {
        require(session != [0; 32], "zero compiler-proof delegation session")?;
        let [endpoint, original] = descriptors;
        let endpoint =
            Endpoint::admit(rustix::io::fcntl_dupfd_cloexec(endpoint, DESCRIPTOR_FLOOR)?)?;
        let original = rustix::io::fcntl_dupfd_cloexec(original, DESCRIPTOR_FLOOR)?;
        let broker = Peer::admit(
            rustix::io::fcntl_dupfd_cloexec(&original, DESCRIPTOR_FLOOR)?,
            endpoint.creator,
            closure.cargo_fe2o3_binding_wrapper_sha256(),
        )?;
        let value = Self {
            endpoint,
            broker,
            original,
            session,
            owner: current_credentials(),
        };
        value.revalidate()?;
        Ok(value)
    }

    fn revalidate(&self) -> Result<()> {
        require(
            current_credentials() == self.owner,
            "compiler-proof delegation owner changed",
        )?;
        self.broker.revalidate()?;
        self.endpoint.revalidate()
    }

    /// Consumes the command and its captured descriptor aliases after one spawn. The caller must
    /// retain the returned child through delegation and kill/reap it if delegation fails.
    pub fn spawn(self, mut command: Command) -> Result<(Child, SpawnedCompilerProofDelegationV1)> {
        self.revalidate()?;
        let endpoint = rustix::io::fcntl_dupfd_cloexec(&self.endpoint.fd, DESCRIPTOR_FLOOR)?;
        let original = rustix::io::fcntl_dupfd_cloexec(&self.original, DESCRIPTOR_FLOOR)?;
        configure_child_descriptors(&mut command, endpoint, original);
        self.revalidate()?;
        let child = crate::executor::spawn_artifact_coordinated_child(&mut command)?;
        let original_child = capture_original_child(&child);
        drop(command);
        let delegation = SpawnedCompilerProofDelegationV1 {
            original_child,
            delegation: self,
        };
        Ok((child, delegation))
    }
}

/// One spawned compiler. Cannot configure another child or expose its proof endpoint.
///
/// ```compile_fail
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<fe2o3_verifier::SpawnedCompilerProofDelegationV1>();
/// ```
pub struct SpawnedCompilerProofDelegationV1 {
    original_child: Result<OwnedFd>,
    delegation: PendingCompilerProofDelegationV1,
}

fn capture_original_child(child: &Child) -> Result<OwnedFd> {
    let pid = i32::try_from(child.id())
        .ok()
        .and_then(rustix::process::Pid::from_raw)
        .ok_or_else(|| invalid("compiler-proof spawned child PID is invalid"))?;
    // This is the sole opening: the original Child is still owned and has never been waited on.
    rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty()).map_err(Into::into)
}

impl SpawnedCompilerProofDelegationV1 {
    /// Sends the original live compiler pidfd and original invocation capability, then closes the
    /// wrapper's endpoint without reading: only the compiler may receive the activation reply.
    /// A pidfd capture failure is reported here, keeping the original Child with the caller for
    /// the same explicit kill/reap path as every other delegation failure.
    pub fn delegate(self, invocation: File, deadline: Instant) -> Result<()> {
        let delegation = self.delegation;
        delegation.revalidate()?;
        let original_child = self.original_child?;
        let target = inspect_pidfd_target(&original_child)
            .map_err(io::Error::other)?
            .pid;
        let process = fe2o3_process_identity::pidfd::ReceivedProcessPidfdV1::admit_received(
            rustix::io::fcntl_dupfd_cloexec(&original_child, DESCRIPTOR_FLOOR)?,
            target,
        )
        .map_err(io::Error::other)?;
        let invocation =
            RustcInvocationCapabilityV1::from_file(invocation).map_err(io::Error::other)?;
        let child_image = File::open(format!("/proc/{target}/exe"))?;
        require(
            fe2o3_process_identity::measure_executable_sha256_v3(Path::new(&format!(
                "/proc/self/fd/{}",
                child_image.as_raw_fd()
            )))
            .map_err(io::Error::other)?
                == *invocation.descriptor().rustc_executable_sha256(),
            "compiler-proof delegate is not admitted rustc",
        )?;
        // The receiver consumes the same original descriptor; never reconstruct it from a PID.
        let invocation = invocation
            .try_clone_for_transfer()
            .map_err(io::Error::other)?;
        let mut body = Vec::with_capacity(16);
        for value in [target, delegation.owner.1, delegation.owner.2, 0] {
            body.extend_from_slice(&value.to_le_bytes());
        }
        let request = Frame {
            kind: transport::DELEGATE,
            session: delegation.session,
            sequence: 0,
            challenge: [0; 32],
            body,
        };
        delegation.endpoint.send(
            &request,
            &[original_child.as_fd(), invocation.as_fd()],
            deadline,
            || {
                delegation.revalidate()?;
                process.revalidate().map_err(io::Error::other)?;
                require_pidfd_not_pollable(&original_child).map_err(io::Error::other)
            },
        )
    }
}

fn configure_child_descriptors(command: &mut Command, endpoint: OwnedFd, original: OwnedFd) {
    use std::os::unix::process::CommandExt;
    // SAFETY: only async-signal-safe scalar descriptor syscalls run between fork and exec.
    unsafe {
        command.pre_exec(move || {
            for target in [
                COMPILER_PROOF_ENDPOINT_CHILD_FD_V1,
                COMPILER_PROOF_BROKER_CHILD_FD_V1,
            ] {
                if libc::fcntl(target, libc::F_GETFD) >= 0
                    || io::Error::last_os_error().raw_os_error() != Some(libc::EBADF)
                {
                    return Err(io::Error::from_raw_os_error(libc::EBUSY));
                }
            }
            if libc::dup3(endpoint.as_raw_fd(), COMPILER_PROOF_ENDPOINT_CHILD_FD_V1, 0) < 0
                || libc::dup3(original.as_raw_fd(), COMPILER_PROOF_BROKER_CHILD_FD_V1, 0) < 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

/// One original wrapper reservation, consumed by the concrete broker-side proof loop.
pub struct PendingCompilerProofServerV1 {
    inner: Arc<BrokerInner>,
    wrapper: Peer,
    endpoint: Endpoint,
    session: [u8; 32],
    attempt: BuildAttempt,
    deadline: Instant,
}

impl PendingCompilerProofServerV1 {
    fn revalidate(&self) -> Result<()> {
        self.inner.revalidate()?;
        self.wrapper.revalidate()?;
        self.endpoint.revalidate()
    }

    /// Executes only verifier-owned generated-source operations. No process output is returned to
    /// the caller; it is sent exclusively to the delegated original compiler on this endpoint.
    pub fn serve(self, invocation_deadline: Instant) -> Result<()> {
        let (delegation, mut rights) =
            self.endpoint
                .receive(self.wrapper.credentials, self.deadline, || {
                    self.revalidate()
                })?;
        require(
            delegation.kind == transport::DELEGATE
                && delegation.session == self.session
                && delegation.body.len() == 16
                && delegation.body[12..] == [0; 4],
            "invalid original compiler delegation",
        )?;
        let child = (
            u32::from_le_bytes(delegation.body[..4].try_into().unwrap()),
            u32::from_le_bytes(delegation.body[4..8].try_into().unwrap()),
            u32::from_le_bytes(delegation.body[8..12].try_into().unwrap()),
        );
        require(
            child.0 != self.wrapper.credentials.0
                && child.0 != std::process::id()
                && (child.1, child.2) == (self.wrapper.credentials.1, self.wrapper.credentials.2),
            "compiler-proof delegated process role",
        )?;
        let invocation = RustcInvocationCapabilityV1::from_file(File::from(rights.pop().unwrap()))
            .map_err(io::Error::other)?;
        require(
            invocation.descriptor().compiler_closure() == &self.inner.closure,
            "compiler-proof delegated compiler closure mismatch",
        )?;
        require(
            invocation
                .descriptor()
                .compile_environment()
                .entries()
                .iter()
                .any(|entry| {
                    entry.key() == BUILD_ATTEMPT_ENV_V1
                        && entry.value() == self.attempt.to_env_value()
                }),
            "compiler-proof delegated attempt mismatch",
        )?;
        let child = Peer::admit(
            rights.pop().unwrap(),
            child,
            self.inner.closure.rustc_executable_sha256(),
        )?;
        child.require_parent(self.wrapper.credentials.0)?;
        let identity = invocation_identity(invocation.descriptor())?;
        let context = context(identity, self.inner.runtime_identity);
        let validate = || {
            self.revalidate()?;
            invocation.revalidate().map_err(io::Error::other)?;
            child.revalidate()?;
            child.require_parent(self.wrapper.credentials.0)
        };
        let activated = Frame {
            kind: transport::ACTIVATED,
            session: self.session,
            sequence: 0,
            challenge: [0; 32],
            body: context.clone(),
        };
        self.endpoint
            .send(&activated, &[], self.deadline, validate)?;
        let mut sequence = 1_u64;
        loop {
            let (request, mut rights) =
                self.endpoint
                    .receive(child.credentials, invocation_deadline, validate)?;
            require(
                request.session == self.session && request.sequence == sequence,
                "compiler-proof session replay or sequence mismatch",
            )?;
            let kind = request.kind;
            require(
                (sequence == 1 && kind == transport::HELLO)
                    || (sequence > 1 && matches!(kind, transport::EXECUTE | transport::PROBE)),
                "compiler-proof operation phase mismatch",
            )?;
            let response = match kind {
                transport::HELLO | transport::PROBE => {
                    require(
                        request.body == context,
                        "compiler-proof runtime or invocation mismatch",
                    )?;
                    let deadline = (Instant::now() + FRAME_TIMEOUT).min(invocation_deadline);
                    let runtime = lock_until(&self.inner.runtime, deadline, validate)?;
                    runtime.revalidate().map_err(io::Error::other)?;
                    request.reply(
                        if kind == transport::HELLO {
                            transport::HELLO_ACK
                        } else {
                            transport::PROBED
                        },
                        context.clone(),
                    )
                }
                transport::EXECUTE => {
                    require(
                        request.body.len() == EXECUTE_BODY
                            && request.body[..64] == context
                            && request.body[116..] == [0; 4],
                        "compiler-proof execution profile or context mismatch",
                    )?;
                    let length = u64::from_le_bytes(request.body[96..104].try_into().unwrap());
                    let deadline = transport::decode_deadline(u64::from_le_bytes(
                        request.body[104..112].try_into().unwrap(),
                    ))?
                    .min(invocation_deadline);
                    let output_limit =
                        u32::from_le_bytes(request.body[112..116].try_into().unwrap()) as usize;
                    require(
                        output_limit > 0 && output_limit <= MAX_OUTPUT,
                        "compiler-proof output limit",
                    )?;
                    let source = transport::read_source(
                        File::from(rights.pop().unwrap()),
                        child.credentials,
                        length,
                    )?;
                    require(
                        source.identity().as_bytes() == request.body[64..96],
                        "compiler-proof source identity mismatch",
                    )?;
                    let runtime = lock_until(&self.inner.runtime, deadline, validate)?;
                    let output = runtime
                        .execute_generated_rust_verify_cancellable(
                            &source,
                            deadline,
                            output_limit,
                            GeneratedVerusExecutionProfileV1::Ranked,
                            &validate,
                        )
                        .map_err(io::Error::other)?;
                    validate()?;
                    check_deadline(deadline)?;
                    let output = FunctionalRefinementRuntimeProcessOutputV1::from(output);
                    let response =
                        request.reply(transport::EXECUTED, encode_output(&request.body, &output)?);
                    self.endpoint.send(&response, &[], deadline, validate)?;
                    sequence = sequence
                        .checked_add(1)
                        .ok_or_else(|| invalid("compiler-proof sequence exhausted"))?;
                    continue;
                }
                _ => unreachable!(),
            };
            self.endpoint.send(
                &response,
                &[],
                (Instant::now() + FRAME_TIMEOUT).min(invocation_deadline),
                validate,
            )?;
            sequence = sequence
                .checked_add(1)
                .ok_or_else(|| invalid("compiler-proof sequence exhausted"))?;
        }
    }
}

// A failed operation, including unwinding or timing out while another operation holds the
// lock, permanently revokes this endpoint. Never reconnect or admit a late response.
fn poison_endpoint(poisoned: &AtomicBool, endpoint: Option<BorrowedFd<'_>>) {
    poisoned.store(true, Ordering::Release);
    if let Some(endpoint) = endpoint {
        let _ = rustix::net::shutdown(endpoint, rustix::net::Shutdown::Both);
    }
}

fn fail_closed<T>(
    poisoned: &AtomicBool,
    endpoint: Option<BorrowedFd<'_>>,
    operation: impl FnOnce() -> Result<T>,
) -> Result<T> {
    struct Guard<'a> {
        poisoned: &'a AtomicBool,
        endpoint: Option<BorrowedFd<'a>>,
        completed: bool,
    }
    impl Drop for Guard<'_> {
        fn drop(&mut self) {
            if !self.completed {
                poison_endpoint(self.poisoned, self.endpoint);
            }
        }
    }
    let mut guard = Guard {
        poisoned,
        endpoint,
        completed: false,
    };
    require(
        !poisoned.load(Ordering::Acquire),
        "compiler-proof session is poisoned",
    )?;
    let value = operation()?;
    require(
        !poisoned.load(Ordering::Acquire),
        "compiler-proof session was revoked",
    )?;
    guard.completed = true;
    Ok(value)
}

pub(crate) struct AuthenticatedCompilerProofSessionV1 {
    endpoint: Endpoint,
    broker: Peer,
    owner: Credentials,
    session: [u8; 32],
    invocation: [u8; 32],
    runtime: [u8; 32],
    sequence: Mutex<u64>,
    poisoned: AtomicBool,
}

impl AuthenticatedCompilerProofSessionV1 {
    fn revalidate_peer(&self) -> Result<()> {
        require(
            !self.poisoned.load(Ordering::Acquire),
            "compiler-proof session is poisoned",
        )?;
        require(
            current_credentials() == self.owner,
            "compiler-proof runtime crossed process ownership",
        )?;
        self.broker.revalidate()?;
        self.endpoint.revalidate()
    }

    fn exchange<T>(
        &self,
        kind: u8,
        body: Vec<u8>,
        rights: &[BorrowedFd<'_>],
        deadline: Instant,
        decode: impl FnOnce(&Frame) -> Result<T>,
    ) -> Result<T> {
        fail_closed(&self.poisoned, Some(self.endpoint.fd.as_fd()), || {
            let mut sequence = lock_until(&self.sequence, deadline, || self.revalidate_peer())?;
            let request = Frame {
                kind,
                session: self.session,
                sequence: *sequence,
                challenge: nonce()?,
                body,
            };
            self.endpoint
                .send(&request, rights, deadline, || self.revalidate_peer())?;
            let (response, _) = self
                .endpoint
                .receive(self.broker.credentials, deadline, || self.revalidate_peer())?;
            response.require_reply(&request, kind + 1)?;
            require(
                response.body.starts_with(&request.body),
                "compiler-proof response binding changed",
            )?;
            if kind != transport::EXECUTE {
                require(
                    response.body == request.body,
                    "compiler-proof probe body changed",
                )?;
            }
            let value = decode(&response)?;
            self.revalidate_peer()?;
            check_deadline(deadline)?;
            *sequence = sequence
                .checked_add(1)
                .ok_or_else(|| invalid("compiler-proof sequence exhausted"))?;
            Ok(value)
        })
    }

    pub(crate) fn revalidate_until(&self, deadline: Instant) -> Result<()> {
        self.exchange(
            transport::PROBE,
            context(self.invocation, self.runtime),
            &[],
            deadline,
            |_| Ok(()),
        )
    }

    pub(crate) fn poison(&self) {
        poison_endpoint(&self.poisoned, Some(self.endpoint.fd.as_fd()));
    }

    pub(crate) fn execute(
        &self,
        source: &CanonicalGeneratedVerusProofInputV3,
        deadline: Instant,
        output_limit: usize,
        profile: GeneratedVerusExecutionProfileV1,
    ) -> Result<FunctionalRefinementRuntimeProcessOutputV1> {
        fail_closed(&self.poisoned, Some(self.endpoint.fd.as_fd()), || {
            check_deadline(deadline)?;
            require(
                profile == GeneratedVerusExecutionProfileV1::Ranked
                    && output_limit > 0
                    && output_limit <= MAX_OUTPUT,
                "unsupported brokered compiler-proof profile",
            )?;
            let source_file = transport::seal_source(source.source())?;
            let mut body = context(self.invocation, self.runtime);
            body.extend_from_slice(&source.identity().as_bytes());
            body.extend_from_slice(&source.byte_len().to_le_bytes());
            body.extend_from_slice(&transport::encode_deadline(deadline)?.to_le_bytes());
            body.extend_from_slice(&(output_limit as u32).to_le_bytes());
            body.extend_from_slice(&[0; 4]);
            self.exchange(
                transport::EXECUTE,
                body,
                &[source_file.as_fd()],
                deadline,
                |response| decode_output(&response.body, output_limit),
            )
        })
    }
}

fn encode_output(
    request: &[u8],
    output: &FunctionalRefinementRuntimeProcessOutputV1,
) -> Result<Vec<u8>> {
    require(
        request.len() == EXECUTE_BODY
            && output.stdout.len() <= MAX_OUTPUT
            && output.stderr.len() <= MAX_OUTPUT,
        "compiler-proof process output bounds",
    )?;
    let mut bytes = request.to_vec();
    for value in [output.exit_code, output.signal] {
        bytes.extend_from_slice(&u32::from(value.is_some()).to_le_bytes());
        bytes.extend_from_slice(&value.unwrap_or_default().to_le_bytes());
    }
    bytes.extend_from_slice(&(output.stdout.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(output.stderr.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&output.stdout);
    bytes.extend_from_slice(&output.stderr);
    Ok(bytes)
}

fn decode_output(bytes: &[u8], limit: usize) -> Result<FunctionalRefinementRuntimeProcessOutputV1> {
    require(
        bytes.len() >= EXECUTE_BODY + 24,
        "short compiler-proof output",
    )?;
    let option = |start| {
        let tag = u32::from_le_bytes(bytes[start..start + 4].try_into().unwrap());
        let value = i32::from_le_bytes(bytes[start + 4..start + 8].try_into().unwrap());
        match (tag, value) {
            (0, 0) => Ok(None),
            (1, value) => Ok(Some(value)),
            _ => Err(invalid("noncanonical compiler-proof status")),
        }
    };
    let stdout_len = u32::from_le_bytes(bytes[136..140].try_into().unwrap()) as usize;
    let stderr_len = u32::from_le_bytes(bytes[140..144].try_into().unwrap()) as usize;
    require(
        stdout_len <= limit && stderr_len <= limit && bytes.len() == 144 + stdout_len + stderr_len,
        "compiler-proof output framing",
    )?;
    Ok(FunctionalRefinementRuntimeProcessOutputV1 {
        exit_code: option(120)?,
        signal: option(128)?,
        stdout: bytes[144..144 + stdout_len].to_vec(),
        stderr: bytes[144 + stdout_len..].to_vec(),
    })
}

/// Admits only the original protected wrapper-to-rustc proof delegation.
///
/// # Safety
/// The caller must be the compiler-private admission owner of `invocation`, validated against
/// this actual rustc process, its argv/environment/cwd, and its release-admitted compiler closure.
/// FDs224/225 must be exclusively owned original descriptors inherited from that wrapper after its
/// authenticated protected-broker preparation, not caller-reconstructed descriptors or records.
/// A matching public descriptor/hash does not establish this provenance premise.
pub unsafe fn admit_inherited_compiler_proof_runtime_v1(
    invocation: &RustcInvocationDescriptorV3,
    local_runtime: FunctionalRefinementVerusRuntimeLeaseV1,
    deadline: Instant,
) -> Result<FunctionalRefinementVerusRuntimeLeaseV1> {
    let take = |fd| -> Result<OwnedFd> {
        // SAFETY: caller supplies the exclusively owned original inherited descriptors.
        let borrowed = unsafe { BorrowedFd::borrow_raw(fd) };
        require(
            rustix::io::fcntl_getfd(borrowed)? == rustix::io::FdFlags::empty(),
            "compiler-proof input is not original inherited descriptor",
        )?;
        rustix::io::fcntl_setfd(borrowed, rustix::io::FdFlags::CLOEXEC)?;
        // SAFETY: this consumes that exclusive inherited ownership after preventing further exec inheritance.
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    };
    let endpoint = Endpoint::admit(take(COMPILER_PROOF_ENDPOINT_CHILD_FD_V1)?)?;
    let broker = Peer::admit(
        take(COMPILER_PROOF_BROKER_CHILD_FD_V1)?,
        endpoint.creator,
        invocation
            .compiler_closure()
            .cargo_fe2o3_binding_wrapper_sha256(),
    )?;
    require(
        endpoint.creator.0 != std::process::id()
            && (endpoint.creator.1, endpoint.creator.2)
                == (current_credentials().1, current_credentials().2),
        "compiler-proof broker role mismatch",
    )?;
    let identity = invocation_identity(invocation)?;
    local_runtime.revalidate().map_err(io::Error::other)?;
    let runtime = local_runtime.identity().as_bytes();
    let (activated, _) = endpoint.receive(broker.credentials, deadline, || broker.revalidate())?;
    require(
        activated.kind == transport::ACTIVATED && activated.body == context(identity, runtime),
        "compiler-proof activation binding mismatch",
    )?;
    let session = AuthenticatedCompilerProofSessionV1 {
        endpoint,
        broker,
        owner: current_credentials(),
        session: activated.session,
        invocation: identity,
        runtime,
        sequence: Mutex::new(1),
        poisoned: AtomicBool::new(false),
    };
    session.exchange(
        transport::HELLO,
        context(identity, runtime),
        &[],
        deadline,
        |_| Ok(()),
    )?;
    local_runtime
        .with_compiler_broker(session, deadline)
        .map_err(io::Error::other)
}

#[cfg(test)]
mod tests;
