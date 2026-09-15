// Definition IDs are dense within one immutable SSA plan. Only successful
// source checks populate this memo; a slot is not issuer or loan authority.
#[derive(Default)]
#[cfg_attr(test, derive(Clone, Debug, Eq, PartialEq))]
struct CapabilityDefinitionMemoV1<'a> {
    owner: Option<(&'a SemanticFunctionDeclV1, &'a SsaConstructionPlanV1)>,
    rows: Vec<Option<CapabilityDefinitionSiteV1>>,
    populated: usize,
}

impl CapabilityDefinitionMemoV1<'_> {
    fn len(&self) -> usize {
        self.populated
    }

    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.populated == 0
    }
}

impl CapabilitySsaGraphV1<'_> {
    pub(super) fn definition(
        &mut self,
        value: SsaValueV1,
    ) -> Result<CapabilityDefinitionSiteV1, ProductionSemanticKirErrorV1> {
        use std::mem::size_of;

        self.charge(1)?;
        let SsaValueV1::Definition(id) = value else {
            return self.definition_uncached(value);
        };
        self.charge(1)?;
        let count = self.ssa.definition_count();
        let index = id.get() as usize;
        if index >= count {
            return self.definition_uncached(value);
        }

        self.charge(3)?;
        if let Some((body, ssa)) = self.reuse.definitions.owner
            && (!std::ptr::eq(body, self.body) || !std::ptr::eq(ssa, self.ssa))
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let slot_words =
            size_of::<Option<CapabilityDefinitionSiteV1>>().div_ceil(size_of::<usize>());
        self.charge(slot_words)?;
        if let Some(Some(site)) = self.reuse.definitions.rows.get(index) {
            return Ok(*site);
        }

        let site = self.definition_uncached(value)?;
        if self.reuse.definitions.owner.is_none() {
            self.charge(size_of::<CapabilityDefinitionMemoV1<'_>>().div_ceil(size_of::<usize>()))?;
            let cells = count.saturating_mul(slot_words);
            self.charge(cells)?;
            let mut rows = Vec::with_capacity(count);
            self.charge((rows.capacity() - count).saturating_mul(slot_words))?;
            self.charge(cells)?;
            rows.resize(count, None);
            // Publish the complete table and its owner only after every debit.
            self.charge(slot_words.saturating_add(3))?;
            rows[index] = Some(site);
            self.reuse.definitions = CapabilityDefinitionMemoV1 {
                owner: Some((self.body, self.ssa)),
                rows,
                populated: 1,
            };
        } else {
            self.charge(slot_words.saturating_add(1))?;
            let slot = self
                .reuse
                .definitions
                .rows
                .get_mut(index)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            *slot = Some(site);
            self.reuse.definitions.populated += 1;
        }
        Ok(site)
    }
}
