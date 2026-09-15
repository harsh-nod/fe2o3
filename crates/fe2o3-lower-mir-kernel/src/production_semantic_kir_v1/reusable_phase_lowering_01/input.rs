//! Inert emission requests. These fields never authenticate original Rust
//! source. Production must supply them while its private live seal is retained.
use super::*;
use fe2o3_mir_model::{SemanticCallInstanceIdV1, SsaValueV1, SsaVariableIdV1};
use fe2o3_pliron::ProductionSemanticSsaSourceSiteV1;

/// Expected SSA event at an original source boundary, checked during replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhaseBoundaryKindV1 {
    /// Introduce the boundary's exact SSA value.
    Define,
    /// Consume a use of the boundary's current SSA value.
    Use,
    /// End the boundary variable's live value.
    Kill,
}

/// Coordinates to match against the retained source-to-SSA event mapping.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhaseEmissionBoundaryV1 {
    /// Original statement or terminator in the expanded source function.
    pub site: ProductionSemanticSsaSourceSiteV1,
    /// Event ordinal within the site's SSA block.
    pub event: u32,
    /// SSA variable selected by the source boundary.
    pub variable: SsaVariableIdV1,
    /// Exact SSA definition or block argument carried by that event.
    pub value: SsaValueV1,
    /// Required event kind, not inferred from the requested emission action.
    pub kind: PhaseBoundaryKindV1,
}

/// Requested lifecycle transition; source ownership is verified separately.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhaseEmissionActionV1 {
    /// Convert the original workgroup capability into its reusable owner.
    OwnerConvert,
    /// Begin the phase at the original issue constructor's return.
    Begin,
    /// Bind an existing allocation to this phase's storage lease.
    Bind {
        /// Index in the owning occurrence's checked lease roster.
        lease: usize,
    },
    /// Seal the issued phase at its original finish call.
    Seal,
    /// Transfer the closure's completion into the wrapper's relay.
    RelayClosure,
    /// Consume the wrapper's completion relay at its original drop.
    RelayDrop,
    /// End a storage lease at its original lifetime boundary.
    CloseStorage {
        /// Index in the owning occurrence's checked lease roster.
        lease: usize,
    },
    /// End the completed phase and restore its reusable owner.
    End,
}

/// One ordered action tied to one retained source boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhaseEmissionRowV1 {
    /// Index in the request's phase-occurrence roster.
    pub phase: usize,
    /// Lifecycle transition to replay and emit.
    pub action: PhaseEmissionActionV1,
    /// Exact source and SSA event that must justify this transition.
    pub boundary: PhaseEmissionBoundaryV1,
}

/// Source call instances and SSA values for one allocation lease, not authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhaseLeaseEmissionInputV1 {
    /// Expanded instance of the original storage-binding call.
    pub bind: SemanticCallInstanceIdV1,
    /// Expanded instance converting the allocation into storage.
    pub storage_conversion: SemanticCallInstanceIdV1,
    /// Original allocation's SSA value.
    pub allocation: SsaValueV1,
    /// Root allocation reference used by the storage conversion.
    pub root_reference: SsaValueV1,
    /// Current phase reference supplied to the binding call.
    pub phase_reference: SsaValueV1,
    /// Converted storage reference supplied to the binding call.
    pub storage_reference: SsaValueV1,
    /// Exact SSA result of the storage-binding call.
    pub result: SsaValueV1,
    /// SSA block and event ordinal of the original allocation borrow.
    pub borrowed_at: (SsaBlockIdV1, u32),
}

/// Replay coordinates for one source phase and its complete allocation roster.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhaseOccurrenceEmissionInputV1 {
    /// Expanded owner-conversion call instance.
    pub owner: SemanticCallInstanceIdV1,
    /// Expanded phase-wrapper call instance.
    pub wrapper: SemanticCallInstanceIdV1,
    /// Expanded phase-issue constructor instance.
    pub issue: SemanticCallInstanceIdV1,
    /// Expanded phase-finish call instance.
    pub finish: SemanticCallInstanceIdV1,
    /// Expanded body instance of the wrapper's original closure.
    pub closure: SemanticCallInstanceIdV1,
    /// Workgroup capability supplied to the owner conversion.
    pub owner_workgroup: SsaValueV1,
    /// SSA result of the owner conversion.
    pub converted_owner: SsaValueV1,
    /// Current owner reference supplied to phase issue.
    pub owner_reference: SsaValueV1,
    /// SSA result of the issue constructor.
    pub issued_phase: SsaValueV1,
    /// Issued phase transferred into the closure's original parameter.
    pub closure_phase: SsaValueV1,
    /// Completion value relayed through the wrapper.
    pub relay: SsaValueV1,
    /// Completion value returned by the closure.
    pub completion: SsaValueV1,
    /// SSA block and event ordinal at phase issue.
    pub begin: (SsaBlockIdV1, u32),
    /// Complete, ordered allocation leases belonging to this occurrence.
    pub leases: Vec<PhaseLeaseEmissionInputV1>,
}

/// Replay data only. Decoding or constructing this request is not a source
/// proof, and cannot turn the ordinary lowerer entry point into phase lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhaseEmissionInputV1 {
    /// Original semantic root whose retained expansion is consumed.
    pub root: SemanticFunctionIdV1,
    /// Digest of the retained source semantic module.
    pub semantic: [u8; 32],
    /// Identity of the retained defined-call expansion.
    pub expansion: [u8; 32],
    /// Identity of this root's expanded execution view.
    pub expanded_root: [u8; 32],
    /// Identity of the original source phase protocol, not a source proof.
    pub source_protocol: [u8; 32],
    /// Existing expansion-owned immutable bindings, not newly minted records.
    pub bindings: Vec<SemanticExpandedDefinedCapabilityV1>,
    /// Complete source-ordered phase occurrences to replay.
    pub phases: Vec<PhaseOccurrenceEmissionInputV1>,
    /// Complete source-boundary action schedule to replay.
    pub rows: Vec<PhaseEmissionRowV1>,
}

impl PhaseEmissionInputV1 {
    pub(super) fn retained_bytes(&self) -> Option<usize> {
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(
                self.bindings
                    .capacity()
                    .checked_mul(std::mem::size_of::<SemanticExpandedDefinedCapabilityV1>())?,
            )?
            .checked_add(
                self.phases
                    .capacity()
                    .checked_mul(std::mem::size_of::<PhaseOccurrenceEmissionInputV1>())?,
            )?
            .checked_add(
                self.rows
                    .capacity()
                    .checked_mul(std::mem::size_of::<PhaseEmissionRowV1>())?,
            )?;
        for phase in &self.phases {
            bytes = bytes.checked_add(
                phase
                    .leases
                    .capacity()
                    .checked_mul(std::mem::size_of::<PhaseLeaseEmissionInputV1>())?,
            )?;
        }
        for binding in &self.bindings {
            bytes = bytes.checked_add(binding.retained_auxiliary_bytes()?)?;
        }
        Some(bytes)
    }
}
