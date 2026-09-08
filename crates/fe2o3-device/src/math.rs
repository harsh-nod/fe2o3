//! Target-gated device math intrinsics.
//!
//! Every admitted operation requires a compiler-recognized [`DeviceMath`]
//! capability paired with an exact numerical policy. Compatibility operations
//! on bare `DeviceMath` are unsafe, deprecated, and carry no diagnostic item;
//! production import can therefore admit only [`PolicyDeviceMath`] terminals.
//!
//! The current backend does not perform that lowering yet. Keeping these
//! stubs inert is deliberate: host libm results are not evidence of OCML or
//! AMDGPU intrinsic behavior.

use core::marker::PhantomData;

use crate::Bf16x2;
use crate::context::UnbrandedCapability;
use crate::numerical::{NumericalPolicy, NumericalPolicyCapability};

/// Version of the device math semantic contract.
pub const DEVICE_MATH_CONTRACT_VERSION_V1: u16 = 1;

/// Compiler-created authority to call target-specific device math operations.
///
/// The value is neither `Copy`, `Clone`, `Send`, nor `Sync`. It carries no
/// memory, launch, synchronization, or cross-invocation authority. Safe
/// acquisition is sound because an unsupported call fails closed; target and
/// floating-point-policy validation remains a compiler obligation.
#[rustc_diagnostic_item = "fe2o3_device_math_context_v1"]
pub struct DeviceMath<Brand = UnbrandedCapability> {
    _private: (),
    _brand: PhantomData<fn(Brand) -> Brand>,
    _not_send_sync: PhantomData<*mut ()>,
}

/// Device-math authority paired with one exact kernel numerical policy.
///
/// The wrapper exposes policy-sensitive operations without erasing the kernel
/// brand. It is neither `Copy`, `Clone`, `Send`, nor `Sync`.
#[must_use = "policy-bound device math must retain its numerical authority"]
#[rustc_diagnostic_item = "fe2o3_device_policy_math_capability_v1"]
pub struct PolicyDeviceMath<'capability, Brand, Policy: NumericalPolicy> {
    math: &'capability DeviceMath<Brand>,
    _policy: &'capability NumericalPolicyCapability<Brand, Policy>,
    _not_send_sync: PhantomData<*mut ()>,
}

