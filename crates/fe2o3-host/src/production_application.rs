use std::{error::Error, fmt};

use fe2o3_kernel_descriptor::KernelId;

#[cfg(target_arch = "x86_64")]
use crate::application_descriptor_handoff::consume_inherited_worker_v3_capability_application_handoff_v1;
use crate::{
    AqlDispatchGeometryV1, AuthenticatedWorkerV3ExecutableV1, CheckedGfx942XnackMinusDevice,
    CompilerGeneratedKernelExpectationV1, CompilerGeneratedKfdArguments,
    GeneratedWorkerV3KfdInvocation, GeneratedWorkerV3KfdInvocationError, OpenedKfd,
    RecoveredWorkerV3PinnedDescriptorV1, WorkerV3ApplicationDescriptorHandoffErrorV1,
    WorkerV3VerificationAuthenticationErrorV1, WorkerV3VerifierV1,
    consume_inherited_worker_v3_application_handoff_v1,
};
#[cfg(target_arch = "x86_64")]
use crate::{
    AuthenticatedWorkerV3CapabilityApplicationV1, CompilerGeneratedKernelExpectationRosterV1,
    ProductionWorkerV3CapabilityRosterVerifierV1, ProductionWorkerV3ProtectedVerifierErrorV1,
    ProductionWorkerV3VerifierDeploymentErrorV1, WorkerV3CapabilityRosterAuthenticationFailureV1,
};
#[cfg(feature = "qualification-legacy-hip-hsa")]
use crate::{
    LoadedWorkerV3HsaExecutableV1, ObservedContext, ReviewedHsaExecutableLifecycleAdapterV1,
    WorkerV3HsaExecutableLoadErrorV1, WorkerV3HsaLoadAuthorizationErrorV1,
};

/// Failure while authenticating and preparing one generated pure-KFD invocation.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProductionWorkerV3KfdPreparationErrorV1<VE> {
    Verification(WorkerV3VerificationAuthenticationErrorV1<VE>),
    Invocation(GeneratedWorkerV3KfdInvocationError),
}

impl<VE: fmt::Display> fmt::Display for ProductionWorkerV3KfdPreparationErrorV1<VE> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Verification(error) => {
                write!(formatter, "application verification failed: {error}")
            }
            Self::Invocation(error) => {
                write!(formatter, "KFD invocation preparation failed: {error}")
            }
        }
    }
}

impl<VE> Error for ProductionWorkerV3KfdPreparationErrorV1<VE>
where
    VE: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Verification(error) => Some(error),
            Self::Invocation(error) => Some(error),
        }
    }
}

/// Failure while consuming the inherited Worker V3 handoff into a pure-KFD invocation.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProductionWorkerV3KfdApplicationErrorV1<VE> {
    Handoff(WorkerV3ApplicationDescriptorHandoffErrorV1),
    Preparation(ProductionWorkerV3KfdPreparationErrorV1<VE>),
}

/// Failure while selecting and binding the first observed gfx942 device by immutable unique ID.
#[cfg(target_arch = "x86_64")]
#[derive(Debug)]
#[non_exhaustive]
pub(crate) enum ProductionGfx942DeviceErrorV1 {
    Topology(fe2o3_kfd::topology::TopologyError),
    MissingDevice,
    Kfd(fe2o3_kfd::KfdAdapterError),
    Binding(fe2o3_kfd::DeviceBindingError),
}

#[cfg(target_arch = "x86_64")]
impl fmt::Display for ProductionGfx942DeviceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Topology(error) => write!(formatter, "gfx942 topology discovery failed: {error}"),
            Self::MissingDevice => formatter.write_str("no gfx942 device is present"),
            Self::Kfd(error) => write!(formatter, "KFD admission failed: {error}"),
            Self::Binding(error) => write!(formatter, "gfx942 device binding failed: {error}"),
        }
    }
}

#[cfg(target_arch = "x86_64")]
impl Error for ProductionGfx942DeviceErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Topology(error) => Some(error),
            Self::Kfd(error) => Some(error),
            Self::Binding(error) => Some(error),
            Self::MissingDevice => None,
        }
    }
}

