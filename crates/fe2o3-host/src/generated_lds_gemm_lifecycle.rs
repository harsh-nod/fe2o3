//! Protected, one-shot HSA lifecycle for the exact LDS GEMM Slice1 profile.
//!
//! This module joins the inert direct-LLVM finalizer receipt with the generated
//! typed host request. It deliberately does not create publication evidence,
//! expose finalized bytes, expose native handles, or provide a generic launch
//! operation.

use crate::{
    HsaCodeObjectLoadObservationV1, HsaDispatchObservationV1, HsaEnvironmentObservationV1,
    HsaExecutableObjectIdentityV1, HsaImplicitKernargInitializationObservationV1,
    HsaKernelObjectIdentityV1, HsaKernelResolutionObservationV1, HsaLaunchGeometryV1,
    HsaUnloadObservationV1, ReviewedHsaImplicitKernargAdapterV1,
    generated_lds_gemm::GeneratedLdsGemmSlice1HostAdapterV1,
};
use fe2o3_amd_target::AmdTargetId;
use fe2o3_artifacts::{DigestAlgorithm, PayloadDigest};
use fe2o3_core::ContextIdentity;
use fe2o3_hsaco_finalize::{
    ExactLdsGemmBufferContractV1, ExactLdsGemmBufferRoleV1, ExactLdsGemmContractV1,
    ExactLdsGemmElementV1, ExactLdsGemmProfileIdV1, ExactLdsGemmProfileIdentityV1,
    FinalizedExactLdsGemmHsacoIdentityV1, FinalizedExactLdsGemmHsacoV1,
    InspectedExactLdsGemmCompilerImportIdentityV1,
};
use fe2o3_kernel_descriptor::{AccessMode, AliasSemantics, CodeObjectVersion, OwnershipSemantics};
use sha2::{Digest, Sha256};
use std::{error::Error, fmt};

const TARGET: &str = "gfx942:xnack-";
const EXPORT_SYMBOL: &str = "tiled_gemm_lds_v1";
const GRID: [u32; 3] = [1, 1, 1];
const WORKGROUP: [u32; 3] = [64, 1, 1];
const WAVEFRONT_SIZE: u32 = 64;
const EXPLICIT_KERNARG_BYTES: usize = 48;
const COMPLETE_KERNARG_BYTES: usize = 304;
const IMPLICIT_KERNARG_BYTES: usize = COMPLETE_KERNARG_BYTES - EXPLICIT_KERNARG_BYTES;
const CONTRACT_KERNARG_ALIGNMENT: u32 = 8;
const HSA_KERNARG_ALIGNMENT: u64 = 16;
const STATIC_LDS_BYTES: u32 = 1_024;
const DYNAMIC_LDS_BYTES: u32 = 0;
const PRIVATE_SEGMENT_BYTES: u32 = 0;
const LDS_ALLOCATIONS: u32 = 2;
const LDS_BYTES_PER_ALLOCATION: u32 = 512;
const LDS_ALIGNMENT: u32 = 16;
const LENGTH_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/EXACT-LDS-GEMM-LENGTH/V1\0";
const UNLOAD_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/EXACT-LDS-GEMM/UNLOAD/V1\0";

/// Runtime-reported resource facts for one exact executable/kernel pair.
///
/// This is descriptive evidence only. It contains no native handle and grants
/// no load, launch, or unload authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactLdsGemmKernelResourceObservationV1 {
    executable_object: HsaExecutableObjectIdentityV1,
    kernel_object: HsaKernelObjectIdentityV1,
    group_segment_size: u32,
    private_segment_size: u32,
}

impl ExactLdsGemmKernelResourceObservationV1 {
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

/// Reviewed exact-profile extension over the generic HSA lifecycle.
///
/// # Safety
///
/// `context_identity_v1` must identify the exact `GpuContext` retained by this
/// adapter, not merely another wrapper for the same ordinal. Resource
/// observations must be queried from the supplied private executable and
/// kernel handles and must remain bound to those exact objects. Neither method
/// may unwind. Implementations inherit every safety obligation from
/// [`ReviewedHsaImplicitKernargAdapterV1`].
pub unsafe trait ReviewedExactLdsGemmRuntimeAdapterV1:
    ReviewedHsaImplicitKernargAdapterV1
{
    /// Reports the exact retained HIP context wrapper identity.
    ///
    /// # Safety
    ///
    /// The returned identity must satisfy this trait's safety contract.
    unsafe fn context_identity_v1(&mut self) -> ContextIdentity;

    /// Queries exact static-LDS and private-segment use for `kernel`.
    ///
    /// # Safety
    ///
    /// The observation must describe only the supplied private handles.
    unsafe fn observe_exact_lds_gemm_kernel_resources_v1(
        &mut self,
        executable: &Self::Executable,
        kernel: &Self::Kernel,
    ) -> Result<ExactLdsGemmKernelResourceObservationV1, Self::Error>;
}

/// Rejection while joining exact finalizer and generated-host evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ExactLdsGemmSlice1JoinErrorV1 {
    ImportIdentity,
    ProfileIdentity,
    Contract,
    FinalizedOutput,
    ContractField(&'static str),
    BufferContract { index: usize },
    HostLengthIdentity { index: usize },
    HostField(&'static str),
}

impl fmt::Display for ExactLdsGemmSlice1JoinErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ImportIdentity => formatter.write_str("compiler-import identity mismatch"),
            Self::ProfileIdentity => formatter.write_str("Slice1 profile identity mismatch"),
            Self::Contract => formatter.write_str("finalizer and host contracts differ"),
            Self::FinalizedOutput => {
                formatter.write_str("finalized output identity does not match retained bytes")
            }
            Self::ContractField(field) => write!(formatter, "Slice1 contract {field} drifted"),
            Self::BufferContract { index } => {
                write!(formatter, "Slice1 buffer contract {index} drifted")
            }
            Self::HostLengthIdentity { index } => {
                write!(formatter, "host length identity {index} drifted")
            }
            Self::HostField(field) => write!(formatter, "generated host {field} drifted"),
        }
    }
}

impl Error for ExactLdsGemmSlice1JoinErrorV1 {}

