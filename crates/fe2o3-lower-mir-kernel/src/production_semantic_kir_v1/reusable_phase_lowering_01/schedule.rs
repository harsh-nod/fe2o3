//! Once-only insertion at the already checked original source event sites.
//! This cursor has no authority to invent values or discharge source checks.
use super::*;
use fe2o3_pliron::ProductionSemanticSsaSourceSiteV1 as Site;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Visit {
    Pending,
    Visiting,
    Complete,
}

#[derive(Clone, Copy, Debug)]
struct SiteRows {
    site: Site,
    start: usize,
    end: usize,
    visit: Visit,
}

pub(super) struct ActionSchedule {
    rows: Vec<(usize, PhaseEmissionRowV1)>,
    sites: Vec<SiteRows>,
    failed: bool,
}

impl ActionSchedule {
    pub(super) fn new(checked: &CheckedRows<'_>, work: &mut usize) -> PhaseResult<Self> {
        let mut rows: Vec<(usize, PhaseEmissionRowV1)> = reserve(checked.order.len(), work)?;
        let mut sites: Vec<SiteRows> = reserve(checked.order.len(), work)?;
        for index in &checked.order {
            spend(work, 1)?;
            let row = checked.input.rows[*index];
            if let Some((_, last)) = rows.last() {
                if (last.boundary.site.block().index(), last.boundary.event)
                    >= (row.boundary.site.block().index(), row.boundary.event)
                {
                    return Err(rejected("phase schedule changed the checked event order"));
                }
            }
            let site = row.boundary.site;
            if let Some(last) = sites.last_mut().filter(|last| last.site == site) {
                last.end += 1;
            } else {
                if sites
                    .last()
                    .is_some_and(|last| site_key(last.site) >= site_key(site))
                {
                    return Err(rejected(
                        "phase source events do not form ordered insertion sites",
                    ));
                }
                sites.push(SiteRows {
                    site,
                    start: rows.len(),
                    end: rows.len() + 1,
                    visit: Visit::Pending,
                });
            }
            rows.push((*index, row));
        }
        Ok(Self {
            rows,
            sites,
            failed: false,
        })
    }

    /// Called by the ordinary source statement/terminator driver, never by
    /// iterating recipe rows and emitting them into a synthetic block.
    pub(super) fn consume_site(
        &mut self,
        site: Site,
        work: &mut usize,
        mut emit: impl FnMut(usize, PhaseEmissionRowV1, &mut usize) -> PhaseResult<()>,
    ) -> PhaseResult<bool> {
        if self.failed {
            return Err(rejected("phase schedule was already abandoned"));
        }
        let index = match self.find(site, work) {
            Ok(index) => index,
            Err(error) => {
                self.failed = true;
                return Err(error);
            }
        };
        let Some(index) = index else {
            return Ok(false);
        };
        let selected = self.sites[index];
        if selected.visit != Visit::Pending {
            self.failed = true;
            return Err(rejected("phase emission visited an original site twice"));
        }
        self.sites[index].visit = Visit::Visiting;
        for (index, row) in &self.rows[selected.start..selected.end] {
            let result = spend(work, 1).and_then(|()| emit(*index, *row, work));
            if let Err(error) = result {
                self.failed = true;
                return Err(error);
            }
        }
        self.sites[index].visit = Visit::Complete;
        Ok(true)
    }

    pub(super) fn complete(&self, work: &mut usize) -> PhaseResult<()> {
        if self.failed {
            return Err(rejected("phase schedule was already abandoned"));
        }
        for site in &self.sites {
            spend(work, 1)?;
            if site.visit != Visit::Complete {
                return Err(rejected(
                    "phase lowering omitted an original lifecycle site",
                ));
            }
        }
        Ok(())
    }

    fn find(&self, site: Site, work: &mut usize) -> PhaseResult<Option<usize>> {
        let key = site_key(site);
        let (mut lo, mut hi) = (0usize, self.sites.len());
        while lo < hi {
            spend(work, 1)?;
            let middle = lo + (hi - lo) / 2;
            match site_key(self.sites[middle].site).cmp(&key) {
                std::cmp::Ordering::Less => lo = middle + 1,
                std::cmp::Ordering::Greater => hi = middle,
                std::cmp::Ordering::Equal => return Ok(Some(middle)),
            }
        }
        Ok(None)
    }
}

fn site_key(site: Site) -> (u32, Option<u32>) {
    // Statements precede the terminator. `None` must not sort first.
    (
        site.block().index(),
        Some(site.statement().unwrap_or(u32::MAX)),
    )
}

#[cfg(test)]
#[path = "schedule_tests.rs"]
mod tests;
