//! Worker V3 authority join for the sole direct-KFD production transition.

use core::{fmt, time::Duration};

use fe2o3_kfd::{CheckedGfx942XnackMinusDevice, execute_gfx942_kfd_dispatch_unchecked_v1};

use crate::{
    Gfx942RuntimeBufferAccessV1, Gfx942RuntimePreparedBufferPolicyV1,
    PreparedGfx942RuntimeDispatchV1,
};

/// Authenticated, invocation-specific Worker V3 authority for one gfx942 dispatch.
///
/// The trait transports authority established elsewhere; it does not define another verifier or
/// permit the runtime to infer authority from descriptive hashes. The safe runtime transition
/// independently compares every returned identity to its private prepared request and checked KFD
/// device before entering native mechanics.
///
/// Safe application code cannot implement this boundary:
///
/// ```compile_fail
/// use fe2o3_runtime::WorkerV3Gfx942ExecutionAuthorityV1;
///
/// struct ForgedAuthority;
///
/// impl WorkerV3Gfx942ExecutionAuthorityV1 for ForgedAuthority {
///     type CurrentnessError = core::convert::Infallible;
///
///     fn finalized_hsaco_sha256(&self) -> [u8; 32] { [0; 32] }
///     fn finalized_hsaco_length(&self) -> u64 { 0 }
///     fn kernel_name(&self) -> &str { "forged" }
///     fn dispatch_contract_sha256(&self) -> [u8; 32] { [0; 32] }
///     fn device_unique_id(&self) -> u64 { 0 }
///     fn revalidate_currentness(&self) -> Result<(), Self::CurrentnessError> { Ok(()) }
/// }
/// ```
///
/// # Safety
///
/// Implementations must be emitted only after one reviewed Worker V3 verifier has authenticated
/// the exact compiler lineage, finalized artifact, generated Rust ABI and effect contract,
/// machine effects, proof-to-executable binding, invocation arguments, launch geometry, alias and
/// race discipline, bounds, initialization, and completion quiescence represented by the returned
/// dispatch-contract identity. `device_unique_id` must identify the exact KFD device covered by
/// those checks. `revalidate_currentness` must retain and recheck the same publication and evidence
/// custody through the call. A false implementation can make safe code execute unauthorised native
/// GPU memory accesses.
pub unsafe trait WorkerV3Gfx942ExecutionAuthorityV1 {
    type CurrentnessError;

    fn finalized_hsaco_sha256(&self) -> [u8; 32];

    fn finalized_hsaco_length(&self) -> u64;

    fn kernel_name(&self) -> &str;

    fn dispatch_contract_sha256(&self) -> [u8; 32];

    fn device_unique_id(&self) -> u64;

    fn revalidate_currentness(&self) -> Result<(), Self::CurrentnessError>;
}

/// Fail-closed rejection before native mutation or after confirmed completion and teardown.
#[derive(Debug)]
#[non_exhaustive]
pub enum Gfx942AuthorizedRuntimeExecutionErrorV1<E> {
    CurrentnessBeforeDispatch(E),
    CurrentnessAfterCompletion(E),
    ArtifactIdentityMismatch,
    ArtifactLengthMismatch,
    KernelNameMismatch,
    DispatchContractMismatch,
    DeviceIdentityMismatch,
    CompletedBufferCardinalityMismatch,
    ReadOnlyBufferModified { index: usize },
}

impl<E: fmt::Display> fmt::Display for Gfx942AuthorizedRuntimeExecutionErrorV1<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentnessBeforeDispatch(error) => {
                write!(
                    formatter,
                    "Worker V3 currentness failed before dispatch: {error}"
                )
            }
            Self::CurrentnessAfterCompletion(error) => {
                write!(
                    formatter,
                    "Worker V3 currentness failed after completion: {error}"
                )
            }
            Self::ArtifactIdentityMismatch => {
                formatter.write_str("Worker V3 finalized-artifact identity mismatch")
            }
            Self::ArtifactLengthMismatch => {
                formatter.write_str("Worker V3 finalized-artifact length mismatch")
            }
            Self::KernelNameMismatch => {
                formatter.write_str("Worker V3 selected-kernel name mismatch")
            }
            Self::DispatchContractMismatch => {
                formatter.write_str("Worker V3 invocation contract mismatch")
            }
            Self::DeviceIdentityMismatch => {
                formatter.write_str("Worker V3 KFD device identity mismatch")
            }
            Self::CompletedBufferCardinalityMismatch => {
                formatter.write_str("KFD completion returned the wrong buffer cardinality")
            }
            Self::ReadOnlyBufferModified { index } => {
                write!(
                    formatter,
                    "KFD completion modified read-only buffer {index}"
                )
            }
        }
    }
}