/// Recoverable failure before packet publication or after proven quiescence.
///
/// Every failure after a successful load performs one terminally checked
/// unload before this error is returned. Unload ambiguity and adapter unwinds
/// abort the process instead of becoming ordinary errors.
#[derive(Debug)]
#[non_exhaustive]
pub enum ExactLdsGemmSlice1LoadErrorV1<E> {
    ContextIdentity,
    EnvironmentAdapter(E),
    Environment(&'static str),
    LoadAdapter(E),
    LoadObservation(&'static str),
    KernelAdapter(E),
    KernelObservation(&'static str),
    ResourceAdapter(E),
    ResourceObservation(&'static str),
}

impl<E: fmt::Display> fmt::Display for ExactLdsGemmSlice1LoadErrorV1<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContextIdentity => formatter.write_str("exact HIP context identity mismatch"),
            Self::EnvironmentAdapter(error) => write!(formatter, "HSA environment failed: {error}"),
            Self::Environment(field) => write!(formatter, "HSA environment {field} drifted"),
            Self::LoadAdapter(error) => write!(formatter, "HSA executable load failed: {error}"),
            Self::LoadObservation(field) => write!(formatter, "HSA load {field} drifted"),
            Self::KernelAdapter(error) => {
                write!(formatter, "HSA kernel resolution failed: {error}")
            }
            Self::KernelObservation(field) => {
                write!(formatter, "HSA kernel resolution {field} drifted")
            }
            Self::ResourceAdapter(error) => write!(formatter, "HSA resource query failed: {error}"),
            Self::ResourceObservation(field) => {
                write!(formatter, "HSA kernel resource {field} drifted")
            }
        }
    }
}

impl<E: Error + 'static> Error for ExactLdsGemmSlice1LoadErrorV1<E> {
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

/// Recoverable failure before publication or after synchronous quiescence.
#[derive(Debug)]
#[non_exhaustive]
pub enum ExactLdsGemmSlice1DispatchErrorV1<E> {
    ImplicitAdapter(E),
    ImplicitObservation(&'static str),
    ExplicitKernargMutation,
    DispatchAdapter(E),
    DispatchObservation(&'static str),
}

impl<E: fmt::Display> fmt::Display for ExactLdsGemmSlice1DispatchErrorV1<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ImplicitAdapter(error) => write!(formatter, "implicit kernarg failed: {error}"),
            Self::ImplicitObservation(field) => {
                write!(formatter, "implicit kernarg {field} drifted")
            }
            Self::ExplicitKernargMutation => {
                formatter.write_str("implicit initialization changed explicit kernarg bytes")
            }
            Self::DispatchAdapter(error) => write!(formatter, "HSA dispatch failed: {error}"),
            Self::DispatchObservation(field) => {
                write!(formatter, "HSA dispatch {field} drifted")
            }
        }
    }
}

impl<E: Error + 'static> Error for ExactLdsGemmSlice1DispatchErrorV1<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ImplicitAdapter(error) | Self::DispatchAdapter(error) => Some(error),
            _ => None,
        }
    }
}

/// Inert exact evidence joined before any runtime is observed.
///
/// This value is linear, has private fields, and exposes neither HSACO bytes nor
/// native/runtime authority.
#[must_use = "the joined Slice1 request must be consumed by the protected lifecycle"]
pub struct JoinedExactLdsGemmSlice1V1<'a, 'b, 'c> {
    artifact: FinalizedExactLdsGemmHsacoV1,
    host: GeneratedLdsGemmSlice1HostAdapterV1<'a, 'b, 'c>,
}

impl fmt::Debug for JoinedExactLdsGemmSlice1V1<'_, '_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JoinedExactLdsGemmSlice1V1")
            .field("finalizer_identity", &self.artifact.identity())
            .field("import_identity", &self.artifact.import_identity())
            .field("profile_identity", &self.artifact.profile_identity())
            .finish_non_exhaustive()
    }
}

/// Consumes and reconciles the #97 artifact with the #99 generated request.
///
/// Validation completes before an adapter can be supplied, so join rejection
/// cannot observe a runtime or create native authority.
pub fn join_exact_lds_gemm_slice1_v1<'a, 'b, 'c>(
    artifact: FinalizedExactLdsGemmHsacoV1,
    host: GeneratedLdsGemmSlice1HostAdapterV1<'a, 'b, 'c>,
) -> Result<JoinedExactLdsGemmSlice1V1<'a, 'b, 'c>, ExactLdsGemmSlice1JoinErrorV1> {
    validate_join(JoinFactsV1::from_exact(&artifact, &host))?;
    Ok(JoinedExactLdsGemmSlice1V1 { artifact, host })
}

impl<'a, 'b, 'c> JoinedExactLdsGemmSlice1V1<'a, 'b, 'c> {
    pub const fn finalizer_identity(&self) -> FinalizedExactLdsGemmHsacoIdentityV1 {
        self.artifact.identity()
    }

    pub const fn import_identity(&self) -> InspectedExactLdsGemmCompilerImportIdentityV1 {
        self.artifact.import_identity()
    }

    pub const fn profile_identity(&self) -> ExactLdsGemmProfileIdentityV1 {
        self.artifact.profile_identity()
    }

    /// Observes and loads this exact joined request into one reviewed runtime.
    pub fn load<A: ReviewedExactLdsGemmRuntimeAdapterV1>(
        self,
        mut adapter: A,
    ) -> Result<LoadedExactLdsGemmSlice1V1<'a, 'b, 'c, A>, ExactLdsGemmSlice1LoadErrorV1<A::Error>>
    {
        // SAFETY: the extension contract requires this to identify the exact
        // retained GpuContext and forbids unwinding.
        let context_identity = reviewed_adapter_call(|| unsafe { adapter.context_identity_v1() });
        if !self
            .host
            .observed_context_v1()
            .matches_core_context_identity_v1(context_identity)
        {
            return Err(ExactLdsGemmSlice1LoadErrorV1::ContextIdentity);
        }
        let state = load_after_context_match(self, adapter)?;
        Ok(LoadedExactLdsGemmSlice1V1 { state })
    }
}

trait RetainedExactLdsGemmSlice1V1 {
    fn target_v1(&self) -> &str;
    fn ordinal_v1(&self) -> i32;
    fn finalized_bytes_v1(&self) -> &[u8];
    fn explicit_kernarg_v1(&self) -> &[u8; EXPLICIT_KERNARG_BYTES];
}

impl RetainedExactLdsGemmSlice1V1 for JoinedExactLdsGemmSlice1V1<'_, '_, '_> {
    fn target_v1(&self) -> &str {
        self.host.target()
    }

    fn ordinal_v1(&self) -> i32 {
        self.host.observed_context_v1().device().ordinal()
    }

    fn finalized_bytes_v1(&self) -> &[u8] {
        self.artifact.exact_finalized_bytes()
    }

    fn explicit_kernarg_v1(&self) -> &[u8; EXPLICIT_KERNARG_BYTES] {
        self.host.explicit_kernarg_bytes_v1()
    }
}

struct LoadedExactStateV1<R, A: ReviewedExactLdsGemmRuntimeAdapterV1> {
    retained: Option<R>,
    adapter: Option<A>,
    environment: HsaEnvironmentObservationV1,
    load: HsaCodeObjectLoadObservationV1,
    resolution: HsaKernelResolutionObservationV1,
    resources: ExactLdsGemmKernelResourceObservationV1,
    executable: Option<A::Executable>,
    kernel: Option<A::Kernel>,
    kernarg: ExactSlice1KernargV1,
}

