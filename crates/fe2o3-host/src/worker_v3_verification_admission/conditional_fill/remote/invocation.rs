//! Conditional native composition. No conversion to an unconditional executable exists.

use super::{RemoteConditionalFillArtifactV1, WorkerV3RemoteConditionalFillErrorV1};
use crate::generated_runtime_arguments::{GeneratedRuntimeStorageV1, prepare_charged_with_plan};
use crate::generated_runtime_carrier::{GeneratedRuntimeAuthorityV1, GeneratedRuntimeCarrierV1};
use crate::{
    CompilerGeneratedKernelExpectationV1, CompilerGeneratedRuntimeArguments,
    ConditionalOutputArgumentBindingV1, ConditionalPackedCoverageErrorV1, GeneratedKfdPrepareError,
    GeneratedRuntimeArgumentErrorV1, GeneratedRuntimeArgumentLimitsV1,
    GeneratedRuntimeResultBudgetV1,
};
use fe2o3_amdhsa_loader::{AdmittedProfile, validate};
use fe2o3_aql::AqlDispatchGeometryV1;
use fe2o3_runtime::{
    Gfx942RuntimePreparationErrorV1, Gfx942RuntimeProjectionErrorV1,
    KfdMultiDeviceRuntimeBackendV1, PreparedGfx942RuntimeDispatchV1, RuntimeAsyncEngineCallErrorV1,
    RuntimeAsyncPreparationV1, RuntimeAsyncProgressHandleV1, RuntimeDeviceIdV1,
    RuntimeGfx942PreparationErrorV1, WorkerV3Gfx942ExecutionAuthorityV1,
};
use std::{error::Error, fmt, sync::Arc, time::Instant};

impl<K: CompilerGeneratedKernelExpectationV1 + 'static> RemoteConditionalFillArtifactV1<K> {
    /// Prepares one conditional invocation on the selected retained device.
    ///
    /// Multiple invocations share this exact artifact's publication token, compiler audit and
    /// remote proof owner. Each runtime ticket retains its share through native settlement or
    /// quarantine; dropping the caller's Arc does not release an outstanding invocation.
    /// The application may drive the returned future with the current-thread engine, without
    /// creating a thread. Preparation alone does not publish work or make output available.
    ///
    /// `deadline` bounds preparation and every later proof probe. Expiration rejects further
    /// use; it does not establish GPU quiescence or release retained native resources.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_generated_multi_context_invocation_async<A>(
        self: &Arc<Self>,
        arguments: A,
        owner: &RuntimeAsyncProgressHandleV1<KfdMultiDeviceRuntimeBackendV1>,
        device: RuntimeDeviceIdV1,
        geometry: AqlDispatchGeometryV1,
        dynamic_group_segment_bytes: u32,
        timeout_milliseconds: u32,
        limits: GeneratedRuntimeArgumentLimitsV1,
        result_budget: &GeneratedRuntimeResultBudgetV1,
        deadline: Instant,
    ) -> Result<
        RuntimeAsyncPreparationV1<
            RuntimeGfx942PreparationErrorV1<WorkerV3ConditionalFillInvocationErrorV1>,
        >,
        WorkerV3ConditionalFillInvocationErrorV1,
    >
    where
        A: CompilerGeneratedRuntimeArguments<K>,
    {
        use WorkerV3ConditionalFillInvocationErrorV1 as E;
        self.revalidate(deadline).map_err(E::Remote)?;
        check_geometry(geometry, dynamic_group_segment_bytes)?;
        let artifact = Arc::clone(self);
        let result_budget = result_budget.clone();
        owner
            .try_prepare_generated_gfx942_completion_v1(device, move |device| {
                artifact.revalidate(deadline).map_err(E::Remote)?;
                let prepared = artifact.prepare_storage(
                    arguments,
                    geometry,
                    timeout_milliseconds,
                    limits,
                    &result_budget,
                );
                // Close currentness even when an argument callback or preparation failed.
                artifact.revalidate(deadline).map_err(E::Remote)?;
                let (storage, footprint) = prepared?;
                let authority = ConditionalFillExecutionAuthorityV1 {
                    dispatch_contract: storage.prepared().dispatch_contract_sha256(),
                    device_unique_id: device.observation().unique_id(),
                    artifact,
                    deadline,
                };
                Ok(GeneratedRuntimeCarrierV1 {
                    storage,
                    authority,
                    footprint,
                    result_budget,
                })
            })
            .map_err(E::Engine)
    }

    fn prepare_storage<A: CompilerGeneratedRuntimeArguments<K>>(
        &self,
        arguments: A,
        geometry: AqlDispatchGeometryV1,
        timeout_milliseconds: u32,
        limits: GeneratedRuntimeArgumentLimitsV1,
        result_budget: &GeneratedRuntimeResultBudgetV1,
    ) -> Result<
        (
            GeneratedRuntimeStorageV1<fe2o3_runtime::GeneratedGfx942PersistentStorageV1>,
            crate::GeneratedRuntimeArgumentFootprintV1,
        ),
        WorkerV3ConditionalFillInvocationErrorV1,
    > {
        use WorkerV3ConditionalFillInvocationErrorV1 as E;
        let packed = pack::<K, A>(
            &self.inputs,
            self.admission.outer_handoff(),
            arguments,
            geometry,
            limits,
            result_budget,
        )?;
        if packed.kernel_id() != self.admission.descriptor().kernel_id() {
            return Err(E::PreparedBinding("packed kernel"));
        }
        let parts = packed.into_runtime_inputs(geometry, 0, timeout_milliseconds);
        let hsaco = self.current.exact_artifact_bytes();
        let storage = parts
            .storage
            .prepare(hsaco, K::EXPORT_NAME)
            .map_err(E::Preparation)?;
        self.check_prepared(storage.prepared())?;
        let storage = storage
            .project_conditional_fill(hsaco)
            .map_err(E::Projection)?;
        Ok((storage, parts.footprint))
    }

    fn check_prepared(
        &self,
        prepared: &PreparedGfx942RuntimeDispatchV1,
    ) -> Result<(), WorkerV3ConditionalFillInvocationErrorV1> {
        check_prepared_binding(
            prepared,
            self.current.exact_artifact_bytes(),
            K::EXPORT_NAME,
            self.admission.descriptor_binding(),
            *self.finalizer.finalized_hsaco_identity().sha256(),
            self.finalizer.finalized_hsaco_identity().byte_len(),
        )
    }
}

