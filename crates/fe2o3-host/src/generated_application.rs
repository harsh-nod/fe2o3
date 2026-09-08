//! Target-neutral generated application preparation and execution.

use std::{error::Error, fmt, time::Duration};

use fe2o3_amd_target::{
    AmdTargetId, PRODUCTION_GFX942_DEVICE_TARGET_V1, PRODUCTION_GFX950_DEVICE_TARGET_V1,
};
use fe2o3_aql::AqlDispatchGeometryV1;

use crate::generated_kfd_invocation::GeneratedWorkerV3CapabilityKfdInvocation;
use crate::{
    AuthenticatedWorkerV3CapabilityApplicationV1, CapabilityGeneratedHostAdmissionErrorV1,
    CompilerGeneratedHostArgumentsV2, CompilerGeneratedKernelExpectationRosterV1,
    CompilerGeneratedKernelExpectationV2, CompilerGeneratedKfdArguments,
    GeneratedHostLaunchGeometryV2, GeneratedHostPrepareErrorV2,
};

/// Compiler-emitted argument custody accepted by every generated application provider.
///
/// This implementation SPI combines the canonical ABI packer with the generated dynamic host
/// contract. It adds no caller-defined contract surface.
#[doc(hidden)]
pub trait CompilerGeneratedApplicationArgumentsV1<'allocation, K>:
    CompilerGeneratedKfdArguments<'allocation, K> + CompilerGeneratedHostArgumentsV2<'allocation, K>
where
    K: CompilerGeneratedKernelExpectationV2,
{
}

impl<'allocation, K, Arguments> CompilerGeneratedApplicationArgumentsV1<'allocation, K>
    for Arguments
where
    K: CompilerGeneratedKernelExpectationV2,
    Arguments: CompilerGeneratedKfdArguments<'allocation, K>
        + CompilerGeneratedHostArgumentsV2<'allocation, K>,
{
}

/// Runtime provider used by one completed generated application dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GeneratedApplicationBackendV1 {
    ProtectedDirectKfd,
    ReviewedHipQualification,
}

/// Redacted, target-neutral completion result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedApplicationDispatchResultV1 {
    target: AmdTargetId,
    backend: GeneratedApplicationBackendV1,
    kernel_name: Box<str>,
    completion_elapsed: Duration,
    native_submission_id: Option<u64>,
    native_queue_id: Option<u32>,
}

impl GeneratedApplicationDispatchResultV1 {
    pub const fn target(&self) -> AmdTargetId {
        self.target
    }

    pub const fn backend(&self) -> GeneratedApplicationBackendV1 {
        self.backend
    }

    pub fn kernel_name(&self) -> &str {
        &self.kernel_name
    }

    pub const fn completion_elapsed(&self) -> Duration {
        self.completion_elapsed
    }

    pub const fn native_submission_id(&self) -> Option<u64> {
        self.native_submission_id
    }

    pub const fn native_queue_id(&self) -> Option<u32> {
        self.native_queue_id
    }
}

/// Failure while opening or using the provider selected from authenticated target custody.
#[derive(Debug)]
#[non_exhaustive]
pub enum GeneratedApplicationProviderErrorV1 {
    Unavailable {
        target: AmdTargetId,
    },
    Backend(Box<str>),
    TargetObservation {
        artifact: AmdTargetId,
        observed: AmdTargetId,
    },
    DeviceResource(&'static str),
    ArtifactSubstitution,
    KernargSubstitution(&'static str),
    ReadOnlyMutation {
        buffer_index: usize,
    },
}

impl fmt::Display for GeneratedApplicationProviderErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable { target } => {
                write!(
                    formatter,
                    "no generated application provider is enabled for {target}"
                )
            }
            Self::Backend(error) => write!(formatter, "target provider failed: {error}"),
            Self::TargetObservation { artifact, observed } => write!(
                formatter,
                "authenticated artifact target {artifact} is incompatible with observed device target {observed}"
            ),
            Self::DeviceResource(resource) => {
                write!(formatter, "observed device rejects generated {resource}")
            }
            Self::ArtifactSubstitution => {
                formatter.write_str("authenticated artifact bytes changed before provider loading")
            }
            Self::KernargSubstitution(field) => {
                write!(formatter, "generated kernarg {field} was substituted")
            }
            Self::ReadOnlyMutation { buffer_index } => write!(
                formatter,
                "provider observed mutation of read-only buffer {buffer_index}"
            ),
        }
    }
}

impl Error for GeneratedApplicationProviderErrorV1 {}

/// Failure before a target-neutral generated invocation is ready to execute.
#[derive(Debug)]
#[non_exhaustive]
pub enum GeneratedApplicationPrepareErrorV1 {
    Contract(CapabilityGeneratedHostAdmissionErrorV1),
    Dynamic(GeneratedHostPrepareErrorV2),
    UnsupportedTarget { target: AmdTargetId },
    Provider(GeneratedApplicationProviderErrorV1),
}

