//! One-shot HSA lifecycle for the exact protected row-softmax V1 profile.

use crate::{
    GeneratedProtectedRowSoftmaxV1HostAdapterV1, HsaCodeObjectLoadObservationV1,
    HsaDispatchObservationV1, HsaEnvironmentObservationV1, HsaExecutableObjectIdentityV1,
    HsaImplicitKernargInitializationObservationV1, HsaKernelObjectIdentityV1,
    HsaKernelResolutionObservationV1, HsaLaunchGeometryV1, HsaUnloadObservationV1,
    ProtectedRowSoftmaxV1HostTokenIdentityV1, ProtectedRowSoftmaxV1HostTokenV1,
    ReviewedHsaImplicitKernargAdapterV1,
};
use fe2o3_amd_target::AmdTargetId;
use fe2o3_artifacts::{DigestAlgorithm, PayloadDigest};
use fe2o3_core::ContextIdentity;
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, FinalizedWorkerV2HsacoIdentityV1, ProtectedRowSoftmaxV1AdmissionIdentityV1,
};
use sha2::{Digest, Sha256};
use std::{error::Error, fmt};

const TARGET: &str = "gfx942:xnack-";
const EXPORT_SYMBOL: &str = "row_softmax_v1";
const GRID: [u32; 3] = [1, 1, 1];
const WORKGROUP: [u32; 3] = [64, 1, 1];
const EXPLICIT_KERNARG_BYTES: usize = 32;
const COMPLETE_KERNARG_BYTES: usize = 288;
const IMPLICIT_KERNARG_BYTES: usize = COMPLETE_KERNARG_BYTES - EXPLICIT_KERNARG_BYTES;
const HSA_KERNARG_ALIGNMENT: u64 = 16;
const DYNAMIC_LDS_BYTES: u32 = 0;
const STATIC_LDS_BYTES: u32 = 0;
const PRIVATE_SEGMENT_BYTES: u32 = 0;
const UNLOAD_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/ROW-SOFTMAX/UNLOAD/V1\0";

/// Runtime-reported resource facts for the exact row-softmax kernel object.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedRowSoftmaxV1KernelResourceObservationV1 {
    executable_object: HsaExecutableObjectIdentityV1,
    kernel_object: HsaKernelObjectIdentityV1,
    group_segment_size: u32,
    private_segment_size: u32,
}

impl ProtectedRowSoftmaxV1KernelResourceObservationV1 {
    pub const fn new(
        executable_object: HsaExecutableObjectIdentityV1,
        kernel_object: HsaKernelObjectIdentityV1,
        group_segment_size: u32,
        private_segment_size: u32,
    ) -> Self {
        Self {
            executable_object,
            kernel_object,
            group_segment_size,
            private_segment_size,
        }
    }

    pub const fn executable_object(self) -> HsaExecutableObjectIdentityV1 {
        self.executable_object
    }

    pub const fn kernel_object(self) -> HsaKernelObjectIdentityV1 {
        self.kernel_object
    }

    pub const fn group_segment_size(self) -> u32 {
        self.group_segment_size
    }

    pub const fn private_segment_size(self) -> u32 {
        self.private_segment_size
    }
}

/// Reviewed production extension for this exact protected profile.
///
/// # Safety
///
/// The context identity and resource observation must describe the exact
/// private context, executable, and kernel retained by this adapter. Methods
/// must not unwind. Implementations inherit all obligations from
/// [`ReviewedHsaImplicitKernargAdapterV1`].
pub unsafe trait ReviewedProtectedRowSoftmaxV1RuntimeAdapterV1:
    ReviewedHsaImplicitKernargAdapterV1
{
    /// Returns the identity of this adapter's retained `GpuContext`.
    ///
    /// # Safety
    ///
    /// The identity must describe the exact live context retained by this
    /// adapter, and the implementation must not unwind.
    unsafe fn context_identity_v1(&mut self) -> ContextIdentity;

    /// Observes exact resources for the retained executable and kernel.
    ///
    /// # Safety
    ///
    /// Both arguments must belong to this adapter. The returned observation
    /// must describe exactly those objects, and the implementation must not
    /// unwind or expose either object's native handle.
    unsafe fn observe_protected_row_softmax_v1_kernel_resources(
        &mut self,
        executable: &Self::Executable,
        kernel: &Self::Kernel,
    ) -> Result<ProtectedRowSoftmaxV1KernelResourceObservationV1, Self::Error>;
}

/// Rejection while joining the sealed token and generated typed invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProtectedRowSoftmaxV1JoinErrorV1 {
    TokenField(&'static str),
    HostField(&'static str),
}

impl fmt::Display for ProtectedRowSoftmaxV1JoinErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TokenField(field) => write!(formatter, "protected token {field} drifted"),
            Self::HostField(field) => write!(formatter, "generated host {field} drifted"),
        }
    }
}