fn pack<K: CompilerGeneratedKernelExpectationV1, A: CompilerGeneratedRuntimeArguments<K>>(
    inputs: &fe2o3_verifier::ValidatedConditionalCompilerProofInputsV1,
    handoff: &fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3,
    arguments: A,
    geometry: AqlDispatchGeometryV1,
    limits: GeneratedRuntimeArgumentLimitsV1,
    result_budget: &GeneratedRuntimeResultBudgetV1,
) -> Result<crate::GeneratedRuntimeChargedArgumentsV1, WorkerV3ConditionalFillInvocationErrorV1> {
    use WorkerV3ConditionalFillInvocationErrorV1 as E;
    check_geometry(geometry, 0)?;
    // One generated layout is shared by packing and conditional coverage.
    let generated = A::generated_argument_layout()
        .map_err(GeneratedKfdPrepareError::GeneratedLayout)
        .map_err(GeneratedRuntimeArgumentErrorV1::Prepare)
        .map_err(E::Arguments)?;
    let binding = ConditionalOutputArgumentBindingV1::from_handoff(inputs, handoff, &generated)
        .map_err(E::Coverage)?;
    let plan = binding.packing_plan();
    let packed = prepare_charged_with_plan(
        arguments,
        plan,
        limits,
        result_budget,
        A::account_runtime_arguments,
        |arguments, budget| arguments.bind_runtime_arguments(plan, budget),
    )
    .map_err(E::Arguments)?;
    let coverage = packed
        .check_conditional_coverage(&binding, geometry)
        .map_err(E::Coverage)?;
    if coverage.output_elements() == 0 {
        return Err(E::Profile);
    }
    Ok(packed)
}

