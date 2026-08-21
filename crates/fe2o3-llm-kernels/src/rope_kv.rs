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

/// Stable request owner identity used by the exclusive-write premise.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KvOwnerIdentityV1(pub [u8; 16]);

impl KvOwnerIdentityV1 {
    /// Returns whether the identity contains a nonzero byte.
    #[must_use]
    pub fn is_present(self) -> bool {
        self.0.iter().any(|byte| *byte != 0)
    }
}

/// Target page-table generation in the target KV namespace.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TargetPageTableGenerationV1 {
    /// Stable target pool identity.
    pub pool_id: [u8; 16],
    /// Nonzero generation counter.
    pub generation: u64,
}

/// Draft page-table generation in the disjoint draft KV namespace.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DraftPageTableGenerationV1 {
    /// Stable draft pool identity.
    pub pool_id: [u8; 16],
    /// Nonzero generation counter.
    pub generation: u64,
}

/// Role-typed page-table generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageTableGenerationV1 {
    /// Target cache generation.
    Target(TargetPageTableGenerationV1),
    /// Draft cache generation.
    Draft(DraftPageTableGenerationV1),
}

impl PageTableGenerationV1 {
    /// Returns the role selected by the generation namespace.
    #[must_use]
    pub const fn role(self) -> Qwen3ModelRoleV1 {
        match self {
            Self::Target(_) => Qwen3ModelRoleV1::Target8B,
            Self::Draft(_) => Qwen3ModelRoleV1::Draft06B,
        }
    }

    /// Returns the generation counter.
    #[must_use]
    pub const fn value(self) -> u64 {
        match self {
            Self::Target(generation) => generation.generation,
            Self::Draft(generation) => generation.generation,
        }
    }

    /// Returns the stable pool identity.
    #[must_use]
    pub const fn pool_id(self) -> [u8; 16] {
        match self {
            Self::Target(generation) => generation.pool_id,
            Self::Draft(generation) => generation.pool_id,
        }
    }

    fn is_present(self) -> bool {
        self.value() != 0 && self.pool_id().iter().any(|byte| *byte != 0)
    }
}

/// One logical-to-physical page-table entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageTableEntryV1 {
    /// Exact zero-based logical page index.
    pub logical_page: u16,
    /// Physical page selected for this logical page.
    pub physical_page: u32,
    /// Physical page generation, equal to the table generation.
    pub physical_generation: u64,
    /// Initialized prefix length inside this page.
    pub initialized_tokens: u16,
    /// Sole live owner permitted to append to the page.
    pub exclusive_owner: KvOwnerIdentityV1,
}

/// Exact role-typed page table for one sequence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Qwen3PageTableV1 {
    /// Target or draft table generation.
    pub generation: PageTableGenerationV1,
    /// Logical context capacity.
    pub context: ContextBucketV1,
    /// Physical page size.
    pub page: PageBucketV1,
    /// Complete page table, including mapped but uninitialized suffix pages.
    pub entries: Vec<PageTableEntryV1>,
}

/// Page-table structural or freshness failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageTableErrorV1 {
    /// Expected or actual generation is absent.
    MissingGeneration,
    /// The role, pool identity, or generation counter is stale.
    StaleGeneration,
    /// Expected or entry owner is absent.
    MissingOwner,
    /// An entry is owned by a different request.
    StaleOwner,
    /// Page-table length is not exact for the page/context buckets.
    EntryCount,
    /// A logical page index is missing, duplicated, or reordered.
    LogicalPageOrder,
    /// A physical page is outside the finite bound.
    PhysicalPageOutOfBounds,
    /// Two logical pages alias one physical page.
    DuplicatePhysicalPage,
    /// An entry carries a stale physical generation.
    StalePhysicalGeneration,
    /// Initialized tokens exceed the physical page size.
    InitializedOutOfBounds,
    /// Initialized entries do not form one contiguous logical prefix.
    NonPrefixInitialization,
    /// A requested logical token is outside the context.
    LogicalTokenOutOfBounds,
    /// A requested logical token has not been initialized.
    UninitializedRead,
    /// Arithmetic used to derive a coordinate overflowed.
    ArithmeticOverflow,
}

