//! Exact B3 shape, effect, schedule, and resource contracts.

/// Exact gfx942 processor name.
pub const GFX942_PROCESSOR_V1: &str = "gfx942";
/// Exact target-feature profile.
pub const GFX942_TARGET_FEATURES_V1: &str = "+wavefrontsize64,-xnack";
/// Fixed logical threads per workgroup.
pub const SWIGLU_THREADS_PER_WORKGROUP_V1: u16 = 256;
/// Fixed contiguous elements owned by each logical thread.
pub const SWIGLU_ELEMENTS_PER_THREAD_V1: u8 = 8;
/// Largest flattened row count in the exact B3 matrix.
pub const MAX_B3_SWIGLU_ROWS_V1: usize = 2_048;
/// Largest intermediate width in the exact B3 matrix.
pub const MAX_B3_SWIGLU_INTERMEDIATE_V1: usize = 12_288;
/// Largest element count in one exact B3 invocation.
pub const MAX_B3_SWIGLU_ELEMENTS_V1: usize = MAX_B3_SWIGLU_ROWS_V1 * MAX_B3_SWIGLU_INTERMEDIATE_V1;

/// Exact Qwen3 role and model geometry.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Qwen3ModelRoleV1 {
    /// Qwen3-8B target.
    Target8B = 1,
    /// Qwen3-0.6B draft.
    Draft06B = 2,
}

impl Qwen3ModelRoleV1 {
    /// Returns the exact hidden width for this role.
    pub const fn hidden_size(self) -> usize {
        match self {
            Self::Target8B => 4_096,
            Self::Draft06B => 1_024,
        }
    }

    /// Returns the exact SwiGLU intermediate width for this role.
    pub const fn intermediate_size(self) -> usize {
        match self {
            Self::Target8B => 12_288,
            Self::Draft06B => 3_072,
        }
    }

    /// Returns the stable identity tag.
    pub const fn identity_tag(self) -> u8 {
        self as u8
    }
}

/// Closed Ferric M1 B3 workload matrix.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum B3SwiGluBucketV1 {
    /// Prefill: one sequence with 128 active tokens.
    PrefillS1T128 = 1,
    /// Prefill: eight sequences with 128 active tokens each.
    PrefillS8T128 = 2,
    /// Prefill: one sequence with 512 active tokens.
    PrefillS1T512 = 3,
    /// Prefill: one sequence with 2048 active tokens.
    PrefillS1T2048 = 4,
    /// Decode: one sequence with one active token.
    DecodeS1 = 5,
    /// Decode: eight sequences with one active token each.
    DecodeS8 = 6,
    /// Decode: 32 sequences with one active token each.
    DecodeS32 = 7,
    /// Speculative decoding: one sequence and K=4.
    SpeculativeS1K4 = 8,
    /// Speculative decoding: eight sequences and K=4.
    SpeculativeS8K4 = 9,
    /// Speculative decoding: one sequence and K=8.
    SpeculativeS1K8 = 10,
    /// Speculative decoding: one sequence and K=16.
    SpeculativeS1K16 = 11,
}

/// All and only admitted B3 buckets, in stable identity order.
pub const B3_SWIGLU_BUCKETS_V1: [B3SwiGluBucketV1; 11] = [
    B3SwiGluBucketV1::PrefillS1T128,
    B3SwiGluBucketV1::PrefillS8T128,
    B3SwiGluBucketV1::PrefillS1T512,
    B3SwiGluBucketV1::PrefillS1T2048,
    B3SwiGluBucketV1::DecodeS1,
    B3SwiGluBucketV1::DecodeS8,
    B3SwiGluBucketV1::DecodeS32,
    B3SwiGluBucketV1::SpeculativeS1K4,
    B3SwiGluBucketV1::SpeculativeS8K4,
    B3SwiGluBucketV1::SpeculativeS1K8,
    B3SwiGluBucketV1::SpeculativeS1K16,
];

