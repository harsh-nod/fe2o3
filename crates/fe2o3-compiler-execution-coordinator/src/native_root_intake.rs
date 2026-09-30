//! One inert transcript on the original root account, not compiler authority.
use super::PreparedCompilerExecutionSupervisorV3 as Prepared;
use crate::compiler_output_directory::{CompilerOutputDirectory as Output, Error as OutputError};
use crate::native_inherited::{
    self as root, CompilerExecutionRootDeploymentErrorV2 as Error, Result,
};
use fe2o3_compiler_closure_capability::RustcInvocationCapabilityV1 as Invocation;
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_ROOT_INTAKE_BYTES_V4 as N,
    COMPILER_EXECUTION_ROOT_INTAKE_STORAGE_V4 as RECORD_SCRATCH,
    COMPILER_EXECUTION_ROOT_INTAKE_WORK_V4 as RECORD_WORK,
    CompilerExecutionRootIntakeErrorV4 as RecordError, CompilerExecutionRootIntakeKindV4 as Kind,
    CompilerExecutionRootIntakeRecordV4 as Record, CompilerExecutionRootIntakeRoleV4 as Role,
};
use fe2o3_compiler_execution_supervisor::ProvisionedProtectedIssuerServiceInputsV2 as Inputs;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_protected_service_spawn::launch_io::{self as io, MessageSender};
use fe2o3_rustc_invocation::{InvocationDigestV3, MAX_DESCRIPTOR_BYTES_V3};
use rustix::{fs, net};
use std::{
    fs::File,
    mem::size_of,
    os::fd::{AsFd, OwnedFd},
    time::{Duration, Instant},
};

const LOCAL_WORK: usize = 8 + 64 * 1024;
const FRAME: usize = 4 * size_of::<Receiver>() + 8192;
const DIGEST_WORK: usize = 4096 * MAX_DESCRIPTOR_BYTES_V3;
const DIGEST_SCRATCH: usize = Invocation::NATIVE_OPERATION_SCRATCH;
// Final refusal probes for trailing input before sending its zero-right ACK.
const EXCHANGE_WORK: usize = 2 * io::packet_receive_work(N);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Accept,
    Hello,
    Challenge,
    Input(usize),
    Ack,
    Refused,
    Failed,
}

/// The outer Native owns this before any accept/receive. A single maximum
/// reservation covers all future owners; it is never rebased between frames.
/// Partial custody remains installed on every refusal/unwind until Native drops.
/// No compiler exists here, so none of these transport FDs replaces a Trace or
/// an independently funded compiler cleanup dependency.
pub(crate) struct Receiver {
    phase: Phase,
    connection: Option<OwnedFd>,
    sender: Option<MessageSender>,
    deadline: Option<Instant>,
    hello: Option<Record>,
    challenge: Option<Record>,
    last: Option<Record>,
    ack: Option<Record>,
    files: [Option<OwnedFd>; 6],
    invocation: Option<Invocation>,
    output: Option<Output>,
}

impl Receiver {
    // Includes the consumed invocation FD as well as its complete decoded owner.
    // Deliberately keep excess reserved until final Drop, including on errors.
    pub(crate) const STORAGE: usize = size_of::<Self>()
        + Invocation::NATIVE_MAX_RETAINED_STORAGE
        + Inputs::CONNECTION_STORAGE
        + 6 * Invocation::NATIVE_FILE_STORAGE
        + Output::STORAGE
        + 8 * N;
    pub(crate) const TURN_WORK: usize = LOCAL_WORK
        + LOCAL_WORK
        + Inputs::WORK
        + EXCHANGE_WORK
        + 4 * RECORD_WORK
        + Invocation::NATIVE_ADMISSION_WORK
        + Invocation::NATIVE_REVALIDATION_WORK
        + DIGEST_WORK
        + 2 * Output::WORK;
    pub(crate) const SCRATCH: usize = FRAME
        + Inputs::SCRATCH
        + Inputs::CONNECTION_STORAGE
        + io::packet_receive_scratch(N)
        + RECORD_SCRATCH
        + Invocation::NATIVE_OPERATION_SCRATCH
        + Invocation::NATIVE_FILE_STORAGE
        + DIGEST_SCRATCH
        + Output::SCRATCH
        + Output::STORAGE;

