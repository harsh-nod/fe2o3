use std::{error::Error, fmt};

use fe2o3_amd_target::{AmdTargetId, PRODUCTION_GFX942_DEVICE_TARGET_V1};
use fe2o3_aql::AqlDispatchGeometryV1;
use fe2o3_artifact_transaction::DurableCurrentLinkPublicationTokenV1;
use fe2o3_kfd::{CheckedGfx942XnackMinusDevice, DeviceBindingError};
use fe2o3_runtime::{
    Gfx942AuthorizedRuntimeDispatchResultV1, Gfx942AuthorizedRuntimeExecutionErrorV1,
    Gfx942RuntimePreparationErrorV1, PreparedGfx942RuntimeDispatchV1,
    WorkerV3Gfx942ExecutionAuthorityV1, execute_authorized_gfx942_runtime_dispatch_v1,
    prepare_gfx942_runtime_dispatch_v1,
};

use crate::{
    AuthenticatedWorkerV3ExecutableV1, CompilerGeneratedKernelExpectationV1,
    CompilerGeneratedKfdArguments, GeneratedKfdCompletion, GeneratedKfdCompletionError,
    GeneratedKfdPrepareError, RecoveredWorkerV3AdmissionErrorV1,
};

/// One exact, move-only Worker V3 invocation ready for the permanent pure-KFD runtime.
///
/// Construction joins authenticated compiler and proof evidence, compiler-generated argument
/// capabilities, the current finalized artifact, physical runtime preparation, and one checked
/// gfx942 device. The private authority cannot be extracted or exchanged with another prepared
/// request or device.
#[must_use = "a prepared KFD invocation retains output borrows and execution authority"]
pub struct GeneratedWorkerV3KfdInvocation<'allocation, K> {
    authority: GeneratedWorkerV3KfdExecutionAuthority<K>,
    device: CheckedGfx942XnackMinusDevice,
    prepared: PreparedGfx942RuntimeDispatchV1,
    completion: GeneratedKfdCompletion<'allocation>,
}

impl<K> fmt::Debug for GeneratedWorkerV3KfdInvocation<'_, K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedWorkerV3KfdInvocation")
            .field("kernel_name", &self.prepared.kernel_name())
            .field(
                "dispatch_contract_sha256",
                &self.prepared.dispatch_contract_sha256(),
            )
            .field("device_unique_id", &self.device.observation().unique_id())
            .finish_non_exhaustive()
    }
}

impl<K: CompilerGeneratedKernelExpectationV1> GeneratedWorkerV3KfdInvocation<'_, K> {
    pub fn kernel_name(&self) -> &str {
        self.prepared.kernel_name()
    }

    pub const fn dispatch_contract_sha256(&self) -> [u8; 32] {
        self.prepared.dispatch_contract_sha256()
    }

    pub const fn device_unique_id(&self) -> u64 {
        self.authority.device_unique_id
    }

    /// Consumes the complete authority, waits for KFD quiescence, validates runtime effects, and
    /// writes completed mutable buffers back through their retained exclusive Rust borrows.
    pub fn execute(
        self,
    ) -> Result<Gfx942AuthorizedRuntimeDispatchResultV1, GeneratedWorkerV3KfdExecutionError> {
        let Self {
            authority,
            device,
            prepared,
            completion,
        } = self;
        let result = execute_authorized_gfx942_runtime_dispatch_v1(authority, device, prepared)
            .map_err(GeneratedWorkerV3KfdExecutionError::Runtime)?;
        completion
            .apply(result)
            .map_err(GeneratedWorkerV3KfdExecutionError::Completion)
    }
}

struct GeneratedWorkerV3KfdExecutionAuthority<K> {
    authenticated: AuthenticatedWorkerV3ExecutableV1<K>,
    current: DurableCurrentLinkPublicationTokenV1,
    finalized_hsaco_sha256: [u8; 32],
    finalized_hsaco_length: u64,
    kernel_name: &'static str,
    dispatch_contract_sha256: [u8; 32],
    device_unique_id: u64,
}

// SAFETY: this private implementation is constructed only by
// `prepare_generated_kfd_invocation`. That transition retains the exact authenticated Worker V3
// decision and current-publication token, admits only compiler-generated argument capabilities,
// prepares the runtime request from the token's exact HSACO bytes, validates the selected kernel
// and artifact identities, and retains the same checked KFD device whose identity is named here.
unsafe impl<K: CompilerGeneratedKernelExpectationV1> WorkerV3Gfx942ExecutionAuthorityV1
    for GeneratedWorkerV3KfdExecutionAuthority<K>
{
    type CurrentnessError = RecoveredWorkerV3AdmissionErrorV1;

    fn finalized_hsaco_sha256(&self) -> [u8; 32] {
        self.finalized_hsaco_sha256
    }

    fn finalized_hsaco_length(&self) -> u64 {
        self.finalized_hsaco_length
    }

    fn kernel_name(&self) -> &str {
        self.kernel_name
    }

    fn dispatch_contract_sha256(&self) -> [u8; 32] {
        self.dispatch_contract_sha256
    }

    fn device_unique_id(&self) -> u64 {
        self.device_unique_id
    }

    fn revalidate_currentness(&self) -> Result<(), Self::CurrentnessError> {
        self.authenticated
            .admission()
            .revalidate_retained_currentness_token(&self.current)
    }
}