fn check_prepared_binding(
    prepared: &PreparedGfx942RuntimeDispatchV1,
    hsaco: &[u8],
    name: &str,
    expected_binding: fe2o3_hsaco::KernelDescriptorBinding,
    expected_sha256: [u8; 32],
    expected_length: u64,
) -> Result<(), WorkerV3ConditionalFillInvocationErrorV1> {
    use WorkerV3ConditionalFillInvocationErrorV1 as E;
    let closure = validate(hsaco, AdmittedProfile::Gfx942XnackOffCov6)
        .map_err(|_| E::PreparedBinding("loader envelope"))?
        .bind_kernel(name)
        .map_err(|_| E::PreparedBinding("loader kernel"))?;
    let binding = closure.selected_binding();
    let offset = binding
        .descriptor_address()
        .checked_sub(closure.envelope().plan().image_start());
    if binding != expected_binding
        || prepared.identity() != closure.identity_inputs()
        || Some(prepared.descriptor_offset()) != offset
        || prepared.identity().object_sha256() != expected_sha256
        || prepared.finalized_hsaco_length() != expected_length
        || prepared.kernel_name() != name
    {
        return Err(E::PreparedBinding("selected finalized entry"));
    }
    Ok(())
}

fn check_geometry(
    geometry: AqlDispatchGeometryV1,
    dynamic_group_segment_bytes: u32,
) -> Result<(), WorkerV3ConditionalFillInvocationErrorV1> {
    let grid = geometry.grid();
    if dynamic_group_segment_bytes != 0
        || geometry.workgroup() != [64, 1, 1]
        || grid[0] == 0
        || !grid[0].is_multiple_of(64)
        || grid[1..] != [1, 1]
    {
        return Err(WorkerV3ConditionalFillInvocationErrorV1::Profile);
    }
    Ok(())
}

struct ConditionalFillExecutionAuthorityV1<K> {
    artifact: Arc<RemoteConditionalFillArtifactV1<K>>,
    dispatch_contract: [u8; 32],
    device_unique_id: u64,
    deadline: Instant,
}

impl<K: CompilerGeneratedKernelExpectationV1> GeneratedRuntimeAuthorityV1
    for ConditionalFillExecutionAuthorityV1<K>
{
    fn artifact_bytes(&self) -> &[u8] {
        self.artifact.current.exact_artifact_bytes()
    }
}

// SAFETY: only the private preparation above constructs this owner. The original compiler
// audit and authenticated live proof custody are shared, not reconstructed. Actual charged
// packing discharges the retained conditional obligation, and the same packed owner becomes
// the checked finalized entry's projection. Its mandatory conditional-fill flag additionally
// requires native full64 coverage, the actual patched kernarg, whole coherent output, original
// memory session and first dispatch generation before publication. Runtime binds the exact
// contract and selected device and retains the complete carrier through settlement/quarantine.
unsafe impl<K: CompilerGeneratedKernelExpectationV1> WorkerV3Gfx942ExecutionAuthorityV1
    for ConditionalFillExecutionAuthorityV1<K>
{
    type CurrentnessError = WorkerV3RemoteConditionalFillErrorV1;

    fn finalized_hsaco_sha256(&self) -> [u8; 32] {
        *self.artifact.finalizer.finalized_hsaco_identity().sha256()
    }
    fn finalized_hsaco_length(&self) -> u64 {
        self.artifact
            .finalizer
            .finalized_hsaco_identity()
            .byte_len()
    }
    fn kernel_name(&self) -> &str {
        K::EXPORT_NAME
    }
    fn dispatch_contract_sha256(&self) -> [u8; 32] {
        self.dispatch_contract
    }
    fn device_unique_id(&self) -> u64 {
        self.device_unique_id
    }
    fn revalidate_currentness(&self) -> Result<(), Self::CurrentnessError> {
        self.artifact.revalidate(self.deadline)
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV3ConditionalFillInvocationErrorV1 {
    Remote(WorkerV3RemoteConditionalFillErrorV1),
    Arguments(GeneratedRuntimeArgumentErrorV1),
    Coverage(ConditionalPackedCoverageErrorV1),
    Preparation(Gfx942RuntimePreparationErrorV1),
    Projection(Gfx942RuntimeProjectionErrorV1),
    PreparedBinding(&'static str),
    Profile,
    Engine(RuntimeAsyncEngineCallErrorV1),
}

impl fmt::Display for WorkerV3ConditionalFillInvocationErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "conditional fill invocation rejected: {self:?}")
    }
}
impl Error for WorkerV3ConditionalFillInvocationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Remote(error) => Some(error),
            Self::Arguments(error) => Some(error),
            Self::Coverage(error) => Some(error),
            Self::Preparation(error) => Some(error),
            Self::Projection(error) => Some(error),
            Self::Engine(error) => Some(error),
            Self::PreparedBinding(_) | Self::Profile => None,
        }
    }
}

#[cfg(test)]
mod tests;
