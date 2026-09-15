//! Closed linear reusable-phase contracts. Source records are inert commitments;
//! the production frontend separately replays its original source/SSA owner.
use crate::{ExecutionCapabilityProvenanceV1, ExecutionCapabilitySignatureV1,
    ExecutionCapabilitySourceV1, ExecutionElementLayoutV1, ExecutionTypeIdentityV1,
    ExecutionSafetyObligationsV1 as Obligations, ValueId};
use sha2::{Digest as _, Sha256};

#[path = "reusable_phase_v1/transition.rs"]
mod transition;
#[path = "reusable_phase_v1/emission.rs"]
mod emission;
#[path = "reusable_phase_v1/ordered.rs"]
mod ordered;
pub use ordered::{ReusablePhaseCheckErrorV1, ReusablePhaseCheckLimitsV1,
    ReusablePhaseCheckUsageV1, verify_reusable_phase_function_v1};
pub use ordered::physical::{ReusablePhasePhysicalOriginV1, ReusablePhasePhysicalProjectionV1,
    ReusablePhasePhysicalResultV1, project_reusable_phase_function_v1};
#[cfg(test)]
#[path = "reusable_phase_v1/tests.rs"]
mod tests;

type Digest = [u8; 32];

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PhaseCallOccurrenceV1 {
    pub source: ExecutionCapabilitySourceV1,
    pub callee_instance: u32,
    pub original_normal_target: u32,
    pub expanded_normal_target: u32,
}
impl PhaseCallOccurrenceV1 {
    pub fn is_complete(self) -> bool {
        self.source.is_complete() && self.source.occurrence.is_some()
            && self.source.occurrence.is_some_and(|o| o.caller_instance() != self.callee_instance)
    }
    pub fn same_expansion(self, other: Self) -> bool {
        match (self.source.occurrence, other.source.occurrence) {
            (Some(a), Some(b)) => a.root_source_identity() == b.root_source_identity()
                && a.expansion_identity() == b.expansion_identity()
                && a.expanded_root_identity() == b.expanded_root_identity(),
            _ => false,
        }
    }
}

/// A retained compiler terminal has an exact caller and normal edge, but no
/// expanded child frame. This inert record is replayed by the source owner.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PhaseTerminalCallOccurrenceV1 {
    pub source: ExecutionCapabilitySourceV1,
    pub original_normal_target: u32,
    pub expanded_normal_target: u32,
}
impl PhaseTerminalCallOccurrenceV1 {
    pub fn is_complete(self) -> bool {
        self.source.is_complete() && self.source.occurrence.is_some()
    }
    pub fn same_expansion(self, other: PhaseCallOccurrenceV1) -> bool {
        same_source_expansion(self.source, other.source)
    }
}

