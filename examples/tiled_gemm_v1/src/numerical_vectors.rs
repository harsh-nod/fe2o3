//! Deterministic profile-neutral vectors for the LDS GEMM numerical contract.

use crate::numerical_contract::{
    ComparisonPolicy, GemmInputs, GemmSpec, HardwareExpectation, HardwareExpectationError,
    build_hardware_expectation,
};

/// Semantic purpose of one deterministic corpus vector.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum NumericalVectorKind {
    /// Positive and negative zero propagation.
    Zero,
    /// Identity-matrix multiplication.
    Identity,
    /// Exactly representable dyadic arithmetic.
    Dyadic,
    /// Positive and negative products that cancel exactly.
    Cancellation,
    /// Seeded mixed-sign BF16 values with a non-tile-aligned reduction.
    Randomized,
    /// Independent padded A, B, and C row strides.
    PaddedStride,
    /// M, N, and K dimensions with partial 16-element tiles.
    Tail,
    /// Nonzero initial C with nontrivial alpha and beta.
    NonzeroC,
    /// Extreme finite normals and signed zeros.
    AdversarialFinite,
}

/// One owned deterministic vector and its sealed comparison policy.
#[derive(Clone, Debug)]
pub struct DeterministicGemmVector {
    name: &'static str,
    kind: NumericalVectorKind,
    spec: GemmSpec,
    a_bits: Vec<u16>,
    b_bits: Vec<u16>,
    c: Vec<f32>,
    alpha: f32,
    beta: f32,
    policy: ComparisonPolicy,
}

impl DeterministicGemmVector {
    /// Returns the stable corpus case name.
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the behavior covered by this case.
    pub const fn kind(&self) -> NumericalVectorKind {
        self.kind
    }

    /// Returns the checked profile-neutral dimensions and strides.
    pub const fn spec(&self) -> GemmSpec {
        self.spec
    }

    /// Returns exact BF16 encodings for A storage, including row padding.
    pub fn a_bits(&self) -> &[u16] {
        &self.a_bits
    }

    /// Returns exact BF16 encodings for B storage, including row padding.
    pub fn b_bits(&self) -> &[u16] {
        &self.b_bits
    }

    /// Returns initial FP32 C storage, including row padding.
    pub fn c(&self) -> &[f32] {
        &self.c
    }

    /// Returns the product coefficient.
    pub const fn alpha(&self) -> f32 {
        self.alpha
    }

    /// Returns the initial-output coefficient.
    pub const fn beta(&self) -> f32 {
        self.beta
    }

    /// Returns the policy fixed by this corpus case.
    pub const fn policy(&self) -> ComparisonPolicy {
        self.policy
    }

    /// Borrows this vector as generic GEMM inputs.
    pub fn inputs(&self) -> GemmInputs<'_> {
        GemmInputs {
            a_bits: &self.a_bits,
            b_bits: &self.b_bits,
            c: &self.c,
            alpha: self.alpha,
            beta: self.beta,
        }
    }

    /// Builds the strict finite expectation and seals this case's policy.
    pub fn expectation(&self) -> Result<HardwareExpectation, HardwareExpectationError> {
        build_hardware_expectation(self.spec, self.inputs(), self.policy)
    }
}

fn packed_spec(m: usize, n: usize, k: usize) -> GemmSpec {
    GemmSpec::checked(m, n, k, k, n, n).expect("fixed corpus shape must be bounded")
}

fn exact_vector(
    name: &'static str,
    kind: NumericalVectorKind,
    spec: GemmSpec,
    a_bits: Vec<u16>,
    b_bits: Vec<u16>,
    c: Vec<f32>,
    coefficients: [f32; 2],
) -> DeterministicGemmVector {
    DeterministicGemmVector {
        name,
        kind,
        spec,
        a_bits,
        b_bits,
        c,
        alpha: coefficients[0],
        beta: coefficients[1],
        policy: ComparisonPolicy::ExactBits,
    }
}

fn zero_vector() -> DeterministicGemmVector {
    let spec = packed_spec(2, 3, 4);
    exact_vector(
        "zero-signed",
        NumericalVectorKind::Zero,
        spec,
        vec![
            0x0000, 0x8000, 0x0000, 0x8000, 0x8000, 0x0000, 0x8000, 0x0000,
        ],
        vec![
            0x0000, 0x8000, 0x0000, 0x8000, 0x0000, 0x8000, 0x0000, 0x8000, 0x0000, 0x8000, 0x0000,
            0x8000,
        ],
        vec![0.0, -0.0, 0.0, -0.0, 0.0, -0.0],
        [1.0, 1.0],
    )
}

fn identity_vector() -> DeterministicGemmVector {
    let spec = packed_spec(3, 3, 3);
    exact_vector(
        "identity-3x3",
        NumericalVectorKind::Identity,
        spec,
        vec![
            0x3f80, 0x0000, 0x0000, 0x0000, 0x3f80, 0x0000, 0x0000, 0x0000, 0x3f80,
        ],
        vec![
            0x3f00, 0xbf80, 0x4000, 0x4040, 0xc000, 0x3e80, 0x4080, 0x3f80, 0xc040,
        ],
        vec![0.0; 9],
        [1.0, 0.0],
    )
}

fn dyadic_vector() -> DeterministicGemmVector {
    let spec = packed_spec(2, 2, 3);
    exact_vector(
        "dyadic-2x2x3",
        NumericalVectorKind::Dyadic,
        spec,
        vec![0x3f00, 0xbf80, 0x4000, 0x3e80, 0x4040, 0xc000],
        vec![0x3f80, 0x3e00, 0xbf00, 0x4000, 0x4080, 0x3f00],
        vec![0.0; 4],
        [1.0, 0.0],
    )
}