impl fmt::Display for PageTableErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Qwen3 page-table contract failure: {self:?}")
    }
}

impl Error for PageTableErrorV1 {}

fn expected_entry_count(context: ContextBucketV1, page: PageBucketV1) -> usize {
    (context.tokens() / u32::from(page.tokens())) as usize
}

impl Qwen3PageTableV1 {
    fn validate_structure(&self) -> Result<(), PageTableErrorV1> {
        if !self.generation.is_present() {
            return Err(PageTableErrorV1::MissingGeneration);
        }
        let expected_entries = expected_entry_count(self.context, self.page);
        if self.entries.len() != expected_entries
            || self.entries.len() > M1_MAX_PAGE_TABLE_ENTRIES_V1
        {
            return Err(PageTableErrorV1::EntryCount);
        }
        let page_tokens = self.page.tokens();
        let mut prefix_closed = false;
        for (index, entry) in self.entries.iter().enumerate() {
            if usize::from(entry.logical_page) != index {
                return Err(PageTableErrorV1::LogicalPageOrder);
            }
            if entry.physical_page >= M1_MAX_PHYSICAL_PAGES_V1 {
                return Err(PageTableErrorV1::PhysicalPageOutOfBounds);
            }
            if self.entries[..index]
                .iter()
                .any(|previous| previous.physical_page == entry.physical_page)
            {
                return Err(PageTableErrorV1::DuplicatePhysicalPage);
            }
            if entry.physical_generation != self.generation.value() {
                return Err(PageTableErrorV1::StalePhysicalGeneration);
            }
            if !entry.exclusive_owner.is_present() {
                return Err(PageTableErrorV1::MissingOwner);
            }
            if entry.initialized_tokens > page_tokens {
                return Err(PageTableErrorV1::InitializedOutOfBounds);
            }
            if prefix_closed && entry.initialized_tokens != 0 {
                return Err(PageTableErrorV1::NonPrefixInitialization);
            }
            if entry.initialized_tokens < page_tokens {
                prefix_closed = true;
            }
        }
        Ok(())
    }

    /// Validates structure, exact expected generation, and exclusive owner.
    pub fn validate_against(
        &self,
        expected_generation: PageTableGenerationV1,
        expected_owner: KvOwnerIdentityV1,
    ) -> Result<(), PageTableErrorV1> {
        if !expected_generation.is_present() {
            return Err(PageTableErrorV1::MissingGeneration);
        }
        if !expected_owner.is_present() {
            return Err(PageTableErrorV1::MissingOwner);
        }
        self.validate_structure()?;
        if self.generation != expected_generation {
            return Err(PageTableErrorV1::StaleGeneration);
        }
        if self
            .entries
            .iter()
            .any(|entry| entry.exclusive_owner != expected_owner)
        {
            return Err(PageTableErrorV1::StaleOwner);
        }
        Ok(())
    }

    /// Returns the total initialized logical prefix length.
    pub fn initialized_prefix_tokens(&self) -> Result<u32, PageTableErrorV1> {
        self.validate_structure()?;
        self.entries.iter().try_fold(0_u32, |sum, entry| {
            sum.checked_add(u32::from(entry.initialized_tokens))
                .ok_or(PageTableErrorV1::ArithmeticOverflow)
        })
    }

    /// Maps any in-capacity logical token to its exact physical page and slot.
    pub fn logical_to_physical(
        &self,
        logical_token: u32,
    ) -> Result<KvPhysicalLocationV1, PageTableErrorV1> {
        self.validate_structure()?;
        if logical_token >= self.context.tokens() {
            return Err(PageTableErrorV1::LogicalTokenOutOfBounds);
        }
        let page_tokens = u32::from(self.page.tokens());
        let logical_page = logical_token / page_tokens;
        let slot = logical_token % page_tokens;
        let entry = self
            .entries
            .get(logical_page as usize)
            .ok_or(PageTableErrorV1::LogicalTokenOutOfBounds)?;
        if u32::from(entry.logical_page) != logical_page {
            return Err(PageTableErrorV1::LogicalPageOrder);
        }
        Ok(KvPhysicalLocationV1 {
            physical_page: entry.physical_page,
            token_slot: slot as u16,
            physical_generation: entry.physical_generation,
        })
    }

