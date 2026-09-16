//! Source contract for compiler-issued kernel identity.
//!
//! This provider is staged for the unified capability importer. Production
//! currently rejects these nominal types. The reserved issuer always traps
//! outside authenticated lowering and adds no physical kernel argument.

#![forbid(unsafe_code)]

use core::marker::PhantomData;

mod sealed {
    pub trait Target {}
    pub trait Launch {}
}

/// Target identity to be bound by the production compiler.
#[derive(Debug)]
pub enum CurrentTarget {}

/// Launch identity to be bound to the enclosing kernel registration.
#[derive(Debug)]
pub enum RegisteredLaunch {}

/// Sealed identity for a compiler-authenticated target.
pub trait KernelTarget: sealed::Target {}

/// Sealed identity for a compiler-authenticated launch contract.
pub trait KernelLaunch: sealed::Launch {}

impl sealed::Target for CurrentTarget {}
impl KernelTarget for CurrentTarget {}
impl sealed::Launch for RegisteredLaunch {}
impl KernelLaunch for RegisteredLaunch {}

/// Placeholder before the compiler binds an exact nominal kernel identity.
#[doc(hidden)]
#[derive(Debug)]
pub enum UnboundKernel {}

/// Invariant association with one logical kernel, target and launch.
#[doc(hidden)]
pub struct KernelCapabilityBrand<'kernel, Kernel, Target, Launch> {
    _kernel: PhantomData<&'kernel mut &'kernel ()>,
    _identity: PhantomData<fn(Kernel) -> Kernel>,
    _target: PhantomData<fn(Target) -> Target>,
    _launch: PhantomData<fn(Launch) -> Launch>,
}

/// Opaque logical root for one kernel's execution capabilities.
///
/// This source-provider type is not yet admitted by the production importer.
/// Its reserved issuance terminal always traps without authenticated lowering;
/// it has no public constructor or caller-supplied payload.
/// It is invariant in its lifetime and brands, and is neither copyable nor
/// transferable between host threads. Its zero size is not issuance evidence.
#[must_use = "the kernel context owns the logical capability root"]
#[rustc_diagnostic_item = "fe2o3_device_kernel_context_v1"]
pub struct KernelContext<
    'kernel,
    Kernel = UnboundKernel,
    Target: KernelTarget = CurrentTarget,
    Launch: KernelLaunch = RegisteredLaunch,
> {
    _kernel: PhantomData<&'kernel mut &'kernel ()>,
    _identity: PhantomData<fn(Kernel) -> Kernel>,
    _target: PhantomData<fn(Target) -> Target>,
    _launch: PhantomData<fn(Launch) -> Launch>,
    _not_send_sync: PhantomData<*mut ()>,
}

impl<'kernel, Kernel, Target: KernelTarget, Launch: KernelLaunch>
    KernelContext<'kernel, Kernel, Target, Launch>
{
    /// Reserved source terminal for an authenticated physical kernel root.
    ///
    /// Calling this function directly cannot create a context. The collector
    /// must authenticate its occurrence and result before any future lowering.
    #[doc(hidden)]
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_kernel_context_issue_v1"]
    pub fn __compiler_issue() -> Self {
        unreachable!("kernel context issuance requires authenticated lowering")
    }
}

const _: () = {
    assert!(core::mem::size_of::<KernelContext<'static>>() == 0);
    assert!(core::mem::align_of::<KernelContext<'static>>() == 1);
};

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;

    #[test]
    fn root_issuer_cannot_construct_a_context_on_host() {
        let result = std::panic::catch_unwind(|| -> KernelContext<'static> {
            KernelContext::__compiler_issue()
        });
        assert!(result.is_err());
    }

    #[test]
    fn workgroup_provider_never_runs_the_callback_on_host() {
        let mut context: KernelContext<'_> = KernelContext {
            _kernel: PhantomData,
            _identity: PhantomData,
            _target: PhantomData,
            _launch: PhantomData,
            _not_send_sync: PhantomData,
        };
        let called = Cell::new(false);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            context.with_workgroup(|_| called.set(true));
        }));
        assert!(result.is_err());
        assert!(!called.get());
    }
}
