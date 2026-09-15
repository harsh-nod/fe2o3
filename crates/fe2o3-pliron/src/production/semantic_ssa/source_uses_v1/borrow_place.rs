use super::*;

impl<'a> ProductionSemanticSsaSourceQueryV1<'a> {
    /// Selects an existing promoted root-local Use of the exact retained Borrow
    /// place. This is source correspondence, not a loan, storage or issuer proof.
    /// Address-observable storage without such an event remains unsupported.
    pub fn borrow_place_use(
        &self,
        site: ProductionSemanticSsaSourceSiteV1,
        place: &'a SemanticPlaceV1,
        charge: &mut impl FnMut() -> bool,
    ) -> Result<SsaValueV1, ProductionSemanticSsaSourceQueryErrorV1> {
        step(charge)?;
        let statement = site.statement.ok_or(QueryError::InvalidSite)?;
        let kind = self
            .function
            .blocks()
            .get(site.block.index() as usize)
            .and_then(|block| block.statements().get(statement as usize))
            .ok_or(QueryError::InvalidSite)?
            .kind();
        if !matches!(kind, SemanticStatementKindV1::Assign(assignment)
            if matches!(assignment.value().kind(), SemanticRvalueKindV1::Borrow { place: original, .. }
                if std::ptr::eq(original, place)))
        {
            return Err(QueryError::OperandOutsideSite);
        }
        if self
            .function
            .locals()
            .get(place.local().index() as usize)
            .is_none()
        {
            return Err(QueryError::UnsupportedOperand);
        }
        self.selected_site_use(site, SsaVariableIdV1::new(place.local().index()), charge)
            .map(|(value, _, _)| value)
    }

    // Shared with operand_use. No definition scan, fresh planner, or uncharged
    // source-wide search is introduced for a Borrow or an ordinary operand.
    pub(super) fn selected_site_use(
        &self,
        site: Site,
        variable: SsaVariableIdV1,
        charge: &mut impl FnMut() -> bool,
    ) -> Result<(SsaValueV1, Range<usize>, usize), QueryError> {
        let range = self.site_range(site, charge)?;
        let events = self
            .plan
            .plan
            .resolved_events(SsaBlockIdV1::new(site.block.index()))
            .ok_or(QueryError::InvalidSite)?;
        let start = partition(events, |(event, _)| (*event as usize) < range.start, charge)?;
        let mut value = None;
        let mut agreeing_uses = 0;
        for (event, resolved) in &events[start..] {
            step(charge)?;
            if *event as usize >= range.end {
                break;
            }
            let SsaResolvedEventV1::Use {
                variable: actual,
                value: next,
            } = resolved
            else {
                continue;
            };
            if *actual != variable {
                continue;
            }
            if value.is_some_and(|value| value != *next) {
                return Err(QueryError::DisagreeingUses);
            }
            value = Some(*next);
            agreeing_uses += 1;
        }
        Ok((
            value.ok_or(QueryError::NoPromotedUse)?,
            range,
            agreeing_uses,
        ))
    }
}