impl fmt::Display for GeneratedApplicationPrepareErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract(error) => write!(formatter, "generated host admission failed: {error}"),
            Self::Dynamic(error) => {
                write!(formatter, "generated dynamic admission failed: {error}")
            }
            Self::UnsupportedTarget { target } => {
                write!(
                    formatter,
                    "authenticated target {target} has no reviewed provider"
                )
            }
            Self::Provider(error) => error.fmt(formatter),
        }
    }
}

impl Error for GeneratedApplicationPrepareErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Contract(error) => Some(error),
            Self::Dynamic(error) => Some(error),
            Self::Provider(error) => Some(error),
            Self::UnsupportedTarget { .. } => None,
        }
    }
}

/// Failure after a checked generated invocation has consumed its retained borrows.
#[derive(Debug)]
#[non_exhaustive]
pub enum GeneratedApplicationExecutionErrorV1 {
    Provider(GeneratedApplicationProviderErrorV1),
    CurrentPublication(crate::RecoveredWorkerV3AdmissionErrorV1),
}

impl fmt::Display for GeneratedApplicationExecutionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Provider(error) => error.fmt(formatter),
            Self::CurrentPublication(error) => {
                write!(
                    formatter,
                    "artifact publication changed before completion: {error}"
                )
            }
        }
    }
}

impl Error for GeneratedApplicationExecutionErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Provider(error) => Some(error),
            Self::CurrentPublication(error) => Some(error),
        }
    }
}

mod provider {
    use super::*;

    pub(super) trait SealedGeneratedApplicationTargetProviderV1 {
        const TARGET: &'static str;
        const BACKEND: GeneratedApplicationBackendV1;
    }

    pub(super) struct Gfx942DirectKfdProviderV1;

    impl SealedGeneratedApplicationTargetProviderV1 for Gfx942DirectKfdProviderV1 {
        const TARGET: &'static str = PRODUCTION_GFX942_DEVICE_TARGET_V1;
        const BACKEND: GeneratedApplicationBackendV1 =
            GeneratedApplicationBackendV1::ProtectedDirectKfd;
    }

    pub(super) struct Gfx950HipQualificationProviderV1;

    impl SealedGeneratedApplicationTargetProviderV1 for Gfx950HipQualificationProviderV1 {
        const TARGET: &'static str = PRODUCTION_GFX950_DEVICE_TARGET_V1;
        const BACKEND: GeneratedApplicationBackendV1 =
            GeneratedApplicationBackendV1::ReviewedHipQualification;
    }
}

use provider::{
    Gfx942DirectKfdProviderV1, Gfx950HipQualificationProviderV1,
    SealedGeneratedApplicationTargetProviderV1,
};

const MAX_GENERATED_APPLICATION_TIMEOUT_MILLISECONDS_V1: u32 = 60_000;

/// Move-only invocation selected from the authenticated target, never from caller data.
#[must_use = "a generated invocation retains application and allocation borrows until completion"]
pub struct GeneratedApplicationInvocation<'application, 'allocation, R, K> {
    target: AmdTargetId,
    backend: GeneratedApplicationInvocationBackend<'application, 'allocation, R, K>,
}

enum GeneratedApplicationInvocationBackend<'application, 'allocation, R, K> {
    DirectKfd(GeneratedWorkerV3CapabilityKfdInvocation<'application, 'allocation, R, K>),
    #[cfg(feature = "generated-gfx950-hip-provider")]
    Hip(GeneratedHipQualificationInvocation<'application, 'allocation, R, K>),
}

impl<R, K> fmt::Debug for GeneratedApplicationInvocation<'_, '_, R, K>
where
    R: CompilerGeneratedKernelExpectationRosterV1,
    K: CompilerGeneratedKernelExpectationV2,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedApplicationInvocation")
            .field("target", &self.target)
            .field("backend", &self.backend())
            .finish_non_exhaustive()
    }
}

impl<R, K> GeneratedApplicationInvocation<'_, '_, R, K>
where
    R: CompilerGeneratedKernelExpectationRosterV1,
    K: CompilerGeneratedKernelExpectationV2,
{
    pub const fn target(&self) -> AmdTargetId {
        self.target
    }

    pub const fn backend(&self) -> GeneratedApplicationBackendV1 {
        match &self.backend {
            GeneratedApplicationInvocationBackend::DirectKfd(_) => {
                GeneratedApplicationBackendV1::ProtectedDirectKfd
            }
            #[cfg(feature = "generated-gfx950-hip-provider")]
            GeneratedApplicationInvocationBackend::Hip(_) => {
                GeneratedApplicationBackendV1::ReviewedHipQualification
            }
        }
    }

    pub fn execute(
        self,
    ) -> Result<GeneratedApplicationDispatchResultV1, GeneratedApplicationExecutionErrorV1> {
        let target = self.target;
        match self.backend {
            GeneratedApplicationInvocationBackend::DirectKfd(invocation) => {
                let kernel_name = invocation.kernel_name().into();
                let result = invocation
                    .execute()
                    .map_err(|error| provider_backend_error(&error))
                    .map_err(GeneratedApplicationExecutionErrorV1::Provider)?;
                Ok(GeneratedApplicationDispatchResultV1 {
                    target,
                    backend: GeneratedApplicationBackendV1::ProtectedDirectKfd,
                    kernel_name,
                    completion_elapsed: result.completion_elapsed(),
                    native_submission_id: Some(result.packet_id()),
                    native_queue_id: Some(result.queue_id()),
                })
            }
            #[cfg(feature = "generated-gfx950-hip-provider")]
            GeneratedApplicationInvocationBackend::Hip(invocation) => {
                GeneratedHipQualificationInvocation::<R, K>::execute(invocation)
            }
        }
    }
}

