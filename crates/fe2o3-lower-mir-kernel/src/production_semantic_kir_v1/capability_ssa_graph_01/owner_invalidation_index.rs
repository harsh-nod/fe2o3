// Compressed block rows, scoped to this graph's fixed body. An index is
// not a live-loan proof; owner SSA, path, cycles, windows and terminals stay checked.
struct CapabilityOwnerInvalidationsV1 {
    offsets: Vec<usize>,
    positions: Vec<usize>,
}

impl CapabilityOwnerInvalidationsV1 {
    fn row(&self, block: usize) -> Option<&[usize]> {
        let next = block.checked_add(1)?;
        let start = *self.offsets.get(block)?;
        let end = *self.offsets.get(next)?;
        self.positions.get(start..end)
    }
}

impl CapabilitySsaGraphV1<'_> {
    fn owner_statement_index(
        &mut self,
        owner: u32,
    ) -> Result<Arc<CapabilityOwnerInvalidationsV1>, ProductionSemanticKirErrorV1> {
        self.charge(lookup_work(self.reuse.owner_invalidations.len()).saturating_add(1))?;
        if let Some(index) = self.reuse.owner_invalidations.get(&owner) {
            return Ok(Arc::clone(index));
        }
        let blocks = self.body.blocks();
        let word = std::mem::size_of::<usize>();
        let index_words = std::mem::size_of::<CapabilityOwnerInvalidationsV1>().div_ceil(word);
        self.charge(index_words)?;
        let offset_count = blocks.len().saturating_add(1);
        self.charge(offset_count)?;
        let mut offsets = Vec::with_capacity(offset_count);
        self.charge(offsets.capacity() - offset_count)?;
        offsets.push(0);
        let mut positions = Vec::new();
        for block in blocks {
            self.charge(1)?;
            let statements = block.statements();
            self.charge(statements.len())?;
            for (position, statement) in statements.iter().enumerate() {
                if let SemanticStatementKindV1::Assign(assignment) = statement.kind()
                    && assignment.destination().local().index() != owner
                    && let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind()
                {
                    self.charge(aggregate.operands().len())?;
                }
                if invalidates(statement.kind(), owner) {
                    self.charge(2)?;
                    if positions.len() == positions.capacity() {
                        let capacity = positions.capacity().saturating_mul(2).max(4);
                        self.charge(capacity.saturating_add(positions.len()))?;
                        positions.reserve_exact(capacity - positions.len());
                        self.charge(positions.capacity() - capacity)?;
                    }
                    positions.push(position);
                }
            }
            offsets.push(positions.len());
        }
        let allocation_words = index_words.saturating_add(2);
        self.charge(
            insertion_work::<u32, Arc<CapabilityOwnerInvalidationsV1>>(
                self.reuse.owner_invalidations.len(),
            )
            .saturating_add(allocation_words),
        )?;
        let index = Arc::new(CapabilityOwnerInvalidationsV1 { offsets, positions });
        self.reuse
            .owner_invalidations
            .insert(owner, Arc::clone(&index));
        Ok(index)
    }

    fn indexed_statement_invalidated(
        &mut self,
        index: &CapabilityOwnerInvalidationsV1,
        block: u32,
        first: usize,
        end: usize,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        self.charge(2)?;
        let positions = index
            .row(block as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if first >= end || positions.is_empty() {
            return Ok(false);
        }
        // Include the final endpoint comparison before inspecting a position.
        self.charge(2 + (usize::BITS - positions.len().leading_zeros()) as usize)?;
        let next = positions.partition_point(|&position| position < first);
        Ok(positions.get(next).is_some_and(|&position| position < end))
    }
}
