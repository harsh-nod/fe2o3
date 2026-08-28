#![doc = include_str!("../README.md")]

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
compile_error!("fe2o3-compiler-execution-issuer requires Linux x86-64");

use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};

use fe2o3_broker_authority_service::{
    CompilerExecutionServiceErrorV1, CompilerExecutionServiceExitV1,
    ExpectedClientProcessIdentityV1, LiveClientPidfdIdentityV1,
    ProtectedCompilerExecutionIssuerAdmissionErrorV1, ProtectedCompilerExecutionIssuerAdmissionV1,
    ProtectedCompilerExecutionIssuerErrorV1, ProtectedCompilerExecutionIssuerV1,
    ProtectedIssuerProcessV1, ProtectedServiceAdmissionErrorV1, ProtectedServiceAdmissionV1,
    serve_compiler_execution_v1,
};
use fe2o3_compiler_closure_capability::{
    COMPILER_EXECUTION_SIGNING_KEY_ISSUER_FD_V1, CompilerExecutionPolicyCapabilityV1,
    CompilerExecutionServiceLaunchCapabilityV1, CompilerExecutionSigningKeyCapabilityV1,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SERVICE_READY_BYTES_V1, CompilerExecutionServiceReadyErrorV1,
    CompilerExecutionServiceReadyV1,
};

/// Service-owned durable issuer and Worker-ledger directory.
pub const COMPILER_EXECUTION_ISSUER_ROOT_FD_V1: RawFd = 3;
/// Service endpoint whose peer is the exact live rustc client.
pub const COMPILER_EXECUTION_ISSUER_PEER_FD_V1: RawFd = 4;
/// Process pidfd retaining the exact live rustc client identity.
pub const COMPILER_EXECUTION_ISSUER_CLIENT_PIDFD_V1: RawFd = 5;
/// Immutable caller-pinned issuer policy capability.
pub const COMPILER_EXECUTION_ISSUER_POLICY_FD_V1: RawFd = 6;
/// Service-owned sealed Ed25519 signing-key image.
pub const COMPILER_EXECUTION_ISSUER_SIGNING_KEY_FD_V1: RawFd =
    COMPILER_EXECUTION_SIGNING_KEY_ISSUER_FD_V1;
/// Immutable expected-client and policy launch manifest capability.
pub const COMPILER_EXECUTION_ISSUER_LAUNCH_MANIFEST_FD_V1: RawFd = 8;
/// Nonblocking pipe writer for exact post-recovery readiness publication.
pub const COMPILER_EXECUTION_ISSUER_READY_FD_V1: RawFd = 9;

const PRIVATE_DESCRIPTOR_FLOOR: RawFd = 10;

const _: () = assert!(COMPILER_EXECUTION_ISSUER_ROOT_FD_V1 > libc::STDERR_FILENO);
const _: () = assert!(COMPILER_EXECUTION_ISSUER_ROOT_FD_V1 < COMPILER_EXECUTION_ISSUER_PEER_FD_V1);
const _: () =
    assert!(COMPILER_EXECUTION_ISSUER_PEER_FD_V1 < COMPILER_EXECUTION_ISSUER_CLIENT_PIDFD_V1);
const _: () =
    assert!(COMPILER_EXECUTION_ISSUER_CLIENT_PIDFD_V1 < COMPILER_EXECUTION_ISSUER_POLICY_FD_V1);
const _: () =
    assert!(COMPILER_EXECUTION_ISSUER_POLICY_FD_V1 < COMPILER_EXECUTION_ISSUER_SIGNING_KEY_FD_V1);
const _: () = assert!(
    COMPILER_EXECUTION_ISSUER_SIGNING_KEY_FD_V1 < COMPILER_EXECUTION_ISSUER_LAUNCH_MANIFEST_FD_V1
);
const _: () = assert!(
    COMPILER_EXECUTION_ISSUER_LAUNCH_MANIFEST_FD_V1 < COMPILER_EXECUTION_ISSUER_READY_FD_V1
);
const _: () = assert!(COMPILER_EXECUTION_ISSUER_READY_FD_V1 < 128);

