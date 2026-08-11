//! Inert synchronization and atomic contracts for a future Kernel IR V2.
//!
//! This file is intentionally not exported by `fe2o3-kernel-ir`. It defines a
//! bounded schema, a fail-closed validator, a canonical codec, and explicit
//! proof obligations. It does not lower to LLVM, execute on a GPU, establish
//! uniform participation, prove happens-before edges, or prove race freedom.
//!
//! The validator establishes only local structural facts: operation/type and
//! ordering legality, target support, scope/address-space compatibility,
//! canonical identifiers, bounded resources, and well-formed synchronization
//! edges. Every dynamic claim remains in [`VerificationReport::obligations`].

use std::collections::BTreeSet;

pub const SYNCHRONIZATION_V2_MAGIC: [u8; 8] = *b"F2SYNCV2";
pub const SYNCHRONIZATION_V2_VERSION: u16 = 4;
pub const SYNCHRONIZATION_V2_LIMITATIONS: &str = "inert unexported schema; gfx942 wave64 only; \
no LLVM emission, execution, runtime admission, race-freedom proof, uniformity proof, \
happens-before proof, direct fence synchronization, bank-conflict proof, or formal-verification claim";

const HEADER_BYTES: u64 = 32;
const LDS_RECORD_BYTES: u64 = 32;
const EDGE_RECORD_BYTES: u64 = 24;
const EVENT_PREFIX_BYTES: u64 = 28;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SynchronizationLimits {
    pub max_lds_allocations: u32,
    pub max_events: u32,
    pub max_edges: u32,
    pub max_total_lds_bytes: u32,
    pub max_encoded_bytes: u64,
    pub max_obligations: u32,
    pub max_pair_checks: u64,
    pub max_workgroup_participants: u32,
    pub max_cooperative_participants: u32,
}