    pub(crate) fn empty() -> Self {
        Self {
            phase: Phase::Accept,
            connection: None,
            sender: None,
            deadline: None,
            hello: None,
            challenge: None,
            last: None,
            ack: None,
            files: std::array::from_fn(|_| None),
            invocation: None,
            output: None,
        }
    }

    pub(crate) fn activate(&self, prepared: &mut Prepared, b: &mut Budget<'_>) -> Result<()> {
        let floor = root::sum(&[Self::STORAGE, prepared.retained_storage()])?;
        b.with_prepaid_scope(floor, 8, LOCAL_WORK, FRAME, |b| {
            if self.phase != Phase::Accept || self.connection.is_some() {
                return Err(rejected("root listener already used"));
            }
            Ok(prepared.service_inputs.activate_original_root_listener(b)?)
        })
    }

    /// One nonblocking attempt. Native validates actual Prepared before each
    /// turn, including the final ACK turn. True means only terminal refusal.
    pub(crate) fn step(&mut self, prepared: &mut Prepared, b: &mut Budget<'_>) -> Result<bool> {
        let floor = root::sum(&[Self::STORAGE, prepared.retained_storage()])?;
        let result = b.with_prepaid_scope(floor, 8, LOCAL_WORK, FRAME, |b| {
            if matches!(self.phase, Phase::Refused | Phase::Failed) {
                return Err(rejected("intake cannot be reused"));
            }
            if self
                .deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
            {
                return Err(rejected("intake deadline exceeded"));
            }
            if self.phase == Phase::Accept {
                if let Some((connection, _)) =
                    prepared.service_inputs.try_accept_original_root(b)?
                {
                    // Install before inspecting anything fallible about the peer.
                    self.connection = Some(connection);
                    let peer = net::sockopt::socket_peercred(self.connection.as_ref().unwrap())
                        .map_err(|source| Error::Io {
                            operation: "root intake peer",
                            source,
                        })?;
                    self.sender = Some(MessageSender::new(
                        peer.pid.as_raw_pid(),
                        peer.uid.as_raw(),
                        peer.gid.as_raw(),
                    ));
                    self.deadline = Some(
                        Instant::now()
                            .checked_add(Duration::from_secs(120))
                            .ok_or_else(|| rejected("intake deadline overflow"))?,
                    );
                    self.phase = Phase::Hello;
                }
                return Ok(false);
            }
            self.exchange(prepared.trust.policy().policy().identity().as_bytes(), b)
        });
        if result.is_err() {
            self.phase = Phase::Failed;
        }
        result
    }