impl Error for ProtectedRowSoftmaxV1JoinErrorV1 {}

/// Recoverable load rejection before dispatch publication.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProtectedRowSoftmaxV1LoadErrorV1<E> {
    ContextIdentity,
    EnvironmentAdapter(E),
    Environment(&'static str),
    FinalizedOutput,
    LoadAdapter(E),
    LoadObservation(&'static str),
    KernelAdapter(E),
    KernelObservation(&'static str),
    ResourceAdapter(E),
    ResourceObservation(&'static str),
}

impl<E: fmt::Display> fmt::Display for ProtectedRowSoftmaxV1LoadErrorV1<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContextIdentity => formatter.write_str("exact HSA context identity mismatch"),
            Self::EnvironmentAdapter(error) => write!(formatter, "HSA environment failed: {error}"),
            Self::Environment(field) => write!(formatter, "HSA environment {field} drifted"),
            Self::FinalizedOutput => formatter.write_str("retained finalized output drifted"),
            Self::LoadAdapter(error) => write!(formatter, "HSA executable load failed: {error}"),
            Self::LoadObservation(field) => write!(formatter, "HSA load {field} drifted"),
            Self::KernelAdapter(error) => {
                write!(formatter, "HSA kernel resolution failed: {error}")
            }
            Self::KernelObservation(field) => write!(formatter, "HSA kernel {field} drifted"),
            Self::ResourceAdapter(error) => write!(formatter, "HSA resource query failed: {error}"),
            Self::ResourceObservation(field) => write!(formatter, "HSA resource {field} drifted"),
        }
    }
}

impl<E: Error + 'static> Error for ProtectedRowSoftmaxV1LoadErrorV1<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::EnvironmentAdapter(error)
            | Self::LoadAdapter(error)
            | Self::KernelAdapter(error)
            | Self::ResourceAdapter(error) => Some(error),
            _ => None,
        }
    }
}

/// Recoverable rejection before publication or after proven quiescence.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProtectedRowSoftmaxV1DispatchErrorV1<E> {
    ImplicitAdapter(E),
    ImplicitObservation(&'static str),
    ExplicitKernargMutation,
    DispatchAdapter(E),
    DispatchObservation(&'static str),
}

impl<E: fmt::Display> fmt::Display for ProtectedRowSoftmaxV1DispatchErrorV1<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ImplicitAdapter(error) => write!(formatter, "implicit kernarg failed: {error}"),
            Self::ImplicitObservation(field) => {
                write!(formatter, "implicit kernarg {field} drifted")
            }
            Self::ExplicitKernargMutation => {
                formatter.write_str("implicit initialization changed explicit bytes")
            }
            Self::DispatchAdapter(error) => write!(formatter, "HSA dispatch failed: {error}"),
            Self::DispatchObservation(field) => write!(formatter, "HSA dispatch {field} drifted"),
        }
    }
}

impl<E: Error + 'static> Error for ProtectedRowSoftmaxV1DispatchErrorV1<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ImplicitAdapter(error) | Self::DispatchAdapter(error) => Some(error),
            _ => None,
        }
    }
}

/// Inert exact token/buffer join before a runtime is observed.
#[must_use = "the joined row-softmax request must enter its one-shot lifecycle"]
pub struct JoinedProtectedRowSoftmaxV1<'input, 'output> {
    token: ProtectedRowSoftmaxV1HostTokenV1,
    host: GeneratedProtectedRowSoftmaxV1HostAdapterV1<'input, 'output>,
}

impl fmt::Debug for JoinedProtectedRowSoftmaxV1<'_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JoinedProtectedRowSoftmaxV1")
            .field("token", &self.token.identity())
            .field("admission", &self.token.admission_identity())
            .field("artifact", &self.token.finalized_artifact_identity())
            .finish_non_exhaustive()
    }
}

/// Consumes the sealed token and exact generated binding into one linear join.
pub fn join_protected_row_softmax_v1<'input, 'output>(
    token: ProtectedRowSoftmaxV1HostTokenV1,
    host: GeneratedProtectedRowSoftmaxV1HostAdapterV1<'input, 'output>,
) -> Result<JoinedProtectedRowSoftmaxV1<'input, 'output>, ProtectedRowSoftmaxV1JoinErrorV1> {
    validate_join(&token, &host)?;
    Ok(JoinedProtectedRowSoftmaxV1 { token, host })
}

impl<'input, 'output> JoinedProtectedRowSoftmaxV1<'input, 'output> {
    pub const fn token_identity(&self) -> ProtectedRowSoftmaxV1HostTokenIdentityV1 {
        self.token.identity()
    }