    /// Maps an initialized logical read and rejects the uninitialized suffix.
    pub fn initialized_logical_to_physical(
        &self,
        logical_token: u32,
    ) -> Result<KvPhysicalLocationV1, PageTableErrorV1> {
        let initialized = self.initialized_prefix_tokens()?;
        if logical_token >= initialized {
            return Err(PageTableErrorV1::UninitializedRead);
        }
        self.logical_to_physical(logical_token)
    }
}

/// Exact physical location of one logical token.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KvPhysicalLocationV1 {
    /// Physical page index.
    pub physical_page: u32,
    /// Token slot within the page.
    pub token_slot: u16,
    /// Physical page generation.
    pub physical_generation: u64,
}

/// Exact append descriptor consumed by the paged-KV coordinate model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Qwen3KvWriteDescriptorV1 {
    /// Exact structural candidate.
    pub candidate: Qwen3RopeKvCandidateV1,
    /// Exact independently expected target/draft page-table generation.
    pub generation: PageTableGenerationV1,
    /// Sole request owner of every writable page.
    pub owner: KvOwnerIdentityV1,
    /// Sequence selected within the finite active-sequence bucket.
    pub sequence_index: u16,
    /// Transformer layer selected for the write.
    pub layer: u16,
    /// First logical token appended by this descriptor.
    pub logical_start: u32,
}

/// Independently trusted identities against which one write is admitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Qwen3KvWriteExpectationV1 {
    /// Exact structural candidate expected by the caller.
    pub candidate: Qwen3RopeKvCandidateV1,
    /// Exact target/draft pool generation expected by the caller.
    pub generation: PageTableGenerationV1,
    /// Exact exclusive request owner expected by the caller.
    pub owner: KvOwnerIdentityV1,
}

/// Exact KV descriptor-admission failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KvWriteErrorV1 {
    /// Structural candidate validation failed.
    Candidate(CandidateErrorV1),
    /// Page-table validation failed.
    PageTable(PageTableErrorV1),
    /// Descriptor role and generation namespace differ.
    RoleGenerationMismatch,
    /// Descriptor owner differs from the independently expected owner.
    OwnerMismatch,
    /// Descriptor page/context buckets differ from the table.
    TableBucketMismatch,
    /// Sequence index is outside the active-sequence bucket.
    SequenceOutOfBounds,
    /// Layer is outside the selected target/draft geometry.
    LayerOutOfBounds,
    /// The write does not begin exactly at the initialized logical prefix.
    NonAppendWrite,
    /// The write would exceed the logical context capacity.
    WriteExceedsContext,
    /// A local token, KV head, or component is outside the descriptor extent.
    CoordinateOutOfBounds,
    /// Rotated-key or value input does not have exact `[tokens][kv_heads][128]` extent.
    InputExtent,
    /// Rotated-key or value input contains NaN or infinity.
    NonFiniteInput {
        /// Index in the concatenated key-then-value input sequence.
        index: usize,
    },
    /// Coordinate arithmetic overflowed.
    ArithmeticOverflow,
}

impl fmt::Display for KvWriteErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Qwen3 paged-KV write contract failure: {self:?}")
    }
}

impl Error for KvWriteErrorV1 {}

impl From<CandidateErrorV1> for KvWriteErrorV1 {
    fn from(value: CandidateErrorV1) -> Self {
        Self::Candidate(value)
    }
}

impl From<PageTableErrorV1> for KvWriteErrorV1 {
    fn from(value: PageTableErrorV1) -> Self {
        Self::PageTable(value)
    }
}