    // Same packet engine for real Prepared and inert socket mechanics tests.
    // The caller holds FRAME and the complete receiver owner reservation. The
    // policy bytes select an inert association, never admit a Prepared/Trace.
    fn exchange(&mut self, policy: &[u8; 32], b: &mut Budget<'_>) -> Result<bool> {
        let result = (|| {
            let connection = self
                .connection
                .as_ref()
                .ok_or_else(|| rejected("missing connection"))?;
            let sender = self
                .sender
                .ok_or_else(|| rejected("missing peer credentials"))?;
            b.with_prepaid_scope(0, 8, EXCHANGE_WORK, io::packet_receive_scratch(N), |b| {
                match self.phase {
                    Phase::Hello => {
                        let Some(bytes) =
                            io::receive_authenticated_packet::<N>(connection.as_fd(), sender)
                                .map_err(transport)?
                        else {
                            return Ok(false);
                        };
                        let (hello, _) = Record::decode(&bytes, b).map_err(record)?;
                        self.hello = Some(hello);
                        let hello = self.hello.as_ref().unwrap();
                        if hello.kind() != Kind::Hello || hello.policy_identity() != policy {
                            return Err(rejected("hello differs from original V3 policy"));
                        }
                        let mut nonce = [0; 32];
                        let count = rustix::rand::getrandom(
                            &mut nonce,
                            rustix::rand::GetRandomFlags::NONBLOCK,
                        )
                        .map_err(|source| Error::Io {
                            operation: "root intake challenge",
                            source,
                        })?;
                        if count != nonce.len() {
                            return Err(rejected("short root challenge"));
                        }
                        self.challenge =
                            Some(Record::challenge(hello, nonce, b).map_err(record)?.0);
                        self.phase = Phase::Challenge;
                    }
                    Phase::Challenge => {
                        if io::send_packet(
                            connection.as_fd(),
                            self.challenge.as_ref().unwrap().canonical_bytes(),
                        )
                        .map_err(transport)?
                        .is_some()
                        {
                            self.phase = Phase::Input(0);
                        }
                    }
                    Phase::Input(index) => {
                        let Some((bytes, fd)) =
                            io::receive_authenticated_descriptor::<N>(connection.as_fd(), sender)
                                .map_err(transport)?
                        else {
                            return Ok(false);
                        };
                        // Install every received right before decoding or inspecting it.
                        self.files[index] = Some(fd);
                        let (input, _) = Record::decode(&bytes, b).map_err(record)?;
                        let challenge = self.challenge.as_ref().unwrap();
                        if input.kind() != Kind::Input
                            || input.role() != challenge.roles().nth(index)
                            || !input.matches_predecessor(challenge, b).map_err(record)?
                        {
                            return Err(rejected("input ordering or challenge mismatch"));
                        }
                        let fd = self.files[index].as_ref().unwrap();
                        check_descriptor(fd)?;
                        match input.role().unwrap() {
                            Role::Invocation => {
                                let stat = fs::fstat(fd).map_err(|source| Error::Io {
                                    operation: "intake invocation length",
                                    source,
                                })?;
                                if stat.st_size <= 0
                                    || stat.st_size as u64 != input.invocation_bytes()
                                {
                                    return Err(rejected(
                                        "actual invocation length differs before read",
                                    ));
                                }
                                // Admission consumes its input on failure. Keep
                                // the original received right installed through
                                // cancellation; only the separately funded inert
                                // duplicate is consumed by the shared decoder.
                                let invocation = b.with_prepaid_scope(
                                    Self::STORAGE,
                                    8,
                                    LOCAL_WORK,
                                    Invocation::NATIVE_FILE_STORAGE,
                                    |b| {
                                        let copy = rustix::io::fcntl_dupfd_cloexec(fd, 0).map_err(
                                            |source| Error::Io {
                                                operation: "duplicate intake invocation",
                                                source,
                                            },
                                        )?;
                                        Ok::<_, Error>(
                                            Invocation::from_file_native(File::from(copy), b)?.0,
                                        )
                                    },
                                )?;
                                self.invocation = Some(invocation);
                                let invocation = self.invocation.as_ref().unwrap();
                                let digest = b.with_prepaid_scope(
                                    invocation.native_retained_storage()?,
                                    8,
                                    DIGEST_WORK,
                                    DIGEST_SCRATCH,
                                    |_| {
                                        InvocationDigestV3::calculate(invocation.descriptor())
                                            .map_err(|_| rejected("invalid invocation digest"))
                                    },
                                )?;
                                if digest.as_bytes() != input.invocation_identity() {
                                    return Err(rejected("actual invocation differs from hello"));
                                }
                            }
                            Role::WorkingDirectory => {
                                let stat = fs::fstat(fd).map_err(|source| Error::Io {
                                    operation: "intake cwd",
                                    source,
                                })?;
                                if fs::FileType::from_raw_mode(stat.st_mode)
                                    != fs::FileType::Directory
                                {
                                    return Err(rejected("cwd is not a directory"));
                                }
                                // Shape is not an approved source/cwd mapping. No
                                // compiler may start on this observation alone.
                            }
                            Role::Stdin | Role::Stdout | Role::Stderr => {}
                            Role::OutputDirectory => {
                                let invocation = self
                                    .invocation
                                    .as_ref()
                                    .ok_or_else(|| rejected("output preceded invocation"))?;
                                // A funded duplicate becomes a typed inert preparation
                                // input. The original right remains installed on every
                                // error, including constructor failure and outer unwind.
                                self.output = Some(
                                    Output::capture(
                                        fd.as_fd(),
                                        input.output_identity(),
                                        invocation.descriptor().artifact_output_directory(),
                                        b,
                                    )
                                    .map_err(output_error)?,
                                );
                            }
                        }
                        let last = index + 1 == challenge.roles().count();
                        self.last = Some(input);
                        if last {
                            self.invocation.as_ref().unwrap().revalidate_native(b)?;
                            Self::revalidate_output(
                                self.output.as_ref(),
                                self.invocation.as_ref(),
                                b,
                            )?;
                            self.ack = Some(
                                Record::enforcement_unavailable(self.last.as_ref().unwrap(), b)
                                    .map_err(record)?
                                    .0,
                            );
                            self.phase = Phase::Ack;
                        } else {
                            self.phase = Phase::Input(index + 1);
                        }
                    }
                    Phase::Ack => {
                        Self::revalidate_output(self.output.as_ref(), self.invocation.as_ref(), b)?;
                        if io::receive_authenticated_packet::<N>(connection.as_fd(), sender)
                            .map_err(transport)?
                            .is_some()
                        {
                            return Err(rejected("trailing input before refusal ACK"));
                        }
                        if io::send_packet(
                            connection.as_fd(),
                            self.ack.as_ref().unwrap().canonical_bytes(),
                        )
                        .map_err(transport)?
                        .is_some()
                        {
                            self.phase = Phase::Refused;
                            return Ok(true);
                        }
                    }
                    Phase::Accept | Phase::Refused | Phase::Failed => {
                        return Err(rejected("intake cannot be reused"));
                    }
                }
                Ok(false)
            })
        })();
        if result.is_err() {
            self.phase = Phase::Failed;
        }
        result
    }

