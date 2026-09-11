//! Production-only generated preparation; Context admission and completion are separate transitions.

use std::{error::Error, fmt, marker::PhantomData, rc::Rc};

use fe2o3_aql::AqlDispatchGeometryV1;
use fe2o3_kfd::CheckedGfx942XnackMinusDevice;
use fe2o3_runtime::PreparedGfx942RuntimeDispatchV1;

use super::{
    GeneratedWorkerV3KfdExecutionAuthority, GeneratedWorkerV3KfdInvocationError,
    WorkerV3ApplicationExecutionBindingV1, application_execution_admission, validate_gfx942_target,
    validate_runtime_binding,
};
use crate::generated_runtime_arguments::GeneratedRuntimeStorageV1;
use crate::{
    AuthenticatedWorkerV3ExecutableV1, CompilerGeneratedKernelExpectationV1,
    CompilerGeneratedRuntimeArguments, GeneratedRuntimeArgumentErrorV1,
    GeneratedRuntimeArgumentFootprintV1, GeneratedRuntimeArgumentLimitsV1,
    GeneratedRuntimeResultBudgetV1,
};

/// Owned, nonexecuting generated invocation for one checked gfx942 device.
///
/// Retains the authenticated executable, exact prepared request and private charged decoder.
/// It is neither `Clone`, `Send` nor `Sync` and exposes no request or decoder extraction.
/// Preparation does not admit a Context operation, publish GPU work or produce output results.
/// Dropping it disposes prepared storage before decoder credits and the device capability.
///
/// R73's result budget does not cover executable images, hidden kernargs, read-only
/// initialization copies or metadata. Whole-invocation accounting remains a separate boundary.
#[must_use]
pub struct GeneratedWorkerV3RuntimeInvocationV1<K> {
    storage: GeneratedRuntimeStorageV1<PreparedGfx942RuntimeDispatchV1>,
    authority: GeneratedWorkerV3KfdExecutionAuthority<K>,
    device: CheckedGfx942XnackMinusDevice,
    footprint: GeneratedRuntimeArgumentFootprintV1,
    owner_local: PhantomData<Rc<()>>,
}

impl<K: CompilerGeneratedKernelExpectationV1> GeneratedWorkerV3RuntimeInvocationV1<K> {
    pub fn kernel_name(&self) -> &str {
        self.storage.prepared().kernel_name()
    }

    pub fn dispatch_contract_sha256(&self) -> [u8; 32] {
        self.storage.prepared().dispatch_contract_sha256()
    }

    pub fn device_unique_id(&self) -> u64 {
        self.device.observation().unique_id()
    }

    pub const fn footprint(&self) -> GeneratedRuntimeArgumentFootprintV1 {
        self.footprint
    }

    /// Descriptive authenticated coordinates, not a transferable execution permit.
    pub fn application_binding(&self) -> &WorkerV3ApplicationExecutionBindingV1<K> {
        &self.authority.binding
    }
}

impl<K: CompilerGeneratedKernelExpectationV1> AuthenticatedWorkerV3ExecutableV1<K> {
    /// Consumes an executable and its raw generated arguments into owner-local custody.
    ///
    /// Missing protected evidence rejects before argument callbacks or encoding, including
    /// under verifier test-support features. This early check is not admission: currentness,
    /// exact runtime identity and consuming semantic-to-machine admission are still required.
    /// Device currentness performs observations, but preparation creates no native VM,
    /// allocation, queue or dispatch. There is no qualification fallback.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_generated_runtime_invocation<Arguments>(
        mut self,
        arguments: Arguments,
        mut device: CheckedGfx942XnackMinusDevice,
        geometry: AqlDispatchGeometryV1,
        dynamic_group_segment_bytes: u32,
        timeout_milliseconds: u32,
        limits: GeneratedRuntimeArgumentLimitsV1,
        result_budget: &GeneratedRuntimeResultBudgetV1,
    ) -> Result<GeneratedWorkerV3RuntimeInvocationV1<K>, GeneratedWorkerV3RuntimeInvocationErrorV1>
    where
        Arguments: CompilerGeneratedRuntimeArguments<K>,
    {
        if !self
            .verification()
            .retains_protected_application_execution_evidence()
        {
            return Err(
                GeneratedWorkerV3KfdInvocationError::ProtectedProductionEvidenceUnavailable.into(),
            );
        }
        self.revalidate_currentness()
            .map_err(GeneratedWorkerV3KfdInvocationError::CurrentPublication)?;
        device
            .check_observable_currentness()
            .map_err(GeneratedWorkerV3KfdInvocationError::DeviceCurrentness)?;
        validate_gfx942_target(&self)?;

        let parts = self
            .prepare_generated_runtime_arguments_charged(arguments, limits, result_budget)
            .map_err(GeneratedWorkerV3RuntimeInvocationErrorV1::Arguments)?
            .into_runtime_inputs(geometry, dynamic_group_segment_bytes, timeout_milliseconds);
        let storage = parts
            .storage
            .prepare(
                self.current_publication_token().exact_artifact_bytes(),
                K::EXPORT_NAME,
            )
            .map_err(GeneratedWorkerV3KfdInvocationError::RuntimePreparation)?;
        validate_runtime_binding(&self, storage.prepared())?;

        device
            .check_observable_currentness()
            .map_err(GeneratedWorkerV3KfdInvocationError::DeviceCurrentness)?;
        self.admission()
            .revalidate_retained_currentness_token(self.current_publication_token())
            .map_err(GeneratedWorkerV3KfdInvocationError::CurrentPublication)?;
        let application = application_execution_admission(
            &mut self,
            &device,
            parts.kernel_id,
            &parts.packing,
            geometry,
            dynamic_group_segment_bytes,
            timeout_milliseconds,
            storage.prepared().dispatch_contract_sha256(),
        )
        .ok_or(GeneratedWorkerV3KfdInvocationError::ProtectedProductionEvidenceUnavailable)?;
        let authority = GeneratedWorkerV3KfdExecutionAuthority::from_application(
            self,
            parts.packing,
            application,
        );
        Ok(GeneratedWorkerV3RuntimeInvocationV1 {
            storage,
            authority,
            device,
            footprint: parts.footprint,
            owner_local: PhantomData,
        })
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum GeneratedWorkerV3RuntimeInvocationErrorV1 {
    Invocation(GeneratedWorkerV3KfdInvocationError),
    Arguments(GeneratedRuntimeArgumentErrorV1),
}

impl From<GeneratedWorkerV3KfdInvocationError> for GeneratedWorkerV3RuntimeInvocationErrorV1 {
    fn from(error: GeneratedWorkerV3KfdInvocationError) -> Self {
        Self::Invocation(error)
    }
}

impl fmt::Display for GeneratedWorkerV3RuntimeInvocationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invocation(error) => error.fmt(formatter),
            Self::Arguments(error) => {
                write!(formatter, "generated runtime arguments failed: {error}")
            }
        }
    }
}

impl Error for GeneratedWorkerV3RuntimeInvocationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Invocation(error) => Some(error),
            Self::Arguments(error) => Some(error),
        }
    }
}
