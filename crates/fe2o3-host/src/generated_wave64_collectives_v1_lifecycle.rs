//! Protected one-shot HSA lifecycle for masked Wave64 collectives V1.

use crate::{
    HsaCodeObjectLoadObservationV1, HsaDispatchObservationV1, HsaEnvironmentObservationV1,
    HsaExecutableObjectIdentityV1, HsaImplicitKernargInitializationObservationV1,
    HsaKernelObjectIdentityV1, HsaKernelResolutionObservationV1, HsaLaunchGeometryV1,
    HsaUnloadObservationV1, ReviewedHsaImplicitKernargAdapterV1,
    generated_wave64_collectives_v1::{
        COMPLETE_KERNARG_BYTES, DESCRIPTOR_KERNARG_ALIGNMENT, EXPLICIT_KERNARG_BYTES, GRID,
        GeneratedWave64CollectivesV1HostAdapterV1, TARGET, WORKGROUP,
    },
};
use fe2o3_amd_target::AmdTargetId;
use fe2o3_artifacts::{DigestAlgorithm, PayloadDigest};
use fe2o3_core::ContextIdentity;
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, FinalizedWave64CollectivesV1HsacoIdentityV1,
    FinalizedWorkerV2HsacoIdentityV1, PreparedFinalizedWave64CollectivesV1HsacoV1,
    Wave64CollectivesV1WorkerExchangeIdentityV1,
};
use fe2o3_kernel_descriptor::CodeObjectVersion;
use sha2::{Digest, Sha256};
use std::{error::Error, fmt};

const EXPORT_SYMBOL: &str = "wave64_collectives_v1";
const WAVEFRONT_SIZE: u32 = 64;
const IMPLICIT_KERNARG_BYTES: usize = COMPLETE_KERNARG_BYTES - EXPLICIT_KERNARG_BYTES;
const RUNTIME_KERNARG_ALIGNMENT: u64 = 16;
const GROUP_SEGMENT_BYTES: u32 = 0;
const PRIVATE_SEGMENT_BYTES: u32 = 0;
const DYNAMIC_LDS_BYTES: u32 = 0;
const UNLOAD_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/WAVE64-COLLECTIVES/UNLOAD/V1\0";

/// Exact compiler, Worker V2, and finalizer lineage retained by the lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Wave64CollectivesLifecycleIdentityV1 {
    finalizer: FinalizedWave64CollectivesV1HsacoIdentityV1,
    structural_finalizer: FinalizedWorkerV2HsacoIdentityV1,
    worker_exchange: Wave64CollectivesV1WorkerExchangeIdentityV1,
    compiler_module: ContentIdentityV1,
    compiler_source_authority: [u8; 32],
    compiler_source: [u8; 32],
    portable_mir: [u8; 32],
    kernel_ir: [u8; 32],
    descriptor_profile: [u8; 32],
    worker_executable: ContentIdentityV1,
    worker_build: [u8; 32],
    llvm_build: [u8; 32],
    linked_output: ContentIdentityV1,
    finalized_output: ContentIdentityV1,
}

