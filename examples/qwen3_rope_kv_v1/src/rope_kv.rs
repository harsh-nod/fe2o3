//! Exact Qwen3 RoPE and paged-KV-write foundation for the Ferric M1 envelope.
//!
//! The executable functions in this module validate a finite structural
//! candidate, evaluate a CPU `f64` RoPE reference, validate generation-owned
//! page tables, and project an exclusive logical append into physical page
//! coordinates. The mathematical proof beside this crate establishes only
//! conditional integer pairing, bounds, reconstruction, and injectivity
//! properties. Neither layer refines device source, Kernel IR, LLVM, ISA, or a
//! running KV system.

use std::error::Error;
use std::fmt;

mod page_table;
mod reference;
mod write;

pub use page_table::{
    DraftPageTableGenerationV1, GenerationPairErrorV1, KvOwnerIdentityV1, KvPhysicalLocationV1,
    PageTableEntryV1, PageTableErrorV1, PageTableGenerationV1, Qwen3PageTableGenerationsV1,
    Qwen3PageTableV1, TargetPageTableGenerationV1, validate_qwen3_page_table_generations_v1,
};
pub use reference::{
    Qwen3RopeOutputV1, RopeReferenceErrorV1, qwen3_rope_pair_candidate_v1, qwen3_rope_reference_v1,
    qwen3_rotary_angle_v1, qwen3_rotary_inverse_frequency_v1, qwen3_rotary_pair_v1,
};
pub use write::{
    KvWriteErrorV1, Qwen3KvWriteCoordinateV1, Qwen3KvWriteDescriptorV1, Qwen3KvWriteElementV1,
    Qwen3KvWriteExpectationV1, Qwen3PagedKvWriteReferenceV1, project_qwen3_kv_write_v1,
    qwen3_kv_write_coordinate_v1, qwen3_paged_kv_write_reference_v1, validate_qwen3_kv_write_v1,
};

/// Maximum admitted M1 context length.
pub const M1_MAX_CONTEXT_TOKENS_V1: u32 = 8_192;
/// Exact Qwen3 rotary and attention head dimension.
pub const QWEN3_HEAD_DIMENSION_V1: u16 = 128;
/// Number of dimensions in one split half of a Qwen3 rotary head.
pub const QWEN3_ROPE_HALF_DIMENSION_V1: u16 = 64;
/// Exact Qwen3 rotary frequency base.
pub const QWEN3_ROPE_THETA_V1: u32 = 1_000_000;
/// Exact target Qwen3-8B transformer layer count.
pub const QWEN3_TARGET_LAYERS_V1: u16 = 36;
/// Exact draft Qwen3-0.6B transformer layer count.
pub const QWEN3_DRAFT_LAYERS_V1: u16 = 28;
/// Exact target query-head count.
pub const QWEN3_TARGET_QUERY_HEADS_V1: u16 = 32;
/// Exact draft query-head count.
pub const QWEN3_DRAFT_QUERY_HEADS_V1: u16 = 16;
/// Exact target and draft KV-head count.
pub const QWEN3_KV_HEADS_V1: u16 = 8;
/// Maximum physical page index accepted by this structural model.
pub const M1_MAX_PHYSICAL_PAGES_V1: u32 = 65_536;
/// Maximum page-table entries, reached by 8192 tokens with 16-token pages.
pub const M1_MAX_PAGE_TABLE_ENTRIES_V1: usize = 512;

/// Canonical UTF-8 preimage for [`QWEN3_ROPE_KV_FAMILY_ID_V1`].
pub const QWEN3_ROPE_KV_FAMILY_ID_PREIMAGE_V1: &str =
    "fe2o3.qwen3.rope_paged_kv.foundation.gfx942.v1";
/// Stable family namespace: SHA-256 of [`QWEN3_ROPE_KV_FAMILY_ID_PREIMAGE_V1`].
///
/// This is reproducible structural identity, not authentication.
pub const QWEN3_ROPE_KV_FAMILY_ID_V1: [u8; 32] = [
    0xe0, 0x31, 0xa4, 0x46, 0xcd, 0xd5, 0x7d, 0xbd, 0x16, 0xd3, 0xe9, 0x24, 0xd4, 0xf3, 0x3e, 0x60,
    0x76, 0x25, 0xbe, 0x1d, 0x2f, 0x7d, 0xd9, 0xd6, 0xfe, 0xd6, 0x01, 0xc8, 0xaa, 0x66, 0x39, 0x7e,
];
/// Canonical UTF-8 preimage for [`QWEN3_ROPE_KV_CANDIDATE_SCHEMA_ID_V1`].
pub const QWEN3_ROPE_KV_CANDIDATE_SCHEMA_ID_PREIMAGE_V1: &str =
    "fe2o3.qwen3.rope_paged_kv.candidate.schema.v1";