impl<E> std::error::Error for Gfx942AuthorizedRuntimeExecutionErrorV1<E>
where
    E: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CurrentnessBeforeDispatch(error) | Self::CurrentnessAfterCompletion(error) => {
                Some(error)
            }
            _ => None,
        }
    }
}

/// One completed runtime buffer retaining its Worker V3 access classification.
#[derive(Debug, Eq, PartialEq)]
pub struct Gfx942AuthorizedRuntimeCompletedBufferV1 {
    bytes: Vec<u8>,
    access: Gfx942RuntimeBufferAccessV1,
}

impl Gfx942AuthorizedRuntimeCompletedBufferV1 {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub const fn access(&self) -> Gfx942RuntimeBufferAccessV1 {
        self.access
    }
}

/// Redacted successful result after confirmed completion, effect checks, and teardown.
#[must_use]
pub struct Gfx942AuthorizedRuntimeDispatchResultV1 {
    buffers: Vec<Gfx942AuthorizedRuntimeCompletedBufferV1>,
    packet_id: u64,
    queue_id: u32,
    completion_elapsed: Duration,
}

impl fmt::Debug for Gfx942AuthorizedRuntimeDispatchResultV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942AuthorizedRuntimeDispatchResultV1")
            .field("buffers", &self.buffers.len())
            .field("packet_id", &self.packet_id)
            .field("queue_id", &self.queue_id)
            .field("completion_elapsed", &self.completion_elapsed)
            .finish()
    }
}

impl Gfx942AuthorizedRuntimeDispatchResultV1 {
    pub fn buffers(&self) -> &[Gfx942AuthorizedRuntimeCompletedBufferV1] {
        &self.buffers
    }

    pub fn into_buffers(self) -> Vec<Gfx942AuthorizedRuntimeCompletedBufferV1> {
        self.buffers
    }

    pub const fn packet_id(&self) -> u64 {
        self.packet_id
    }

    pub const fn queue_id(&self) -> u32 {
        self.queue_id
    }

    pub const fn completion_elapsed(&self) -> Duration {
        self.completion_elapsed
    }
}

/// Consumes one exact Worker V3 authority and executes its matching prepared KFD request.
///
/// Every authority and device mismatch fails before native mutation. Once KFD mutation starts, the
/// low-level transaction requires process termination on every error; this safe boundary enforces
/// that contract by aborting instead of returning an error into potentially live application code.
pub fn execute_authorized_gfx942_runtime_dispatch_v1<A>(
    authority: A,
    device: CheckedGfx942XnackMinusDevice,
    prepared: PreparedGfx942RuntimeDispatchV1,
) -> Result<
    Gfx942AuthorizedRuntimeDispatchResultV1,
    Gfx942AuthorizedRuntimeExecutionErrorV1<A::CurrentnessError>,
>
where
    A: WorkerV3Gfx942ExecutionAuthorityV1,
{
    authority
        .revalidate_currentness()
        .map_err(Gfx942AuthorizedRuntimeExecutionErrorV1::CurrentnessBeforeDispatch)?;
    validate_authority_v1(&authority, &device, &prepared)?;
    let (request, buffer_policies) = prepared.into_authorized_execution_parts();
    // SAFETY: the unsafe authority implementation promises the complete semantic obligations for
    // the exact identities independently compared above. Every native failure aborts immediately,
    // satisfying the mechanics transaction's terminal-failure contract.
    let result = match unsafe { execute_gfx942_kfd_dispatch_unchecked_v1(device, request) } {
        Ok(result) => result,
        Err(_) => std::process::abort(),
    };
    let result = validate_completed_buffers_v1(result, buffer_policies);
    authority
        .revalidate_currentness()
        .map_err(Gfx942AuthorizedRuntimeExecutionErrorV1::CurrentnessAfterCompletion)?;
    result.map_err(|error| match error {
        CompletedBufferValidationErrorV1::Cardinality => {
            Gfx942AuthorizedRuntimeExecutionErrorV1::CompletedBufferCardinalityMismatch
        }
        CompletedBufferValidationErrorV1::ReadOnlyModified { index } => {
            Gfx942AuthorizedRuntimeExecutionErrorV1::ReadOnlyBufferModified { index }
        }
    })
}

