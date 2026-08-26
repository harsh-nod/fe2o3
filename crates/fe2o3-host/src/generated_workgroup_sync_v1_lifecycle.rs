//! Protected one-shot HSA lifecycle for the two exact workgroup-sync profiles.

use crate::{
    HsaCodeObjectLoadObservationV1, HsaDispatchObservationV1, HsaEnvironmentObservationV1,
    HsaExecutableObjectIdentityV1, HsaKernelObjectIdentityV1, HsaKernelResolutionObservationV1,
    HsaLaunchGeometryV1, HsaUnloadObservationV1, ReviewedHsaExecutableLifecycleAdapterV1,
    generated_workgroup_lds_reduction_v1::{
        COMPLETE_KERNARG_BYTES as LDS_COMPLETE_KERNARG_BYTES, DESCRIPTOR_KERNARG_ALIGNMENT,
        DYNAMIC_LDS_BYTES, EXPLICIT_KERNARG_BYTES as LDS_EXPLICIT_KERNARG_BYTES,
        EXPORT_SYMBOL as LDS_EXPORT_SYMBOL, GRID, GeneratedWorkgroupLdsReductionV1HostAdapterV1,
        HIDDEN_DYNAMIC_LDS_OFFSET, HIDDEN_DYNAMIC_LDS_VALUE, PRIVATE_SEGMENT_BYTES,
        RUNTIME_KERNARG_ALIGNMENT, STATIC_GROUP_SEGMENT_BYTES, TARGET, WAVEFRONT_SIZE, WORKGROUP,
    },
    generated_workgroup_scoped_atomic_v1::{
        COMPLETE_KERNARG_BYTES as ATOMIC_COMPLETE_KERNARG_BYTES,
        DYNAMIC_LDS_BYTES as ATOMIC_DYNAMIC_LDS_BYTES,
        EXPLICIT_KERNARG_BYTES as ATOMIC_EXPLICIT_KERNARG_BYTES,
        EXPORT_SYMBOL as ATOMIC_EXPORT_SYMBOL, GeneratedWorkgroupScopedAtomicV1HostAdapterV1,
    },
};
use fe2o3_amd_target::AmdTargetId;
use fe2o3_artifacts::{DigestAlgorithm, PayloadDigest};
use fe2o3_core::ContextIdentity;
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, FinalizedWorkerV2HsacoIdentityV1, FinalizedWorkgroupSyncHsacoIdentityV1,
    PreparedFinalizedWorkgroupSyncHsacoV1, WorkgroupSyncProfileKindV1,
    WorkgroupSyncWorkerExchangeIdentityV1,
};
use fe2o3_kernel_descriptor::CodeObjectVersion;
use sha2::{Digest, Sha256};
use std::{error::Error, fmt, marker::PhantomData};

const IMPLICIT_KERNARG_BYTES: usize = 256;
const MAX_COMPLETE_KERNARG_BYTES: usize = ATOMIC_COMPLETE_KERNARG_BYTES;
const UNLOAD_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/WORKGROUP-SYNC/UNLOAD/V1\0";

/// Exact finalizer and compiler-profile identities retained by the lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkgroupSyncLifecycleIdentityV1 {
    profile: WorkgroupSyncProfileKindV1,
    finalizer: FinalizedWorkgroupSyncHsacoIdentityV1,
    structural_finalizer: FinalizedWorkerV2HsacoIdentityV1,
    worker_exchange: WorkgroupSyncWorkerExchangeIdentityV1,
    compiler_module: ContentIdentityV1,
    compiler_source_authority: [u8; 32],
    compiler_source: [u8; 32],
    kernel_ir: [u8; 32],
    descriptor_profile: [u8; 32],
    linked_output: ContentIdentityV1,
    finalized_output: ContentIdentityV1,
}

impl WorkgroupSyncLifecycleIdentityV1 {
    pub const fn profile(self) -> WorkgroupSyncProfileKindV1 {
        self.profile
    }
    pub const fn finalizer(self) -> FinalizedWorkgroupSyncHsacoIdentityV1 {
        self.finalizer
    }
    pub const fn structural_finalizer(self) -> FinalizedWorkerV2HsacoIdentityV1 {
        self.structural_finalizer
    }
    pub const fn worker_exchange(self) -> WorkgroupSyncWorkerExchangeIdentityV1 {
        self.worker_exchange
    }
    pub const fn compiler_module(self) -> ContentIdentityV1 {
        self.compiler_module
    }
    pub const fn compiler_source_authority(self) -> [u8; 32] {
        self.compiler_source_authority
    }
    pub const fn compiler_source(self) -> [u8; 32] {
        self.compiler_source
    }
    pub const fn kernel_ir(self) -> [u8; 32] {
        self.kernel_ir
    }
    pub const fn descriptor_profile(self) -> [u8; 32] {
        self.descriptor_profile
    }
    pub const fn linked_output(self) -> ContentIdentityV1 {
        self.linked_output
    }
    pub const fn finalized_output(self) -> ContentIdentityV1 {
        self.finalized_output
    }

    fn from_admission(admission: &PreparedFinalizedWorkgroupSyncHsacoV1) -> Self {
        let exchange = admission.exchange();
        let compiler = exchange.compiler_pins();
        Self {
            profile: admission.profile(),
            finalizer: admission.identity(),
            structural_finalizer: admission.structural_finalization_identity(),
            worker_exchange: exchange.identity(),
            compiler_module: exchange.compiler_module_identity(),
            compiler_source_authority: *compiler.source_authority(),
            compiler_source: *compiler.source_sha256(),
            kernel_ir: compiler.kernel_ir_identity(),
            descriptor_profile: compiler.descriptor_profile_identity(),
            linked_output: exchange.linked_output_identity(),
            finalized_output: admission.finalized_output_identity(),
        }
    }
}

/// Runtime-reported exact resource facts, without native handles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkgroupSyncKernelResourceObservationV1 {
    profile: WorkgroupSyncProfileKindV1,
    executable: HsaExecutableObjectIdentityV1,
    kernel: HsaKernelObjectIdentityV1,
    static_group_segment_bytes: u32,
    private_segment_bytes: u32,
}

impl WorkgroupSyncKernelResourceObservationV1 {
    pub const fn new(
        profile: WorkgroupSyncProfileKindV1,
        executable: HsaExecutableObjectIdentityV1,
        kernel: HsaKernelObjectIdentityV1,
        static_group_segment_bytes: u32,
        private_segment_bytes: u32,
    ) -> Self {
        Self {
            profile,
            executable,
            kernel,
            static_group_segment_bytes,
            private_segment_bytes,
        }
    }
    pub const fn profile(self) -> WorkgroupSyncProfileKindV1 {
        self.profile
    }
    pub const fn executable(self) -> HsaExecutableObjectIdentityV1 {
        self.executable
    }
    pub const fn kernel(self) -> HsaKernelObjectIdentityV1 {
        self.kernel
    }
    pub const fn static_group_segment_bytes(self) -> u32 {
        self.static_group_segment_bytes
    }
    pub const fn private_segment_bytes(self) -> u32 {
        self.private_segment_bytes
    }
}