/// Stable candidate schema identity: SHA-256 of its canonical preimage.
pub const QWEN3_ROPE_KV_CANDIDATE_SCHEMA_ID_V1: [u8; 32] = [
    0x46, 0x82, 0x27, 0x54, 0x65, 0xba, 0xcc, 0x5e, 0xe1, 0x9e, 0xe1, 0x11, 0x2d, 0x8d, 0xcf, 0x46,
    0x70, 0x4c, 0x00, 0x23, 0xa3, 0x72, 0x0d, 0x7a, 0xbd, 0x0b, 0xa4, 0x0d, 0x90, 0x47, 0xff, 0x0a,
];
/// Canonical UTF-8 preimage for [`QWEN3_ROPE_KV_SCHEDULE_ID_V1`].
pub const QWEN3_ROPE_KV_SCHEDULE_ID_PREIMAGE_V1: &str =
    "fe2o3.qwen3.rope_paged_kv.schedule.wave64.split_half.exclusive_pages.v1";
/// Stable schedule schema identity: SHA-256 of its canonical preimage.
pub const QWEN3_ROPE_KV_SCHEDULE_ID_V1: [u8; 32] = [
    0xa2, 0x18, 0xfd, 0x31, 0xa2, 0x05, 0x5d, 0x36, 0x9e, 0xc9, 0xbe, 0xee, 0x8b, 0x33, 0x8e, 0xee,
    0x99, 0x19, 0xa0, 0x14, 0x37, 0xaf, 0x5a, 0x42, 0x55, 0xf5, 0x39, 0xd5, 0x83, 0x80, 0xe7, 0xf7,
];

/// Qwen3 model role whose exact geometry is selected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Qwen3ModelRoleV1 {
    /// Pinned Qwen3-8B target model.
    Target8B,
    /// Pinned Qwen3-0.6B draft model.
    Draft06B,
}

/// Exact model geometry relevant to RoPE and KV writes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Qwen3RopeKvGeometryV1 {
    /// Transformer layer count.
    pub layers: u16,
    /// Query-head count.
    pub query_heads: u16,
    /// Key/value-head count.
    pub kv_heads: u16,
    /// Dimension of every query, key, and value head.
    pub head_dimension: u16,
    /// Dimension rotated by RoPE, equal to the full head dimension.
    pub rotary_dimension: u16,
    /// Number of query heads sharing one key/value head.
    pub gqa_group_size: u16,
}

impl Qwen3ModelRoleV1 {
    /// Returns the one exact geometry admitted for this role.
    #[must_use]
    pub const fn geometry(self) -> Qwen3RopeKvGeometryV1 {
        match self {
            Self::Target8B => Qwen3RopeKvGeometryV1 {
                layers: QWEN3_TARGET_LAYERS_V1,
                query_heads: QWEN3_TARGET_QUERY_HEADS_V1,
                kv_heads: QWEN3_KV_HEADS_V1,
                head_dimension: QWEN3_HEAD_DIMENSION_V1,
                rotary_dimension: QWEN3_HEAD_DIMENSION_V1,
                gqa_group_size: 4,
            },
            Self::Draft06B => Qwen3RopeKvGeometryV1 {
                layers: QWEN3_DRAFT_LAYERS_V1,
                query_heads: QWEN3_DRAFT_QUERY_HEADS_V1,
                kv_heads: QWEN3_KV_HEADS_V1,
                head_dimension: QWEN3_HEAD_DIMENSION_V1,
                rotary_dimension: QWEN3_HEAD_DIMENSION_V1,
                gqa_group_size: 2,
            },
        }
    }
}

/// Finite admitted active-sequence buckets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SequenceBucketV1 {
    /// One active sequence.
    S1,
    /// Four active sequences.
    S4,
    /// Sixteen active sequences.
    S16,
    /// Thirty-two active sequences.
    S32,
}

impl SequenceBucketV1 {
    /// Returns the exact active-sequence count.
    #[must_use]
    pub const fn sequences(self) -> u16 {
        match self {
            Self::S1 => 1,
            Self::S4 => 4,
            Self::S16 => 16,
            Self::S32 => 32,
        }
    }
}