impl<R, A: ReviewedExactLdsGemmRuntimeAdapterV1> Drop for LoadedExactStateV1<R, A> {
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

/// Loaded exact Slice1 authority. It is linear and has no raw launch method.
#[must_use = "the loaded Slice1 lifecycle must dispatch or be terminally unloaded"]
pub struct LoadedExactLdsGemmSlice1V1<'a, 'b, 'c, A: ReviewedExactLdsGemmRuntimeAdapterV1> {
    state: LoadedExactStateV1<JoinedExactLdsGemmSlice1V1<'a, 'b, 'c>, A>,
}

impl<A: ReviewedExactLdsGemmRuntimeAdapterV1> fmt::Debug
    for LoadedExactLdsGemmSlice1V1<'_, '_, '_, A>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LoadedExactLdsGemmSlice1V1")
            .field("load", &self.state.load)
            .field("resolution", &self.state.resolution)
            .field("resources", &self.state.resources)
            .finish_non_exhaustive()
    }
}

impl<'a, 'b, 'c, A: ReviewedExactLdsGemmRuntimeAdapterV1>
    LoadedExactLdsGemmSlice1V1<'a, 'b, 'c, A>
{
    /// Initializes the exact COV6 suffix, submits once, and waits synchronously.
    pub fn dispatch_and_wait(
        self,
    ) -> Result<CompletedExactLdsGemmSlice1V1<A>, ExactLdsGemmSlice1DispatchErrorV1<A::Error>> {
        let quiescent = self.state.dispatch_and_wait()?;
        let retained = quiescent
            .retained
            .as_ref()
            .expect("quiescent lifecycle retains the joined request");
        let receipt = CompletionReceiptIdentitiesV1 {
            finalizer_identity: retained.artifact.identity(),
            import_identity: retained.artifact.import_identity(),
            profile_identity: retained.artifact.profile_identity(),
        };
        let state = quiescent.release_retained();
        Ok(CompletedExactLdsGemmSlice1V1 { state, receipt })
    }
}

struct QuiescentExactStateV1<R, A: ReviewedExactLdsGemmRuntimeAdapterV1> {
    retained: Option<R>,
    adapter: Option<A>,
    environment: HsaEnvironmentObservationV1,
    load: HsaCodeObjectLoadObservationV1,
    resolution: HsaKernelResolutionObservationV1,
    executable: Option<A::Executable>,
    dispatch_identity: [u8; 16],
}

impl<R, A: ReviewedExactLdsGemmRuntimeAdapterV1> Drop for QuiescentExactStateV1<R, A> {
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

impl<R, A: ReviewedExactLdsGemmRuntimeAdapterV1> QuiescentExactStateV1<R, A> {
    fn release_retained(mut self) -> CompletedExactStateV1<A> {
        // Synchronous completion has been validated. The artifact and generated
        // A/B/C leases are no longer GPU-reachable and become reusable here,
        // independently of the executable's later terminal unload.
        drop(self.retained.take());
        CompletedExactStateV1 {
            adapter: self.adapter.take(),
            environment: self.environment.clone(),
            load: self.load.clone(),
            resolution: self.resolution.clone(),
            executable: self.executable.take(),
            dispatch_identity: self.dispatch_identity,
        }
    }
}

struct CompletedExactStateV1<A: ReviewedExactLdsGemmRuntimeAdapterV1> {
    adapter: Option<A>,
    environment: HsaEnvironmentObservationV1,
    load: HsaCodeObjectLoadObservationV1,
    resolution: HsaKernelResolutionObservationV1,
    executable: Option<A::Executable>,
    dispatch_identity: [u8; 16],
}

impl<A: ReviewedExactLdsGemmRuntimeAdapterV1> Drop for CompletedExactStateV1<A> {
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

#[derive(Clone, Copy, Debug)]
struct CompletionReceiptIdentitiesV1 {
    finalizer_identity: FinalizedExactLdsGemmHsacoIdentityV1,
    import_identity: InspectedExactLdsGemmCompilerImportIdentityV1,
    profile_identity: ExactLdsGemmProfileIdentityV1,
}

/// Quiescent exact Slice1 dispatch retaining only runtime unload authority.
///
/// This type has no buffer lifetime parameters: the joined artifact and A/B/C
/// leases are dropped before it is returned.
#[must_use = "the completed Slice1 lifecycle must be terminally unloaded"]
pub struct CompletedExactLdsGemmSlice1V1<A: ReviewedExactLdsGemmRuntimeAdapterV1> {
    state: CompletedExactStateV1<A>,
    receipt: CompletionReceiptIdentitiesV1,
}

impl<A: ReviewedExactLdsGemmRuntimeAdapterV1> fmt::Debug for CompletedExactLdsGemmSlice1V1<A> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompletedExactLdsGemmSlice1V1")
            .field("executable_object", &self.state.load.executable_object())
            .field("kernel_object", &self.state.resolution.kernel_object())
            .field("dispatch_identity", &self.state.dispatch_identity)
            .finish_non_exhaustive()
    }
}

impl<A: ReviewedExactLdsGemmRuntimeAdapterV1> CompletedExactLdsGemmSlice1V1<A> {
    pub const fn finalizer_identity(&self) -> FinalizedExactLdsGemmHsacoIdentityV1 {
        self.receipt.finalizer_identity
    }

    pub const fn import_identity(&self) -> InspectedExactLdsGemmCompilerImportIdentityV1 {
        self.receipt.import_identity
    }