impl Wave64CollectivesLifecycleIdentityV1 {
    pub const fn finalizer(self) -> FinalizedWave64CollectivesV1HsacoIdentityV1 {
        self.finalizer
    }
    pub const fn structural_finalizer(self) -> FinalizedWorkerV2HsacoIdentityV1 {
        self.structural_finalizer
    }
    pub const fn worker_exchange(self) -> Wave64CollectivesV1WorkerExchangeIdentityV1 {
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
    pub const fn portable_mir(self) -> [u8; 32] {
        self.portable_mir
    }
    pub const fn kernel_ir(self) -> [u8; 32] {
        self.kernel_ir
    }
    pub const fn descriptor_profile(self) -> [u8; 32] {
        self.descriptor_profile
    }
    pub const fn worker_executable(self) -> ContentIdentityV1 {
        self.worker_executable
    }
    pub const fn worker_build(self) -> [u8; 32] {
        self.worker_build
    }
    pub const fn llvm_build(self) -> [u8; 32] {
        self.llvm_build
    }
    pub const fn linked_output(self) -> ContentIdentityV1 {
        self.linked_output
    }
    pub const fn finalized_output(self) -> ContentIdentityV1 {
        self.finalized_output
    }

    fn from_admission(admission: &PreparedFinalizedWave64CollectivesV1HsacoV1) -> Self {
        let exchange = admission.exchange();
        let compiler = exchange.compiler_pins();
        let worker = exchange.worker_pins();
        Self {
            finalizer: admission.identity(),
            structural_finalizer: admission.structural_finalization_identity(),
            worker_exchange: exchange.identity(),
            compiler_module: exchange.compiler_module_identity(),
            compiler_source_authority: *compiler.source_authority(),
            compiler_source: *compiler.source_sha256(),
            portable_mir: *compiler.portable_mir_sha256(),
            kernel_ir: compiler.canonical_kernel_ir_identity(),
            descriptor_profile: compiler.descriptor_profile_identity(),
            worker_executable: worker.executable(),
            worker_build: worker.worker_build_identity_sha256(),
            llvm_build: worker.llvm_build_identity_sha256(),
            linked_output: exchange.linked_output_identity(),
            finalized_output: admission.finalized_output_identity(),
        }
    }
}

/// Runtime-reported exact resource facts, without native handles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Wave64CollectivesKernelResourceObservationV1 {
    executable: HsaExecutableObjectIdentityV1,
    kernel: HsaKernelObjectIdentityV1,
    group_segment_bytes: u32,
    private_segment_bytes: u32,
}

impl Wave64CollectivesKernelResourceObservationV1 {
    pub const fn new(
        executable: HsaExecutableObjectIdentityV1,
        kernel: HsaKernelObjectIdentityV1,
        group_segment_bytes: u32,
        private_segment_bytes: u32,
    ) -> Self {
        Self {
            executable,
            kernel,
            group_segment_bytes,
            private_segment_bytes,
        }
    }
    pub const fn executable(self) -> HsaExecutableObjectIdentityV1 {
        self.executable
    }
    pub const fn kernel(self) -> HsaKernelObjectIdentityV1 {
        self.kernel
    }
    pub const fn group_segment_bytes(self) -> u32 {
        self.group_segment_bytes
    }
    pub const fn private_segment_bytes(self) -> u32 {
        self.private_segment_bytes
    }
}

/// Reviewed exact-profile extension over the generic HSA lifecycle.
///
/// # Safety
///
/// The context identity and resource observation must describe the exact
/// private objects supplied to each call. Methods may not unwind. All inherited
/// lifecycle methods must provide bounded completion and terminal ambiguity
/// behavior as required by [`ReviewedHsaImplicitKernargAdapterV1`].
pub unsafe trait ReviewedWave64CollectivesRuntimeAdapterV1:
    ReviewedHsaImplicitKernargAdapterV1
{
    /// Returns the identity of the exact retained host context.
    ///
    /// # Safety
    ///
    /// The identity must come from the context retained by this adapter and
    /// the method must not unwind.
    unsafe fn context_identity_v1(&mut self) -> ContextIdentity;

    /// Observes resources for the supplied private executable/kernel pair.
    ///
    /// # Safety
    ///
    /// The observation must be derived only from these exact objects, must
    /// not expose their native handles, and the method must not unwind.
    unsafe fn observe_wave64_collectives_resources_v1(
        &mut self,
        executable: &Self::Executable,
        kernel: &Self::Kernel,
    ) -> Result<Wave64CollectivesKernelResourceObservationV1, Self::Error>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Wave64CollectivesJoinErrorV1 {
    FinalizedOutput,
    ProfileField(&'static str),
    HostField(&'static str),
}

impl fmt::Display for Wave64CollectivesJoinErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FinalizedOutput => formatter.write_str("finalized output identity mismatch"),
            Self::ProfileField(field) => write!(formatter, "finalized Wave64 {field} drifted"),
            Self::HostField(field) => write!(formatter, "generated Wave64 host {field} drifted"),
        }
    }
}
impl Error for Wave64CollectivesJoinErrorV1 {}