/// Finite active-token extents admitted by M1 plans.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenBucketV1 {
    /// One token.
    T1,
    /// Two tokens.
    T2,
    /// Three tokens.
    T3,
    /// Four tokens.
    T4,
    /// Five tokens.
    T5,
    /// Eight tokens.
    T8,
    /// Nine tokens.
    T9,
    /// Sixteen tokens.
    T16,
    /// Seventeen tokens.
    T17,
    /// 128 tokens.
    T128,
    /// 512 tokens.
    T512,
    /// 2048 tokens.
    T2048,
    /// 8192 tokens.
    T8192,
}

impl TokenBucketV1 {
    /// Returns the exact token count.
    #[must_use]
    pub const fn tokens(self) -> u32 {
        match self {
            Self::T1 => 1,
            Self::T2 => 2,
            Self::T3 => 3,
            Self::T4 => 4,
            Self::T5 => 5,
            Self::T8 => 8,
            Self::T9 => 9,
            Self::T16 => 16,
            Self::T17 => 17,
            Self::T128 => 128,
            Self::T512 => 512,
            Self::T2048 => 2_048,
            Self::T8192 => 8_192,
        }
    }
}

/// Finite logical context-capacity buckets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextBucketV1 {
    /// 128-token context.
    C128,
    /// 1024-token context.
    C1024,
    /// 4096-token context.
    C4096,
    /// 8192-token context.
    C8192,
}

impl ContextBucketV1 {
    /// Returns the exact context capacity.
    #[must_use]
    pub const fn tokens(self) -> u32 {
        match self {
            Self::C128 => 128,
            Self::C1024 => 1_024,
            Self::C4096 => 4_096,
            Self::C8192 => 8_192,
        }
    }
}

/// Finite physical page-size buckets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageBucketV1 {
    /// Sixteen tokens per page; the Ferric canonical bundle default.
    P16,
    /// Sixty-four tokens per page.
    P64,
    /// 256 tokens per page; the M1 envelope maximum.
    P256,
}

impl PageBucketV1 {
    /// Returns the exact number of tokens in one page.
    #[must_use]
    pub const fn tokens(self) -> u16 {
        match self {
            Self::P16 => 16,
            Self::P64 => 64,
            Self::P256 => 256,
        }
    }
}

/// Exact rotary dimension-pairing policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RotaryPairingPolicyV1 {
    /// Pair dimension `i` with `i + 64`, matching Qwen3 `rotate_half`.
    SplitHalfD128,
}

/// Exact absolute-position policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RotaryPositionPolicyV1 {
    /// Zero-based absolute position, restricted to `0..8192`.
    AbsoluteZeroBasedBelow8192,
}

/// Exact rotary-frequency policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RotaryFrequencyPolicyV1 {
    /// Frequency base, exactly 1,000,000.
    pub theta: u32,
    /// Exponent numerator multiplier in `theta^(-2*i/128)`.
    pub exponent_numerator_multiplier: u16,
    /// Exponent denominator, exactly the head dimension.
    pub exponent_denominator: u16,
}

/// Logical memory region named by the operator contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RopeKvMemoryRegionV1 {
    /// Query input.
    QueryInput,
    /// Key input.
    KeyInput,
    /// Value input.
    ValueInput,
    /// Absolute-position input.
    Positions,
    /// Rotated query output.
    RotatedQueryOutput,
    /// Rotated key output.
    RotatedKeyOutput,
    /// Paged key-cache allocation.
    KeyCache,
    /// Paged value-cache allocation.
    ValueCache,
}

/// Access kind required for one logical region.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RopeKvAccessV1 {
    /// Initialized read.
    Read,
    /// Exclusive write.
    Write,
}

/// One ordered logical memory effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RopeKvMemoryEffectV1 {
    /// Logical region.
    pub region: RopeKvMemoryRegionV1,
    /// Required access.
    pub access: RopeKvAccessV1,
    /// Reads require initialized values.
    pub requires_initialized: bool,
    /// Writes require unique live ownership.
    pub requires_exclusive_owner: bool,
}

/// Conditional race premises recorded by the structural schedule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RopeKvRaceContractV1 {
    /// Every rotated-Q coordinate has one logical writer.
    pub query_coordinate_single_writer: bool,
    /// Every rotated-K coordinate has one logical writer.
    pub key_coordinate_single_writer: bool,
    /// Physical page mappings are injective within one table.
    pub physical_pages_unique: bool,
    /// Key and value pools are distinct allocations.
    pub key_value_allocations_disjoint: bool,
    /// The page-table owner is exclusive for the write generation.
    pub exclusive_page_owner_required: bool,
    /// The schedule uses no atomics.
    pub atomics: u16,
    /// The schedule uses no workgroup barrier.
    pub barriers: u16,
}