/// Validates exact candidate identity plus append, initialization, ownership,
/// generation, role, bucket, sequence, layer, and capacity premises.
pub fn validate_qwen3_kv_write_v1(
    descriptor: &Qwen3KvWriteDescriptorV1,
    table: &Qwen3PageTableV1,
    expectation: &Qwen3KvWriteExpectationV1,
) -> Result<(), KvWriteErrorV1> {
    validate_qwen3_rope_kv_candidate_v1(&descriptor.candidate)?;
    validate_qwen3_rope_kv_candidate_v1(&expectation.candidate)?;
    if descriptor.candidate != expectation.candidate {
        return Err(KvWriteErrorV1::Candidate(CandidateErrorV1::NonCanonical));
    }
    if descriptor.generation != expectation.generation
        || descriptor.generation.role() != descriptor.candidate.role
    {
        return Err(KvWriteErrorV1::RoleGenerationMismatch);
    }
    if descriptor.owner != expectation.owner {
        return Err(KvWriteErrorV1::OwnerMismatch);
    }
    if table.context != descriptor.candidate.context || table.page != descriptor.candidate.page {
        return Err(KvWriteErrorV1::TableBucketMismatch);
    }
    table.validate_against(expectation.generation, expectation.owner)?;
    if descriptor.sequence_index >= descriptor.candidate.sequences.sequences() {
        return Err(KvWriteErrorV1::SequenceOutOfBounds);
    }
    if descriptor.layer >= descriptor.candidate.geometry.layers {
        return Err(KvWriteErrorV1::LayerOutOfBounds);
    }
    let initialized = table.initialized_prefix_tokens()?;
    if descriptor.logical_start != initialized {
        return Err(KvWriteErrorV1::NonAppendWrite);
    }
    let end = descriptor
        .logical_start
        .checked_add(descriptor.candidate.active_tokens.tokens())
        .ok_or(KvWriteErrorV1::ArithmeticOverflow)?;
    if end > descriptor.candidate.context.tokens() {
        return Err(KvWriteErrorV1::WriteExceedsContext);
    }
    Ok(())
}

/// Physical key/value element coordinate for one append component.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Qwen3KvWriteCoordinateV1 {
    /// Logical token position.
    pub logical_token: u32,
    /// Physical page and slot.
    pub location: KvPhysicalLocationV1,
    /// Transformer layer.
    pub layer: u16,
    /// KV head.
    pub kv_head: u16,
    /// Component within the 128-element head.
    pub component: u16,
    /// Element offset in a per-layer key or value physical-page pool.
    pub pool_element_offset: u64,
}

fn qwen3_kv_write_coordinate_after_validation_v1(
    descriptor: &Qwen3KvWriteDescriptorV1,
    table: &Qwen3PageTableV1,
    local_token: u32,
    kv_head: u16,
    component: u16,
) -> Result<Qwen3KvWriteCoordinateV1, KvWriteErrorV1> {
    let logical_token = descriptor
        .logical_start
        .checked_add(local_token)
        .ok_or(KvWriteErrorV1::ArithmeticOverflow)?;
    let page_tokens_u32 = u32::from(table.page.tokens());
    let logical_page = logical_token / page_tokens_u32;
    let entry = table
        .entries
        .get(logical_page as usize)
        .ok_or(PageTableErrorV1::LogicalTokenOutOfBounds)?;
    let location = KvPhysicalLocationV1 {
        physical_page: entry.physical_page,
        token_slot: (logical_token % page_tokens_u32) as u16,
        physical_generation: entry.physical_generation,
    };
    let page_tokens = u64::from(page_tokens_u32);
    let kv_heads = u64::from(descriptor.candidate.geometry.kv_heads);
    let head_dimension = u64::from(descriptor.candidate.geometry.head_dimension);
    let physical_token = u64::from(location.physical_page)
        .checked_mul(page_tokens)
        .and_then(|base| base.checked_add(u64::from(location.token_slot)))
        .ok_or(KvWriteErrorV1::ArithmeticOverflow)?;
    let pool_element_offset = physical_token
        .checked_mul(kv_heads)
        .and_then(|base| base.checked_add(u64::from(kv_head)))
        .and_then(|head| head.checked_mul(head_dimension))
        .and_then(|base| base.checked_add(u64::from(component)))
        .ok_or(KvWriteErrorV1::ArithmeticOverflow)?;
    Ok(Qwen3KvWriteCoordinateV1 {
        logical_token,
        location,
        layer: descriptor.layer,
        kv_head,
        component,
        pool_element_offset,
    })
}