    pub const fn admission_identity(&self) -> ProtectedRowSoftmaxV1AdmissionIdentityV1 {
        self.token.admission_identity()
    }

    pub const fn finalized_artifact_identity(&self) -> FinalizedWorkerV2HsacoIdentityV1 {
        self.token.finalized_artifact_identity()
    }

    pub fn load<A: ReviewedProtectedRowSoftmaxV1RuntimeAdapterV1>(
        self,
        mut adapter: A,
    ) -> Result<
        LoadedProtectedRowSoftmaxV1<'input, 'output, A>,
        ProtectedRowSoftmaxV1LoadErrorV1<A::Error>,
    > {
        let context_identity = reviewed_adapter_call(|| unsafe { adapter.context_identity_v1() });
        if !self
            .host
            .observed_context_v1()
            .matches_core_context_identity_v1(context_identity)
        {
            return Err(ProtectedRowSoftmaxV1LoadErrorV1::ContextIdentity);
        }
        let state = load_after_context_match(self, adapter)?;
        Ok(LoadedProtectedRowSoftmaxV1 { state })
    }
}

trait RetainedProtectedRowSoftmaxV1 {
    fn target_v1(&self) -> &str;
    fn ordinal_v1(&self) -> i32;
    fn explicit_kernarg_v1(&self) -> &[u8; EXPLICIT_KERNARG_BYTES];
    fn with_finalized_bytes_v1<T, F: FnOnce(&[u8], ContentIdentityV1) -> T>(&self, load: F) -> T;
}

impl RetainedProtectedRowSoftmaxV1 for JoinedProtectedRowSoftmaxV1<'_, '_> {
    fn target_v1(&self) -> &str {
        self.host.target()
    }

    fn ordinal_v1(&self) -> i32 {
        self.host.observed_context_v1().device().ordinal()
    }

    fn explicit_kernarg_v1(&self) -> &[u8; EXPLICIT_KERNARG_BYTES] {
        self.host.explicit_kernarg_bytes_v1()
    }

    fn with_finalized_bytes_v1<T, F: FnOnce(&[u8], ContentIdentityV1) -> T>(&self, load: F) -> T {
        // SAFETY: this method is reachable only inside the reviewed exact-load
        // transition below, which forbids byte escape and retains authority.
        unsafe {
            self.token
                .load_exact_finalized_with_reviewed_runtime_v1(load)
        }
    }
}

struct LoadedState<R, A: ReviewedProtectedRowSoftmaxV1RuntimeAdapterV1> {
    retained: Option<R>,
    adapter: Option<A>,
    environment: HsaEnvironmentObservationV1,
    load: HsaCodeObjectLoadObservationV1,
    resolution: HsaKernelResolutionObservationV1,
    resources: ProtectedRowSoftmaxV1KernelResourceObservationV1,
    executable: Option<A::Executable>,
    kernel: Option<A::Kernel>,
    kernarg: CompleteKernargV1,
}

impl<R, A: ReviewedProtectedRowSoftmaxV1RuntimeAdapterV1> Drop for LoadedState<R, A> {
    fn drop(&mut self) {
        self.kernel.take();
        let Some(executable) = self.executable.take() else {
            return;
        };
        let Some(adapter) = self.adapter.as_mut() else {
            std::process::abort();
        };
        terminal_unload(adapter, executable, &self.environment, &self.load);
    }
}

/// Loaded one-shot authority with no raw or generic launch operation.
#[must_use = "the loaded row-softmax lifecycle must dispatch or terminally unload"]
pub struct LoadedProtectedRowSoftmaxV1<
    'input,
    'output,
    A: ReviewedProtectedRowSoftmaxV1RuntimeAdapterV1,
> {
    state: LoadedState<JoinedProtectedRowSoftmaxV1<'input, 'output>, A>,
}

impl<A: ReviewedProtectedRowSoftmaxV1RuntimeAdapterV1> fmt::Debug
    for LoadedProtectedRowSoftmaxV1<'_, '_, A>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LoadedProtectedRowSoftmaxV1")
            .field("load", &self.state.load)
            .field("resolution", &self.state.resolution)
            .field("resources", &self.state.resources)
            .finish_non_exhaustive()
    }
}

