//! Deterministic host vectors shared by debug and release validation.

/// One exact LDS reduction vector.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReductionVectorV1 {
    /// Human-readable stable case name.
    pub name: &'static str,
    /// Epoch identity supplied to the trace and source profile.
    pub epoch: u32,
    /// Exactly 64 admitted lane values.
    pub values: [i32; 64],
    /// Exact mathematical result.
    pub expected: i32,
}

/// One exact scoped atomic vector.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtomicVectorV1 {
    /// Human-readable stable case name.
    pub name: &'static str,
    /// Initial atomic-object value.
    pub initial: u32,
    /// Exactly 64 lane values.
    pub values: [u32; 64],
    /// Exactly 64 lane eligibility bits.
    pub eligible: [bool; 64],
    /// Exact final atomic-object value.
    pub expected: u32,
}

/// Returns deterministic reduction cases covering signs, zeros, and epochs.
pub fn reduction_vectors_v1() -> Vec<ReductionVectorV1> {
    let mut alternating = [0_i32; 64];
    let mut ramp = [0_i32; 64];
    let mut sparse = [0_i32; 64];
    for lane in 0..64 {
        alternating[lane] = if lane % 2 == 0 { 1_000 } else { -999 };
        ramp[lane] = lane as i32 - 32;
        sparse[lane] = if lane % 11 == 0 { lane as i32 * 7 } else { 0 };
    }
    vec![
        ReductionVectorV1 {
            name: "all-zero",
            epoch: 0,
            values: [0; 64],
            expected: 0,
        },
        ReductionVectorV1 {
            name: "alternating-cancellation",
            epoch: 1,
            values: alternating,
            expected: 32,
        },
        ReductionVectorV1 {
            name: "signed-ramp",
            epoch: 7,
            values: ramp,
            expected: -32,
        },
        ReductionVectorV1 {
            name: "sparse-positive",
            epoch: u32::MAX,
            values: sparse,
            expected: 1_155,
        },
    ]
}

/// Returns deterministic atomic cases covering eligibility and initial state.
pub fn atomic_vectors_v1() -> Vec<AtomicVectorV1> {
    let mut ramp = [0_u32; 64];
    let mut alternating = [false; 64];
    let mut sparse = [false; 64];
    for lane in 0..64 {
        ramp[lane] = lane as u32;
        alternating[lane] = lane % 2 == 0;
        sparse[lane] = lane % 13 == 0;
    }
    vec![
        AtomicVectorV1 {
            name: "none-eligible",
            initial: 17,
            values: ramp,
            eligible: [false; 64],
            expected: 17,
        },
        AtomicVectorV1 {
            name: "all-eligible",
            initial: 0,
            values: ramp,
            eligible: [true; 64],
            expected: 2_016,
        },
        AtomicVectorV1 {
            name: "alternating-eligible",
            initial: 11,
            values: ramp,
            eligible: alternating,
            expected: 1_003,
        },
        AtomicVectorV1 {
            name: "sparse-eligible",
            initial: 1_000,
            values: ramp,
            eligible: sparse,
            expected: 1_130,
        },
    ]
}