/// Mirrors the two checked expansion origins accepted for wrapper drop only.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PhaseDropOccurrenceV1 {
    Source(PhaseTerminalCallOccurrenceV1),
    CallEntry(PhaseCallOccurrenceV1),
}
impl PhaseDropOccurrenceV1 {
    pub fn is_complete(self) -> bool {
        match self { Self::Source(s) => s.is_complete(), Self::CallEntry(c) => c.is_complete() }
    }
    pub const fn source(self) -> ExecutionCapabilitySourceV1 {
        match self { Self::Source(s) => s.source, Self::CallEntry(c) => c.source }
    }
    pub fn same_expansion(self, other: PhaseCallOccurrenceV1) -> bool {
        same_source_expansion(self.source(), other.source)
    }
}
fn same_source_expansion(a: ExecutionCapabilitySourceV1, b: ExecutionCapabilitySourceV1) -> bool {
    match (a.occurrence, b.occurrence) {
        (Some(a), Some(b)) => a.root_source_identity() == b.root_source_identity()
            && a.expansion_identity() == b.expansion_identity()
            && a.expanded_root_identity() == b.expanded_root_identity(),
        _ => false,
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PhaseDefinedCallV1 {
    pub call: PhaseCallOccurrenceV1,
    pub signature: ExecutionCapabilitySignatureV1,
    pub defined_abi: Digest,
    pub defined_body: Digest,
    pub incoming_count: u32,
    pub incoming_digest: Digest,
    pub source_binding: Digest,
}
impl PhaseDefinedCallV1 {
    pub fn is_complete(self) -> bool {
        self.call.is_complete() && self.signature.output().is_complete()
            && self.signature.arguments().all(ExecutionTypeIdentityV1::is_complete)
            && self.incoming_count != 0
            && [self.defined_abi, self.defined_body, self.incoming_digest, self.source_binding]
                .iter().all(|d| *d != [0; 32])
    }
}

/// Equality label only; every verifier also follows the exact Begin SSA chain.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PhaseKeyV1(Digest);
impl PhaseKeyV1 {
    pub const fn from_untrusted_bytes(bytes: Digest) -> Self { Self(bytes) }
    pub const fn bytes(self) -> Digest { self.0 }
    pub fn for_begin(begin: PhaseCallOccurrenceV1) -> Option<Self> {
        if !begin.is_complete() { return None; }
        let o = begin.source.occurrence?;
        let mut h = Sha256::new();
        h.update(b"fe2o3.kir.reusable-phase.begin.v14\0");
        for digest in [begin.source.function, begin.source.operation, o.root_source_identity(),
            o.expansion_identity(), o.expanded_root_identity()] { h.update(digest); }
        for id in [begin.source.block, o.caller_instance(), o.expanded_block(),
            begin.callee_instance, begin.original_normal_target, begin.expanded_normal_target] {
            h.update(id.to_le_bytes());
        }
        Some(Self(h.finalize().into()))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PhaseLoanStateV1 { Active, Sealed, Returned }
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PhaseStorageRestoreV1 {
    pub reusable_source: ExecutionTypeIdentityV1,
    pub element: ExecutionTypeIdentityV1,
    pub layout: ExecutionElementLayoutV1,
    pub elements: u64,
    pub allocation_anchor_epoch: Digest,
}
impl PhaseStorageRestoreV1 {
    fn is_complete(self) -> bool {
        self.reusable_source.is_complete() && self.element.is_complete()
            && self.layout.checked_footprint(self.elements).is_some_and(|n| n != 0)
            && self.allocation_anchor_epoch != [0; 32]
    }
}
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ReusablePhaseTokenRoleV1 {
    OwnerLoan(PhaseLoanStateV1),
    StorageLoan(PhaseStorageRestoreV1),
    ClosedStorage(PhaseStorageRestoreV1),
    CompletionReady { completion_source: ExecutionTypeIdentityV1, barrier_epoch: Digest },
}
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReusablePhaseTokenTypeV1 {
    pub provenance: ExecutionCapabilityProvenanceV1,
    pub phase: PhaseKeyV1,
    pub owner_source: ExecutionTypeIdentityV1,
    pub owner_anchor_epoch: Digest,
    pub outer_brand: Digest,
    pub phase_brand: Digest,
    pub initial_epoch: Digest,
    pub role: ReusablePhaseTokenRoleV1,
}
impl ReusablePhaseTokenTypeV1 {
    pub fn is_complete(&self) -> bool {
        self.provenance.is_complete() && self.provenance.root.as_str().len() <= 256
            && self.owner_source.is_complete()
            && [self.phase.0, self.owner_anchor_epoch, self.outer_brand, self.phase_brand,
                self.initial_epoch].iter().all(|d| *d != [0; 32])
            && match self.role {
                ReusablePhaseTokenRoleV1::OwnerLoan(_) => true,
                ReusablePhaseTokenRoleV1::StorageLoan(s) | ReusablePhaseTokenRoleV1::ClosedStorage(s) => s.is_complete(),
                ReusablePhaseTokenRoleV1::CompletionReady { completion_source, barrier_epoch } =>
                    completion_source.is_complete() && barrier_epoch != [0; 32],
            }
    }
    pub fn same_phase(&self, other: &Self) -> bool {
        self.provenance == other.provenance && self.phase == other.phase
            && self.owner_source == other.owner_source && self.owner_anchor_epoch == other.owner_anchor_epoch
            && self.outer_brand == other.outer_brand && self.phase_brand == other.phase_brand
            && self.initial_epoch == other.initial_epoch
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PhaseSourcePositionV1 { Statement(u32), Terminator }
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PhaseLifetimeEndV1 {
    pub function: Digest,
    pub instance: u32,
    pub original_block: u32,
    pub original_position: PhaseSourcePositionV1,
    pub expanded_block: u32,
    pub expanded_position: PhaseSourcePositionV1,
}
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PhaseOperationSourceV1 {
    Defined(PhaseDefinedCallV1),
    ClosureReturn { phase: PhaseKeyV1, closure: PhaseDefinedCallV1, pack_block: u32,
        pack_statement: u32, return_block: u32, source_protocol: Digest },
    WrapperDrop { phase: PhaseKeyV1, drop_call: PhaseDropOccurrenceV1, drop_abi: Digest,
        source_protocol: Digest },
    LeaseEnd { phase: PhaseKeyV1, bind: PhaseCallOccurrenceV1, source_event: PhaseLifetimeEndV1,
        source_protocol: Digest },
    WrapperEnd { phase: PhaseKeyV1, wrapper_normal_target: u32, source_protocol: Digest },
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ReusablePhaseOperationV1 {
    OwnerConvert { workgroup: ExecutionTypeIdentityV1, owner: ExecutionTypeIdentityV1 },
    Begin { owner_reference: ExecutionTypeIdentityV1, owner: ExecutionTypeIdentityV1,
        phase_workgroup: ExecutionTypeIdentityV1, outer_brand: Digest, phase_brand: Digest,
        dynamic_epoch: Digest, wrapper: PhaseDefinedCallV1, invoke: PhaseDefinedCallV1,
        source_protocol: Digest },
    Bind { phase_reference: ExecutionTypeIdentityV1, storage_reference: ExecutionTypeIdentityV1,
        phase_workgroup: ExecutionTypeIdentityV1, reusable_storage: ExecutionTypeIdentityV1,
        phase_lds: ExecutionTypeIdentityV1, element: ExecutionTypeIdentityV1,
        layout: ExecutionElementLayoutV1, elements: u64 },
    Seal { workgroup_before_barrier: ExecutionTypeIdentityV1,
        workgroup_after_barrier: ExecutionTypeIdentityV1, completion: ExecutionTypeIdentityV1,
        barrier_call: PhaseTerminalCallOccurrenceV1 },
    RelayClosure { completion: ExecutionTypeIdentityV1 },
    RelayDrop { completion: ExecutionTypeIdentityV1 },
    CloseStorage { last_lease: ExecutionTypeIdentityV1 },
    End { storage_count: u8 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReusablePhaseOpV1 {
    pub operands: Vec<ValueId>,
    pub operation: ReusablePhaseOperationV1,
    pub provenance: ExecutionCapabilityProvenanceV1,
    pub source: PhaseOperationSourceV1,
    pub obligations: Obligations,
}
impl ReusablePhaseOperationV1 {
    pub fn arity(&self) -> (usize, usize) {
        match self {
            Self::OwnerConvert { .. } => (1, 1), Self::Begin { .. } => (1, 2),
            Self::Bind { .. } => (3, 3), Self::Seal { .. } => (2, 2),
            Self::RelayClosure { .. } => (1, 1), Self::RelayDrop { .. } => (2, 2),
            Self::CloseStorage { .. } => (2, 1),
            Self::End { storage_count } => (2 + usize::from(*storage_count), 1 + usize::from(*storage_count)),
        }
    }
    pub const fn required_obligations(&self) -> u32 {
        let b = Obligations::TARGET_SUPPORT | Obligations::DYNAMIC_WORKGROUP_IDENTITY
            | Obligations::LIFETIME_VALIDITY | Obligations::ALIASING_VALIDITY;
        let c = Obligations::WORKGROUP_CONVERGENCE | Obligations::EXACT_PARTICIPATION;
        match self {
            Self::OwnerConvert { .. } | Self::RelayClosure { .. } | Self::RelayDrop { .. } => b,
            Self::Begin { .. } => b | c,
            Self::Bind { .. } | Self::CloseStorage { .. } => b | Obligations::DISJOINT_LDS_ALLOCATION,
            Self::Seal { .. } => b | c | Obligations::MEMORY_MODEL,
            Self::End { .. } => b | c | Obligations::DISJOINT_LDS_ALLOCATION | Obligations::MEMORY_MODEL,
        }
    }
}