/// Runtime observation of the exact COV6 hidden span and AQL LDS field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkgroupSyncImplicitKernargObservationV1 {
    profile: WorkgroupSyncProfileKindV1,
    executable: HsaExecutableObjectIdentityV1,
    kernel: HsaKernelObjectIdentityV1,
    geometry: HsaLaunchGeometryV1,
    explicit_byte_len: u64,
    implicit_byte_offset: u64,
    implicit_byte_len: u64,
    hidden_dynamic_lds_offset: Option<u64>,
    hidden_dynamic_lds_value: u32,
    aql_group_segment_bytes: u32,
    initialized: bool,
}

impl WorkgroupSyncImplicitKernargObservationV1 {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        profile: WorkgroupSyncProfileKindV1,
        executable: HsaExecutableObjectIdentityV1,
        kernel: HsaKernelObjectIdentityV1,
        geometry: HsaLaunchGeometryV1,
        explicit_byte_len: u64,
        implicit_byte_offset: u64,
        implicit_byte_len: u64,
        hidden_dynamic_lds_offset: Option<u64>,
        hidden_dynamic_lds_value: u32,
        aql_group_segment_bytes: u32,
        initialized: bool,
    ) -> Self {
        Self {
            profile,
            executable,
            kernel,
            geometry,
            explicit_byte_len,
            implicit_byte_offset,
            implicit_byte_len,
            hidden_dynamic_lds_offset,
            hidden_dynamic_lds_value,
            aql_group_segment_bytes,
            initialized,
        }
    }
    pub const fn profile(self) -> WorkgroupSyncProfileKindV1 {
        self.profile
    }
    pub const fn executable(self) -> HsaExecutableObjectIdentityV1 {
        self.executable
    }
    pub const fn kernel(self) -> HsaKernelObjectIdentityV1 {
        self.kernel
    }
    pub const fn geometry(self) -> HsaLaunchGeometryV1 {
        self.geometry
    }
    pub const fn explicit_byte_len(self) -> u64 {
        self.explicit_byte_len
    }
    pub const fn implicit_byte_offset(self) -> u64 {
        self.implicit_byte_offset
    }
    pub const fn implicit_byte_len(self) -> u64 {
        self.implicit_byte_len
    }
    pub const fn hidden_dynamic_lds_offset(self) -> Option<u64> {
        self.hidden_dynamic_lds_offset
    }
    pub const fn hidden_dynamic_lds_value(self) -> u32 {
        self.hidden_dynamic_lds_value
    }
    pub const fn aql_group_segment_bytes(self) -> u32 {
        self.aql_group_segment_bytes
    }
    pub const fn initialized(self) -> bool {
        self.initialized
    }
}

/// Reviewed exact-profile extension over the private HSA lifecycle.
///
/// # Safety
///
/// Every observation must derive from the exact supplied private objects and
/// retained context. The initializer must preserve explicit bytes, initialize
/// exactly one COV6 hidden span, bind the exact profile's dynamic LDS value to
/// the pending AQL packet, and may not unwind.
pub unsafe trait ReviewedWorkgroupSyncRuntimeAdapterV1:
    ReviewedHsaExecutableLifecycleAdapterV1
{
    /// Returns the identity of the exact retained host context.
    ///
    /// # Safety
    ///
    /// The identity must derive from the context retained by this adapter and
    /// the method must not unwind.
    unsafe fn context_identity_v1(&mut self) -> ContextIdentity;

    /// Initializes and observes the exact profile-specific COV6 hidden span.
    ///
    /// # Safety
    ///
    /// All objects and bytes must be the exact values supplied by the caller.
    /// The explicit prefix must remain unchanged, the pending AQL binding must
    /// use the reported group segment value, and the method must not unwind.
    #[allow(clippy::too_many_arguments)]
    unsafe fn initialize_workgroup_sync_implicit_kernarg_v1(
        &mut self,
        profile: WorkgroupSyncProfileKindV1,
        executable: &Self::Executable,
        kernel: &Self::Kernel,
        geometry: HsaLaunchGeometryV1,
        explicit_byte_len: usize,
        implicit_byte_offset: usize,
        implicit_byte_len: usize,
        kernarg: &mut [u8],
    ) -> Result<WorkgroupSyncImplicitKernargObservationV1, Self::Error>;

    /// Observes resources for the supplied private executable/kernel pair.
    ///
    /// # Safety
    ///
    /// The observation must derive only from these exact objects, expose no
    /// native handles, and the method must not unwind.
    unsafe fn observe_workgroup_sync_resources_v1(
        &mut self,
        profile: WorkgroupSyncProfileKindV1,
        executable: &Self::Executable,
        kernel: &Self::Kernel,
    ) -> Result<WorkgroupSyncKernelResourceObservationV1, Self::Error>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkgroupSyncJoinErrorV1 {
    FinalizedOutput,
    WrongProfile,
    ProfileField(&'static str),
    HostField(&'static str),
}

impl fmt::Display for WorkgroupSyncJoinErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FinalizedOutput => {
                formatter.write_str("finalized workgroup-sync output identity mismatch")
            }
            Self::WrongProfile => {
                formatter.write_str("finalizer receipt belongs to the other workgroup-sync profile")
            }
            Self::ProfileField(field) => {
                write!(formatter, "finalized workgroup-sync {field} drifted")
            }
            Self::HostField(field) => {
                write!(formatter, "generated workgroup-sync host {field} drifted")
            }
        }
    }
}
impl Error for WorkgroupSyncJoinErrorV1 {}

#[derive(Debug)]
#[non_exhaustive]
pub enum WorkgroupSyncLoadErrorV1<E> {
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

impl<E: fmt::Display> fmt::Display for WorkgroupSyncLoadErrorV1<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContextIdentity => formatter.write_str("exact context identity mismatch"),
            Self::EnvironmentAdapter(error) => write!(formatter, "HSA environment failed: {error}"),
            Self::Environment(field) => write!(formatter, "HSA environment {field} drifted"),
            Self::LoadAdapter(error) => write!(formatter, "HSA executable load failed: {error}"),
            Self::LoadObservation(field) => write!(formatter, "HSA load {field} drifted"),
            Self::KernelAdapter(error) => {
                write!(formatter, "HSA kernel resolution failed: {error}")
            }
            Self::KernelObservation(field) => write!(formatter, "HSA kernel {field} drifted"),
            Self::ResourceAdapter(error) => write!(formatter, "HSA resource query failed: {error}"),
            Self::ResourceObservation(field) => write!(formatter, "HSA resources {field} drifted"),
        }
    }
}
impl<E: Error + 'static> Error for WorkgroupSyncLoadErrorV1<E> {}

