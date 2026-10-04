//! Deterministic finite-logit corpus for the exact router.

use crate::contract::MOE_LOGIT_ELEMENTS_V1;

/// One named deterministic input vector.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeterministicVectorV1 {
    /// Stable corpus name.
    pub name: &'static str,
    /// Token-major `[8][4]` finite logits.
    pub logits: [f32; MOE_LOGIT_ELEMENTS_V1],
}

const NOMINAL: [f32; MOE_LOGIT_ELEMENTS_V1] = [
    4.0, 3.0, 2.0, 1.0, // 0,1
    1.0, 4.0, 3.0, 2.0, // 1,2
    2.0, 1.0, 4.0, 3.0, // 2,3
    3.0, 2.0, 1.0, 4.0, // 3,0
    4.0, 3.0, 2.0, 1.0, // 0,1
    1.0, 4.0, 3.0, 2.0, // 1,2
    2.0, 1.0, 4.0, 3.0, // 2,3
    3.0, 2.0, 1.0, 4.0, // 3,0
];

const ALL_EQUAL: [f32; MOE_LOGIT_ELEMENTS_V1] = [1.0; MOE_LOGIT_ELEMENTS_V1];

const REPEATED_LOGITS: [f32; MOE_LOGIT_ELEMENTS_V1] = [
    5.0, 5.0, 5.0, 4.0, // 0,1
    1.0, 3.0, 3.0, 3.0, // 1,2
    -2.0, -2.0, -2.0, -2.0, // 0,1
    7.0, 6.0, 7.0, 6.0, // 0,2
    0.0, -1.0, -1.0, 0.0, // 0,3
    2.0, 2.0, 1.0, 2.0, // 0,1
    8.0, 9.0, 9.0, 8.0, // 1,2
    -3.0, -2.0, -3.0, -2.0, // 1,3
];

const CAPACITY_OVERFLOW: [f32; MOE_LOGIT_ELEMENTS_V1] = [
    10.0, 9.0, 2.0, 1.0, // 0,1
    10.0, 1.0, 9.0, 2.0, // 0,2
    10.0, 2.0, 1.0, 9.0, // 0,3
    10.0, 9.0, 2.0, 1.0, // 0,1
    10.0, 1.0, 9.0, 2.0, // 0,2; 0 drops here
    10.0, 2.0, 1.0, 9.0, // 0,3
    10.0, 9.0, 2.0, 1.0, // 0,1
    10.0, 1.0, 9.0, 2.0, // 0,2
];

const EMPTY_EXPERTS: [f32; MOE_LOGIT_ELEMENTS_V1] = [
    2.0, 1.0, -4.0, -5.0, 2.0, 1.0, -3.0, -6.0, 2.0, 1.0, -2.0, -7.0, 2.0, 1.0, -1.0, -8.0, 2.0,
    1.0, -8.0, -1.0, 2.0, 1.0, -7.0, -2.0, 2.0, 1.0, -6.0, -3.0, 2.0, 1.0, -5.0, -4.0,
];

const ADVERSARIAL_FINITE: [f32; MOE_LOGIT_ELEMENTS_V1] = [
    f32::MAX,
    f32::MIN,
    f32::MIN_POSITIVE,
    -0.0,
    -f32::MIN_POSITIVE,
    0.0,
    f32::MIN,
    f32::MAX,
    f32::MAX,
    f32::MAX,
    f32::MIN,
    f32::MIN,
    f32::MIN,
    -0.0,
    0.0,
    f32::MIN_POSITIVE,
    1.0e30,
    -1.0e30,
    1.0e-30,
    -1.0e-30,
    -42.0,
    -42.0,
    -42.0,
    -43.0,
    17.0,
    18.0,
    17.0,
    18.0,
    3.5,
    3.25,
    3.75,
    3.0,
];

/// Returns the complete deterministic corpus.
pub const fn deterministic_vectors_v1() -> [DeterministicVectorV1; 6] {
    [
        DeterministicVectorV1 {
            name: "nominal-balanced",
            logits: NOMINAL,
        },
        DeterministicVectorV1 {
            name: "all-equal-tie-break-and-empty-experts",
            logits: ALL_EQUAL,
        },
        DeterministicVectorV1 {
            name: "repeated-logits",
            logits: REPEATED_LOGITS,
        },
        DeterministicVectorV1 {
            name: "stable-capacity-overflow",
            logits: CAPACITY_OVERFLOW,
        },
        DeterministicVectorV1 {
            name: "empty-experts",
            logits: EMPTY_EXPERTS,
        },
        DeterministicVectorV1 {
            name: "adversarial-finite-values",
            logits: ADVERSARIAL_FINITE,
        },
    ]
}
