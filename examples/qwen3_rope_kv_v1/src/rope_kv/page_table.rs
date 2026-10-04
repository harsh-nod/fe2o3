use super::{
    ContextBucketV1, M1_MAX_PAGE_TABLE_ENTRIES_V1, M1_MAX_PHYSICAL_PAGES_V1, PageBucketV1,
    Qwen3ModelRoleV1,
};
use std::error::Error;
use std::fmt;

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
    /// Context capacity is not divisible by the selected page size.
    PageDoesNotDivideContext,
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
        if !self
            .context
            .tokens()
            .is_multiple_of(u32::from(self.page.tokens()))
        {
            return Err(PageTableErrorV1::PageDoesNotDivideContext);
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