enum CompletedBufferValidationErrorV1 {
    Cardinality,
    ReadOnlyModified { index: usize },
}

fn validate_completed_buffers_v1(
    result: fe2o3_kfd::Gfx942KfdDispatchResultV1,
    policies: Vec<Gfx942RuntimePreparedBufferPolicyV1>,
) -> Result<Gfx942AuthorizedRuntimeDispatchResultV1, CompletedBufferValidationErrorV1> {
    let packet_id = result.packet_id();
    let queue_id = result.queue_id();
    let completion_elapsed = result.completion_elapsed();
    let buffers = result.into_buffers();
    if buffers.len() != policies.len() {
        return Err(CompletedBufferValidationErrorV1::Cardinality);
    }
    let mut completed = Vec::with_capacity(buffers.len());
    for (index, (buffer, policy)) in buffers.into_iter().zip(policies).enumerate() {
        let bytes = buffer.into_bytes();
        if !completed_buffer_satisfies_policy_v1(&policy, &bytes) {
            return Err(CompletedBufferValidationErrorV1::ReadOnlyModified { index });
        }
        completed.push(Gfx942AuthorizedRuntimeCompletedBufferV1 {
            bytes,
            access: policy.access(),
        });
    }
    Ok(Gfx942AuthorizedRuntimeDispatchResultV1 {
        buffers: completed,
        packet_id,
        queue_id,
        completion_elapsed,
    })
}

fn completed_buffer_satisfies_policy_v1(
    policy: &Gfx942RuntimePreparedBufferPolicyV1,
    completed_bytes: &[u8],
) -> bool {
    policy
        .read_only_initial_bytes()
        .is_none_or(|initial| initial == completed_bytes)
}

fn validate_authority_v1<A>(
    authority: &A,
    device: &CheckedGfx942XnackMinusDevice,
    prepared: &PreparedGfx942RuntimeDispatchV1,
) -> Result<(), Gfx942AuthorizedRuntimeExecutionErrorV1<A::CurrentnessError>>
where
    A: WorkerV3Gfx942ExecutionAuthorityV1,
{
    validate_authority_bindings_v1(
        authority,
        prepared.identity().object_sha256(),
        prepared.finalized_hsaco_length(),
        prepared.kernel_name(),
        prepared.dispatch_contract_sha256(),
        device.observation().unique_id(),
    )
}