/// Runs one exact protected compiler-execution service occurrence from inherited descriptors.
///
/// This function reads no arguments or environment. The caller must install the seven fixed
/// descriptors through the freestanding static pre-exec launcher. Admission requires a service
/// UID distinct from the rustc client UID, a service-owned mode-0700 root, a service-owned sealed
/// mode-0400 key, the exact sealed-static running image named by the policy, and exact agreement
/// among the launch manifest, service-peer credentials, and client pidfd.
pub fn run_inherited_compiler_execution_issuer_v1()
-> Result<CompilerExecutionServiceExitV1, CompilerExecutionIssuerEntrypointErrorV1> {
    let process = ProtectedIssuerProcessV1::harden()
        .map_err(CompilerExecutionIssuerEntrypointErrorV1::IssuerAdmission)?;

    let policy = CompilerExecutionPolicyCapabilityV1::from_inherited_at(
        COMPILER_EXECUTION_ISSUER_POLICY_FD_V1,
    )
    .map_err(CompilerExecutionIssuerEntrypointErrorV1::PolicyCapability)?;
    close_inherited(COMPILER_EXECUTION_ISSUER_POLICY_FD_V1)?;
    let launch = CompilerExecutionServiceLaunchCapabilityV1::from_inherited_at(
        COMPILER_EXECUTION_ISSUER_LAUNCH_MANIFEST_FD_V1,
    )
    .map_err(CompilerExecutionIssuerEntrypointErrorV1::LaunchCapability)?;
    close_inherited(COMPILER_EXECUTION_ISSUER_LAUNCH_MANIFEST_FD_V1)?;
    if !launch.manifest().matches_policy(policy.policy()) {
        return Err(CompilerExecutionIssuerEntrypointErrorV1::PolicyMismatch);
    }

    let root = take_inherited(COMPILER_EXECUTION_ISSUER_ROOT_FD_V1)?;
    let peer = take_inherited(COMPILER_EXECUTION_ISSUER_PEER_FD_V1)?;
    let client_pidfd = take_inherited(COMPILER_EXECUTION_ISSUER_CLIENT_PIDFD_V1)?;
    let signing_key = take_inherited(COMPILER_EXECUTION_ISSUER_SIGNING_KEY_FD_V1)?;
    let signing_key = CompilerExecutionSigningKeyCapabilityV1::from_file(
        File::from(signing_key),
        policy.policy(),
    )
    .map_err(CompilerExecutionIssuerEntrypointErrorV1::SigningKeyCapability)?;
    let readiness_writer = take_readiness_writer()?;

    let client = launch.manifest().client();
    let expected = ExpectedClientProcessIdentityV1::new(client.pid(), client.uid(), client.gid())
        .map_err(CompilerExecutionIssuerEntrypointErrorV1::ServiceAdmission)?;
    let live_client = LiveClientPidfdIdentityV1::admit(client_pidfd, expected)
        .map_err(CompilerExecutionIssuerEntrypointErrorV1::ServiceAdmission)?;
    let service = ProtectedServiceAdmissionV1::admit(root, peer, live_client)
        .map_err(CompilerExecutionIssuerEntrypointErrorV1::ServiceAdmission)?;
    policy
        .revalidate()
        .map_err(CompilerExecutionIssuerEntrypointErrorV1::PolicyCapability)?;
    launch
        .revalidate()
        .map_err(CompilerExecutionIssuerEntrypointErrorV1::LaunchCapability)?;
    let admission = ProtectedCompilerExecutionIssuerAdmissionV1::admit(
        process,
        service,
        policy.policy().clone(),
        signing_key,
    )
    .map_err(CompilerExecutionIssuerEntrypointErrorV1::IssuerAdmission)?;
    let (issuer, _) = ProtectedCompilerExecutionIssuerV1::admit(admission)
        .map_err(CompilerExecutionIssuerEntrypointErrorV1::Issuer)?;
    // SAFETY: getpid has no pointer arguments and cannot fail for a live process.
    let issuer_pid = u32::try_from(unsafe { libc::getpid() })
        .map_err(|_| CompilerExecutionIssuerEntrypointErrorV1::InvalidIssuerPid)?;
    let readiness =
        CompilerExecutionServiceReadyV1::new(issuer_pid, launch.manifest(), policy.policy())
            .map_err(CompilerExecutionIssuerEntrypointErrorV1::ReadinessProtocol)?;
    publish_readiness(readiness_writer, &readiness)?;
    let result = serve_compiler_execution_v1(issuer)
        .map_err(CompilerExecutionIssuerEntrypointErrorV1::Service);
    let launch_result = launch.revalidate();
    let policy_result = policy.revalidate();
    match (result, launch_result, policy_result) {
        (Ok(exit), Ok(()), Ok(())) => Ok(exit),
        (Err(error), _, _) => Err(error),
        (Ok(_), Err(error), _) => Err(CompilerExecutionIssuerEntrypointErrorV1::LaunchCapability(
            error,
        )),
        (Ok(_), Ok(()), Err(error)) => Err(
            CompilerExecutionIssuerEntrypointErrorV1::PolicyCapability(error),
        ),
    }
}