/// Projects one in-descriptor logical KV component to the exact physical pool offset.
pub fn qwen3_kv_write_coordinate_v1(
    descriptor: &Qwen3KvWriteDescriptorV1,
    table: &Qwen3PageTableV1,
    expectation: &Qwen3KvWriteExpectationV1,
    local_token: u32,
    kv_head: u16,
    component: u16,
) -> Result<Qwen3KvWriteCoordinateV1, KvWriteErrorV1> {
    validate_qwen3_kv_write_v1(descriptor, table, expectation)?;
    if local_token >= descriptor.candidate.active_tokens.tokens()
        || kv_head >= descriptor.candidate.geometry.kv_heads
        || component >= descriptor.candidate.geometry.head_dimension
    {
        return Err(KvWriteErrorV1::CoordinateOutOfBounds);
    }
    qwen3_kv_write_coordinate_after_validation_v1(
        descriptor,
        table,
        local_token,
        kv_head,
        component,
    )
}

/// One pure CPU reference record for matching key/value cache coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Qwen3KvWriteElementV1 {
    /// Exact physical coordinate shared by the key and value cache pools.
    pub coordinate: Qwen3KvWriteCoordinateV1,
    /// Rotated-key value written to the key pool at this coordinate.
    pub rotated_key: f64,
    /// Unrotated value written to the value pool at this coordinate.
    pub value: f64,
}

/// Complete pure CPU reference projection for one bounded paged-KV append.
#[derive(Clone, Debug, PartialEq)]
pub struct Qwen3PagedKvWriteReferenceV1 {
    /// Records in canonical token-major, KV-head-major, component-major order.
    pub elements: Vec<Qwen3KvWriteElementV1>,
}

/// Projects exact rotated-key and value tensors into physical write records.
///
/// This pure `f64` model does not commit memory and is not an IEEE-754, GPU,
/// compiler, launch, or KV-system refinement claim.
pub fn qwen3_paged_kv_write_reference_v1(
    descriptor: &Qwen3KvWriteDescriptorV1,
    table: &Qwen3PageTableV1,
    expectation: &Qwen3KvWriteExpectationV1,
    rotated_key: &[f64],
    value: &[f64],
) -> Result<Qwen3PagedKvWriteReferenceV1, KvWriteErrorV1> {
    validate_qwen3_kv_write_v1(descriptor, table, expectation)?;
    let tokens = usize::try_from(descriptor.candidate.active_tokens.tokens())
        .map_err(|_| KvWriteErrorV1::ArithmeticOverflow)?;
    let heads = usize::from(descriptor.candidate.geometry.kv_heads);
    let dimension = usize::from(descriptor.candidate.geometry.head_dimension);
    let extent = tokens
        .checked_mul(heads)
        .and_then(|value| value.checked_mul(dimension))
        .ok_or(KvWriteErrorV1::ArithmeticOverflow)?;
    if rotated_key.len() != extent || value.len() != extent {
        return Err(KvWriteErrorV1::InputExtent);
    }
    for (index, element) in rotated_key.iter().chain(value).enumerate() {
        if !element.is_finite() {
            return Err(KvWriteErrorV1::NonFiniteInput { index });
        }
    }

    let mut elements = Vec::with_capacity(extent);
    for local_token in 0..descriptor.candidate.active_tokens.tokens() {
        for kv_head in 0..descriptor.candidate.geometry.kv_heads {
            for component in 0..descriptor.candidate.geometry.head_dimension {
                let index = (local_token as usize * heads + usize::from(kv_head)) * dimension
                    + usize::from(component);
                let coordinate = qwen3_kv_write_coordinate_after_validation_v1(
                    descriptor,
                    table,
                    local_token,
                    kv_head,
                    component,
                )?;
                elements.push(Qwen3KvWriteElementV1 {
                    coordinate,
                    rotated_key: rotated_key[index],
                    value: value[index],
                });
            }
        }
    }
    Ok(Qwen3PagedKvWriteReferenceV1 { elements })
}