#[derive(Debug)]
#[non_exhaustive]
pub enum Wave64CollectivesLoadErrorV1<E> {
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

impl<E: fmt::Display> fmt::Display for Wave64CollectivesLoadErrorV1<E> {
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
impl<E: Error + 'static> Error for Wave64CollectivesLoadErrorV1<E> {}

#[derive(Debug)]
#[non_exhaustive]
pub enum Wave64CollectivesDispatchErrorV1<E> {
    ImplicitAdapter(E),
    ImplicitObservation(&'static str),
    ExplicitKernargMutation,
    DispatchAdapter(E),
    DispatchObservation(&'static str),
}

impl<E: fmt::Display> fmt::Display for Wave64CollectivesDispatchErrorV1<E> {
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
impl<E: Error + 'static> Error for Wave64CollectivesDispatchErrorV1<E> {}

/// Inert one-shot join of finalizer admission and exact typed arguments.
#[must_use = "the joined Wave64 request must be loaded or dropped"]
pub struct JoinedWave64CollectivesV1<'input, 'reduction, 'inclusive, 'exclusive> {
    admission: PreparedFinalizedWave64CollectivesV1HsacoV1,
    host: GeneratedWave64CollectivesV1HostAdapterV1<'input, 'reduction, 'inclusive, 'exclusive>,
    lineage: Wave64CollectivesLifecycleIdentityV1,
}

impl fmt::Debug for JoinedWave64CollectivesV1<'_, '_, '_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JoinedWave64CollectivesV1")
            .field("lineage", &self.lineage)
            .finish_non_exhaustive()
    }
}

pub fn join_wave64_collectives_v1<'input, 'reduction, 'inclusive, 'exclusive>(
    admission: PreparedFinalizedWave64CollectivesV1HsacoV1,
    host: GeneratedWave64CollectivesV1HostAdapterV1<'input, 'reduction, 'inclusive, 'exclusive>,
) -> Result<
    JoinedWave64CollectivesV1<'input, 'reduction, 'inclusive, 'exclusive>,
    Wave64CollectivesJoinErrorV1,
> {
    validate_join(&admission, &host)?;
    let lineage = Wave64CollectivesLifecycleIdentityV1::from_admission(&admission);
    Ok(JoinedWave64CollectivesV1 {
        admission,
        host,
        lineage,
    })
}

impl<'input, 'reduction, 'inclusive, 'exclusive>
    JoinedWave64CollectivesV1<'input, 'reduction, 'inclusive, 'exclusive>
{
    pub const fn lineage(&self) -> Wave64CollectivesLifecycleIdentityV1 {
        self.lineage
    }

    pub fn load<A: ReviewedWave64CollectivesRuntimeAdapterV1>(
        self,
        mut adapter: A,
    ) -> Result<
        LoadedWave64CollectivesV1<'input, 'reduction, 'inclusive, 'exclusive, A>,
        Wave64CollectivesLoadErrorV1<A::Error>,
    > {
        let context = reviewed_call(|| unsafe { adapter.context_identity_v1() });
        if !self
            .host
            .observed_context_v1()
            .matches_core_context_identity_v1(context)
        {
            return Err(Wave64CollectivesLoadErrorV1::ContextIdentity);
        }
        Ok(LoadedWave64CollectivesV1 {
            state: load_after_context_match(self, adapter)?,
        })
    }
}

trait RetainedWave64CollectivesV1 {
    fn target_v1(&self) -> &str;
    fn ordinal_v1(&self) -> i32;
    fn finalized_bytes_v1(&self) -> &[u8];
    fn finalized_identity_v1(&self) -> ContentIdentityV1;
    fn explicit_kernarg_v1(&self) -> &[u8; EXPLICIT_KERNARG_BYTES];
}

impl RetainedWave64CollectivesV1 for JoinedWave64CollectivesV1<'_, '_, '_, '_> {
    fn target_v1(&self) -> &str {
        self.host.target()
    }
    fn ordinal_v1(&self) -> i32 {
        self.host.observed_context_v1().device().ordinal()
    }
    fn finalized_bytes_v1(&self) -> &[u8] {
        // SAFETY: this private borrow is used only by the exact one-shot load
        // below, is never copied, and remains tied to the retained admission.
        unsafe {
            self.admission
                .exact_finalized_bytes_for_reviewed_wave64_runtime_v1()
        }
    }
    fn finalized_identity_v1(&self) -> ContentIdentityV1 {
        self.lineage.finalized_output
    }
    fn explicit_kernarg_v1(&self) -> &[u8; EXPLICIT_KERNARG_BYTES] {
        self.host.explicit_kernarg_bytes_v1()
    }
}

#[repr(C, align(16))]
struct Wave64KernargStorageV1 {
    bytes: [u8; COMPLETE_KERNARG_BYTES],
}

impl Wave64KernargStorageV1 {
    fn from_explicit(explicit: &[u8; EXPLICIT_KERNARG_BYTES]) -> Self {
        let mut value = Self {
            bytes: [0; COMPLETE_KERNARG_BYTES],
        };
        value.bytes[..EXPLICIT_KERNARG_BYTES].copy_from_slice(explicit);
        value
    }
}

struct LoadedStateV1<R, A: ReviewedWave64CollectivesRuntimeAdapterV1> {
    retained: Option<R>,
    adapter: Option<A>,
    environment: HsaEnvironmentObservationV1,
    load: HsaCodeObjectLoadObservationV1,
    resolution: HsaKernelResolutionObservationV1,
    resources: Wave64CollectivesKernelResourceObservationV1,
    executable: Option<A::Executable>,
    kernel: Option<A::Kernel>,
    kernarg: Wave64KernargStorageV1,
}

impl<R, A: ReviewedWave64CollectivesRuntimeAdapterV1> Drop for LoadedStateV1<R, A> {
    fn drop(&mut self) {
        self.kernel.take();
        if let Some(executable) = self.executable.take() {
            let adapter = self
                .adapter
                .as_mut()
                .unwrap_or_else(|| std::process::abort());
            terminal_unload(adapter, executable, &self.environment, &self.load);
        }
    }
}

#[must_use = "the loaded Wave64 request must dispatch or terminally unload"]
pub struct LoadedWave64CollectivesV1<
    'input,
    'reduction,
    'inclusive,
    'exclusive,
    A: ReviewedWave64CollectivesRuntimeAdapterV1,
> {
    state: LoadedStateV1<JoinedWave64CollectivesV1<'input, 'reduction, 'inclusive, 'exclusive>, A>,
}

impl<A: ReviewedWave64CollectivesRuntimeAdapterV1> fmt::Debug
    for LoadedWave64CollectivesV1<'_, '_, '_, '_, A>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LoadedWave64CollectivesV1")
            .field("load", &self.state.load)
            .field("resolution", &self.state.resolution)
            .field("resources", &self.state.resources)
            .finish_non_exhaustive()
    }
}

impl<'input, 'reduction, 'inclusive, 'exclusive, A: ReviewedWave64CollectivesRuntimeAdapterV1>
    LoadedWave64CollectivesV1<'input, 'reduction, 'inclusive, 'exclusive, A>
{
    pub fn dispatch_and_wait(
        self,
    ) -> Result<CompletedWave64CollectivesV1<A>, Wave64CollectivesDispatchErrorV1<A::Error>> {
        let lineage = self.state.retained.as_ref().expect("retained join").lineage;
        let quiescent = self.state.dispatch_and_wait()?;
        let state = quiescent.release_retained();
        Ok(CompletedWave64CollectivesV1 { state, lineage })
    }
}

struct QuiescentStateV1<R, A: ReviewedWave64CollectivesRuntimeAdapterV1> {
    retained: Option<R>,
    adapter: Option<A>,
    environment: HsaEnvironmentObservationV1,
    load: HsaCodeObjectLoadObservationV1,
    resolution: HsaKernelResolutionObservationV1,
    executable: Option<A::Executable>,
    dispatch_identity: [u8; 16],
}

impl<R, A: ReviewedWave64CollectivesRuntimeAdapterV1> Drop for QuiescentStateV1<R, A> {
    fn drop(&mut self) {
        if let Some(executable) = self.executable.take() {
            let adapter = self
                .adapter
                .as_mut()
                .unwrap_or_else(|| std::process::abort());
            terminal_unload(adapter, executable, &self.environment, &self.load);
        }
    }
}

impl<R, A: ReviewedWave64CollectivesRuntimeAdapterV1> QuiescentStateV1<R, A> {
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

struct CompletedStateV1<A: ReviewedWave64CollectivesRuntimeAdapterV1> {
    adapter: Option<A>,
    environment: HsaEnvironmentObservationV1,
    load: HsaCodeObjectLoadObservationV1,
    resolution: HsaKernelResolutionObservationV1,
    executable: Option<A::Executable>,
    dispatch_identity: [u8; 16],
}

impl<A: ReviewedWave64CollectivesRuntimeAdapterV1> Drop for CompletedStateV1<A> {
    fn drop(&mut self) {
        if let Some(executable) = self.executable.take() {
            let adapter = self
                .adapter
                .as_mut()
                .unwrap_or_else(|| std::process::abort());
            terminal_unload(adapter, executable, &self.environment, &self.load);
        }
    }
}

#[must_use = "the completed Wave64 request must be terminally unloaded"]
pub struct CompletedWave64CollectivesV1<A: ReviewedWave64CollectivesRuntimeAdapterV1> {
    state: CompletedStateV1<A>,
    lineage: Wave64CollectivesLifecycleIdentityV1,
}

impl<A: ReviewedWave64CollectivesRuntimeAdapterV1> CompletedWave64CollectivesV1<A> {
    pub const fn lineage(&self) -> Wave64CollectivesLifecycleIdentityV1 {
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

    pub fn unload(mut self) -> UnloadedWave64CollectivesV1 {
        let mut adapter = self.state.adapter.take().expect("completed adapter");
        let executable = self.state.executable.take().expect("completed executable");
        let unload = terminal_unload(
            &mut adapter,
            executable,
            &self.state.environment,
            &self.state.load,
        );
        let receipt = UnloadedWave64CollectivesV1 {
            lineage: self.lineage,
            executable: self.state.load.executable_object(),
            kernel: self.state.resolution.kernel_object(),
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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Wave64CollectivesUnloadIdentityV1([u8; 32]);
impl Wave64CollectivesUnloadIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Debug)]
pub struct UnloadedWave64CollectivesV1 {
    lineage: Wave64CollectivesLifecycleIdentityV1,
    executable: HsaExecutableObjectIdentityV1,
    kernel: HsaKernelObjectIdentityV1,
    dispatch_identity: [u8; 16],
    unload_identity: Wave64CollectivesUnloadIdentityV1,
}

impl UnloadedWave64CollectivesV1 {
    pub const fn lineage(&self) -> Wave64CollectivesLifecycleIdentityV1 {
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
    pub const fn unload_identity(&self) -> Wave64CollectivesUnloadIdentityV1 {
        self.unload_identity
    }
    pub const fn proves_functional_collectives(&self) -> bool {
        false
    }
    pub const fn proves_source_machine_refinement(&self) -> bool {
        false
    }
    pub const fn proves_verus_verification(&self) -> bool {
        false
    }
}

impl<R: RetainedWave64CollectivesV1, A: ReviewedWave64CollectivesRuntimeAdapterV1>
    LoadedStateV1<R, A>
{
    fn dispatch_and_wait(
        mut self,
    ) -> Result<QuiescentStateV1<R, A>, Wave64CollectivesDispatchErrorV1<A::Error>> {
        let geometry = exact_geometry();
        let explicit = self.kernarg.bytes[..EXPLICIT_KERNARG_BYTES].to_vec();
        let executable = self.executable.as_ref().expect("loaded executable");
        let kernel = self.kernel.as_ref().expect("loaded kernel");
        let adapter = self.adapter.as_mut().expect("loaded adapter");
        let implicit = reviewed_call(|| unsafe {
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
        .map_err(Wave64CollectivesDispatchErrorV1::ImplicitAdapter)?;
        if self.kernarg.bytes[..EXPLICIT_KERNARG_BYTES] != *explicit {
            return Err(Wave64CollectivesDispatchErrorV1::ExplicitKernargMutation);
        }
        validate_implicit(&self.load, &self.resolution, geometry, &implicit)
            .map_err(Wave64CollectivesDispatchErrorV1::ImplicitObservation)?;
        let dispatch = reviewed_call(|| unsafe {
            adapter.launch_and_wait(executable, kernel, geometry, &mut self.kernarg.bytes)
        })
        .map_err(Wave64CollectivesDispatchErrorV1::DispatchAdapter)?;
        validate_dispatch(&self.load, &self.resolution, geometry, &dispatch)
            .map_err(Wave64CollectivesDispatchErrorV1::DispatchObservation)?;
        drop(self.kernel.take());
        Ok(QuiescentStateV1 {
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

fn load_after_context_match<
    R: RetainedWave64CollectivesV1,
    A: ReviewedWave64CollectivesRuntimeAdapterV1,
>(
    retained: R,
    mut adapter: A,
) -> Result<LoadedStateV1<R, A>, Wave64CollectivesLoadErrorV1<A::Error>> {
    let environment = reviewed_call(|| unsafe { adapter.observe_environment() })
        .map_err(Wave64CollectivesLoadErrorV1::EnvironmentAdapter)?;
    validate_environment(retained.target_v1(), retained.ordinal_v1(), &environment)
        .map_err(Wave64CollectivesLoadErrorV1::Environment)?;
    let bytes = retained.finalized_bytes_v1();
    if !retained.finalized_identity_v1().matches(bytes) {
        return Err(Wave64CollectivesLoadErrorV1::LoadObservation(
            "finalized identity",
        ));
    }
    let digest = DigestAlgorithm::Sha256.calculate(bytes);
    let byte_len = u64::try_from(bytes.len())
        .map_err(|_| Wave64CollectivesLoadErrorV1::LoadObservation("byte length"))?;
    let (executable, load) = reviewed_call(|| unsafe { adapter.load_executable(bytes, digest) })
        .map_err(Wave64CollectivesLoadErrorV1::LoadAdapter)?;
    if let Err(field) = validate_load(&environment, digest, byte_len, &load) {
        terminal_unload(&mut adapter, executable, &environment, &load);
        return Err(Wave64CollectivesLoadErrorV1::LoadObservation(field));
    }
    let (kernel, resolution) =
        match reviewed_call(|| unsafe { adapter.resolve_kernel(&executable, EXPORT_SYMBOL) }) {
            Ok(value) => value,
            Err(error) => {
                terminal_unload(&mut adapter, executable, &environment, &load);
                return Err(Wave64CollectivesLoadErrorV1::KernelAdapter(error));
            }
        };
    if let Err(field) = validate_kernel(&load, &resolution) {
        drop(kernel);
        terminal_unload(&mut adapter, executable, &environment, &load);
        return Err(Wave64CollectivesLoadErrorV1::KernelObservation(field));
    }
    let resources = match reviewed_call(|| unsafe {
        adapter.observe_wave64_collectives_resources_v1(&executable, &kernel)
    }) {
        Ok(value) => value,
        Err(error) => {
            drop(kernel);
            terminal_unload(&mut adapter, executable, &environment, &load);
            return Err(Wave64CollectivesLoadErrorV1::ResourceAdapter(error));
        }
    };
    if let Err(field) = validate_resources(&load, &resolution, resources) {
        drop(kernel);
        terminal_unload(&mut adapter, executable, &environment, &load);
        return Err(Wave64CollectivesLoadErrorV1::ResourceObservation(field));
    }
    let kernarg = Wave64KernargStorageV1::from_explicit(retained.explicit_kernarg_v1());
    if !(kernarg.bytes.as_ptr() as usize).is_multiple_of(RUNTIME_KERNARG_ALIGNMENT as usize) {
        drop(kernel);
        terminal_unload(&mut adapter, executable, &environment, &load);
        return Err(Wave64CollectivesLoadErrorV1::KernelObservation(
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

fn validate_join(
    admission: &PreparedFinalizedWave64CollectivesV1HsacoV1,
    host: &GeneratedWave64CollectivesV1HostAdapterV1<'_, '_, '_, '_>,
) -> Result<(), Wave64CollectivesJoinErrorV1> {
    // SAFETY: validation borrows without retaining or exposing the bytes.
    let bytes = unsafe { admission.exact_finalized_bytes_for_reviewed_wave64_runtime_v1() };
    if !admission.finalized_output_identity().matches(bytes) {
        return Err(Wave64CollectivesJoinErrorV1::FinalizedOutput);
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
            admission.exact_profile_descriptor_source_was_checked(),
            "descriptor admission",
        ),
        (
            admission.exact_five_argument_abi_was_checked(),
            "five-argument ABI",
        ),
        (
            admission.direct_upstream_llvm_worker_exchange_was_checked(),
            "worker exchange",
        ),
    ] {
        if !matches {
            return Err(Wave64CollectivesJoinErrorV1::ProfileField(field));
        }
    }
    for (matches, field) in [
        (host.target() == TARGET, "target"),
        (host.grid() == GRID, "grid"),
        (host.workgroup() == WORKGROUP, "workgroup"),
        (
            host.explicit_kernarg_byte_len() == EXPLICIT_KERNARG_BYTES,
            "explicit kernarg",
        ),
        (
            host.complete_kernarg_byte_len() == COMPLETE_KERNARG_BYTES,
            "complete kernarg",
        ),
        (
            host.descriptor_kernarg_alignment() == DESCRIPTOR_KERNARG_ALIGNMENT,
            "descriptor alignment",
        ),
        (
            host.static_lds_bytes() == GROUP_SEGMENT_BYTES,
            "group segment",
        ),
        (
            host.private_segment_bytes() == PRIVATE_SEGMENT_BYTES,
            "private segment",
        ),
    ] {
        if !matches {
            return Err(Wave64CollectivesJoinErrorV1::HostField(field));
        }
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

fn terminal_unload<A: ReviewedWave64CollectivesRuntimeAdapterV1>(
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

fn validate_kernel(
    load: &HsaCodeObjectLoadObservationV1,
    value: &HsaKernelResolutionObservationV1,
) -> Result<(), &'static str> {
    let expected = HsaKernelResolutionObservationV1::new(
        load.executable_object(),
        value.kernel_object(),
        EXPORT_SYMBOL,
        COMPLETE_KERNARG_BYTES as u64,
        RUNTIME_KERNARG_ALIGNMENT,
    )
    .expect("static kernel observation");
    if value == &expected {
        Ok(())
    } else {
        Err("object, symbol, size, or runtime alignment")
    }
}

fn validate_resources(
    load: &HsaCodeObjectLoadObservationV1,
    resolution: &HsaKernelResolutionObservationV1,
    value: Wave64CollectivesKernelResourceObservationV1,
) -> Result<(), &'static str> {
    let expected = Wave64CollectivesKernelResourceObservationV1::new(
        load.executable_object(),
        resolution.kernel_object(),
        GROUP_SEGMENT_BYTES,
        PRIVATE_SEGMENT_BYTES,
    );
    if value == expected {
        Ok(())
    } else {
        Err("object, group, or private segment")
    }
}

fn validate_implicit(
    load: &HsaCodeObjectLoadObservationV1,
    resolution: &HsaKernelResolutionObservationV1,
    geometry: HsaLaunchGeometryV1,
    value: &HsaImplicitKernargInitializationObservationV1,
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
    if value == &expected {
        Ok(())
    } else {
        Err("object, geometry, span, or completion")
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

fn exact_geometry() -> HsaLaunchGeometryV1 {
    HsaLaunchGeometryV1::new(GRID, WORKGROUP, DYNAMIC_LDS_BYTES)
}

fn unload_identity(
    unload: &HsaUnloadObservationV1,
    runtime: [u8; 16],
    agent: u64,
) -> Wave64CollectivesUnloadIdentityV1 {
    let mut digest = Sha256::new();
    for field in [
        UNLOAD_IDENTITY_DOMAIN_V1,
        unload.executable_object().as_bytes(),
        &runtime,
        &agent.to_le_bytes(),
        &[u8::from(unload.released())],
    ] {
        digest.update((field.len() as u64).to_le_bytes());
        digest.update(field);
    }
    Wave64CollectivesUnloadIdentityV1(digest.finalize().into())
}

const _: () = assert!(WAVEFRONT_SIZE == WORKGROUP[0]);
const _: () = assert!(IMPLICIT_KERNARG_BYTES == 256);
const _: () = assert!(COMPLETE_KERNARG_BYTES == 328);

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
        explicit: [u8; EXPLICIT_KERNARG_BYTES],
        drops: Arc<AtomicUsize>,
    }

    impl Drop for TestRetainedV1 {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::SeqCst);
        }
    }

    impl RetainedWave64CollectivesV1 for TestRetainedV1 {
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
        fn explicit_kernarg_v1(&self) -> &[u8; EXPLICIT_KERNARG_BYTES] {
            &self.explicit
        }
    }

    pub(crate) struct TestLoadedWave64V1<A: ReviewedWave64CollectivesRuntimeAdapterV1> {
        state: LoadedStateV1<TestRetainedV1, A>,
    }

    impl<A: ReviewedWave64CollectivesRuntimeAdapterV1> TestLoadedWave64V1<A> {
        pub(crate) fn dispatch_and_wait(
            self,
        ) -> Result<TestCompletedWave64V1<A>, Wave64CollectivesDispatchErrorV1<A::Error>> {
            self.state
                .dispatch_and_wait()
                .map(|state| TestCompletedWave64V1 {
                    state: state.release_retained(),
                })
        }
    }

    pub(crate) struct TestCompletedWave64V1<A: ReviewedWave64CollectivesRuntimeAdapterV1> {
        state: CompletedStateV1<A>,
    }

    impl<A: ReviewedWave64CollectivesRuntimeAdapterV1> TestCompletedWave64V1<A> {
        pub(crate) fn unload(mut self) -> TestUnloadedWave64V1 {
            let mut adapter = self.state.adapter.take().expect("test adapter");
            let executable = self.state.executable.take().expect("test executable");
            let unload = terminal_unload(
                &mut adapter,
                executable,
                &self.state.environment,
                &self.state.load,
            );
            let receipt = TestUnloadedWave64V1 {
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

    pub(crate) struct TestUnloadedWave64V1 {
        pub(crate) executable_object: HsaExecutableObjectIdentityV1,
        pub(crate) kernel_object: HsaKernelObjectIdentityV1,
        pub(crate) dispatch_identity: [u8; 16],
        pub(crate) unload_identity: Wave64CollectivesUnloadIdentityV1,
    }

    pub(crate) fn load_test_lifecycle_v1<A: ReviewedWave64CollectivesRuntimeAdapterV1>(
        adapter: A,
        context_matches: bool,
        drops: Arc<AtomicUsize>,
    ) -> Result<TestLoadedWave64V1<A>, Wave64CollectivesLoadErrorV1<A::Error>> {
        let bytes = vec![0x5a; 96];
        let retained = TestRetainedV1 {
            identity: ContentIdentityV1::calculate(&bytes),
            bytes,
            explicit: std::array::from_fn(|index| index as u8),
            drops,
        };
        if !context_matches {
            return Err(Wave64CollectivesLoadErrorV1::ContextIdentity);
        }
        load_after_context_match(retained, adapter).map(|state| TestLoadedWave64V1 { state })
    }

    pub(crate) const fn test_explicit_bytes_v1() -> usize {
        EXPLICIT_KERNARG_BYTES
    }
    pub(crate) const fn test_implicit_bytes_v1() -> usize {
        IMPLICIT_KERNARG_BYTES
    }
    pub(crate) const fn test_complete_bytes_v1() -> usize {
        COMPLETE_KERNARG_BYTES
    }
    pub(crate) const fn test_runtime_alignment_v1() -> u64 {
        RUNTIME_KERNARG_ALIGNMENT
    }
    pub(crate) const fn test_group_segment_v1() -> u32 {
        GROUP_SEGMENT_BYTES
    }
}
