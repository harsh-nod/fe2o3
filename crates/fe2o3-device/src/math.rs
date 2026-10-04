//! Target-gated device math intrinsics.
//!
//! Every operation requires a compiler-recognized [`DeviceMath`] capability.
//! The safe acquisition and operation fallback bodies panic if reached on the
//! host. The fe2o3 backend must recognize the diagnostic identity,
//! authenticate the target and floating-point policy, and replace the complete
//! call before emitting AMDGPU code.
//!
//! The current backend does not perform that lowering yet. Keeping these
//! stubs inert is deliberate: host libm results are not evidence of OCML or
//! AMDGPU intrinsic behavior.

use core::marker::PhantomData;

use crate::Bf16x2;

/// Version of the device math semantic contract.
pub const DEVICE_MATH_CONTRACT_VERSION_V1: u16 = 1;

/// Compiler-created authority to call target-specific device math operations.
///
/// The value is neither `Copy`, `Clone`, `Send`, nor `Sync`. It carries no
/// memory, launch, synchronization, or cross-invocation authority. Safe
/// acquisition is sound because an unsupported call fails closed; target and
/// floating-point-policy validation remains a compiler obligation.
#[rustc_diagnostic_item = "fe2o3_device_math_context_v1"]
pub struct DeviceMath {
    _private: (),
    _not_send_sync: PhantomData<*mut ()>,
}

macro_rules! device_unary_intrinsic {
    ($(#[$meta:meta])* $name:ident, $diagnostic:literal, $llvm:literal) => {
        $(#[$meta])*
        #[must_use]
        #[inline(never)]
        #[rustc_diagnostic_item = $diagnostic]
        pub fn $name(&self, value: f32) -> f32 {
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
    ($(#[$meta:meta])* $name:ident, $diagnostic:literal, $llvm:literal) => {
        $(#[$meta])*
        #[must_use]
        #[inline(never)]
        #[rustc_diagnostic_item = $diagnostic]
        pub fn $name(&self, value: f32, multiplier: f32, addend: f32) -> f32 {
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

impl DeviceMath {
    /// Acquires the capability consumed by device intrinsic lowering.
    ///
    /// The fallback always panics. The backend may replace it only after
    /// validating the AMDGPU target, code-object version, denormal mode,
    /// contraction policy, and linked OCML/OCKL identity required by every
    /// reachable operation. Making acquisition safe does not relax any of
    /// those checks and grants no memory or collective-execution authority.
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
            _not_send_sync: PhantomData,
        }
    }

    device_unary_intrinsic!(
        /// Computes square root with the authenticated device floating-point policy.
        sqrt_f32,
        "fe2o3_device_math_sqrt_f32_v1",
        "llvm.sqrt.f32"
    );
    device_ternary_intrinsic!(
        /// Computes one fused `value * multiplier + addend` operation.
        mul_add_f32,
        "fe2o3_device_math_fma_f32_v1",
        "llvm.fma.f32"
    );
    device_unary_intrinsic!(
        /// Rounds toward negative infinity.
        floor_f32,
        "fe2o3_device_math_floor_f32_v1",
        "llvm.floor.f32"
    );
    device_unary_intrinsic!(
        /// Rounds toward positive infinity.
        ceil_f32,
        "fe2o3_device_math_ceil_f32_v1",
        "llvm.ceil.f32"
    );
    device_unary_intrinsic!(
        /// Rounds toward zero.
        trunc_f32,
        "fe2o3_device_math_trunc_f32_v1",
        "llvm.trunc.f32"
    );
    device_unary_intrinsic!(
        /// Rounds to the nearest integer, with halfway cases rounded to even.
        round_ties_even_f32,
        "fe2o3_device_math_roundeven_f32_v1",
        "llvm.roundeven.f32"
    );
    device_unary_intrinsic!(
        /// Computes sine in radians.
        sin_f32,
        "fe2o3_device_math_sin_f32_v1",
        "llvm.sin.f32"
    );
    device_unary_intrinsic!(
        /// Computes cosine in radians.
        cos_f32,
        "fe2o3_device_math_cos_f32_v1",
        "llvm.cos.f32"
    );
    device_unary_intrinsic!(
        /// Computes `e^value`.
        exp_f32,
        "fe2o3_device_math_exp_f32_v1",
        "llvm.exp.f32"
    );
    device_unary_intrinsic!(
        /// Computes `2^value`.
        exp2_f32,
        "fe2o3_device_math_exp2_f32_v1",
        "llvm.exp2.f32"
    );
    device_unary_intrinsic!(
        /// Computes the natural logarithm.
        ln_f32,
        "fe2o3_device_math_log_f32_v1",
        "llvm.log.f32"
    );
    device_unary_intrinsic!(
        /// Computes the base-two logarithm.
        log2_f32,
        "fe2o3_device_math_log2_f32_v1",
        "llvm.log2.f32"
    );
    device_unary_intrinsic!(
        /// Computes the base-ten logarithm.
        log10_f32,
        "fe2o3_device_math_log10_f32_v1",
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
    #[rustc_diagnostic_item = "fe2o3_device_math_fma_bf16x2_v1"]
    pub fn mul_add_bf16x2(&self, value: Bf16x2, multiplier: Bf16x2, addend: Bf16x2) -> Bf16x2 {
        let _ = (self, value, multiplier, addend);
        unreachable!(
            "DeviceMath::mul_add_bf16x2 must be lowered for an authenticated AMDGPU target"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{DEVICE_MATH_CONTRACT_VERSION_V1, DeviceMath};
    use crate::{Bf16, Bf16x2};
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
        let unary: &[fn(&DeviceMath, f32) -> f32] = &[
            DeviceMath::sqrt_f32,
            DeviceMath::floor_f32,
            DeviceMath::ceil_f32,
            DeviceMath::trunc_f32,
            DeviceMath::round_ties_even_f32,
            DeviceMath::sin_f32,
            DeviceMath::cos_f32,
            DeviceMath::exp_f32,
            DeviceMath::exp2_f32,
            DeviceMath::ln_f32,
            DeviceMath::log2_f32,
            DeviceMath::log10_f32,
        ];

        for operation in unary {
            assert!(catch_unwind(AssertUnwindSafe(|| operation(&math, 1.25))).is_err());
        }
        assert!(catch_unwind(AssertUnwindSafe(|| math.mul_add_f32(1.0, 2.0, 3.0))).is_err());
    }

    #[test]
    fn packed_fma_stub_fails_closed_on_host() {
        let math = DeviceMath::for_host_test();
        let lanes = Bf16x2::new(Bf16::ONE, Bf16::ONE);
        assert!(
            catch_unwind(AssertUnwindSafe(|| math.mul_add_bf16x2(lanes, lanes, lanes))).is_err()
        );
    }
}