impl<'input, 'output, A: ReviewedProtectedRowSoftmaxV1RuntimeAdapterV1>
    LoadedProtectedRowSoftmaxV1<'input, 'output, A>
{
    pub fn dispatch_and_wait(
        self,
    ) -> Result<CompletedProtectedRowSoftmaxV1<A>, ProtectedRowSoftmaxV1DispatchErrorV1<A::Error>>
    {
        let quiescent = self.state.dispatch_and_wait()?;
        let retained = quiescent
            .retained
            .as_ref()
            .expect("quiescent lifecycle retains the exact join");
        let receipt = CompletionReceiptV1 {
            token_identity: retained.token.identity(),
            admission_identity: retained.token.admission_identity(),
            finalized_artifact_identity: retained.token.finalized_artifact_identity(),
            finalized_output_identity: retained.token.finalized_output_identity(),
        };
        Ok(CompletedProtectedRowSoftmaxV1 {
            state: quiescent.release_retained(),
            receipt,
        })
    }
}

struct QuiescentState<R, A: ReviewedProtectedRowSoftmaxV1RuntimeAdapterV1> {
    retained: Option<R>,
    adapter: Option<A>,
    environment: HsaEnvironmentObservationV1,
    load: HsaCodeObjectLoadObservationV1,
    resolution: HsaKernelResolutionObservationV1,
    executable: Option<A::Executable>,
    dispatch_identity: [u8; 16],
}

impl<R, A: ReviewedProtectedRowSoftmaxV1RuntimeAdapterV1> Drop for QuiescentState<R, A> {
    fn drop(&mut self) {
        let Some(executable) = self.executable.take() else {
            return;
        };
        let Some(adapter) = self.adapter.as_mut() else {
            std::process::abort();
        };
        terminal_unload(adapter, executable, &self.environment, &self.load);
    }
}

impl<R, A: ReviewedProtectedRowSoftmaxV1RuntimeAdapterV1> QuiescentState<R, A> {
    fn release_retained(mut self) -> CompletedState<A> {
        drop(self.retained.take());
        CompletedState {
            adapter: self.adapter.take(),
            environment: self.environment.clone(),
            load: self.load.clone(),
            resolution: self.resolution.clone(),
            executable: self.executable.take(),
            dispatch_identity: self.dispatch_identity,
        }
    }
}

struct CompletedState<A: ReviewedProtectedRowSoftmaxV1RuntimeAdapterV1> {
    adapter: Option<A>,
    environment: HsaEnvironmentObservationV1,
    load: HsaCodeObjectLoadObservationV1,
    resolution: HsaKernelResolutionObservationV1,
    executable: Option<A::Executable>,
    dispatch_identity: [u8; 16],
}

impl<A: ReviewedProtectedRowSoftmaxV1RuntimeAdapterV1> Drop for CompletedState<A> {
    fn drop(&mut self) {
        let Some(executable) = self.executable.take() else {
            return;
        };
        let Some(adapter) = self.adapter.as_mut() else {
            std::process::abort();
        };
        terminal_unload(adapter, executable, &self.environment, &self.load);
    }
}

#[derive(Clone, Copy)]
struct CompletionReceiptV1 {
    token_identity: ProtectedRowSoftmaxV1HostTokenIdentityV1,
    admission_identity: ProtectedRowSoftmaxV1AdmissionIdentityV1,
    finalized_artifact_identity: FinalizedWorkerV2HsacoIdentityV1,
    finalized_output_identity: ContentIdentityV1,
}

/// Quiescent completion; input/output leases have already been released.
#[must_use = "the completed row-softmax lifecycle must be terminally unloaded"]
pub struct CompletedProtectedRowSoftmaxV1<A: ReviewedProtectedRowSoftmaxV1RuntimeAdapterV1> {
    state: CompletedState<A>,
    receipt: CompletionReceiptV1,
}

impl<A: ReviewedProtectedRowSoftmaxV1RuntimeAdapterV1> CompletedProtectedRowSoftmaxV1<A> {
    pub const fn token_identity(&self) -> ProtectedRowSoftmaxV1HostTokenIdentityV1 {
        self.receipt.token_identity
    }

    pub const fn admission_identity(&self) -> ProtectedRowSoftmaxV1AdmissionIdentityV1 {
        self.receipt.admission_identity
    }

    pub const fn finalized_artifact_identity(&self) -> FinalizedWorkerV2HsacoIdentityV1 {
        self.receipt.finalized_artifact_identity
    }

    pub const fn finalized_output_identity(&self) -> ContentIdentityV1 {
        self.receipt.finalized_output_identity
    }

    pub const fn executable_object(&self) -> HsaExecutableObjectIdentityV1 {
        self.state.load.executable_object()
    }

    pub const fn kernel_object(&self) -> HsaKernelObjectIdentityV1 {
        self.state.resolution.kernel_object()
    }

    pub const fn dispatch_identity(&self) -> [u8; 16] {
        self.state.dispatch_identity
    }