/// Finite resource envelope for the structural schedule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RopeKvResourceContractV1 {
    /// Maximum active sequences.
    pub max_sequences: u16,
    /// Maximum tokens processed per sequence.
    pub max_active_tokens: u32,
    /// Maximum logical context.
    pub max_context_tokens: u32,
    /// Maximum page-table entries.
    pub max_page_table_entries: u16,
    /// Maximum query heads.
    pub max_query_heads: u16,
    /// Maximum KV heads.
    pub max_kv_heads: u16,
    /// Exact head dimension.
    pub head_dimension: u16,
    /// Structural Wave64 schedule width.
    pub wave_width: u16,
    /// Required static LDS bytes.
    pub static_lds_bytes: u32,
    /// Upper bound on scalar coordinate work in one sequence.
    pub max_coordinate_work: u64,
}

/// Explicit absence of executable authority at this foundation boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RopeKvAuthorityBoundaryV1 {
    /// Whether an authenticated compiler artifact is exposed.
    pub artifact_authority: bool,
    /// Whether a loaded-kernel capability is exposed.
    pub load_authority: bool,
    /// Whether a dispatch or launch capability is exposed.
    pub launch_authority: bool,
    /// Whether source-to-Kernel-IR refinement is claimed.
    pub source_to_kernel_ir_refinement: bool,
    /// Whether Kernel-IR-to-machine refinement is claimed.
    pub kernel_ir_to_machine_refinement: bool,
    /// Whether whole KV-system refinement is claimed.
    pub kv_system_refinement: bool,
}

/// Complete finite structural candidate and schedule identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Qwen3RopeKvCandidateV1 {
    /// Structural family identity.
    pub family_id: [u8; 32],
    /// Structural schema identity.
    pub candidate_schema_id: [u8; 32],
    /// Structural schedule identity.
    pub schedule_id: [u8; 32],
    /// Exact processor name.
    pub processor: &'static str,
    /// Exact target features.
    pub target_features: &'static str,
    /// Selected model role.
    pub role: Qwen3ModelRoleV1,
    /// Exact role geometry.
    pub geometry: Qwen3RopeKvGeometryV1,
    /// Active-sequence bucket.
    pub sequences: SequenceBucketV1,
    /// Active-token bucket.
    pub active_tokens: TokenBucketV1,
    /// Context-capacity bucket.
    pub context: ContextBucketV1,
    /// Page-size bucket.
    pub page: PageBucketV1,
    /// Rotary pairing policy.
    pub pairing: RotaryPairingPolicyV1,
    /// Rotary position policy.
    pub position: RotaryPositionPolicyV1,
    /// Rotary frequency policy.
    pub frequency: RotaryFrequencyPolicyV1,
    /// Ordered memory-effect contract.
    pub effects: [RopeKvMemoryEffectV1; 8],
    /// Conditional race contract.
    pub race: RopeKvRaceContractV1,
    /// Finite resource contract.
    pub resources: RopeKvResourceContractV1,
    /// Explicit non-authority boundary.
    pub authority: RopeKvAuthorityBoundaryV1,
}

const fn effect(
    region: RopeKvMemoryRegionV1,
    access: RopeKvAccessV1,
    requires_initialized: bool,
    requires_exclusive_owner: bool,
) -> RopeKvMemoryEffectV1 {
    RopeKvMemoryEffectV1 {
        region,
        access,
        requires_initialized,
        requires_exclusive_owner,
    }
}