/// Selects one observed gfx942 by its kernel-reported unique ID and admits the exact KFD device.
#[cfg(target_arch = "x86_64")]
pub(crate) fn open_default_gfx942_device_v1()
-> Result<CheckedGfx942XnackMinusDevice, ProductionGfx942DeviceErrorV1> {
    let topology = fe2o3_kfd::topology::discover_default_topology()
        .map_err(ProductionGfx942DeviceErrorV1::Topology)?;
    let unique_id = topology
        .topology()
        .gpu_nodes()
        .first()
        .ok_or(ProductionGfx942DeviceErrorV1::MissingDevice)?
        .unique_id();
    OpenedKfd::open_default()
        .map_err(ProductionGfx942DeviceErrorV1::Kfd)?
        .admit_uapi()
        .map_err(ProductionGfx942DeviceErrorV1::Kfd)?
        .bind_gfx942_xnack_minus(crate::DeviceSelector::UniqueId(unique_id))
        .map_err(ProductionGfx942DeviceErrorV1::Binding)
}

/// Failure while consuming and authenticating one complete inherited capability application.
#[cfg(target_arch = "x86_64")]
#[derive(Debug)]
#[non_exhaustive]
pub enum ProductionWorkerV3CapabilityApplicationErrorV1<R> {
    Handoff(WorkerV3ApplicationDescriptorHandoffErrorV1),
    ProductionResult(String),
    VerifierDeployment(ProductionWorkerV3VerifierDeploymentErrorV1),
    Authentication(
        WorkerV3CapabilityRosterAuthenticationFailureV1<
            R,
            ProductionWorkerV3ProtectedVerifierErrorV1,
        >,
    ),
}

#[cfg(target_arch = "x86_64")]
impl<R> fmt::Display for ProductionWorkerV3CapabilityApplicationErrorV1<R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Handoff(error) => write!(formatter, "application handoff failed: {error}"),
            Self::ProductionResult(error) => {
                write!(
                    formatter,
                    "completed V5 result could not be retained: {error}"
                )
            }
            Self::VerifierDeployment(error) => {
                write!(formatter, "protected verifier deployment failed: {error}")
            }
            Self::Authentication(error) => {
                write!(
                    formatter,
                    "capability roster authentication failed: {error}"
                )
            }
        }
    }
}

#[cfg(target_arch = "x86_64")]
impl<R> Error for ProductionWorkerV3CapabilityApplicationErrorV1<R>
where
    R: fmt::Debug + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Handoff(error) => Some(error),
            Self::VerifierDeployment(error) => Some(error),
            Self::Authentication(error) => Some(error),
            Self::ProductionResult(_) => None,
        }
    }
}

/// Consumes the exact inherited V2/V5 handoff and authenticates its generated marker roster.
///
/// # Safety
///
/// This is the same cooperative process-start boundary as
/// [`consume_inherited_worker_v3_capability_application_handoff_v1`]. Generated application
/// entrypoints call it before creating threads or installing signal handlers.
#[cfg(target_arch = "x86_64")]
pub unsafe fn authenticate_inherited_worker_v3_capability_application_v1<R>() -> Result<
    AuthenticatedWorkerV3CapabilityApplicationV1<R>,
    ProductionWorkerV3CapabilityApplicationErrorV1<R>,
>
where
    R: CompilerGeneratedKernelExpectationRosterV1,
{
    // SAFETY: this function exposes the same cooperative startup contract as the consumer.
    let recovered = unsafe { consume_inherited_worker_v3_capability_application_handoff_v1() }
        .map_err(ProductionWorkerV3CapabilityApplicationErrorV1::Handoff)?;
    let result = fe2o3_compiler_ffi::InertProductionCapabilityResultV5::decode(
        recovered.production_result().canonical_bytes(),
    )
    .map_err(|error| {
        ProductionWorkerV3CapabilityApplicationErrorV1::ProductionResult(error.to_string())
    })?;
    let mut verifier =
        ProductionWorkerV3CapabilityRosterVerifierV1::<R>::from_production_deployment(result)
            .map_err(ProductionWorkerV3CapabilityApplicationErrorV1::VerifierDeployment)?;
    recovered
        .authenticate(&mut verifier)
        .map_err(ProductionWorkerV3CapabilityApplicationErrorV1::Authentication)
}

/// Runs generated application code only after the inherited V2/V5 startup handoff is consumed.
///
/// The callback is the normal application entry boundary. It receives authenticated, move-only
/// custody and cannot run before the host-owned cooperative startup transition has completed.
#[cfg(target_arch = "x86_64")]
pub fn run_generated_application_v1<R, T>(
    application: impl FnOnce(
        &AuthenticatedWorkerV3CapabilityApplicationV1<R>,
    ) -> Result<T, Box<dyn Error>>,
) -> Result<T, Box<dyn Error>>
where
    R: CompilerGeneratedKernelExpectationRosterV1 + fmt::Debug,
{
    // SAFETY: no application callback is invoked until the one-shot process-entry transition has
    // completed, and the private raw operation cannot be reached from the normal public surface.
    let authenticated =
        unsafe { authenticate_inherited_worker_v3_capability_application_v1::<R>() }
            .map_err(|error| Box::new(error) as Box<dyn Error>)?;
    application(&authenticated)
}