    pub fn unload(mut self) -> UnloadedProtectedRowSoftmaxV1 {
        let mut adapter = self.state.adapter.take().expect("completed adapter");
        let executable = self.state.executable.take().expect("completed executable");
        let unload = terminal_unload(
            &mut adapter,
            executable,
            &self.state.environment,
            &self.state.load,
        );
        let receipt = UnloadedProtectedRowSoftmaxV1 {
            token_identity: self.receipt.token_identity,
            admission_identity: self.receipt.admission_identity,
            finalized_artifact_identity: self.receipt.finalized_artifact_identity,
            finalized_output_identity: self.receipt.finalized_output_identity,
            executable_object: self.state.load.executable_object(),
            kernel_object: self.state.resolution.kernel_object(),
            dispatch_identity: self.state.dispatch_identity,
            unload_identity: unload_identity(
                &unload,
                self.state.environment.runtime().instance(),
                self.state.environment.agent().agent_handle(),
            ),
        };
        drop(adapter);
        receipt
    }
}

/// Stable descriptive identity of one exact terminal unload.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProtectedRowSoftmaxV1UnloadIdentityV1([u8; 32]);

impl ProtectedRowSoftmaxV1UnloadIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Inert terminal receipt for one exact completed lifecycle.
#[derive(Debug)]
pub struct UnloadedProtectedRowSoftmaxV1 {
    token_identity: ProtectedRowSoftmaxV1HostTokenIdentityV1,
    admission_identity: ProtectedRowSoftmaxV1AdmissionIdentityV1,
    finalized_artifact_identity: FinalizedWorkerV2HsacoIdentityV1,
    finalized_output_identity: ContentIdentityV1,
    executable_object: HsaExecutableObjectIdentityV1,
    kernel_object: HsaKernelObjectIdentityV1,
    dispatch_identity: [u8; 16],
    unload_identity: ProtectedRowSoftmaxV1UnloadIdentityV1,
}

impl UnloadedProtectedRowSoftmaxV1 {
    pub const fn token_identity(&self) -> ProtectedRowSoftmaxV1HostTokenIdentityV1 {
        self.token_identity
    }

    pub const fn admission_identity(&self) -> ProtectedRowSoftmaxV1AdmissionIdentityV1 {
        self.admission_identity
    }

    pub const fn finalized_artifact_identity(&self) -> FinalizedWorkerV2HsacoIdentityV1 {
        self.finalized_artifact_identity
    }

    pub const fn finalized_output_identity(&self) -> ContentIdentityV1 {
        self.finalized_output_identity
    }

    pub const fn executable_object(&self) -> HsaExecutableObjectIdentityV1 {
        self.executable_object
    }

    pub const fn kernel_object(&self) -> HsaKernelObjectIdentityV1 {
        self.kernel_object
    }

    pub const fn dispatch_identity(&self) -> [u8; 16] {
        self.dispatch_identity
    }

    pub const fn unload_identity(&self) -> ProtectedRowSoftmaxV1UnloadIdentityV1 {
        self.unload_identity
    }

    pub const fn proves_masked_execution(&self) -> bool {
        false
    }
}

#[repr(C, align(16))]
struct CompleteKernargV1 {
    bytes: [u8; COMPLETE_KERNARG_BYTES],
}

const _: () = assert!(std::mem::size_of::<CompleteKernargV1>() == COMPLETE_KERNARG_BYTES);
const _: () = assert!(std::mem::align_of::<CompleteKernargV1>() == HSA_KERNARG_ALIGNMENT as usize);

impl CompleteKernargV1 {
    fn from_explicit(explicit: &[u8; EXPLICIT_KERNARG_BYTES]) -> Self {
        let mut value = Self {
            bytes: [0; COMPLETE_KERNARG_BYTES],
        };
        value.bytes[..EXPLICIT_KERNARG_BYTES].copy_from_slice(explicit);
        value
    }
}

