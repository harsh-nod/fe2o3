//! Kernel-branded access to the existing matrix instruction capability.

use core::marker::PhantomData;
use core::ops::Deref;

use crate::SubgroupWidth;
use crate::context::{KernelCapabilityBrand, KernelLaunch, KernelTarget, UnbrandedCapability};
use crate::execution::{ReusableWorkgroupBrand, SubgroupBrand, SynchronizationEpoch};
use crate::numerical::{NumericalPolicy, NumericalPolicyCapability};
use crate::tensor::DeviceMatrix;

type Invariant<T> = fn(T) -> T;

mod sealed {
    pub trait GlobalAccess<GlobalBrand> {}
}

/// Sealed proof that a matrix capability belongs to one global-memory brand.
///
/// The relation permits a root matrix compatibility capability and subgroups
/// derived from that root, including a reusable workgroup phase. Downstream
/// code cannot implement it to relabel unrelated allocation authority.
pub trait MatrixGlobalAccess<GlobalBrand>: sealed::GlobalAccess<GlobalBrand> {}

impl<'kernel, Kernel, Target, Launch>
    sealed::GlobalAccess<KernelCapabilityBrand<'kernel, Kernel, Target, Launch>>
    for KernelCapabilityBrand<'kernel, Kernel, Target, Launch>
where
    Target: KernelTarget,
    Launch: KernelLaunch,
{
}

impl<'kernel, Kernel, Target, Launch>
    MatrixGlobalAccess<KernelCapabilityBrand<'kernel, Kernel, Target, Launch>>
    for KernelCapabilityBrand<'kernel, Kernel, Target, Launch>
where
    Target: KernelTarget,
    Launch: KernelLaunch,
{
}

impl<'workgroup, Width, KernelBrand, Epoch, GlobalBrand> sealed::GlobalAccess<GlobalBrand>
    for SubgroupBrand<'workgroup, Width, KernelBrand, Epoch>
where
    Width: SubgroupWidth,
    Epoch: SynchronizationEpoch,
    KernelBrand: sealed::GlobalAccess<GlobalBrand>,
{
}

impl<'workgroup, Width, KernelBrand, Epoch, GlobalBrand> MatrixGlobalAccess<GlobalBrand>
    for SubgroupBrand<'workgroup, Width, KernelBrand, Epoch>
where
    Width: SubgroupWidth,
    Epoch: SynchronizationEpoch,
    KernelBrand: sealed::GlobalAccess<GlobalBrand>,
{
}

impl<'workgroup, KernelBrand, GlobalBrand> sealed::GlobalAccess<GlobalBrand>
    for ReusableWorkgroupBrand<'workgroup, KernelBrand>
where
    KernelBrand: sealed::GlobalAccess<GlobalBrand>,
{
}

impl<'workgroup, KernelBrand, GlobalBrand> MatrixGlobalAccess<GlobalBrand>
    for ReusableWorkgroupBrand<'workgroup, KernelBrand>
where
    KernelBrand: sealed::GlobalAccess<GlobalBrand>,
{
}

/// Matrix authority carrying the kernel context's invariant execution brand.
///
/// `DeviceMatrix` remains available as an unbranded compatibility API. New
/// kernels derive this wrapper from [`crate::KernelContext`]; dereferencing it
/// exposes the established matrix operation surface without discarding the
/// brand from the owned handle.
pub struct MatrixCapability<Brand = UnbrandedCapability> {
    inner: DeviceMatrix,
    _brand: PhantomData<fn(Brand) -> Brand>,
    _not_send_sync: PhantomData<*mut ()>,
}

/// Matrix authority paired with an exact compiler-issued numerical policy.
///
/// This borrow cannot outlive either input capability. Its fields are private
/// and it is neither `Copy`, `Clone`, `Send`, nor `Sync`.
#[must_use = "matrix operations must retain their numerical-policy authority"]
#[rustc_diagnostic_item = "fe2o3_device_policy_matrix_capability_v1"]
pub struct PolicyMatrixCapability<'capability, MatrixBrand, GlobalBrand, Policy>
where
    Policy: NumericalPolicy,
    MatrixBrand: MatrixGlobalAccess<GlobalBrand>,
{
    matrix: &'capability MatrixCapability<MatrixBrand>,
    _policy: &'capability NumericalPolicyCapability<GlobalBrand, Policy>,
    _brands: PhantomData<Invariant<(MatrixBrand, GlobalBrand, Policy)>>,
    _not_send_sync: PhantomData<*mut ()>,
}

impl<Brand> MatrixCapability<Brand> {
    pub(crate) fn current_branded() -> Self {
        Self {
            inner: DeviceMatrix::current(),
            _brand: PhantomData,
            _not_send_sync: PhantomData,
        }
    }

    #[cfg(test)]
    pub(crate) fn for_host_test() -> Self {
        Self {
            inner: DeviceMatrix::for_host_test(),
            _brand: PhantomData,
            _not_send_sync: PhantomData,
        }
    }

    /// Pairs this matrix authority with its kernel root's numerical policy.
    ///
    /// The sealed brand relation rejects a policy from another kernel and does
    /// not permit a subgroup or reusable phase to shed its execution identity.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_policy_matrix_bind_v1"]
    pub fn with_numerical_policy<'capability, GlobalBrand, Policy>(
        &'capability self,
        policy: &'capability NumericalPolicyCapability<GlobalBrand, Policy>,
    ) -> PolicyMatrixCapability<'capability, Brand, GlobalBrand, Policy>
    where
        Policy: NumericalPolicy,
        Brand: MatrixGlobalAccess<GlobalBrand>,
    {
        PolicyMatrixCapability {
            matrix: self,
            _policy: policy,
            _brands: PhantomData,
            _not_send_sync: PhantomData,
        }
    }
}

#[cfg(test)]
impl sealed::GlobalAccess<UnbrandedCapability> for UnbrandedCapability {}
#[cfg(test)]
impl MatrixGlobalAccess<UnbrandedCapability> for UnbrandedCapability {}

impl<'capability, MatrixBrand, GlobalBrand, Policy>
    PolicyMatrixCapability<'capability, MatrixBrand, GlobalBrand, Policy>
where
    Policy: NumericalPolicy,
    MatrixBrand: MatrixGlobalAccess<GlobalBrand>,
{
    pub(crate) const fn matrix(&self) -> &MatrixCapability<MatrixBrand> {
        self.matrix
    }
}

impl<Brand> Deref for MatrixCapability<Brand> {
    type Target = DeviceMatrix;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<Brand> core::fmt::Debug for MatrixCapability<Brand> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("MatrixCapability")
            .finish_non_exhaustive()
    }
}

impl<'capability, MatrixBrand, GlobalBrand, Policy> core::fmt::Debug
    for PolicyMatrixCapability<'capability, MatrixBrand, GlobalBrand, Policy>
where
    Policy: NumericalPolicy,
    MatrixBrand: MatrixGlobalAccess<GlobalBrand>,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PolicyMatrixCapability")
            .finish_non_exhaustive()
    }
}
