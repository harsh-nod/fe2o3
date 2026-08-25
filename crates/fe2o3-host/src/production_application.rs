use std::{error::Error, fmt};

use fe2o3_kernel_descriptor::KernelId;

use crate::{
    AuthenticatedWorkerV3ExecutableV1, CompilerGeneratedKernelExpectationV1,
    LoadedWorkerV3HsaExecutableV1, ObservedContext, RecoveredWorkerV3PinnedDescriptorV1,
    ReviewedHsaExecutableLifecycleAdapterV1, WorkerV3ApplicationDescriptorHandoffErrorV1,
    WorkerV3HsaExecutableLoadErrorV1, WorkerV3HsaLoadAuthorizationErrorV1,
    WorkerV3VerificationAuthenticationErrorV1, WorkerV3VerifierV1,
    consume_inherited_worker_v3_application_handoff_v1,
};

/// Failure at one mandatory stage of the production application load transaction.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProductionWorkerV3ApplicationLoadErrorV1<VE, AE> {
    Handoff(WorkerV3ApplicationDescriptorHandoffErrorV1),
    Verification(WorkerV3VerificationAuthenticationErrorV1<VE>),
    LoadAuthorization(WorkerV3HsaLoadAuthorizationErrorV1<AE>),
    ExecutableLoad(WorkerV3HsaExecutableLoadErrorV1<AE>),
}

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

/// Recovers and loads the one production executable inherited from `cargo fe2o3`.
///
/// This is the application entry point for the production pipeline. It consumes Cargo's exact
/// Worker V3 handoff, authenticates the compiler and Verus evidence, authorizes the observed HSA
/// environment, and loads the exact current executable. No intermediate authority escapes.
///
/// # Safety
///
/// The caller must invoke this operation before creating threads, installing signal handlers that
/// can access the environment or descriptor table, spawning descendants, or allowing unrelated
/// descriptor mutation. A hostile same-process caller violates this cooperative startup contract.
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
    let admission =
        unsafe { consume_inherited_worker_v3_application_handoff_v1(kernel_id, observed) }
            .map_err(ProductionWorkerV3ApplicationLoadErrorV1::Handoff)?;
    load_admitted_worker_v3_application_v1::<K, V, A>(admission, verifier, adapter)
}

/// Loads an already-admitted descriptor through the production verification and HSA boundary.
///
/// This lower-level operation supports generated application glue and tests that obtain descriptor
/// custody independently. Applications launched by `cargo fe2o3` should use
/// [`load_inherited_worker_v3_application_v1`].
#[doc(hidden)]
pub fn load_admitted_worker_v3_application_v1<K, V, A>(
    admission: RecoveredWorkerV3PinnedDescriptorV1,
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
        .authorize_hsa_load(adapter)
        .map_err(ProductionWorkerV3ApplicationLoadErrorV1::LoadAuthorization)?;
    authorized
        .load()
        .map_err(ProductionWorkerV3ApplicationLoadErrorV1::ExecutableLoad)
}