impl<R, A: ReviewedProtectedRowSoftmaxV1RuntimeAdapterV1> LoadedState<R, A> {
    fn dispatch_and_wait(
        mut self,
    ) -> Result<QuiescentState<R, A>, ProtectedRowSoftmaxV1DispatchErrorV1<A::Error>> {
        let geometry = exact_geometry();
        let explicit = self.kernarg.bytes[..EXPLICIT_KERNARG_BYTES].to_vec();
        let executable = self.executable.as_ref().expect("loaded executable");
        let kernel = self.kernel.as_ref().expect("loaded kernel");
        let adapter = self.adapter.as_mut().expect("loaded adapter");
        let implicit = reviewed_adapter_call(|| unsafe {
            adapter.initialize_implicit_kernarg(
                executable,
                kernel,
                geometry,
                EXPLICIT_KERNARG_BYTES,
                EXPLICIT_KERNARG_BYTES,
                IMPLICIT_KERNARG_BYTES,
                &mut self.kernarg.bytes,
            )
        })
        .map_err(ProtectedRowSoftmaxV1DispatchErrorV1::ImplicitAdapter)?;
        if self.kernarg.bytes[..EXPLICIT_KERNARG_BYTES] != *explicit {
            return Err(ProtectedRowSoftmaxV1DispatchErrorV1::ExplicitKernargMutation);
        }
        validate_implicit(&self.load, &self.resolution, geometry, &implicit)
            .map_err(ProtectedRowSoftmaxV1DispatchErrorV1::ImplicitObservation)?;

        let dispatch = reviewed_adapter_call(|| unsafe {
            adapter.launch_and_wait(executable, kernel, geometry, &mut self.kernarg.bytes)
        })
        .map_err(ProtectedRowSoftmaxV1DispatchErrorV1::DispatchAdapter)?;
        validate_dispatch(&self.load, &self.resolution, geometry, &dispatch)
            .map_err(ProtectedRowSoftmaxV1DispatchErrorV1::DispatchObservation)?;

        drop(self.kernel.take());
        Ok(QuiescentState {
            retained: self.retained.take(),
            adapter: self.adapter.take(),
            environment: self.environment.clone(),
            load: self.load.clone(),
            resolution: self.resolution.clone(),
            executable: self.executable.take(),
            dispatch_identity: dispatch.dispatch_identity(),
        })
    }
}

fn load_after_context_match<R, A>(
    retained: R,
    mut adapter: A,
) -> Result<LoadedState<R, A>, ProtectedRowSoftmaxV1LoadErrorV1<A::Error>>
where
    R: RetainedProtectedRowSoftmaxV1,
    A: ReviewedProtectedRowSoftmaxV1RuntimeAdapterV1,
{
    let environment = reviewed_adapter_call(|| unsafe { adapter.observe_environment() })
        .map_err(ProtectedRowSoftmaxV1LoadErrorV1::EnvironmentAdapter)?;
    validate_environment(retained.target_v1(), retained.ordinal_v1(), &environment)
        .map_err(ProtectedRowSoftmaxV1LoadErrorV1::Environment)?;

    enum BridgeLoadError<E> {
        FinalizedOutput,
        Adapter(E),
    }
    let loaded = reviewed_adapter_call(|| {
        retained.with_finalized_bytes_v1(|bytes, finalized_output| {
            if !finalized_output.matches(bytes) {
                return Err(BridgeLoadError::FinalizedOutput);
            }
            let digest = DigestAlgorithm::Sha256.calculate(bytes);
            // SAFETY: the bridge supplies the exact retained finalizer
            // bytes and the reviewed adapter contract owns exact loading.
            unsafe { adapter.load_executable(bytes, digest) }
                .map(|value| (value, digest, bytes.len() as u64))
                .map_err(BridgeLoadError::Adapter)
        })
    });
    let ((executable, load), digest, byte_len) = match loaded {
        Ok(value) => value,
        Err(BridgeLoadError::FinalizedOutput) => {
            return Err(ProtectedRowSoftmaxV1LoadErrorV1::FinalizedOutput);
        }
        Err(BridgeLoadError::Adapter(error)) => {
            return Err(ProtectedRowSoftmaxV1LoadErrorV1::LoadAdapter(error));
        }
    };
    if let Err(field) = validate_load(&environment, digest, byte_len, &load) {
        terminal_unload(&mut adapter, executable, &environment, &load);
        return Err(ProtectedRowSoftmaxV1LoadErrorV1::LoadObservation(field));
    }

    let (kernel, resolution) = match reviewed_adapter_call(|| unsafe {
        adapter.resolve_kernel(&executable, EXPORT_SYMBOL)
    }) {
        Ok(value) => value,
        Err(error) => {
            terminal_unload(&mut adapter, executable, &environment, &load);
            return Err(ProtectedRowSoftmaxV1LoadErrorV1::KernelAdapter(error));
        }
    };
    if let Err(field) = validate_resolution(&load, &resolution) {
        drop(kernel);
        terminal_unload(&mut adapter, executable, &environment, &load);
        return Err(ProtectedRowSoftmaxV1LoadErrorV1::KernelObservation(field));
    }

    let resources = match reviewed_adapter_call(|| unsafe {
        adapter.observe_protected_row_softmax_v1_kernel_resources(&executable, &kernel)
    }) {
        Ok(value) => value,
        Err(error) => {
            drop(kernel);
            terminal_unload(&mut adapter, executable, &environment, &load);
            return Err(ProtectedRowSoftmaxV1LoadErrorV1::ResourceAdapter(error));
        }
    };
    if let Err(field) = validate_resources(&load, &resolution, resources) {
        drop(kernel);
        terminal_unload(&mut adapter, executable, &environment, &load);
        return Err(ProtectedRowSoftmaxV1LoadErrorV1::ResourceObservation(field));
    }

    let kernarg = CompleteKernargV1::from_explicit(retained.explicit_kernarg_v1());
    if !(kernarg.bytes.as_ptr() as usize).is_multiple_of(HSA_KERNARG_ALIGNMENT as usize) {
        drop(kernel);
        terminal_unload(&mut adapter, executable, &environment, &load);
        return Err(ProtectedRowSoftmaxV1LoadErrorV1::KernelObservation(
            "staging kernarg alignment",
        ));
    }
    Ok(LoadedState {
        retained: Some(retained),
        adapter: Some(adapter),
        environment,
        load,
        resolution,
        resources,
        executable: Some(executable),
        kernel: Some(kernel),
        kernarg,
    })
}

