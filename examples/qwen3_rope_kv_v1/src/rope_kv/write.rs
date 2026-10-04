use super::{
    CandidateErrorV1, KvOwnerIdentityV1, KvPhysicalLocationV1, PageTableErrorV1,
    PageTableGenerationV1, Qwen3PageTableV1, Qwen3RopeKvCandidateV1,
    validate_qwen3_rope_kv_candidate_v1,
};
use std::error::Error;
use std::fmt;

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
