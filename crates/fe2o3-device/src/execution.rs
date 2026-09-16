//! Generative workgroup source contracts.
//!
//! Rust brands prevent source-level scope substitution. They do not prove
//! collective participation, launch validity or producer authenticity. These
//! staged providers remain unadmitted by the production compiler.

#![forbid(unsafe_code)]

use core::marker::PhantomData;

use crate::context::{KernelCapabilityBrand, KernelContext, KernelLaunch, KernelTarget};

type Invariant<T> = fn(T) -> T;
type InvariantLifetime<'scope> = fn(&'scope mut ()) -> &'scope mut ();

mod sealed {
    pub trait Epoch {}
}

/// Initial synchronization epoch of a compiler-issued workgroup.
#[derive(Debug)]
pub enum InitialEpoch {}

/// Sealed synchronization-epoch identity.
pub trait SynchronizationEpoch: sealed::Epoch {}

impl sealed::Epoch for InitialEpoch {}
impl SynchronizationEpoch for InitialEpoch {}

/// Invariant identity for one generative workgroup within a kernel.
#[doc(hidden)]
pub struct WorkgroupBrand<'workgroup, KernelBrand> {
    _workgroup: PhantomData<InvariantLifetime<'workgroup>>,
    _kernel: PhantomData<Invariant<KernelBrand>>,
}

struct WorkgroupEpoch<'workgroup, KernelBrand, Epoch: SynchronizationEpoch> {
    _workgroup: PhantomData<Invariant<WorkgroupBrand<'workgroup, KernelBrand>>>,
    _epoch: PhantomData<Invariant<Epoch>>,
    _not_send_sync: PhantomData<*mut ()>,
}

/// Move-only authority associated with one workgroup and synchronization epoch.
///
/// The higher-ranked callback in [KernelContext::with_workgroup] supplies the
/// generative scope. No public constructor or epoch transition is provided.
/// Production rejects this staged provider until authenticated issuance and
/// capability transport are integrated.
#[must_use = "workgroup authority retains its scope and synchronization epoch"]
#[rustc_diagnostic_item = "fe2o3_device_workgroup_capability_v1"]
pub struct WorkgroupCapability<'workgroup, KernelBrand, Epoch: SynchronizationEpoch = InitialEpoch>
{
    _size: u64,
    _rank: u64,
    _epoch: WorkgroupEpoch<'workgroup, KernelBrand, Epoch>,
    _not_send_sync: PhantomData<*mut ()>,
}

impl<'kernel, Kernel, Target: KernelTarget, Launch: KernelLaunch>
    KernelContext<'kernel, Kernel, Target, Launch>
{
    /// Reserved provider terminal; no fallback issues workgroup authority.
    #[doc(hidden)]
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_workgroup_capability_current_v1"]
    fn __compiler_workgroup_capability_current<'workgroup>(
        &'workgroup mut self,
    ) -> WorkgroupCapability<
        'workgroup,
        KernelCapabilityBrand<'kernel, Kernel, Target, Launch>,
        InitialEpoch,
    > {
        unreachable!("workgroup capability issuance requires authenticated lowering")
    }

    /// Opens a generative scope for the current workgroup.
    ///
    /// Branded results cannot escape the callback. Ordinary computed values
    /// may be returned; borrowed inputs and outputs keep their Rust lifetimes.
    /// This provider stage always traps before invoking the callback and is
    /// explicitly rejected by production, not an executable host fallback.
    pub fn with_workgroup<Result>(
        &mut self,
        operation: impl for<'workgroup> FnOnce(
            WorkgroupCapability<
                'workgroup,
                KernelCapabilityBrand<'kernel, Kernel, Target, Launch>,
                InitialEpoch,
            >,
        ) -> Result,
    ) -> Result {
        operation(self.__compiler_workgroup_capability_current())
    }
}

const _: () = {
    assert!(core::mem::size_of::<WorkgroupEpoch<'static, (), InitialEpoch>>() == 0);
    assert!(core::mem::size_of::<WorkgroupCapability<'static, ()>>() == 16);
};