impl<VE: fmt::Display> fmt::Display for ProductionWorkerV3KfdApplicationErrorV1<VE> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Handoff(error) => write!(formatter, "application handoff failed: {error}"),
            Self::Preparation(error) => error.fmt(formatter),
        }
    }
}

impl<VE> Error for ProductionWorkerV3KfdApplicationErrorV1<VE>
where
    VE: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Handoff(error) => Some(error),
            Self::Preparation(error) => Some(error),
        }
    }
}

/// Consumes Cargo's inherited Worker V3 custody into one generated pure-KFD invocation.
///
/// The kernel identity comes from `K`; callers cannot select a different descriptor. Recovery and
/// verification remain device-independent, then the final transition consumes the checked KFD
/// device together with compiler-generated arguments and geometry. The returned invocation is
/// move-only and retains all output borrows until checked completion.
///
/// # Safety
///
/// The caller must invoke this operation before creating threads, installing signal handlers that
/// can access the environment or descriptor table, spawning descendants, or allowing unrelated
/// descriptor mutation. A hostile same-process caller violates this cooperative startup contract.
#[allow(clippy::too_many_arguments)]
pub unsafe fn prepare_inherited_worker_v3_kfd_application_v1<'allocation, K, V, Arguments>(
    verifier: &mut V,
    arguments: Arguments,
    device: CheckedGfx942XnackMinusDevice,
    geometry: AqlDispatchGeometryV1,
    dynamic_group_segment_bytes: u32,
    timeout_milliseconds: u32,
) -> Result<
    GeneratedWorkerV3KfdInvocation<'allocation, K>,
    ProductionWorkerV3KfdApplicationErrorV1<V::Error>,
>
where
    K: CompilerGeneratedKernelExpectationV1,
    V: WorkerV3VerifierV1<K>,
    Arguments: CompilerGeneratedKfdArguments<'allocation, K>,
{
    let kernel_id = KernelId::from_bytes(K::KERNEL_BINDING_ID_V1);
    // SAFETY: this function has the same cooperative startup contract as the handoff consumer.
    let admission = unsafe { consume_inherited_worker_v3_application_handoff_v1(kernel_id) }
        .map_err(ProductionWorkerV3KfdApplicationErrorV1::Handoff)?;
    prepare_admitted_worker_v3_kfd_application_v1(
        admission,
        verifier,
        arguments,
        device,
        geometry,
        dynamic_group_segment_bytes,
        timeout_milliseconds,
    )
    .map_err(ProductionWorkerV3KfdApplicationErrorV1::Preparation)
}

/// Prepares an already-admitted Worker V3 artifact through the canonical pure-KFD boundary.
#[doc(hidden)]
#[allow(clippy::too_many_arguments)]
pub fn prepare_admitted_worker_v3_kfd_application_v1<'allocation, K, V, Arguments>(
    admission: RecoveredWorkerV3PinnedDescriptorV1,
    verifier: &mut V,
    arguments: Arguments,
    device: CheckedGfx942XnackMinusDevice,
    geometry: AqlDispatchGeometryV1,
    dynamic_group_segment_bytes: u32,
    timeout_milliseconds: u32,
) -> Result<
    GeneratedWorkerV3KfdInvocation<'allocation, K>,
    ProductionWorkerV3KfdPreparationErrorV1<V::Error>,
>
where
    K: CompilerGeneratedKernelExpectationV1,
    V: WorkerV3VerifierV1<K>,
    Arguments: CompilerGeneratedKfdArguments<'allocation, K>,
{
    let authenticated = AuthenticatedWorkerV3ExecutableV1::<K>::authenticate(admission, verifier)
        .map_err(ProductionWorkerV3KfdPreparationErrorV1::Verification)?;
    authenticated
        .prepare_generated_kfd_invocation(
            arguments,
            device,
            geometry,
            dynamic_group_segment_bytes,
            timeout_milliseconds,
        )
        .map_err(ProductionWorkerV3KfdPreparationErrorV1::Invocation)
}