impl<R> AuthenticatedWorkerV3CapabilityApplicationV1<R>
where
    R: CompilerGeneratedKernelExpectationRosterV1,
{
    /// Prepares one generated kernel through the provider fixed by authenticated target custody.
    ///
    /// Application code supplies only compiler-generated borrowed arguments and dynamic launch
    /// values. Static ABI, memory roles, artifact bytes, kernel identity, target, and provider are
    /// recovered from the exact V2/V5 application handoff.
    pub fn prepare_generated_application_invocation_v1<'application, 'allocation, K, Arguments>(
        &'application self,
        arguments: Arguments,
        geometry: AqlDispatchGeometryV1,
        dynamic_group_segment_bytes: u32,
        timeout_milliseconds: u32,
    ) -> Result<
        GeneratedApplicationInvocation<'application, 'allocation, R, K>,
        GeneratedApplicationPrepareErrorV1,
    >
    where
        K: CompilerGeneratedKernelExpectationV2,
        Arguments: CompilerGeneratedApplicationArgumentsV1<'allocation, K> + 'allocation,
    {
        let contract = self
            .admit_generated_host_contract_v2::<K>()
            .map_err(GeneratedApplicationPrepareErrorV1::Contract)?;
        let host_geometry = GeneratedHostLaunchGeometryV2::new(
            geometry.grid(),
            geometry.workgroup().map(u32::from),
            dynamic_group_segment_bytes,
        );
        contract
            .preflight_dynamic_v2(host_geometry, &arguments)
            .map_err(GeneratedApplicationPrepareErrorV1::Dynamic)?;
        if timeout_milliseconds == 0
            || timeout_milliseconds > MAX_GENERATED_APPLICATION_TIMEOUT_MILLISECONDS_V1
        {
            return Err(GeneratedApplicationPrepareErrorV1::Provider(
                GeneratedApplicationProviderErrorV1::DeviceResource("completion timeout"),
            ));
        }

        let target = self.roster().target();
        if provider_matches::<Gfx942DirectKfdProviderV1>(target) {
            let device = crate::production_application::open_default_gfx942_device_v1()
                .map_err(|error| provider_backend_error(&error))
                .map_err(GeneratedApplicationPrepareErrorV1::Provider)?;
            let invocation = contract
                .prepare_capability_direct_kfd_invocation_v2(
                    self,
                    arguments,
                    device,
                    geometry,
                    dynamic_group_segment_bytes,
                    timeout_milliseconds,
                )
                .map_err(|error| provider_backend_error(&error))
                .map_err(GeneratedApplicationPrepareErrorV1::Provider)?;
            return Ok(GeneratedApplicationInvocation {
                target,
                backend: GeneratedApplicationInvocationBackend::DirectKfd(invocation),
            });
        }
        if provider_matches::<Gfx950HipQualificationProviderV1>(target) {
            #[cfg(feature = "generated-gfx950-hip-provider")]
            {
                let invocation = GeneratedHipQualificationInvocation::prepare(
                    self,
                    &contract,
                    arguments,
                    geometry,
                    dynamic_group_segment_bytes,
                    timeout_milliseconds,
                )?;
                return Ok(GeneratedApplicationInvocation {
                    target,
                    backend: GeneratedApplicationInvocationBackend::Hip(invocation),
                });
            }
            #[cfg(not(feature = "generated-gfx950-hip-provider"))]
            {
                let _ = arguments;
                return Err(GeneratedApplicationPrepareErrorV1::Provider(
                    GeneratedApplicationProviderErrorV1::Unavailable { target },
                ));
            }
        }
        Err(GeneratedApplicationPrepareErrorV1::UnsupportedTarget { target })
    }
}

fn provider_matches<P: SealedGeneratedApplicationTargetProviderV1>(target: AmdTargetId) -> bool {
    let expected = AmdTargetId::parse(P::TARGET).expect("sealed provider target is canonical");
    let _ = P::BACKEND;
    target == expected
}

fn provider_backend_error(error: &impl fmt::Display) -> GeneratedApplicationProviderErrorV1 {
    GeneratedApplicationProviderErrorV1::Backend(error.to_string().into())
}

#[cfg(feature = "generated-gfx950-hip-provider")]
mod hip {
    use std::alloc::{Layout, alloc_zeroed, dealloc};
    use std::ffi::c_void;
    use std::marker::PhantomData;
    use std::ptr::NonNull;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Instant;