impl<K: CompilerGeneratedKernelExpectationV1> AuthenticatedWorkerV3ExecutableV1<K> {
    /// Joins this authenticated executable with one generated host-memory invocation and checked
    /// gfx942 device. No caller-created digest or raw pointer enters the transition.
    pub fn prepare_generated_kfd_invocation<'allocation, Arguments>(
        self,
        arguments: Arguments,
        mut device: CheckedGfx942XnackMinusDevice,
        geometry: AqlDispatchGeometryV1,
        dynamic_group_segment_bytes: u32,
        timeout_milliseconds: u32,
    ) -> Result<GeneratedWorkerV3KfdInvocation<'allocation, K>, GeneratedWorkerV3KfdInvocationError>
    where
        Arguments: CompilerGeneratedKfdArguments<'allocation, K>,
    {
        let current = self
            .admission()
            .acquire_retained_currentness_token()
            .map_err(GeneratedWorkerV3KfdInvocationError::CurrentPublication)?;
        device
            .check_observable_currentness()
            .map_err(GeneratedWorkerV3KfdInvocationError::DeviceCurrentness)?;
        validate_gfx942_target(&self)?;

        let packed = self
            .prepare_generated_kfd_arguments_with_current(&current, arguments)
            .map_err(GeneratedWorkerV3KfdInvocationError::Arguments)?;
        let (inputs, completion) =
            packed.into_runtime_inputs(geometry, dynamic_group_segment_bytes, timeout_milliseconds);
        let prepared = prepare_gfx942_runtime_dispatch_v1(
            current.exact_artifact_bytes(),
            K::EXPORT_NAME,
            inputs,
        )
        .map_err(GeneratedWorkerV3KfdInvocationError::RuntimePreparation)?;
        validate_runtime_binding(&self, &prepared)?;

        device
            .check_observable_currentness()
            .map_err(GeneratedWorkerV3KfdInvocationError::DeviceCurrentness)?;
        self.admission()
            .revalidate_retained_currentness_token(&current)
            .map_err(GeneratedWorkerV3KfdInvocationError::CurrentPublication)?;

        let verification = self.verification();
        let authority = GeneratedWorkerV3KfdExecutionAuthority {
            finalized_hsaco_sha256: verification.finalized_hsaco_sha256(),
            finalized_hsaco_length: verification.finalized_hsaco_length(),
            kernel_name: K::EXPORT_NAME,
            dispatch_contract_sha256: prepared.dispatch_contract_sha256(),
            device_unique_id: device.observation().unique_id(),
            authenticated: self,
            current,
        };
        Ok(GeneratedWorkerV3KfdInvocation {
            authority,
            device,
            prepared,
            completion,
        })
    }
}

fn validate_gfx942_target<K: CompilerGeneratedKernelExpectationV1>(
    authenticated: &AuthenticatedWorkerV3ExecutableV1<K>,
) -> Result<(), GeneratedWorkerV3KfdInvocationError> {
    let expected = AmdTargetId::parse(PRODUCTION_GFX942_DEVICE_TARGET_V1)
        .expect("the canonical production gfx942 target is valid");
    let artifact = authenticated.target();
    if artifact != expected {
        return Err(GeneratedWorkerV3KfdInvocationError::TargetMismatch { artifact });
    }
    Ok(())
}

fn validate_runtime_binding<K: CompilerGeneratedKernelExpectationV1>(
    authenticated: &AuthenticatedWorkerV3ExecutableV1<K>,
    prepared: &PreparedGfx942RuntimeDispatchV1,
) -> Result<(), GeneratedWorkerV3KfdInvocationError> {
    let verification = authenticated.verification();
    validate_runtime_identity_fields(
        verification.finalized_hsaco_sha256(),
        verification.finalized_hsaco_length(),
        K::EXPORT_NAME,
        prepared.identity().object_sha256(),
        prepared.finalized_hsaco_length(),
        prepared.kernel_name(),
    )
}