impl B3SwiGluBucketV1 {
    /// Returns the exact number of sequences in this bucket.
    pub const fn sequences(self) -> usize {
        match self {
            Self::PrefillS8T128 | Self::DecodeS8 | Self::SpeculativeS8K4 => 8,
            Self::DecodeS32 => 32,
            _ => 1,
        }
    }

    /// Returns the exact active-token count per sequence for the role.
    pub const fn active_tokens(self, role: Qwen3ModelRoleV1) -> usize {
        match self {
            Self::PrefillS1T128 | Self::PrefillS8T128 => 128,
            Self::PrefillS1T512 => 512,
            Self::PrefillS1T2048 => 2_048,
            Self::DecodeS1 | Self::DecodeS8 | Self::DecodeS32 => 1,
            Self::SpeculativeS1K4 | Self::SpeculativeS8K4 => match role {
                Qwen3ModelRoleV1::Target8B => 5,
                Qwen3ModelRoleV1::Draft06B => 4,
            },
            Self::SpeculativeS1K8 => match role {
                Qwen3ModelRoleV1::Target8B => 9,
                Qwen3ModelRoleV1::Draft06B => 8,
            },
            Self::SpeculativeS1K16 => match role {
                Qwen3ModelRoleV1::Target8B => 17,
                Qwen3ModelRoleV1::Draft06B => 16,
            },
        }
    }

    /// Returns the exact flattened row count.
    pub const fn rows(self, role: Qwen3ModelRoleV1) -> usize {
        self.sequences() * self.active_tokens(role)
    }

    /// Returns the stable identity tag.
    pub const fn identity_tag(self) -> u8 {
        self as u8
    }
}

/// Public inert profile record accepted only after exact validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SwiGluProfileDescriptorV1 {
    /// Exact Qwen3 role.
    pub role: Qwen3ModelRoleV1,
    /// Exact B3 workload bucket.
    pub bucket: B3SwiGluBucketV1,
    /// Independent sequences.
    pub sequences: usize,
    /// Active tokens per sequence.
    pub active_tokens: usize,
    /// Flattened rows.
    pub rows: usize,
    /// Model hidden width feeding the gate and up projections.
    pub hidden_size: usize,
    /// Gate/up/down intermediate width.
    pub intermediate_size: usize,
}

impl SwiGluProfileDescriptorV1 {
    /// Constructs the sole canonical descriptor for a role and bucket.
    pub const fn canonical(role: Qwen3ModelRoleV1, bucket: B3SwiGluBucketV1) -> Self {
        Self {
            role,
            bucket,
            sequences: bucket.sequences(),
            active_tokens: bucket.active_tokens(role),
            rows: bucket.rows(role),
            hidden_size: role.hidden_size(),
            intermediate_size: role.intermediate_size(),
        }
    }
}

/// Exact profile validation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SwiGluProfileErrorV1 {
    /// Sequence count differs from the named bucket.
    SequenceCount,
    /// Active-token count differs from the bucket and role.
    ActiveTokenCount,
    /// Flattened rows differ from the checked product.
    FlattenedRows,
    /// Hidden width differs from the role.
    HiddenSize,
    /// Intermediate width differs from the role.
    IntermediateSize,
    /// Checked resource arithmetic overflowed.
    ResourceArithmeticOverflow,
    /// A derived resource exceeded the reviewed B3 ceiling.
    ResourceLimit,
}

/// Checked resource envelope for one exact profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SwiGluResourceContractV1 {
    /// Total gate/up/output elements.
    pub elements: usize,
    /// Exact BF16 bytes per buffer.
    pub bytes_per_buffer: usize,
    /// Exact global bytes read from gate and up.
    pub global_read_bytes: usize,
    /// Exact global bytes written to activated output.
    pub global_write_bytes: usize,
    /// Number of 256-thread logical workgroups.
    pub workgroups: usize,
    /// Logical launched threads, including masked tail owners.
    pub logical_threads: usize,
    /// Transactional host scratch bytes.
    pub host_scratch_bytes: usize,
    /// No LDS is used by the exact elementwise schedule.
    pub lds_bytes_per_workgroup: usize,
    /// No barrier is used by the exact elementwise schedule.
    pub barriers_per_workgroup: u8,
}

