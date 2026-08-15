//! Frozen shape, layout, launch, ownership, and numerical contracts.

/// Fixed sequence length for Q, K, V, and O.
pub const FLASH_ATTENTION_SEQUENCE_LENGTH_V1: usize = 8;
/// Fixed feature dimension for each Q, K, V, and O row.
pub const FLASH_ATTENTION_HEAD_DIMENSION_V1: usize = 16;
/// Element count of each contiguous Q, K, V, and O allocation.
pub const FLASH_ATTENTION_INPUT_ELEMENTS_V1: usize =
    FLASH_ATTENTION_SEQUENCE_LENGTH_V1 * FLASH_ATTENTION_HEAD_DIMENSION_V1;
/// Output element count, equal to the input tensor element count.
pub const FLASH_ATTENTION_OUTPUT_ELEMENTS_V1: usize = FLASH_ATTENTION_INPUT_ELEMENTS_V1;
/// One physical gfx942 Wave64 executes the complete fixed profile.
pub const FLASH_ATTENTION_WAVE_LANES_V1: usize = 64;
/// Number of adjacent O elements exclusively owned by each physical lane.
pub const FLASH_ATTENTION_OUTPUT_ELEMENTS_PER_LANE_V1: usize =
    FLASH_ATTENTION_OUTPUT_ELEMENTS_V1 / FLASH_ATTENTION_WAVE_LANES_V1;

/// Q, K, V, and O use IEEE-754 binary32 elements.
pub const DTYPE_POLICY_V1: &str = "Q/K/V/O are f32; no input or output conversion";
/// All tensors are contiguous row-major `[sequence][head_dimension]` arrays.
pub const LAYOUT_POLICY_V1: &str = "row-major contiguous [8][16], stride [16,1]";
/// Query row `r` attends exactly to key rows `0..=r`.
pub const CAUSAL_POLICY_V1: &str = "causal lower triangle, diagonal included";
/// QK dot products, online state, and O accumulation all use strict FP32.
pub const ACCUMULATION_POLICY_V1: &str =
    "sequential strict f32 dot, scale, online max/sum, and V accumulation; no contraction";
/// Non-finite values and non-finite intermediates are rejected.
pub const EXCEPTIONAL_VALUE_POLICY_V1: &str =
    "finite Q/K/V required; trap before lane-owned writes on any non-finite intermediate";
/// The exact attention scale `1/sqrt(16) = 0.25` as binary32 bits.
pub const ATTENTION_SCALE_BITS_V1: u32 = 0x3e80_0000;

/// Tensor whose contiguous length or layout is being described.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TensorV1 {
    /// Query tensor.
    Q,
    /// Key tensor.
    K,
    /// Value tensor.
    V,
    /// Output tensor.
    O,
}

/// Layout admitted by the exact Phase A profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutV1 {
    /// Contiguous row-major `[sequence][head_dimension]`.
    RowMajorContiguous,
    /// Deliberately unsupported column-major layout for negative admission tests.
    ColumnMajorContiguous,
}

/// Mask policy admitted by the exact Phase A profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaskPolicyV1 {
    /// Lower triangle including the diagonal.
    CausalLowerTriangularInclusive,
    /// Deliberately unsupported unmasked profile for negative admission tests.
    NonCausal,
}

/// Complete identity-bearing fixed profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlashAttentionProfileV1 {
    /// AMDGPU processor name.
    pub processor: &'static str,
    /// Required target feature identity.
    pub target_features: &'static str,
    /// Physical wave width.
    pub wave_width: usize,
    /// Batch count.
    pub batches: usize,
    /// Attention head count.
    pub heads: usize,
    /// Sequence length.
    pub sequence_length: usize,
    /// Feature dimension.
    pub head_dimension: usize,
    /// Tensor layout.
    pub layout: LayoutV1,
    /// Mask policy.
    pub mask: MaskPolicyV1,
    /// Exact binary32 attention-scale bits.
    pub attention_scale_bits: u32,
    /// Workgroup dimensions.
    pub workgroup: [u32; 3],
    /// Grid dimensions in workgroups.
    pub grid: [u32; 3],
}

