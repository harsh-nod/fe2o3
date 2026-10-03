//! Compile-only concrete generated arguments; no executable or device is fabricated.

use crate::transform_gpu::{Marker, RuntimeArguments};
use gpu_host::{
    AuthenticatedWorkerV3ExecutableV1, GeneratedRuntimeArgumentLimitsV1,
    GeneratedRuntimeResultBudgetV1, GeneratedWorkerV3ContextInvocationV1,
};
use gpu_runtime::{
    KfdMultiDeviceRuntimeBackendV1, KfdRuntimeBackendV1, RuntimeAsyncProgressHandleV1,
    RuntimeContextV1, RuntimeDeviceIdV1,
};

pub fn prepare_single(
    executable: AuthenticatedWorkerV3ExecutableV1<Marker>,
    arguments: RuntimeArguments,
    context: &mut RuntimeContextV1<KfdRuntimeBackendV1>,
    device: RuntimeDeviceIdV1,
    geometry: gpu_host::AqlDispatchGeometryV1,
    budget: &GeneratedRuntimeResultBudgetV1,
) -> Result<GeneratedWorkerV3ContextInvocationV1<Marker>, Box<dyn std::error::Error>> {
    let invocation = executable.prepare_generated_context_invocation(
        arguments,
        context,
        device,
        geometry,
        0,
        1000,
        GeneratedRuntimeArgumentLimitsV1::new(4096, 4096, 3),
        budget,
    )?;
    let _ = context.devices();
    invocation.validate_context(context)?;
    Ok(invocation)
}

pub fn prepare_multi(
    executable: AuthenticatedWorkerV3ExecutableV1<Marker>,
    arguments: RuntimeArguments,
    context: &mut RuntimeContextV1<KfdMultiDeviceRuntimeBackendV1>,
    device: RuntimeDeviceIdV1,
    geometry: gpu_host::AqlDispatchGeometryV1,
    budget: &GeneratedRuntimeResultBudgetV1,
) -> Result<GeneratedWorkerV3ContextInvocationV1<Marker>, Box<dyn std::error::Error>> {
    let invocation = executable.prepare_generated_multi_context_invocation(
        arguments,
        context,
        device,
        geometry,
        0,
        1000,
        GeneratedRuntimeArgumentLimitsV1::new(4096, 4096, 3),
        budget,
    )?;
    let _ = context.devices();
    invocation.validate_multi_context(context)?;
    Ok(invocation)
}

pub fn prepare_multi_async(
    executable: AuthenticatedWorkerV3ExecutableV1<Marker>,
    arguments: RuntimeArguments,
    owner: &RuntimeAsyncProgressHandleV1<KfdMultiDeviceRuntimeBackendV1>,
    device: RuntimeDeviceIdV1,
    geometry: gpu_host::AqlDispatchGeometryV1,
    budget: &GeneratedRuntimeResultBudgetV1,
) -> Result<
    gpu_runtime::RuntimeAsyncPreparationV1<gpu_host::GeneratedWorkerV3ContextInvocationErrorV1>,
    gpu_host::GeneratedWorkerV3RuntimeInvocationErrorV1,
> {
    executable.prepare_generated_multi_context_invocation_async(
        arguments,
        owner,
        device,
        geometry,
        0,
        1000,
        GeneratedRuntimeArgumentLimitsV1::new(4096, 4096, 3),
        budget,
    )
}

pub fn prepare_single_async(
    executable: AuthenticatedWorkerV3ExecutableV1<Marker>,
    arguments: RuntimeArguments,
    owner: &RuntimeAsyncProgressHandleV1<KfdRuntimeBackendV1>,
    device: RuntimeDeviceIdV1,
    geometry: gpu_host::AqlDispatchGeometryV1,
    budget: &GeneratedRuntimeResultBudgetV1,
) -> Result<
    gpu_runtime::RuntimeAsyncPreparationV1<gpu_host::GeneratedWorkerV3ContextInvocationErrorV1>,
    gpu_host::GeneratedWorkerV3RuntimeInvocationErrorV1,
> {
    executable.prepare_generated_context_invocation_async(
        arguments,
        owner,
        device,
        geometry,
        0,
        1000,
        GeneratedRuntimeArgumentLimitsV1::new(4096, 4096, 3),
        budget,
    )
}