/// Validated exact profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedSwiGluProfileV1 {
    descriptor: SwiGluProfileDescriptorV1,
    resources: SwiGluResourceContractV1,
}

impl ValidatedSwiGluProfileV1 {
    /// Returns the exact validated descriptor.
    pub const fn descriptor(self) -> SwiGluProfileDescriptorV1 {
        self.descriptor
    }

    /// Returns the checked resource envelope.
    pub const fn resources(self) -> SwiGluResourceContractV1 {
        self.resources
    }
}

/// Unique logical schedule owner for one admitted output element.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SwiGluElementOwnerV1 {
    /// Zero-based logical workgroup.
    pub workgroup: usize,
    /// Zero-based thread within the 256-thread workgroup.
    pub thread: u16,
    /// Contiguous element position within the thread's eight-element tile.
    pub element_in_thread: u8,
}

/// Returns the unique logical owner of one in-bounds output element.
pub fn swiglu_element_owner_v1(
    profile: ValidatedSwiGluProfileV1,
    element: usize,
) -> Option<SwiGluElementOwnerV1> {
    if element >= profile.resources().elements {
        return None;
    }
    let elements_per_thread = usize::from(SWIGLU_ELEMENTS_PER_THREAD_V1);
    let elements_per_workgroup = usize::from(SWIGLU_THREADS_PER_WORKGROUP_V1) * elements_per_thread;
    let within_workgroup = element % elements_per_workgroup;
    Some(SwiGluElementOwnerV1 {
        workgroup: element / elements_per_workgroup,
        thread: u16::try_from(within_workgroup / elements_per_thread)
            .expect("owner thread is below the exact 256-thread bound"),
        element_in_thread: u8::try_from(within_workgroup % elements_per_thread)
            .expect("owner element is below the exact eight-element bound"),
    })
}

/// Resolves a logical owner to its element, rejecting masked tail owners.
pub fn swiglu_owned_element_v1(
    profile: ValidatedSwiGluProfileV1,
    owner: SwiGluElementOwnerV1,
) -> Option<usize> {
    if owner.workgroup >= profile.resources().workgroups
        || owner.thread >= SWIGLU_THREADS_PER_WORKGROUP_V1
        || owner.element_in_thread >= SWIGLU_ELEMENTS_PER_THREAD_V1
    {
        return None;
    }
    let elements_per_thread = usize::from(SWIGLU_ELEMENTS_PER_THREAD_V1);
    let elements_per_workgroup = usize::from(SWIGLU_THREADS_PER_WORKGROUP_V1) * elements_per_thread;
    let element = owner
        .workgroup
        .checked_mul(elements_per_workgroup)?
        .checked_add(usize::from(owner.thread).checked_mul(elements_per_thread)?)?
        .checked_add(usize::from(owner.element_in_thread))?;
    (element < profile.resources().elements).then_some(element)
}