    use fe2o3_core::{DeviceBuffer, Event, GpuContext, GpuFunction, Stream};
    use fe2o3_kfd::Gfx942KfdDispatchPointerFixupV1;
    use fe2o3_runtime::Gfx942RuntimeBufferAccessV1;
    use sha2::{Digest, Sha256};

    use super::*;
    use crate::GeneratedKfdCompletion;
    use crate::generated_host_contract_v2::CheckedGeneratedHostDispatchV2;
    use crate::generated_kfd_arguments::GeneratedTargetBackendArgumentsV1;
    use crate::generated_kfd_invocation::prepare_capability_target_arguments_v2;

    const CONTEXT_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/HOST/GENERATED-APPLICATION-HIP-CONTEXT/V1\0";
    const STREAM_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/HOST/GENERATED-APPLICATION-HIP-STREAM/V1\0";
    const POINTER_BYTES: usize = size_of::<u64>();
    const MAX_BACKEND_ALIGNMENT: usize = 4096;
    const HIP_LAUNCH_PARAM_BUFFER_POINTER: *mut c_void = 1_usize as *mut c_void;
    const HIP_LAUNCH_PARAM_BUFFER_SIZE: *mut c_void = 2_usize as *mut c_void;
    const HIP_LAUNCH_PARAM_END: *mut c_void = 3_usize as *mut c_void;
    static NEXT_CONTEXT_IDENTITY: AtomicU64 = AtomicU64::new(1);
    static NEXT_STREAM_IDENTITY: AtomicU64 = AtomicU64::new(1);

    pub(super) struct GeneratedHipQualificationInvocation<'application, 'allocation, R, K> {
        application: &'application AuthenticatedWorkerV3CapabilityApplicationV1<R>,
        target: AmdTargetId,
        context: Arc<GpuContext>,
        stream: Stream,
        function: GpuFunction,
        kernarg: AlignedKernargV1,
        buffers: Vec<DeviceBuffer<u8>>,
        policies: Vec<HipBufferPolicyV1>,
        completion: GeneratedKfdCompletion<'allocation>,
        grid_blocks: [u32; 3],
        workgroup: [u32; 3],
        dynamic_group_segment_bytes: u32,
        timeout: Duration,
        _checked: CheckedGeneratedHostDispatchV2<K>,
        _roster: PhantomData<fn() -> R>,
    }

    struct HipBufferPolicyV1 {
        access: Gfx942RuntimeBufferAccessV1,
        initial: Vec<u8>,
    }

