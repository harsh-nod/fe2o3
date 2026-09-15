impl<'a> ProductionSemanticSsaSourceQueryV1<'a> {
    /// Borrows the existing adapter window for one exact source site. This is
    /// correspondence only; it neither inserts events nor grants source custody.
    /// Empty or unrecorded windows retain the existing NoPromotedUse rejection.
    pub fn resolved_events_at(
        &self,
        site: ProductionSemanticSsaSourceSiteV1,
        charge: &mut impl FnMut() -> bool,
    ) -> Result<&'a [(u32, SsaResolvedEventV1)], QueryError> {
        let range = self.site_range(site, charge)?;
        let events = self
            .plan
            .plan
            .resolved_events(SsaBlockIdV1::new(site.block.index()))
            .ok_or(QueryError::InvalidSite)?;
        let start = partition(events, |(event, _)| (*event as usize) < range.start, charge)?;
        let end = partition(events, |(event, _)| (*event as usize) < range.end, charge)?;
        events.get(start..end).ok_or(QueryError::MissingEventOrigin)
    }
}