#[derive(Debug)]
#[non_exhaustive]
pub enum WorkgroupSyncDispatchErrorV1<E> {
    ImplicitAdapter(E),
    ImplicitObservation(&'static str),
    ExplicitKernargMutation,
    HiddenDynamicLdsMutation,
    DispatchAdapter(E),
    DispatchObservation(&'static str),
}

impl<E: fmt::Display> fmt::Display for WorkgroupSyncDispatchErrorV1<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ImplicitAdapter(error) => {
                write!(formatter, "COV6 implicit kernarg failed: {error}")
            }
            Self::ImplicitObservation(field) => {
                write!(formatter, "COV6 implicit kernarg {field} drifted")
            }
            Self::ExplicitKernargMutation => {
                formatter.write_str("implicit initialization changed explicit bytes")
            }
            Self::HiddenDynamicLdsMutation => {
                formatter.write_str("hidden dynamic-LDS argument differs from the exact profile")
            }
            Self::DispatchAdapter(error) => write!(formatter, "HSA dispatch failed: {error}"),
            Self::DispatchObservation(field) => write!(formatter, "HSA dispatch {field} drifted"),
        }
    }
}
impl<E: Error + 'static> Error for WorkgroupSyncDispatchErrorV1<E> {}

#[doc(hidden)]
pub struct WorkgroupLdsReductionProfileV1;
#[doc(hidden)]
pub struct WorkgroupScopedAtomicProfileV1;

#[doc(hidden)]
pub trait ExactWorkgroupSyncHostProfileV1 {
    const KIND: WorkgroupSyncProfileKindV1;
    const EXPORT_SYMBOL: &'static str;
    const EXPLICIT_KERNARG_BYTES: usize;
    const COMPLETE_KERNARG_BYTES: usize;
    const DYNAMIC_LDS_BYTES: u32;
    const HIDDEN_DYNAMIC_LDS_OFFSET: Option<usize>;
    const HIDDEN_DYNAMIC_LDS_VALUE: u32;
}

impl ExactWorkgroupSyncHostProfileV1 for WorkgroupLdsReductionProfileV1 {
    const KIND: WorkgroupSyncProfileKindV1 = WorkgroupSyncProfileKindV1::LdsReduction;
    const EXPORT_SYMBOL: &'static str = LDS_EXPORT_SYMBOL;
    const EXPLICIT_KERNARG_BYTES: usize = LDS_EXPLICIT_KERNARG_BYTES;
    const COMPLETE_KERNARG_BYTES: usize = LDS_COMPLETE_KERNARG_BYTES;
    const DYNAMIC_LDS_BYTES: u32 = DYNAMIC_LDS_BYTES;
    const HIDDEN_DYNAMIC_LDS_OFFSET: Option<usize> = Some(HIDDEN_DYNAMIC_LDS_OFFSET);
    const HIDDEN_DYNAMIC_LDS_VALUE: u32 = HIDDEN_DYNAMIC_LDS_VALUE;
}

impl ExactWorkgroupSyncHostProfileV1 for WorkgroupScopedAtomicProfileV1 {
    const KIND: WorkgroupSyncProfileKindV1 = WorkgroupSyncProfileKindV1::ScopedAtomic;
    const EXPORT_SYMBOL: &'static str = ATOMIC_EXPORT_SYMBOL;
    const EXPLICIT_KERNARG_BYTES: usize = ATOMIC_EXPLICIT_KERNARG_BYTES;
    const COMPLETE_KERNARG_BYTES: usize = ATOMIC_COMPLETE_KERNARG_BYTES;
    const DYNAMIC_LDS_BYTES: u32 = ATOMIC_DYNAMIC_LDS_BYTES;
    const HIDDEN_DYNAMIC_LDS_OFFSET: Option<usize> = None;
    const HIDDEN_DYNAMIC_LDS_VALUE: u32 = 0;
}

enum ExactHostArgumentsV1<'first, 'second, 'third> {
    Lds(GeneratedWorkgroupLdsReductionV1HostAdapterV1<'first, 'second>),
    Atomic(GeneratedWorkgroupScopedAtomicV1HostAdapterV1<'first, 'second, 'third>),
}

impl ExactHostArgumentsV1<'_, '_, '_> {
    fn observed_context(&self) -> &crate::ObservedContext {
        match self {
            Self::Lds(host) => host.observed_context_v1(),
            Self::Atomic(host) => host.observed_context_v1(),
        }
    }
    fn explicit_kernarg(&self) -> &[u8] {
        match self {
            Self::Lds(host) => host.explicit_kernarg_bytes_v1(),
            Self::Atomic(host) => host.explicit_kernarg_bytes_v1(),
        }
    }
}

/// Private-field linear join used only through the two exact public aliases.
#[doc(hidden)]
#[must_use = "the joined workgroup-sync request must be loaded or dropped"]
pub struct JoinedWorkgroupSyncV1<'first, 'second, 'third, P> {
    admission: PreparedFinalizedWorkgroupSyncHsacoV1,
    host: ExactHostArgumentsV1<'first, 'second, 'third>,
    lineage: WorkgroupSyncLifecycleIdentityV1,
    _profile: PhantomData<P>,
}

pub type JoinedWorkgroupLdsReductionV1<'values, 'output> =
    JoinedWorkgroupSyncV1<'values, 'output, 'static, WorkgroupLdsReductionProfileV1>;
pub type JoinedWorkgroupScopedAtomicV1<'values, 'eligible, 'target> =
    JoinedWorkgroupSyncV1<'values, 'eligible, 'target, WorkgroupScopedAtomicProfileV1>;

impl<P> fmt::Debug for JoinedWorkgroupSyncV1<'_, '_, '_, P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JoinedWorkgroupSyncV1")
            .field("lineage", &self.lineage)
            .finish_non_exhaustive()
    }
}

pub fn join_workgroup_lds_reduction_v1<'values, 'output>(
    admission: PreparedFinalizedWorkgroupSyncHsacoV1,
    host: GeneratedWorkgroupLdsReductionV1HostAdapterV1<'values, 'output>,
) -> Result<JoinedWorkgroupLdsReductionV1<'values, 'output>, WorkgroupSyncJoinErrorV1> {
    let host = ExactHostArgumentsV1::Lds(host);
    validate_join_borrowed::<WorkgroupLdsReductionProfileV1>(&admission, &host)?;
    let lineage = WorkgroupSyncLifecycleIdentityV1::from_admission(&admission);
    Ok(JoinedWorkgroupSyncV1 {
        admission,
        host,
        lineage,
        _profile: PhantomData,
    })
}

pub fn join_workgroup_scoped_atomic_v1<'values, 'eligible, 'target>(
    admission: PreparedFinalizedWorkgroupSyncHsacoV1,
    host: GeneratedWorkgroupScopedAtomicV1HostAdapterV1<'values, 'eligible, 'target>,
) -> Result<JoinedWorkgroupScopedAtomicV1<'values, 'eligible, 'target>, WorkgroupSyncJoinErrorV1> {
    let host = ExactHostArgumentsV1::Atomic(host);
    validate_join_borrowed::<WorkgroupScopedAtomicProfileV1>(&admission, &host)?;
    let lineage = WorkgroupSyncLifecycleIdentityV1::from_admission(&admission);
    Ok(JoinedWorkgroupSyncV1 {
        admission,
        host,
        lineage,
        _profile: PhantomData,
    })
}

