type SourceAddressPointKeyV29 = (usize, u32, u32);

struct SourceAddressEmittedSpanV29<'source> {
    span: &'source InstanceMappedSpanV1,
    first: SourceAddressPointKeyV29,
    end: SourceAddressPointKeyV29,
    subtree_end: SourceAddressPointKeyV29,
}

// Borrowed locators only. Every overlapping original span remains a candidate
// and must agree under the same operation mapper used by unindexed replay.
struct SourceAddressEmittedIndexV29<'source> {
    spans: Vec<SourceAddressEmittedSpanV29<'source>>,
    anchors: Vec<&'source InstanceCallAnchorV1>,
    blocks: Vec<(u32, &'source BasicBlock)>,
    storage: usize,
}

impl<'source> SourceAddressEmittedIndexV29<'source> {
    fn new(
        pending: &'source PendingScopedRootEmissionV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let body = pending.function.body.as_ref().ok_or_else(source_raw_physical_error_v29)?;
        Self::from_rows(&pending.coordinates.spans.rows, &pending.coordinates.anchors.rows,
            &body.blocks, budget)
    }

    fn from_rows(
        source_spans: &'source [InstanceMappedSpanV1],
        source_anchors: &'source [InstanceCallAnchorV1],
        source_blocks: &'source [BasicBlock],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let mut spans = emission_vec_v1(source_spans.len(), budget)?;
        for span in source_spans {
            budget.charge_work(4)?;
            let original = span.source.coordinates().2;
            let first = (span.instance.index(), original.block.0, original.first);
            let end = (span.instance.index(), original.block.0,
                original.first.checked_add(original.count).ok_or(ArgumentResourceV1::Arithmetic)?);
            spans.push(SourceAddressEmittedSpanV29 { span, first, end, subtree_end: end });
        }
        call_splice_sort_work_v1(argument_product_v1(spans.len(), 3)?, budget)
            .map_err(source_address_call_error_v29)?;
        spans.sort_unstable_by_key(|row| row.first);
        Self::index_ends(&mut spans, budget)?;
        let mut anchors = emission_vec_v1(source_anchors.len(), budget)?;
        for anchor in source_anchors {
            budget.charge_work(1)?;
            if anchor.removed { anchors.push(anchor); }
        }
        call_splice_sort_work_v1(argument_product_v1(anchors.len(), 2)?, budget)
            .map_err(source_address_call_error_v29)?;
        anchors.sort_unstable_by_key(|row| (row.instance.index(), row.source.semantic_block.index()));
        let mut blocks = emission_vec_v1(source_blocks.len(), budget)?;
        for block in source_blocks {
            budget.charge_work(1)?;
            blocks.push((block.id.0, block));
        }
        call_splice_sort_work_v1(blocks.len(), budget).map_err(source_address_call_error_v29)?;
        blocks.sort_unstable_by_key(|row| row.0);
        let storage = argument_sum_v1(&[
            argument_product_v1(spans.capacity(), std::mem::size_of::<SourceAddressEmittedSpanV29<'_>>())?,
            argument_product_v1(anchors.capacity(), std::mem::size_of::<&InstanceCallAnchorV1>())?,
            argument_product_v1(blocks.capacity(), std::mem::size_of::<(u32, &BasicBlock)>())?,
        ])?;
        Ok(Self { spans, anchors, blocks, storage })
    }

    fn index_ends(
        spans: &mut [SourceAddressEmittedSpanV29<'_>],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<SourceAddressPointKeyV29>, ProductionSemanticKirErrorV1> {
        if spans.is_empty() { return Ok(None); }
        budget.charge_work(3)?;
        let middle = spans.len() / 2;
        let (left, tail) = spans.split_at_mut(middle);
        let (node, right) = tail.split_first_mut().ok_or_else(source_raw_physical_error_v29)?;
        for end in [Self::index_ends(left, budget)?, Self::index_ends(right, budget)?].into_iter().flatten() {
            node.subtree_end = node.subtree_end.max(end);
        }
        Ok(Some(node.subtree_end))
    }

    fn removed_call(
        &self,
        span: &InstanceMappedSpanV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<u32>, ProductionSemanticKirErrorV1> {
        let Some(call) = span.removed_call else { return Ok(None); };
        let key = (call.caller.index(), call.block.index());
        budget.charge_work(argument_sum_v1(&[
            argument_product_v1(call_splice_search_work_v1(self.anchors.len()), 2)?, 5,
        ])?)?;
        let anchor_key = |row: &&InstanceCallAnchorV1| (row.instance.index(), row.source.semantic_block.index());
        let index = self.anchors.binary_search_by_key(&key, anchor_key)
            .map_err(|_| source_raw_physical_error_v29())?;
        if index.checked_sub(1).and_then(|i| self.anchors.get(i)).is_some_and(|row| anchor_key(row) == key)
            || self.anchors.get(index + 1).is_some_and(|row| anchor_key(row) == key)
        {
            return Err(source_raw_physical_error_v29());
        }
        match self.anchors[index].source.kind {
            SemanticKirCallReturnKindV1::Call { call_operation, .. }
            | SemanticKirCallReturnKindV1::NoNormalReturnCall { call_operation, .. } => Ok(Some(call_operation)),
            _ => Err(source_raw_physical_error_v29()),
        }
    }

    fn point_in(
        &self,
        spans: &[SourceAddressEmittedSpanV29<'_>],
        key: SourceAddressPointKeyV29,
        found: &mut Option<Option<(BlockId, u32)>>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if spans.is_empty() { return Ok(()); }
        budget.charge_work(9)?;
        let middle = spans.len() / 2;
        let node = &spans[middle];
        if node.subtree_end <= key { return Ok(()); }
        self.point_in(&spans[..middle], key, found, budget)?;
        if node.first <= key {
            if key < node.end {
                let call = self.removed_call(node.span, budget)?;
                let mapped = ScopedEmittedPointsV29::operation_point(*node.span, key.2, call)
                    .map_err(source_address_point_error_v29)?;
                if found.is_some_and(|previous| previous != mapped) {
                    return Err(source_raw_physical_error_v29());
                }
                *found = Some(mapped);
            }
            self.point_in(&spans[middle + 1..], key, found, budget)?;
        }
        Ok(())
    }

    fn point(
        &self,
        instance: ProductionCallInstanceIdV1,
        block: BlockId,
        operation: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<(BlockId, u32)>, ProductionSemanticKirErrorV1> {
        let mut found = None;
        self.point_in(&self.spans, (instance.index(), block.0, operation), &mut found, budget)?;
        found.ok_or_else(source_raw_physical_error_v29)
    }

    fn operation(
        &self,
        instance: ProductionCallInstanceIdV1,
        block: BlockId,
        operation: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<&'source Operation, ProductionSemanticKirErrorV1> {
        let (block, operation) = self.point(instance, block,
            u32::try_from(operation).map_err(|_| ArgumentResourceV1::Arithmetic)?, budget)?
            .ok_or_else(source_descriptor_error_v29)?;
        budget.charge_work(argument_sum_v1(&[call_splice_search_work_v1(self.blocks.len()), 3])?)?;
        let index = self.blocks.binary_search_by_key(&block.0, |row| row.0)
            .map_err(|_| source_descriptor_error_v29())?;
        if index.checked_sub(1).and_then(|i| self.blocks.get(i)).is_some_and(|row| row.0 == block.0)
            || self.blocks.get(index + 1).is_some_and(|row| row.0 == block.0)
        {
            return Err(source_descriptor_error_v29());
        }
        self.blocks[index].1.operations.get(operation as usize).ok_or_else(source_descriptor_error_v29)
    }
}
