//! Fixed source-call record for the exact MIR37 grammar.
//! These are inert observations. Only the live backend producer supplies source
//! custody; decoding or calling this constructor cannot authenticate Rust.
use super::*;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
/// Fixed inert identity observations for the exact source marker call.
pub struct SemanticPhysicalEntrySourceV37 {
    root_axes: [[u8; 32]; 5],
    mir_body: [u8; 32],
    block_identity: [u8; 32],
    source_signature: [u8; 32],
    rustc_fn_abi: [u8; 32],
    frontend_bytes_sha256: [u8; 32],
    raw_block: u32,
    occurrence: u8,
}
impl SemanticPhysicalEntrySourceV37 {
    /// Fixed-size data only. This does not validate a source occurrence.
    pub fn new(
        root_axes: [[u8; 32]; 5],
        mir_body: [u8; 32],
        block_identity: [u8; 32],
        source_signature: [u8; 32],
        rustc_fn_abi: [u8; 32],
        frontend_bytes_sha256: [u8; 32],
        (raw_block, occurrence): (u32, u8),
    ) -> Result<Self, &'static str> {
        if usize::from(occurrence)
            >= super::physical_entry_v37::SEMANTIC_PHYSICAL_ENTRY_MAX_OCCURRENCES_V37
        {
            return Err("physical-entry occurrence exceeds closed bound");
        }
        if root_axes
            .into_iter()
            .chain([
                mir_body,
                block_identity,
                source_signature,
                rustc_fn_abi,
                frontend_bytes_sha256,
            ])
            .any(|value| value == [0; 32])
        {
            return Err("physical-entry source record has an empty identity");
        }
        Ok(Self {
            root_axes,
            mir_body,
            block_identity,
            source_signature,
            rustc_fn_abi,
            frontend_bytes_sha256,
            raw_block,
            occurrence,
        })
    }
    /// Five exact source root identity axes.
    pub const fn root_axes(self) -> [[u8; 32]; 5] {
        self.root_axes
    }
    /// Observed rustc MIR body identity.
    pub const fn mir_body(self) -> [u8; 32] {
        self.mir_body
    }
    /// Observed source block identity.
    pub const fn block_identity(self) -> [u8; 32] {
        self.block_identity
    }
    /// Observed normalized source signature identity.
    pub const fn source_signature(self) -> [u8; 32] {
        self.source_signature
    }
    /// Observed rustc physical ABI identity.
    pub const fn rustc_fn_abi(self) -> [u8; 32] {
        self.rustc_fn_abi
    }
    /// Observed exact frontend registration bytes identity.
    pub const fn frontend_bytes_sha256(self) -> [u8; 32] {
        self.frontend_bytes_sha256
    }
    /// Original raw rustc MIR block index.
    pub const fn raw_block(self) -> u32 {
        self.raw_block
    }

    /// Exact source-order occurrence in the bounded live census.
    pub const fn occurrence(self) -> u8 {
        self.occurrence
    }

    /// Full current declaration comparison, not a function-name heuristic.
    pub fn matches_function(self, function: &SemanticFunctionDeclV1) -> bool {
        self.root_axes
            == [
                *function.identity().as_bytes(),
                *function.item_definition_identity().as_bytes(),
                *function.monomorphization_identity().as_bytes(),
                *function.generic_type_arguments_identity().as_bytes(),
                *function.const_generic_arguments_identity().as_bytes(),
            ]
    }
}
