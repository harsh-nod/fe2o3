//! Compiler-issued execution capabilities for one kernel invocation.

use core::marker::PhantomData;

use crate::{
    DeviceMath, Grid, Invocation3D, MatrixCapability, SubgroupLane, SubgroupTile, SubgroupWidth,
    ValidWave64TileWidth, Wave64, Wave64TileWidth, WaveLane, WaveWidth, Workgroup,
    WorkgroupCollectives, WorkgroupLdsScope,
};

mod sealed {
    pub trait KernelContext {}
    pub trait KernelTarget {}
    pub trait KernelLaunch {}
}

/// Target selected and authenticated by the production compiler transaction.
///
/// This marker deliberately exposes no architecture-specific fact. Operations
/// that need such a fact remain separately capability-gated and are accepted
/// only after exact target binding.
#[derive(Debug)]
pub enum CurrentTarget {}

impl sealed::KernelTarget for CurrentTarget {}

/// Launch contract attached to the enclosing `#[kernel]` registration.
///
/// Its dimensions and dynamic-resource limits live in compiler metadata and
/// canonical KIR rather than in a caller-constructed source value.
#[derive(Debug)]
pub enum RegisteredLaunch {}

impl sealed::KernelLaunch for RegisteredLaunch {}

/// Sealed marker for a compiler-authenticated kernel target.
pub trait KernelTarget: sealed::KernelTarget {}

impl KernelTarget for CurrentTarget {}

/// Sealed marker for a compiler-authenticated kernel launch contract.
pub trait KernelLaunch: sealed::KernelLaunch {}

impl KernelLaunch for RegisteredLaunch {}

/// Placeholder used only before `#[kernel]` binds a unique nominal kernel type.
#[doc(hidden)]
#[derive(Debug)]
pub enum UnboundKernel {}

/// Brand used by legacy snapshot APIs that carry no compiler authority.
#[doc(hidden)]
#[derive(Debug)]
pub enum UnbrandedCapability {}

/// Invariant identity shared by capabilities issued from one kernel context.
///
/// The type is public only because it appears in context-derived handle types.
/// Its fields are private, so downstream code can name a brand but cannot
/// issue branded authority.
#[doc(hidden)]
pub struct KernelCapabilityBrand<'kernel, Kernel, Target, Launch> {
    _kernel: PhantomData<&'kernel mut &'kernel ()>,
    _kernel_identity: PhantomData<fn(Kernel) -> Kernel>,
    _target: PhantomData<fn(Target) -> Target>,
    _launch: PhantomData<fn(Launch) -> Launch>,
}

/// Root authority for execution state belonging to one kernel invocation.
///
/// `KernelContext` is a logical source argument. It is zero-sized and has an
/// ignored Rust ABI pass mode, so it contributes no caller-supplied kernarg
/// bytes. The production compiler authenticates the type at the kernel root and
/// rederives every observation through reviewed device intrinsics.
///
/// Safe code cannot construct this value. It is deliberately neither `Copy`,
/// `Clone`, `Send`, nor `Sync`; every derived handle remains branded by
/// `'kernel` and cannot be substituted across kernel scopes.
#[must_use = "the compiler-issued kernel context owns this invocation's capability root"]
#[rustc_diagnostic_item = "fe2o3_device_kernel_context_v1"]
pub struct KernelContext<
    'kernel,
    Kernel = UnboundKernel,
    Target: KernelTarget = CurrentTarget,
    Launch: KernelLaunch = RegisteredLaunch,
> {
    _kernel: PhantomData<&'kernel mut &'kernel ()>,
    _kernel_identity: PhantomData<fn(Kernel) -> Kernel>,
    _target: PhantomData<fn(Target) -> Target>,
    _launch: PhantomData<fn(Launch) -> Launch>,
    _not_send_sync: PhantomData<*mut ()>,
}

impl<'kernel, Kernel, Target: KernelTarget, Launch: KernelLaunch> sealed::KernelContext
    for KernelContext<'kernel, Kernel, Target, Launch>
{
}

/// Sealed source-type identity for the genuine compiler-issued context.
///
/// Macro-generated code uses this trait to reject downstream lookalike types.
/// Only this crate can implement its private sealing supertrait.
///
/// # Safety
///
/// Implementors must be the exact zero-sized logical context recognized by
/// the production compiler and must not add caller-controlled state.
#[doc(hidden)]
pub unsafe trait KernelContextTypeV1: sealed::KernelContext {}

// SAFETY: this is the sole compiler-recognized logical context definition.
unsafe impl<'kernel, Kernel, Target: KernelTarget, Launch: KernelLaunch> KernelContextTypeV1
    for KernelContext<'kernel, Kernel, Target, Launch>
{
}

