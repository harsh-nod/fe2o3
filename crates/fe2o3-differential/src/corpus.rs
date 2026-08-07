use core::fmt;

use crate::{
    AccessKind, AtomicOperation, AtomicScope, AtomicSpec, CopyNonoverlappingSpec,
    IntegerSwitchSpec, LayoutSpec, MAX_ATOMIC_STEPS, MAX_LAYOUT_MEMBERS, MAX_OBLIGATION_ACCESSES,
    MAX_SEMANTIC_WORDS, MAX_SWITCH_ARMS, MemoryAccess, MemoryOrdering, ObligationSpec,
    PointerDistanceSpec, ScalarLayout, SemanticCase, SemanticFeature, SemanticModelError,
    SemanticSpec, VolatileOperation, VolatileSpec,
};

pub const SEMANTIC_CORPUS_VERSION_V1: u8 = 1;
pub const MAX_CASES_PER_FEATURE: u8 = 16;
pub const MAX_SEMANTIC_CORPUS_CASES: usize =
    SemanticFeature::ALL.len() * MAX_CASES_PER_FEATURE as usize;
pub const MAX_SEMANTIC_CANONICAL_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticCorpusConfig {
    cases_per_feature: u8,
}

impl SemanticCorpusConfig {
    pub fn new(cases_per_feature: u8) -> Result<Self, CorpusError> {
        if cases_per_feature == 0 || cases_per_feature > MAX_CASES_PER_FEATURE {
            return Err(CorpusError::InvalidCasesPerFeature {
                actual: cases_per_feature,
            });
        }
        Ok(Self { cases_per_feature })
    }

    pub fn cases_per_feature(self) -> u8 {
        self.cases_per_feature
    }
}

impl Default for SemanticCorpusConfig {
    fn default() -> Self {
        Self {
            cases_per_feature: 8,
        }
    }
}

/// A deterministic locator and mutation-detection fingerprint for a generated case.
///
/// This identity is reproducibility metadata, not cryptographic authentication or
/// compiler, proof, artifact, load, or launch authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticReplayIdentityV1 {
    pub corpus_version: u8,
    pub seed: u64,
    pub feature: SemanticFeature,
    pub ordinal: u16,
    pub canonical_fingerprint: [u8; 32],
}

/// Exact canonical identity for any valid case, including reduced cases.
///
/// Like `SemanticReplayIdentityV1`, this is deterministic test metadata and
/// carries no authentication or execution authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticCaseIdentityV1 {
    pub corpus_version: u8,
    pub canonical_fingerprint: [u8; 32],
}

/// Generates the bounded corpus in stable feature-major, ordinal-minor order.
pub fn generate_semantic_corpus(seed: u64, config: SemanticCorpusConfig) -> Vec<SemanticCase> {
    let mut cases =
        Vec::with_capacity(SemanticFeature::ALL.len() * usize::from(config.cases_per_feature));
    for feature in SemanticFeature::ALL {
        for ordinal in 0..u16::from(config.cases_per_feature) {
            cases.push(
                generate_semantic_case(seed, feature, ordinal).expect(
                    "the bounded deterministic semantic generator must construct valid cases",
                ),
            );
        }
    }
    cases
}

/// Reconstructs one generated case directly from its stable coordinates.
pub fn generate_semantic_case(
    seed: u64,
    feature: SemanticFeature,
    ordinal: u16,
) -> Result<SemanticCase, CorpusError> {
    if ordinal >= u16::from(MAX_CASES_PER_FEATURE) {
        return Err(CorpusError::OrdinalOutOfRange { ordinal });
    }
    let mut random = SplitMix64::new(
        seed ^ feature_domain(feature) ^ u64::from(ordinal).wrapping_mul(0xd6e8_feb8_6659_fd93),
    );
    let specification = match feature {
        SemanticFeature::PointerDistance => pointer_case(ordinal, &mut random),
        SemanticFeature::VolatileMemory => volatile_case(ordinal, &mut random),
        SemanticFeature::CopyNonoverlapping => copy_case(ordinal, &mut random),
        SemanticFeature::RustLayout => layout_case(ordinal, &mut random),
        SemanticFeature::IntegerSwitch => switch_case(ordinal, &mut random),
        SemanticFeature::AtomicScopes => atomic_case(ordinal, &mut random),
        SemanticFeature::BoundsAndRaces => obligation_case(ordinal, &mut random),
    };
    SemanticCase::new(seed, ordinal, feature, specification).map_err(CorpusError::InvalidCase)
}