macro_rules! device_unary_intrinsic {
    ($(#[$meta:meta])* $name:ident, $llvm:literal) => {
        $(#[$meta])*
        #[must_use]
        #[inline(never)]
        #[deprecated(note = "use PolicyDeviceMath; bare math has no admitted numerical policy")]
        pub unsafe fn $name(&self, value: f32) -> f32 {
            let _ = (self, value);
            unreachable!(concat!(
                "DeviceMath::",
                stringify!($name),
                " must be lowered to ",
                $llvm,
                " for an authenticated AMDGPU target"
            ))
        }
    };
}

macro_rules! device_ternary_intrinsic {
    ($(#[$meta:meta])* $name:ident, $llvm:literal) => {
        $(#[$meta])*
        #[must_use]
        #[inline(never)]
        #[deprecated(note = "use PolicyDeviceMath; bare math has no admitted numerical policy")]
        pub unsafe fn $name(&self, value: f32, multiplier: f32, addend: f32) -> f32 {
            let _ = (self, value, multiplier, addend);
            unreachable!(concat!(
                "DeviceMath::",
                stringify!($name),
                " must be lowered to ",
                $llvm,
                " for an authenticated AMDGPU target"
            ))
        }
    };
}

macro_rules! policy_unary_intrinsic {
    ($(#[$meta:meta])* $name:ident, $diagnostic:literal) => {
        $(#[$meta])*
        #[must_use]
        #[inline(never)]
        #[rustc_diagnostic_item = $diagnostic]
        pub fn $name(&self, value: f32) -> f32 {
            // SAFETY: this wrapper retains the matching compiler-issued policy;
            // the bare operation is not independently admissible.
            unsafe { self.math.$name(value) }
        }
    };
}

macro_rules! policy_ternary_intrinsic {
    ($(#[$meta:meta])* $name:ident, $diagnostic:literal) => {
        $(#[$meta])*
        #[must_use]
        #[inline(never)]
        #[rustc_diagnostic_item = $diagnostic]
        pub fn $name(&self, value: f32, multiplier: f32, addend: f32) -> f32 {
            // SAFETY: this wrapper retains the matching compiler-issued policy;
            // the bare operation is not independently admissible.
            unsafe { self.math.$name(value, multiplier, addend) }
        }
    };
}

impl DeviceMath {
    /// Acquires the unbranded compatibility math capability.
    ///
    /// The fallback always panics. The backend may replace it only after
    /// validating the AMDGPU target, code-object version, denormal mode,
    /// contraction policy, and linked OCML/OCKL identity required by every
    /// reachable operation. Making acquisition safe does not relax any of
    /// those checks and grants no memory or collective-execution authority.
    /// The result carries no nominal kernel, target, or launch brand; new
    /// kernels should use [`crate::KernelContext::math`].
    #[must_use]
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_math_context_from_compiler_v1"]
    pub fn current() -> Self {
        unreachable!("DeviceMath must be created by authenticated fe2o3 device lowering")
    }

    #[cfg(test)]
    fn for_host_test() -> Self {
        Self {
            _private: (),
            _brand: PhantomData,
            _not_send_sync: PhantomData,
        }
    }
}

impl<Brand> DeviceMath<Brand> {
    pub(crate) fn current_branded() -> Self {
        let _ = DeviceMath::current();
        Self {
            _private: (),
            _brand: PhantomData,
            _not_send_sync: PhantomData,
        }
    }

    /// Pairs this math authority with the same kernel brand's exact policy.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_policy_math_bind_v1"]
    pub fn with_numerical_policy<'capability, Policy: NumericalPolicy>(
        &'capability self,
        policy: &'capability NumericalPolicyCapability<Brand, Policy>,
    ) -> PolicyDeviceMath<'capability, Brand, Policy> {
        PolicyDeviceMath {
            math: self,
            _policy: policy,
            _not_send_sync: PhantomData,
        }
    }

    device_unary_intrinsic!(
        /// Computes square root with the authenticated device floating-point policy.
        sqrt_f32,
        "llvm.sqrt.f32"
    );
    device_ternary_intrinsic!(
        /// Computes one fused `value * multiplier + addend` operation.
        mul_add_f32,
        "llvm.fma.f32"
    );
    device_unary_intrinsic!(
        /// Rounds toward negative infinity.
        floor_f32,
        "llvm.floor.f32"
    );
    device_unary_intrinsic!(
        /// Rounds toward positive infinity.
        ceil_f32,
        "llvm.ceil.f32"
    );
    device_unary_intrinsic!(
        /// Rounds toward zero.
        trunc_f32,
        "llvm.trunc.f32"
    );
    device_unary_intrinsic!(
        /// Rounds to the nearest integer, with halfway cases rounded to even.
        round_ties_even_f32,
        "llvm.roundeven.f32"
    );
    device_unary_intrinsic!(
        /// Computes sine in radians.
        sin_f32,
        "llvm.sin.f32"
    );
    device_unary_intrinsic!(
        /// Computes cosine in radians.
        cos_f32,
        "llvm.cos.f32"
    );
    device_unary_intrinsic!(
        /// Computes `e^value`.
        exp_f32,
        "llvm.exp.f32"
    );
    device_unary_intrinsic!(
        /// Computes `2^value`.
        exp2_f32,
        "llvm.exp2.f32"
    );
    device_unary_intrinsic!(
        /// Computes the natural logarithm.
        ln_f32,
        "llvm.log.f32"
    );
    device_unary_intrinsic!(
        /// Computes the base-two logarithm.
        log2_f32,
        "llvm.log2.f32"
    );
    device_unary_intrinsic!(
        /// Computes the base-ten logarithm.
        log10_f32,
        "llvm.log10.f32"
    );

    /// Computes lane-wise packed bfloat16 fused multiply-add.
    ///
    /// On a target with native packed BF16 FMA this may select that operation.
    /// On gfx942 the accepted equivalent is two `f32` FMAs followed by
    /// round-to-nearest, ties-to-even bfloat16 packing, matching
    /// [`Bf16x2::mul_add_widened`]. The backend must reject any target for
    /// which it cannot prove one of those implementations and its denormal/NaN
    /// policy.
    #[must_use]
    #[inline(never)]
    #[deprecated(note = "use PolicyDeviceMath; bare math has no admitted numerical policy")]
    pub unsafe fn mul_add_bf16x2(
        &self,
        value: Bf16x2,
        multiplier: Bf16x2,
        addend: Bf16x2,
    ) -> Bf16x2 {
        let _ = (self, value, multiplier, addend);
        unreachable!(
            "DeviceMath::mul_add_bf16x2 must be lowered for an authenticated AMDGPU target"
        )
    }
}

impl<Brand, Policy: NumericalPolicy> PolicyDeviceMath<'_, Brand, Policy> {
    policy_unary_intrinsic!(
        /// Computes square root under the owned policy.
        sqrt_f32,
        "fe2o3_device_math_sqrt_f32_v1"
    );
    policy_ternary_intrinsic!(
        /// Computes one explicitly fused FP32 multiply-add under the owned policy.
        mul_add_f32,
        "fe2o3_device_math_fma_f32_v1"
    );
    policy_unary_intrinsic!(
        /// Rounds toward negative infinity under the owned policy.
        floor_f32,
        "fe2o3_device_math_floor_f32_v1"
    );
    policy_unary_intrinsic!(
        /// Rounds toward positive infinity under the owned policy.
        ceil_f32,
        "fe2o3_device_math_ceil_f32_v1"
    );
    policy_unary_intrinsic!(
        /// Rounds toward zero under the owned policy.
        trunc_f32,
        "fe2o3_device_math_trunc_f32_v1"
    );
    policy_unary_intrinsic!(
        /// Rounds to nearest, ties to even, under the owned policy.
        round_ties_even_f32,
        "fe2o3_device_math_roundeven_f32_v1"
    );
    policy_unary_intrinsic!(
        /// Computes sine under the owned policy.
        sin_f32,
        "fe2o3_device_math_sin_f32_v1"
    );
    policy_unary_intrinsic!(
        /// Computes cosine under the owned policy.
        cos_f32,
        "fe2o3_device_math_cos_f32_v1"
    );
    policy_unary_intrinsic!(
        /// Computes `e^value` under the owned policy.
        exp_f32,
        "fe2o3_device_math_exp_f32_v1"
    );
    policy_unary_intrinsic!(
        /// Computes `2^value` under the owned policy.
        exp2_f32,
        "fe2o3_device_math_exp2_f32_v1"
    );
    policy_unary_intrinsic!(
        /// Computes the natural logarithm under the owned policy.
        ln_f32,
        "fe2o3_device_math_log_f32_v1"
    );
    policy_unary_intrinsic!(
        /// Computes the base-two logarithm under the owned policy.
        log2_f32,
        "fe2o3_device_math_log2_f32_v1"
    );
    policy_unary_intrinsic!(
        /// Computes the base-ten logarithm under the owned policy.
        log10_f32,
        "fe2o3_device_math_log10_f32_v1"
    );

    /// Computes packed BF16 fused multiply-add under the owned policy.
    #[must_use]
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_math_fma_bf16x2_v1"]
    pub fn mul_add_bf16x2(&self, value: Bf16x2, multiplier: Bf16x2, addend: Bf16x2) -> Bf16x2 {
        // SAFETY: this wrapper retains the matching compiler-issued policy.
        unsafe { self.math.mul_add_bf16x2(value, multiplier, addend) }
    }
}

impl<Brand, Policy: NumericalPolicy> core::fmt::Debug for PolicyDeviceMath<'_, Brand, Policy> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PolicyDeviceMath")
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::{DEVICE_MATH_CONTRACT_VERSION_V1, DeviceMath};
    use crate::{Bf16, Bf16x2, NumericalPolicyCapability, StrictIeee, UnbrandedCapability};
    use std::panic::{AssertUnwindSafe, catch_unwind};

    #[test]
    fn contract_version_is_stable() {
        assert_eq!(DEVICE_MATH_CONTRACT_VERSION_V1, 1);
    }

    #[test]
    fn safe_acquisition_fails_closed_on_host() {
        let result = catch_unwind(DeviceMath::current);
        assert!(result.is_err());
    }

    #[test]
    fn scalar_stubs_fail_closed_on_host() {
        let math = DeviceMath::for_host_test();
        let policy = NumericalPolicyCapability::<UnbrandedCapability, StrictIeee>::for_host_test();
        let math = math.with_numerical_policy(&policy);
        macro_rules! assert_traps {
            ($($operation:ident),+ $(,)?) => {
                $(assert!(catch_unwind(AssertUnwindSafe(|| math.$operation(1.25))).is_err());)+
            };
        }
        assert_traps!(
            sqrt_f32,
            floor_f32,
            ceil_f32,
            trunc_f32,
            round_ties_even_f32,
            sin_f32,
            cos_f32,
            exp_f32,
            exp2_f32,
            ln_f32,
            log2_f32,
            log10_f32,
        );
        assert!(catch_unwind(AssertUnwindSafe(|| math.mul_add_f32(1.0, 2.0, 3.0))).is_err());
    }

    #[test]
    fn packed_fma_stub_fails_closed_on_host() {
        let math = DeviceMath::for_host_test();
        let policy = NumericalPolicyCapability::<UnbrandedCapability, StrictIeee>::for_host_test();
        let math = math.with_numerical_policy(&policy);
        let lanes = Bf16x2::new(Bf16::ONE, Bf16::ONE);
        assert!(
            catch_unwind(AssertUnwindSafe(|| math.mul_add_bf16x2(lanes, lanes, lanes))).is_err()
        );
    }
}