/// Returns a new page-table state whose initialized prefix includes the exact append.
///
/// Physical pages, generations, ownership, and all untouched initialized
/// counts are framed. This is a pure host projection, not a device commit.
pub fn project_qwen3_kv_write_v1(
    descriptor: &Qwen3KvWriteDescriptorV1,
    table: &Qwen3PageTableV1,
    expectation: &Qwen3KvWriteExpectationV1,
) -> Result<Qwen3PageTableV1, KvWriteErrorV1> {
    validate_qwen3_kv_write_v1(descriptor, table, expectation)?;
    let mut projected = table.clone();
    let new_prefix = descriptor
        .logical_start
        .checked_add(descriptor.candidate.active_tokens.tokens())
        .ok_or(KvWriteErrorV1::ArithmeticOverflow)?;
    let page_tokens = u32::from(projected.page.tokens());
    for entry in &mut projected.entries {
        let page_start = u32::from(entry.logical_page)
            .checked_mul(page_tokens)
            .ok_or(KvWriteErrorV1::ArithmeticOverflow)?;
        entry.initialized_tokens = if new_prefix <= page_start {
            0
        } else {
            new_prefix.saturating_sub(page_start).min(page_tokens) as u16
        };
    }
    projected.validate_against(expectation.generation, expectation.owner)?;
    Ok(projected)
}

/// Explicit independently expected target and draft page-table generations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Qwen3PageTableGenerationsV1 {
    /// Expected target generation.
    pub target: TargetPageTableGenerationV1,
    /// Expected draft generation.
    pub draft: DraftPageTableGenerationV1,
}

/// Target/draft generation-pair admission failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GenerationPairErrorV1 {
    /// At least one pool identity or generation counter is absent.
    Missing,
    /// Target and draft use the same pool identity instead of disjoint namespaces.
    AliasedPoolIdentity,
}

/// Validates nonzero, explicitly role-typed, disjoint target/draft generations.
pub fn validate_qwen3_page_table_generations_v1(
    generations: Qwen3PageTableGenerationsV1,
) -> Result<(), GenerationPairErrorV1> {
    let target = PageTableGenerationV1::Target(generations.target);
    let draft = PageTableGenerationV1::Draft(generations.draft);
    if !target.is_present() || !draft.is_present() {
        return Err(GenerationPairErrorV1::Missing);
    }
    if generations.target.pool_id == generations.draft.pool_id {
        return Err(GenerationPairErrorV1::AliasedPoolIdentity);
    }
    Ok(())
}

/// Exact split-half pair for one rotary dimension.
#[must_use]
pub const fn qwen3_rotary_pair_v1(dimension: u16) -> Option<u16> {
    if dimension < QWEN3_ROPE_HALF_DIMENSION_V1 {
        Some(dimension + QWEN3_ROPE_HALF_DIMENSION_V1)
    } else if dimension < QWEN3_HEAD_DIMENSION_V1 {
        Some(dimension - QWEN3_ROPE_HALF_DIMENSION_V1)
    } else {
        None
    }
}

/// Returns the inverse frequency `theta^(-2*i/128)` for one split-half pair.
#[must_use]
pub fn qwen3_rotary_inverse_frequency_v1(pair_index: u16) -> Option<f64> {
    if pair_index >= QWEN3_ROPE_HALF_DIMENSION_V1 {
        return None;
    }
    let exponent = -2.0 * f64::from(pair_index) / f64::from(QWEN3_HEAD_DIMENSION_V1);
    Some(f64::from(QWEN3_ROPE_THETA_V1).powf(exponent))
}

/// Returns the exact-model angle for an admitted absolute position and pair.
#[must_use]
pub fn qwen3_rotary_angle_v1(position: u32, pair_index: u16) -> Option<f64> {
    if position >= M1_MAX_CONTEXT_TOKENS_V1 {
        return None;
    }
    qwen3_rotary_inverse_frequency_v1(pair_index).map(|frequency| f64::from(position) * frequency)
}