/// Creates a replay identity only for a case exactly emitted by the V1 generator.
pub fn semantic_replay_identity_v1(
    case: &SemanticCase,
) -> Result<SemanticReplayIdentityV1, CorpusError> {
    let generated = generate_semantic_case(case.seed(), case.feature(), case.ordinal())?;
    if &generated != case {
        return Err(CorpusError::CaseNotGenerated);
    }
    let bytes = encode_semantic_case_v1(case)?;
    Ok(SemanticReplayIdentityV1 {
        corpus_version: SEMANTIC_CORPUS_VERSION_V1,
        seed: case.seed(),
        feature: case.feature(),
        ordinal: case.ordinal(),
        canonical_fingerprint: replay_fingerprint(&bytes),
    })
}

pub fn semantic_case_identity_v1(
    case: &SemanticCase,
) -> Result<SemanticCaseIdentityV1, CorpusError> {
    let bytes = encode_semantic_case_v1(case)?;
    Ok(SemanticCaseIdentityV1 {
        corpus_version: SEMANTIC_CORPUS_VERSION_V1,
        canonical_fingerprint: replay_fingerprint(&bytes),
    })
}

/// Regenerates a case and rejects version, coordinate, or fingerprint substitution.
pub fn replay_semantic_case_v1(
    identity: SemanticReplayIdentityV1,
) -> Result<SemanticCase, CorpusError> {
    if identity.corpus_version != SEMANTIC_CORPUS_VERSION_V1 {
        return Err(CorpusError::UnsupportedCorpusVersion {
            actual: identity.corpus_version,
        });
    }
    let case = generate_semantic_case(identity.seed, identity.feature, identity.ordinal)?;
    let bytes = encode_semantic_case_v1(&case)?;
    if replay_fingerprint(&bytes) != identity.canonical_fingerprint {
        return Err(CorpusError::ReplayFingerprintMismatch);
    }
    Ok(case)
}

/// Encodes a semantic case into a unique, bounded V1 representation.
pub fn encode_semantic_case_v1(case: &SemanticCase) -> Result<Vec<u8>, CorpusError> {
    case.validate().map_err(CorpusError::InvalidCase)?;
    let mut bytes = Vec::with_capacity(128);
    bytes.extend_from_slice(b"F2SC");
    bytes.push(SEMANTIC_CORPUS_VERSION_V1);
    bytes.extend_from_slice(&case.seed().to_le_bytes());
    bytes.extend_from_slice(&case.ordinal().to_le_bytes());
    bytes.push(feature_tag(case.feature()));
    encode_specification(case.specification(), &mut bytes);
    if bytes.len() > MAX_SEMANTIC_CANONICAL_BYTES {
        return Err(CorpusError::CanonicalCaseTooLarge {
            actual: bytes.len(),
        });
    }
    Ok(bytes)
}

fn pointer_case(ordinal: u16, random: &mut SplitMix64) -> SemanticSpec {
    let element_bytes = [1_u8, 2, 4, 8][random.bounded(4)];
    let allocation_bytes = 64;
    let from_offset = u16::from(element_bytes) * random.bounded_u16(8);
    let forward = u16::from(element_bytes) * random.bounded_u16(8);
    let mut specification = PointerDistanceSpec {
        allocation_bytes,
        from_offset,
        to_offset: forward,
        element_bytes,
        same_allocation: true,
        signed: ordinal.is_multiple_of(2),
    };
    match ordinal % 4 {
        1 => specification.same_allocation = false,
        2 => {
            specification.signed = false;
            specification.from_offset = 32;
            specification.to_offset = 0;
        }
        3 => specification.to_offset = 65,
        _ => {}
    }
    SemanticSpec::PointerDistance(specification)
}

