#![forbid(unsafe_code)]

//! Target-neutral shape, resource, and numerical contract for general GEMM.

/// Logical rows produced by one matrix operation.
pub const TILE_M_V1: usize = 16;
/// Logical columns produced by one matrix operation.
pub const TILE_N_V1: usize = 16;
/// Reduction elements consumed by one matrix operation.
pub const TILE_K_V1: usize = 16;
/// Invocations in the required subgroup and workgroup.
pub const SUBGROUP_WIDTH_V1: usize = 64;
/// Values owned by one lane in each matrix fragment.
pub const FRAGMENT_ELEMENTS_PER_LANE_V1: usize = 4;
/// Number of reusable workgroup-memory stages.
pub const PIPELINE_BUFFERS_V1: usize = 2;
/// Number of K phases prefetched ahead of consumption.
pub const PIPELINE_PREFETCH_V1: usize = 1;
/// Exact storage for two operands, two stages, and four BF16 values per lane.
pub const STATIC_WORKGROUP_MEMORY_BYTES_V1: usize = 2
    * PIPELINE_BUFFERS_V1
    * SUBGROUP_WIDTH_V1
    * FRAGMENT_ELEMENTS_PER_LANE_V1
    * core::mem::size_of::<u16>();

/// Stable source-level numerical policy expected in canonical KIR V13.
///
/// This value is a requirement, not compiler or target evidence. Production
/// qualification must recover the same policy from the finalized graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GemmNumericalPolicyV1 {
    /// BF16 input bits widen exactly to FP32 before multiplication.
    pub exact_bf16_widening: bool,
    /// Each product and each depth-ordered sum rounds independently to FP32.
    pub strict_ieee_f32: bool,
    /// K-tail values are positive BF16 zero.
    pub positive_zero_k_tail: bool,
    /// The epilogue evaluates both products before their final FP32 addition.
    pub separate_alpha_beta_products: bool,
    /// Floating-point contraction and reassociation are forbidden.
    pub contraction_and_reassociation: bool,
}

/// Numerical policy for the target-neutral source and CPU oracle.
pub const GENERAL_GEMM_NUMERICAL_POLICY_V1: GemmNumericalPolicyV1 = GemmNumericalPolicyV1 {
    exact_bf16_widening: true,
    strict_ieee_f32: true,
    positive_zero_k_tail: true,
    separate_alpha_beta_products: true,
    contraction_and_reassociation: false,
};

/// Returns the physical extent touched by a row-major strided matrix.
pub const fn strided_extent_v1(rows: u32, columns: u32, stride: u32) -> u64 {
    if rows == 0 || columns == 0 {
        return 0;
    }
    (rows - 1) as u64 * stride as u64 + columns as u64
}

/// Applies the exact, non-contracted FP32 alpha/beta epilogue order.
#[inline(always)]
pub fn epilogue_v1(product: f32, prior: f32, alpha: f32, beta: f32) -> f32 {
    let scaled_product = alpha * product;
    let scaled_prior = beta * prior;
    scaled_product + scaled_prior
}

const _: () = {
    assert!(TILE_M_V1 * TILE_N_V1 == SUBGROUP_WIDTH_V1 * FRAGMENT_ELEMENTS_PER_LANE_V1);
    assert!(STATIC_WORKGROUP_MEMORY_BYTES_V1 == 2048);
    assert!(PIPELINE_PREFETCH_V1 < PIPELINE_BUFFERS_V1);
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_contract_derives_the_complete_static_resource_shape() {
        assert_eq!((TILE_M_V1, TILE_N_V1, TILE_K_V1), (16, 16, 16));
        assert_eq!(SUBGROUP_WIDTH_V1, 64);
        assert_eq!(FRAGMENT_ELEMENTS_PER_LANE_V1, 4);
        assert_eq!((PIPELINE_BUFFERS_V1, PIPELINE_PREFETCH_V1), (2, 1));
        assert_eq!(STATIC_WORKGROUP_MEMORY_BYTES_V1, 2048);
        assert_eq!(strided_extent_v1(19, 23, 27), 509);
    }

    #[test]
    fn numerical_policy_is_exact_and_non_contracted() {
        assert_eq!(
            GENERAL_GEMM_NUMERICAL_POLICY_V1,
            GemmNumericalPolicyV1 {
                exact_bf16_widening: true,
                strict_ieee_f32: true,
                positive_zero_k_tail: true,
                separate_alpha_beta_products: true,
                contraction_and_reassociation: false,
            }
        );

        let product = f32::from_bits(0x3f80_0001);
        let prior = f32::from_bits(0xbf00_0001);
        let alpha = f32::from_bits(0x3f7f_ffff);
        let beta = f32::from_bits(0x3f00_0001);
        let expected = (alpha * product) + (beta * prior);
        assert_eq!(
            epilogue_v1(product, prior, alpha, beta).to_bits(),
            expected.to_bits()
        );

        let almost_above_one = f32::from_bits(1.0_f32.to_bits() + 1);
        let almost_below_one = f32::from_bits(1.0_f32.to_bits() - 1);
        let separate = epilogue_v1(almost_below_one, -1.0, almost_above_one, 1.0);
        let contracted = almost_above_one.mul_add(almost_below_one, -1.0);
        assert_eq!(separate.to_bits(), 0.0_f32.to_bits());
        assert_ne!(separate.to_bits(), contracted.to_bits());
    }
}
