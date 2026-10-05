// Shared pre-lifecycle coordinates on the one expanded and relocated graph.
// This maps locations only; it proves neither source values nor pointer origins.
struct ScopedEmittedPointsV29<'a, 'budget, 'work> {
    coordinates: &'a OwnedInstanceCoordinatesV1,
    relocation: &'a Option<scoped_slot_relocation_v29::RelocationV29>,
    budget: &'budget mut ArgumentBudgetV1<'work>,
}

impl ScopedEmittedPointsV29<'_, '_, '_> {
    fn removed_anchor(
        &mut self,
        span: InstanceMappedSpanV1,
    ) -> Result<Option<&InstanceCallAnchorV1>, ScopedTileFailureKindV29> {
        let Some(call) = span.removed_call else {
            return Ok(None);
        };
        self.budget
            .charge_work(self.coordinates.anchors.rows.len())?;
        let mut anchors = self.coordinates.anchors.rows.iter().filter(|row| {
            row.instance == call.caller && row.source.semantic_block == call.block && row.removed
        });
        let anchor = anchors
            .next()
            .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
        if anchors.next().is_some() {
            return Err(ScopedTileFailureKindV29::ReplayMismatch);
        }
        Ok(Some(anchor))
    }

    // An explicit emitted gap has source authority of its own. A removed-call
    // span still has no gap, even when an independently captured gap coincides.
    fn emitted_point(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        block: BlockId,
        p: u32,
        gap: bool,
    ) -> Result<Option<(BlockId, u32)>, ScopedTileFailureKindV29> {
        let mut found = None;
        let spans = &self.coordinates.spans.rows;
        for span in spans {
            self.budget.charge_work(1)?;
            let original = span.source.coordinates().2;
            let end = original
                .first
                .checked_add(original.count)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            if span.instance != instance
                || original.block != block
                || p < original.first
                || if gap { p > end } else { p >= end }
            {
                continue;
            }
            if gap
                && matches!(span.source, InstanceSpanSourceV1::Synthetic(source)
                if source.rule == SemanticKirSyntheticOperationRuleV1::RetainedLocalStorage)
            {
                continue;
            }
            let offset = p - original.first;
            let cut = if let Some(anchor) = self.removed_anchor(*span)? {
                let (SemanticKirCallReturnKindV1::Call { call_operation, .. }
                | SemanticKirCallReturnKindV1::NoNormalReturnCall { call_operation, .. }) =
                    anchor.source.kind
                else {
                    return Err(ScopedTileFailureKindV29::ReplayMismatch);
                };
                Some((call_operation, anchor.physical))
            } else {
                None
            };
            let mapped = if let Some((call, physical)) = cut {
                if gap {
                    let location = if p <= call {
                        let first = u32::try_from(physical.operation_index)
                            .map_err(|_| ArgumentResourceV1::Arithmetic)?
                            .checked_sub(call - p)
                            .ok_or(ArgumentResourceV1::Arithmetic)?;
                        InstancePhysicalSpanV1 {
                            block: physical.block,
                            first,
                            count: 0,
                        }
                    } else {
                        self.budget
                            .charge_work(self.coordinates.controls.rows.len())?;
                        let mut controls = self.coordinates.controls.rows.iter().filter(|row| {
                            row.instance == instance
                                && row.original_block == block
                                && row.origin == InstanceControlOriginV1::Retained
                        });
                        let control = controls
                            .next()
                            .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
                        if controls.next().is_some() {
                            return Err(ScopedTileFailureKindV29::ReplayMismatch);
                        }
                        InstancePhysicalSpanV1 {
                            block: control.physical_block,
                            first: p
                                .checked_sub(
                                    call.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?,
                                )
                                .ok_or(ArgumentResourceV1::Arithmetic)?,
                            count: 0,
                        }
                    };
                    let location = match &self.relocation {
                        Some(relocation) => relocation.assertion_span(location, self.budget)?,
                        None => location,
                    };
                    Some((location.block, location.first))
                } else {
                    Self::operation_point(*span, p, Some(call))?
                }
            } else if gap {
                Some(Self::segment_point(*span, offset, true)?)
            } else {
                Self::operation_point(*span, p, None)?
            };
            if found.is_some_and(|previous| previous != mapped) {
                return Err(ScopedTileFailureKindV29::ReplayMismatch);
            }
            found = Some(mapped);
        }
        found.ok_or(ScopedTileFailureKindV29::ReplayMismatch)
    }

    // Both the scan and the prepaid source index use this operation-only map.
    // A removed call has no operation even when another span covers its key.
    fn operation_point(
        span: InstanceMappedSpanV1,
        p: u32,
        removed_call: Option<u32>,
    ) -> Result<Option<(BlockId, u32)>, ScopedTileFailureKindV29> {
        let original = span.source.coordinates().2;
        let end = original
            .first
            .checked_add(original.count)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        if p < original.first || p >= end {
            return Err(ScopedTileFailureKindV29::ReplayMismatch);
        }
        if removed_call == Some(p) {
            return Ok(None);
        }
        let mut offset = p - original.first;
        if removed_call.is_some_and(|call| p > call) {
            offset = offset
                .checked_sub(1)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
        }
        Self::segment_point(span, offset, false).map(Some)
    }
    fn segment_point(
        span: InstanceMappedSpanV1,
        mut offset: u32,
        gap: bool,
    ) -> Result<(BlockId, u32), ScopedTileFailureKindV29> {
        for segment in span.segments.into_iter().flatten() {
            if offset < segment.count || (gap && offset == segment.count) {
                return Ok((
                    segment.block,
                    segment
                        .first
                        .checked_add(offset)
                        .ok_or(ArgumentResourceV1::Arithmetic)?,
                ));
            }
            offset = offset
                .checked_sub(segment.count)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
        }
        Err(ScopedTileFailureKindV29::ReplayMismatch)
    }
}