impl<P: ExactWorkgroupSyncHostProfileV1> JoinedWorkgroupSyncV1<'_, '_, '_, P> {
    pub const fn lineage(&self) -> WorkgroupSyncLifecycleIdentityV1 {
        self.lineage
    }

    pub fn load<A: ReviewedWorkgroupSyncRuntimeAdapterV1>(
        self,
        mut adapter: A,
    ) -> Result<LoadedWorkgroupSyncV1<Self, P, A>, WorkgroupSyncLoadErrorV1<A::Error>> {
        let context = reviewed_call(|| unsafe { adapter.context_identity_v1() });
        if !self
            .host
            .observed_context()
            .matches_core_context_identity_v1(context)
        {
            return Err(WorkgroupSyncLoadErrorV1::ContextIdentity);
        }
        let lineage = self.lineage;
        Ok(LoadedWorkgroupSyncV1 {
            state: load_after_context_match::<_, P, A>(self, adapter)?,
            lineage,
            _profile: PhantomData,
        })
    }
}

pub type LoadedWorkgroupLdsReductionV1<'values, 'output, A> = LoadedWorkgroupSyncV1<
    JoinedWorkgroupLdsReductionV1<'values, 'output>,
    WorkgroupLdsReductionProfileV1,
    A,
>;
pub type LoadedWorkgroupScopedAtomicV1<'values, 'eligible, 'target, A> = LoadedWorkgroupSyncV1<
    JoinedWorkgroupScopedAtomicV1<'values, 'eligible, 'target>,
    WorkgroupScopedAtomicProfileV1,
    A,
>;

trait RetainedWorkgroupSyncV1 {
    fn target_v1(&self) -> &str;
    fn ordinal_v1(&self) -> i32;
    fn finalized_bytes_v1(&self) -> &[u8];
    fn finalized_identity_v1(&self) -> ContentIdentityV1;
    fn explicit_kernarg_v1(&self) -> &[u8];
}

impl<P> RetainedWorkgroupSyncV1 for JoinedWorkgroupSyncV1<'_, '_, '_, P> {
    fn target_v1(&self) -> &str {
        TARGET
    }
    fn ordinal_v1(&self) -> i32 {
        self.host.observed_context().device().ordinal()
    }
    fn finalized_bytes_v1(&self) -> &[u8] {
        // SAFETY: the exact one-shot state retains the receipt and never
        // exposes or copies this borrow outside the reviewed load call.
        unsafe {
            self.admission
                .exact_finalized_bytes_for_reviewed_workgroup_sync_runtime_v1()
        }
    }
    fn finalized_identity_v1(&self) -> ContentIdentityV1 {
        self.lineage.finalized_output
    }
    fn explicit_kernarg_v1(&self) -> &[u8] {
        self.host.explicit_kernarg()
    }
}

#[repr(C, align(16))]
struct CompleteKernargV1 {
    bytes: [u8; MAX_COMPLETE_KERNARG_BYTES],
}

impl CompleteKernargV1 {
    fn from_explicit(explicit: &[u8], complete_bytes: usize) -> Self {
        assert_eq!(complete_bytes - explicit.len(), IMPLICIT_KERNARG_BYTES);
        assert!(complete_bytes <= MAX_COMPLETE_KERNARG_BYTES);
        let mut value = Self {
            bytes: [0; MAX_COMPLETE_KERNARG_BYTES],
        };
        value.bytes[..explicit.len()].copy_from_slice(explicit);
        value
    }
}

struct LoadedStateV1<R, A: ReviewedWorkgroupSyncRuntimeAdapterV1> {
    retained: Option<R>,
    adapter: Option<A>,
    environment: HsaEnvironmentObservationV1,
    load: HsaCodeObjectLoadObservationV1,
    resolution: HsaKernelResolutionObservationV1,
    resources: WorkgroupSyncKernelResourceObservationV1,
    executable: Option<A::Executable>,
    kernel: Option<A::Kernel>,
    kernarg: CompleteKernargV1,
}

impl<R, A: ReviewedWorkgroupSyncRuntimeAdapterV1> Drop for LoadedStateV1<R, A> {
    fn drop(&mut self) {
        self.kernel.take();
        if let Some(executable) = self.executable.take() {
            terminal_unload(
                self.adapter
                    .as_mut()
                    .unwrap_or_else(|| std::process::abort()),
                executable,
                &self.environment,
                &self.load,
            );
        }
    }
}

#[doc(hidden)]
#[must_use = "the loaded workgroup-sync request must dispatch or terminally unload"]
pub struct LoadedWorkgroupSyncV1<R, P, A: ReviewedWorkgroupSyncRuntimeAdapterV1> {
    state: LoadedStateV1<R, A>,
    lineage: WorkgroupSyncLifecycleIdentityV1,
    _profile: PhantomData<P>,
}

impl<R, P, A: ReviewedWorkgroupSyncRuntimeAdapterV1> fmt::Debug for LoadedWorkgroupSyncV1<R, P, A> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LoadedWorkgroupSyncV1")
            .field("load", &self.state.load)
            .field("resolution", &self.state.resolution)
            .field("resources", &self.state.resources)
            .finish_non_exhaustive()
    }
}

#[allow(private_bounds)]
impl<
    R: RetainedWorkgroupSyncV1,
    P: ExactWorkgroupSyncHostProfileV1,
    A: ReviewedWorkgroupSyncRuntimeAdapterV1,
> LoadedWorkgroupSyncV1<R, P, A>
{
    pub fn dispatch_and_wait(
        self,
    ) -> Result<CompletedWorkgroupSyncV1<P, A>, WorkgroupSyncDispatchErrorV1<A::Error>> {
        let LoadedWorkgroupSyncV1 { state, lineage, .. } = self;
        let quiescent = dispatch_and_wait::<R, P, A>(state)?;
        let state = quiescent.release_retained();
        Ok(CompletedWorkgroupSyncV1 {
            state,
            lineage,
            _profile: PhantomData,
        })
    }
}

struct QuiescentStateV1<R, A: ReviewedWorkgroupSyncRuntimeAdapterV1> {
    retained: Option<R>,
    adapter: Option<A>,
    environment: HsaEnvironmentObservationV1,
    load: HsaCodeObjectLoadObservationV1,
    resolution: HsaKernelResolutionObservationV1,
    executable: Option<A::Executable>,
    dispatch_identity: [u8; 16],
}

impl<R, A: ReviewedWorkgroupSyncRuntimeAdapterV1> Drop for QuiescentStateV1<R, A> {
    fn drop(&mut self) {
        if let Some(executable) = self.executable.take() {
            terminal_unload(
                self.adapter
                    .as_mut()
                    .unwrap_or_else(|| std::process::abort()),
                executable,
                &self.environment,
                &self.load,
            );
        }
    }
}

impl<R, A: ReviewedWorkgroupSyncRuntimeAdapterV1> QuiescentStateV1<R, A> {
    fn release_retained(mut self) -> CompletedStateV1<A> {
        drop(self.retained.take());
        CompletedStateV1 {
            adapter: self.adapter.take(),
            environment: self.environment.clone(),
            load: self.load.clone(),
            resolution: self.resolution.clone(),
            executable: self.executable.take(),
            dispatch_identity: self.dispatch_identity,
        }
    }
}

