//! Coordinate consistency, not authentication of the retained source owner.

use crate::ExecutionCapabilitySourceV1;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Custody {
    Original,
    Expanded {
        source_root: [u8; 32],
        expansion: [u8; 32],
        expanded_root: [u8; 32],
    },
}

#[derive(Eq, Ord, PartialEq, PartialOrd)]
struct Location {
    operation: [u8; 32],
    block: u32,
    occurrence: Option<(u32, u32)>,
}

#[derive(Default)]
pub(super) struct SourceLocations {
    custody: Option<Custody>,
    locations: BTreeSet<Location>,
}

impl SourceLocations {
    pub(super) fn insert(
        &mut self,
        source: ExecutionCapabilitySourceV1,
    ) -> Result<(), &'static str> {
        let (custody, occurrence) = match source.occurrence {
            None => (Custody::Original, None),
            Some(site) => (
                Custody::Expanded {
                    source_root: site.root_source_identity(),
                    expansion: site.expansion_identity(),
                    expanded_root: site.expanded_root_identity(),
                },
                Some((site.caller_instance(), site.expanded_block())),
            ),
        };
        if self.custody.is_some_and(|previous| previous != custody) {
            return Err("execution source occurrences disagree on their root, expansion, or mode");
        }
        self.custody = Some(custody);
        if !self.locations.insert(Location {
            operation: source.operation,
            block: source.block,
            occurrence,
        }) {
            return Err("execution source operation identity/location was duplicated or replayed");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