/// Constructs the only structural candidate admitted for the selected finite buckets.
#[must_use]
pub const fn exact_qwen3_rope_kv_candidate_v1(
    role: Qwen3ModelRoleV1,
    sequences: SequenceBucketV1,
    active_tokens: TokenBucketV1,
    context: ContextBucketV1,
    page: PageBucketV1,
) -> Qwen3RopeKvCandidateV1 {
    let query_heads = role.geometry().query_heads as u64;
    let kv_heads = role.geometry().kv_heads as u64;
    let tokens = active_tokens.tokens() as u64;
    let rotary_pairs = QWEN3_ROPE_HALF_DIMENSION_V1 as u64;
    let kv_components = QWEN3_HEAD_DIMENSION_V1 as u64;
    Qwen3RopeKvCandidateV1 {
        family_id: QWEN3_ROPE_KV_FAMILY_ID_V1,
        candidate_schema_id: QWEN3_ROPE_KV_CANDIDATE_SCHEMA_ID_V1,
        schedule_id: QWEN3_ROPE_KV_SCHEDULE_ID_V1,
        processor: "gfx942",
        target_features: "+wavefrontsize64,-xnack",
        role,
        geometry: role.geometry(),
        sequences,
        active_tokens,
        context,
        page,
        pairing: RotaryPairingPolicyV1::SplitHalfD128,
        position: RotaryPositionPolicyV1::AbsoluteZeroBasedBelow8192,
        frequency: RotaryFrequencyPolicyV1 {
            theta: QWEN3_ROPE_THETA_V1,
            exponent_numerator_multiplier: 2,
            exponent_denominator: QWEN3_HEAD_DIMENSION_V1,
        },
        effects: [
            effect(
                RopeKvMemoryRegionV1::QueryInput,
                RopeKvAccessV1::Read,
                true,
                false,
            ),
            effect(
                RopeKvMemoryRegionV1::KeyInput,
                RopeKvAccessV1::Read,
                true,
                false,
            ),
            effect(
                RopeKvMemoryRegionV1::ValueInput,
                RopeKvAccessV1::Read,
                true,
                false,
            ),
            effect(
                RopeKvMemoryRegionV1::Positions,
                RopeKvAccessV1::Read,
                true,
                false,
            ),
            effect(
                RopeKvMemoryRegionV1::RotatedQueryOutput,
                RopeKvAccessV1::Write,
                false,
                true,
            ),
            effect(
                RopeKvMemoryRegionV1::RotatedKeyOutput,
                RopeKvAccessV1::Write,
                false,
                true,
            ),
            effect(
                RopeKvMemoryRegionV1::KeyCache,
                RopeKvAccessV1::Write,
                false,
                true,
            ),
            effect(
                RopeKvMemoryRegionV1::ValueCache,
                RopeKvAccessV1::Write,
                false,
                true,
            ),
        ],
        race: RopeKvRaceContractV1 {
            query_coordinate_single_writer: true,
            key_coordinate_single_writer: true,
            physical_pages_unique: true,
            key_value_allocations_disjoint: true,
            exclusive_page_owner_required: true,
            atomics: 0,
            barriers: 0,
        },
        resources: RopeKvResourceContractV1 {
            max_sequences: 32,
            max_active_tokens: M1_MAX_CONTEXT_TOKENS_V1,
            max_context_tokens: M1_MAX_CONTEXT_TOKENS_V1,
            max_page_table_entries: M1_MAX_PAGE_TABLE_ENTRIES_V1 as u16,
            max_query_heads: QWEN3_TARGET_QUERY_HEADS_V1,
            max_kv_heads: QWEN3_KV_HEADS_V1,
            head_dimension: QWEN3_HEAD_DIMENSION_V1,
            wave_width: 64,
            static_lds_bytes: 0,
            max_coordinate_work: tokens * (query_heads + kv_heads) * rotary_pairs
                + tokens * kv_heads * kv_components,
        },
        authority: RopeKvAuthorityBoundaryV1 {
            artifact_authority: false,
            load_authority: false,
            launch_authority: false,
            source_to_kernel_ir_refinement: false,
            kernel_ir_to_machine_refinement: false,
            kv_system_refinement: false,
        },
    }
}

/// Structural candidate-admission failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateErrorV1 {
    /// Candidate identity, schedule, effect, resource, or non-authority field drifted.
    NonCanonical,
    /// Active-token extent exceeds the selected context capacity.
    TokensExceedContext,
    /// Context capacity is not divisible by the selected page size.
    PageDoesNotDivideContext,
}

impl fmt::Display for CandidateErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonCanonical => formatter.write_str("Qwen3 RoPE/KV candidate is noncanonical"),
            Self::TokensExceedContext => {
                formatter.write_str("active-token bucket exceeds the context bucket")
            }
            Self::PageDoesNotDivideContext => {
                formatter.write_str("page bucket does not divide the context bucket")
            }
        }
    }
}

impl Error for CandidateErrorV1 {}

/// Validates exact candidate identity and every structural schedule field.
pub fn validate_qwen3_rope_kv_candidate_v1(
    candidate: &Qwen3RopeKvCandidateV1,
) -> Result<(), CandidateErrorV1> {
    if candidate.active_tokens.tokens() > candidate.context.tokens() {
        return Err(CandidateErrorV1::TokensExceedContext);
    }
    if !candidate
        .context
        .tokens()
        .is_multiple_of(u32::from(candidate.page.tokens()))
    {
        return Err(CandidateErrorV1::PageDoesNotDivideContext);
    }
    let expected = exact_qwen3_rope_kv_candidate_v1(
        candidate.role,
        candidate.sequences,
        candidate.active_tokens,
        candidate.context,
        candidate.page,
    );
    if candidate != &expected {
        return Err(CandidateErrorV1::NonCanonical);
    }
    Ok(())
}