struct CompletedStateV1<A: ReviewedWorkgroupSyncRuntimeAdapterV1> {
    adapter: Option<A>,
    environment: HsaEnvironmentObservationV1,
    load: HsaCodeObjectLoadObservationV1,
    resolution: HsaKernelResolutionObservationV1,
    executable: Option<A::Executable>,
    dispatch_identity: [u8; 16],
}

impl<A: ReviewedWorkgroupSyncRuntimeAdapterV1> Drop for CompletedStateV1<A> {
    fn drop(&mut self) {
        if let Some(executable) = self.executable.take() {
            terminal_unload(
                self.adapter
                    .as_mut()
                    .unwrap_or_else(|| std::process::abort()),
                executable,
                &self.environment,
                &self.load,
            );
        }
    }
}

#[doc(hidden)]
#[must_use = "the completed workgroup-sync request must be terminally unloaded"]
pub struct CompletedWorkgroupSyncV1<P, A: ReviewedWorkgroupSyncRuntimeAdapterV1> {
    state: CompletedStateV1<A>,
    lineage: WorkgroupSyncLifecycleIdentityV1,
    _profile: PhantomData<P>,
}

pub type CompletedWorkgroupLdsReductionV1<A> =
    CompletedWorkgroupSyncV1<WorkgroupLdsReductionProfileV1, A>;
pub type CompletedWorkgroupScopedAtomicV1<A> =
    CompletedWorkgroupSyncV1<WorkgroupScopedAtomicProfileV1, A>;

