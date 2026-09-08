//! Compiler-issued numerical-policy ownership.
//!
//! A policy capability is derived only from one authenticated kernel context.
//! It is an ownership token, not a claim that arbitrary source expressions
//! satisfy the policy. Production import and target legalization must retain and
//! discharge that obligation for every operation reached through a policy-bound
//! capability.

use core::marker::PhantomData;

use crate::{KernelCapabilityBrand, KernelContext, KernelLaunch, KernelTarget};

type Invariant<T> = fn(T) -> T;

/// Version of the source numerical-policy ownership contract.
pub const NUMERICAL_POLICY_CAPABILITY_CONTRACT_VERSION_V1: u16 = 1;

mod sealed {
    pub trait Policy {}
}

/// A closed numerical policy that can be owned by kernel source.
pub trait NumericalPolicy: sealed::Policy {}

/// Strict IEEE operator semantics for floating-point source expressions.
///
/// Every scalar operation rounds independently to nearest, ties to even. The
/// compiler may not reassociate or contract separate operations. Explicitly
/// fused or matrix operations retain their operation-defined evaluation order;
/// their exact target implementation must be legalized separately.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[rustc_diagnostic_item = "fe2o3_device_strict_ieee_numerical_policy_v1"]
pub enum StrictIeee {}

impl sealed::Policy for StrictIeee {}
impl NumericalPolicy for StrictIeee {}

/// Numerical-policy authority belonging to one exact kernel capability brand.
///
/// Fields and construction remain private. The value is invariant in its
/// kernel brand and policy and is neither `Copy`, `Clone`, `Send`, nor `Sync`.
#[must_use = "numerical-policy authority must remain attached to admitted operations"]
#[rustc_diagnostic_item = "fe2o3_device_numerical_policy_capability_v1"]
pub struct NumericalPolicyCapability<KernelBrand, Policy: NumericalPolicy> {
    _kernel: PhantomData<Invariant<KernelBrand>>,
    _policy: PhantomData<Invariant<Policy>>,
    _not_send_sync: PhantomData<*mut ()>,
}

#[cfg(test)]
impl<KernelBrand, Policy: NumericalPolicy> NumericalPolicyCapability<KernelBrand, Policy> {
    pub(crate) fn for_host_test() -> Self {
        Self {
            _kernel: PhantomData,
            _policy: PhantomData,
            _not_send_sync: PhantomData,
        }
    }
}

impl<KernelBrand, Policy: NumericalPolicy> core::fmt::Debug
    for NumericalPolicyCapability<KernelBrand, Policy>
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("NumericalPolicyCapability")
            .finish_non_exhaustive()
    }
}

impl<'kernel, Kernel, Target, Launch> KernelContext<'kernel, Kernel, Target, Launch>
where
    Target: KernelTarget,
    Launch: KernelLaunch,
{
    /// Issues one exact numerical policy for this authenticated kernel root.
    ///
    /// The production importer must recognize this terminal and carry `Policy`
    /// into canonical KIR, proof inputs, target legalization, and artifact
    /// evidence. Its fallback body never manufactures policy authority.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_numerical_policy_issue_v1"]
    pub fn numerical_policy<Policy: NumericalPolicy>(
        &self,
    ) -> NumericalPolicyCapability<KernelCapabilityBrand<'kernel, Kernel, Target, Launch>, Policy>
    {
        unreachable!("numerical-policy issuance requires authenticated lowering")
    }
}

const _: () = {
    assert!(core::mem::size_of::<NumericalPolicyCapability<(), StrictIeee>>() == 0);
    assert!(core::mem::align_of::<NumericalPolicyCapability<(), StrictIeee>>() == 1);
};

#[cfg(test)]
mod tests {
    use super::{
        NUMERICAL_POLICY_CAPABILITY_CONTRACT_VERSION_V1, NumericalPolicyCapability, StrictIeee,
    };

    #[test]
    fn policy_contract_and_representation_are_exact() {
        assert_eq!(NUMERICAL_POLICY_CAPABILITY_CONTRACT_VERSION_V1, 1);
        assert_eq!(
            core::mem::size_of::<NumericalPolicyCapability<(), StrictIeee>>(),
            0
        );
        assert_eq!(
            core::mem::align_of::<NumericalPolicyCapability<(), StrictIeee>>(),
            1
        );
    }
}