/// Validates every role, bucket, shape, and resource field.
pub fn validate_swiglu_profile_v1(
    descriptor: SwiGluProfileDescriptorV1,
) -> Result<ValidatedSwiGluProfileV1, SwiGluProfileErrorV1> {
    let canonical = SwiGluProfileDescriptorV1::canonical(descriptor.role, descriptor.bucket);
    if descriptor.sequences != canonical.sequences {
        return Err(SwiGluProfileErrorV1::SequenceCount);
    }
    if descriptor.active_tokens != canonical.active_tokens {
        return Err(SwiGluProfileErrorV1::ActiveTokenCount);
    }
    let rows = descriptor
        .sequences
        .checked_mul(descriptor.active_tokens)
        .ok_or(SwiGluProfileErrorV1::ResourceArithmeticOverflow)?;
    if descriptor.rows != rows || descriptor.rows != canonical.rows {
        return Err(SwiGluProfileErrorV1::FlattenedRows);
    }
    if descriptor.hidden_size != canonical.hidden_size {
        return Err(SwiGluProfileErrorV1::HiddenSize);
    }
    if descriptor.intermediate_size != canonical.intermediate_size {
        return Err(SwiGluProfileErrorV1::IntermediateSize);
    }
    let elements = descriptor
        .rows
        .checked_mul(descriptor.intermediate_size)
        .ok_or(SwiGluProfileErrorV1::ResourceArithmeticOverflow)?;
    if descriptor.rows > MAX_B3_SWIGLU_ROWS_V1
        || descriptor.intermediate_size > MAX_B3_SWIGLU_INTERMEDIATE_V1
        || elements > MAX_B3_SWIGLU_ELEMENTS_V1
    {
        return Err(SwiGluProfileErrorV1::ResourceLimit);
    }
    let bytes_per_buffer = elements
        .checked_mul(2)
        .ok_or(SwiGluProfileErrorV1::ResourceArithmeticOverflow)?;
    let global_read_bytes = bytes_per_buffer
        .checked_mul(2)
        .ok_or(SwiGluProfileErrorV1::ResourceArithmeticOverflow)?;
    let covered_per_workgroup = usize::from(SWIGLU_THREADS_PER_WORKGROUP_V1)
        .checked_mul(usize::from(SWIGLU_ELEMENTS_PER_THREAD_V1))
        .ok_or(SwiGluProfileErrorV1::ResourceArithmeticOverflow)?;
    let workgroups = elements
        .checked_add(covered_per_workgroup - 1)
        .ok_or(SwiGluProfileErrorV1::ResourceArithmeticOverflow)?
        / covered_per_workgroup;
    let logical_threads = workgroups
        .checked_mul(usize::from(SWIGLU_THREADS_PER_WORKGROUP_V1))
        .ok_or(SwiGluProfileErrorV1::ResourceArithmeticOverflow)?;
    Ok(ValidatedSwiGluProfileV1 {
        descriptor,
        resources: SwiGluResourceContractV1 {
            elements,
            bytes_per_buffer,
            global_read_bytes,
            global_write_bytes: bytes_per_buffer,
            workgroups,
            logical_threads,
            host_scratch_bytes: bytes_per_buffer,
            lds_bytes_per_workgroup: 0,
            barriers_per_workgroup: 0,
        },
    })
}

/// Inert generation-bound logical buffer region.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SwiGluBufferBindingV1 {
    /// Nonzero allocation identity.
    pub allocation_id: u64,
    /// Nonzero allocation generation.
    pub generation: u64,
    /// Byte offset from the allocation base.
    pub byte_offset: u64,
    /// Exact accessible byte length.
    pub byte_len: u64,
}

impl SwiGluBufferBindingV1 {
    fn end(self) -> Option<u64> {
        self.byte_offset.checked_add(self.byte_len)
    }
}

/// Fixed inert schedule record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SwiGluScheduleDescriptorV1 {
    /// Logical threads per workgroup.
    pub threads_per_workgroup: u16,
    /// Contiguous elements per logical thread.
    pub elements_per_thread: u8,
    /// LDS bytes per workgroup.
    pub lds_bytes_per_workgroup: u32,
    /// Uniform barriers per workgroup.
    pub barriers_per_workgroup: u8,
}

impl SwiGluScheduleDescriptorV1 {
    /// Returns the sole exact structural schedule.
    pub const fn canonical() -> Self {
        Self {
            threads_per_workgroup: SWIGLU_THREADS_PER_WORKGROUP_V1,
            elements_per_thread: SWIGLU_ELEMENTS_PER_THREAD_V1,
            lds_bytes_per_workgroup: 0,
            barriers_per_workgroup: 0,
        }
    }
}

