//! Bounded source-occurrence wire data. Structural completeness is not a proof
//! of source correspondence; production lowering must replay its source owner.

use super::{ContractReader, ContractWriter};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExecutionCapabilitySourceOccurrenceV1 {
    root_source_identity: [u8; 32],
    expansion_identity: [u8; 32],
    expanded_root_identity: [u8; 32],
    caller_instance: u32,
    expanded_block: u32,
}

impl ExecutionCapabilitySourceOccurrenceV1 {
    /// Constructs inert wire data, not checked source provenance. The lowerer
    /// obtains every field from its replay-checked execution view and origins.
    pub fn from_untrusted_parts(
        root_source_identity: [u8; 32],
        expansion_identity: [u8; 32],
        expanded_root_identity: [u8; 32],
        caller_instance: u32,
        expanded_block: u32,
    ) -> Option<Self> {
        let value = Self {
            root_source_identity,
            expansion_identity,
            expanded_root_identity,
            caller_instance,
            expanded_block,
        };
        value.is_complete().then_some(value)
    }

    pub const fn root_source_identity(self) -> [u8; 32] {
        self.root_source_identity
    }
    pub const fn expansion_identity(self) -> [u8; 32] {
        self.expansion_identity
    }
    pub const fn expanded_root_identity(self) -> [u8; 32] {
        self.expanded_root_identity
    }
    pub const fn caller_instance(self) -> u32 {
        self.caller_instance
    }
    pub const fn expanded_block(self) -> u32 {
        self.expanded_block
    }

    pub fn is_complete(self) -> bool {
        self.root_source_identity != [0; 32]
            && self.expansion_identity != [0; 32]
            && self.expanded_root_identity != [0; 32]
    }

    pub(super) fn encode(self, writer: &mut ContractWriter) {
        writer.digest(self.root_source_identity);
        writer.digest(self.expansion_identity);
        writer.digest(self.expanded_root_identity);
        writer.u32(self.caller_instance);
        writer.u32(self.expanded_block);
    }

    pub(super) fn decode(reader: &mut ContractReader<'_>) -> Option<Self> {
        Self::from_untrusted_parts(
            reader.digest()?,
            reader.digest()?,
            reader.digest()?,
            reader.u32()?,
            reader.u32()?,
        )
    }
}

#[cfg(test)]
mod tests;