    impl<'application, 'allocation, R, K>
        GeneratedHipQualificationInvocation<'application, 'allocation, R, K>
    where
        R: CompilerGeneratedKernelExpectationRosterV1,
        K: CompilerGeneratedKernelExpectationV2,
    {
        #[allow(clippy::too_many_arguments)]
        pub(super) fn prepare<Arguments>(
            application: &'application AuthenticatedWorkerV3CapabilityApplicationV1<R>,
            contract: &crate::AdmittedGeneratedHostContractV2<K>,
            arguments: Arguments,
            geometry: AqlDispatchGeometryV1,
            dynamic_group_segment_bytes: u32,
            timeout_milliseconds: u32,
        ) -> Result<Self, GeneratedApplicationPrepareErrorV1>
        where
            Arguments: CompilerGeneratedApplicationArgumentsV1<'allocation, K> + 'allocation,
        {
            let target = application.roster().target();
            let context = GpuContext::new(0)
                .map_err(|error| provider_backend_error(&error))
                .map_err(GeneratedApplicationPrepareErrorV1::Provider)?;
            let observed = context
                .observe_target()
                .map_err(|error| provider_backend_error(&error))
                .map_err(GeneratedApplicationPrepareErrorV1::Provider)?;
            if !target.is_compatible_with_observed(&observed.target_id())
                || observed.hip_default_warp_size() != 64
            {
                return Err(GeneratedApplicationPrepareErrorV1::Provider(
                    GeneratedApplicationProviderErrorV1::TargetObservation {
                        artifact: target,
                        observed: observed.target_id(),
                    },
                ));
            }
            validate_device_geometry::<R, K>(
                application,
                &observed,
                geometry,
                dynamic_group_segment_bytes,
            )?;
            let stream = context
                .create_stream()
                .map_err(|error| provider_backend_error(&error))
                .map_err(GeneratedApplicationPrepareErrorV1::Provider)?;
            let runtime = hip_runtime_coordinates::<K>(contract, observed.target_id(), geometry)?;
            let (checked, packed) = prepare_capability_target_arguments_v2(
                contract,
                application,
                arguments,
                runtime,
                geometry,
                dynamic_group_segment_bytes,
            )
            .map_err(|error| provider_backend_error(&error))
            .map_err(GeneratedApplicationPrepareErrorV1::Provider)?;
            let parts = packed.into_target_backend_arguments_v1();
            validate_fixups(&parts)?;
            let image = application.roster().exact_current_hsaco_bytes();
            let verification = application.roster().verification();
            if u64::try_from(image.len()).ok() != Some(verification.finalized_hsaco_length())
                || <[u8; 32]>::from(Sha256::digest(image)) != verification.finalized_hsaco_sha256()
            {
                return Err(GeneratedApplicationPrepareErrorV1::Provider(
                    GeneratedApplicationProviderErrorV1::ArtifactSubstitution,
                ));
            }
            application
                .revalidate_currentness()
                .map_err(|error| provider_backend_error(&error))
                .map_err(GeneratedApplicationPrepareErrorV1::Provider)?;
            let (kernarg, buffers, policies, completion) = materialize_arguments(parts, &stream)?;
            // SAFETY: the authenticated carrier supplies these exact bytes; generated ABI packing,
            // target compatibility, resources, and retained memory capabilities were checked above.
            let module = unsafe { context.load_module_from_bytes_unchecked(image) }
                .map_err(|error| provider_backend_error(&error))
                .map_err(GeneratedApplicationPrepareErrorV1::Provider)?;
            let function = module
                .load_function(K::EXPORT_NAME)
                .map_err(|error| provider_backend_error(&error))
                .map_err(GeneratedApplicationPrepareErrorV1::Provider)?;
            application
                .revalidate_currentness()
                .map_err(|error| provider_backend_error(&error))
                .map_err(GeneratedApplicationPrepareErrorV1::Provider)?;

            Ok(Self {
                application,
                target,
                context,
                stream,
                function,
                kernarg,
                buffers,
                policies,
                completion,
                grid_blocks: grid_blocks(geometry)?,
                workgroup: geometry.workgroup().map(u32::from),
                dynamic_group_segment_bytes,
                timeout: Duration::from_millis(u64::from(timeout_milliseconds)),
                _checked: checked,
                _roster: PhantomData,
            })
        }

        pub(super) fn execute(
            self,
        ) -> Result<GeneratedApplicationDispatchResultV1, GeneratedApplicationExecutionErrorV1>
        {
            let Self {
                application,
                target,
                context,
                stream,
                function,
                kernarg,
                buffers,
                policies,
                completion,
                grid_blocks,
                workgroup,
                dynamic_group_segment_bytes,
                timeout,
                _checked,
                _roster: _,
            } = self;
            let started = Instant::now();
            let mut terminal = Event::new(&context)
                .map_err(|error| provider_backend_error(&error))
                .map_err(GeneratedApplicationExecutionErrorV1::Provider)?;
            let launch = launch_packed(
                &function,
                &stream,
                &kernarg,
                grid_blocks,
                workgroup,
                dynamic_group_segment_bytes,
            );
            if let Err(error) = launch {
                recover_or_abort(&stream);
                return Err(GeneratedApplicationExecutionErrorV1::Provider(error));
            }
            if let Err(error) = terminal.record(&stream) {
                recover_or_abort(&stream);
                return Err(GeneratedApplicationExecutionErrorV1::Provider(
                    provider_backend_error(&error),
                ));
            }
            loop {
                match terminal.query() {
                    Ok(true) => break,
                    Ok(false) if started.elapsed() < timeout => std::thread::yield_now(),
                    Ok(false) => std::process::abort(),
                    Err(error) => {
                        recover_or_abort(&stream);
                        return Err(GeneratedApplicationExecutionErrorV1::Provider(
                            provider_backend_error(&error),
                        ));
                    }
                }
            }
            let completion_elapsed = started.elapsed();
            let completed = buffers
                .iter()
                .map(|buffer| buffer.to_host_vec(&stream))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| provider_backend_error(&error))
                .map_err(GeneratedApplicationExecutionErrorV1::Provider)?;
            for (buffer_index, (policy, bytes)) in policies.iter().zip(&completed).enumerate() {
                if policy.access == Gfx942RuntimeBufferAccessV1::ReadOnly
                    && policy.initial != *bytes
                {
                    return Err(GeneratedApplicationExecutionErrorV1::Provider(
                        GeneratedApplicationProviderErrorV1::ReadOnlyMutation { buffer_index },
                    ));
                }
            }
            application
                .revalidate_currentness()
                .map_err(GeneratedApplicationExecutionErrorV1::CurrentPublication)?;
            let views = policies
                .iter()
                .zip(&completed)
                .map(|(policy, bytes)| (policy.access, bytes.as_slice()))
                .collect::<Vec<_>>();
            completion
                .apply_completed_buffers(&views)
                .map_err(|error| provider_backend_error(&error))
                .map_err(GeneratedApplicationExecutionErrorV1::Provider)?;
            Ok(GeneratedApplicationDispatchResultV1 {
                target,
                backend: GeneratedApplicationBackendV1::ReviewedHipQualification,
                kernel_name: K::EXPORT_NAME.into(),
                completion_elapsed,
                native_submission_id: None,
                native_queue_id: None,
            })
        }
    }

    fn validate_device_geometry<R, K>(
        application: &AuthenticatedWorkerV3CapabilityApplicationV1<R>,
        observed: &fe2o3_core::ObservedDeviceTarget,
        geometry: AqlDispatchGeometryV1,
        dynamic_group_segment_bytes: u32,
    ) -> Result<(), GeneratedApplicationPrepareErrorV1>
    where
        R: CompilerGeneratedKernelExpectationRosterV1,
        K: CompilerGeneratedKernelExpectationV2,
    {
        let workgroup = geometry.workgroup().map(u32::from);
        let flat = workgroup.into_iter().try_fold(1_u32, u32::checked_mul);
        if flat.is_none_or(|flat| flat > observed.max_threads_per_block()) {
            return Err(GeneratedApplicationPrepareErrorV1::Provider(
                GeneratedApplicationProviderErrorV1::DeviceResource("flat workgroup size"),
            ));
        }
        if workgroup
            .iter()
            .zip(observed.max_block_dimensions())
            .any(|(actual, maximum)| *actual > maximum)
        {
            return Err(GeneratedApplicationPrepareErrorV1::Provider(
                GeneratedApplicationProviderErrorV1::DeviceResource("workgroup dimensions"),
            ));
        }
        let blocks = grid_blocks(geometry)?;
        if blocks
            .iter()
            .zip(observed.max_grid_dimensions())
            .any(|(actual, maximum)| *actual > maximum)
        {
            return Err(GeneratedApplicationPrepareErrorV1::Provider(
                GeneratedApplicationProviderErrorV1::DeviceResource("grid dimensions"),
            ));
        }
        let entry = application
            .roster()
            .entry::<K>()
            .map_err(|error| provider_backend_error(&error))
            .map_err(GeneratedApplicationPrepareErrorV1::Provider)?;
        let descriptor = entry.descriptor();
        let lds = u64::from(descriptor.launch().static_shared_memory_bytes())
            .checked_add(u64::from(dynamic_group_segment_bytes));
        let maximum_lds = observed
            .shared_memory_per_block_optin()
            .unwrap_or(observed.shared_memory_per_block());
        if lds.is_none_or(|lds| lds > maximum_lds) {
            return Err(GeneratedApplicationPrepareErrorV1::Provider(
                GeneratedApplicationProviderErrorV1::DeviceResource("LDS requirement"),
            ));
        }
        Ok(())
    }

    fn grid_blocks(
        geometry: AqlDispatchGeometryV1,
    ) -> Result<[u32; 3], GeneratedApplicationPrepareErrorV1> {
        let grid = geometry.grid();
        let workgroup = geometry.workgroup().map(u32::from);
        let mut blocks = [0; 3];
        for axis in 0..3 {
            if !grid[axis].is_multiple_of(workgroup[axis]) {
                return Err(GeneratedApplicationPrepareErrorV1::Provider(
                    GeneratedApplicationProviderErrorV1::DeviceResource(
                        "grid/workgroup divisibility",
                    ),
                ));
            }
            blocks[axis] = grid[axis] / workgroup[axis];
        }
        Ok(blocks)
    }

    fn hip_runtime_coordinates<K: CompilerGeneratedKernelExpectationV2>(
        contract: &crate::AdmittedGeneratedHostContractV2<K>,
        observed: AmdTargetId,
        geometry: AqlDispatchGeometryV1,
    ) -> Result<crate::GeneratedHostRuntimeCoordinatesV2, GeneratedApplicationPrepareErrorV1> {
        let context_nonce =
            NEXT_CONTEXT_IDENTITY.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            });
        let stream_nonce =
            NEXT_STREAM_IDENTITY.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            });
        let (context_nonce, stream_nonce) = match (context_nonce, stream_nonce) {
            (Ok(context), Ok(stream)) => (context, stream),
            _ => {
                return Err(GeneratedApplicationPrepareErrorV1::Provider(
                    GeneratedApplicationProviderErrorV1::Backend(
                        "process-local provider identity space exhausted".into(),
                    ),
                ));
            }
        };
        let target_model = contract.subject().target_model_identity();
        let mut context_hash = Sha256::new();
        context_hash.update(CONTEXT_IDENTITY_DOMAIN_V1);
        context_hash.update(target_model);
        context_hash.update(observed.to_string().as_bytes());
        context_hash.update(context_nonce.to_le_bytes());
        let context: [u8; 32] = context_hash.finalize().into();
        let mut stream_hash = Sha256::new();
        stream_hash.update(STREAM_IDENTITY_DOMAIN_V1);
        stream_hash.update(context);
        stream_hash.update(K::KERNEL_BINDING_ID_V1);
        stream_hash.update(stream_nonce.to_le_bytes());
        for value in geometry.grid() {
            stream_hash.update(value.to_le_bytes());
        }
        for value in geometry.workgroup() {
            stream_hash.update(value.to_le_bytes());
        }
        let stream: [u8; 32] = stream_hash.finalize().into();
        crate::GeneratedHostRuntimeCoordinatesV2::new(target_model, context, stream)
            .map_err(|error| provider_backend_error(&error))
            .map_err(GeneratedApplicationPrepareErrorV1::Provider)
    }

    fn validate_fixups(
        parts: &GeneratedTargetBackendArgumentsV1<'_>,
    ) -> Result<(), GeneratedApplicationPrepareErrorV1> {
        let mut offsets = Vec::with_capacity(parts.pointer_fixups.len());
        for fixup in &parts.pointer_fixups {
            let end = fixup
                .kernarg_offset()
                .checked_add(POINTER_BYTES)
                .ok_or_else(|| kernarg_error("pointer range overflow"))?;
            let buffer = parts
                .buffers
                .get(fixup.buffer_index())
                .ok_or_else(|| kernarg_error("pointer buffer index"))?;
            if end > parts.explicit_kernarg.len()
                || !fixup.kernarg_offset().is_multiple_of(POINTER_BYTES)
                || fixup.buffer_byte_offset() >= buffer.bytes().len()
                || fixup.required_alignment() == 0
                || fixup.required_alignment() > MAX_BACKEND_ALIGNMENT as u64
                || !fixup.required_alignment().is_power_of_two()
                || !(fixup.buffer_byte_offset() as u64).is_multiple_of(fixup.required_alignment())
                || parts.explicit_kernarg[fixup.kernarg_offset()..end]
                    .iter()
                    .any(|byte| *byte != 0)
            {
                return Err(kernarg_error("pointer fixup"));
            }
            offsets.push(fixup.kernarg_offset());
        }
        offsets.sort_unstable();
        if offsets.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(kernarg_error("duplicate pointer fixup"));
        }
        Ok(())
    }

    fn materialize_arguments<'allocation>(
        parts: GeneratedTargetBackendArgumentsV1<'allocation>,
        stream: &Stream,
    ) -> Result<
        (
            AlignedKernargV1,
            Vec<DeviceBuffer<u8>>,
            Vec<HipBufferPolicyV1>,
            GeneratedKfdCompletion<'allocation>,
        ),
        GeneratedApplicationPrepareErrorV1,
    > {
        let GeneratedTargetBackendArgumentsV1 {
            alignment,
            explicit_kernarg,
            buffers: inputs,
            pointer_fixups,
            completion,
        } = parts;
        let mut buffers = Vec::with_capacity(inputs.len());
        let mut policies = Vec::with_capacity(inputs.len());
        for input in inputs {
            let initial = input.bytes().to_vec();
            buffers.push(
                DeviceBuffer::from_host(stream, &initial)
                    .map_err(|error| provider_backend_error(&error))
                    .map_err(GeneratedApplicationPrepareErrorV1::Provider)?,
            );
            policies.push(HipBufferPolicyV1 {
                access: input.access(),
                initial,
            });
        }
        let alignment = pointer_fixups
            .iter()
            .map(|fixup| fixup.required_alignment() as usize)
            .max()
            .unwrap_or(POINTER_BYTES)
            .max(usize::try_from(alignment).map_err(|_| kernarg_error("alignment"))?)
            .max(POINTER_BYTES);
        let mut kernarg = AlignedKernargV1::new(&explicit_kernarg, alignment)?;
        for fixup in pointer_fixups {
            patch_pointer(&mut kernarg, fixup, &buffers)?;
        }
        Ok((kernarg, buffers, policies, completion))
    }

    fn patch_pointer(
        kernarg: &mut AlignedKernargV1,
        fixup: Gfx942KfdDispatchPointerFixupV1,
        buffers: &[DeviceBuffer<u8>],
    ) -> Result<(), GeneratedApplicationPrepareErrorV1> {
        let buffer = buffers
            .get(fixup.buffer_index())
            .ok_or_else(|| kernarg_error("materialized pointer buffer index"))?;
        // SAFETY: the allocation remains owned by `buffers` through terminal completion; the
        // resulting address is written into device kernarg storage and never dereferenced on host.
        let base = unsafe { buffer.raw_device_ptr() } as usize;
        let address = base
            .checked_add(fixup.buffer_byte_offset())
            .ok_or_else(|| kernarg_error("device pointer overflow"))?;
        if !(address as u64).is_multiple_of(fixup.required_alignment()) {
            return Err(kernarg_error("device pointer alignment"));
        }
        let end = fixup
            .kernarg_offset()
            .checked_add(POINTER_BYTES)
            .ok_or_else(|| kernarg_error("materialized pointer range"))?;
        kernarg
            .bytes_mut()
            .get_mut(fixup.kernarg_offset()..end)
            .ok_or_else(|| kernarg_error("materialized pointer range"))?
            .copy_from_slice(&(address as u64).to_le_bytes());
        Ok(())
    }

    fn launch_packed(
        function: &GpuFunction,
        stream: &Stream,
        kernarg: &AlignedKernargV1,
        grid: [u32; 3],
        workgroup: [u32; 3],
        dynamic_group_segment_bytes: u32,
    ) -> Result<(), GeneratedApplicationProviderErrorV1> {
        let mut byte_length = kernarg.len();
        let mut extra = [
            HIP_LAUNCH_PARAM_BUFFER_POINTER,
            kernarg.as_ptr().cast_mut().cast(),
            HIP_LAUNCH_PARAM_BUFFER_SIZE,
            (&mut byte_length as *mut usize).cast(),
            HIP_LAUNCH_PARAM_END,
        ];
        stream
            .context()
            .bind_to_thread()
            .map_err(|error| provider_backend_error(&error))?;
        // SAFETY: this is the private raw target backend. The generated host contract has checked
        // the exact function ABI, target, geometry, resources, memory effects, pointer fixups, and
        // retained lifetimes. `extra` follows HIP's packed-buffer launch ABI.
        fe2o3_core::check(unsafe {
            fe2o3_hip_sys::hipModuleLaunchKernel(
                function.raw(),
                grid[0],
                grid[1],
                grid[2],
                workgroup[0],
                workgroup[1],
                workgroup[2],
                dynamic_group_segment_bytes,
                stream.raw(),
                std::ptr::null_mut(),
                extra.as_mut_ptr(),
            )
        })
        .map_err(|error| provider_backend_error(&error))
    }

    fn recover_or_abort(stream: &Stream) {
        if stream.synchronize().is_err() {
            std::process::abort();
        }
    }

    fn kernarg_error(field: &'static str) -> GeneratedApplicationPrepareErrorV1 {
        GeneratedApplicationPrepareErrorV1::Provider(
            GeneratedApplicationProviderErrorV1::KernargSubstitution(field),
        )
    }

    struct AlignedKernargV1 {
        pointer: NonNull<u8>,
        layout: Layout,
        len: usize,
    }

    impl AlignedKernargV1 {
        fn new(bytes: &[u8], alignment: usize) -> Result<Self, GeneratedApplicationPrepareErrorV1> {
            if alignment == 0 || alignment > MAX_BACKEND_ALIGNMENT || !alignment.is_power_of_two() {
                return Err(kernarg_error("alignment"));
            }
            let layout = Layout::from_size_align(bytes.len().max(1), alignment)
                .map_err(|_| kernarg_error("allocation layout"))?;
            // SAFETY: `layout` has nonzero size and valid power-of-two alignment.
            let pointer = NonNull::new(unsafe { alloc_zeroed(layout) })
                .ok_or_else(|| kernarg_error("allocation"))?;
            if !bytes.is_empty() {
                // SAFETY: the allocation covers at least `bytes.len()` bytes and does not overlap
                // the immutable source slice.
                unsafe {
                    pointer
                        .as_ptr()
                        .copy_from_nonoverlapping(bytes.as_ptr(), bytes.len());
                }
            }
            Ok(Self {
                pointer,
                layout,
                len: bytes.len(),
            })
        }

        fn as_ptr(&self) -> *const u8 {
            self.pointer.as_ptr()
        }

        const fn len(&self) -> usize {
            self.len
        }

        fn bytes_mut(&mut self) -> &mut [u8] {
            // SAFETY: this owner uniquely retains the allocation for `len` initialized bytes.
            unsafe { std::slice::from_raw_parts_mut(self.pointer.as_ptr(), self.len) }
        }
    }

    impl Drop for AlignedKernargV1 {
        fn drop(&mut self) {
            // SAFETY: `pointer` was allocated with this exact layout and is owned here.
            unsafe { dealloc(self.pointer.as_ptr(), self.layout) };
        }
    }
}