impl Default for SynchronizationLimits {
    fn default() -> Self {
        Self {
            max_lds_allocations: 512,
            max_events: 4_096,
            max_edges: 16_384,
            max_total_lds_bytes: 64 * 1024,
            max_encoded_bytes: 8 * 1024 * 1024,
            max_obligations: 65_536,
            max_pair_checks: 1_000_000,
            max_workgroup_participants: 1_024,
            max_cooperative_participants: 1_048_576,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum TargetProfile {
    Gfx942Wave64 = 1,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TargetHardLimits {
    pub wave_size: u32,
    pub max_lds_bytes: u32,
    pub max_workgroup_participants: u32,
    pub max_cooperative_participants: u32,
}

impl TargetProfile {
    pub const fn hard_limits(self) -> TargetHardLimits {
        match self {
            Self::Gfx942Wave64 => TargetHardLimits {
                wave_size: 64,
                max_lds_bytes: 64 * 1024,
                max_workgroup_participants: 1_024,
                max_cooperative_participants: 1_048_576,
            },
        }
    }

    pub const fn wave_size(self) -> u32 {
        self.hard_limits().wave_size
    }

    pub const fn max_lds_bytes(self) -> u32 {
        self.hard_limits().max_lds_bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum IntegerWidth {
    W8 = 1,
    W16 = 2,
    W32 = 3,
    W64 = 4,
    W128 = 5,
}

impl IntegerWidth {
    pub const fn bits(self) -> u16 {
        match self {
            Self::W8 => 8,
            Self::W16 => 16,
            Self::W32 => 32,
            Self::W64 => 64,
            Self::W128 => 128,
        }
    }

    pub const fn bytes(self) -> u32 {
        (self.bits() / 8) as u32
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ScalarType {
    Bool,
    Integer { width: IntegerWidth, signed: bool },
    Float32,
    Float64,
    Pointer64,
}

impl ScalarType {
    pub const fn bit_width(self) -> u16 {
        match self {
            Self::Bool => 1,
            Self::Integer { width, .. } => width.bits(),
            Self::Float32 => 32,
            Self::Float64 | Self::Pointer64 => 64,
        }
    }

    pub const fn storage_bytes(self) -> u32 {
        match self {
            Self::Bool => 1,
            _ => (self.bit_width() / 8) as u32,
        }
    }

    const fn is_integer(self) -> bool {
        matches!(self, Self::Integer { .. })
    }

    const fn is_unsigned_integer(self) -> bool {
        matches!(self, Self::Integer { signed: false, .. })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum AddressSpace {
    Private = 1,
    Global = 2,
    Constant = 3,
    Lds = 4,
    Generic = 5,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum MemoryOrdering {
    Relaxed = 1,
    Acquire = 2,
    Release = 3,
    AcquireRelease = 4,
    SequentiallyConsistent = 5,
}

impl MemoryOrdering {
    const fn has_acquire(self) -> bool {
        matches!(
            self,
            Self::Acquire | Self::AcquireRelease | Self::SequentiallyConsistent
        )
    }

    const fn has_release(self) -> bool {
        matches!(
            self,
            Self::Release | Self::AcquireRelease | Self::SequentiallyConsistent
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum MemoryScope {
    Wavefront = 1,
    Workgroup = 2,
    Agent = 3,
    System = 4,
}

impl MemoryScope {
    const fn rank(self) -> u8 {
        self as u8
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MemoryDomains(u8);

impl MemoryDomains {
    pub const NONE: Self = Self(0);
    pub const GLOBAL: Self = Self(1);
    pub const LDS: Self = Self(2);
    pub const ALL: Self = Self(3);

    pub const fn from_bits(bits: u8) -> Option<Self> {
        if bits != 0 && bits & !Self::ALL.0 == 0 {
            Some(Self(bits))
        } else {
            None
        }
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn contains(self, address_space: AddressSpace) -> bool {
        let bit = match address_space {
            AddressSpace::Global => Self::GLOBAL.0,
            AddressSpace::Lds => Self::LDS.0,
            _ => 0,
        };
        bit != 0 && self.0 & bit != 0
    }

    const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EventId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LdsAllocationId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum LdsAllocationKind {
    Static = 1,
    DynamicReservation = 2,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LdsSwizzle {
    Linear,
    Xor { shift: u8 },
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LdsAllocation {
    pub id: LdsAllocationId,
    pub kind: LdsAllocationKind,
    pub bytes: u32,
    pub alignment: u32,
    pub bank_count: u16,
    pub bank_width: u16,
    pub element_stride: u32,
    pub elements: u32,
    pub swizzle: LdsSwizzle,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MemoryRegion {
    pub allocation: u32,
    pub offset: u32,
    pub bytes: u32,
}

/// Untrusted claim naming the authority that can attest system coherence for
/// one exact global allocation. Validation emits an authentication obligation;
/// possession of this value is never itself treated as authority.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CoherentAllocationClaim {
    pub allocation: u32,
    pub authority: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum AtomicDialect {
    Rust = 1,
    AmdGpu = 2,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum AtomicOperation {
    Load = 1,
    Store = 2,
    Exchange = 3,
    CompareExchangeStrong = 4,
    CompareExchangeWeak = 5,
    FetchAdd = 6,
    FetchSub = 7,
    FetchAnd = 8,
    FetchOr = 9,
    FetchXor = 10,
    FetchNand = 11,
    FetchMin = 12,
    FetchMax = 13,
    AmdInc = 14,
    AmdDec = 15,
    FloatAdd = 16,
}

impl AtomicOperation {
    const fn is_compare_exchange(self) -> bool {
        matches!(
            self,
            Self::CompareExchangeStrong | Self::CompareExchangeWeak
        )
    }

    const fn is_load(self) -> bool {
        matches!(self, Self::Load)
    }

    const fn is_store(self) -> bool {
        matches!(self, Self::Store)
    }

    const fn is_min_max(self) -> bool {
        matches!(self, Self::FetchMin | Self::FetchMax)
    }

    const fn is_bitwise(self) -> bool {
        matches!(
            self,
            Self::FetchAnd | Self::FetchOr | Self::FetchXor | Self::FetchNand
        )
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AtomicAccess {
    pub region: MemoryRegion,
    pub dialect: AtomicDialect,
    pub operation: AtomicOperation,
    pub value_type: ScalarType,
    pub address_space: AddressSpace,
    pub alignment: u32,
    pub scope: MemoryScope,
    pub success_ordering: MemoryOrdering,
    pub failure_ordering: Option<MemoryOrdering>,
    pub coherent_allocation: Option<CoherentAllocationClaim>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum AccessKind {
    Read = 1,
    Write = 2,
    ReadWrite = 3,
}

impl AccessKind {
    const fn writes(self) -> bool {
        !matches!(self, Self::Read)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NonAtomicAccess {
    pub region: MemoryRegion,
    pub kind: AccessKind,
    pub value_type: ScalarType,
    pub address_space: AddressSpace,
    pub alignment: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum GroupKind {
    Invocation = 1,
    Subgroup = 2,
    Workgroup = 3,
    CooperativeGrid = 4,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ConvergenceContract {
    NotRequired = 1,
    UniformRequired = 2,
    ExplicitMask = 3,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ParticipationContract {
    pub group: GroupKind,
    pub convergence: ConvergenceContract,
    pub expected_participants: u32,
    pub active_mask: Option<u64>,
}

impl ParticipationContract {
    pub const fn invocation() -> Self {
        Self {
            group: GroupKind::Invocation,
            convergence: ConvergenceContract::NotRequired,
            expected_participants: 1,
            active_mask: None,
        }
    }

    pub const fn full_subgroup(wave_size: u32) -> Self {
        Self {
            group: GroupKind::Subgroup,
            convergence: ConvergenceContract::UniformRequired,
            expected_participants: wave_size,
            active_mask: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum BarrierKind {
    Subgroup = 1,
    Workgroup = 2,
    CooperativeGroup = 3,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Barrier {
    pub kind: BarrierKind,
    pub scope: MemoryScope,
    pub ordering: MemoryOrdering,
    pub domains: MemoryDomains,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Fence {
    pub scope: MemoryScope,
    pub ordering: MemoryOrdering,
    pub domains: MemoryDomains,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum CollectiveKind {
    ReduceAdd = 1,
    ReduceMin = 2,
    ReduceMax = 3,
    InclusiveScanAdd = 4,
    ExclusiveScanAdd = 5,
    Broadcast = 6,
    Any = 7,
    All = 8,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Collective {
    pub kind: CollectiveKind,
    pub value_type: ScalarType,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ShuffleKind {
    Index = 1,
    Up = 2,
    Down = 3,
    Xor = 4,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Shuffle {
    pub kind: ShuffleKind,
    pub value_type: ScalarType,
    pub tile_width: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Ballot {
    pub wave_size: u32,
    pub result_width: IntegerWidth,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EventKind {
    Atomic(AtomicAccess),
    NonAtomic(NonAtomicAccess),
    Fence(Fence),
    Barrier(Barrier),
    Collective(Collective),
    Shuffle(Shuffle),
    Ballot(Ballot),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Event {
    pub id: EventId,
    pub participation: ParticipationContract,
    pub kind: EventKind,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SynchronizationEdgeKind {
    /// Same-participant control-flow ordering. This is an obligation, not a proof.
    ProgramOrder,
    /// A release/acquire pairing intended to synchronize participants.
    SynchronizesWith,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum EventOutcome {
    Unconditional = 1,
    CompareExchangeSuccess = 2,
    CompareExchangeFailure = 3,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ReadFromCondition {
    NotApplicable = 1,
    VerifierMustProve = 2,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SynchronizationEdge {
    pub before: EventId,
    pub after: EventId,
    pub kind: SynchronizationEdgeKind,
    pub scope: MemoryScope,
    pub domains: MemoryDomains,
    pub before_outcome: EventOutcome,
    pub after_outcome: EventOutcome,
    pub read_from: ReadFromCondition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SynchronizationModuleV2 {
    pub target: TargetProfile,
    pub lds_allocations: Vec<LdsAllocation>,
    pub events: Vec<Event>,
    pub edges: Vec<SynchronizationEdge>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Resource {
    LdsAllocations,
    Events,
    Edges,
    TotalLdsBytes,
    EncodedBytes,
    Obligations,
    PairChecks,
    WorkgroupParticipants,
    CooperativeParticipants,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValidationError {
    ResourceLimit {
        resource: Resource,
        observed: u64,
        limit: u64,
    },
    ArithmeticOverflow,
    NonCanonicalLdsId {
        position: u32,
        actual: LdsAllocationId,
    },
    NonCanonicalEventId {
        position: u32,
        actual: EventId,
    },
    NonCanonicalEdgeOrder,
    DuplicateEdge,
    InvalidLdsAllocation(LdsAllocationId),
    UnsupportedLdsBanking(LdsAllocationId),
    InvalidParticipation(EventId),
    UnsupportedCooperativeGroup(EventId),
    InvalidAtomicType(EventId),
    InvalidAtomicOperation(EventId),
    UnsupportedAtomicWidth {
        event: EventId,
        bits: u16,
    },
    UnsupportedPlatformOperation(EventId),
    InvalidAtomicOrdering(EventId),
    InvalidCompareExchangeOrdering(EventId),
    InvalidCoherentAllocationClaim(EventId),
    InvalidAddressSpace(EventId),
    InvalidScope(EventId),
    InvalidAlignment(EventId),
    InvalidMemoryRegion(EventId),
    IncompatibleAtomicObject {
        first: EventId,
        second: EventId,
    },
    IncompatibleAtomicScope {
        first: EventId,
        second: EventId,
        required: MemoryScope,
    },
    UnknownLdsAllocation {
        event: EventId,
        allocation: LdsAllocationId,
    },
    InvalidFence(EventId),
    InvalidBarrier(EventId),
    InvalidCollective(EventId),
    InvalidShuffle(EventId),
    InvalidBallot(EventId),
    UnknownEdgeEndpoint {
        edge: u32,
        endpoint: EventId,
    },
    BackwardOrSelfEdge(u32),
    InvalidEdgeEndpointKind(u32),
    IncompatibleEdgeScope(u32),
    IncompatibleEdgeDomains(u32),
    EncodingSizeOverflow,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum VerifierObligation {
    UniformParticipation {
        event: EventId,
        group: GroupKind,
        expected_participants: u32,
        active_mask: Option<u64>,
    },
    CooperativeParticipation {
        event: EventId,
        expected_participants: u32,
    },
    HappensBefore {
        edge: u32,
        before: EventId,
        after: EventId,
        kind: SynchronizationEdgeKind,
        scope: MemoryScope,
        domains: MemoryDomains,
        before_outcome: EventOutcome,
        after_outcome: EventOutcome,
        read_from: ReadFromCondition,
        before_participation: ParticipationContract,
        after_participation: ParticipationContract,
        before_kind: EventKind,
        after_kind: EventKind,
        participant_witness: ParticipantWitness,
        operation_witness: SynchronizationOperationWitness,
    },
    NonAtomicConflict {
        first: EventId,
        second: EventId,
        address_space: AddressSpace,
        structurally_ordered: bool,
        aliasing: AliasingCondition,
    },
    DischargeAllocationAlias {
        first: EventId,
        second: EventId,
        address_space: AddressSpace,
        first_region: MemoryRegion,
        second_region: MemoryRegion,
        consequence: AllocationAliasConsequence,
    },
    ScopeCompatibility {
        first: EventId,
        second: EventId,
        required_scope: MemoryScope,
    },
    AuthenticateCoherentAllocation {
        event: EventId,
        allocation: u32,
        authority: u64,
    },
    LdsBankMapping {
        allocation: LdsAllocationId,
        base_offset: u32,
        bank_count: u16,
        bank_width: u16,
        element_stride: u32,
        swizzle: LdsSwizzle,
    },
    FenceSemantics {
        event: EventId,
        participation: ParticipationContract,
        fence: Fence,
    },
    BarrierSemantics {
        event: EventId,
        participation: ParticipationContract,
        barrier: Barrier,
    },
    CollectiveSemantics {
        event: EventId,
        participation: ParticipationContract,
        collective: Collective,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AliasingCondition {
    ConfirmedOverlap,
    VerifierMustProveDisjoint,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AllocationAliasConsequence {
    ReadOnlyOverlap,
    NonAtomicConflict,
    AtomicObjectCompatibility,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ParticipantWitness {
    SameParticipantMustProve,
    SynchronizingParticipantsMustProve,
    SameBarrierCohortMustProve,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SynchronizationOperationWitness {
    ProgramOrder,
    AtomicReadFrom {
        region: MemoryRegion,
        before_operation: AtomicOperation,
        after_operation: AtomicOperation,
    },
    BarrierPhase {
        kind: BarrierKind,
        expected_participants: u32,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationReport {
    pub module_digest: [u8; 32],
    pub obligations_digest: [u8; 32],
    pub report_digest: [u8; 32],
    pub target: TargetProfile,
    pub target_limits: TargetHardLimits,
    pub policy_limits: SynchronizationLimits,
    pub obligations: Vec<VerifierObligation>,
}

impl VerificationReport {
    pub fn verifies_module(
        &self,
        module: &SynchronizationModuleV2,
        limits: &SynchronizationLimits,
    ) -> Result<bool, ValidationError> {
        Ok(self == &module.validate(limits)?)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodeError {
    ResourceLimit {
        resource: Resource,
        observed: u64,
        limit: u64,
    },
    Truncated,
    BadMagic,
    UnsupportedVersion(u16),
    NonZeroReserved,
    LengthMismatch,
    UnknownTag,
    InvalidBoolean,
    NonCanonicalEncoding,
    Model(ValidationError),
}

impl From<ValidationError> for DecodeError {
    fn from(value: ValidationError) -> Self {
        Self::Model(value)
    }
}

impl SynchronizationModuleV2 {
    pub fn validate(
        &self,
        limits: &SynchronizationLimits,
    ) -> Result<VerificationReport, ValidationError> {
        check_limit(
            Resource::LdsAllocations,
            usize_u64(self.lds_allocations.len())?,
            u64::from(limits.max_lds_allocations),
        )?;
        check_limit(
            Resource::Events,
            usize_u64(self.events.len())?,
            u64::from(limits.max_events),
        )?;
        check_limit(
            Resource::Edges,
            usize_u64(self.edges.len())?,
            u64::from(limits.max_edges),
        )?;
        let event_count = usize_u64(self.events.len())?;
        let pair_checks = event_count
            .checked_mul(event_count.saturating_sub(1))
            .and_then(|value| value.checked_div(2))
            .ok_or(ValidationError::ArithmeticOverflow)?;
        check_limit(Resource::PairChecks, pair_checks, limits.max_pair_checks)?;

        let mut obligations = BTreeSet::new();
        let lds_layout = canonical_lds_layout(self, limits)?;
        for (position, allocation) in self.lds_allocations.iter().enumerate() {
            let expected =
                u32::try_from(position).map_err(|_| ValidationError::ArithmeticOverflow)?;
            if allocation.id != LdsAllocationId(expected) {
                return Err(ValidationError::NonCanonicalLdsId {
                    position: expected,
                    actual: allocation.id,
                });
            }
            insert_obligation(
                &mut obligations,
                VerifierObligation::LdsBankMapping {
                    allocation: allocation.id,
                    base_offset: lds_layout[position].base_offset,
                    bank_count: allocation.bank_count,
                    bank_width: allocation.bank_width,
                    element_stride: allocation.element_stride,
                    swizzle: allocation.swizzle,
                },
                limits,
            )?;
        }
        for (position, event) in self.events.iter().enumerate() {
            let expected =
                u32::try_from(position).map_err(|_| ValidationError::ArithmeticOverflow)?;
            if event.id != EventId(expected) {
                return Err(ValidationError::NonCanonicalEventId {
                    position: expected,
                    actual: event.id,
                });
            }
            validate_participation(event, self.target, limits)?;
            validate_event(event, self, &lds_layout, limits)?;
            if let EventKind::Atomic(AtomicAccess {
                coherent_allocation: Some(claim),
                ..
            }) = &event.kind
            {
                insert_obligation(
                    &mut obligations,
                    VerifierObligation::AuthenticateCoherentAllocation {
                        event: event.id,
                        allocation: claim.allocation,
                        authority: claim.authority,
                    },
                    limits,
                )?;
            }
            if matches!(
                event.participation.convergence,
                ConvergenceContract::UniformRequired | ConvergenceContract::ExplicitMask
            ) {
                insert_obligation(
                    &mut obligations,
                    VerifierObligation::UniformParticipation {
                        event: event.id,
                        group: event.participation.group,
                        expected_participants: event.participation.expected_participants,
                        active_mask: event.participation.active_mask,
                    },
                    limits,
                )?;
            }
            if event.participation.group == GroupKind::CooperativeGrid {
                insert_obligation(
                    &mut obligations,
                    VerifierObligation::CooperativeParticipation {
                        event: event.id,
                        expected_participants: event.participation.expected_participants,
                    },
                    limits,
                )?;
            }
            match &event.kind {
                EventKind::Fence(fence) => insert_obligation(
                    &mut obligations,
                    VerifierObligation::FenceSemantics {
                        event: event.id,
                        participation: event.participation,
                        fence: fence.clone(),
                    },
                    limits,
                )?,
                EventKind::Barrier(barrier) => insert_obligation(
                    &mut obligations,
                    VerifierObligation::BarrierSemantics {
                        event: event.id,
                        participation: event.participation,
                        barrier: barrier.clone(),
                    },
                    limits,
                )?,
                EventKind::Collective(collective) => insert_obligation(
                    &mut obligations,
                    VerifierObligation::CollectiveSemantics {
                        event: event.id,
                        participation: event.participation,
                        collective: collective.clone(),
                    },
                    limits,
                )?,
                _ => {}
            }
        }

        let mut prior_edge: Option<&SynchronizationEdge> = None;
        for (index, edge) in self.edges.iter().enumerate() {
            if let Some(prior) = prior_edge {
                if edge < prior {
                    return Err(ValidationError::NonCanonicalEdgeOrder);
                }
                if edge == prior {
                    return Err(ValidationError::DuplicateEdge);
                }
            }
            prior_edge = Some(edge);
            let edge_index =
                u32::try_from(index).map_err(|_| ValidationError::ArithmeticOverflow)?;
            let witness = validate_edge(edge_index, edge, self)?;
            insert_obligation(
                &mut obligations,
                VerifierObligation::HappensBefore {
                    edge: edge_index,
                    before: edge.before,
                    after: edge.after,
                    kind: edge.kind.clone(),
                    scope: edge.scope,
                    domains: edge.domains,
                    before_outcome: edge.before_outcome,
                    after_outcome: edge.after_outcome,
                    read_from: edge.read_from,
                    before_participation: witness.before.participation,
                    after_participation: witness.after.participation,
                    before_kind: witness.before.kind.clone(),
                    after_kind: witness.after.kind.clone(),
                    participant_witness: witness.participant_witness,
                    operation_witness: witness.operation_witness,
                },
                limits,
            )?;
        }

        let reachability = Reachability::new(self)?;
        for left_index in 0..self.events.len() {
            for right_index in (left_index + 1)..self.events.len() {
                let left = &self.events[left_index];
                let right = &self.events[right_index];
                let Some(left_access) = memory_access(left) else {
                    continue;
                };
                let Some(right_access) = memory_access(right) else {
                    continue;
                };
                let relation = access_relation(left_access, right_access)?;
                if relation == AccessRelation::Disjoint {
                    continue;
                }
                if relation == AccessRelation::UnknownGlobalAllocationAlias {
                    let consequence = if left_access.atomic && right_access.atomic {
                        AllocationAliasConsequence::AtomicObjectCompatibility
                    } else if left_access.writes || right_access.writes {
                        AllocationAliasConsequence::NonAtomicConflict
                    } else {
                        AllocationAliasConsequence::ReadOnlyOverlap
                    };
                    insert_obligation(
                        &mut obligations,
                        VerifierObligation::DischargeAllocationAlias {
                            first: left.id,
                            second: right.id,
                            address_space: left_access.address_space,
                            first_region: left_access.region,
                            second_region: right_access.region,
                            consequence,
                        },
                        limits,
                    )?;
                    if left_access.atomic && right_access.atomic {
                        continue;
                    }
                    if !(left_access.writes || right_access.writes) {
                        continue;
                    }
                    let domain = domains_for(left_access.address_space);
                    let ordered = reachability.path_covers(left.id, right.id, domain)
                        || reachability.path_covers(right.id, left.id, domain);
                    insert_obligation(
                        &mut obligations,
                        VerifierObligation::NonAtomicConflict {
                            first: left.id,
                            second: right.id,
                            address_space: left_access.address_space,
                            structurally_ordered: ordered,
                            aliasing: AliasingCondition::VerifierMustProveDisjoint,
                        },
                        limits,
                    )?;
                    continue;
                }
                if left_access.atomic && right_access.atomic {
                    if !atomic_objects_compatible(left_access, right_access) {
                        return Err(ValidationError::IncompatibleAtomicObject {
                            first: left.id,
                            second: right.id,
                        });
                    }
                    let required_scope = required_access_pair_scope(left_access.address_space);
                    if left_access
                        .scope
                        .is_none_or(|scope| scope.rank() < required_scope.rank())
                        || right_access
                            .scope
                            .is_none_or(|scope| scope.rank() < required_scope.rank())
                    {
                        return Err(ValidationError::IncompatibleAtomicScope {
                            first: left.id,
                            second: right.id,
                            required: required_scope,
                        });
                    }
                    insert_obligation(
                        &mut obligations,
                        VerifierObligation::ScopeCompatibility {
                            first: left.id,
                            second: right.id,
                            required_scope,
                        },
                        limits,
                    )?;
                    continue;
                }
                if !(left_access.writes || right_access.writes) {
                    continue;
                }
                let domain = domains_for(left_access.address_space);
                let ordered = reachability.path_covers(left.id, right.id, domain)
                    || reachability.path_covers(right.id, left.id, domain);
                insert_obligation(
                    &mut obligations,
                    VerifierObligation::NonAtomicConflict {
                        first: left.id,
                        second: right.id,
                        address_space: left_access.address_space,
                        structurally_ordered: ordered,
                        aliasing: AliasingCondition::ConfirmedOverlap,
                    },
                    limits,
                )?;
            }
        }

        check_limit(
            Resource::Obligations,
            usize_u64(obligations.len())?,
            u64::from(limits.max_obligations),
        )?;
        let obligations: Vec<_> = obligations.into_iter().collect();
        let canonical_module = encode_validated_synchronization_v2(self, limits)?;
        let module_digest = digest_module(&canonical_module);
        let obligations_digest = digest_obligations(&obligations);
        let target_limits = self.target.hard_limits();
        let report_digest = digest_report(
            module_digest,
            obligations_digest,
            self.target,
            target_limits,
            *limits,
        );
        Ok(VerificationReport {
            module_digest,
            obligations_digest,
            report_digest,
            target: self.target,
            target_limits,
            policy_limits: *limits,
            obligations,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LdsPlacement {
    base_offset: u32,
    end_offset: u32,
}

fn canonical_lds_layout(
    module: &SynchronizationModuleV2,
    limits: &SynchronizationLimits,
) -> Result<Vec<LdsPlacement>, ValidationError> {
    let mut placements = Vec::new();
    placements
        .try_reserve_exact(module.lds_allocations.len())
        .map_err(|_| ValidationError::ArithmeticOverflow)?;
    let mut cursor = 0_u64;
    for allocation in &module.lds_allocations {
        validate_lds(allocation, module.target)?;
        let alignment = u64::from(allocation.alignment);
        let padding = (alignment - cursor % alignment) % alignment;
        let base = cursor
            .checked_add(padding)
            .ok_or(ValidationError::ArithmeticOverflow)?;
        let end = base
            .checked_add(u64::from(allocation.bytes))
            .ok_or(ValidationError::ArithmeticOverflow)?;
        placements.push(LdsPlacement {
            base_offset: u32::try_from(base).map_err(|_| ValidationError::ArithmeticOverflow)?,
            end_offset: u32::try_from(end).map_err(|_| ValidationError::ArithmeticOverflow)?,
        });
        cursor = end;
    }
    let lds_limit = limits
        .max_total_lds_bytes
        .min(module.target.max_lds_bytes());
    check_limit(Resource::TotalLdsBytes, cursor, u64::from(lds_limit))?;
    Ok(placements)
}

fn validate_lds(allocation: &LdsAllocation, target: TargetProfile) -> Result<(), ValidationError> {
    if allocation.bytes == 0
        || allocation.bytes > target.max_lds_bytes()
        || !valid_alignment(allocation.alignment, 16)
        || allocation.element_stride == 0
        || allocation.elements == 0
    {
        return Err(ValidationError::InvalidLdsAllocation(allocation.id));
    }
    let extent = allocation
        .element_stride
        .checked_mul(allocation.elements)
        .ok_or(ValidationError::ArithmeticOverflow)?;
    if extent > allocation.bytes
        || !allocation
            .element_stride
            .is_multiple_of(u32::from(allocation.bank_width))
    {
        return Err(ValidationError::InvalidLdsAllocation(allocation.id));
    }
    if allocation.bank_count != 32 || allocation.bank_width != 4 {
        return Err(ValidationError::UnsupportedLdsBanking(allocation.id));
    }
    if let LdsSwizzle::Xor { shift } = allocation.swizzle
        && !(1..=5).contains(&shift)
    {
        return Err(ValidationError::UnsupportedLdsBanking(allocation.id));
    }
    Ok(())
}

fn validate_participation(
    event: &Event,
    target: TargetProfile,
    limits: &SynchronizationLimits,
) -> Result<(), ValidationError> {
    let participation = event.participation;
    if participation.expected_participants == 0 {
        return Err(ValidationError::InvalidParticipation(event.id));
    }
    match participation.group {
        GroupKind::Invocation => {
            if participation.expected_participants != 1
                || participation.active_mask.is_some()
                || participation.convergence != ConvergenceContract::NotRequired
            {
                return Err(ValidationError::InvalidParticipation(event.id));
            }
        }
        GroupKind::Subgroup => {
            if participation.expected_participants > target.wave_size() {
                return Err(ValidationError::InvalidParticipation(event.id));
            }
            match participation.convergence {
                ConvergenceContract::NotRequired if participation.active_mask.is_none() => {}
                ConvergenceContract::UniformRequired if participation.active_mask.is_none() => {
                    if participation.expected_participants != target.wave_size() {
                        return Err(ValidationError::InvalidParticipation(event.id));
                    }
                }
                ConvergenceContract::ExplicitMask => {
                    let Some(mask) = participation.active_mask else {
                        return Err(ValidationError::InvalidParticipation(event.id));
                    };
                    if mask.count_ones() != participation.expected_participants
                        || (target.wave_size() < 64 && mask >> target.wave_size() != 0)
                    {
                        return Err(ValidationError::InvalidParticipation(event.id));
                    }
                }
                _ => return Err(ValidationError::InvalidParticipation(event.id)),
            }
        }
        GroupKind::Workgroup => {
            let effective_limit = limits
                .max_workgroup_participants
                .min(target.hard_limits().max_workgroup_participants);
            check_limit(
                Resource::WorkgroupParticipants,
                u64::from(participation.expected_participants),
                u64::from(effective_limit),
            )?;
            if participation.active_mask.is_some()
                || participation.convergence == ConvergenceContract::ExplicitMask
            {
                return Err(ValidationError::InvalidParticipation(event.id));
            }
        }
        GroupKind::CooperativeGrid => {
            let effective_limit = limits
                .max_cooperative_participants
                .min(target.hard_limits().max_cooperative_participants);
            check_limit(
                Resource::CooperativeParticipants,
                u64::from(participation.expected_participants),
                u64::from(effective_limit),
            )?;
            if participation.active_mask.is_some()
                || participation.convergence != ConvergenceContract::UniformRequired
            {
                return Err(ValidationError::InvalidParticipation(event.id));
            }
        }
    }
    Ok(())
}

fn validate_event(
    event: &Event,
    module: &SynchronizationModuleV2,
    lds_layout: &[LdsPlacement],
    _limits: &SynchronizationLimits,
) -> Result<(), ValidationError> {
    match &event.kind {
        EventKind::Atomic(atomic) => validate_atomic(event.id, atomic, module, lds_layout),
        EventKind::NonAtomic(access) => validate_non_atomic(event.id, access, module, lds_layout),
        EventKind::Fence(fence) => validate_fence(event.id, fence),
        EventKind::Barrier(barrier) => validate_barrier(event, barrier),
        EventKind::Collective(collective) => validate_collective(event, collective),
        EventKind::Shuffle(shuffle) => validate_shuffle(event, shuffle, module.target),
        EventKind::Ballot(ballot) => validate_ballot(event, ballot, module.target),
    }
}

fn validate_atomic(
    id: EventId,
    atomic: &AtomicAccess,
    module: &SynchronizationModuleV2,
    lds_layout: &[LdsPlacement],
) -> Result<(), ValidationError> {
    if !matches!(
        atomic.address_space,
        AddressSpace::Global | AddressSpace::Lds
    ) {
        return Err(ValidationError::InvalidAddressSpace(id));
    }
    match atomic.address_space {
        AddressSpace::Global if atomic.scope != MemoryScope::System => {
            return Err(ValidationError::InvalidScope(id));
        }
        AddressSpace::Lds if atomic.scope != MemoryScope::Workgroup => {
            return Err(ValidationError::InvalidScope(id));
        }
        _ => {}
    }
    match atomic.coherent_allocation {
        Some(claim)
            if atomic.address_space == AddressSpace::Global
                && atomic.scope == MemoryScope::System
                && claim.allocation == atomic.region.allocation
                && claim.authority != 0 => {}
        None if atomic.address_space != AddressSpace::Global
            || atomic.scope != MemoryScope::System => {}
        _ => return Err(ValidationError::InvalidCoherentAllocationClaim(id)),
    }
    if atomic.value_type == ScalarType::Bool {
        return Err(ValidationError::InvalidAtomicType(id));
    }
    let bits = atomic.value_type.bit_width();
    if bits == 128 {
        return Err(ValidationError::UnsupportedAtomicWidth { event: id, bits });
    }
    if !matches!(bits, 32 | 64) {
        return Err(ValidationError::UnsupportedPlatformOperation(id));
    }
    if atomic.region.bytes != atomic.value_type.storage_bytes()
        || !valid_region(id, atomic.region, atomic.address_space, module, lds_layout)?
    {
        return Err(ValidationError::InvalidMemoryRegion(id));
    }
    if !valid_alignment(atomic.alignment, 16)
        || atomic.alignment < atomic.value_type.storage_bytes()
        || !valid_access_alignment(
            id,
            atomic.region,
            atomic.address_space,
            atomic.alignment,
            module,
            lds_layout,
        )?
    {
        return Err(ValidationError::InvalidAlignment(id));
    }
    let operation = atomic.operation;
    match atomic.dialect {
        AtomicDialect::Rust
            if matches!(
                operation,
                AtomicOperation::AmdInc | AtomicOperation::AmdDec | AtomicOperation::FloatAdd
            ) =>
        {
            return Err(ValidationError::InvalidAtomicOperation(id));
        }
        AtomicDialect::AmdGpu if operation == AtomicOperation::FetchNand => {
            return Err(ValidationError::InvalidAtomicOperation(id));
        }
        _ => {}
    }
    match atomic.value_type {
        ScalarType::Integer { .. } => {
            if operation == AtomicOperation::FloatAdd {
                return Err(ValidationError::InvalidAtomicOperation(id));
            }
            if matches!(operation, AtomicOperation::AmdInc | AtomicOperation::AmdDec)
                && (!atomic.value_type.is_unsigned_integer() || bits != 32)
            {
                return Err(ValidationError::InvalidAtomicOperation(id));
            }
        }
        ScalarType::Pointer64 => {
            if !matches!(
                operation,
                AtomicOperation::Load
                    | AtomicOperation::Store
                    | AtomicOperation::Exchange
                    | AtomicOperation::CompareExchangeStrong
                    | AtomicOperation::CompareExchangeWeak
            ) {
                return Err(ValidationError::InvalidAtomicOperation(id));
            }
        }
        ScalarType::Float32 => {
            if atomic.dialect != AtomicDialect::AmdGpu || operation != AtomicOperation::FloatAdd {
                return Err(ValidationError::InvalidAtomicOperation(id));
            }
        }
        ScalarType::Float64 => return Err(ValidationError::UnsupportedPlatformOperation(id)),
        ScalarType::Bool => return Err(ValidationError::InvalidAtomicType(id)),
    }
    if operation.is_bitwise() && !atomic.value_type.is_integer() {
        return Err(ValidationError::InvalidAtomicOperation(id));
    }
    if operation.is_min_max() && !atomic.value_type.is_integer() {
        return Err(ValidationError::InvalidAtomicOperation(id));
    }
    if operation.is_load()
        && !matches!(
            atomic.success_ordering,
            MemoryOrdering::Relaxed
                | MemoryOrdering::Acquire
                | MemoryOrdering::SequentiallyConsistent
        )
    {
        return Err(ValidationError::InvalidAtomicOrdering(id));
    }
    if operation.is_store()
        && !matches!(
            atomic.success_ordering,
            MemoryOrdering::Relaxed
                | MemoryOrdering::Release
                | MemoryOrdering::SequentiallyConsistent
        )
    {
        return Err(ValidationError::InvalidAtomicOrdering(id));
    }
    if operation.is_compare_exchange() {
        let Some(failure) = atomic.failure_ordering else {
            return Err(ValidationError::InvalidCompareExchangeOrdering(id));
        };
        if !valid_failure_ordering(atomic.success_ordering, failure) {
            return Err(ValidationError::InvalidCompareExchangeOrdering(id));
        }
    } else if atomic.failure_ordering.is_some() {
        return Err(ValidationError::InvalidAtomicOrdering(id));
    }
    Ok(())
}

fn validate_non_atomic(
    id: EventId,
    access: &NonAtomicAccess,
    module: &SynchronizationModuleV2,
    lds_layout: &[LdsPlacement],
) -> Result<(), ValidationError> {
    if !matches!(
        access.address_space,
        AddressSpace::Global | AddressSpace::Lds
    ) {
        return Err(ValidationError::InvalidAddressSpace(id));
    }
    if access.region.bytes != access.value_type.storage_bytes()
        || !valid_region(id, access.region, access.address_space, module, lds_layout)?
    {
        return Err(ValidationError::InvalidMemoryRegion(id));
    }
    if !valid_alignment(access.alignment, 16)
        || access.alignment < access.value_type.storage_bytes()
        || !valid_access_alignment(
            id,
            access.region,
            access.address_space,
            access.alignment,
            module,
            lds_layout,
        )?
    {
        return Err(ValidationError::InvalidAlignment(id));
    }
    Ok(())
}

fn valid_region(
    id: EventId,
    region: MemoryRegion,
    address_space: AddressSpace,
    module: &SynchronizationModuleV2,
    lds_layout: &[LdsPlacement],
) -> Result<bool, ValidationError> {
    if region.bytes == 0 {
        return Ok(false);
    }
    let end = region
        .offset
        .checked_add(region.bytes)
        .ok_or(ValidationError::ArithmeticOverflow)?;
    if address_space != AddressSpace::Lds {
        return Ok(true);
    }
    let allocation_id = LdsAllocationId(region.allocation);
    let Some(allocation) = module.lds_allocations.get(region.allocation as usize) else {
        return Err(ValidationError::UnknownLdsAllocation {
            event: id,
            allocation: allocation_id,
        });
    };
    if allocation.id != allocation_id {
        return Err(ValidationError::UnknownLdsAllocation {
            event: id,
            allocation: allocation_id,
        });
    }
    let placement = lds_layout.get(region.allocation as usize).ok_or(
        ValidationError::UnknownLdsAllocation {
            event: id,
            allocation: allocation_id,
        },
    )?;
    Ok(end <= allocation.bytes
        && placement
            .base_offset
            .checked_add(end)
            .is_some_and(|effective_end| effective_end <= placement.end_offset))
}

fn valid_access_alignment(
    id: EventId,
    region: MemoryRegion,
    address_space: AddressSpace,
    access_alignment: u32,
    module: &SynchronizationModuleV2,
    lds_layout: &[LdsPlacement],
) -> Result<bool, ValidationError> {
    if address_space != AddressSpace::Lds {
        return Ok(region.offset.is_multiple_of(access_alignment));
    }
    let allocation_id = LdsAllocationId(region.allocation);
    let allocation = module
        .lds_allocations
        .get(region.allocation as usize)
        .filter(|allocation| allocation.id == allocation_id)
        .ok_or(ValidationError::UnknownLdsAllocation {
            event: id,
            allocation: allocation_id,
        })?;
    let placement = lds_layout.get(region.allocation as usize).ok_or(
        ValidationError::UnknownLdsAllocation {
            event: id,
            allocation: allocation_id,
        },
    )?;
    let effective = placement
        .base_offset
        .checked_add(region.offset)
        .ok_or(ValidationError::ArithmeticOverflow)?;
    Ok(allocation.alignment >= access_alignment && effective.is_multiple_of(access_alignment))
}

fn validate_fence(id: EventId, fence: &Fence) -> Result<(), ValidationError> {
    if fence.ordering == MemoryOrdering::Relaxed
        || fence.domains == MemoryDomains::NONE
        || (fence.domains.contains(AddressSpace::Lds)
            && fence.scope.rank() > MemoryScope::Workgroup.rank())
    {
        return Err(ValidationError::InvalidFence(id));
    }
    Ok(())
}

fn validate_barrier(event: &Event, barrier: &Barrier) -> Result<(), ValidationError> {
    if barrier.ordering == MemoryOrdering::Relaxed || barrier.domains == MemoryDomains::NONE {
        return Err(ValidationError::InvalidBarrier(event.id));
    }
    match barrier.kind {
        BarrierKind::Subgroup => {
            if event.participation.group != GroupKind::Subgroup
                || event.participation.convergence == ConvergenceContract::NotRequired
                || barrier.scope != MemoryScope::Wavefront
            {
                return Err(ValidationError::InvalidBarrier(event.id));
            }
        }
        BarrierKind::Workgroup => {
            if event.participation.group != GroupKind::Workgroup
                || event.participation.convergence != ConvergenceContract::UniformRequired
                || barrier.scope != MemoryScope::Workgroup
            {
                return Err(ValidationError::InvalidBarrier(event.id));
            }
        }
        BarrierKind::CooperativeGroup => {
            if event.participation.group != GroupKind::CooperativeGrid {
                return Err(ValidationError::InvalidBarrier(event.id));
            }
            return Err(ValidationError::UnsupportedCooperativeGroup(event.id));
        }
    }
    Ok(())
}

fn validate_collective(event: &Event, collective: &Collective) -> Result<(), ValidationError> {
    if event.participation.convergence == ConvergenceContract::NotRequired
        || !matches!(
            event.participation.group,
            GroupKind::Subgroup | GroupKind::Workgroup | GroupKind::CooperativeGrid
        )
    {
        return Err(ValidationError::InvalidCollective(event.id));
    }
    if event.participation.group == GroupKind::CooperativeGrid {
        return Err(ValidationError::UnsupportedCooperativeGroup(event.id));
    }
    match collective.kind {
        CollectiveKind::Any | CollectiveKind::All => {
            if collective.value_type != ScalarType::Bool {
                return Err(ValidationError::InvalidCollective(event.id));
            }
        }
        CollectiveKind::Broadcast => {
            if collective.value_type.bit_width() > 64 {
                return Err(ValidationError::InvalidCollective(event.id));
            }
        }
        _ => {
            if !matches!(
                collective.value_type,
                ScalarType::Integer {
                    width: IntegerWidth::W32 | IntegerWidth::W64,
                    ..
                } | ScalarType::Float32
            ) {
                return Err(ValidationError::InvalidCollective(event.id));
            }
        }
    }
    Ok(())
}

fn validate_shuffle(
    event: &Event,
    shuffle: &Shuffle,
    target: TargetProfile,
) -> Result<(), ValidationError> {
    if event.participation.group != GroupKind::Subgroup
        || event.participation.convergence == ConvergenceContract::NotRequired
        || shuffle.value_type == ScalarType::Bool
        || !matches!(shuffle.value_type.bit_width(), 32 | 64)
        || shuffle.tile_width == 0
        || !shuffle.tile_width.is_power_of_two()
        || shuffle.tile_width > target.wave_size()
        || shuffle.tile_width > event.participation.expected_participants
    {
        return Err(ValidationError::InvalidShuffle(event.id));
    }
    Ok(())
}

fn validate_ballot(
    event: &Event,
    ballot: &Ballot,
    target: TargetProfile,
) -> Result<(), ValidationError> {
    if event.participation.group != GroupKind::Subgroup
        || event.participation.convergence == ConvergenceContract::NotRequired
        || ballot.wave_size != target.wave_size()
        || ballot.wave_size != event.participation.expected_participants
        || ballot.result_width.bits() != ballot.wave_size as u16
    {
        return Err(ValidationError::InvalidBallot(event.id));
    }
    Ok(())
}

struct ValidatedEdgeWitness<'a> {
    before: &'a Event,
    after: &'a Event,
    participant_witness: ParticipantWitness,
    operation_witness: SynchronizationOperationWitness,
}

fn validate_edge<'a>(
    edge_index: u32,
    edge: &SynchronizationEdge,
    module: &'a SynchronizationModuleV2,
) -> Result<ValidatedEdgeWitness<'a>, ValidationError> {
    if edge.domains == MemoryDomains::NONE {
        return Err(ValidationError::IncompatibleEdgeDomains(edge_index));
    }
    let Some(before) = module.events.get(edge.before.0 as usize) else {
        return Err(ValidationError::UnknownEdgeEndpoint {
            edge: edge_index,
            endpoint: edge.before,
        });
    };
    let Some(after) = module.events.get(edge.after.0 as usize) else {
        return Err(ValidationError::UnknownEdgeEndpoint {
            edge: edge_index,
            endpoint: edge.after,
        });
    };
    if before.id != edge.before || after.id != edge.after {
        return Err(ValidationError::UnknownEdgeEndpoint {
            edge: edge_index,
            endpoint: if before.id != edge.before {
                edge.before
            } else {
                edge.after
            },
        });
    }
    if edge.before >= edge.after {
        return Err(ValidationError::BackwardOrSelfEdge(edge_index));
    }
    match edge.kind {
        SynchronizationEdgeKind::ProgramOrder => {
            if edge.before_outcome != EventOutcome::Unconditional
                || edge.after_outcome != EventOutcome::Unconditional
                || edge.read_from != ReadFromCondition::NotApplicable
            {
                return Err(ValidationError::InvalidEdgeEndpointKind(edge_index));
            }
            if before.participation != after.participation
                || edge.scope
                    != required_pair_scope(before.participation.group, after.participation.group)
            {
                return Err(ValidationError::IncompatibleEdgeScope(edge_index));
            }
            for endpoint_domains in [event_domains(before), event_domains(after)]
                .into_iter()
                .flatten()
            {
                if edge.domains.bits() & !endpoint_domains.bits() != 0 {
                    return Err(ValidationError::IncompatibleEdgeDomains(edge_index));
                }
            }
            Ok(ValidatedEdgeWitness {
                before,
                after,
                participant_witness: ParticipantWitness::SameParticipantMustProve,
                operation_witness: SynchronizationOperationWitness::ProgramOrder,
            })
        }
        SynchronizationEdgeKind::SynchronizesWith => {
            if !event_has_release(before, edge.before_outcome)
                || !event_has_acquire(after, edge.after_outcome)
            {
                return Err(ValidationError::InvalidEdgeEndpointKind(edge_index));
            }
            let (participant_witness, operation_witness) = match (&before.kind, &after.kind) {
                (EventKind::Atomic(before_atomic), EventKind::Atomic(after_atomic)) => {
                    let before_access = memory_access(before)
                        .ok_or(ValidationError::InvalidEdgeEndpointKind(edge_index))?;
                    let after_access = memory_access(after)
                        .ok_or(ValidationError::InvalidEdgeEndpointKind(edge_index))?;
                    if edge.read_from != ReadFromCondition::VerifierMustProve
                        || !atomic_objects_compatible(before_access, after_access)
                    {
                        return Err(ValidationError::InvalidEdgeEndpointKind(edge_index));
                    }
                    (
                        ParticipantWitness::SynchronizingParticipantsMustProve,
                        SynchronizationOperationWitness::AtomicReadFrom {
                            region: before_atomic.region,
                            before_operation: before_atomic.operation,
                            after_operation: after_atomic.operation,
                        },
                    )
                }
                (EventKind::Barrier(before_barrier), EventKind::Barrier(after_barrier)) => {
                    if edge.read_from != ReadFromCondition::NotApplicable
                        || before_barrier.kind != after_barrier.kind
                        || before.participation != after.participation
                    {
                        return Err(ValidationError::InvalidEdgeEndpointKind(edge_index));
                    }
                    (
                        ParticipantWitness::SameBarrierCohortMustProve,
                        SynchronizationOperationWitness::BarrierPhase {
                            kind: before_barrier.kind,
                            expected_participants: before.participation.expected_participants,
                        },
                    )
                }
                _ => return Err(ValidationError::InvalidEdgeEndpointKind(edge_index)),
            };
            let before_scope =
                event_scope(before).ok_or(ValidationError::InvalidEdgeEndpointKind(edge_index))?;
            let after_scope =
                event_scope(after).ok_or(ValidationError::InvalidEdgeEndpointKind(edge_index))?;
            let participant_scope = required_edge_scope(edge.domains);
            if edge.scope.rank() < participant_scope.rank()
                || edge.scope.rank() > before_scope.rank()
                || edge.scope.rank() > after_scope.rank()
            {
                return Err(ValidationError::IncompatibleEdgeScope(edge_index));
            }
            let before_domains = event_domains(before)
                .ok_or(ValidationError::InvalidEdgeEndpointKind(edge_index))?;
            let after_domains =
                event_domains(after).ok_or(ValidationError::InvalidEdgeEndpointKind(edge_index))?;
            if !edge.domains.intersects(before_domains)
                || !edge.domains.intersects(after_domains)
                || edge.domains.bits() & !(before_domains.bits() & after_domains.bits()) != 0
            {
                return Err(ValidationError::IncompatibleEdgeDomains(edge_index));
            }
            Ok(ValidatedEdgeWitness {
                before,
                after,
                participant_witness,
                operation_witness,
            })
        }
    }
}

fn event_has_release(event: &Event, outcome: EventOutcome) -> bool {
    event_ordering_for_outcome(event, outcome).is_some_and(MemoryOrdering::has_release)
}

fn event_has_acquire(event: &Event, outcome: EventOutcome) -> bool {
    event_ordering_for_outcome(event, outcome).is_some_and(MemoryOrdering::has_acquire)
}

fn event_ordering_for_outcome(event: &Event, outcome: EventOutcome) -> Option<MemoryOrdering> {
    match &event.kind {
        EventKind::Atomic(atomic) if atomic.operation.is_compare_exchange() => match outcome {
            EventOutcome::CompareExchangeSuccess => Some(atomic.success_ordering),
            EventOutcome::CompareExchangeFailure => atomic.failure_ordering,
            EventOutcome::Unconditional => None,
        },
        EventKind::Atomic(atomic) if outcome == EventOutcome::Unconditional => {
            Some(atomic.success_ordering)
        }
        EventKind::Fence(fence) if outcome == EventOutcome::Unconditional => Some(fence.ordering),
        EventKind::Barrier(barrier) if outcome == EventOutcome::Unconditional => {
            Some(barrier.ordering)
        }
        _ => None,
    }
}

fn event_scope(event: &Event) -> Option<MemoryScope> {
    match &event.kind {
        EventKind::Atomic(atomic) => Some(atomic.scope),
        EventKind::Fence(fence) => Some(fence.scope),
        EventKind::Barrier(barrier) => Some(barrier.scope),
        _ => None,
    }
}

fn event_domains(event: &Event) -> Option<MemoryDomains> {
    match &event.kind {
        EventKind::Atomic(atomic) => Some(domains_for(atomic.address_space)),
        EventKind::NonAtomic(access) => Some(domains_for(access.address_space)),
        EventKind::Fence(fence) => Some(fence.domains),
        EventKind::Barrier(barrier) => Some(barrier.domains),
        _ => None,
    }
}

fn valid_failure_ordering(success: MemoryOrdering, failure: MemoryOrdering) -> bool {
    match success {
        MemoryOrdering::Relaxed => failure == MemoryOrdering::Relaxed,
        MemoryOrdering::Acquire => {
            matches!(failure, MemoryOrdering::Relaxed | MemoryOrdering::Acquire)
        }
        MemoryOrdering::Release => failure == MemoryOrdering::Relaxed,
        MemoryOrdering::AcquireRelease => {
            matches!(failure, MemoryOrdering::Relaxed | MemoryOrdering::Acquire)
        }
        MemoryOrdering::SequentiallyConsistent => matches!(
            failure,
            MemoryOrdering::Relaxed
                | MemoryOrdering::Acquire
                | MemoryOrdering::SequentiallyConsistent
        ),
    }
}

fn valid_alignment(alignment: u32, maximum: u32) -> bool {
    alignment != 0 && alignment.is_power_of_two() && alignment <= maximum
}

fn domains_for(address_space: AddressSpace) -> MemoryDomains {
    match address_space {
        AddressSpace::Global => MemoryDomains::GLOBAL,
        AddressSpace::Lds => MemoryDomains::LDS,
        _ => MemoryDomains::NONE,
    }
}

#[derive(Clone, Copy)]
struct AccessView {
    region: MemoryRegion,
    address_space: AddressSpace,
    value_type: ScalarType,
    alignment: u32,
    scope: Option<MemoryScope>,
    writes: bool,
    atomic: bool,
}

fn memory_access(event: &Event) -> Option<AccessView> {
    match &event.kind {
        EventKind::Atomic(access) => Some(AccessView {
            region: access.region,
            address_space: access.address_space,
            value_type: access.value_type,
            alignment: access.alignment,
            scope: Some(access.scope),
            writes: !access.operation.is_load(),
            atomic: true,
        }),
        EventKind::NonAtomic(access) => Some(AccessView {
            region: access.region,
            address_space: access.address_space,
            value_type: access.value_type,
            alignment: access.alignment,
            scope: None,
            writes: access.kind.writes(),
            atomic: false,
        }),
        _ => None,
    }
}

fn atomic_objects_compatible(left: AccessView, right: AccessView) -> bool {
    left.address_space == right.address_space
        && left.region == right.region
        && left.value_type == right.value_type
        && left.alignment == right.alignment
}

fn required_access_pair_scope(address_space: AddressSpace) -> MemoryScope {
    match address_space {
        AddressSpace::Lds => MemoryScope::Workgroup,
        _ => MemoryScope::System,
    }
}

fn required_edge_scope(domains: MemoryDomains) -> MemoryScope {
    if domains.contains(AddressSpace::Global) {
        MemoryScope::System
    } else {
        MemoryScope::Workgroup
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AccessRelation {
    Disjoint,
    ConfirmedOverlap,
    UnknownGlobalAllocationAlias,
}

fn access_relation(left: AccessView, right: AccessView) -> Result<AccessRelation, ValidationError> {
    if left.address_space != right.address_space {
        return Ok(AccessRelation::Disjoint);
    }
    if left.region.allocation != right.region.allocation {
        return Ok(if left.address_space == AddressSpace::Global {
            AccessRelation::UnknownGlobalAllocationAlias
        } else {
            AccessRelation::Disjoint
        });
    }
    let left_end = left
        .region
        .offset
        .checked_add(left.region.bytes)
        .ok_or(ValidationError::ArithmeticOverflow)?;
    let right_end = right
        .region
        .offset
        .checked_add(right.region.bytes)
        .ok_or(ValidationError::ArithmeticOverflow)?;
    Ok(
        if left.region.offset < right_end && right.region.offset < left_end {
            AccessRelation::ConfirmedOverlap
        } else {
            AccessRelation::Disjoint
        },
    )
}

fn required_pair_scope(left: GroupKind, right: GroupKind) -> MemoryScope {
    match left.max(right) {
        GroupKind::Invocation | GroupKind::Subgroup => MemoryScope::Wavefront,
        GroupKind::Workgroup => MemoryScope::Workgroup,
        GroupKind::CooperativeGrid => MemoryScope::Agent,
    }
}

struct Reachability {
    matrix: Vec<u8>,
    event_count: usize,
}

impl Reachability {
    fn new(module: &SynchronizationModuleV2) -> Result<Self, ValidationError> {
        let event_count = module.events.len();
        let cells = event_count
            .checked_mul(event_count)
            .ok_or(ValidationError::ArithmeticOverflow)?;
        let mut matrix = Vec::new();
        matrix
            .try_reserve_exact(cells)
            .map_err(|_| ValidationError::ArithmeticOverflow)?;
        matrix.resize(cells, 0);
        let mut outgoing = vec![Vec::<(usize, u8)>::new(); event_count];
        for edge in &module.edges {
            outgoing[edge.before.0 as usize].push((edge.after.0 as usize, edge.domains.bits()));
        }
        for source in (0..event_count).rev() {
            for &(next, edge_domains) in &outgoing[source] {
                matrix[source * event_count + next] |= edge_domains;
                for target in (next + 1)..event_count {
                    let propagated = edge_domains & matrix[next * event_count + target];
                    matrix[source * event_count + target] |= propagated;
                }
            }
        }
        Ok(Self {
            matrix,
            event_count,
        })
    }

    fn path_covers(&self, from: EventId, to: EventId, required: MemoryDomains) -> bool {
        if from >= to || to.0 as usize >= self.event_count {
            return false;
        }
        let domains = self.matrix[from.0 as usize * self.event_count + to.0 as usize];
        domains & required.bits() == required.bits()
    }
}

pub fn encode_synchronization_v2(
    module: &SynchronizationModuleV2,
    limits: &SynchronizationLimits,
) -> Result<Vec<u8>, ValidationError> {
    module.validate(limits)?;
    encode_validated_synchronization_v2(module, limits)
}

fn encode_validated_synchronization_v2(
    module: &SynchronizationModuleV2,
    limits: &SynchronizationLimits,
) -> Result<Vec<u8>, ValidationError> {
    let mut writer = Writer::new(limits.max_encoded_bytes);
    writer.bytes(&SYNCHRONIZATION_V2_MAGIC)?;
    writer.u16(SYNCHRONIZATION_V2_VERSION)?;
    writer.u16(0)?;
    let payload_offset = writer.len();
    writer.u32(0)?;
    writer.u8(module.target as u8)?;
    writer.bytes(&[0; 3])?;
    writer.u32(u32_len(module.lds_allocations.len())?)?;
    writer.u32(u32_len(module.events.len())?)?;
    writer.u32(u32_len(module.edges.len())?)?;
    for allocation in &module.lds_allocations {
        encode_lds(&mut writer, allocation)?;
    }
    for event in &module.events {
        encode_event(&mut writer, event)?;
    }
    for edge in &module.edges {
        writer.u32(edge.before.0)?;
        writer.u32(edge.after.0)?;
        writer.u8(match edge.kind {
            SynchronizationEdgeKind::ProgramOrder => 1,
            SynchronizationEdgeKind::SynchronizesWith => 2,
        })?;
        writer.u8(edge.scope as u8)?;
        writer.u8(edge.domains.bits())?;
        writer.u8(edge.before_outcome as u8)?;
        writer.u8(edge.after_outcome as u8)?;
        writer.u8(edge.read_from as u8)?;
        writer.bytes(&[0; 2])?;
        writer.u64(0)?;
    }
    let payload_len = writer
        .len()
        .checked_sub(HEADER_BYTES as usize)
        .ok_or(ValidationError::EncodingSizeOverflow)?;
    let payload_len =
        u32::try_from(payload_len).map_err(|_| ValidationError::EncodingSizeOverflow)?;
    writer.patch_u32(payload_offset, payload_len);
    Ok(writer.finish())
}

pub fn decode_synchronization_v2(
    bytes: &[u8],
    limits: &SynchronizationLimits,
) -> Result<SynchronizationModuleV2, DecodeError> {
    check_decode_limit(
        Resource::EncodedBytes,
        usize_u64_decode(bytes.len())?,
        limits.max_encoded_bytes,
    )?;
    let mut reader = Reader::new(bytes);
    if reader.array::<8>()? != SYNCHRONIZATION_V2_MAGIC {
        return Err(DecodeError::BadMagic);
    }
    let version = reader.u16()?;
    if version != SYNCHRONIZATION_V2_VERSION {
        return Err(DecodeError::UnsupportedVersion(version));
    }
    if reader.u16()? != 0 {
        return Err(DecodeError::NonZeroReserved);
    }
    let payload_len = reader.u32()? as usize;
    if payload_len != bytes.len().saturating_sub(HEADER_BYTES as usize) {
        return Err(DecodeError::LengthMismatch);
    }
    let target = decode_target(reader.u8()?)?;
    if reader.array::<3>()? != [0; 3] {
        return Err(DecodeError::NonZeroReserved);
    }
    let lds_count = reader.u32()?;
    let event_count = reader.u32()?;
    let edge_count = reader.u32()?;
    check_decode_limit(
        Resource::LdsAllocations,
        u64::from(lds_count),
        u64::from(limits.max_lds_allocations),
    )?;
    check_decode_limit(
        Resource::Events,
        u64::from(event_count),
        u64::from(limits.max_events),
    )?;
    check_decode_limit(
        Resource::Edges,
        u64::from(edge_count),
        u64::from(limits.max_edges),
    )?;
    let minimum = u64::from(lds_count)
        .checked_mul(LDS_RECORD_BYTES)
        .and_then(|value| {
            value.checked_add(u64::from(event_count).checked_mul(EVENT_PREFIX_BYTES)?)
        })
        .and_then(|value| value.checked_add(u64::from(edge_count).checked_mul(EDGE_RECORD_BYTES)?))
        .ok_or(DecodeError::LengthMismatch)?;
    if minimum > reader.remaining() as u64 {
        return Err(DecodeError::Truncated);
    }
    let mut lds_allocations = Vec::new();
    lds_allocations
        .try_reserve_exact(lds_count as usize)
        .map_err(|_| DecodeError::ResourceLimit {
            resource: Resource::LdsAllocations,
            observed: u64::from(lds_count),
            limit: u64::from(limits.max_lds_allocations),
        })?;
    for _ in 0..lds_count {
        lds_allocations.push(decode_lds(&mut reader)?);
    }
    let mut events = Vec::new();
    events
        .try_reserve_exact(event_count as usize)
        .map_err(|_| DecodeError::ResourceLimit {
            resource: Resource::Events,
            observed: u64::from(event_count),
            limit: u64::from(limits.max_events),
        })?;
    for _ in 0..event_count {
        events.push(decode_event(&mut reader)?);
    }
    let mut edges = Vec::new();
    edges
        .try_reserve_exact(edge_count as usize)
        .map_err(|_| DecodeError::ResourceLimit {
            resource: Resource::Edges,
            observed: u64::from(edge_count),
            limit: u64::from(limits.max_edges),
        })?;
    for _ in 0..edge_count {
        let before = EventId(reader.u32()?);
        let after = EventId(reader.u32()?);
        let kind = match reader.u8()? {
            1 => SynchronizationEdgeKind::ProgramOrder,
            2 => SynchronizationEdgeKind::SynchronizesWith,
            _ => return Err(DecodeError::UnknownTag),
        };
        let scope = decode_scope(reader.u8()?)?;
        let domains = MemoryDomains::from_bits(reader.u8()?).ok_or(DecodeError::UnknownTag)?;
        let before_outcome = decode_event_outcome(reader.u8()?)?;
        let after_outcome = decode_event_outcome(reader.u8()?)?;
        let read_from = decode_read_from(reader.u8()?)?;
        if reader.array::<2>()? != [0; 2] || reader.u64()? != 0 {
            return Err(DecodeError::NonZeroReserved);
        }
        edges.push(SynchronizationEdge {
            before,
            after,
            kind,
            scope,
            domains,
            before_outcome,
            after_outcome,
            read_from,
        });
    }
    if reader.remaining() != 0 {
        return Err(DecodeError::LengthMismatch);
    }
    let module = SynchronizationModuleV2 {
        target,
        lds_allocations,
        events,
        edges,
    };
    module.validate(limits)?;
    let canonical = encode_synchronization_v2(&module, limits)?;
    if canonical != bytes {
        return Err(DecodeError::NonCanonicalEncoding);
    }
    Ok(module)
}

fn encode_lds(writer: &mut Writer, allocation: &LdsAllocation) -> Result<(), ValidationError> {
    writer.u32(allocation.id.0)?;
    writer.u8(allocation.kind as u8)?;
    match allocation.swizzle {
        LdsSwizzle::Linear => {
            writer.u8(1)?;
            writer.u8(0)?;
        }
        LdsSwizzle::Xor { shift } => {
            writer.u8(2)?;
            writer.u8(shift)?;
        }
    }
    writer.u8(0)?;
    writer.u32(allocation.bytes)?;
    writer.u32(allocation.alignment)?;
    writer.u16(allocation.bank_count)?;
    writer.u16(allocation.bank_width)?;
    writer.u32(allocation.element_stride)?;
    writer.u32(allocation.elements)?;
    writer.u32(0)?;
    Ok(())
}

fn decode_lds(reader: &mut Reader<'_>) -> Result<LdsAllocation, DecodeError> {
    let id = LdsAllocationId(reader.u32()?);
    let kind = match reader.u8()? {
        1 => LdsAllocationKind::Static,
        2 => LdsAllocationKind::DynamicReservation,
        _ => return Err(DecodeError::UnknownTag),
    };
    let swizzle_tag = reader.u8()?;
    let swizzle_arg = reader.u8()?;
    let swizzle = match (swizzle_tag, swizzle_arg) {
        (1, 0) => LdsSwizzle::Linear,
        (2, shift) => LdsSwizzle::Xor { shift },
        _ => return Err(DecodeError::UnknownTag),
    };
    if reader.u8()? != 0 {
        return Err(DecodeError::NonZeroReserved);
    }
    let allocation = LdsAllocation {
        id,
        kind,
        bytes: reader.u32()?,
        alignment: reader.u32()?,
        bank_count: reader.u16()?,
        bank_width: reader.u16()?,
        element_stride: reader.u32()?,
        elements: reader.u32()?,
        swizzle,
    };
    if reader.u32()? != 0 {
        return Err(DecodeError::NonZeroReserved);
    }
    Ok(allocation)
}

fn encode_event(writer: &mut Writer, event: &Event) -> Result<(), ValidationError> {
    writer.u32(event.id.0)?;
    writer.u8(event.participation.group as u8)?;
    writer.u8(event.participation.convergence as u8)?;
    writer.u8(event.participation.active_mask.is_some() as u8)?;
    writer.u8(0)?;
    writer.u32(event.participation.expected_participants)?;
    writer.u64(event.participation.active_mask.unwrap_or(0))?;
    let kind_length_offset = writer.len();
    writer.u32(0)?;
    let kind_start = writer.len();
    encode_event_kind(writer, &event.kind)?;
    let kind_length = writer
        .len()
        .checked_sub(kind_start)
        .ok_or(ValidationError::EncodingSizeOverflow)?;
    writer.patch_u32(
        kind_length_offset,
        u32::try_from(kind_length).map_err(|_| ValidationError::EncodingSizeOverflow)?,
    );
    writer.u32(0)?;
    Ok(())
}

fn decode_event(reader: &mut Reader<'_>) -> Result<Event, DecodeError> {
    let id = EventId(reader.u32()?);
    let group = decode_group(reader.u8()?)?;
    let convergence = decode_convergence(reader.u8()?)?;
    let has_mask = decode_bool(reader.u8()?)?;
    if reader.u8()? != 0 {
        return Err(DecodeError::NonZeroReserved);
    }
    let expected_participants = reader.u32()?;
    let mask = reader.u64()?;
    let active_mask = match (has_mask, mask) {
        (false, 0) => None,
        (true, value) => Some(value),
        _ => return Err(DecodeError::NonCanonicalEncoding),
    };
    let kind_length = reader.u32()? as usize;
    let mut kind_reader = reader.take_reader(kind_length)?;
    let kind = decode_event_kind(&mut kind_reader)?;
    if kind_reader.remaining() != 0 {
        return Err(DecodeError::LengthMismatch);
    }
    if reader.u32()? != 0 {
        return Err(DecodeError::NonZeroReserved);
    }
    Ok(Event {
        id,
        participation: ParticipationContract {
            group,
            convergence,
            expected_participants,
            active_mask,
        },
        kind,
    })
}

fn encode_event_kind(writer: &mut Writer, kind: &EventKind) -> Result<(), ValidationError> {
    match kind {
        EventKind::Atomic(atomic) => {
            writer.u8(1)?;
            encode_region(writer, atomic.region)?;
            writer.u8(atomic.dialect as u8)?;
            writer.u8(atomic.operation as u8)?;
            encode_scalar(writer, atomic.value_type)?;
            writer.u8(atomic.address_space as u8)?;
            writer.u32(atomic.alignment)?;
            writer.u8(atomic.scope as u8)?;
            writer.u8(atomic.success_ordering as u8)?;
            writer.u8(atomic.failure_ordering.map_or(0, |ordering| ordering as u8))?;
            match atomic.coherent_allocation {
                None => {
                    writer.u8(0)?;
                    writer.u32(0)?;
                    writer.u64(0)?;
                }
                Some(claim) => {
                    writer.u8(1)?;
                    writer.u32(claim.allocation)?;
                    writer.u64(claim.authority)?;
                }
            }
        }
        EventKind::NonAtomic(access) => {
            writer.u8(2)?;
            encode_region(writer, access.region)?;
            writer.u8(access.kind as u8)?;
            encode_scalar(writer, access.value_type)?;
            writer.u8(access.address_space as u8)?;
            writer.u32(access.alignment)?;
            writer.u32(0)?;
        }
        EventKind::Fence(fence) => {
            writer.u8(3)?;
            writer.u8(fence.scope as u8)?;
            writer.u8(fence.ordering as u8)?;
            writer.u8(fence.domains.bits())?;
            writer.u32(0)?;
        }
        EventKind::Barrier(barrier) => {
            writer.u8(4)?;
            writer.u8(barrier.kind as u8)?;
            writer.u8(barrier.scope as u8)?;
            writer.u8(barrier.ordering as u8)?;
            writer.u8(barrier.domains.bits())?;
            writer.bytes(&[0; 3])?;
        }
        EventKind::Collective(collective) => {
            writer.u8(5)?;
            writer.u8(collective.kind as u8)?;
            encode_scalar(writer, collective.value_type)?;
            writer.u32(0)?;
        }
        EventKind::Shuffle(shuffle) => {
            writer.u8(6)?;
            writer.u8(shuffle.kind as u8)?;
            encode_scalar(writer, shuffle.value_type)?;
            writer.u32(shuffle.tile_width)?;
            writer.u32(0)?;
        }
        EventKind::Ballot(ballot) => {
            writer.u8(7)?;
            writer.u32(ballot.wave_size)?;
            writer.u8(ballot.result_width as u8)?;
            writer.bytes(&[0; 3])?;
        }
    }
    Ok(())
}

fn decode_event_kind(reader: &mut Reader<'_>) -> Result<EventKind, DecodeError> {
    match reader.u8()? {
        1 => {
            let region = decode_region(reader)?;
            let dialect = match reader.u8()? {
                1 => AtomicDialect::Rust,
                2 => AtomicDialect::AmdGpu,
                _ => return Err(DecodeError::UnknownTag),
            };
            let operation = decode_atomic_operation(reader.u8()?)?;
            let value_type = decode_scalar(reader)?;
            let address_space = decode_address_space(reader.u8()?)?;
            let alignment = reader.u32()?;
            let scope = decode_scope(reader.u8()?)?;
            let success_ordering = decode_ordering(reader.u8()?)?;
            let failure_ordering = match reader.u8()? {
                0 => None,
                tag => Some(decode_ordering(tag)?),
            };
            let coherent_allocation = match reader.u8()? {
                0 => {
                    if reader.u32()? != 0 || reader.u64()? != 0 {
                        return Err(DecodeError::NonZeroReserved);
                    }
                    None
                }
                1 => Some(CoherentAllocationClaim {
                    allocation: reader.u32()?,
                    authority: reader.u64()?,
                }),
                _ => return Err(DecodeError::UnknownTag),
            };
            Ok(EventKind::Atomic(AtomicAccess {
                region,
                dialect,
                operation,
                value_type,
                address_space,
                alignment,
                scope,
                success_ordering,
                failure_ordering,
                coherent_allocation,
            }))
        }
        2 => {
            let region = decode_region(reader)?;
            let kind = match reader.u8()? {
                1 => AccessKind::Read,
                2 => AccessKind::Write,
                3 => AccessKind::ReadWrite,
                _ => return Err(DecodeError::UnknownTag),
            };
            let value_type = decode_scalar(reader)?;
            let address_space = decode_address_space(reader.u8()?)?;
            let alignment = reader.u32()?;
            if reader.u32()? != 0 {
                return Err(DecodeError::NonZeroReserved);
            }
            Ok(EventKind::NonAtomic(NonAtomicAccess {
                region,
                kind,
                value_type,
                address_space,
                alignment,
            }))
        }
        3 => {
            let scope = decode_scope(reader.u8()?)?;
            let ordering = decode_ordering(reader.u8()?)?;
            let domains = MemoryDomains::from_bits(reader.u8()?).ok_or(DecodeError::UnknownTag)?;
            if reader.u32()? != 0 {
                return Err(DecodeError::NonZeroReserved);
            }
            Ok(EventKind::Fence(Fence {
                scope,
                ordering,
                domains,
            }))
        }
        4 => {
            let kind = match reader.u8()? {
                1 => BarrierKind::Subgroup,
                2 => BarrierKind::Workgroup,
                3 => BarrierKind::CooperativeGroup,
                _ => return Err(DecodeError::UnknownTag),
            };
            let scope = decode_scope(reader.u8()?)?;
            let ordering = decode_ordering(reader.u8()?)?;
            let domains = MemoryDomains::from_bits(reader.u8()?).ok_or(DecodeError::UnknownTag)?;
            if reader.array::<3>()? != [0; 3] {
                return Err(DecodeError::NonZeroReserved);
            }
            Ok(EventKind::Barrier(Barrier {
                kind,
                scope,
                ordering,
                domains,
            }))
        }
        5 => {
            let kind = decode_collective(reader.u8()?)?;
            let value_type = decode_scalar(reader)?;
            if reader.u32()? != 0 {
                return Err(DecodeError::NonZeroReserved);
            }
            Ok(EventKind::Collective(Collective { kind, value_type }))
        }
        6 => {
            let kind = match reader.u8()? {
                1 => ShuffleKind::Index,
                2 => ShuffleKind::Up,
                3 => ShuffleKind::Down,
                4 => ShuffleKind::Xor,
                _ => return Err(DecodeError::UnknownTag),
            };
            let value_type = decode_scalar(reader)?;
            let tile_width = reader.u32()?;
            if reader.u32()? != 0 {
                return Err(DecodeError::NonZeroReserved);
            }
            Ok(EventKind::Shuffle(Shuffle {
                kind,
                value_type,
                tile_width,
            }))
        }
        7 => {
            let wave_size = reader.u32()?;
            let result_width = decode_integer_width(reader.u8()?)?;
            if reader.array::<3>()? != [0; 3] {
                return Err(DecodeError::NonZeroReserved);
            }
            Ok(EventKind::Ballot(Ballot {
                wave_size,
                result_width,
            }))
        }
        _ => Err(DecodeError::UnknownTag),
    }
}

fn encode_region(writer: &mut Writer, region: MemoryRegion) -> Result<(), ValidationError> {
    writer.u32(region.allocation)?;
    writer.u32(region.offset)?;
    writer.u32(region.bytes)?;
    Ok(())
}

fn decode_region(reader: &mut Reader<'_>) -> Result<MemoryRegion, DecodeError> {
    Ok(MemoryRegion {
        allocation: reader.u32()?,
        offset: reader.u32()?,
        bytes: reader.u32()?,
    })
}

fn encode_scalar(writer: &mut Writer, value_type: ScalarType) -> Result<(), ValidationError> {
    match value_type {
        ScalarType::Bool => writer.bytes(&[1, 0, 0, 0])?,
        ScalarType::Integer { width, signed } => {
            writer.bytes(&[2, width as u8, signed as u8, 0])?;
        }
        ScalarType::Float32 => writer.bytes(&[3, 0, 0, 0])?,
        ScalarType::Float64 => writer.bytes(&[4, 0, 0, 0])?,
        ScalarType::Pointer64 => writer.bytes(&[5, 0, 0, 0])?,
    }
    Ok(())
}

fn decode_scalar(reader: &mut Reader<'_>) -> Result<ScalarType, DecodeError> {
    let tag = reader.u8()?;
    let first = reader.u8()?;
    let second = reader.u8()?;
    let reserved = reader.u8()?;
    if reserved != 0 {
        return Err(DecodeError::NonZeroReserved);
    }
    match (tag, first, second) {
        (1, 0, 0) => Ok(ScalarType::Bool),
        (2, width, signed) => Ok(ScalarType::Integer {
            width: decode_integer_width(width)?,
            signed: decode_bool(signed)?,
        }),
        (3, 0, 0) => Ok(ScalarType::Float32),
        (4, 0, 0) => Ok(ScalarType::Float64),
        (5, 0, 0) => Ok(ScalarType::Pointer64),
        _ => Err(DecodeError::UnknownTag),
    }
}

fn decode_integer_width(tag: u8) -> Result<IntegerWidth, DecodeError> {
    match tag {
        1 => Ok(IntegerWidth::W8),
        2 => Ok(IntegerWidth::W16),
        3 => Ok(IntegerWidth::W32),
        4 => Ok(IntegerWidth::W64),
        5 => Ok(IntegerWidth::W128),
        _ => Err(DecodeError::UnknownTag),
    }
}

fn decode_target(tag: u8) -> Result<TargetProfile, DecodeError> {
    match tag {
        1 => Ok(TargetProfile::Gfx942Wave64),
        _ => Err(DecodeError::UnknownTag),
    }
}

fn decode_address_space(tag: u8) -> Result<AddressSpace, DecodeError> {
    match tag {
        1 => Ok(AddressSpace::Private),
        2 => Ok(AddressSpace::Global),
        3 => Ok(AddressSpace::Constant),
        4 => Ok(AddressSpace::Lds),
        5 => Ok(AddressSpace::Generic),
        _ => Err(DecodeError::UnknownTag),
    }
}

fn decode_scope(tag: u8) -> Result<MemoryScope, DecodeError> {
    match tag {
        1 => Ok(MemoryScope::Wavefront),
        2 => Ok(MemoryScope::Workgroup),
        3 => Ok(MemoryScope::Agent),
        4 => Ok(MemoryScope::System),
        _ => Err(DecodeError::UnknownTag),
    }
}

fn decode_ordering(tag: u8) -> Result<MemoryOrdering, DecodeError> {
    match tag {
        1 => Ok(MemoryOrdering::Relaxed),
        2 => Ok(MemoryOrdering::Acquire),
        3 => Ok(MemoryOrdering::Release),
        4 => Ok(MemoryOrdering::AcquireRelease),
        5 => Ok(MemoryOrdering::SequentiallyConsistent),
        _ => Err(DecodeError::UnknownTag),
    }
}

fn decode_event_outcome(tag: u8) -> Result<EventOutcome, DecodeError> {
    match tag {
        1 => Ok(EventOutcome::Unconditional),
        2 => Ok(EventOutcome::CompareExchangeSuccess),
        3 => Ok(EventOutcome::CompareExchangeFailure),
        _ => Err(DecodeError::UnknownTag),
    }
}

fn decode_read_from(tag: u8) -> Result<ReadFromCondition, DecodeError> {
    match tag {
        1 => Ok(ReadFromCondition::NotApplicable),
        2 => Ok(ReadFromCondition::VerifierMustProve),
        _ => Err(DecodeError::UnknownTag),
    }
}

fn decode_group(tag: u8) -> Result<GroupKind, DecodeError> {
    match tag {
        1 => Ok(GroupKind::Invocation),
        2 => Ok(GroupKind::Subgroup),
        3 => Ok(GroupKind::Workgroup),
        4 => Ok(GroupKind::CooperativeGrid),
        _ => Err(DecodeError::UnknownTag),
    }
}

fn decode_convergence(tag: u8) -> Result<ConvergenceContract, DecodeError> {
    match tag {
        1 => Ok(ConvergenceContract::NotRequired),
        2 => Ok(ConvergenceContract::UniformRequired),
        3 => Ok(ConvergenceContract::ExplicitMask),
        _ => Err(DecodeError::UnknownTag),
    }
}

fn decode_atomic_operation(tag: u8) -> Result<AtomicOperation, DecodeError> {
    match tag {
        1 => Ok(AtomicOperation::Load),
        2 => Ok(AtomicOperation::Store),
        3 => Ok(AtomicOperation::Exchange),
        4 => Ok(AtomicOperation::CompareExchangeStrong),
        5 => Ok(AtomicOperation::CompareExchangeWeak),
        6 => Ok(AtomicOperation::FetchAdd),
        7 => Ok(AtomicOperation::FetchSub),
        8 => Ok(AtomicOperation::FetchAnd),
        9 => Ok(AtomicOperation::FetchOr),
        10 => Ok(AtomicOperation::FetchXor),
        11 => Ok(AtomicOperation::FetchNand),
        12 => Ok(AtomicOperation::FetchMin),
        13 => Ok(AtomicOperation::FetchMax),
        14 => Ok(AtomicOperation::AmdInc),
        15 => Ok(AtomicOperation::AmdDec),
        16 => Ok(AtomicOperation::FloatAdd),
        _ => Err(DecodeError::UnknownTag),
    }
}

fn decode_collective(tag: u8) -> Result<CollectiveKind, DecodeError> {
    match tag {
        1 => Ok(CollectiveKind::ReduceAdd),
        2 => Ok(CollectiveKind::ReduceMin),
        3 => Ok(CollectiveKind::ReduceMax),
        4 => Ok(CollectiveKind::InclusiveScanAdd),
        5 => Ok(CollectiveKind::ExclusiveScanAdd),
        6 => Ok(CollectiveKind::Broadcast),
        7 => Ok(CollectiveKind::Any),
        8 => Ok(CollectiveKind::All),
        _ => Err(DecodeError::UnknownTag),
    }
}

fn decode_bool(tag: u8) -> Result<bool, DecodeError> {
    match tag {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(DecodeError::InvalidBoolean),
    }
}

fn digest_module(canonical_module: &[u8]) -> [u8; 32] {
    let mut digest = DomainSeparatedDigest::new(b"fe2o3.synchronization.module.v4");
    digest.bytes(canonical_module);
    digest.finish()
}

fn digest_obligations(obligations: &[VerifierObligation]) -> [u8; 32] {
    let mut digest = DomainSeparatedDigest::new(b"fe2o3.synchronization.obligations.v4");
    digest.u32(obligations.len() as u32);
    for obligation in obligations {
        match obligation {
            VerifierObligation::UniformParticipation {
                event,
                group,
                expected_participants,
                active_mask,
            } => {
                digest.u8(1);
                digest.u32(event.0);
                digest.u8(*group as u8);
                digest.u32(*expected_participants);
                digest.option_u64(*active_mask);
            }
            VerifierObligation::CooperativeParticipation {
                event,
                expected_participants,
            } => {
                digest.u8(2);
                digest.u32(event.0);
                digest.u32(*expected_participants);
            }
            VerifierObligation::HappensBefore {
                edge,
                before,
                after,
                kind,
                scope,
                domains,
                before_outcome,
                after_outcome,
                read_from,
                before_participation,
                after_participation,
                before_kind,
                after_kind,
                participant_witness,
                operation_witness,
            } => {
                digest.u8(3);
                digest.u32(*edge);
                digest.u32(before.0);
                digest.u32(after.0);
                digest.u8(match kind {
                    SynchronizationEdgeKind::ProgramOrder => 1,
                    SynchronizationEdgeKind::SynchronizesWith => 2,
                });
                digest.u8(*scope as u8);
                digest.u8(domains.bits());
                digest.u8(*before_outcome as u8);
                digest.u8(*after_outcome as u8);
                digest.u8(*read_from as u8);
                digest.participation(*before_participation);
                digest.participation(*after_participation);
                digest.event_kind(before_kind);
                digest.event_kind(after_kind);
                digest.u8(match participant_witness {
                    ParticipantWitness::SameParticipantMustProve => 1,
                    ParticipantWitness::SynchronizingParticipantsMustProve => 2,
                    ParticipantWitness::SameBarrierCohortMustProve => 3,
                });
                match operation_witness {
                    SynchronizationOperationWitness::ProgramOrder => digest.u8(1),
                    SynchronizationOperationWitness::AtomicReadFrom {
                        region,
                        before_operation,
                        after_operation,
                    } => {
                        digest.u8(2);
                        digest.region(*region);
                        digest.u8(*before_operation as u8);
                        digest.u8(*after_operation as u8);
                    }
                    SynchronizationOperationWitness::BarrierPhase {
                        kind,
                        expected_participants,
                    } => {
                        digest.u8(3);
                        digest.u8(*kind as u8);
                        digest.u32(*expected_participants);
                    }
                }
            }
            VerifierObligation::NonAtomicConflict {
                first,
                second,
                address_space,
                structurally_ordered,
                aliasing,
            } => {
                digest.u8(4);
                digest.u32(first.0);
                digest.u32(second.0);
                digest.u8(*address_space as u8);
                digest.boolean(*structurally_ordered);
                digest.u8(match aliasing {
                    AliasingCondition::ConfirmedOverlap => 1,
                    AliasingCondition::VerifierMustProveDisjoint => 2,
                });
            }
            VerifierObligation::DischargeAllocationAlias {
                first,
                second,
                address_space,
                first_region,
                second_region,
                consequence,
            } => {
                digest.u8(5);
                digest.u32(first.0);
                digest.u32(second.0);
                digest.u8(*address_space as u8);
                digest.region(*first_region);
                digest.region(*second_region);
                digest.u8(match consequence {
                    AllocationAliasConsequence::ReadOnlyOverlap => 1,
                    AllocationAliasConsequence::NonAtomicConflict => 2,
                    AllocationAliasConsequence::AtomicObjectCompatibility => 3,
                });
            }
            VerifierObligation::ScopeCompatibility {
                first,
                second,
                required_scope,
            } => {
                digest.u8(6);
                digest.u32(first.0);
                digest.u32(second.0);
                digest.u8(*required_scope as u8);
            }
            VerifierObligation::AuthenticateCoherentAllocation {
                event,
                allocation,
                authority,
            } => {
                digest.u8(7);
                digest.u32(event.0);
                digest.u32(*allocation);
                digest.u64(*authority);
            }
            VerifierObligation::LdsBankMapping {
                allocation,
                base_offset,
                bank_count,
                bank_width,
                element_stride,
                swizzle,
            } => {
                digest.u8(8);
                digest.u32(allocation.0);
                digest.u32(*base_offset);
                digest.u16(*bank_count);
                digest.u16(*bank_width);
                digest.u32(*element_stride);
                match swizzle {
                    LdsSwizzle::Linear => {
                        digest.u8(1);
                        digest.u8(0);
                    }
                    LdsSwizzle::Xor { shift } => {
                        digest.u8(2);
                        digest.u8(*shift);
                    }
                }
            }
            VerifierObligation::FenceSemantics {
                event,
                participation,
                fence,
            } => {
                digest.u8(9);
                digest.u32(event.0);
                digest.participation(*participation);
                digest.fence(fence);
            }
            VerifierObligation::BarrierSemantics {
                event,
                participation,
                barrier,
            } => {
                digest.u8(10);
                digest.u32(event.0);
                digest.participation(*participation);
                digest.barrier(barrier);
            }
            VerifierObligation::CollectiveSemantics {
                event,
                participation,
                collective,
            } => {
                digest.u8(11);
                digest.u32(event.0);
                digest.participation(*participation);
                digest.collective(collective);
            }
        }
    }
    digest.finish()
}

fn digest_report(
    module_digest: [u8; 32],
    obligations_digest: [u8; 32],
    target: TargetProfile,
    target_limits: TargetHardLimits,
    policy_limits: SynchronizationLimits,
) -> [u8; 32] {
    let mut digest = DomainSeparatedDigest::new(b"fe2o3.synchronization.report.v4");
    digest.bytes(&module_digest);
    digest.bytes(&obligations_digest);
    digest.u8(target as u8);
    digest.u32(target_limits.wave_size);
    digest.u32(target_limits.max_lds_bytes);
    digest.u32(target_limits.max_workgroup_participants);
    digest.u32(target_limits.max_cooperative_participants);
    digest.u32(policy_limits.max_lds_allocations);
    digest.u32(policy_limits.max_events);
    digest.u32(policy_limits.max_edges);
    digest.u32(policy_limits.max_total_lds_bytes);
    digest.u64(policy_limits.max_encoded_bytes);
    digest.u32(policy_limits.max_obligations);
    digest.u64(policy_limits.max_pair_checks);
    digest.u32(policy_limits.max_workgroup_participants);
    digest.u32(policy_limits.max_cooperative_participants);
    digest.finish()
}

struct DomainSeparatedDigest {
    sha256: Sha256,
}

impl DomainSeparatedDigest {
    fn new(domain: &[u8]) -> Self {
        let mut sha256 = Sha256::new();
        sha256.update(&(domain.len() as u32).to_le_bytes());
        sha256.update(domain);
        Self { sha256 }
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.sha256.update(bytes);
    }

    fn boolean(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    fn u8(&mut self, value: u8) {
        self.bytes(&[value]);
    }

    fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    fn option_u64(&mut self, value: Option<u64>) {
        self.boolean(value.is_some());
        self.u64(value.unwrap_or(0));
    }

    fn region(&mut self, region: MemoryRegion) {
        self.u32(region.allocation);
        self.u32(region.offset);
        self.u32(region.bytes);
    }

    fn scalar(&mut self, value_type: ScalarType) {
        match value_type {
            ScalarType::Bool => self.u8(1),
            ScalarType::Integer { width, signed } => {
                self.u8(2);
                self.u8(width as u8);
                self.boolean(signed);
            }
            ScalarType::Float32 => self.u8(3),
            ScalarType::Float64 => self.u8(4),
            ScalarType::Pointer64 => self.u8(5),
        }
    }

    fn participation(&mut self, participation: ParticipationContract) {
        self.u8(participation.group as u8);
        self.u8(participation.convergence as u8);
        self.u32(participation.expected_participants);
        self.option_u64(participation.active_mask);
    }

    fn fence(&mut self, fence: &Fence) {
        self.u8(fence.scope as u8);
        self.u8(fence.ordering as u8);
        self.u8(fence.domains.bits());
    }

    fn barrier(&mut self, barrier: &Barrier) {
        self.u8(barrier.kind as u8);
        self.u8(barrier.scope as u8);
        self.u8(barrier.ordering as u8);
        self.u8(barrier.domains.bits());
    }

    fn collective(&mut self, collective: &Collective) {
        self.u8(collective.kind as u8);
        self.scalar(collective.value_type);
    }

    fn event_kind(&mut self, kind: &EventKind) {
        match kind {
            EventKind::Atomic(atomic) => {
                self.u8(1);
                self.region(atomic.region);
                self.u8(atomic.dialect as u8);
                self.u8(atomic.operation as u8);
                self.scalar(atomic.value_type);
                self.u8(atomic.address_space as u8);
                self.u32(atomic.alignment);
                self.u8(atomic.scope as u8);
                self.u8(atomic.success_ordering as u8);
                self.u8(atomic.failure_ordering.map_or(0, |ordering| ordering as u8));
                self.boolean(atomic.coherent_allocation.is_some());
                let claim = atomic
                    .coherent_allocation
                    .unwrap_or(CoherentAllocationClaim {
                        allocation: 0,
                        authority: 0,
                    });
                self.u32(claim.allocation);
                self.u64(claim.authority);
            }
            EventKind::NonAtomic(access) => {
                self.u8(2);
                self.region(access.region);
                self.u8(access.kind as u8);
                self.scalar(access.value_type);
                self.u8(access.address_space as u8);
                self.u32(access.alignment);
            }
            EventKind::Fence(fence) => {
                self.u8(3);
                self.fence(fence);
            }
            EventKind::Barrier(barrier) => {
                self.u8(4);
                self.barrier(barrier);
            }
            EventKind::Collective(collective) => {
                self.u8(5);
                self.collective(collective);
            }
            EventKind::Shuffle(shuffle) => {
                self.u8(6);
                self.u8(shuffle.kind as u8);
                self.scalar(shuffle.value_type);
                self.u32(shuffle.tile_width);
            }
            EventKind::Ballot(ballot) => {
                self.u8(7);
                self.u32(ballot.wave_size);
                self.u8(ballot.result_width as u8);
            }
        }
    }

    fn finish(self) -> [u8; 32] {
        self.sha256.finalize()
    }
}

struct Sha256 {
    state: [u32; 8],
    buffer: [u8; 64],
    buffer_len: usize,
    message_len: u64,
}

impl Sha256 {
    const ROUND_CONSTANTS: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    const fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            buffer: [0; 64],
            buffer_len: 0,
            message_len: 0,
        }
    }

    fn update(&mut self, mut bytes: &[u8]) {
        self.message_len = self
            .message_len
            .checked_add(bytes.len() as u64)
            .expect("bounded synchronization digest length");
        if self.buffer_len != 0 {
            let count = (64 - self.buffer_len).min(bytes.len());
            self.buffer[self.buffer_len..self.buffer_len + count].copy_from_slice(&bytes[..count]);
            self.buffer_len += count;
            bytes = &bytes[count..];
            if self.buffer_len != 64 {
                return;
            }
            let block = self.buffer;
            self.compress(&block);
            self.buffer_len = 0;
        }
        while bytes.len() >= 64 {
            let block: &[u8; 64] = bytes[..64].try_into().expect("exact SHA-256 block");
            self.compress(block);
            bytes = &bytes[64..];
        }
        self.buffer[..bytes.len()].copy_from_slice(bytes);
        self.buffer_len = bytes.len();
    }

    fn finalize(mut self) -> [u8; 32] {
        let bit_len = self
            .message_len
            .checked_mul(8)
            .expect("bounded synchronization digest bit length");
        self.buffer[self.buffer_len] = 0x80;
        self.buffer_len += 1;
        if self.buffer_len > 56 {
            self.buffer[self.buffer_len..].fill(0);
            let block = self.buffer;
            self.compress(&block);
            self.buffer = [0; 64];
        } else {
            self.buffer[self.buffer_len..56].fill(0);
        }
        self.buffer[56..].copy_from_slice(&bit_len.to_be_bytes());
        let block = self.buffer;
        self.compress(&block);

        let mut output = [0_u8; 32];
        for (chunk, word) in output.chunks_exact_mut(4).zip(self.state) {
            chunk.copy_from_slice(&word.to_be_bytes());
        }
        output
    }

    fn compress(&mut self, block: &[u8; 64]) {
        let mut words = [0_u32; 64];
        for (index, chunk) in block.chunks_exact(4).enumerate() {
            words[index] = u32::from_be_bytes(chunk.try_into().expect("four-byte SHA-256 word"));
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;
        for (word, constant) in words.into_iter().zip(Self::ROUND_CONSTANTS) {
            let sum1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choose = (e & f) ^ ((!e) & g);
            let temporary1 = h
                .wrapping_add(sum1)
                .wrapping_add(choose)
                .wrapping_add(constant)
                .wrapping_add(word);
            let sum0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temporary2 = sum0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temporary1);
            d = c;
            c = b;
            b = a;
            a = temporary1.wrapping_add(temporary2);
        }
        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }
}

#[cfg(test)]
pub fn sha256_test_vector(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(bytes);
    digest.finalize()
}

struct Writer {
    bytes: Vec<u8>,
    maximum: u64,
}

impl Writer {
    fn new(maximum: u64) -> Self {
        Self {
            bytes: Vec::new(),
            maximum,
        }
    }

    fn len(&self) -> usize {
        self.bytes.len()
    }

    fn bytes(&mut self, value: &[u8]) -> Result<(), ValidationError> {
        let new_len = self
            .bytes
            .len()
            .checked_add(value.len())
            .ok_or(ValidationError::EncodingSizeOverflow)?;
        check_limit(Resource::EncodedBytes, usize_u64(new_len)?, self.maximum)?;
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn u8(&mut self, value: u8) -> Result<(), ValidationError> {
        self.bytes(&[value])
    }

    fn u16(&mut self, value: u16) -> Result<(), ValidationError> {
        self.bytes(&value.to_le_bytes())
    }

    fn u32(&mut self, value: u32) -> Result<(), ValidationError> {
        self.bytes(&value.to_le_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<(), ValidationError> {
        self.bytes(&value.to_le_bytes())
    }

    fn patch_u32(&mut self, offset: usize, value: u32) {
        self.bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], DecodeError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(DecodeError::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(DecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn take_reader(&mut self, length: usize) -> Result<Reader<'a>, DecodeError> {
        Ok(Reader::new(self.take(length)?))
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        self.take(N)?.try_into().map_err(|_| DecodeError::Truncated)
    }

    fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, DecodeError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, DecodeError> {
        Ok(u64::from_le_bytes(self.array()?))
    }
}

fn check_limit(resource: Resource, observed: u64, limit: u64) -> Result<(), ValidationError> {
    if observed > limit {
        Err(ValidationError::ResourceLimit {
            resource,
            observed,
            limit,
        })
    } else {
        Ok(())
    }
}

fn insert_obligation(
    obligations: &mut BTreeSet<VerifierObligation>,
    obligation: VerifierObligation,
    limits: &SynchronizationLimits,
) -> Result<(), ValidationError> {
    obligations.insert(obligation);
    check_limit(
        Resource::Obligations,
        usize_u64(obligations.len())?,
        u64::from(limits.max_obligations),
    )
}

fn check_decode_limit(resource: Resource, observed: u64, limit: u64) -> Result<(), DecodeError> {
    if observed > limit {
        Err(DecodeError::ResourceLimit {
            resource,
            observed,
            limit,
        })
    } else {
        Ok(())
    }
}

fn usize_u64(value: usize) -> Result<u64, ValidationError> {
    u64::try_from(value).map_err(|_| ValidationError::ArithmeticOverflow)
}

fn usize_u64_decode(value: usize) -> Result<u64, DecodeError> {
    u64::try_from(value).map_err(|_| DecodeError::LengthMismatch)
}

fn u32_len(value: usize) -> Result<u32, ValidationError> {
    u32::try_from(value).map_err(|_| ValidationError::EncodingSizeOverflow)
}