fn cancellation_vector() -> DeterministicGemmVector {
    let spec = packed_spec(1, 2, 4);
    exact_vector(
        "cancellation-pairs",
        NumericalVectorKind::Cancellation,
        spec,
        vec![0x3f80, 0x3f80, 0x4000, 0x4000],
        vec![
            0x4080, 0x4040, 0xc080, 0xc040, 0x3f00, 0xbf00, 0xbf00, 0x3f00,
        ],
        vec![0.0; 2],
        [1.0, 0.0],
    )
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut value = *state;
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn randomized_vector() -> DeterministicGemmVector {
    const ALPHABET: [u16; 12] = [
        0xbd80, 0xbe00, 0xbe80, 0xbf00, 0xbf80, 0x0000, 0x3d80, 0x3e00, 0x3e80, 0x3f00, 0x3f80,
        0x4000,
    ];
    let spec = packed_spec(5, 4, 17);
    let mut state = 0x1090_5eed_d15c_a11eu64;
    let mut generate = |len: usize| {
        (0..len)
            .map(|_| ALPHABET[(splitmix64(&mut state) as usize) % ALPHABET.len()])
            .collect::<Vec<_>>()
    };
    DeterministicGemmVector {
        name: "randomized-seed-109",
        kind: NumericalVectorKind::Randomized,
        spec,
        a_bits: generate(spec.a_len()),
        b_bits: generate(spec.b_len()),
        c: vec![0.0; spec.c_len()],
        alpha: 1.0,
        beta: 0.0,
        policy: ComparisonPolicy::bounded(0.000_01, 0.000_01, 8)
            .expect("fixed bounded policy must be valid"),
    }
}

fn padded_stride_vector() -> DeterministicGemmVector {
    let spec = GemmSpec::checked(3, 2, 4, 7, 5, 4).expect("fixed padded shape must be bounded");
    let mut a_bits = vec![0x7fc1; spec.a_len()];
    let mut b_bits = vec![0x7f80; spec.b_len()];
    let mut c = vec![f32::NAN; spec.c_len()];
    for row in 0..3 {
        for depth in 0..4 {
            a_bits[row * 7 + depth] = [0x3f80, 0x4000, 0xbf80, 0x3f00][(row + depth) % 4];
        }
    }
    for depth in 0..4 {
        for column in 0..2 {
            b_bits[depth * 5 + column] = [0x3f00, 0xbf00, 0x4000, 0xc000][depth];
        }
    }
    for row in 0..3 {
        for column in 0..2 {
            c[row * 4 + column] = 0.0;
        }
    }
    exact_vector(
        "padded-independent-strides",
        NumericalVectorKind::PaddedStride,
        spec,
        a_bits,
        b_bits,
        c,
        [1.0, 0.0],
    )
}

fn tail_vector() -> DeterministicGemmVector {
    const ALPHABET: [u16; 8] = [
        0xbe80, 0xbe00, 0x0000, 0x3e00, 0x3e80, 0x3f00, 0xbf00, 0x3f80,
    ];
    let spec = packed_spec(17, 19, 23);
    let a_bits = (0..spec.a_len())
        .map(|index| ALPHABET[(index * 5 + index / 7 + 3) % ALPHABET.len()])
        .collect();
    let b_bits = (0..spec.b_len())
        .map(|index| ALPHABET[(index * 3 + index / 11 + 1) % ALPHABET.len()])
        .collect();
    exact_vector(
        "tails-m17-n19-k23",
        NumericalVectorKind::Tail,
        spec,
        a_bits,
        b_bits,
        vec![0.0; spec.c_len()],
        [1.0, 0.0],
    )
}

fn nonzero_c_vector() -> DeterministicGemmVector {
    let spec = packed_spec(2, 3, 3);
    exact_vector(
        "alpha-beta-nonzero-c",
        NumericalVectorKind::NonzeroC,
        spec,
        vec![0x3f80, 0x4000, 0xbf80, 0x3f00, 0xc000, 0x4040],
        vec![
            0x3f80, 0x3f00, 0xbf00, 0x4000, 0xc000, 0x3e80, 0x4080, 0x3f80, 0xc080,
        ],
        vec![4.0, -8.0, 12.0, -16.0, 20.0, -24.0],
        [0.5, 0.25],
    )
}

fn adversarial_finite_vector() -> DeterministicGemmVector {
    let spec = packed_spec(6, 1, 1);
    exact_vector(
        "finite-extremes-and-signed-zero",
        NumericalVectorKind::AdversarialFinite,
        spec,
        vec![0x7f7f, 0xff7f, 0x0080, 0x8080, 0x0000, 0x8000],
        vec![0x3f80],
        vec![0.0, -0.0, 0.0, -0.0, 0.0, -0.0],
        [1.0, 0.0],
    )
}

/// Returns the complete version-one deterministic numerical corpus.
///
/// The returned order and each case name are stable. Randomized values use a
/// fixed local SplitMix64 recurrence and never depend on platform RNG state.
pub fn deterministic_gemm_vectors() -> Vec<DeterministicGemmVector> {
    vec![
        zero_vector(),
        identity_vector(),
        dyadic_vector(),
        cancellation_vector(),
        randomized_vector(),
        padded_stride_vector(),
        tail_vector(),
        nonzero_c_vector(),
        adversarial_finite_vector(),
    ]
}
