//! Production-only generated preparation; Context admission and completion are separate transitions.

use std::{error::Error, fmt, marker::PhantomData, rc::Rc};

use fe2o3_aql::AqlDispatchGeometryV1;
use fe2o3_kfd::CheckedGfx942XnackMinusDevice;
use fe2o3_runtime::{
    Gfx942RuntimeProjectionErrorV1, KfdRuntimeBackendErrorV1, KfdRuntimeBackendV1,
    PreparedGfx942PersistentDispatchV1, PreparedGfx942RuntimeDispatchV1, RuntimeContextV1,
    RuntimeDeviceIdV1, RuntimeErrorV1, RuntimeGfx942PreparationErrorV1, RuntimeGfx942PreparedV1,
};

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

struct GeneratedContextPreparationV1<K, P = PreparedGfx942RuntimeDispatchV1> {
    storage: GeneratedRuntimeStorageV1<P>,
    authority: GeneratedWorkerV3KfdExecutionAuthority<K>,
    footprint: GeneratedRuntimeArgumentFootprintV1,
}

impl<K: CompilerGeneratedKernelExpectationV1> GeneratedContextPreparationV1<K> {
    fn project_persistent(
        self,
    ) -> Result<
        GeneratedContextPreparationV1<K, PreparedGfx942PersistentDispatchV1>,
        GeneratedWorkerV3RuntimeInvocationErrorV1,
    > {
        let storage = self
            .storage
            .project_persistent(
                self.authority
                    .binding
                    .authenticated
                    .current_publication_token()
                    .exact_artifact_bytes(),
            )
            .map_err(GeneratedWorkerV3RuntimeInvocationErrorV1::Projection)?;
        Ok(GeneratedContextPreparationV1 {
            storage,
            authority: self.authority,
            footprint: self.footprint,
        })
    }
}

/// Generated custody prepared against an existing Context's retained device.
///
/// The private payload and decoder remain owner-local. There is no native
/// publication, output completion, device extraction or qualification fallback.
/// This may outlive the Context only as inert host storage; validation requires
/// the same live Context and exact native device generation.
#[must_use]
pub struct GeneratedWorkerV3ContextInvocationV1<K> {
    prepared: RuntimeGfx942PreparedV1<
        GeneratedContextPreparationV1<K, PreparedGfx942PersistentDispatchV1>,
    >,
}

pub type GeneratedWorkerV3ContextInvocationErrorV1 =
    RuntimeGfx942PreparationErrorV1<GeneratedWorkerV3RuntimeInvocationErrorV1>;

impl<K: CompilerGeneratedKernelExpectationV1> GeneratedWorkerV3ContextInvocationV1<K> {
    pub fn kernel_name(&self) -> &str {
        self.prepared.value().storage.prepared().kernel_name()
    }

    pub fn dispatch_contract_sha256(&self) -> [u8; 32] {
        self.prepared
            .value()
            .storage
            .prepared()
            .dispatch_contract_sha256()
    }

    pub fn footprint(&self) -> GeneratedRuntimeArgumentFootprintV1 {
        self.prepared.value().footprint
    }

    pub fn application_binding(&self) -> &WorkerV3ApplicationExecutionBindingV1<K> {
        &self.prepared.value().authority.binding
    }

