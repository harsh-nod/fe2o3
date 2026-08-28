//! Protected rustc custody for one exact compiler-execution receipt session.

use std::fmt;
use std::io;
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
    let policy = retain_inherited_policy();
    let policy = match policy {
        Ok(policy) => policy,
        Err(error) => {
            close_service_child_slot();
            return Err(error);
        }
    };
    let client = CompilerExecutionClientV1::admit_inherited_child(RECEIPT_ACQUISITION_TIMEOUT_V1)
        .map_err(ProtectedCompilerExecutionErrorV1::Client)?;
    policy
        .revalidate()
        .map_err(ProtectedCompilerExecutionErrorV1::Policy)?;
    Ok(AdmittedProtectedCompilerExecutionV1 { policy, client })
}

fn retain_inherited_policy()
-> Result<CompilerExecutionPolicyCapabilityV1, ProtectedCompilerExecutionErrorV1> {
    let admission = CompilerExecutionPolicyCapabilityV1::from_inherited_child();
    // The capability retains a private CLOEXEC duplicate on success. Consume the canonical slot
    // on every path so no rejected policy can remain available to later backend code.
    // SAFETY: close consumes only the scalar reserved descriptor and reports absence via EBADF.
    let close_result = unsafe { libc::close(COMPILER_EXECUTION_POLICY_CHILD_FD_V1) };
    match (admission, close_result) {
        (Ok(policy), 0) => Ok(policy),
        (Ok(_), _) => Err(ProtectedCompilerExecutionErrorV1::Descriptor(
            io::Error::last_os_error(),
        )),
        (Err(error), _) => Err(ProtectedCompilerExecutionErrorV1::Policy(error)),
    }
}

fn close_service_child_slot() {
    // SAFETY: this failure cleanup consumes only the scalar reserved descriptor. EBADF is the
    // expected result when the child channel was never installed.
    unsafe { libc::close(COMPILER_EXECUTION_SERVICE_CHILD_FD_V1) };
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