fn volatile_case(ordinal: u16, random: &mut SplitMix64) -> SemanticSpec {
    let mut words = random_words(random, 8);
    let mut specification = VolatileSpec {
        index: random.bounded_u16(words.len() as u16),
        byte_alignment: 4,
        readable: true,
        writable: true,
        operation: if ordinal.is_multiple_of(2) {
            VolatileOperation::Load
        } else {
            VolatileOperation::Store(random.next_u64() as i32)
        },
        words: core::mem::take(&mut words),
    };
    match ordinal % 4 {
        2 => {
            specification.readable = false;
            specification.operation = VolatileOperation::Load;
        }
        3 => specification.index = specification.words.len() as u16,
        _ => {}
    }
    SemanticSpec::Volatile(specification)
}

fn copy_case(ordinal: u16, random: &mut SplitMix64) -> SemanticSpec {
    let mut specification = CopyNonoverlappingSpec {
        words: random_words(random, 12),
        source: 0,
        destination: 8,
        count: 4,
    };
    match ordinal % 4 {
        1 => {
            specification.source = 2;
            specification.destination = 4;
            specification.count = 4;
        }
        2 => {
            specification.destination = 11;
            specification.count = 2;
        }
        3 => specification.count = 0,
        _ => {}
    }
    SemanticSpec::CopyNonoverlapping(specification)
}

fn layout_case(ordinal: u16, random: &mut SplitMix64) -> SemanticSpec {
    let scalar = |random: &mut SplitMix64| {
        let size = [1_u8, 2, 4, 8][random.bounded(4)];
        ScalarLayout {
            size,
            alignment: size,
        }
    };
    let specification = match ordinal % 4 {
        0 | 3 => LayoutSpec::Aggregate {
            fields: (0..3).map(|_| scalar(random)).collect(),
        },
        1 => LayoutSpec::TaggedEnum {
            tag: ScalarLayout {
                size: 1,
                alignment: 1,
            },
            payloads: (0..3).map(|_| scalar(random)).collect(),
        },
        _ => LayoutSpec::NicheEnum {
            payload: scalar(random),
        },
    };
    SemanticSpec::Layout(specification)
}

fn switch_case(ordinal: u16, random: &mut SplitMix64) -> SemanticSpec {
    let base = random.next_u64() as i32;
    let mut arms = vec![
        (base, random.next_u64() as i32),
        (base.wrapping_add(1), random.next_u64() as i32),
        (base.wrapping_add(2), random.next_u64() as i32),
    ];
    let selector = match ordinal % 4 {
        0 => base.wrapping_add(1),
        1 => base.wrapping_add(9),
        2 => {
            arms[2].0 = base;
            base
        }
        _ => base.wrapping_add(2),
    };
    SemanticSpec::IntegerSwitch(IntegerSwitchSpec {
        selector,
        arms,
        default: random.next_u64() as i32,
    })
}

fn atomic_case(ordinal: u16, random: &mut SplitMix64) -> SemanticSpec {
    let initial = random.next_u64() as i32;
    let mut specification = AtomicSpec {
        initial,
        scope: AtomicScope::Device,
        operations: vec![
            AtomicOperation::FetchAdd {
                value: 1,
                ordering: MemoryOrdering::AcquireRelease,
            },
            AtomicOperation::CompareExchange {
                current: initial.wrapping_add(1),
                new: -1,
                success: MemoryOrdering::SequentiallyConsistent,
                failure: MemoryOrdering::Acquire,
            },
            AtomicOperation::Load {
                ordering: MemoryOrdering::Relaxed,
            },
        ],
    };
    match ordinal % 4 {
        1 => specification.scope = AtomicScope::System,
        2 => {
            specification.operations = vec![AtomicOperation::Load {
                ordering: MemoryOrdering::Release,
            }];
        }
        3 => specification.scope = AtomicScope::Workgroup,
        _ => {}
    }
    SemanticSpec::Atomics(specification)
}