fn validate_join(
    token: &ProtectedRowSoftmaxV1HostTokenV1,
    host: &GeneratedProtectedRowSoftmaxV1HostAdapterV1<'_, '_>,
) -> Result<(), ProtectedRowSoftmaxV1JoinErrorV1> {
    for (matches, field) in [
        (token.identity().as_bytes() != &[0; 32], "identity"),
        (
            token.admission_identity().as_bytes() != &[0; 32],
            "admission identity",
        ),
        (
            token.finalized_artifact_identity().as_bytes() != &[0; 32],
            "artifact identity",
        ),
        (
            token.finalized_output_identity().byte_len() != 0,
            "finalized output",
        ),
        (token.target() == TARGET, "target"),
        (token.row_elements() == 64, "row width"),
        (token.grid_size() == GRID, "grid"),
        (token.workgroup_size() == WORKGROUP, "workgroup"),
        (
            token.explicit_kernarg_bytes() == EXPLICIT_KERNARG_BYTES as u32,
            "explicit ABI",
        ),
        (
            token.total_kernarg_bytes() == COMPLETE_KERNARG_BYTES as u32,
            "complete ABI",
        ),
    ] {
        if !matches {
            return Err(ProtectedRowSoftmaxV1JoinErrorV1::TokenField(field));
        }
    }
    for (matches, field) in [
        (host.target() == TARGET, "target"),
        (host.row_elements() == 64, "row width"),
        (host.grid() == GRID, "grid"),
        (host.workgroup() == WORKGROUP, "workgroup"),
        (
            host.explicit_kernarg_byte_len() == EXPLICIT_KERNARG_BYTES,
            "explicit ABI",
        ),
        (
            host.complete_kernarg_byte_len() == COMPLETE_KERNARG_BYTES as u32,
            "complete ABI",
        ),
        (host.kernarg_alignment() == 8, "ABI alignment"),
        (host.is_unmasked_all_64_profile(), "activity policy"),
    ] {
        if !matches {
            return Err(ProtectedRowSoftmaxV1JoinErrorV1::HostField(field));
        }
    }
    Ok(())
}

fn reviewed_adapter_call<T>(call: impl FnOnce() -> T) -> T {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(call)) {
        Ok(value) => value,
        Err(payload) => {
            std::mem::forget(payload);
            std::process::abort()
        }
    }
}

fn terminal_unload<A: ReviewedProtectedRowSoftmaxV1RuntimeAdapterV1>(
    adapter: &mut A,
    executable: A::Executable,
    environment: &HsaEnvironmentObservationV1,
    load: &HsaCodeObjectLoadObservationV1,
) -> HsaUnloadObservationV1 {
    let unload = match reviewed_adapter_call(|| unsafe { adapter.unload_executable(executable) }) {
        Ok(value) => value,
        Err(error) => {
            std::mem::forget(error);
            std::process::abort()
        }
    };
    if validate_unload(environment, load, &unload).is_err() {
        std::process::abort();
    }
    unload
}

fn validate_environment(
    target: &str,
    ordinal: i32,
    environment: &HsaEnvironmentObservationV1,
) -> Result<(), &'static str> {
    if target != TARGET {
        return Err("requested target");
    }
    let expected = AmdTargetId::parse(TARGET).expect("static target");
    let actual = environment.physical_device().target();
    for (matches, field) in [
        (
            expected.is_compatible_with_observed(&actual),
            "physical target",
        ),
        (environment.agent().target() == actual, "agent target"),
        (
            environment.physical_device().hip_ordinal() == ordinal,
            "HIP ordinal",
        ),
        (
            environment.agent().runtime_instance() == environment.runtime().instance(),
            "runtime",
        ),
        (
            environment.agent().physical_device_uuid() == environment.physical_device().uuid(),
            "device",
        ),
    ] {
        if !matches {
            return Err(field);
        }
    }
    Ok(())
}