    fn revalidate_output(
        output: Option<&Output>,
        invocation: Option<&Invocation>,
        b: &mut Budget<'_>,
    ) -> Result<()> {
        let output = output.ok_or_else(|| rejected("missing original output directory"))?;
        let invocation = invocation.ok_or_else(|| rejected("missing original invocation"))?;
        output
            .revalidate(invocation.descriptor().artifact_output_directory(), b)
            .map_err(output_error)
    }
}

fn output_error(error: OutputError) -> Error {
    match error {
        OutputError::Resource(e) => e.into(),
        OutputError::Io { operation, source } => Error::Io { operation, source },
        OutputError::Invalid(message) => rejected(message),
    }
}

fn check_descriptor(fd: &OwnedFd) -> Result<()> {
    let flags = rustix::io::fcntl_getfd(fd).map_err(|source| Error::Io {
        operation: "intake CLOEXEC",
        source,
    })?;
    if !flags.contains(rustix::io::FdFlags::CLOEXEC) {
        return Err(rejected("input is not CLOEXEC"));
    }
    Ok(())
}
fn rejected(reason: &'static str) -> Error {
    root::invalid("root intake", reason)
}
fn record(error: RecordError) -> Error {
    match error {
        RecordError::Resource(error) => error.into(),
        RecordError::Framing(reason) => rejected(reason),
    }
}
fn transport(error: io::Failure) -> Error {
    match error {
        io::Failure::Io { operation, source } => Error::Io { operation, source },
        _ => rejected("authenticated intake transport refused"),
    }
}

#[cfg(test)]
#[path = "native_root_intake_tests.rs"]
mod tests;