fn obligation_case(ordinal: u16, _random: &mut SplitMix64) -> SemanticSpec {
    let specification = match ordinal % 4 {
        0 => ObligationSpec::Bounds {
            length: 8,
            index: 7,
        },
        1 => ObligationSpec::Bounds {
            length: 8,
            index: 8,
        },
        2 => ObligationSpec::Race {
            allocation_words: 8,
            accesses: vec![
                MemoryAccess {
                    lane: 0,
                    index: 3,
                    kind: AccessKind::Write,
                    atomic: true,
                },
                MemoryAccess {
                    lane: 1,
                    index: 3,
                    kind: AccessKind::Read,
                    atomic: true,
                },
            ],
        },
        _ => ObligationSpec::Race {
            allocation_words: 8,
            accesses: vec![
                MemoryAccess {
                    lane: 0,
                    index: 3,
                    kind: AccessKind::Write,
                    atomic: false,
                },
                MemoryAccess {
                    lane: 1,
                    index: 3,
                    kind: AccessKind::Read,
                    atomic: false,
                },
            ],
        },
    };
    SemanticSpec::Obligation(specification)
}

fn random_words(random: &mut SplitMix64, length: usize) -> Vec<i32> {
    debug_assert!(length <= MAX_SEMANTIC_WORDS);
    (0..length).map(|_| random.next_u64() as i32).collect()
}

fn encode_specification(specification: &SemanticSpec, bytes: &mut Vec<u8>) {
    match specification {
        SemanticSpec::PointerDistance(value) => {
            bytes.extend_from_slice(&value.allocation_bytes.to_le_bytes());
            bytes.extend_from_slice(&value.from_offset.to_le_bytes());
            bytes.extend_from_slice(&value.to_offset.to_le_bytes());
            bytes.push(value.element_bytes);
            bytes.push(u8::from(value.same_allocation));
            bytes.push(u8::from(value.signed));
        }
        SemanticSpec::Volatile(value) => {
            encode_words(&value.words, bytes);
            bytes.extend_from_slice(&value.index.to_le_bytes());
            bytes.push(value.byte_alignment);
            bytes.push(u8::from(value.readable));
            bytes.push(u8::from(value.writable));
            match value.operation {
                VolatileOperation::Load => bytes.push(0),
                VolatileOperation::Store(stored) => {
                    bytes.push(1);
                    bytes.extend_from_slice(&stored.to_le_bytes());
                }
            }
        }
        SemanticSpec::CopyNonoverlapping(value) => {
            encode_words(&value.words, bytes);
            bytes.extend_from_slice(&value.source.to_le_bytes());
            bytes.extend_from_slice(&value.destination.to_le_bytes());
            bytes.extend_from_slice(&value.count.to_le_bytes());
        }
        SemanticSpec::Layout(value) => encode_layout(value, bytes),
        SemanticSpec::IntegerSwitch(value) => {
            bytes.extend_from_slice(&value.selector.to_le_bytes());
            bytes.push(value.arms.len() as u8);
            for (selector, result) in &value.arms {
                bytes.extend_from_slice(&selector.to_le_bytes());
                bytes.extend_from_slice(&result.to_le_bytes());
            }
            bytes.extend_from_slice(&value.default.to_le_bytes());
        }
        SemanticSpec::Atomics(value) => {
            bytes.extend_from_slice(&value.initial.to_le_bytes());
            bytes.push(scope_tag(value.scope));
            bytes.push(value.operations.len() as u8);
            for operation in &value.operations {
                encode_atomic(*operation, bytes);
            }
        }
        SemanticSpec::Obligation(value) => encode_obligation(value, bytes),
    }
}