impl<P: ExactWorkgroupSyncHostProfileV1, A: ReviewedWorkgroupSyncRuntimeAdapterV1>
    CompletedWorkgroupSyncV1<P, A>
{
    pub const fn lineage(&self) -> WorkgroupSyncLifecycleIdentityV1 {
        self.lineage
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

    pub fn unload(mut self) -> UnloadedWorkgroupSyncV1<P> {
        let mut adapter = self.state.adapter.take().expect("completed adapter");
        let executable = self.state.executable.take().expect("completed executable");
        let unload = terminal_unload(
            &mut adapter,
            executable,
            &self.state.environment,
            &self.state.load,
        );
        let receipt = UnloadedWorkgroupSyncV1 {
            lineage: self.lineage,
            executable: self.state.load.executable_object(),
            kernel: self.state.resolution.kernel_object(),
            dispatch_identity: self.state.dispatch_identity,
            unload_identity: unload_identity(
                P::KIND,
                &unload,
                self.state.environment.runtime().instance(),
                self.state.environment.agent().agent_handle(),
            ),
            _profile: PhantomData,
        };
        drop(adapter);
        receipt
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkgroupSyncUnloadIdentityV1([u8; 32]);
impl WorkgroupSyncUnloadIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[doc(hidden)]
#[derive(Debug)]
pub struct UnloadedWorkgroupSyncV1<P> {
    lineage: WorkgroupSyncLifecycleIdentityV1,
    executable: HsaExecutableObjectIdentityV1,
    kernel: HsaKernelObjectIdentityV1,
    dispatch_identity: [u8; 16],
    unload_identity: WorkgroupSyncUnloadIdentityV1,
    _profile: PhantomData<P>,
}

pub type UnloadedWorkgroupLdsReductionV1 = UnloadedWorkgroupSyncV1<WorkgroupLdsReductionProfileV1>;
pub type UnloadedWorkgroupScopedAtomicV1 = UnloadedWorkgroupSyncV1<WorkgroupScopedAtomicProfileV1>;

impl<P: ExactWorkgroupSyncHostProfileV1> UnloadedWorkgroupSyncV1<P> {
    pub const fn lineage(&self) -> WorkgroupSyncLifecycleIdentityV1 {
        self.lineage
    }
    pub const fn executable_object(&self) -> HsaExecutableObjectIdentityV1 {
        self.executable
    }
    pub const fn kernel_object(&self) -> HsaKernelObjectIdentityV1 {
        self.kernel
    }
    pub const fn dispatch_identity(&self) -> [u8; 16] {
        self.dispatch_identity
    }
    pub const fn unload_identity(&self) -> WorkgroupSyncUnloadIdentityV1 {
        self.unload_identity
    }
    pub const fn proves_compiler_origin(&self) -> bool {
        false
    }
    pub const fn proves_compiler_refinement(&self) -> bool {
        false
    }
    pub const fn proves_generalized_race_freedom(&self) -> bool {
        false
    }
    pub const fn proves_formal_machine_safety(&self) -> bool {
        false
    }
}

fn dispatch_and_wait<
    R: RetainedWorkgroupSyncV1,
    P: ExactWorkgroupSyncHostProfileV1,
    A: ReviewedWorkgroupSyncRuntimeAdapterV1,
>(
    mut state: LoadedStateV1<R, A>,
) -> Result<QuiescentStateV1<R, A>, WorkgroupSyncDispatchErrorV1<A::Error>> {
    let geometry = exact_geometry::<P>();
    let explicit = state.kernarg.bytes[..P::EXPLICIT_KERNARG_BYTES].to_vec();
    let kernarg = &mut state.kernarg.bytes[..P::COMPLETE_KERNARG_BYTES];
    let executable = state.executable.as_ref().expect("loaded executable");
    let kernel = state.kernel.as_ref().expect("loaded kernel");
    let adapter = state.adapter.as_mut().expect("loaded adapter");
    let implicit = reviewed_call(|| unsafe {
        adapter.initialize_workgroup_sync_implicit_kernarg_v1(
            P::KIND,
            executable,
            kernel,
            geometry,
            P::EXPLICIT_KERNARG_BYTES,
            P::EXPLICIT_KERNARG_BYTES,
            IMPLICIT_KERNARG_BYTES,
            kernarg,
        )
    })
    .map_err(WorkgroupSyncDispatchErrorV1::ImplicitAdapter)?;
    if kernarg[..P::EXPLICIT_KERNARG_BYTES] != *explicit {
        return Err(WorkgroupSyncDispatchErrorV1::ExplicitKernargMutation);
    }
    validate_implicit::<P>(&state.load, &state.resolution, geometry, implicit)
        .map_err(WorkgroupSyncDispatchErrorV1::ImplicitObservation)?;
    if let Some(offset) = P::HIDDEN_DYNAMIC_LDS_OFFSET {
        let actual = u32::from_le_bytes(
            kernarg[offset..offset + 4]
                .try_into()
                .expect("exact hidden field"),
        );
        if actual != P::HIDDEN_DYNAMIC_LDS_VALUE {
            return Err(WorkgroupSyncDispatchErrorV1::HiddenDynamicLdsMutation);
        }
    }
    let dispatch =
        reviewed_call(|| unsafe { adapter.launch_and_wait(executable, kernel, geometry, kernarg) })
            .map_err(WorkgroupSyncDispatchErrorV1::DispatchAdapter)?;
    validate_dispatch(&state.load, &state.resolution, geometry, &dispatch)
        .map_err(WorkgroupSyncDispatchErrorV1::DispatchObservation)?;
    drop(state.kernel.take());
    Ok(QuiescentStateV1 {
        retained: state.retained.take(),
        adapter: state.adapter.take(),
        environment: state.environment.clone(),
        load: state.load.clone(),
        resolution: state.resolution.clone(),
        executable: state.executable.take(),
        dispatch_identity: dispatch.dispatch_identity(),
    })
}

fn load_after_context_match<
    R: RetainedWorkgroupSyncV1,
    P: ExactWorkgroupSyncHostProfileV1,
    A: ReviewedWorkgroupSyncRuntimeAdapterV1,
>(
    retained: R,
    mut adapter: A,
) -> Result<LoadedStateV1<R, A>, WorkgroupSyncLoadErrorV1<A::Error>> {
    let environment = reviewed_call(|| unsafe { adapter.observe_environment() })
        .map_err(WorkgroupSyncLoadErrorV1::EnvironmentAdapter)?;
    validate_environment(retained.target_v1(), retained.ordinal_v1(), &environment)
        .map_err(WorkgroupSyncLoadErrorV1::Environment)?;
    let bytes = retained.finalized_bytes_v1();
    if !retained.finalized_identity_v1().matches(bytes) {
        return Err(WorkgroupSyncLoadErrorV1::LoadObservation(
            "finalized identity",
        ));
    }
    let digest = DigestAlgorithm::Sha256.calculate(bytes);
    let byte_len = u64::try_from(bytes.len())
        .map_err(|_| WorkgroupSyncLoadErrorV1::LoadObservation("byte length"))?;
    let (executable, load) = reviewed_call(|| unsafe { adapter.load_executable(bytes, digest) })
        .map_err(WorkgroupSyncLoadErrorV1::LoadAdapter)?;
    if let Err(field) = validate_load(&environment, digest, byte_len, &load) {
        terminal_unload(&mut adapter, executable, &environment, &load);
        return Err(WorkgroupSyncLoadErrorV1::LoadObservation(field));
    }
    let (kernel, resolution) =
        match reviewed_call(|| unsafe { adapter.resolve_kernel(&executable, P::EXPORT_SYMBOL) }) {
            Ok(value) => value,
            Err(error) => {
                terminal_unload(&mut adapter, executable, &environment, &load);
                return Err(WorkgroupSyncLoadErrorV1::KernelAdapter(error));
            }
        };
    if let Err(field) = validate_kernel::<P>(&load, &resolution) {
        drop(kernel);
        terminal_unload(&mut adapter, executable, &environment, &load);
        return Err(WorkgroupSyncLoadErrorV1::KernelObservation(field));
    }
    let resources = match reviewed_call(|| unsafe {
        adapter.observe_workgroup_sync_resources_v1(P::KIND, &executable, &kernel)
    }) {
        Ok(value) => value,
        Err(error) => {
            drop(kernel);
            terminal_unload(&mut adapter, executable, &environment, &load);
            return Err(WorkgroupSyncLoadErrorV1::ResourceAdapter(error));
        }
    };
    if let Err(field) = validate_resources::<P>(&load, &resolution, resources) {
        drop(kernel);
        terminal_unload(&mut adapter, executable, &environment, &load);
        return Err(WorkgroupSyncLoadErrorV1::ResourceObservation(field));
    }
    let explicit = retained.explicit_kernarg_v1();
    if explicit.len() != P::EXPLICIT_KERNARG_BYTES {
        drop(kernel);
        terminal_unload(&mut adapter, executable, &environment, &load);
        return Err(WorkgroupSyncLoadErrorV1::KernelObservation(
            "explicit kernarg length",
        ));
    }
    let kernarg = CompleteKernargV1::from_explicit(explicit, P::COMPLETE_KERNARG_BYTES);
    if !(kernarg.bytes.as_ptr() as usize).is_multiple_of(RUNTIME_KERNARG_ALIGNMENT as usize) {
        drop(kernel);
        terminal_unload(&mut adapter, executable, &environment, &load);
        return Err(WorkgroupSyncLoadErrorV1::KernelObservation(
            "staging alignment",
        ));
    }
    Ok(LoadedStateV1 {
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

fn validate_join_borrowed<P: ExactWorkgroupSyncHostProfileV1>(
    admission: &PreparedFinalizedWorkgroupSyncHsacoV1,
    host: &ExactHostArgumentsV1<'_, '_, '_>,
) -> Result<(), WorkgroupSyncJoinErrorV1> {
    // SAFETY: validation borrows exact bytes only to match the retained
    // finalized identity; no bytes leave this function.
    let bytes = unsafe { admission.exact_finalized_bytes_for_reviewed_workgroup_sync_runtime_v1() };
    if !admission.finalized_output_identity().matches(bytes) {
        return Err(WorkgroupSyncJoinErrorV1::FinalizedOutput);
    }
    if admission.profile() != P::KIND || admission.exchange().compiler_pins().profile() != P::KIND {
        return Err(WorkgroupSyncJoinErrorV1::WrongProfile);
    }
    for (matches, field) in [
        (admission.target().to_string() == TARGET, "target"),
        (
            admission.code_object_version() == CodeObjectVersion::V6,
            "code-object version",
        ),
        (
            admission.canonical_digest().as_bytes() != &[0; 32],
            "descriptor digest",
        ),
        (
            admission.exact_source_kir_profile_was_checked(),
            "source/KIR/profile admission",
        ),
        (
            admission.direct_upstream_llvm_lld_exchange_was_checked(),
            "direct LLVM/LLD exchange",
        ),
        (
            !admission.grants_publication_authority(),
            "inert publication boundary",
        ),
        (!admission.grants_load_authority(), "inert load boundary"),
        (
            !admission.grants_launch_authority(),
            "inert launch boundary",
        ),
    ] {
        if !matches {
            return Err(WorkgroupSyncJoinErrorV1::ProfileField(field));
        }
    }
    let profile_matches = match (P::KIND, host) {
        (WorkgroupSyncProfileKindV1::LdsReduction, ExactHostArgumentsV1::Lds(host)) => [
            host.target() == TARGET,
            host.export_symbol() == P::EXPORT_SYMBOL,
            host.grid() == GRID,
            host.workgroup() == WORKGROUP,
            host.wavefront_size() == WAVEFRONT_SIZE,
            host.explicit_kernarg_byte_len() == P::EXPLICIT_KERNARG_BYTES,
            host.complete_kernarg_byte_len() == P::COMPLETE_KERNARG_BYTES,
            host.descriptor_kernarg_alignment() == DESCRIPTOR_KERNARG_ALIGNMENT,
            host.runtime_kernarg_alignment() == RUNTIME_KERNARG_ALIGNMENT,
            host.static_group_segment_bytes() == STATIC_GROUP_SEGMENT_BYTES,
            host.private_segment_bytes() == PRIVATE_SEGMENT_BYTES,
            host.dynamic_lds_bytes() == P::DYNAMIC_LDS_BYTES,
            Some(host.hidden_dynamic_lds_offset()) == P::HIDDEN_DYNAMIC_LDS_OFFSET,
            host.hidden_dynamic_lds_value() == P::HIDDEN_DYNAMIC_LDS_VALUE,
        ]
        .into_iter()
        .all(|value| value),
        (WorkgroupSyncProfileKindV1::ScopedAtomic, ExactHostArgumentsV1::Atomic(host)) => [
            host.target() == TARGET,
            host.export_symbol() == P::EXPORT_SYMBOL,
            host.grid() == GRID,
            host.workgroup() == WORKGROUP,
            host.wavefront_size() == WAVEFRONT_SIZE,
            host.explicit_kernarg_byte_len() == P::EXPLICIT_KERNARG_BYTES,
            host.complete_kernarg_byte_len() == P::COMPLETE_KERNARG_BYTES,
            host.descriptor_kernarg_alignment() == DESCRIPTOR_KERNARG_ALIGNMENT,
            host.runtime_kernarg_alignment() == RUNTIME_KERNARG_ALIGNMENT,
            host.static_group_segment_bytes() == STATIC_GROUP_SEGMENT_BYTES,
            host.private_segment_bytes() == PRIVATE_SEGMENT_BYTES,
            host.dynamic_lds_bytes() == P::DYNAMIC_LDS_BYTES,
            host.host_visible_target_elements() == 1,
            P::HIDDEN_DYNAMIC_LDS_OFFSET.is_none(),
        ]
        .into_iter()
        .all(|value| value),
        _ => false,
    };
    if !profile_matches {
        return Err(WorkgroupSyncJoinErrorV1::HostField(
            "exact ABI/resources/profile",
        ));
    }
    Ok(())
}

fn reviewed_call<T>(call: impl FnOnce() -> T) -> T {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(call)) {
        Ok(value) => value,
        Err(payload) => {
            std::mem::forget(payload);
            std::process::abort()
        }
    }
}

fn terminal_unload<A: ReviewedWorkgroupSyncRuntimeAdapterV1>(
    adapter: &mut A,
    executable: A::Executable,
    environment: &HsaEnvironmentObservationV1,
    load: &HsaCodeObjectLoadObservationV1,
) -> HsaUnloadObservationV1 {
    let unload = match reviewed_call(|| unsafe { adapter.unload_executable(executable) }) {
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
    value: &HsaEnvironmentObservationV1,
) -> Result<(), &'static str> {
    if target != TARGET {
        return Err("requested target");
    }
    let expected = AmdTargetId::parse(TARGET).expect("static target");
    let actual = value.physical_device().target();
    for (matches, field) in [
        (
            expected.is_compatible_with_observed(&actual),
            "physical target",
        ),
        (value.agent().target() == actual, "agent target"),
        (
            value.physical_device().hip_ordinal() == ordinal,
            "HIP ordinal",
        ),
        (
            value.agent().runtime_instance() == value.runtime().instance(),
            "runtime instance",
        ),
        (
            value.agent().physical_device_uuid() == value.physical_device().uuid(),
            "physical device",
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
    value: &HsaCodeObjectLoadObservationV1,
) -> Result<(), &'static str> {
    let expected = HsaCodeObjectLoadObservationV1::new(
        digest,
        byte_len,
        environment.runtime().instance(),
        environment.agent().agent_handle(),
        value.executable_object(),
    );
    if value == &expected {
        Ok(())
    } else {
        Err("identity or content")
    }
}

fn validate_kernel<P: ExactWorkgroupSyncHostProfileV1>(
    load: &HsaCodeObjectLoadObservationV1,
    value: &HsaKernelResolutionObservationV1,
) -> Result<(), &'static str> {
    let expected = HsaKernelResolutionObservationV1::new(
        load.executable_object(),
        value.kernel_object(),
        P::EXPORT_SYMBOL,
        P::COMPLETE_KERNARG_BYTES as u64,
        RUNTIME_KERNARG_ALIGNMENT,
        0,
        0,
    )
    .expect("static kernel observation");
    if value == &expected {
        Ok(())
    } else {
        Err("object, symbol, size, or runtime alignment")
    }
}

fn validate_resources<P: ExactWorkgroupSyncHostProfileV1>(
    load: &HsaCodeObjectLoadObservationV1,
    resolution: &HsaKernelResolutionObservationV1,
    value: WorkgroupSyncKernelResourceObservationV1,
) -> Result<(), &'static str> {
    let expected = WorkgroupSyncKernelResourceObservationV1::new(
        P::KIND,
        load.executable_object(),
        resolution.kernel_object(),
        STATIC_GROUP_SEGMENT_BYTES,
        PRIVATE_SEGMENT_BYTES,
    );
    if value == expected {
        Ok(())
    } else {
        Err("profile, object, group, or private segment")
    }
}

fn validate_implicit<P: ExactWorkgroupSyncHostProfileV1>(
    load: &HsaCodeObjectLoadObservationV1,
    resolution: &HsaKernelResolutionObservationV1,
    geometry: HsaLaunchGeometryV1,
    value: WorkgroupSyncImplicitKernargObservationV1,
) -> Result<(), &'static str> {
    let expected = WorkgroupSyncImplicitKernargObservationV1::new(
        P::KIND,
        load.executable_object(),
        resolution.kernel_object(),
        geometry,
        P::EXPLICIT_KERNARG_BYTES as u64,
        P::EXPLICIT_KERNARG_BYTES as u64,
        IMPLICIT_KERNARG_BYTES as u64,
        P::HIDDEN_DYNAMIC_LDS_OFFSET.map(|offset| offset as u64),
        P::HIDDEN_DYNAMIC_LDS_VALUE,
        P::DYNAMIC_LDS_BYTES,
        true,
    );
    if value == expected {
        Ok(())
    } else {
        Err("profile, object, geometry, span, hidden LDS, AQL LDS, or completion")
    }
}

fn validate_dispatch(
    load: &HsaCodeObjectLoadObservationV1,
    resolution: &HsaKernelResolutionObservationV1,
    geometry: HsaLaunchGeometryV1,
    value: &HsaDispatchObservationV1,
) -> Result<(), &'static str> {
    let expected = HsaDispatchObservationV1::new(
        value.dispatch_identity(),
        load.executable_object(),
        resolution.kernel_object(),
        geometry,
        true,
    )
    .expect("valid dispatch identity");
    if value == &expected {
        Ok(())
    } else {
        Err("object, geometry, or completion")
    }
}

fn validate_unload(
    environment: &HsaEnvironmentObservationV1,
    load: &HsaCodeObjectLoadObservationV1,
    value: &HsaUnloadObservationV1,
) -> Result<(), &'static str> {
    let expected = HsaUnloadObservationV1::new(
        load.executable_object(),
        environment.runtime().instance(),
        environment.agent().agent_handle(),
        true,
    );
    if value == &expected {
        Ok(())
    } else {
        Err("object, runtime, agent, or completion")
    }
}

fn exact_geometry<P: ExactWorkgroupSyncHostProfileV1>() -> HsaLaunchGeometryV1 {
    HsaLaunchGeometryV1::new(GRID, WORKGROUP, P::DYNAMIC_LDS_BYTES)
}

fn unload_identity(
    profile: WorkgroupSyncProfileKindV1,
    unload: &HsaUnloadObservationV1,
    runtime: [u8; 16],
    agent: u64,
) -> WorkgroupSyncUnloadIdentityV1 {
    let mut digest = Sha256::new();
    let profile = [match profile {
        WorkgroupSyncProfileKindV1::LdsReduction => 1,
        WorkgroupSyncProfileKindV1::ScopedAtomic => 2,
    }];
    for field in [
        UNLOAD_IDENTITY_DOMAIN_V1,
        &profile,
        unload.executable_object().as_bytes(),
        &runtime,
        &agent.to_le_bytes(),
        &[u8::from(unload.released())],
    ] {
        digest.update((field.len() as u64).to_le_bytes());
        digest.update(field);
    }
    WorkgroupSyncUnloadIdentityV1(digest.finalize().into())
}

const _: () = assert!(LDS_EXPLICIT_KERNARG_BYTES == 32);
const _: () = assert!(LDS_COMPLETE_KERNARG_BYTES == 288);
const _: () = assert!(ATOMIC_EXPLICIT_KERNARG_BYTES == 40);
const _: () = assert!(ATOMIC_COMPLETE_KERNARG_BYTES == 296);
const _: () = assert!(IMPLICIT_KERNARG_BYTES == 256);
const _: () = assert!(STATIC_GROUP_SEGMENT_BYTES == 0);
const _: () = assert!(PRIVATE_SEGMENT_BYTES == 0);
const _: () = assert!(HIDDEN_DYNAMIC_LDS_OFFSET == 152);

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    struct TestRetainedV1 {
        bytes: Vec<u8>,
        identity: ContentIdentityV1,
        explicit: Vec<u8>,
        drops: Arc<AtomicUsize>,
    }

    impl Drop for TestRetainedV1 {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::SeqCst);
        }
    }

    impl RetainedWorkgroupSyncV1 for TestRetainedV1 {
        fn target_v1(&self) -> &str {
            TARGET
        }
        fn ordinal_v1(&self) -> i32 {
            0
        }
        fn finalized_bytes_v1(&self) -> &[u8] {
            &self.bytes
        }
        fn finalized_identity_v1(&self) -> ContentIdentityV1 {
            self.identity
        }
        fn explicit_kernarg_v1(&self) -> &[u8] {
            &self.explicit
        }
    }

    pub(crate) struct TestLoadedWorkgroupSyncV1<
        P: ExactWorkgroupSyncHostProfileV1,
        A: ReviewedWorkgroupSyncRuntimeAdapterV1,
    > {
        state: LoadedStateV1<TestRetainedV1, A>,
        _profile: PhantomData<P>,
    }

    impl<P: ExactWorkgroupSyncHostProfileV1, A: ReviewedWorkgroupSyncRuntimeAdapterV1>
        TestLoadedWorkgroupSyncV1<P, A>
    {
        pub(crate) fn dispatch_and_wait(
            self,
        ) -> Result<TestCompletedWorkgroupSyncV1<P, A>, WorkgroupSyncDispatchErrorV1<A::Error>>
        {
            dispatch_and_wait::<TestRetainedV1, P, A>(self.state).map(|state| {
                TestCompletedWorkgroupSyncV1 {
                    state: state.release_retained(),
                    _profile: PhantomData,
                }
            })
        }
    }

    pub(crate) struct TestCompletedWorkgroupSyncV1<
        P: ExactWorkgroupSyncHostProfileV1,
        A: ReviewedWorkgroupSyncRuntimeAdapterV1,
    > {
        state: CompletedStateV1<A>,
        _profile: PhantomData<P>,
    }

    impl<P: ExactWorkgroupSyncHostProfileV1, A: ReviewedWorkgroupSyncRuntimeAdapterV1>
        TestCompletedWorkgroupSyncV1<P, A>
    {
        pub(crate) fn unload(mut self) -> TestUnloadedWorkgroupSyncV1 {
            let mut adapter = self.state.adapter.take().expect("test adapter");
            let executable = self.state.executable.take().expect("test executable");
            let unload = terminal_unload(
                &mut adapter,
                executable,
                &self.state.environment,
                &self.state.load,
            );
            let receipt = TestUnloadedWorkgroupSyncV1 {
                executable_object: self.state.load.executable_object(),
                kernel_object: self.state.resolution.kernel_object(),
                dispatch_identity: self.state.dispatch_identity,
                unload_identity: unload_identity(
                    P::KIND,
                    &unload,
                    self.state.environment.runtime().instance(),
                    self.state.environment.agent().agent_handle(),
                ),
            };
            drop(adapter);
            receipt
        }
    }

    pub(crate) struct TestUnloadedWorkgroupSyncV1 {
        pub(crate) executable_object: HsaExecutableObjectIdentityV1,
        pub(crate) kernel_object: HsaKernelObjectIdentityV1,
        pub(crate) dispatch_identity: [u8; 16],
        pub(crate) unload_identity: WorkgroupSyncUnloadIdentityV1,
    }

    pub(crate) fn load_test_lifecycle_v1<
        P: ExactWorkgroupSyncHostProfileV1,
        A: ReviewedWorkgroupSyncRuntimeAdapterV1,
    >(
        adapter: A,
        context_matches: bool,
        drops: Arc<AtomicUsize>,
    ) -> Result<TestLoadedWorkgroupSyncV1<P, A>, WorkgroupSyncLoadErrorV1<A::Error>> {
        let bytes = vec![0x5a; 96];
        let retained = TestRetainedV1 {
            identity: ContentIdentityV1::calculate(&bytes),
            bytes,
            explicit: (0..P::EXPLICIT_KERNARG_BYTES)
                .map(|index| index as u8)
                .collect(),
            drops,
        };
        if !context_matches {
            return Err(WorkgroupSyncLoadErrorV1::ContextIdentity);
        }
        load_after_context_match::<_, P, A>(retained, adapter).map(|state| {
            TestLoadedWorkgroupSyncV1 {
                state,
                _profile: PhantomData,
            }
        })
    }

    pub(crate) const fn test_complete_bytes_v1(profile: WorkgroupSyncProfileKindV1) -> usize {
        match profile {
            WorkgroupSyncProfileKindV1::LdsReduction => LDS_COMPLETE_KERNARG_BYTES,
            WorkgroupSyncProfileKindV1::ScopedAtomic => ATOMIC_COMPLETE_KERNARG_BYTES,
        }
    }
    pub(crate) const fn test_explicit_bytes_v1(profile: WorkgroupSyncProfileKindV1) -> usize {
        match profile {
            WorkgroupSyncProfileKindV1::LdsReduction => LDS_EXPLICIT_KERNARG_BYTES,
            WorkgroupSyncProfileKindV1::ScopedAtomic => ATOMIC_EXPLICIT_KERNARG_BYTES,
        }
    }
    pub(crate) const fn test_hidden_lds_offset_v1() -> usize {
        HIDDEN_DYNAMIC_LDS_OFFSET
    }
    pub(crate) const fn test_implicit_bytes_v1() -> usize {
        IMPLICIT_KERNARG_BYTES
    }
    pub(crate) const fn test_runtime_alignment_v1() -> u64 {
        RUNTIME_KERNARG_ALIGNMENT
    }
}
