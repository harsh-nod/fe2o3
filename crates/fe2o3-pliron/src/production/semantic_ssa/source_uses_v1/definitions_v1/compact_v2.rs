//! Checked physical encoding of correspondence, not a new source of SSA values.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DefinitionOrigin {
    Missing,
    Entry { argument: usize },
    Event { block: SsaBlockIdV1, event: u32 },
    Edge { incoming: usize, definition: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DefinitionRow {
    tagged: u32,
    coordinate: u32,
}

impl DefinitionRow {
    const TAG_SHIFT: u32 = 30;
    const MASK: u32 = (1 << Self::TAG_SHIFT) - 1;
    pub(super) const MISSING: Self = Self {
        tagged: 3 << Self::TAG_SHIFT,
        coordinate: 0,
    };

    pub(super) fn new(origin: DefinitionOrigin) -> Result<Self, ProductionSemanticSsaErrorV1> {
        let overflow = || ProductionSemanticSsaErrorV1::ResourceOverflow;
        let (tag, low, coordinate) = match origin {
            DefinitionOrigin::Missing => return Ok(Self::MISSING),
            DefinitionOrigin::Entry { argument } => {
                (1, 0, u32::try_from(argument).map_err(|_| overflow())?)
            }
            DefinitionOrigin::Event { block, event } => (0, block.get(), event),
            DefinitionOrigin::Edge {
                incoming,
                definition,
            } => (
                2,
                u32::try_from(incoming).map_err(|_| overflow())?,
                u32::try_from(definition).map_err(|_| overflow())?,
            ),
        };
        if low > Self::MASK {
            return Err(overflow());
        }
        Ok(Self {
            tagged: (tag << Self::TAG_SHIFT) | low,
            coordinate,
        })
    }

    pub(super) const fn origin(self) -> DefinitionOrigin {
        let low = self.tagged & Self::MASK;
        match self.tagged >> Self::TAG_SHIFT {
            0 => DefinitionOrigin::Event {
                block: SsaBlockIdV1::new(low),
                event: self.coordinate,
            },
            1 => DefinitionOrigin::Entry {
                argument: self.coordinate as usize,
            },
            2 => DefinitionOrigin::Edge {
                incoming: low as usize,
                definition: self.coordinate as usize,
            },
            _ => DefinitionOrigin::Missing,
        }
    }

    pub(super) fn hash_into(self, hash: &mut Sha256) {
        hash.update(self.tagged.to_le_bytes());
        hash.update(self.coordinate.to_le_bytes());
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct IncomingRow {
    source: u32,
    target: u32,
    ordinal_role: u32,
}

impl IncomingRow {
    pub(super) fn new(
        edge: SsaEdgeIdV1,
        role: SsaEdgeRoleV1,
        target: SsaBlockIdV1,
    ) -> Result<Self, ProductionSemanticSsaErrorV1> {
        // The unchanged planner ceiling allows at most 65536 edges, hence
        // at most ordinal 65535. Roles retain their entire independent u16.
        let ordinal = u16::try_from(edge.ordinal())
            .map_err(|_| ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        Ok(Self {
            source: edge.source().get(),
            target: target.get(),
            ordinal_role: ((role.get() as u32) << 16) | ordinal as u32,
        })
    }
    pub(super) const fn edge(self) -> SsaEdgeIdV1 {
        SsaEdgeIdV1::new(
            SsaBlockIdV1::new(self.source),
            self.ordinal_role & u16::MAX as u32,
        )
    }
    pub(super) const fn role(self) -> SsaEdgeRoleV1 {
        SsaEdgeRoleV1::new((self.ordinal_role >> 16) as u16)
    }
    pub(super) const fn target(self) -> SsaBlockIdV1 {
        SsaBlockIdV1::new(self.target)
    }
    pub(super) fn hash_into(self, hash: &mut Sha256) {
        hash.update(self.source.to_le_bytes());
        hash.update(self.target.to_le_bytes());
        hash.update(self.ordinal_role.to_le_bytes());
    }
}

#[cfg(test)]
mod tests;