/// The only profile admitted by this Phase A crate.
pub const EXACT_PROFILE_V1: FlashAttentionProfileV1 = FlashAttentionProfileV1 {
    processor: "gfx942",
    target_features: "+wavefrontsize64,-xnack",
    wave_width: FLASH_ATTENTION_WAVE_LANES_V1,
    batches: 1,
    heads: 1,
    sequence_length: FLASH_ATTENTION_SEQUENCE_LENGTH_V1,
    head_dimension: FLASH_ATTENTION_HEAD_DIMENSION_V1,
    layout: LayoutV1::RowMajorContiguous,
    mask: MaskPolicyV1::CausalLowerTriangularInclusive,
    attention_scale_bits: ATTENTION_SCALE_BITS_V1,
    workgroup: [64, 1, 1],
    grid: [1, 1, 1],
};

/// Exact profile-admission failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileMismatchV1 {
    /// Processor or feature identity drifted.
    Target,
    /// Wave width, workgroup, or grid drifted.
    Launch,
    /// Batch, head, sequence, or feature shape drifted.
    Shape,
    /// Tensor layout drifted.
    Layout,
    /// Causal policy drifted.
    Mask,
    /// Attention scale drifted.
    NumericalPolicy,
}

/// Admits only [`EXACT_PROFILE_V1`], with a stable mismatch category.
pub fn validate_profile_v1(profile: FlashAttentionProfileV1) -> Result<(), ProfileMismatchV1> {
    if !str_equal(profile.processor, EXACT_PROFILE_V1.processor)
        || !str_equal(profile.target_features, EXACT_PROFILE_V1.target_features)
    {
        return Err(ProfileMismatchV1::Target);
    }
    if profile.wave_width != EXACT_PROFILE_V1.wave_width
        || profile.workgroup != EXACT_PROFILE_V1.workgroup
        || profile.grid != EXACT_PROFILE_V1.grid
    {
        return Err(ProfileMismatchV1::Launch);
    }
    if profile.batches != EXACT_PROFILE_V1.batches
        || profile.heads != EXACT_PROFILE_V1.heads
        || profile.sequence_length != EXACT_PROFILE_V1.sequence_length
        || profile.head_dimension != EXACT_PROFILE_V1.head_dimension
    {
        return Err(ProfileMismatchV1::Shape);
    }
    if !matches!(profile.layout, LayoutV1::RowMajorContiguous) {
        return Err(ProfileMismatchV1::Layout);
    }
    if !matches!(profile.mask, MaskPolicyV1::CausalLowerTriangularInclusive) {
        return Err(ProfileMismatchV1::Mask);
    }
    if profile.attention_scale_bits != EXACT_PROFILE_V1.attention_scale_bits {
        return Err(ProfileMismatchV1::NumericalPolicy);
    }
    Ok(())
}

fn str_equal(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    if left.len() != right.len() {
        return false;
    }
    let mut index = 0;
    while index < left.len() {
        if left[index] != right[index] {
            return false;
        }
        index += 1;
    }
    true
}

/// Returns the exact grid and workgroup dimensions.
pub const fn exact_launch_v1() -> ([u32; 3], [u32; 3]) {
    (EXACT_PROFILE_V1.grid, EXACT_PROFILE_V1.workgroup)
}

/// Returns the contiguous row-major index for a valid tensor coordinate.
pub const fn qkv_index_v1(row: usize, column: usize) -> Option<usize> {
    if row < FLASH_ATTENTION_SEQUENCE_LENGTH_V1 && column < FLASH_ATTENTION_HEAD_DIMENSION_V1 {
        Some(row * FLASH_ATTENTION_HEAD_DIMENSION_V1 + column)
    } else {
        None
    }
}

/// Returns whether `key_row` participates in causal query `query_row`.
pub const fn key_participates_v1(query_row: usize, key_row: usize) -> bool {
    query_row < FLASH_ATTENTION_SEQUENCE_LENGTH_V1
        && key_row < FLASH_ATTENTION_SEQUENCE_LENGTH_V1
        && key_row <= query_row
}

/// Returns the two adjacent O indices exclusively owned by one Wave64 lane.
pub const fn lane_outputs_v1(lane: usize) -> Option<[usize; 2]> {
    if lane < FLASH_ATTENTION_WAVE_LANES_V1 {
        let first = lane * FLASH_ATTENTION_OUTPUT_ELEMENTS_PER_LANE_V1;
        Some([first, first + 1])
    } else {
        None
    }
}