/// CPU RoPE model failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RopeReferenceErrorV1 {
    /// Candidate was structurally invalid.
    Candidate(CandidateErrorV1),
    /// Position extent differs from the token bucket.
    PositionCount,
    /// Query extent differs from `[tokens][query_heads][128]`.
    QueryExtent,
    /// Key extent differs from `[tokens][kv_heads][128]`.
    KeyExtent,
    /// Position is outside the M1 absolute-position domain.
    PositionOutOfBounds {
        /// Token carrying the invalid position.
        token: usize,
    },
    /// Query or key input is NaN or infinite.
    NonFiniteInput {
        /// Flattened input index.
        index: usize,
    },
    /// Host trigonometry produced a non-finite output.
    NonFiniteOutput {
        /// Flattened output index.
        index: usize,
    },
    /// Shape arithmetic overflowed.
    ArithmeticOverflow,
}

impl fmt::Display for RopeReferenceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Qwen3 CPU RoPE model failure: {self:?}")
    }
}

impl Error for RopeReferenceErrorV1 {}

impl From<CandidateErrorV1> for RopeReferenceErrorV1 {
    fn from(value: CandidateErrorV1) -> Self {
        Self::Candidate(value)
    }
}

/// Rotated query and key tensors from the CPU model.
#[derive(Clone, Debug, PartialEq)]
pub struct Qwen3RopeOutputV1 {
    /// Flattened `[tokens][query_heads][128]` rotated queries.
    pub query: Vec<f64>,
    /// Flattened `[tokens][kv_heads][128]` rotated keys.
    pub key: Vec<f64>,
}

fn checked_tensor_extent(tokens: u32, heads: u16) -> Option<usize> {
    usize::try_from(tokens)
        .ok()?
        .checked_mul(usize::from(heads))?
        .checked_mul(usize::from(QWEN3_HEAD_DIMENSION_V1))
}

fn validate_rope_inputs(
    candidate: &Qwen3RopeKvCandidateV1,
    positions: &[u32],
    query: &[f64],
    key: &[f64],
) -> Result<(usize, usize), RopeReferenceErrorV1> {
    validate_qwen3_rope_kv_candidate_v1(candidate)?;
    let tokens = candidate.active_tokens.tokens();
    if positions.len() != tokens as usize {
        return Err(RopeReferenceErrorV1::PositionCount);
    }
    let query_extent = checked_tensor_extent(tokens, candidate.geometry.query_heads)
        .ok_or(RopeReferenceErrorV1::ArithmeticOverflow)?;
    let key_extent = checked_tensor_extent(tokens, candidate.geometry.kv_heads)
        .ok_or(RopeReferenceErrorV1::ArithmeticOverflow)?;
    if query.len() != query_extent {
        return Err(RopeReferenceErrorV1::QueryExtent);
    }
    if key.len() != key_extent {
        return Err(RopeReferenceErrorV1::KeyExtent);
    }
    for (token, position) in positions.iter().enumerate() {
        if *position >= M1_MAX_CONTEXT_TOKENS_V1 {
            return Err(RopeReferenceErrorV1::PositionOutOfBounds { token });
        }
    }
    for (index, value) in query.iter().chain(key).enumerate() {
        if !value.is_finite() {
            return Err(RopeReferenceErrorV1::NonFiniteInput { index });
        }
    }
    Ok((query_extent, key_extent))
}

fn tensor_base(token: usize, head: usize, heads: usize) -> usize {
    (token * heads + head) * usize::from(QWEN3_HEAD_DIMENSION_V1)
}

fn rotate_reference_dimensions(
    input: &[f64],
    output: &mut [f64],
    position: u32,
    base: usize,
) -> Result<(), RopeReferenceErrorV1> {
    for dimension in 0..QWEN3_HEAD_DIMENSION_V1 {
        let pair =
            qwen3_rotary_pair_v1(dimension).ok_or(RopeReferenceErrorV1::ArithmeticOverflow)?;
        let pair_index = dimension.min(pair);
        let angle = qwen3_rotary_angle_v1(position, pair_index)
            .ok_or(RopeReferenceErrorV1::PositionOutOfBounds { token: 0 })?;
        let (sine, cosine) = angle.sin_cos();
        let index = base + usize::from(dimension);
        let paired_index = base + usize::from(pair);
        output[index] = if dimension < QWEN3_ROPE_HALF_DIMENSION_V1 {
            input[index] * cosine - input[paired_index] * sine
        } else {
            input[index] * cosine + input[paired_index] * sine
        };
        if !output[index].is_finite() {
            return Err(RopeReferenceErrorV1::NonFiniteOutput { index });
        }
    }
    Ok(())
}

