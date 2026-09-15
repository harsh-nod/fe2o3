//! Merge an ordered statement walk with the existing source-checked capture map.
use super::*;
use std::collections::btree_map;
use std::iter::Peekable;

type Site = SemanticTransparentBorrowSiteV1;
type Result<T> = std::result::Result<T, ProductionSemanticSsaErrorV1>;

pub(in super::super::super) struct CaptureCursor<'map, 'source> {
    entries: Peekable<btree_map::Iter<'map, Site, Capture<'source>>>,
    previous: Option<Site>,
    empty: bool,
    failed: bool,
}

impl<'map, 'source> CaptureCursor<'map, 'source> {
    pub(super) fn new(
        sites: &'map BTreeMap<Site, Capture<'source>>,
        charge: &mut impl FnMut(usize) -> Result<()>,
    ) -> Result<Self> {
        if !sites.is_empty() {
            // Fixed cursor storage and logical ordered-map initialization. No
            // allocation or claim about the standard library's tree fanout.
            charge(size_of::<Self>().div_ceil(size_of::<usize>()) + key_work(sites.len()))?;
        }
        Ok(Self {
            entries: sites.iter().peekable(),
            previous: None,
            empty: sites.is_empty(),
            failed: false,
        })
    }

    pub(in super::super::super) fn captured(
        &mut self,
        site: Site,
        kind: &SemanticStatementKindV1,
        charge: &mut impl FnMut(usize) -> Result<()>,
    ) -> Result<Option<&'map [u32]>> {
        if self.failed {
            return Err(mismatch());
        }
        let result = self.next_capture(site, kind, charge);
        // A partially advanced cursor must not be resumed with fresh work or a
        // different query after any failure. The production caller propagates it.
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    fn next_capture(
        &mut self,
        site: Site,
        kind: &SemanticStatementKindV1,
        charge: &mut impl FnMut(usize) -> Result<()>,
    ) -> Result<Option<&'map [u32]>> {
        if self.empty {
            return Ok(None);
        }
        charge(1)?;
        if self.previous.is_some_and(|previous| previous >= site) {
            return Err(mismatch());
        }
        self.previous = Some(site);
        loop {
            // One logical peek/comparison per query and per skipped record.
            charge(1)?;
            let Some(&(key, capture)) = self.entries.peek() else {
                return Ok(None);
            };
            if *key > site {
                return Ok(None);
            }
            charge(1)?;
            self.entries.next();
            if *key == site {
                // Exact original node membership remains necessary, including
                // for the authenticated projected getter. No shape-based match.
                return Ok(matches!(kind, SemanticStatementKindV1::Assign(actual)
                    if std::ptr::eq(actual, capture.assignment))
                .then_some(&capture.locals[..capture.len]));
            }
        }
    }
}

#[cfg(test)]
#[path = "cursor_tests.rs"]
mod tests;