    /// Nonexecuting revalidation, not admission or permission to replay.
    pub fn validate_context(
        &self,
        context: &mut RuntimeContextV1<KfdRuntimeBackendV1>,
    ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        context.validate_gfx942_prepared_v1(&self.prepared)
    }
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
        self,
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
        self.require_runtime_evidence()?;
        let prepared = device
            .with_retained_device_v1(|device| {
                self.prepare_context_payload(
                    arguments,
                    device,
                    geometry,
                    dynamic_group_segment_bytes,
                    timeout_milliseconds,
                    limits,
                    result_budget,
                )
            })
            .map_err(GeneratedWorkerV3KfdInvocationError::DeviceCurrentness)??;
        let GeneratedContextPreparationV1 {
            storage,
            authority,
            footprint,
        } = prepared;
        Ok(GeneratedWorkerV3RuntimeInvocationV1 {
            storage,
            authority,
            device,
            footprint,
            owner_local: PhantomData,
        })
    }

    /// Consumes generated arguments without consuming or readmitting the Context device.
    /// Missing protected evidence rejects before Context access or argument callbacks.
    ///
    /// ```no_run
    /// use fe2o3_host::{AuthenticatedWorkerV3ExecutableV1, CompilerGeneratedKernelExpectationV1,
    ///     CompilerGeneratedRuntimeArguments, GeneratedRuntimeArgumentLimitsV1,
    ///     GeneratedRuntimeResultBudgetV1, GeneratedWorkerV3ContextInvocationV1,
    ///     GeneratedWorkerV3ContextInvocationErrorV1, AqlDispatchGeometryV1};
    /// use fe2o3_runtime::{RuntimeContextV1, KfdRuntimeBackendV1, RuntimeDeviceIdV1};
    /// fn prepare<K: CompilerGeneratedKernelExpectationV1, A: CompilerGeneratedRuntimeArguments<K>>(
    ///     executable: AuthenticatedWorkerV3ExecutableV1<K>, args: A,
    ///     context: &mut RuntimeContextV1<KfdRuntimeBackendV1>, device: RuntimeDeviceIdV1,
    ///     geometry: AqlDispatchGeometryV1, budget: &GeneratedRuntimeResultBudgetV1,
    /// ) -> Result<GeneratedWorkerV3ContextInvocationV1<K>, GeneratedWorkerV3ContextInvocationErrorV1> {
    ///     let invocation = executable.prepare_generated_context_invocation(args, context,
    ///         device, geometry, 0, 1000, GeneratedRuntimeArgumentLimitsV1::new(4096, 4096, 16), budget)?;
    ///     let _devices = context.devices(); // No Context borrow remains in the invocation.
    ///     Ok(invocation)
    /// }
    /// ```
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_generated_context_invocation<Arguments>(
        self,
        arguments: Arguments,
        context: &mut RuntimeContextV1<KfdRuntimeBackendV1>,
        device: RuntimeDeviceIdV1,
        geometry: AqlDispatchGeometryV1,
        dynamic_group_segment_bytes: u32,
        timeout_milliseconds: u32,
        limits: GeneratedRuntimeArgumentLimitsV1,
        result_budget: &GeneratedRuntimeResultBudgetV1,
    ) -> Result<GeneratedWorkerV3ContextInvocationV1<K>, GeneratedWorkerV3ContextInvocationErrorV1>
    where
        Arguments: CompilerGeneratedRuntimeArguments<K>,
    {
        self.require_runtime_evidence()
            .map_err(RuntimeGfx942PreparationErrorV1::Preparation)?;
        let prepared = context.with_gfx942_preparation_device_v1(device, |device| {
            self.prepare_context_payload(
                arguments,
                device,
                geometry,
                dynamic_group_segment_bytes,
                timeout_milliseconds,
                limits,
                result_budget,
            )?
            .project_persistent()
        })?;
        Ok(GeneratedWorkerV3ContextInvocationV1 { prepared })
    }

    fn require_runtime_evidence(&self) -> Result<(), GeneratedWorkerV3RuntimeInvocationErrorV1> {
        if self
            .verification()
            .retains_protected_application_execution_evidence()
        {
            Ok(())
        } else {
            Err(GeneratedWorkerV3KfdInvocationError::ProtectedProductionEvidenceUnavailable.into())
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_context_payload<Arguments>(
        mut self,
        arguments: Arguments,
        device: &CheckedGfx942XnackMinusDevice,
        geometry: AqlDispatchGeometryV1,
        dynamic_group_segment_bytes: u32,
        timeout_milliseconds: u32,
        limits: GeneratedRuntimeArgumentLimitsV1,
        result_budget: &GeneratedRuntimeResultBudgetV1,
    ) -> Result<GeneratedContextPreparationV1<K>, GeneratedWorkerV3RuntimeInvocationErrorV1>
    where
        Arguments: CompilerGeneratedRuntimeArguments<K>,
    {
        self.revalidate_currentness()
            .map_err(GeneratedWorkerV3KfdInvocationError::CurrentPublication)?;
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

        self.admission()
            .revalidate_retained_currentness_token(self.current_publication_token())
            .map_err(GeneratedWorkerV3KfdInvocationError::CurrentPublication)?;
        let application = application_execution_admission(
            &mut self,
            device,
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
        Ok(GeneratedContextPreparationV1 {
            storage,
            authority,
            footprint: parts.footprint,
        })
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum GeneratedWorkerV3RuntimeInvocationErrorV1 {
    Invocation(GeneratedWorkerV3KfdInvocationError),
    Arguments(GeneratedRuntimeArgumentErrorV1),
    Projection(Gfx942RuntimeProjectionErrorV1),
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
            Self::Projection(error) => {
                write!(formatter, "generated persistent projection failed: {error}")
            }
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
            Self::Projection(error) => Some(error),
            Self::Arguments(error) => Some(error),
        }
    }
}