impl<'kernel, Kernel, Target: KernelTarget, Launch: KernelLaunch>
    KernelContext<'kernel, Kernel, Target, Launch>
{
    /// Issues the logical context at an authenticated kernel entry.
    ///
    /// # Safety
    ///
    /// Only the `#[kernel]` entry shim emitted by the matching fe2o3 compiler may
    /// call this function. The production importer authenticates this exact
    /// diagnostic item and expands it as a semantic terminal; the fallback body
    /// never fabricates a context value.
    #[doc(hidden)]
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_kernel_context_issue_v1"]
    pub unsafe fn __compiler_issue() -> Self {
        unreachable!("KernelContext must be issued by the authenticated fe2o3 compiler")
    }

    /// Observes the current invocation's complete three-dimensional geometry.
    ///
    /// The returned value can derive branded index witnesses for memory views
    /// issued by this same context.
    #[inline(always)]
    pub fn invocation(
        &self,
    ) -> Invocation3D<KernelCapabilityBrand<'kernel, Kernel, Target, Launch>> {
        Invocation3D::current_branded()
    }

    /// Derives the current invocation's checked linear grid rank and extent.
    #[inline(always)]
    pub fn grid(
        &self,
    ) -> Option<Grid<'kernel, KernelCapabilityBrand<'kernel, Kernel, Target, Launch>>> {
        Grid::current_branded()
    }

    /// Derives the current invocation's workgroup rank and extent.
    #[inline(always)]
    pub fn workgroup(
        &self,
    ) -> Workgroup<'kernel, KernelCapabilityBrand<'kernel, Kernel, Target, Launch>> {
        Workgroup::current_branded()
    }

    /// Derives the current lane observation for a statically selected width.
    ///
    /// Exact target legalization must reject an unsupported width.
    #[inline(always)]
    pub fn subgroup_lane<Width: SubgroupWidth>(
        &self,
    ) -> SubgroupLane<Width, KernelCapabilityBrand<'kernel, Kernel, Target, Launch>> {
        SubgroupLane::current_branded()
    }

    /// AMD wave-lane compatibility spelling.
    #[deprecated(note = "use subgroup_lane or with_workgroup(...).subgroup()")]
    #[inline(always)]
    #[allow(deprecated)]
    pub fn lane<Width: WaveWidth>(
        &self,
    ) -> WaveLane<Width, KernelCapabilityBrand<'kernel, Kernel, Target, Launch>> {
        self.subgroup_lane()
    }

    /// Derives one power-of-two AMD wave64 tile.
    ///
    /// This is an explicit target-specific compatibility operation, not the
    /// portable subgroup API. Exact target legalization must reject it when a
    /// wave64 execution mode is unavailable.
    #[inline(always)]
    pub fn wave64_tile<const N: u32>(
        &self,
    ) -> SubgroupTile<'kernel, N, KernelCapabilityBrand<'kernel, Kernel, Target, Launch>>
    where
        Wave64TileWidth<N>: ValidWave64TileWidth,
    {
        SubgroupTile::current_branded()
    }

    /// Borrows the root exclusively to issue one workgroup LDS allocation scope.
    #[inline(always)]
    #[deprecated(note = "use with_workgroup for branded, epoch-aware workgroup memory")]
    pub fn workgroup_lds(
        &mut self,
    ) -> WorkgroupLdsScope<'_, KernelCapabilityBrand<'kernel, Kernel, Target, Launch>> {
        WorkgroupLdsScope::current_branded()
    }

    /// Derives the target-neutral workgroup collective capability.
    #[inline(always)]
    #[deprecated(note = "use with_workgroup for branded, epoch-aware collectives")]
    pub fn workgroup_collectives(
        &self,
    ) -> WorkgroupCollectives<KernelCapabilityBrand<'kernel, Kernel, Target, Launch>> {
        WorkgroupCollectives::current_branded()
    }

    /// Derives the target-neutral device math capability.
    #[inline(always)]
    pub fn math(&self) -> DeviceMath<KernelCapabilityBrand<'kernel, Kernel, Target, Launch>> {
        DeviceMath::current_branded()
    }

    /// Derives the target-neutral matrix capability.
    #[inline(always)]
    #[deprecated(
        note = "use Subgroup::with_matrix so matrix access carries subgroup width and epoch"
    )]
    pub fn matrix(
        &self,
    ) -> MatrixCapability<KernelCapabilityBrand<'kernel, Kernel, Target, Launch>> {
        MatrixCapability::current_branded()
    }
}

const _: () = assert!(core::mem::size_of::<KernelContext<'static>>() == 0);
const _: () = assert!(core::mem::align_of::<KernelContext<'static>>() == 1);

// Keep the concrete width in this module's semantic closure. The subgroup
// helper is intentionally wave64-specific until the neutral subgroup contract
// carries a target-provided width.
type LegacyWave64Lane = WaveLane<
    Wave64,
    KernelCapabilityBrand<'static, UnboundKernel, CurrentTarget, RegisteredLaunch>,
>;
const _: fn(&KernelContext<'static>) -> LegacyWave64Lane = KernelContext::lane::<Wave64>;

#[cfg(test)]
mod tests {
    use super::KernelContext;

    #[test]
    fn compiler_context_issuance_fails_closed_on_host() {
        type Context = KernelContext<'static>;
        assert!(std::panic::catch_unwind(|| unsafe { Context::__compiler_issue() }).is_err());
    }
}
