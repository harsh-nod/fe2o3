//! Closed length namespace for exact same-owner checked-view switches.
//! No ordinary predicate table is admitted and no condition becomes a sentinel.
use super::*;
use crate::production_ranked_projection_v1::canonical_assertion_facts_v1::NominalCheckedViewV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Length {
    parameter: u32,
    source_local: u32,
    source_argument: u32,
    slot: u32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Site {
    pub(super) fact: NominalCheckedViewV1,
    pub(super) slot: u32,
    pub(super) constant: ProductionRankedValueIdV1,
    pub(super) ranked_block: Option<u32>,
}
pub(super) struct Controls {
    lengths: [Option<Length>; MAX_BLOCKS],
    sites: [Option<Site>; MAX_BLOCKS],
    length_count: usize,
    prepared: bool,
}
impl Controls {
    pub(super) const fn empty() -> Self {
        Self {
            lengths: [None; MAX_BLOCKS],
            sites: [None; MAX_BLOCKS],
            length_count: 0,
            prepared: false,
        }
    }
    pub(super) fn argument_count(&self) -> usize {
        1 + self.length_count
    }
    pub(super) fn site(&self, source: usize) -> Option<Site> {
        self.sites.get(source).copied().flatten()
    }
    pub(super) fn site_at_ranked(&self, block: usize) -> Option<Site> {
        self.sites
            .iter()
            .flatten()
            .find(|s| s.ranked_block == Some(block as u32))
            .copied()
    }
    fn length_slot(
        &mut self,
        parameter: u32,
        source_local: u32,
        source_argument: u32,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<u32> {
        if !resources.is_metered() || resources.has_denial() {
            return Err(resource(Resource::Accounting));
        }
        resources.work(32 + MAX_BLOCKS * 8)?;
        for row in self.lengths.iter().flatten() {
            if row.parameter == parameter {
                if (row.source_local, row.source_argument) != (source_local, source_argument) {
                    return Err(Error::Incomplete(
                        "checked-view canonical parameter has conflicting source identity",
                    ));
                }
                return Ok(row.slot);
            }
            if row.source_local == source_local || row.source_argument == source_argument {
                return Err(Error::Incomplete(
                    "checked-view distinct parameters alias source identity",
                ));
            }
        }
        if self.length_count >= MAX_BLOCKS {
            return Err(Error::Incomplete(
                "checked-view input-length namespace is full",
            ));
        }
        // Slot zero belongs to the existing extent input. Never alias it.
        let slot =
            u32::try_from(self.length_count + 1).map_err(|_| resource(Resource::Arithmetic))?;
        self.lengths[self.length_count] = Some(Length {
            parameter,
            source_local,
            source_argument,
            slot,
        });
        self.length_count += 1;
        Ok(slot)
    }
    pub(super) fn prepare(
        &mut self,
        flow: &NominalPreparedControlFlowV1<'_, '_>,
        prefix: &mut RootEntryPrefixV1,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<()> {
        if !resources.is_metered() || resources.has_denial() {
            return Err(resource(Resource::Accounting));
        }
        resources.work(64 + MAX_BLOCKS * 4)?;
        if self.prepared
            || self.length_count != 0
            || self.sites.iter().any(Option::is_some)
            || self.lengths.iter().any(Option::is_some)
        {
            return Err(resource(Resource::Accounting));
        }
        self.prepared = true; // One-shot, including partial refusal.
        for (source, fact) in flow.checked_views().iter().copied().enumerate() {
            resources.work(32)?;
            let Some(fact) = fact else { continue };
            if source != fact.source_block() || fact.required() != 256 {
                return Err(Error::Incomplete(
                    "checked-view retained source coordinate or bound differs",
                ));
            }
            let Some(ProjectedCfgTerminatorV1::AnalysisSplit {
                first_block,
                second_block,
            }) = flow.terminators().get(source)
            else {
                return Err(Error::Incomplete(
                    "checked-view requires the original two-way split",
                ));
            };
            if !same_edges(*first_block, *second_block, fact.success(), fact.failure()) {
                return Err(Error::Incomplete(
                    "checked-view loses an original success or failure edge",
                ));
            }
            let slot = self.length_slot(
                fact.parameter(),
                fact.source_local(),
                fact.source_argument(),
                resources,
            )?;
            // Prefix owns the new exact constant before any enclosing callback.
            // ID comes from its original namespace; no restart or fabricated ID.
            resources.reserve(&mut prefix.entry_operations, 1)?;
            let constant = next_value_id(&mut prefix.next_value)?;
            prefix
                .entry_operations
                .push(ProductionRankedOperationV1::IndexConstant {
                    result: constant,
                    value: fact.required(),
                });
            self.sites[source] = Some(Site {
                fact,
                slot,
                constant,
                ranked_block: None,
            });
        }
        Ok(())
    }
    pub(super) fn record(&mut self, source: usize, ranked: u32) -> Result<()> {
        let site = self
            .sites
            .get_mut(source)
            .and_then(Option::as_mut)
            .ok_or(Error::Incomplete(
                "checked-view emitted source site is absent",
            ))?;
        if site.ranked_block.is_some() {
            return Err(Error::Incomplete(
                "checked-view source emitted more than once",
            ));
        }
        site.ranked_block = Some(ranked);
        Ok(())
    }
    fn validate_namespace(&self, resources: &mut PreparationResourcesV1<'_, '_>) -> Result<()> {
        if !resources.is_metered() || resources.has_denial() {
            return Err(resource(Resource::Accounting));
        }
        // Two bounded pairwise uniqueness scans plus all fixed field checks.
        resources.work(64 + MAX_BLOCKS * MAX_BLOCKS * 24)?;
        if self.length_count > MAX_BLOCKS {
            return Err(Error::Incomplete(
                "checked-view length argument count exceeds its fixed namespace",
            ));
        }
        for (index, row) in self.lengths.iter().enumerate() {
            if index >= self.length_count {
                if row.is_some() {
                    return Err(Error::Incomplete(
                        "checked-view length row exceeds its census",
                    ));
                }
                continue;
            }
            let row = row.ok_or(Error::Incomplete(
                "checked-view length namespace has a hole",
            ))?;
            if row.slot as usize != index + 1 {
                return Err(Error::Incomplete(
                    "checked-view length slot aliases the extent or another argument",
                ));
            }
            for previous in self.lengths[..index].iter().flatten() {
                if row.parameter == previous.parameter
                    || row.source_local == previous.source_local
                    || row.source_argument == previous.source_argument
                {
                    return Err(Error::Incomplete(
                        "checked-view length identity is duplicated",
                    ));
                }
            }
        }
        for (source, site) in self.sites.iter().enumerate() {
            let Some(site) = site else { continue };
            let index = site.slot.checked_sub(1).ok_or(Error::Incomplete(
                "checked-view length cannot use extent slot zero",
            ))? as usize;
            let row = self
                .lengths
                .get(index)
                .copied()
                .flatten()
                .ok_or(Error::Incomplete(
                    "checked-view length slot has no original identity",
                ))?;
            if source != site.fact.source_block()
                || row.slot != site.slot
                || row.parameter != site.fact.parameter()
                || row.source_local != site.fact.source_local()
                || row.source_argument != site.fact.source_argument()
                || site.fact.required() != 256
                || site.fact.success() == site.fact.failure()
            {
                return Err(Error::Incomplete(
                    "checked-view condition and length identity disagree",
                ));
            }
            for previous in self.sites[..source].iter().flatten() {
                if previous.constant == site.constant
                    || (site.ranked_block.is_some() && previous.ranked_block == site.ranked_block)
                {
                    return Err(Error::Incomplete(
                        "checked-view emitted coordinate is duplicated",
                    ));
                }
            }
        }
        if !self.prepared && (self.length_count != 0 || self.sites.iter().any(Option::is_some)) {
            return Err(Error::Incomplete(
                "checked-view populated namespace was not prepared",
            ));
        }
        Ok(())
    }
    pub(super) fn rejoin(
        &self,
        flow: &NominalPreparedControlFlowV1<'_, '_>,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<()> {
        self.validate_namespace(resources)?;
        resources.work(64 + MAX_BLOCKS * 16)?;
        if !self.prepared || self.length_count > MAX_BLOCKS {
            return Err(Error::Incomplete(
                "checked-view source preparation is absent",
            ));
        }
        for (source, expected) in flow.checked_views().iter().enumerate() {
            if self.sites[source].map(|site| site.fact) != *expected {
                return Err(Error::Incomplete(
                    "checked-view certificate differs from original source loan",
                ));
            }
            if let Some(site) = self.sites[source] {
                let row = self
                    .lengths
                    .get(site.slot.checked_sub(1).ok_or(Error::Incomplete(
                        "checked-view length cannot use extent slot zero",
                    ))? as usize)
                    .copied()
                    .flatten()
                    .ok_or(Error::Incomplete(
                        "checked-view dedicated length argument is absent",
                    ))?;
                if site.slot == 0
                    || row.slot != site.slot
                    || row.parameter != site.fact.parameter()
                    || row.source_local != site.fact.source_local()
                    || row.source_argument != site.fact.source_argument()
                    || site.ranked_block.is_none()
                {
                    return Err(Error::Incomplete(
                        "checked-view source and length argument differ",
                    ));
                }
            }
        }
        Ok(())
    }
    pub(super) fn validate(
        &self,
        blocks: &[ProductionRankedBlockV1],
        bases: &[Option<usize>; MAX_BLOCKS],
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<()> {
        self.validate_namespace(resources)?;
        resources.work(64 + MAX_BLOCKS * 16)?;
        for site in self.sites.iter().flatten() {
            let block = site
                .ranked_block
                .ok_or(Error::Incomplete("checked-view branch was not emitted"))?;
            let expected = expected_terminator(*site, bases)?;
            if blocks
                .get(block as usize)
                .map(ProductionRankedBlockV1::terminator)
                != Some(&expected)
            {
                return Err(Error::Incomplete(
                    "checked-view comparison or edge polarity changed",
                ));
            }
            let mut constants = 0usize;
            for operation in blocks
                .first()
                .ok_or(Error::Incomplete("checked-view entry absent"))?
                .operations()
            {
                resources.work(8)?;
                if let ProductionRankedOperationV1::IndexConstant { result, value } = operation {
                    if *result == site.constant {
                        if *value != site.fact.required() {
                            return Err(Error::Incomplete("checked-view required bound changed"));
                        }
                        constants += 1;
                    }
                }
            }
            if constants != 1 {
                return Err(Error::Incomplete(
                    "checked-view exact prefix bound is absent or duplicate",
                ));
            }
        }
        Ok(())
    }
}
fn same_edges(first: usize, second: usize, success: usize, failure: usize) -> bool {
    success != failure
        && ((first == success && second == failure) || (first == failure && second == success))
}
pub(super) fn expected_terminator(
    site: Site,
    bases: &[Option<usize>; MAX_BLOCKS],
) -> Result<ProductionRankedTerminatorV1> {
    let target = |source| {
        bases
            .get(source)
            .copied()
            .flatten()
            .ok_or(Error::Incomplete("checked-view edge was pruned"))
            .and_then(ranked_block_id)
    };
    Ok(ProductionRankedTerminatorV1::IndexLessThan {
        lhs: ProductionRankedValueV1::Argument(site.slot),
        rhs: ProductionRankedValueV1::Local(site.constant),
        true_block: target(site.fact.failure())?,
        false_block: target(site.fact.success())?,
    })
}

#[cfg(test)]
#[path = "bf16_nominal_checked_control_v1_tests.rs"]
mod tests;