fn validate_authority_bindings_v1<A>(
    authority: &A,
    finalized_hsaco_sha256: [u8; 32],
    finalized_hsaco_length: u64,
    kernel_name: &str,
    dispatch_contract_sha256: [u8; 32],
    device_unique_id: u64,
) -> Result<(), Gfx942AuthorizedRuntimeExecutionErrorV1<A::CurrentnessError>>
where
    A: WorkerV3Gfx942ExecutionAuthorityV1,
{
    if authority.finalized_hsaco_sha256() != finalized_hsaco_sha256 {
        return Err(Gfx942AuthorizedRuntimeExecutionErrorV1::ArtifactIdentityMismatch);
    }
    if authority.finalized_hsaco_length() != finalized_hsaco_length {
        return Err(Gfx942AuthorizedRuntimeExecutionErrorV1::ArtifactLengthMismatch);
    }
    if authority.kernel_name() != kernel_name {
        return Err(Gfx942AuthorizedRuntimeExecutionErrorV1::KernelNameMismatch);
    }
    if authority.dispatch_contract_sha256() != dispatch_contract_sha256 {
        return Err(Gfx942AuthorizedRuntimeExecutionErrorV1::DispatchContractMismatch);
    }
    if authority.device_unique_id() != device_unique_id {
        return Err(Gfx942AuthorizedRuntimeExecutionErrorV1::DeviceIdentityMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestAuthorityV1 {
        object: [u8; 32],
        length: u64,
        kernel: &'static str,
        dispatch: [u8; 32],
        device: u64,
    }

    // SAFETY: this implementation is confined to pure identity-comparison unit tests and can
    // never reach a native device token or the execution function.
    unsafe impl WorkerV3Gfx942ExecutionAuthorityV1 for TestAuthorityV1 {
        type CurrentnessError = core::convert::Infallible;

        fn finalized_hsaco_sha256(&self) -> [u8; 32] {
            self.object
        }

        fn finalized_hsaco_length(&self) -> u64 {
            self.length
        }

        fn kernel_name(&self) -> &str {
            self.kernel
        }

        fn dispatch_contract_sha256(&self) -> [u8; 32] {
            self.dispatch
        }

        fn device_unique_id(&self) -> u64 {
            self.device
        }

        fn revalidate_currentness(&self) -> Result<(), Self::CurrentnessError> {
            Ok(())
        }
    }

    fn authority() -> TestAuthorityV1 {
        TestAuthorityV1 {
            object: [1; 32],
            length: 7_000,
            kernel: "kernel_v1",
            dispatch: [2; 32],
            device: 0x1234,
        }
    }

    fn validate(
        authority: &TestAuthorityV1,
    ) -> Result<(), Gfx942AuthorizedRuntimeExecutionErrorV1<core::convert::Infallible>> {
        validate_authority_bindings_v1(authority, [1; 32], 7_000, "kernel_v1", [2; 32], 0x1234)
    }

    #[test]
    fn exact_worker_v3_runtime_bindings_are_required() {
        assert!(validate(&authority()).is_ok());

        let mut changed = authority();
        changed.object[0] ^= 1;
        assert!(matches!(
            validate(&changed),
            Err(Gfx942AuthorizedRuntimeExecutionErrorV1::ArtifactIdentityMismatch)
        ));
        let mut changed = authority();
        changed.length += 1;
        assert!(matches!(
            validate(&changed),
            Err(Gfx942AuthorizedRuntimeExecutionErrorV1::ArtifactLengthMismatch)
        ));
        let mut changed = authority();
        changed.kernel = "other";
        assert!(matches!(
            validate(&changed),
            Err(Gfx942AuthorizedRuntimeExecutionErrorV1::KernelNameMismatch)
        ));
        let mut changed = authority();
        changed.dispatch[0] ^= 1;
        assert!(matches!(
            validate(&changed),
            Err(Gfx942AuthorizedRuntimeExecutionErrorV1::DispatchContractMismatch)
        ));
        let mut changed = authority();
        changed.device += 1;
        assert!(matches!(
            validate(&changed),
            Err(Gfx942AuthorizedRuntimeExecutionErrorV1::DeviceIdentityMismatch)
        ));
    }

    #[test]
    fn completed_read_only_buffers_must_preserve_every_byte() {
        let read_only = Gfx942RuntimePreparedBufferPolicyV1 {
            access: Gfx942RuntimeBufferAccessV1::ReadOnly,
            read_only_initial_bytes: Some(vec![1, 2, 3, 4]),
        };
        assert!(completed_buffer_satisfies_policy_v1(
            &read_only,
            &[1, 2, 3, 4]
        ));
        assert!(!completed_buffer_satisfies_policy_v1(
            &read_only,
            &[1, 2, 0, 4]
        ));

        for access in [
            Gfx942RuntimeBufferAccessV1::WriteOnly,
            Gfx942RuntimeBufferAccessV1::ReadWrite,
        ] {
            let writable = Gfx942RuntimePreparedBufferPolicyV1 {
                access,
                read_only_initial_bytes: None,
            };
            assert!(completed_buffer_satisfies_policy_v1(&writable, &[9, 8, 7]));
        }
    }
}