fn validate_runtime_identity_fields(
    expected_sha256: [u8; 32],
    expected_length: u64,
    expected_kernel_name: &str,
    actual_sha256: [u8; 32],
    actual_length: u64,
    actual_kernel_name: &str,
) -> Result<(), GeneratedWorkerV3KfdInvocationError> {
    if actual_sha256 != expected_sha256 {
        return Err(GeneratedWorkerV3KfdInvocationError::ArtifactIdentityMismatch);
    }
    if actual_length != expected_length {
        return Err(GeneratedWorkerV3KfdInvocationError::ArtifactLengthMismatch);
    }
    if actual_kernel_name != expected_kernel_name {
        return Err(GeneratedWorkerV3KfdInvocationError::KernelNameMismatch);
    }
    Ok(())
}

#[derive(Debug)]
#[non_exhaustive]
pub enum GeneratedWorkerV3KfdInvocationError {
    CurrentPublication(RecoveredWorkerV3AdmissionErrorV1),
    DeviceCurrentness(DeviceBindingError),
    TargetMismatch { artifact: AmdTargetId },
    Arguments(GeneratedKfdPrepareError),
    RuntimePreparation(Gfx942RuntimePreparationErrorV1),
    ArtifactIdentityMismatch,
    ArtifactLengthMismatch,
    KernelNameMismatch,
}

impl fmt::Display for GeneratedWorkerV3KfdInvocationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentPublication(error) => {
                write!(formatter, "Worker V3 publication is not current: {error}")
            }
            Self::DeviceCurrentness(error) => {
                write!(formatter, "checked KFD device is not current: {error}")
            }
            Self::TargetMismatch { artifact } => write!(
                formatter,
                "pure-KFD invocation requires {PRODUCTION_GFX942_DEVICE_TARGET_V1}; artifact is {artifact}"
            ),
            Self::Arguments(error) => write!(formatter, "generated KFD arguments failed: {error}"),
            Self::RuntimePreparation(error) => {
                write!(formatter, "pure-KFD runtime preparation failed: {error}")
            }
            Self::ArtifactIdentityMismatch => {
                formatter.write_str("runtime prepared a different finalized HSACO")
            }
            Self::ArtifactLengthMismatch => {
                formatter.write_str("runtime prepared a different finalized HSACO length")
            }
            Self::KernelNameMismatch => {
                formatter.write_str("runtime prepared a different kernel entry")
            }
        }
    }
}

impl Error for GeneratedWorkerV3KfdInvocationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CurrentPublication(error) => Some(error),
            Self::DeviceCurrentness(error) => Some(error),
            Self::Arguments(error) => Some(error),
            Self::RuntimePreparation(error) => Some(error),
            Self::TargetMismatch { .. }
            | Self::ArtifactIdentityMismatch
            | Self::ArtifactLengthMismatch
            | Self::KernelNameMismatch => None,
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum GeneratedWorkerV3KfdExecutionError {
    Runtime(Gfx942AuthorizedRuntimeExecutionErrorV1<RecoveredWorkerV3AdmissionErrorV1>),
    Completion(GeneratedKfdCompletionError),
}

impl fmt::Display for GeneratedWorkerV3KfdExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Runtime(error) => write!(formatter, "authorized KFD execution failed: {error}"),
            Self::Completion(error) => {
                write!(formatter, "generated KFD completion failed: {error}")
            }
        }
    }
}

impl Error for GeneratedWorkerV3KfdExecutionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Runtime(error) => Some(error),
            Self::Completion(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_binding_requires_exact_artifact_length_and_kernel() {
        let expected_sha256 = [0x41; 32];
        assert!(
            validate_runtime_identity_fields(
                expected_sha256,
                4_096,
                "kernel_v1",
                expected_sha256,
                4_096,
                "kernel_v1",
            )
            .is_ok()
        );

        let mut changed_sha256 = expected_sha256;
        changed_sha256[0] ^= 1;
        assert!(matches!(
            validate_runtime_identity_fields(
                expected_sha256,
                4_096,
                "kernel_v1",
                changed_sha256,
                4_096,
                "kernel_v1",
            ),
            Err(GeneratedWorkerV3KfdInvocationError::ArtifactIdentityMismatch)
        ));
        assert!(matches!(
            validate_runtime_identity_fields(
                expected_sha256,
                4_096,
                "kernel_v1",
                expected_sha256,
                4_097,
                "kernel_v1",
            ),
            Err(GeneratedWorkerV3KfdInvocationError::ArtifactLengthMismatch)
        ));
        assert!(matches!(
            validate_runtime_identity_fields(
                expected_sha256,
                4_096,
                "kernel_v1",
                expected_sha256,
                4_096,
                "other",
            ),
            Err(GeneratedWorkerV3KfdInvocationError::KernelNameMismatch)
        ));
    }
}
