#![doc = include_str!("../README.md")]

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
compile_error!("fe2o3-compiler-execution-issuer requires Linux x86-64");

use std::error::Error;
use std::fmt;
use std::io;
use std::os::fd::{FromRawFd, OwnedFd, RawFd};

use fe2o3_broker_authority_service::{
    CompilerExecutionServiceErrorV1, CompilerExecutionServiceExitV1,
    ExpectedClientProcessIdentityV1, LiveClientPidfdIdentityV1,
    ProtectedCompilerExecutionIssuerAdmissionErrorV1, ProtectedCompilerExecutionIssuerAdmissionV1,
    ProtectedCompilerExecutionIssuerErrorV1, ProtectedCompilerExecutionIssuerV1,
    ProtectedIssuerProcessV1, ProtectedServiceAdmissionErrorV1, ProtectedServiceAdmissionV1,
    serve_compiler_execution_v1,
};
use fe2o3_compiler_closure_capability::{
    CompilerExecutionPolicyCapabilityV1, CompilerExecutionServiceLaunchCapabilityV1,
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
pub const COMPILER_EXECUTION_ISSUER_SIGNING_KEY_FD_V1: RawFd = 7;
/// Immutable expected-client and policy launch manifest capability.
pub const COMPILER_EXECUTION_ISSUER_LAUNCH_MANIFEST_FD_V1: RawFd = 8;

const PRIVATE_DESCRIPTOR_FLOOR: RawFd = 9;

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
const _: () = assert!(COMPILER_EXECUTION_ISSUER_LAUNCH_MANIFEST_FD_V1 < 128);

/// Runs one exact protected compiler-execution service occurrence from inherited descriptors.
///
/// This function reads no arguments or environment. The caller must install the six fixed
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
    PolicyMismatch,
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
            Self::PolicyMismatch => {
                formatter.write_str("issuer launch manifest names another policy")
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
            Self::ServiceAdmission(error) => Some(error),
            Self::IssuerAdmission(error) => Some(error),
            Self::Issuer(error) => Some(error),
            Self::Service(error) => Some(error),
            Self::UnexpectedCloseOnExec(_)
            | Self::PolicyCapability(_)
            | Self::LaunchCapability(_)
            | Self::PolicyMismatch => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_descriptor_contract_is_unique_low_and_static_launcher_compatible() {
        let descriptors = [
            COMPILER_EXECUTION_ISSUER_ROOT_FD_V1,
            COMPILER_EXECUTION_ISSUER_PEER_FD_V1,
            COMPILER_EXECUTION_ISSUER_CLIENT_PIDFD_V1,
            COMPILER_EXECUTION_ISSUER_POLICY_FD_V1,
            COMPILER_EXECUTION_ISSUER_SIGNING_KEY_FD_V1,
            COMPILER_EXECUTION_ISSUER_LAUNCH_MANIFEST_FD_V1,
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
}