fn validate_load(
    environment: &HsaEnvironmentObservationV1,
    digest: PayloadDigest,
    byte_len: u64,
    load: &HsaCodeObjectLoadObservationV1,
) -> Result<(), &'static str> {
    let expected = HsaCodeObjectLoadObservationV1::new(
        digest,
        byte_len,
        environment.runtime().instance(),
        environment.agent().agent_handle(),
        load.executable_object(),
    );
    (load == &expected)
        .then_some(())
        .ok_or("identity or content")
}

fn validate_resolution(
    load: &HsaCodeObjectLoadObservationV1,
    resolution: &HsaKernelResolutionObservationV1,
) -> Result<(), &'static str> {
    let expected = HsaKernelResolutionObservationV1::new(
        load.executable_object(),
        resolution.kernel_object(),
        EXPORT_SYMBOL,
        COMPLETE_KERNARG_BYTES as u64,
        HSA_KERNARG_ALIGNMENT,
    )
    .expect("static resolution");
    (resolution == &expected)
        .then_some(())
        .ok_or("object, symbol, size, or alignment")
}

fn validate_resources(
    load: &HsaCodeObjectLoadObservationV1,
    resolution: &HsaKernelResolutionObservationV1,
    resources: ProtectedRowSoftmaxV1KernelResourceObservationV1,
) -> Result<(), &'static str> {
    let expected = ProtectedRowSoftmaxV1KernelResourceObservationV1::new(
        load.executable_object(),
        resolution.kernel_object(),
        STATIC_LDS_BYTES,
        PRIVATE_SEGMENT_BYTES,
    );
    (resources == expected)
        .then_some(())
        .ok_or("object, group segment, or private segment")
}

fn validate_implicit(
    load: &HsaCodeObjectLoadObservationV1,
    resolution: &HsaKernelResolutionObservationV1,
    geometry: HsaLaunchGeometryV1,
    observation: &HsaImplicitKernargInitializationObservationV1,
) -> Result<(), &'static str> {
    let expected = HsaImplicitKernargInitializationObservationV1::new(
        load.executable_object(),
        resolution.kernel_object(),
        geometry,
        EXPLICIT_KERNARG_BYTES as u64,
        EXPLICIT_KERNARG_BYTES as u64,
        IMPLICIT_KERNARG_BYTES as u64,
        true,
    );
    (observation == &expected)
        .then_some(())
        .ok_or("object, geometry, span, or completion")
}

fn validate_dispatch(
    load: &HsaCodeObjectLoadObservationV1,
    resolution: &HsaKernelResolutionObservationV1,
    geometry: HsaLaunchGeometryV1,
    dispatch: &HsaDispatchObservationV1,
) -> Result<(), &'static str> {
    let expected = HsaDispatchObservationV1::new(
        dispatch.dispatch_identity(),
        load.executable_object(),
        resolution.kernel_object(),
        geometry,
        true,
    )
    .expect("adapter supplied valid dispatch identity");
    (dispatch == &expected)
        .then_some(())
        .ok_or("object, geometry, or completion")
}

fn validate_unload(
    environment: &HsaEnvironmentObservationV1,
    load: &HsaCodeObjectLoadObservationV1,
    unload: &HsaUnloadObservationV1,
) -> Result<(), &'static str> {
    let expected = HsaUnloadObservationV1::new(
        load.executable_object(),
        environment.runtime().instance(),
        environment.agent().agent_handle(),
        true,
    );
    (unload == &expected)
        .then_some(())
        .ok_or("object, runtime, agent, or completion")
}

fn exact_geometry() -> HsaLaunchGeometryV1 {
    HsaLaunchGeometryV1::new(GRID, WORKGROUP, DYNAMIC_LDS_BYTES)
}

fn unload_identity(
    unload: &HsaUnloadObservationV1,
    runtime_instance: [u8; 16],
    agent_handle: u64,
) -> ProtectedRowSoftmaxV1UnloadIdentityV1 {
    let mut digest = Sha256::new();
    for field in [
        UNLOAD_IDENTITY_DOMAIN_V1,
        unload.executable_object().as_bytes(),
        &runtime_instance,
        &agent_handle.to_le_bytes(),
        &[u8::from(unload.released())],
    ] {
        digest.update((field.len() as u64).to_le_bytes());
        digest.update(field);
    }
    ProtectedRowSoftmaxV1UnloadIdentityV1(digest.finalize().into())
}

#[cfg(test)]
#[path = "protected_row_softmax_v1_lifecycle_tests.rs"]
mod tests;
