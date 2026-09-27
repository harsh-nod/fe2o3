//! Protected rustc custody for one exact compiler-execution receipt session.

#[path = "protected_compiler_execution_native_v3.rs"]
pub(crate) mod native_v3;

use std::fmt;
use std::io;
use std::os::fd::RawFd;
use std::time::Duration;

use fe2o3_compiler_closure_capability::{
    COMPILER_EXECUTION_POLICY_CHILD_FD_V1, CompilerExecutionPolicyCapabilityV1,
};
use fe2o3_compiler_execution_client::{
    COMPILER_EXECUTION_SERVICE_CHILD_FD_V1, CompilerExecutionClientErrorV1,
    CompilerExecutionClientV1,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V1, CompilerExecutionReceiptCarriageV1,
    CompilerExecutionReceiptPublicationErrorV1,
};

const RECEIPT_ACQUISITION_TIMEOUT_V1: Duration = Duration::from_secs(120);
const _: () = assert!(
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V1
        <= fe2o3_artifact_transaction::MAX_COMPILER_EXECUTION_RECEIPT_TRANSPORT_BYTES_V1
);

/// Move-only custody of the exact sealed issuer policy and child-created service peer.
pub(crate) struct AdmittedProtectedCompilerExecutionV1 {
    policy: CompilerExecutionPolicyCapabilityV1,
    client: CompilerExecutionClientV1,
}

impl AdmittedProtectedCompilerExecutionV1 {
    /// Acquires and independently revalidates the receipt for one exact published subject.
    pub(crate) fn acquire(
        self,
        subject: fe2o3_artifact_transaction::InertCompilerExecutionSubjectV1,
    ) -> Result<CompilerExecutionReceiptCarriageV1, ProtectedCompilerExecutionErrorV1> {
        let Self { policy, client } = self;
        policy
            .revalidate()
            .map_err(ProtectedCompilerExecutionErrorV1::Policy)?;
        let carriage = client
            .acquire(policy.policy(), subject.clone())
            .map_err(ProtectedCompilerExecutionErrorV1::Client)?;
        policy
            .revalidate()
            .map_err(ProtectedCompilerExecutionErrorV1::Policy)?;
        let decoded = CompilerExecutionReceiptCarriageV1::decode(carriage.canonical_bytes())
            .map_err(ProtectedCompilerExecutionErrorV1::Carriage)?;
        if decoded != carriage
            || decoded.policy() != policy.policy()
            || decoded.request().subject() != &subject
        {
            return Err(ProtectedCompilerExecutionErrorV1::BindingMismatch);
        }
        Ok(carriage)
    }
}

/// Admits both canonical compiler-execution descriptors without a fallback path.
pub(crate) fn admit_for_production_codegen()
-> Result<AdmittedProtectedCompilerExecutionV1, ProtectedCompilerExecutionErrorV1> {
    let mut slots = InheritedExecutionSlots::new();
    slots
        .validate()
        .map_err(ProtectedCompilerExecutionErrorV1::Descriptor)?;
    let policy = CompilerExecutionPolicyCapabilityV1::from_inherited_child()
        .map_err(ProtectedCompilerExecutionErrorV1::Policy)?;
    slots
        .close_policy()
        .map_err(ProtectedCompilerExecutionErrorV1::Descriptor)?;
    // The client consumes this fixed slot on every admission exit.
    slots.service = None;
    let client = CompilerExecutionClientV1::admit_inherited_child(RECEIPT_ACQUISITION_TIMEOUT_V1)
        .map_err(ProtectedCompilerExecutionErrorV1::Client)?;
    policy
        .revalidate()
        .map_err(ProtectedCompilerExecutionErrorV1::Policy)?;
    Ok(AdmittedProtectedCompilerExecutionV1 { policy, client })
}

// Admission exclusively consumes the protocol slots. Never fabricate OwnedFd
// for a possibly missing input, and never duplicate until both are occupied:
// a policy duplicate in an absent service slot would otherwise get two closers.
struct InheritedExecutionSlots {
    policy: Option<RawFd>,
    service: Option<RawFd>,
}

impl InheritedExecutionSlots {
    fn new() -> Self {
        Self {
            policy: Some(COMPILER_EXECUTION_POLICY_CHILD_FD_V1),
            service: Some(COMPILER_EXECUTION_SERVICE_CHILD_FD_V1),
        }
    }

    fn validate(&self) -> io::Result<()> {
        for fd in [
            COMPILER_EXECUTION_POLICY_CHILD_FD_V1,
            COMPILER_EXECUTION_SERVICE_CHILD_FD_V1,
        ] {
            // SAFETY: F_GETFD observes the exclusively held admission slots.
            let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
            if flags < 0 {
                return Err(io::Error::last_os_error());
            }
            if flags & libc::FD_CLOEXEC != 0 {
                return Err(io::Error::from_raw_os_error(libc::EINVAL));
            }
        }
        Ok(())
    }

    fn close_policy(&mut self) -> io::Result<()> {
        close_slot(self.policy.take().expect("policy slot has one closer"))
    }
}

impl Drop for InheritedExecutionSlots {
    fn drop(&mut self) {
        for fd in [self.policy.take(), self.service.take()]
            .into_iter()
            .flatten()
        {
            let _ = close_slot(fd);
        }
    }
}

fn close_slot(fd: RawFd) -> io::Result<()> {
    // SAFETY: admission owns this protocol slot. Never retry close: even an
    // error must not close a descriptor newly allocated at the same number.
    if unsafe { libc::close(fd) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[derive(Debug)]
pub(crate) enum ProtectedCompilerExecutionErrorV1 {
    Policy(String),
    Client(CompilerExecutionClientErrorV1),
    Carriage(CompilerExecutionReceiptPublicationErrorV1),
    Descriptor(io::Error),
    BindingMismatch,
}

impl fmt::Display for ProtectedCompilerExecutionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Policy(error) => write!(
                formatter,
                "cannot admit or revalidate inherited compiler-execution policy: {error}"
            ),
            Self::Client(error) => write!(formatter, "compiler-execution client failed: {error}"),
            Self::Carriage(error) => write!(
                formatter,
                "compiler-execution receipt carriage is not canonical: {error}"
            ),
            Self::Descriptor(error) => write!(
                formatter,
                "cannot consume inherited compiler-execution descriptor: {error}"
            ),
            Self::BindingMismatch => formatter.write_str(
                "compiler-execution receipt changed its exact subject or sealed issuer policy",
            ),
        }
    }
}

impl std::error::Error for ProtectedCompilerExecutionErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Client(error) => Some(error),
            Self::Carriage(error) => Some(error),
            Self::Descriptor(error) => Some(error),
            Self::Policy(_) | Self::BindingMismatch => None,
        }
    }
}

#[cfg(test)]
#[path = "protected_compiler_execution_tests.rs"]
mod tests;