fn rotate_candidate_pairs(
    input: &[f64],
    output: &mut [f64],
    position: u32,
    base: usize,
) -> Result<(), RopeReferenceErrorV1> {
    for pair in 0..QWEN3_ROPE_HALF_DIMENSION_V1 {
        let angle = qwen3_rotary_angle_v1(position, pair)
            .ok_or(RopeReferenceErrorV1::PositionOutOfBounds { token: 0 })?;
        let (sine, cosine) = angle.sin_cos();
        let lower = base + usize::from(pair);
        let upper = lower + usize::from(QWEN3_ROPE_HALF_DIMENSION_V1);
        output[lower] = input[lower] * cosine - input[upper] * sine;
        output[upper] = input[upper] * cosine + input[lower] * sine;
        if !output[lower].is_finite() {
            return Err(RopeReferenceErrorV1::NonFiniteOutput { index: lower });
        }
        if !output[upper].is_finite() {
            return Err(RopeReferenceErrorV1::NonFiniteOutput { index: upper });
        }
    }
    Ok(())
}

type RotateHeadV1 = fn(&[f64], &mut [f64], u32, usize) -> Result<(), RopeReferenceErrorV1>;

fn rotate_tensor(
    input: &[f64],
    output: &mut [f64],
    positions: &[u32],
    heads: u16,
    rotate: RotateHeadV1,
) -> Result<(), RopeReferenceErrorV1> {
    let heads = usize::from(heads);
    for (token, position) in positions.iter().copied().enumerate() {
        for head in 0..heads {
            rotate(input, output, position, tensor_base(token, head, heads))?;
        }
    }
    Ok(())
}

/// Evaluates the dimension-oriented CPU `f64` Qwen3 RoPE reference.
///
/// This function is not an IEEE-754, BF16, FP32, OCML, compiler, or GPU
/// refinement claim.
pub fn qwen3_rope_reference_v1(
    candidate: &Qwen3RopeKvCandidateV1,
    positions: &[u32],
    query: &[f64],
    key: &[f64],
) -> Result<Qwen3RopeOutputV1, RopeReferenceErrorV1> {
    let (query_extent, key_extent) = validate_rope_inputs(candidate, positions, query, key)?;
    let mut output = Qwen3RopeOutputV1 {
        query: vec![0.0; query_extent],
        key: vec![0.0; key_extent],
    };
    rotate_tensor(
        query,
        &mut output.query,
        positions,
        candidate.geometry.query_heads,
        rotate_reference_dimensions,
    )?;
    rotate_tensor(
        key,
        &mut output.key,
        positions,
        candidate.geometry.kv_heads,
        rotate_reference_dimensions,
    )?;
    Ok(output)
}

/// Evaluates an independently indexed pair-oriented CPU `f64` candidate.
///
/// Differential equality with [`qwen3_rope_reference_v1`] validates the two
/// host algorithms only; it grants no executable GPU authority.
pub fn qwen3_rope_pair_candidate_v1(
    candidate: &Qwen3RopeKvCandidateV1,
    positions: &[u32],
    query: &[f64],
    key: &[f64],
) -> Result<Qwen3RopeOutputV1, RopeReferenceErrorV1> {
    let (query_extent, key_extent) = validate_rope_inputs(candidate, positions, query, key)?;
    let mut output = Qwen3RopeOutputV1 {
        query: vec![0.0; query_extent],
        key: vec![0.0; key_extent],
    };
    rotate_tensor(
        query,
        &mut output.query,
        positions,
        candidate.geometry.query_heads,
        rotate_candidate_pairs,
    )?;
    rotate_tensor(
        key,
        &mut output.key,
        positions,
        candidate.geometry.kv_heads,
        rotate_candidate_pairs,
    )?;
    Ok(output)
}