#[cfg(feature = "generated-gfx950-hip-provider")]
use hip::GeneratedHipQualificationInvocation;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_providers_accept_only_their_exact_artifact_target() {
        let gfx942 = AmdTargetId::parse("gfx942:xnack-").unwrap();
        let gfx950 = AmdTargetId::parse("gfx950:xnack-").unwrap();
        let gfx950_observed = AmdTargetId::parse("gfx950:sramecc+:xnack-").unwrap();
        assert!(provider_matches::<Gfx942DirectKfdProviderV1>(gfx942));
        assert!(!provider_matches::<Gfx942DirectKfdProviderV1>(gfx950));
        assert!(provider_matches::<Gfx950HipQualificationProviderV1>(gfx950));
        assert!(!provider_matches::<Gfx950HipQualificationProviderV1>(
            gfx942
        ));
        assert!(!provider_matches::<Gfx950HipQualificationProviderV1>(
            gfx950_observed
        ));
        assert!(gfx950.is_compatible_with_observed(&gfx950_observed));
    }

    #[test]
    fn public_backend_identity_is_target_neutral() {
        assert_eq!(
            Gfx942DirectKfdProviderV1::BACKEND,
            GeneratedApplicationBackendV1::ProtectedDirectKfd
        );
        assert_eq!(
            Gfx950HipQualificationProviderV1::BACKEND,
            GeneratedApplicationBackendV1::ReviewedHipQualification
        );
    }
}
