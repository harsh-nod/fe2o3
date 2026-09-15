use super::{
    CapabilityDefinitionMemoV1, CapabilityDefinitionSiteV1, CapabilityLoanRegionV1,
    CapabilityLoanV1, CapabilityOwnerInvalidationsV1, CapabilityPathRegionScratchV1,
    CapabilityRegionAcyclicScratchV1, SsaValueV1,
};
use std::collections::BTreeMap;
use std::mem::size_of;
use std::sync::Arc;

#[derive(Default)]
pub(super) struct CapabilityQueryReuseV1<'a> {
    pub(super) uses: BTreeMap<(u32, u32), SsaValueV1>,
    pub(super) definitions: CapabilityDefinitionMemoV1<'a>,
    pub(super) enum_guards: super::enum_guard::Memo<'a>,
    pub(super) reachability: BTreeMap<(u32, u32), bool>,
    pub(super) loans: BTreeMap<(CapabilityLoanV1, CapabilityDefinitionSiteV1), ()>,
    pub(super) loan_regions: BTreeMap<(u32, u32), Arc<CapabilityLoanRegionV1>>,
    pub(super) owner_invalidations: BTreeMap<u32, Arc<CapabilityOwnerInvalidationsV1>>,
    pub(super) path_region_scratch: Option<Box<CapabilityPathRegionScratchV1<'a>>>,
    pub(super) region_acyclic_scratch: Option<Box<CapabilityRegionAcyclicScratchV1>>,
    pub(super) endpoint_scc: Option<Box<super::endpoint_scc::Index<'a>>>,
    pub(super) definition_source: Option<fe2o3_pliron::ProductionSemanticSsaSourceQueryV1<'a>>,
}

pub(super) const fn header_words() -> usize {
    size_of::<CapabilityQueryReuseV1<'_>>().div_ceil(size_of::<usize>())
}

// Logical key/value words, not a claim about BTreeMap allocator-byte capacity.
pub(super) const fn entry_words<K, V>() -> usize {
    size_of::<(K, V)>()
        .div_ceil(size_of::<usize>())
        .saturating_add(1)
}

pub(super) fn lookup_work(len: usize) -> usize {
    // The pinned standard library uses at most 11 keys per B-tree node.
    // Binary depth overestimates its height; include the empty-map lookup.
    1usize.saturating_add(12usize.saturating_mul((usize::BITS - len.leading_zeros()) as usize))
}

pub(super) fn insertion_work<K, V>(len: usize) -> usize {
    let words = entry_words::<K, V>();
    // Precharge the index walk, possible row movement and retained key/value.
    lookup_work(len)
        .saturating_mul(words.saturating_add(1))
        .saturating_add(words)
}