fn encode_words(words: &[i32], bytes: &mut Vec<u8>) {
    bytes.push(words.len() as u8);
    for word in words {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
}

fn encode_layout(layout: &LayoutSpec, bytes: &mut Vec<u8>) {
    match layout {
        LayoutSpec::Aggregate { fields } => {
            bytes.push(0);
            encode_scalar_layouts(fields, bytes);
        }
        LayoutSpec::TaggedEnum { tag, payloads } => {
            bytes.push(1);
            bytes.extend_from_slice(&[tag.size, tag.alignment]);
            encode_scalar_layouts(payloads, bytes);
        }
        LayoutSpec::NicheEnum { payload } => {
            bytes.push(2);
            bytes.extend_from_slice(&[payload.size, payload.alignment]);
        }
    }
}

fn encode_scalar_layouts(layouts: &[ScalarLayout], bytes: &mut Vec<u8>) {
    bytes.push(layouts.len() as u8);
    for layout in layouts {
        bytes.extend_from_slice(&[layout.size, layout.alignment]);
    }
}

fn encode_atomic(operation: AtomicOperation, bytes: &mut Vec<u8>) {
    match operation {
        AtomicOperation::Load { ordering } => bytes.extend_from_slice(&[0, ordering_tag(ordering)]),
        AtomicOperation::Store { value, ordering } => {
            bytes.extend_from_slice(&[1, ordering_tag(ordering)]);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        AtomicOperation::FetchAdd { value, ordering } => {
            bytes.extend_from_slice(&[2, ordering_tag(ordering)]);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        AtomicOperation::CompareExchange {
            current,
            new,
            success,
            failure,
        } => {
            bytes.extend_from_slice(&[3, ordering_tag(success), ordering_tag(failure)]);
            bytes.extend_from_slice(&current.to_le_bytes());
            bytes.extend_from_slice(&new.to_le_bytes());
        }
    }
}

fn encode_obligation(obligation: &ObligationSpec, bytes: &mut Vec<u8>) {
    match obligation {
        ObligationSpec::Bounds { length, index } => {
            bytes.push(0);
            bytes.extend_from_slice(&length.to_le_bytes());
            bytes.extend_from_slice(&index.to_le_bytes());
        }
        ObligationSpec::Race {
            allocation_words,
            accesses,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&allocation_words.to_le_bytes());
            bytes.push(accesses.len() as u8);
            for access in accesses {
                bytes.extend_from_slice(&access.lane.to_le_bytes());
                bytes.extend_from_slice(&access.index.to_le_bytes());
                bytes.push(match access.kind {
                    AccessKind::Read => 0,
                    AccessKind::Write => 1,
                });
                bytes.push(u8::from(access.atomic));
            }
        }
    }
}

fn replay_fingerprint(bytes: &[u8]) -> [u8; 32] {
    const OFFSETS: [u64; 4] = [
        0xcbf2_9ce4_8422_2325,
        0x8422_2325_cbf2_9ce4,
        0x9e37_79b9_7f4a_7c15,
        0x6a09_e667_f3bc_c909,
    ];
    const PRIMES: [u64; 4] = [
        0x0000_0100_0000_01b3,
        0x9e37_79b1_85eb_ca87,
        0xc2b2_ae3d_27d4_eb4f,
        0x1656_67b1_9e37_79f9,
    ];
    let mut state = OFFSETS;
    for (index, byte) in bytes.iter().copied().enumerate() {
        for lane in 0..state.len() {
            state[lane] ^=
                u64::from(byte).wrapping_add((index as u64).rotate_left((lane * 11) as u32));
            state[lane] = state[lane]
                .wrapping_mul(PRIMES[lane])
                .rotate_left((lane * 7 + 5) as u32);
        }
    }
    let mut fingerprint = [0; 32];
    for (chunk, value) in fingerprint.chunks_exact_mut(8).zip(state) {
        chunk.copy_from_slice(&value.to_le_bytes());
    }
    fingerprint
}

const fn feature_domain(feature: SemanticFeature) -> u64 {
    match feature {
        SemanticFeature::PointerDistance => 0x4645_324f_3350_5452,
        SemanticFeature::VolatileMemory => 0x4645_324f_3356_4f4c,
        SemanticFeature::CopyNonoverlapping => 0x4645_324f_3343_4f50,
        SemanticFeature::RustLayout => 0x4645_324f_334c_4159,
        SemanticFeature::IntegerSwitch => 0x4645_324f_3353_5749,
        SemanticFeature::AtomicScopes => 0x4645_324f_3341_544f,
        SemanticFeature::BoundsAndRaces => 0x4645_324f_334f_424c,
    }
}

const fn feature_tag(feature: SemanticFeature) -> u8 {
    match feature {
        SemanticFeature::PointerDistance => 0,
        SemanticFeature::VolatileMemory => 1,
        SemanticFeature::CopyNonoverlapping => 2,
        SemanticFeature::RustLayout => 3,
        SemanticFeature::IntegerSwitch => 4,
        SemanticFeature::AtomicScopes => 5,
        SemanticFeature::BoundsAndRaces => 6,
    }
}

const fn scope_tag(scope: AtomicScope) -> u8 {
    match scope {
        AtomicScope::Workgroup => 0,
        AtomicScope::Device => 1,
        AtomicScope::System => 2,
    }
}

const fn ordering_tag(ordering: MemoryOrdering) -> u8 {
    match ordering {
        MemoryOrdering::Relaxed => 0,
        MemoryOrdering::Acquire => 1,
        MemoryOrdering::Release => 2,
        MemoryOrdering::AcquireRelease => 3,
        MemoryOrdering::SequentiallyConsistent => 4,
    }
}

#[derive(Clone, Copy, Debug)]
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn bounded(&mut self, exclusive_upper: usize) -> usize {
        debug_assert!(exclusive_upper > 0);
        (self.next_u64() % exclusive_upper as u64) as usize
    }

    fn bounded_u16(&mut self, exclusive_upper: u16) -> u16 {
        debug_assert!(exclusive_upper > 0);
        (self.next_u64() % u64::from(exclusive_upper)) as u16
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CorpusError {
    InvalidCasesPerFeature { actual: u8 },
    OrdinalOutOfRange { ordinal: u16 },
    UnsupportedCorpusVersion { actual: u8 },
    ReplayFingerprintMismatch,
    CaseNotGenerated,
    CanonicalCaseTooLarge { actual: usize },
    InvalidCase(SemanticModelError),
}

impl fmt::Display for CorpusError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCasesPerFeature { actual } => {
                write!(formatter, "cases per feature {actual} is outside the bound")
            }
            Self::OrdinalOutOfRange { ordinal } => {
                write!(
                    formatter,
                    "semantic corpus ordinal {ordinal} is outside the bound"
                )
            }
            Self::UnsupportedCorpusVersion { actual } => {
                write!(formatter, "semantic corpus version {actual} is unsupported")
            }
            Self::ReplayFingerprintMismatch => {
                formatter.write_str("semantic replay fingerprint does not match regenerated case")
            }
            Self::CaseNotGenerated => {
                formatter.write_str("case was not emitted by the deterministic V1 generator")
            }
            Self::CanonicalCaseTooLarge { actual } => {
                write!(formatter, "canonical semantic case has {actual} bytes")
            }
            Self::InvalidCase(error) => write!(formatter, "semantic case is invalid: {error}"),
        }
    }
}

impl std::error::Error for CorpusError {}

const _: () = {
    assert!(MAX_LAYOUT_MEMBERS <= u8::MAX as usize);
    assert!(MAX_SWITCH_ARMS <= u8::MAX as usize);
    assert!(MAX_ATOMIC_STEPS <= u8::MAX as usize);
    assert!(MAX_OBLIGATION_ACCESSES <= u8::MAX as usize);
};