fn take_readiness_writer() -> Result<OwnedFd, CompilerExecutionIssuerEntrypointErrorV1> {
    admit_readiness_writer(take_inherited(COMPILER_EXECUTION_ISSUER_READY_FD_V1)?)
}

fn admit_readiness_writer(
    writer: OwnedFd,
) -> Result<OwnedFd, CompilerExecutionIssuerEntrypointErrorV1> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: `stat` points to writable storage and `writer` remains open for the call.
    if unsafe { libc::fstat(writer.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
        return Err(
            CompilerExecutionIssuerEntrypointErrorV1::ReadinessDescriptor(
                io::Error::last_os_error(),
            ),
        );
    }
    // SAFETY: fstat initialized `stat` after its successful return.
    let stat = unsafe { stat.assume_init() };
    // SAFETY: F_GETFD consumes only one valid scalar descriptor.
    let descriptor_flags = unsafe { libc::fcntl(writer.as_raw_fd(), libc::F_GETFD) };
    // SAFETY: F_GETFL consumes only one valid scalar descriptor.
    let status = unsafe { libc::fcntl(writer.as_raw_fd(), libc::F_GETFL) };
    // SAFETY: F_GETPIPE_SZ consumes only one valid pipe descriptor.
    let capacity = unsafe { libc::fcntl(writer.as_raw_fd(), libc::F_GETPIPE_SZ) };
    // SAFETY: fpathconf consumes only one valid descriptor and one scalar query name.
    let atomic_write_bytes = unsafe { libc::fpathconf(writer.as_raw_fd(), libc::_PC_PIPE_BUF) };
    let forbidden_status = libc::O_APPEND | libc::O_ASYNC | libc::O_DIRECT;
    if stat.st_mode & libc::S_IFMT != libc::S_IFIFO
        || descriptor_flags < 0
        || descriptor_flags != libc::FD_CLOEXEC
        || status < 0
        || status & libc::O_ACCMODE != libc::O_WRONLY
        || status & libc::O_NONBLOCK == 0
        || status & forbidden_status != 0
        || capacity < COMPILER_EXECUTION_SERVICE_READY_BYTES_V1 as i32
        || atomic_write_bytes < COMPILER_EXECUTION_SERVICE_READY_BYTES_V1 as i64
    {
        return Err(CompilerExecutionIssuerEntrypointErrorV1::InvalidReadinessDescriptor);
    }
    Ok(writer)
}

fn publish_readiness(
    writer: OwnedFd,
    readiness: &CompilerExecutionServiceReadyV1,
) -> Result<(), CompilerExecutionIssuerEntrypointErrorV1> {
    let bytes = readiness.canonical_bytes();
    loop {
        // SAFETY: `bytes` is valid for its exact immutable length and `writer` remains owned.
        let written =
            unsafe { libc::write(writer.as_raw_fd(), bytes.as_ptr().cast(), bytes.len()) };
        if written == bytes.len() as isize {
            return Ok(());
        }
        if written >= 0 {
            return Err(CompilerExecutionIssuerEntrypointErrorV1::ReadinessShortWrite);
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(CompilerExecutionIssuerEntrypointErrorV1::ReadinessDescriptor(error));
        }
    }
}

fn take_inherited(descriptor: RawFd) -> Result<OwnedFd, CompilerExecutionIssuerEntrypointErrorV1> {
    require_inherited(descriptor)?;
    // SAFETY: F_DUPFD_CLOEXEC atomically returns one newly owned descriptor or reports failure.
    let retained =
        unsafe { libc::fcntl(descriptor, libc::F_DUPFD_CLOEXEC, PRIVATE_DESCRIPTOR_FLOOR) };
    if retained < 0 {
        return Err(CompilerExecutionIssuerEntrypointErrorV1::Descriptor(
            io::Error::last_os_error(),
        ));
    }
    // SAFETY: successful duplication returned one newly owned descriptor.
    let retained = unsafe { OwnedFd::from_raw_fd(retained) };
    close_inherited(descriptor)?;
    Ok(retained)
}

fn require_inherited(descriptor: RawFd) -> Result<(), CompilerExecutionIssuerEntrypointErrorV1> {
    // SAFETY: F_GETFD consumes only one scalar descriptor.
    let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFD) };
    if flags < 0 {
        return Err(CompilerExecutionIssuerEntrypointErrorV1::Descriptor(
            io::Error::last_os_error(),
        ));
    }
    if flags & libc::FD_CLOEXEC != 0 {
        return Err(CompilerExecutionIssuerEntrypointErrorV1::UnexpectedCloseOnExec(descriptor));
    }
    Ok(())
}

