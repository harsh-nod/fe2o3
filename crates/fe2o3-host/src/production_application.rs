use std::{error::Error, fmt};

mod current_thread;
pub use current_thread::*;

use fe2o3_kernel_descriptor::KernelId;

use crate::{
    AqlDispatchGeometryV1, AuthenticatedWorkerV3ExecutableV1, CheckedGfx942XnackMinusDevice,
    CompilerGeneratedKernelExpectationV1, CompilerGeneratedKfdArguments,
    GeneratedWorkerV3KfdInvocation, GeneratedWorkerV3KfdInvocationError,
    RecoveredWorkerV3PinnedDescriptorV1, WorkerV3ApplicationDescriptorHandoffErrorV1,
    WorkerV3VerificationAuthenticationErrorV1, WorkerV3VerifierV1,
    consume_inherited_worker_v3_application_handoff_v1,
};
#[cfg(feature = "qualification-legacy-hip-hsa")]
use crate::{
    LoadedWorkerV3HsaExecutableV1, ObservedContext, ReviewedHsaExecutableLifecycleAdapterV1,
    WorkerV3HsaExecutableLoadErrorV1, WorkerV3HsaLoadAuthorizationErrorV1,
};

/// Failure while consuming and authenticating the inherited Worker V3 application.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProductionWorkerV3AuthenticationErrorV1<VE> {
    Handoff(WorkerV3ApplicationDescriptorHandoffErrorV1),
    Verification(WorkerV3VerificationAuthenticationErrorV1<VE>),
}

impl<VE: fmt::Display> fmt::Display for ProductionWorkerV3AuthenticationErrorV1<VE> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Handoff(error) => write!(formatter, "application handoff failed: {error}"),
            Self::Verification(error) => {
                write!(formatter, "application verification failed: {error}")
            }
        }
    }
}

impl<VE> Error for ProductionWorkerV3AuthenticationErrorV1<VE>
where
    VE: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Handoff(error) => Some(error),
            Self::Verification(error) => Some(error),
        }
    }
}

/// Consumes Cargo's inherited Worker V3 handoff into its authenticated linear owner.
///
/// This device-independent bootstrap leaves runtime construction, generated argument
/// reservation, and execution to the caller. It grants neither load nor launch
/// authority; the returned owner still requires the existing protected generated
/// preparation and adoption route.
///
/// ```no_run
/// use fe2o3_host::{
///     AuthenticatedWorkerV3ExecutableV1, CompilerGeneratedKernelExpectationV1,
///     ProductionWorkerV3AuthenticationErrorV1, WorkerV3VerifierV1,
///     authenticate_inherited_worker_v3_application_v1,
/// };
///
/// fn authenticate_at_startup<K, V>(
///     verifier: &mut V,
/// ) -> Result<
///     AuthenticatedWorkerV3ExecutableV1<K>,
///     ProductionWorkerV3AuthenticationErrorV1<V::Error>,
/// >
/// where
///     K: CompilerGeneratedKernelExpectationV1,
///     V: WorkerV3VerifierV1<K>,
/// {
///     // SAFETY: the application calls this before creating threads or exposing
///     // the inherited environment and descriptors to unrelated code.
///     unsafe { authenticate_inherited_worker_v3_application_v1::<K, V>(verifier) }
/// }
/// ```
///
/// # Safety
///
/// The caller must invoke this operation before creating threads, installing signal handlers that
/// can access the environment or descriptor table, spawning descendants, or allowing unrelated
/// descriptor mutation. A hostile same-process caller violates this cooperative startup contract.
pub unsafe fn authenticate_inherited_worker_v3_application_v1<K, V>(
    verifier: &mut V,
) -> Result<AuthenticatedWorkerV3ExecutableV1<K>, ProductionWorkerV3AuthenticationErrorV1<V::Error>>
where
    K: CompilerGeneratedKernelExpectationV1,
    V: WorkerV3VerifierV1<K>,
{
    let kernel_id = KernelId::from_bytes(K::KERNEL_BINDING_ID_V1);
    // SAFETY: this function has the same cooperative startup contract as the handoff consumer.
    let admission = unsafe { consume_inherited_worker_v3_application_handoff_v1(kernel_id) }
        .map_err(ProductionWorkerV3AuthenticationErrorV1::Handoff)?;
    AuthenticatedWorkerV3ExecutableV1::<K>::authenticate(admission, verifier)
        .map_err(ProductionWorkerV3AuthenticationErrorV1::Verification)
}

fn preparation_error_from_authentication_v1<VE>(
    error: ProductionWorkerV3AuthenticationErrorV1<VE>,
) -> ProductionWorkerV3KfdApplicationErrorV1<VE> {
    match error {
        ProductionWorkerV3AuthenticationErrorV1::Handoff(error) => {
            ProductionWorkerV3KfdApplicationErrorV1::Handoff(error)
        }
        ProductionWorkerV3AuthenticationErrorV1::Verification(error) => {
            ProductionWorkerV3KfdApplicationErrorV1::Preparation(
                ProductionWorkerV3KfdPreparationErrorV1::Verification(error),
            )
        }
    }
}

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
    // SAFETY: this function has the same cooperative startup contract as the bootstrap helper.
    let authenticated =
        unsafe { authenticate_inherited_worker_v3_application_v1::<K, V>(verifier) }
            .map_err(preparation_error_from_authentication_v1)?;
    authenticated
        .prepare_generated_kfd_invocation(
            arguments,
            device,
            geometry,
            dynamic_group_segment_bytes,
            timeout_milliseconds,
        )
        .map_err(|error| {
            ProductionWorkerV3KfdApplicationErrorV1::Preparation(
                ProductionWorkerV3KfdPreparationErrorV1::Invocation(error),
            )
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct VerifierErrorV1;

    impl fmt::Display for VerifierErrorV1 {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("verifier error")
        }
    }

    impl Error for VerifierErrorV1 {}

    #[test]
    fn bootstrap_error_mapping_preserves_public_variants() {
        let handoff = preparation_error_from_authentication_v1::<VerifierErrorV1>(
            ProductionWorkerV3AuthenticationErrorV1::Handoff(
                WorkerV3ApplicationDescriptorHandoffErrorV1::AlreadyConsumed,
            ),
        );
        assert!(matches!(
            handoff,
            ProductionWorkerV3KfdApplicationErrorV1::Handoff(
                WorkerV3ApplicationDescriptorHandoffErrorV1::AlreadyConsumed
            )
        ));

        let verification =
            preparation_error_from_authentication_v1(ProductionWorkerV3AuthenticationErrorV1::<
                VerifierErrorV1,
            >::Verification(
                WorkerV3VerificationAuthenticationErrorV1::UnsupportedGeneratedProfile,
            ));
        assert!(matches!(
            verification,
            ProductionWorkerV3KfdApplicationErrorV1::Preparation(
                ProductionWorkerV3KfdPreparationErrorV1::Verification(
                    WorkerV3VerificationAuthenticationErrorV1::UnsupportedGeneratedProfile
                )
            )
        ));
    }
}