/// Failure at one mandatory stage of the HSA-backed application migration transaction.
#[cfg(feature = "qualification-legacy-hip-hsa")]
#[derive(Debug)]
#[non_exhaustive]
pub enum ProductionWorkerV3ApplicationLoadErrorV1<VE, AE> {
    Handoff(WorkerV3ApplicationDescriptorHandoffErrorV1),
    Verification(WorkerV3VerificationAuthenticationErrorV1<VE>),
    LoadAuthorization(WorkerV3HsaLoadAuthorizationErrorV1<AE>),
    ExecutableLoad(WorkerV3HsaExecutableLoadErrorV1<AE>),
}

#[cfg(feature = "qualification-legacy-hip-hsa")]
impl<VE: fmt::Display, AE: fmt::Display> fmt::Display
    for ProductionWorkerV3ApplicationLoadErrorV1<VE, AE>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Handoff(error) => write!(formatter, "application handoff failed: {error}"),
            Self::Verification(error) => {
                write!(formatter, "application verification failed: {error}")
            }
            Self::LoadAuthorization(error) => {
                write!(formatter, "application load authorization failed: {error}")
            }
            Self::ExecutableLoad(error) => {
                write!(formatter, "application executable load failed: {error}")
            }
        }
    }
}

#[cfg(feature = "qualification-legacy-hip-hsa")]
impl<VE, AE> Error for ProductionWorkerV3ApplicationLoadErrorV1<VE, AE>
where
    VE: Error + 'static,
    AE: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Handoff(error) => Some(error),
            Self::Verification(error) => Some(error),
            Self::LoadAuthorization(error) => Some(error),
            Self::ExecutableLoad(error) => Some(error),
        }
    }
}

/// Recovers and loads one inherited Worker V3 executable through the HSA migration route.
///
/// New generated applications use [`prepare_inherited_worker_v3_kfd_application_v1`]. This
/// temporary route consumes the same device-independent Cargo handoff and verifier decision, then
/// binds a separately supplied HIP context while loading the exact current executable. No
/// intermediate authority escapes.
///
/// # Safety
///
/// The caller must invoke this operation before creating threads, installing signal handlers that
/// can access the environment or descriptor table, spawning descendants, or allowing unrelated
/// descriptor mutation. A hostile same-process caller violates this cooperative startup contract.
#[cfg(feature = "qualification-legacy-hip-hsa")]
pub unsafe fn load_inherited_worker_v3_application_v1<K, V, A>(
    kernel_id: KernelId,
    observed: &ObservedContext,
    verifier: &mut V,
    adapter: A,
) -> Result<
    LoadedWorkerV3HsaExecutableV1<K, A>,
    ProductionWorkerV3ApplicationLoadErrorV1<V::Error, A::Error>,
>
where
    K: CompilerGeneratedKernelExpectationV1,
    V: WorkerV3VerifierV1<K>,
    A: ReviewedHsaExecutableLifecycleAdapterV1,
{
    // SAFETY: this function has the same cooperative startup contract as the handoff consumer.
    let admission = unsafe { consume_inherited_worker_v3_application_handoff_v1(kernel_id) }
        .map_err(ProductionWorkerV3ApplicationLoadErrorV1::Handoff)?;
    load_admitted_worker_v3_application_v1::<K, V, A>(admission, observed, verifier, adapter)
}

/// Loads an already-admitted descriptor through the HSA migration boundary.
///
/// This lower-level operation exists only for the deprecated HSA qualification surface. Production
/// applications use the direct-KFD preparation boundary.
#[doc(hidden)]
#[cfg(feature = "qualification-legacy-hip-hsa")]
pub fn load_admitted_worker_v3_application_v1<K, V, A>(
    admission: RecoveredWorkerV3PinnedDescriptorV1,
    observed: &ObservedContext,
    verifier: &mut V,
    adapter: A,
) -> Result<
    LoadedWorkerV3HsaExecutableV1<K, A>,
    ProductionWorkerV3ApplicationLoadErrorV1<V::Error, A::Error>,
>
where
    K: CompilerGeneratedKernelExpectationV1,
    V: WorkerV3VerifierV1<K>,
    A: ReviewedHsaExecutableLifecycleAdapterV1,
{
    let authenticated = AuthenticatedWorkerV3ExecutableV1::<K>::authenticate(admission, verifier)
        .map_err(ProductionWorkerV3ApplicationLoadErrorV1::Verification)?;
    let authorized = authenticated
        .authorize_hsa_load(observed.clone(), adapter)
        .map_err(ProductionWorkerV3ApplicationLoadErrorV1::LoadAuthorization)?;
    authorized
        .load()
        .map_err(ProductionWorkerV3ApplicationLoadErrorV1::ExecutableLoad)
}