fn close_inherited(descriptor: RawFd) -> Result<(), CompilerExecutionIssuerEntrypointErrorV1> {
    // SAFETY: every caller owns the inherited fixed descriptor and closes it exactly once.
    if unsafe { libc::close(descriptor) } != 0 {
        return Err(CompilerExecutionIssuerEntrypointErrorV1::Descriptor(
            io::Error::last_os_error(),
        ));
    }
    Ok(())
}

/// Stable protected-issuer entrypoint failure.
#[derive(Debug)]
pub enum CompilerExecutionIssuerEntrypointErrorV1 {
    Descriptor(io::Error),
    UnexpectedCloseOnExec(RawFd),
    PolicyCapability(String),
    LaunchCapability(String),
    SigningKeyCapability(String),
    PolicyMismatch,
    InvalidIssuerPid,
    ReadinessDescriptor(io::Error),
    InvalidReadinessDescriptor,
    ReadinessProtocol(CompilerExecutionServiceReadyErrorV1),
    ReadinessShortWrite,
    ServiceAdmission(ProtectedServiceAdmissionErrorV1),
    IssuerAdmission(ProtectedCompilerExecutionIssuerAdmissionErrorV1),
    Issuer(ProtectedCompilerExecutionIssuerErrorV1),
    Service(CompilerExecutionServiceErrorV1),
}

impl fmt::Display for CompilerExecutionIssuerEntrypointErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Descriptor(error) => {
                write!(formatter, "issuer descriptor admission failed: {error}")
            }
            Self::UnexpectedCloseOnExec(descriptor) => write!(
                formatter,
                "inherited issuer descriptor {descriptor} is unexpectedly close-on-exec"
            ),
            Self::PolicyCapability(error) => {
                write!(formatter, "issuer policy capability failed: {error}")
            }
            Self::LaunchCapability(error) => {
                write!(formatter, "issuer launch capability failed: {error}")
            }
            Self::SigningKeyCapability(error) => {
                write!(formatter, "issuer signing-key capability failed: {error}")
            }
            Self::PolicyMismatch => {
                formatter.write_str("issuer launch manifest names another policy")
            }
            Self::InvalidIssuerPid => formatter.write_str("issuer PID is invalid"),
            Self::ReadinessDescriptor(error) => {
                write!(formatter, "issuer readiness descriptor failed: {error}")
            }
            Self::InvalidReadinessDescriptor => {
                formatter.write_str("issuer readiness descriptor has the wrong shape")
            }
            Self::ReadinessProtocol(error) => {
                write!(formatter, "issuer readiness record failed: {error}")
            }
            Self::ReadinessShortWrite => {
                formatter.write_str("issuer readiness record was only partially written")
            }
            Self::ServiceAdmission(error) => {
                write!(formatter, "issuer service admission failed: {error}")
            }
            Self::IssuerAdmission(error) => {
                write!(formatter, "issuer process admission failed: {error}")
            }
            Self::Issuer(error) => write!(formatter, "issuer durable recovery failed: {error}"),
            Self::Service(error) => write!(formatter, "issuer service failed: {error}"),
        }
    }
}