    pub const fn profile_identity(&self) -> ExactLdsGemmProfileIdentityV1 {
        self.receipt.profile_identity
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

    pub fn unload(mut self) -> UnloadedExactLdsGemmSlice1V1 {
        let mut adapter = self
            .state
            .adapter
            .take()
            .expect("completed lifecycle retains its adapter");
        let executable = self
            .state
            .executable
            .take()
            .expect("completed lifecycle retains its executable");
        let unload = terminal_unload(
            &mut adapter,
            executable,
            &self.state.environment,
            &self.state.load,
        );
        let receipt = UnloadedExactLdsGemmSlice1V1 {
            finalizer_identity: self.receipt.finalizer_identity,
            import_identity: self.receipt.import_identity,
            profile_identity: self.receipt.profile_identity,
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

/// Stable descriptive identity for one validated terminal unload.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExactLdsGemmUnloadIdentityV1([u8; 32]);

impl ExactLdsGemmUnloadIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Terminal, inert receipt for one completed exact Slice1 lifecycle.
#[derive(Debug)]
pub struct UnloadedExactLdsGemmSlice1V1 {
    finalizer_identity: FinalizedExactLdsGemmHsacoIdentityV1,
    import_identity: InspectedExactLdsGemmCompilerImportIdentityV1,
    profile_identity: ExactLdsGemmProfileIdentityV1,
    executable_object: HsaExecutableObjectIdentityV1,
    kernel_object: HsaKernelObjectIdentityV1,
    dispatch_identity: [u8; 16],
    unload_identity: ExactLdsGemmUnloadIdentityV1,
}

impl UnloadedExactLdsGemmSlice1V1 {
    pub const fn finalizer_identity(&self) -> FinalizedExactLdsGemmHsacoIdentityV1 {
        self.finalizer_identity
    }

    pub const fn import_identity(&self) -> InspectedExactLdsGemmCompilerImportIdentityV1 {
        self.import_identity
    }

    pub const fn profile_identity(&self) -> ExactLdsGemmProfileIdentityV1 {
        self.profile_identity
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

    pub const fn unload_identity(&self) -> ExactLdsGemmUnloadIdentityV1 {
        self.unload_identity
    }

    pub const fn proves_verus_verification(&self) -> bool {
        false
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }
}

#[repr(C, align(16))]
struct ExactSlice1KernargV1 {
    bytes: [u8; COMPLETE_KERNARG_BYTES],
}

const _: () = assert!(std::mem::size_of::<ExactSlice1KernargV1>() == COMPLETE_KERNARG_BYTES);
const _: () =
    assert!(std::mem::align_of::<ExactSlice1KernargV1>() == HSA_KERNARG_ALIGNMENT as usize);

impl ExactSlice1KernargV1 {
    fn from_explicit(explicit: &[u8; EXPLICIT_KERNARG_BYTES]) -> Self {
        let mut value = Self {
            bytes: [0; COMPLETE_KERNARG_BYTES],
        };
        value.bytes[..EXPLICIT_KERNARG_BYTES].copy_from_slice(explicit);
        value
    }
}

impl<R: RetainedExactLdsGemmSlice1V1, A: ReviewedExactLdsGemmRuntimeAdapterV1>
    LoadedExactStateV1<R, A>
{
    fn dispatch_and_wait(
        mut self,
    ) -> Result<QuiescentExactStateV1<R, A>, ExactLdsGemmSlice1DispatchErrorV1<A::Error>> {
        let geometry = exact_geometry();
        let explicit = self.kernarg.bytes[..EXPLICIT_KERNARG_BYTES].to_vec();
        let executable = self
            .executable
            .as_ref()
            .expect("loaded lifecycle retains its executable");
        let kernel = self
            .kernel
            .as_ref()
            .expect("loaded lifecycle retains its kernel");
        let adapter = self
            .adapter
            .as_mut()
            .expect("loaded lifecycle retains its adapter");

        // SAFETY: exact private handles and spans are retained by this state;
        // the reviewed extension owns COV6 initialization and quiescence.
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
        .map_err(ExactLdsGemmSlice1DispatchErrorV1::ImplicitAdapter)?;
        if self.kernarg.bytes[..EXPLICIT_KERNARG_BYTES] != *explicit {
            return Err(ExactLdsGemmSlice1DispatchErrorV1::ExplicitKernargMutation);
        }
        validate_implicit_observation(&self.load, &self.resolution, geometry, &implicit)
            .map_err(ExactLdsGemmSlice1DispatchErrorV1::ImplicitObservation)?;

        // SAFETY: the exact explicit prefix and reviewed implicit suffix are
        // complete, and all buffer leases remain retained in `self.retained`.
        let dispatch = reviewed_adapter_call(|| unsafe {
            adapter.launch_and_wait(executable, kernel, geometry, &mut self.kernarg.bytes)
        })
        .map_err(ExactLdsGemmSlice1DispatchErrorV1::DispatchAdapter)?;
        validate_dispatch_observation(&self.load, &self.resolution, geometry, &dispatch)
            .map_err(ExactLdsGemmSlice1DispatchErrorV1::DispatchObservation)?;

        // The selected kernel must die before either explicit or drop-time
        // executable unload can run.
        drop(self.kernel.take());
        let completed = QuiescentExactStateV1 {
            retained: self.retained.take(),
            adapter: self.adapter.take(),
            environment: self.environment.clone(),
            load: self.load.clone(),
            resolution: self.resolution.clone(),
            executable: self.executable.take(),
            dispatch_identity: dispatch.dispatch_identity(),
        };
        Ok(completed)
    }
}

fn load_after_context_match<R, A>(
    retained: R,
    mut adapter: A,
) -> Result<LoadedExactStateV1<R, A>, ExactLdsGemmSlice1LoadErrorV1<A::Error>>
where
    R: RetainedExactLdsGemmSlice1V1,
    A: ReviewedExactLdsGemmRuntimeAdapterV1,
{
    let environment = reviewed_adapter_call(|| unsafe { adapter.observe_environment() })
        .map_err(ExactLdsGemmSlice1LoadErrorV1::EnvironmentAdapter)?;
    validate_environment(retained.target_v1(), retained.ordinal_v1(), &environment)
        .map_err(ExactLdsGemmSlice1LoadErrorV1::Environment)?;

    let bytes = retained.finalized_bytes_v1();
    let digest = DigestAlgorithm::Sha256.calculate(bytes);
    let byte_len = u64::try_from(bytes.len())
        .map_err(|_| ExactLdsGemmSlice1LoadErrorV1::LoadObservation("byte length"))?;
    let (executable, load) =
        reviewed_adapter_call(|| unsafe { adapter.load_executable(bytes, digest) })
            .map_err(ExactLdsGemmSlice1LoadErrorV1::LoadAdapter)?;
    if let Err(field) = validate_load_observation(&environment, digest, byte_len, &load) {
        terminal_unload(&mut adapter, executable, &environment, &load);
        return Err(ExactLdsGemmSlice1LoadErrorV1::LoadObservation(field));
    }

    let (kernel, resolution) = match reviewed_adapter_call(|| unsafe {
        adapter.resolve_kernel(&executable, EXPORT_SYMBOL)
    }) {
        Ok(value) => value,
        Err(error) => {
            terminal_unload(&mut adapter, executable, &environment, &load);
            return Err(ExactLdsGemmSlice1LoadErrorV1::KernelAdapter(error));
        }
    };
    if let Err(field) = validate_kernel_observation(&load, &resolution) {
        drop(kernel);
        terminal_unload(&mut adapter, executable, &environment, &load);
        return Err(ExactLdsGemmSlice1LoadErrorV1::KernelObservation(field));
    }

    let resources = match reviewed_adapter_call(|| unsafe {
        adapter.observe_exact_lds_gemm_kernel_resources_v1(&executable, &kernel)
    }) {
        Ok(value) => value,
        Err(error) => {
            drop(kernel);
            terminal_unload(&mut adapter, executable, &environment, &load);
            return Err(ExactLdsGemmSlice1LoadErrorV1::ResourceAdapter(error));
        }
    };
    if let Err(field) = validate_resource_observation(&load, &resolution, resources) {
        drop(kernel);
        terminal_unload(&mut adapter, executable, &environment, &load);
        return Err(ExactLdsGemmSlice1LoadErrorV1::ResourceObservation(field));
    }

    let kernarg = ExactSlice1KernargV1::from_explicit(retained.explicit_kernarg_v1());
    if !(kernarg.bytes.as_ptr() as usize).is_multiple_of(HSA_KERNARG_ALIGNMENT as usize) {
        drop(kernel);
        terminal_unload(&mut adapter, executable, &environment, &load);
        return Err(ExactLdsGemmSlice1LoadErrorV1::KernelObservation(
            "staging kernarg alignment",
        ));
    }
    Ok(LoadedExactStateV1 {
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

fn reviewed_adapter_call<T>(call: impl FnOnce() -> T) -> T {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(call)) {
        Ok(value) => value,
        Err(payload) => {
            std::mem::forget(payload);
            std::process::abort()
        }
    }
}

fn terminal_unload<A: ReviewedExactLdsGemmRuntimeAdapterV1>(
    adapter: &mut A,
    executable: A::Executable,
    environment: &HsaEnvironmentObservationV1,
    load: &HsaCodeObjectLoadObservationV1,
) -> HsaUnloadObservationV1 {
    let unload = match reviewed_adapter_call(|| unsafe { adapter.unload_executable(executable) }) {
        Ok(unload) => unload,
        Err(error) => {
            std::mem::forget(error);
            std::process::abort()
        }
    };
    if validate_unload_observation(environment, load, &unload).is_err() {
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
    let expected = AmdTargetId::parse(TARGET).expect("exact static target is canonical");
    let actual = environment.physical_device().target();
    for (matches, field) in [
        (
            expected.is_compatible_with_observed(&actual),
            "physical-device target",
        ),
        (environment.agent().target() == actual, "agent target"),
        (
            environment.physical_device().hip_ordinal() == ordinal,
            "HIP ordinal",
        ),
        (
            environment.agent().runtime_instance() == environment.runtime().instance(),
            "runtime instance",
        ),
        (
            environment.agent().physical_device_uuid() == environment.physical_device().uuid(),
            "physical device",
        ),
    ] {
        if !matches {
            return Err(field);
        }
    }
    Ok(())
}

fn validate_load_observation(
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
    if load == &expected {
        Ok(())
    } else {
        Err("identity or content observation")
    }
}

fn validate_kernel_observation(
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
    .expect("exact static kernel observation is valid");
    if resolution == &expected {
        Ok(())
    } else {
        Err("object, symbol, size, or HSA alignment")
    }
}

fn validate_resource_observation(
    load: &HsaCodeObjectLoadObservationV1,
    resolution: &HsaKernelResolutionObservationV1,
    resources: ExactLdsGemmKernelResourceObservationV1,
) -> Result<(), &'static str> {
    let expected = ExactLdsGemmKernelResourceObservationV1::new(
        load.executable_object(),
        resolution.kernel_object(),
        STATIC_LDS_BYTES,
        PRIVATE_SEGMENT_BYTES,
    );
    if resources == expected {
        Ok(())
    } else {
        Err("object, static LDS, or private segment")
    }
}

fn validate_implicit_observation(
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
    if observation == &expected {
        Ok(())
    } else {
        Err("object, geometry, span, or completion")
    }
}

fn validate_dispatch_observation(
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
    .expect("adapter supplied a valid dispatch identity");
    if dispatch == &expected {
        Ok(())
    } else {
        Err("object, geometry, or completion")
    }
}

fn validate_unload_observation(
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
    if unload == &expected {
        Ok(())
    } else {
        Err("object, runtime, agent, or completion")
    }
}

fn exact_geometry() -> HsaLaunchGeometryV1 {
    HsaLaunchGeometryV1::new(GRID, WORKGROUP, DYNAMIC_LDS_BYTES)
}

fn unload_identity(
    unload: &HsaUnloadObservationV1,
    runtime_instance: [u8; 16],
    agent_handle: u64,
) -> ExactLdsGemmUnloadIdentityV1 {
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
    ExactLdsGemmUnloadIdentityV1(digest.finalize().into())
}

#[derive(Clone, Copy)]
struct JoinFactsV1<'bytes> {
    import_matches: bool,
    profile_matches: bool,
    contract_matches: bool,
    finalized_output_matches: bool,
    contract: ContractFactsV1,
    host_target: &'bytes str,
    host_profile: ExactLdsGemmProfileIdV1,
    host_profile_identity_matches: bool,
    host_grid: [u32; 3],
    host_workgroup: [u32; 3],
    host_static_lds: u32,
    host_dynamic_lds: u32,
    host_explicit_kernarg: usize,
    host_complete_kernarg: u32,
    host_kernarg_alignment: u32,
    host_length_identities: [[u8; 32]; 3],
}

impl<'bytes> JoinFactsV1<'bytes> {
    fn from_exact(
        artifact: &'bytes FinalizedExactLdsGemmHsacoV1,
        host: &'bytes GeneratedLdsGemmSlice1HostAdapterV1<'_, '_, '_>,
    ) -> Self {
        let contract = artifact.contract();
        Self {
            import_matches: artifact.import_identity() == host.compiler_import_identity_v1(),
            profile_matches: artifact.profile_identity() == host.profile_identity(),
            contract_matches: contract == host.contract_v1(),
            finalized_output_matches: artifact
                .finalized_output_identity()
                .matches(artifact.exact_finalized_bytes()),
            contract: ContractFactsV1::from_contract(contract),
            host_target: host.target(),
            host_profile: host.profile(),
            host_profile_identity_matches: host.profile_identity() == contract.identity(),
            host_grid: host.grid(),
            host_workgroup: host.workgroup(),
            host_static_lds: host.static_lds_bytes(),
            host_dynamic_lds: host.dynamic_lds_bytes(),
            host_explicit_kernarg: host.explicit_kernarg_byte_len(),
            host_complete_kernarg: host.complete_kernarg_byte_len(),
            host_kernarg_alignment: host.kernarg_alignment(),
            host_length_identities: host
                .length_identities_v1()
                .map(|identity| *identity.as_bytes()),
        }
    }
}

fn validate_join(facts: JoinFactsV1<'_>) -> Result<(), ExactLdsGemmSlice1JoinErrorV1> {
    for (matches, error) in [
        (
            facts.import_matches,
            ExactLdsGemmSlice1JoinErrorV1::ImportIdentity,
        ),
        (
            facts.profile_matches,
            ExactLdsGemmSlice1JoinErrorV1::ProfileIdentity,
        ),
        (
            facts.contract_matches,
            ExactLdsGemmSlice1JoinErrorV1::Contract,
        ),
        (
            facts.finalized_output_matches,
            ExactLdsGemmSlice1JoinErrorV1::FinalizedOutput,
        ),
    ] {
        if !matches {
            return Err(error);
        }
    }
    validate_contract(facts.contract)?;
    for (matches, field) in [
        (facts.host_target == TARGET, "target"),
        (
            facts.host_profile == ExactLdsGemmProfileIdV1::Slice1M16N16K16,
            "profile",
        ),
        (facts.host_profile_identity_matches, "profile identity"),
        (facts.host_grid == GRID, "grid"),
        (facts.host_workgroup == WORKGROUP, "workgroup"),
        (facts.host_static_lds == STATIC_LDS_BYTES, "static LDS"),
        (facts.host_dynamic_lds == DYNAMIC_LDS_BYTES, "dynamic LDS"),
        (
            facts.host_explicit_kernarg == EXPLICIT_KERNARG_BYTES,
            "explicit kernarg size",
        ),
        (
            facts.host_complete_kernarg == COMPLETE_KERNARG_BYTES as u32,
            "complete kernarg size",
        ),
        (
            facts.host_kernarg_alignment == CONTRACT_KERNARG_ALIGNMENT,
            "contract kernarg alignment",
        ),
    ] {
        if !matches {
            return Err(ExactLdsGemmSlice1JoinErrorV1::HostField(field));
        }
    }
    for (index, (host, contract)) in facts
        .host_length_identities
        .into_iter()
        .zip(facts.contract.buffers)
        .enumerate()
    {
        if host != contract.length_identity {
            return Err(ExactLdsGemmSlice1JoinErrorV1::HostLengthIdentity { index });
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct ContractFactsV1 {
    profile: ExactLdsGemmProfileIdV1,
    target: &'static str,
    code_object_version: CodeObjectVersion,
    grid: [u32; 3],
    workgroup: [u32; 3],
    wavefront_size: u32,
    explicit_kernarg_bytes: u32,
    complete_kernarg_bytes: u32,
    kernarg_alignment: u32,
    static_lds_bytes: u32,
    lds_allocations: u32,
    lds_bytes_per_allocation: u32,
    lds_alignment: u32,
    buffers: [BufferContractFactsV1; 3],
}

impl ContractFactsV1 {
    fn from_contract(contract: ExactLdsGemmContractV1) -> Self {
        Self {
            profile: contract.profile(),
            target: contract.target(),
            code_object_version: contract.code_object_version(),
            grid: contract.grid(),
            workgroup: contract.workgroup(),
            wavefront_size: contract.wavefront_size(),
            explicit_kernarg_bytes: contract.explicit_kernarg_bytes(),
            complete_kernarg_bytes: contract.complete_kernarg_bytes(),
            kernarg_alignment: contract.kernarg_alignment(),
            static_lds_bytes: contract.static_lds_bytes(),
            lds_allocations: contract.lds_allocations(),
            lds_bytes_per_allocation: contract.lds_bytes_per_allocation(),
            lds_alignment: contract.lds_alignment(),
            buffers: contract.buffers().map(BufferContractFactsV1::from_contract),
        }
    }
}

#[derive(Clone, Copy)]
struct BufferContractFactsV1 {
    role: ExactLdsGemmBufferRoleV1,
    element: ExactLdsGemmElementV1,
    elements: u64,
    bytes: u64,
    length_identity: [u8; 32],
    ownership: OwnershipSemantics,
    access: AccessMode,
    alias: AliasSemantics,
}

impl BufferContractFactsV1 {
    fn from_contract(contract: ExactLdsGemmBufferContractV1) -> Self {
        Self {
            role: contract.role(),
            element: contract.element(),
            elements: contract.elements(),
            bytes: contract.bytes(),
            length_identity: *contract.length_identity().as_bytes(),
            ownership: contract.ownership(),
            access: contract.access(),
            alias: contract.alias(),
        }
    }
}

fn validate_contract(contract: ContractFactsV1) -> Result<(), ExactLdsGemmSlice1JoinErrorV1> {
    for (matches, field) in [
        (
            contract.profile == ExactLdsGemmProfileIdV1::Slice1M16N16K16,
            "profile",
        ),
        (contract.target == TARGET, "target"),
        (
            contract.code_object_version == CodeObjectVersion::V6,
            "code-object version",
        ),
        (contract.grid == GRID, "grid"),
        (contract.workgroup == WORKGROUP, "workgroup"),
        (contract.wavefront_size == WAVEFRONT_SIZE, "wavefront"),
        (
            contract.explicit_kernarg_bytes == EXPLICIT_KERNARG_BYTES as u32,
            "explicit kernarg size",
        ),
        (
            contract.complete_kernarg_bytes == COMPLETE_KERNARG_BYTES as u32,
            "complete kernarg size",
        ),
        (
            contract.kernarg_alignment == CONTRACT_KERNARG_ALIGNMENT,
            "descriptor kernarg alignment",
        ),
        (contract.static_lds_bytes == STATIC_LDS_BYTES, "static LDS"),
        (
            contract.lds_allocations == LDS_ALLOCATIONS,
            "LDS allocation count",
        ),
        (
            contract.lds_bytes_per_allocation == LDS_BYTES_PER_ALLOCATION,
            "LDS bytes per allocation",
        ),
        (contract.lds_alignment == LDS_ALIGNMENT, "LDS alignment"),
    ] {
        if !matches {
            return Err(ExactLdsGemmSlice1JoinErrorV1::ContractField(field));
        }
    }
    for (index, (actual, expected)) in contract
        .buffers
        .into_iter()
        .zip(expected_buffer_contracts())
        .enumerate()
    {
        if !buffer_matches(actual, expected) {
            return Err(ExactLdsGemmSlice1JoinErrorV1::BufferContract { index });
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct ExpectedBufferContractV1 {
    role: ExactLdsGemmBufferRoleV1,
    element: ExactLdsGemmElementV1,
    elements: u64,
    bytes: u64,
    ownership: OwnershipSemantics,
    access: AccessMode,
    alias: AliasSemantics,
}

fn expected_buffer_contracts() -> [ExpectedBufferContractV1; 3] {
    [
        ExpectedBufferContractV1 {
            role: ExactLdsGemmBufferRoleV1::A,
            element: ExactLdsGemmElementV1::Bf16BitsU16,
            elements: 256,
            bytes: 512,
            ownership: OwnershipSemantics::SharedBorrow,
            access: AccessMode::ReadOnly,
            alias: AliasSemantics::SharedReadOnly,
        },
        ExpectedBufferContractV1 {
            role: ExactLdsGemmBufferRoleV1::B,
            element: ExactLdsGemmElementV1::Bf16BitsU16,
            elements: 256,
            bytes: 512,
            ownership: OwnershipSemantics::SharedBorrow,
            access: AccessMode::ReadOnly,
            alias: AliasSemantics::SharedReadOnly,
        },
        ExpectedBufferContractV1 {
            role: ExactLdsGemmBufferRoleV1::C,
            element: ExactLdsGemmElementV1::F32,
            elements: 256,
            bytes: 1_024,
            ownership: OwnershipSemantics::UniqueBorrow,
            access: AccessMode::ReadWrite,
            alias: AliasSemantics::Exclusive,
        },
    ]
}

fn buffer_matches(actual: BufferContractFactsV1, expected: ExpectedBufferContractV1) -> bool {
    actual.role == expected.role
        && actual.element == expected.element
        && actual.elements == expected.elements
        && actual.bytes == expected.bytes
        && actual.length_identity
            == expected_length_identity_bytes(
                expected.role,
                expected.element,
                expected.elements,
                expected.bytes,
            )
        && actual.ownership == expected.ownership
        && actual.access == expected.access
        && actual.alias == expected.alias
}

fn expected_length_identity_bytes(
    role: ExactLdsGemmBufferRoleV1,
    element: ExactLdsGemmElementV1,
    elements: u64,
    bytes: u64,
) -> [u8; 32] {
    let element_tag = match element {
        ExactLdsGemmElementV1::Bf16BitsU16 => 1,
        ExactLdsGemmElementV1::F32 => 2,
    };
    let mut digest = Sha256::new();
    for field in [
        LENGTH_IDENTITY_DOMAIN_V1,
        &[role as u8],
        &[element_tag],
        &elements.to_le_bytes(),
        &bytes.to_le_bytes(),
    ] {
        digest.update((field.len() as u64).to_le_bytes());
        digest.update(field);
    }
    digest.finalize().into()
}

#[cfg(test)]
pub mod test_support {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum TestJoinMutationV1 {
        None,
        ImportIdentity,
        ProfileIdentity,
        ContractBinding,
        FinalizedOutput,
        Profile,
        Target,
        CodeObjectVersion,
        Grid,
        Workgroup,
        Wavefront,
        ExplicitKernarg,
        CompleteKernarg,
        ContractKernargAlignment,
        StaticLds,
        LdsAllocations,
        LdsBytesPerAllocation,
        LdsAlignment,
        BufferRole,
        BufferElement,
        BufferElements,
        BufferBytes,
        BufferLengthIdentity,
        BufferOwnership,
        BufferAccess,
        BufferAlias,
        HostTarget,
        HostProfile,
        HostProfileIdentity,
        HostGrid,
        HostWorkgroup,
        HostStaticLds,
        HostDynamicLds,
        HostExplicitKernarg,
        HostCompleteKernarg,
        HostContractKernargAlignment,
        HostLengthIdentity,
    }

    pub fn validate_join_mutation_v1(
        mutation: TestJoinMutationV1,
    ) -> Result<(), ExactLdsGemmSlice1JoinErrorV1> {
        let mut facts = canonical_join_facts();
        match mutation {
            TestJoinMutationV1::None => {}
            TestJoinMutationV1::ImportIdentity => facts.import_matches = false,
            TestJoinMutationV1::ProfileIdentity => facts.profile_matches = false,
            TestJoinMutationV1::ContractBinding => facts.contract_matches = false,
            TestJoinMutationV1::FinalizedOutput => facts.finalized_output_matches = false,
            TestJoinMutationV1::Profile => {
                facts.contract.profile = ExactLdsGemmProfileIdV1::KPhaseM16N16K32;
            }
            TestJoinMutationV1::Target => facts.contract.target = "gfx942:xnack+",
            TestJoinMutationV1::CodeObjectVersion => {
                facts.contract.code_object_version = CodeObjectVersion::V5;
            }
            TestJoinMutationV1::Grid => facts.contract.grid[0] = 2,
            TestJoinMutationV1::Workgroup => facts.contract.workgroup[0] = 256,
            TestJoinMutationV1::Wavefront => facts.contract.wavefront_size = 32,
            TestJoinMutationV1::ExplicitKernarg => facts.contract.explicit_kernarg_bytes = 40,
            TestJoinMutationV1::CompleteKernarg => facts.contract.complete_kernarg_bytes = 296,
            TestJoinMutationV1::ContractKernargAlignment => {
                facts.contract.kernarg_alignment = HSA_KERNARG_ALIGNMENT as u32;
            }
            TestJoinMutationV1::StaticLds => facts.contract.static_lds_bytes = 512,
            TestJoinMutationV1::LdsAllocations => facts.contract.lds_allocations = 1,
            TestJoinMutationV1::LdsBytesPerAllocation => {
                facts.contract.lds_bytes_per_allocation = 256;
            }
            TestJoinMutationV1::LdsAlignment => facts.contract.lds_alignment = 8,
            TestJoinMutationV1::BufferRole => {
                facts.contract.buffers[0].role = ExactLdsGemmBufferRoleV1::B;
            }
            TestJoinMutationV1::BufferElement => {
                facts.contract.buffers[0].element = ExactLdsGemmElementV1::F32;
            }
            TestJoinMutationV1::BufferElements => facts.contract.buffers[0].elements = 255,
            TestJoinMutationV1::BufferBytes => facts.contract.buffers[0].bytes = 510,
            TestJoinMutationV1::BufferLengthIdentity => {
                facts.contract.buffers[0].length_identity[0] ^= 1;
            }
            TestJoinMutationV1::BufferOwnership => {
                facts.contract.buffers[0].ownership = OwnershipSemantics::UniqueBorrow;
            }
            TestJoinMutationV1::BufferAccess => {
                facts.contract.buffers[0].access = AccessMode::ReadWrite;
            }
            TestJoinMutationV1::BufferAlias => {
                facts.contract.buffers[0].alias = AliasSemantics::Exclusive;
            }
            TestJoinMutationV1::HostTarget => facts.host_target = "gfx942:xnack+",
            TestJoinMutationV1::HostProfile => {
                facts.host_profile = ExactLdsGemmProfileIdV1::KPhaseM16N16K32;
            }
            TestJoinMutationV1::HostProfileIdentity => {
                facts.host_profile_identity_matches = false;
            }
            TestJoinMutationV1::HostGrid => facts.host_grid[0] = 2,
            TestJoinMutationV1::HostWorkgroup => facts.host_workgroup[0] = 256,
            TestJoinMutationV1::HostStaticLds => facts.host_static_lds = 512,
            TestJoinMutationV1::HostDynamicLds => facts.host_dynamic_lds = 1,
            TestJoinMutationV1::HostExplicitKernarg => facts.host_explicit_kernarg = 40,
            TestJoinMutationV1::HostCompleteKernarg => facts.host_complete_kernarg = 296,
            TestJoinMutationV1::HostContractKernargAlignment => {
                facts.host_kernarg_alignment = HSA_KERNARG_ALIGNMENT as u32;
            }
            TestJoinMutationV1::HostLengthIdentity => {
                facts.host_length_identities[0][0] ^= 1;
            }
        }
        validate_join(facts)
    }

    fn canonical_join_facts() -> JoinFactsV1<'static> {
        let expected = expected_buffer_contracts();
        let buffers = expected.map(|value| BufferContractFactsV1 {
            role: value.role,
            element: value.element,
            elements: value.elements,
            bytes: value.bytes,
            length_identity: expected_length_identity_bytes(
                value.role,
                value.element,
                value.elements,
                value.bytes,
            ),
            ownership: value.ownership,
            access: value.access,
            alias: value.alias,
        });
        let contract = ContractFactsV1 {
            profile: ExactLdsGemmProfileIdV1::Slice1M16N16K16,
            target: TARGET,
            code_object_version: CodeObjectVersion::V6,
            grid: GRID,
            workgroup: WORKGROUP,
            wavefront_size: WAVEFRONT_SIZE,
            explicit_kernarg_bytes: EXPLICIT_KERNARG_BYTES as u32,
            complete_kernarg_bytes: COMPLETE_KERNARG_BYTES as u32,
            kernarg_alignment: CONTRACT_KERNARG_ALIGNMENT,
            static_lds_bytes: STATIC_LDS_BYTES,
            lds_allocations: LDS_ALLOCATIONS,
            lds_bytes_per_allocation: LDS_BYTES_PER_ALLOCATION,
            lds_alignment: LDS_ALIGNMENT,
            buffers,
        };
        JoinFactsV1 {
            import_matches: true,
            profile_matches: true,
            contract_matches: true,
            finalized_output_matches: true,
            contract,
            host_target: TARGET,
            host_profile: ExactLdsGemmProfileIdV1::Slice1M16N16K16,
            host_profile_identity_matches: true,
            host_grid: GRID,
            host_workgroup: WORKGROUP,
            host_static_lds: STATIC_LDS_BYTES,
            host_dynamic_lds: DYNAMIC_LDS_BYTES,
            host_explicit_kernarg: EXPLICIT_KERNARG_BYTES,
            host_complete_kernarg: COMPLETE_KERNARG_BYTES as u32,
            host_kernarg_alignment: CONTRACT_KERNARG_ALIGNMENT,
            host_length_identities: buffers.map(|buffer| buffer.length_identity),
        }
    }

    struct TestRetainedV1 {
        bytes: Vec<u8>,
        explicit: [u8; EXPLICIT_KERNARG_BYTES],
        drops: Arc<AtomicUsize>,
    }

    impl Drop for TestRetainedV1 {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::SeqCst);
        }
    }

    impl RetainedExactLdsGemmSlice1V1 for TestRetainedV1 {
        fn target_v1(&self) -> &str {
            TARGET
        }

        fn ordinal_v1(&self) -> i32 {
            0
        }

        fn finalized_bytes_v1(&self) -> &[u8] {
            &self.bytes
        }

        fn explicit_kernarg_v1(&self) -> &[u8; EXPLICIT_KERNARG_BYTES] {
            &self.explicit
        }
    }

    pub struct TestLoadedExactLdsGemmV1<A: ReviewedExactLdsGemmRuntimeAdapterV1> {
        state: LoadedExactStateV1<TestRetainedV1, A>,
    }

    impl<A: ReviewedExactLdsGemmRuntimeAdapterV1> TestLoadedExactLdsGemmV1<A> {
        pub fn dispatch_and_wait(
            self,
        ) -> Result<TestCompletedExactLdsGemmV1<A>, ExactLdsGemmSlice1DispatchErrorV1<A::Error>>
        {
            self.state
                .dispatch_and_wait()
                .map(|state| TestCompletedExactLdsGemmV1 {
                    state: state.release_retained(),
                })
        }
    }

    pub struct TestCompletedExactLdsGemmV1<A: ReviewedExactLdsGemmRuntimeAdapterV1> {
        state: CompletedExactStateV1<A>,
    }

    impl<A: ReviewedExactLdsGemmRuntimeAdapterV1> TestCompletedExactLdsGemmV1<A> {
        pub fn unload(mut self) -> TestUnloadedExactLdsGemmV1 {
            let mut adapter = self
                .state
                .adapter
                .take()
                .expect("test completion retains its adapter");
            let executable = self
                .state
                .executable
                .take()
                .expect("test completion retains its executable");
            let unload = terminal_unload(
                &mut adapter,
                executable,
                &self.state.environment,
                &self.state.load,
            );
            let receipt = TestUnloadedExactLdsGemmV1 {
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

    #[derive(Debug)]
    pub struct TestUnloadedExactLdsGemmV1 {
        pub executable_object: HsaExecutableObjectIdentityV1,
        pub kernel_object: HsaKernelObjectIdentityV1,
        pub dispatch_identity: [u8; 16],
        pub unload_identity: ExactLdsGemmUnloadIdentityV1,
    }

    pub fn load_test_lifecycle_v1<A: ReviewedExactLdsGemmRuntimeAdapterV1>(
        adapter: A,
        context_matches: bool,
        drops: Arc<AtomicUsize>,
    ) -> Result<TestLoadedExactLdsGemmV1<A>, ExactLdsGemmSlice1LoadErrorV1<A::Error>> {
        let retained = TestRetainedV1 {
            bytes: vec![0x5a; 96],
            explicit: std::array::from_fn(|index| index as u8),
            drops,
        };
        if !context_matches {
            return Err(ExactLdsGemmSlice1LoadErrorV1::ContextIdentity);
        }
        load_after_context_match(retained, adapter).map(|state| TestLoadedExactLdsGemmV1 { state })
    }

    pub const fn exact_test_explicit_bytes_v1() -> usize {
        EXPLICIT_KERNARG_BYTES
    }

    pub const fn exact_test_implicit_bytes_v1() -> usize {
        IMPLICIT_KERNARG_BYTES
    }

    pub const fn exact_test_complete_bytes_v1() -> usize {
        COMPLETE_KERNARG_BYTES
    }

    pub const fn exact_test_hsa_alignment_v1() -> u64 {
        HSA_KERNARG_ALIGNMENT
    }

    pub const fn exact_test_static_lds_v1() -> u32 {
        STATIC_LDS_BYTES
    }
}