/// Untrusted inert candidate record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SwiGluCandidateDescriptorV1 {
    /// Exact profile descriptor.
    pub profile: SwiGluProfileDescriptorV1,
    /// Read-only BF16 gate-projection output.
    pub gate: SwiGluBufferBindingV1,
    /// Read-only BF16 up-projection output.
    pub up: SwiGluBufferBindingV1,
    /// Write-only BF16 activated output consumed by down projection.
    pub activated: SwiGluBufferBindingV1,
    /// Exact inert schedule.
    pub schedule: SwiGluScheduleDescriptorV1,
}

/// Candidate validation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SwiGluCandidateErrorV1 {
    /// Exact profile validation failed.
    Profile(SwiGluProfileErrorV1),
    /// One buffer used absent allocation or generation authority.
    AbsentBufferAuthority,
    /// One BF16 byte offset was not two-byte aligned.
    MisalignedBuffer,
    /// One buffer length differed from the exact profile extent.
    BufferLength,
    /// One buffer range overflowed `u64`.
    BufferRangeOverflow,
    /// Two logical buffers overlap in the same allocation generation.
    BufferOverlap,
    /// The schedule differs from the sole reviewed structure.
    Schedule,
}

/// Validated inert candidate. It owns no allocation or launch authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedSwiGluCandidateV1 {
    descriptor: SwiGluCandidateDescriptorV1,
    profile: ValidatedSwiGluProfileV1,
}

impl ValidatedSwiGluCandidateV1 {
    /// Returns the complete inert candidate descriptor.
    pub const fn descriptor(self) -> SwiGluCandidateDescriptorV1 {
        self.descriptor
    }

    /// Returns the exact validated profile.
    pub const fn profile(self) -> ValidatedSwiGluProfileV1 {
        self.profile
    }
}

fn ranges_overlap(left: SwiGluBufferBindingV1, right: SwiGluBufferBindingV1) -> Option<bool> {
    let left_end = left.end()?;
    let right_end = right.end()?;
    Some(
        left.allocation_id == right.allocation_id
            && left.generation == right.generation
            && left.byte_offset < right_end
            && right.byte_offset < left_end,
    )
}

/// Validates exact shape, buffer, effect, and schedule requirements.
pub fn validate_swiglu_candidate_v1(
    descriptor: SwiGluCandidateDescriptorV1,
) -> Result<ValidatedSwiGluCandidateV1, SwiGluCandidateErrorV1> {
    let profile =
        validate_swiglu_profile_v1(descriptor.profile).map_err(SwiGluCandidateErrorV1::Profile)?;
    let exact_bytes = u64::try_from(profile.resources().bytes_per_buffer)
        .map_err(|_| SwiGluCandidateErrorV1::BufferLength)?;
    for binding in [descriptor.gate, descriptor.up, descriptor.activated] {
        if binding.allocation_id == 0 || binding.generation == 0 {
            return Err(SwiGluCandidateErrorV1::AbsentBufferAuthority);
        }
        if binding.byte_offset % 2 != 0 {
            return Err(SwiGluCandidateErrorV1::MisalignedBuffer);
        }
        if binding.byte_len != exact_bytes {
            return Err(SwiGluCandidateErrorV1::BufferLength);
        }
        if binding.end().is_none() {
            return Err(SwiGluCandidateErrorV1::BufferRangeOverflow);
        }
    }
    for (left, right) in [
        (descriptor.gate, descriptor.up),
        (descriptor.gate, descriptor.activated),
        (descriptor.up, descriptor.activated),
    ] {
        if ranges_overlap(left, right).ok_or(SwiGluCandidateErrorV1::BufferRangeOverflow)? {
            return Err(SwiGluCandidateErrorV1::BufferOverlap);
        }
    }
    if descriptor.schedule != SwiGluScheduleDescriptorV1::canonical() {
        return Err(SwiGluCandidateErrorV1::Schedule);
    }
    Ok(ValidatedSwiGluCandidateV1 {
        descriptor,
        profile,
    })
}