impl Error for CompilerExecutionIssuerEntrypointErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Descriptor(error) => Some(error),
            Self::ReadinessDescriptor(error) => Some(error),
            Self::ReadinessProtocol(error) => Some(error),
            Self::ServiceAdmission(error) => Some(error),
            Self::IssuerAdmission(error) => Some(error),
            Self::Issuer(error) => Some(error),
            Self::Service(error) => Some(error),
            Self::UnexpectedCloseOnExec(_)
            | Self::PolicyCapability(_)
            | Self::LaunchCapability(_)
            | Self::SigningKeyCapability(_)
            | Self::PolicyMismatch
            | Self::InvalidIssuerPid
            | Self::InvalidReadinessDescriptor
            | Self::ReadinessShortWrite => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use ed25519_dalek::SigningKey;
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionClientProcessIdentityV1, CompilerExecutionExternalAnchorServiceIdentityV1,
        CompilerExecutionIssuerMeasurementV1, CompilerExecutionIssuerPolicyV1,
        CompilerExecutionServiceLaunchManifestV1,
    };

    use super::*;

    fn pipe(flags: i32) -> (OwnedFd, OwnedFd) {
        let mut descriptors = [-1; 2];
        // SAFETY: `descriptors` points to storage for exactly two returned descriptors.
        assert_eq!(unsafe { libc::pipe2(descriptors.as_mut_ptr(), flags) }, 0);
        // SAFETY: successful pipe2 returned two newly owned descriptors.
        unsafe {
            (
                OwnedFd::from_raw_fd(descriptors[0]),
                OwnedFd::from_raw_fd(descriptors[1]),
            )
        }
    }

    fn readiness() -> CompilerExecutionServiceReadyV1 {
        let signing_key = SigningKey::from_bytes(&[7; 32]);
        let policy = CompilerExecutionIssuerPolicyV1::new(
            7,
            CompilerExecutionIssuerMeasurementV1::new([8; 32], 123).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([9; 32], 456).unwrap(),
            signing_key.verifying_key().to_bytes(),
            SigningKey::from_bytes(&[10; 32]).verifying_key().to_bytes(),
        )
        .unwrap();
        let launch = CompilerExecutionServiceLaunchManifestV1::new(
            CompilerExecutionClientProcessIdentityV1::new(1234, 1000, 1001).unwrap(),
            CompilerExecutionExternalAnchorServiceIdentityV1::new(6_000, 7_000).unwrap(),
            &policy,
        );
        CompilerExecutionServiceReadyV1::new(5678, &launch, &policy).unwrap()
    }

    #[test]
    fn fixed_descriptor_contract_is_unique_low_and_static_launcher_compatible() {
        let descriptors = [
            COMPILER_EXECUTION_ISSUER_ROOT_FD_V1,
            COMPILER_EXECUTION_ISSUER_PEER_FD_V1,
            COMPILER_EXECUTION_ISSUER_CLIENT_PIDFD_V1,
            COMPILER_EXECUTION_ISSUER_POLICY_FD_V1,
            COMPILER_EXECUTION_ISSUER_SIGNING_KEY_FD_V1,
            COMPILER_EXECUTION_ISSUER_LAUNCH_MANIFEST_FD_V1,
            COMPILER_EXECUTION_ISSUER_READY_FD_V1,
        ];
        assert!(
            descriptors
                .iter()
                .all(|descriptor| (3..128).contains(descriptor))
        );
        for (index, descriptor) in descriptors.iter().enumerate() {
            assert!(!descriptors[..index].contains(descriptor));
        }
    }

    #[test]
    fn nonblocking_pipe_publishes_one_exact_readiness_record() {
        let (reader, writer) = pipe(libc::O_CLOEXEC | libc::O_NONBLOCK);
        let writer = admit_readiness_writer(writer).unwrap();
        let expected = readiness();
        publish_readiness(writer, &expected).unwrap();

        let mut bytes = [0_u8; COMPILER_EXECUTION_SERVICE_READY_BYTES_V1];
        // SAFETY: `bytes` is writable for its exact length and reader remains owned.
        let count =
            unsafe { libc::read(reader.as_raw_fd(), bytes.as_mut_ptr().cast(), bytes.len()) };
        assert_eq!(count, bytes.len() as isize);
        assert_eq!(
            CompilerExecutionServiceReadyV1::decode(&bytes).unwrap(),
            expected
        );
        let mut trailing = 0_u8;
        // SAFETY: `trailing` is writable for one byte and reader remains owned.
        assert_eq!(
            unsafe {
                libc::read(
                    reader.as_raw_fd(),
                    std::ptr::from_mut(&mut trailing).cast(),
                    1,
                )
            },
            0
        );
    }

    #[test]
    fn hostile_readiness_descriptor_shapes_reject() {
        let (blocking_reader, blocking_writer) = pipe(libc::O_CLOEXEC);
        assert!(admit_readiness_writer(blocking_writer).is_err());
        drop(blocking_reader);

        let (inheritable_reader, inheritable_writer) = pipe(libc::O_NONBLOCK);
        assert!(admit_readiness_writer(inheritable_writer).is_err());
        drop(inheritable_reader);

        let (reader, writer) = pipe(libc::O_CLOEXEC | libc::O_NONBLOCK);
        assert!(admit_readiness_writer(reader).is_err());
        drop(writer);

        let (packet_reader, packet_writer) =
            pipe(libc::O_CLOEXEC | libc::O_NONBLOCK | libc::O_DIRECT);
        assert!(admit_readiness_writer(packet_writer).is_err());
        drop(packet_reader);

        let ordinary = File::open("/dev/null").unwrap();
        assert!(admit_readiness_writer(ordinary.into()).is_err());
    }
}
