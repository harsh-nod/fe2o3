//! Inert, bounded memory-safety and provenance model for a future Kernel IR V2.
//!
//! This module is deliberately not exported by `fe2o3-kernel-ir`. It is a pure,
//! sequential transition model over caller-authored facts. Successful execution
//! means only that this model discharged its local predicates. It authenticates
//! no runtime allocation, GPU behavior, compiler refinement, proof execution,
//! or inter-invocation race-freedom.

use std::fmt;

const MAGIC: [u8; 8] = *b"FE2OMEM2";
const VERSION: u16 = 2;
const HARD_MAX_TYPES: u32 = 4_096;
const HARD_MAX_TYPE_EDGES: u32 = 16_384;
const HARD_MAX_VALIDITY_RANGES: u32 = 16_384;
const HARD_MAX_ACTIONS: u32 = 65_536;
const HARD_MAX_PROJECTIONS_PER_PLACE: u32 = 64;
const HARD_MAX_ALLOCATIONS: u32 = 4_096;
const HARD_MAX_LOANS: u32 = 16_384;
const HARD_MAX_CAPABILITIES: u32 = 16_384;
const HARD_MAX_STATE_RANGES: u32 = 65_536;
const HARD_MAX_OBLIGATIONS: u32 = 262_144;
const HARD_MAX_CANONICAL_BYTES: u32 = 16 * 1024 * 1024;
const HARD_MAX_VALIDATION_WORK: u64 = 4_000_000;
const HARD_MAX_EXECUTION_WORK: u64 = 4_000_000;
const PROGRAM_IDENTITY_DOMAIN: &[u8] = b"fe2o3.memory-proof-v2.program-identity.v2\0";
const ACTION_IDENTITY_DOMAIN: &[u8] = b"fe2o3.memory-proof-v2.action-identity.v2\0";
const OBLIGATION_IDENTITY_DOMAIN: &[u8] = b"fe2o3.memory-proof-v2.obligation-identity.v2\0";
const TRANSITION_IDENTITY_DOMAIN: &[u8] = b"fe2o3.memory-proof-v2.transition-identity.v2\0";
const REPORT_IDENTITY_DOMAIN: &[u8] = b"fe2o3.memory-proof-v2.report-identity.v2\0";
const MIN_TARGET_ENTRY_BYTES: usize = 5;
const MIN_TYPE_BYTES: usize = 21;
const MIN_ACTION_BYTES: usize = 9;
const MIN_FIELD_BYTES: usize = 12;
const MIN_VALIDITY_RANGE_BYTES: usize = 32;
const MIN_PROJECTION_BYTES: usize = 5;
const GFX942_ADDRESS_SPACES: [AddressSpaceLayoutV2; 5] = [
    AddressSpaceLayoutV2 {
        address_space: AddressSpaceV2::Flat,
        pointer_bits: 64,
        pointer_alignment: 64,
    },
    AddressSpaceLayoutV2 {
        address_space: AddressSpaceV2::Global,
        pointer_bits: 64,
        pointer_alignment: 64,
    },
    AddressSpaceLayoutV2 {
        address_space: AddressSpaceV2::Workgroup,
        pointer_bits: 32,
        pointer_alignment: 32,
    },
    AddressSpaceLayoutV2 {
        address_space: AddressSpaceV2::Constant,
        pointer_bits: 64,
        pointer_alignment: 64,
    },
    AddressSpaceLayoutV2 {
        address_space: AddressSpaceV2::Private,
        pointer_bits: 32,
        pointer_alignment: 32,
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WorkMeterV2 {
    resource: &'static str,
    used: u64,
    max: u64,
    decoded_type_edges: u64,
    decoded_validity_ranges: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DecodeCollectionKindV2 {
    Ordinary,
    TypeEdges,
    ValidityRanges,
}

impl WorkMeterV2 {
    const fn validation(max: u64) -> Self {
        Self {
            resource: "validation work",
            used: 0,
            max,
            decoded_type_edges: 0,
            decoded_validity_ranges: 0,
        }
    }

    const fn execution(max: u64) -> Self {
        Self {
            resource: "execution work",
            used: 0,
            max,
            decoded_type_edges: 0,
            decoded_validity_ranges: 0,
        }
    }

    fn charge(&mut self, amount: u64) -> Result<(), MemoryErrorReasonV2> {
        self.used = charge_resource(self.resource, self.used, amount, self.max)?;
        Ok(())
    }

    const fn used(self) -> u64 {
        self.used
    }

    fn with_used(resource: &'static str, used: u64, max: u64) -> Result<Self, MemoryErrorReasonV2> {
        enforce(resource, used, max)?;
        Ok(Self {
            resource,
            used,
            max,
            decoded_type_edges: 0,
            decoded_validity_ranges: 0,
        })
    }

    fn admit_decoded_collection(
        &mut self,
        kind: DecodeCollectionKindV2,
        count: usize,
        budgets: MemoryBudgetsV2,
    ) -> Result<(), MemoryErrorReasonV2> {
        let count = u64::try_from(count).map_err(|_| MemoryErrorReasonV2::ResourceLimit {
            resource: "decoded collection items",
            actual: u64::MAX,
            max: u64::from(HARD_MAX_ACTIONS),
        })?;
        match kind {
            DecodeCollectionKindV2::Ordinary => {}
            DecodeCollectionKindV2::TypeEdges => {
                self.decoded_type_edges = charge_resource(
                    "type edges",
                    self.decoded_type_edges,
                    count,
                    u64::from(budgets.max_type_edges),
                )?;
            }
            DecodeCollectionKindV2::ValidityRanges => {
                self.decoded_validity_ranges = charge_resource(
                    "validity ranges",
                    self.decoded_validity_ranges,
                    count,
                    u64::from(budgets.max_validity_ranges),
                )?;
            }
        }
        self.charge(count)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MemoryBudgetsV2 {
    pub max_types: u32,
    pub max_type_edges: u32,
    pub max_validity_ranges: u32,
    pub max_actions: u32,
    pub max_projections_per_place: u32,
    pub max_allocations: u32,
    pub max_loans: u32,
    pub max_capabilities: u32,
    pub max_state_ranges: u32,
    pub max_obligations: u32,
    pub max_canonical_bytes: u32,
    pub max_validation_work: u64,
    pub max_execution_work: u64,
}

impl Default for MemoryBudgetsV2 {
    fn default() -> Self {
        Self {
            max_types: 4_096,
            max_type_edges: 16_384,
            max_validity_ranges: 16_384,
            max_actions: 65_536,
            max_projections_per_place: 64,
            max_allocations: 4_096,
            max_loans: 16_384,
            max_capabilities: 16_384,
            max_state_ranges: 65_536,
            max_obligations: 262_144,
            max_canonical_bytes: 16 * 1024 * 1024,
            max_validation_work: 4_000_000,
            max_execution_work: 4_000_000,
        }
    }
}

impl MemoryBudgetsV2 {
    fn validate_hard_caps(self) -> Result<(), MemoryErrorReasonV2> {
        let configured = [
            (
                "configured types",
                u64::from(self.max_types),
                u64::from(HARD_MAX_TYPES),
            ),
            (
                "configured type edges",
                u64::from(self.max_type_edges),
                u64::from(HARD_MAX_TYPE_EDGES),
            ),
            (
                "configured validity ranges",
                u64::from(self.max_validity_ranges),
                u64::from(HARD_MAX_VALIDITY_RANGES),
            ),
            (
                "configured actions",
                u64::from(self.max_actions),
                u64::from(HARD_MAX_ACTIONS),
            ),
            (
                "configured place projections",
                u64::from(self.max_projections_per_place),
                u64::from(HARD_MAX_PROJECTIONS_PER_PLACE),
            ),
            (
                "configured allocations",
                u64::from(self.max_allocations),
                u64::from(HARD_MAX_ALLOCATIONS),
            ),
            (
                "configured loans",
                u64::from(self.max_loans),
                u64::from(HARD_MAX_LOANS),
            ),
            (
                "configured capabilities",
                u64::from(self.max_capabilities),
                u64::from(HARD_MAX_CAPABILITIES),
            ),
            (
                "configured state ranges",
                u64::from(self.max_state_ranges),
                u64::from(HARD_MAX_STATE_RANGES),
            ),
            (
                "configured obligations",
                u64::from(self.max_obligations),
                u64::from(HARD_MAX_OBLIGATIONS),
            ),
            (
                "configured canonical bytes",
                u64::from(self.max_canonical_bytes),
                u64::from(HARD_MAX_CANONICAL_BYTES),
            ),
            (
                "configured validation work",
                self.max_validation_work,
                HARD_MAX_VALIDATION_WORK,
            ),
            (
                "configured execution work",
                self.max_execution_work,
                HARD_MAX_EXECUTION_WORK,
            ),
        ];
        for (resource, actual, max) in configured {
            enforce(resource, actual, max)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AddressSpaceV2 {
    Flat,
    Global,
    Workgroup,
    Constant,
    Private,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PhysicalAliasDomainV2 {
    GlobalFlat,
    Workgroup,
    Private,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MemoryMutabilityV2 {
    ReadOnly,
    ReadWrite,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct AddressSpaceSemanticsV2 {
    pub alias_domain: PhysicalAliasDomainV2,
    pub mutability: MemoryMutabilityV2,
}

impl AddressSpaceV2 {
    const fn tag(self) -> u8 {
        match self {
            Self::Flat => 0,
            Self::Global => 1,
            Self::Workgroup => 3,
            Self::Constant => 4,
            Self::Private => 5,
        }
    }
    fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::Flat),
            1 => Some(Self::Global),
            3 => Some(Self::Workgroup),
            4 => Some(Self::Constant),
            5 => Some(Self::Private),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct AddressSpaceLayoutV2 {
    pub address_space: AddressSpaceV2,
    pub pointer_bits: u16,
    pub pointer_alignment: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetLayoutV2 {
    architecture: String,
    xnack_disabled: bool,
    little_endian: bool,
    address_spaces: Vec<AddressSpaceLayoutV2>,
}

impl TargetLayoutV2 {
    pub fn gfx942_xnack_minus() -> Self {
        Self {
            architecture: "gfx942".into(),
            xnack_disabled: true,
            little_endian: true,
            address_spaces: GFX942_ADDRESS_SPACES.to_vec(),
        }
    }
    pub fn architecture(&self) -> &str {
        &self.architecture
    }
    pub const fn xnack_disabled(&self) -> bool {
        self.xnack_disabled
    }
    pub const fn little_endian(&self) -> bool {
        self.little_endian
    }
    pub fn address_spaces(&self) -> &[AddressSpaceLayoutV2] {
        &self.address_spaces
    }
    fn pointer_bits(&self, space: AddressSpaceV2) -> Option<u16> {
        self.address_spaces
            .iter()
            .find(|entry| entry.address_space == space)
            .map(|entry| entry.pointer_bits)
    }
    pub const fn address_space_semantics(&self, space: AddressSpaceV2) -> AddressSpaceSemanticsV2 {
        match space {
            AddressSpaceV2::Flat | AddressSpaceV2::Global => AddressSpaceSemanticsV2 {
                alias_domain: PhysicalAliasDomainV2::GlobalFlat,
                mutability: MemoryMutabilityV2::ReadWrite,
            },
            AddressSpaceV2::Constant => AddressSpaceSemanticsV2 {
                alias_domain: PhysicalAliasDomainV2::GlobalFlat,
                mutability: MemoryMutabilityV2::ReadOnly,
            },
            AddressSpaceV2::Workgroup => AddressSpaceSemanticsV2 {
                alias_domain: PhysicalAliasDomainV2::Workgroup,
                mutability: MemoryMutabilityV2::ReadWrite,
            },
            AddressSpaceV2::Private => AddressSpaceSemanticsV2 {
                alias_domain: PhysicalAliasDomainV2::Private,
                mutability: MemoryMutabilityV2::ReadWrite,
            },
        }
    }
    fn validate(&self) -> Result<(), MemoryErrorReasonV2> {
        if self.architecture != "gfx942"
            || !self.xnack_disabled
            || !self.little_endian
            || self.address_spaces.as_slice() != GFX942_ADDRESS_SPACES
        {
            Err(MemoryErrorReasonV2::UnsupportedTargetLayout)
        } else {
            Ok(())
        }
    }
}

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(u32);
        impl $name {
            pub const fn new(value: u32) -> Option<Self> {
                if value == 0 { None } else { Some(Self(value)) }
            }
            pub const fn get(self) -> u32 {
                self.0
            }
        }
    };
}

id_type!(MemoryTypeIdV2);
id_type!(AllocationIdV2);
id_type!(OwnerIdV2);
id_type!(LoanIdV2);
id_type!(CapabilityIdV2);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct EpochV2(pub u64);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LifetimeRegionV2 {
    pub start: EpochV2,
    pub end_inclusive: EpochV2,
}
impl LifetimeRegionV2 {
    pub const fn contains(self, epoch: EpochV2) -> bool {
        self.start.0 <= epoch.0 && epoch.0 <= self.end_inclusive.0
    }
    fn valid(self) -> bool {
        self.start.0 <= self.end_inclusive.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ByteRangeV2 {
    pub start: u64,
    pub len: u64,
}
impl ByteRangeV2 {
    pub fn end(self) -> Option<u64> {
        self.start.checked_add(self.len)
    }
    pub fn contains(self, other: Self) -> bool {
        match (self.end(), other.end()) {
            (Some(end), Some(other_end)) => self.start <= other.start && other_end <= end,
            _ => false,
        }
    }
    pub fn overlaps(self, other: Self) -> bool {
        if self.len == 0 || other.len == 0 {
            return false;
        }
        match (self.end(), other.end()) {
            (Some(end), Some(other_end)) => self.start < other_end && other.start < end,
            _ => true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct BitValidityRangeV2 {
    pub start: u128,
    pub end_inclusive: u128,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BitValidityV2 {
    Any,
    Bool,
    Char,
    NonZero,
    Ranges(Vec<BitValidityRangeV2>),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MemoryFieldV2 {
    pub offset: u64,
    pub ty: MemoryTypeIdV2,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MemoryTypeKindV2 {
    Scalar {
        bit_width: u16,
        validity: BitValidityV2,
    },
    Array {
        element: MemoryTypeIdV2,
        length: u64,
        stride: u64,
    },
    Aggregate {
        fields: Vec<MemoryFieldV2>,
    },
    OpaqueBytes,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MemoryTypeV2 {
    pub id: MemoryTypeIdV2,
    pub size: u64,
    pub alignment: u64,
    pub kind: MemoryTypeKindV2,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProvenanceV2 {
    pub allocation: AllocationIdV2,
    pub generation: u64,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProjectionV2 {
    Field(u32),
    Index(u64),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TypedPlaceV2 {
    pub provenance: ProvenanceV2,
    pub base_offset: u64,
    pub root_type: MemoryTypeIdV2,
    pub projections: Vec<ProjectionV2>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RawPlaceV2 {
    pub provenance: ProvenanceV2,
    pub pointer_address_space: AddressSpaceV2,
    pub byte_offset: u64,
    pub byte_len: u64,
    pub alignment: u64,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BorrowKindV2 {
    Shared,
    Exclusive,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AccessActorV2 {
    Owner(OwnerIdV2),
    Loan { loan: LoanIdV2, borrow_epoch: u64 },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RawAccessV2 {
    Read,
    Write,
    ReadWrite,
}
impl RawAccessV2 {
    fn permits(self, write: bool) -> bool {
        matches!(
            (self, write),
            (Self::Read, false) | (Self::Write, true) | (Self::ReadWrite, _)
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CapabilityScopeV2 {
    Owner(OwnerIdV2),
    Loan { loan: LoanIdV2, borrow_epoch: u64 },
}
impl CapabilityScopeV2 {
    fn actor(self) -> AccessActorV2 {
        match self {
            Self::Owner(owner) => AccessActorV2::Owner(owner),
            Self::Loan { loan, borrow_epoch } => AccessActorV2::Loan { loan, borrow_epoch },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TypedWriteValueV2 {
    KnownBits(u128),
    ValidOpaque,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MemoryActionV2 {
    Allocate {
        allocation: AllocationIdV2,
        generation: u64,
        owner: OwnerIdV2,
        address_space: AddressSpaceV2,
        base_address: u64,
        byte_len: u64,
        alignment: u64,
        lifetime: LifetimeRegionV2,
    },
    AdvanceEpoch {
        to: EpochV2,
    },
    BeginBorrow {
        loan: LoanIdV2,
        owner: OwnerIdV2,
        place: TypedPlaceV2,
        kind: BorrowKindV2,
        lifetime: LifetimeRegionV2,
    },
    EndBorrow {
        loan: LoanIdV2,
        owner: OwnerIdV2,
    },
    WriteTyped {
        actor: AccessActorV2,
        place: TypedPlaceV2,
        value: TypedWriteValueV2,
    },
    ReadTyped {
        actor: AccessActorV2,
        place: TypedPlaceV2,
    },
    GrantRawCapability {
        capability: CapabilityIdV2,
        owner: OwnerIdV2,
        provenance: ProvenanceV2,
        scope: CapabilityScopeV2,
        range: ByteRangeV2,
        access: RawAccessV2,
        lifetime: LifetimeRegionV2,
    },
    GrantAddressSpaceCastCapability {
        capability: CapabilityIdV2,
        owner: OwnerIdV2,
        provenance: ProvenanceV2,
        scope: CapabilityScopeV2,
        range: ByteRangeV2,
        from: AddressSpaceV2,
        to: AddressSpaceV2,
        lifetime: LifetimeRegionV2,
    },
    ReadRaw {
        actor: AccessActorV2,
        place: RawPlaceV2,
        raw_capability: CapabilityIdV2,
        cast_capability: Option<CapabilityIdV2>,
    },
    WriteRaw {
        actor: AccessActorV2,
        place: RawPlaceV2,
        raw_capability: CapabilityIdV2,
        cast_capability: Option<CapabilityIdV2>,
    },
    PointerDistance {
        actor: AccessActorV2,
        left: RawPlaceV2,
        right: RawPlaceV2,
        element_size: u64,
        left_capability: CapabilityIdV2,
        right_capability: CapabilityIdV2,
        left_cast_capability: Option<CapabilityIdV2>,
        right_cast_capability: Option<CapabilityIdV2>,
    },
    CopyNonOverlapping {
        actor: AccessActorV2,
        source: RawPlaceV2,
        destination: RawPlaceV2,
        source_capability: CapabilityIdV2,
        destination_capability: CapabilityIdV2,
        source_cast_capability: Option<CapabilityIdV2>,
        destination_cast_capability: Option<CapabilityIdV2>,
    },
    Deallocate {
        allocation: AllocationIdV2,
        owner: OwnerIdV2,
    },
}

impl MemoryActionV2 {
    fn typed_place(&self) -> Option<&TypedPlaceV2> {
        match self {
            Self::BeginBorrow { place, .. }
            | Self::WriteTyped { place, .. }
            | Self::ReadTyped { place, .. } => Some(place),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct MemoryProgramV2 {
    target: TargetLayoutV2,
    types: Vec<MemoryTypeV2>,
    actions: Vec<MemoryActionV2>,
    admission_validation_work: u64,
}

impl PartialEq for MemoryProgramV2 {
    fn eq(&self, other: &Self) -> bool {
        self.target == other.target && self.types == other.types && self.actions == other.actions
    }
}

impl Eq for MemoryProgramV2 {}

impl MemoryProgramV2 {
    pub fn new(
        target: TargetLayoutV2,
        types: Vec<MemoryTypeV2>,
        actions: Vec<MemoryActionV2>,
        budgets: MemoryBudgetsV2,
    ) -> Result<Self, MemoryModelErrorV2> {
        Self::new_with_work(target, types, actions, budgets).map(|(program, _work)| program)
    }
    pub fn new_with_work(
        target: TargetLayoutV2,
        mut types: Vec<MemoryTypeV2>,
        actions: Vec<MemoryActionV2>,
        budgets: MemoryBudgetsV2,
    ) -> Result<(Self, u64), MemoryModelErrorV2> {
        budgets
            .validate_hard_caps()
            .map_err(MemoryModelErrorV2::static_error)?;
        let mut work = WorkMeterV2::validation(budgets.max_validation_work);
        validate_program_envelope(&target, types.len(), actions.len(), budgets, &mut work)
            .map_err(MemoryModelErrorV2::static_error)?;
        work.charge(sort_work(types.len() as u64))
            .map_err(MemoryModelErrorV2::static_error)?;
        types.sort_unstable_by_key(|ty| ty.id);
        let mut program = Self {
            target,
            types,
            actions,
            admission_validation_work: 0,
        };
        program
            .validate_types(budgets, &mut work)
            .map_err(MemoryModelErrorV2::static_error)?;
        program
            .validate_action_shapes(budgets, &mut work)
            .map_err(MemoryModelErrorV2::static_error)?;
        let bytes = program
            .encode_unchecked(budgets, &mut work)
            .map_err(MemoryModelErrorV2::static_error)?;
        enforce(
            "canonical bytes",
            bytes.len() as u64,
            budgets.max_canonical_bytes as u64,
        )
        .map_err(MemoryModelErrorV2::static_error)?;
        program.admission_validation_work = work.used();
        Ok((program, work.used()))
    }
    pub fn target(&self) -> &TargetLayoutV2 {
        &self.target
    }
    pub fn types(&self) -> &[MemoryTypeV2] {
        &self.types
    }
    pub fn actions(&self) -> &[MemoryActionV2] {
        &self.actions
    }
    pub const fn admission_validation_work(&self) -> u64 {
        self.admission_validation_work
    }
    pub fn canonical_bytes(&self, budgets: MemoryBudgetsV2) -> Result<Vec<u8>, MemoryModelErrorV2> {
        self.canonical_bytes_with_work(budgets)
            .map(|(bytes, _work)| bytes)
    }
    pub fn canonical_bytes_with_work(
        &self,
        budgets: MemoryBudgetsV2,
    ) -> Result<(Vec<u8>, u64), MemoryModelErrorV2> {
        let (bytes, work) = self.canonical_bytes_continuing_from(budgets, 0)?;
        Ok((bytes, work.used()))
    }

    fn canonical_bytes_continuing_from(
        &self,
        budgets: MemoryBudgetsV2,
        used: u64,
    ) -> Result<(Vec<u8>, WorkMeterV2), MemoryModelErrorV2> {
        budgets
            .validate_hard_caps()
            .map_err(MemoryModelErrorV2::static_error)?;
        let mut work = WorkMeterV2::with_used("validation work", used, budgets.max_validation_work)
            .map_err(MemoryModelErrorV2::static_error)?;
        validate_program_envelope(
            &self.target,
            self.types.len(),
            self.actions.len(),
            budgets,
            &mut work,
        )
        .map_err(MemoryModelErrorV2::static_error)?;
        self.validate_types(budgets, &mut work)
            .map_err(MemoryModelErrorV2::static_error)?;
        self.validate_action_shapes(budgets, &mut work)
            .map_err(MemoryModelErrorV2::static_error)?;
        let bytes = self
            .encode_unchecked(budgets, &mut work)
            .map_err(MemoryModelErrorV2::static_error)?;
        Ok((bytes, work))
    }
    pub fn decode_canonical(
        input: &[u8],
        budgets: MemoryBudgetsV2,
    ) -> Result<Self, MemoryModelErrorV2> {
        Self::decode_canonical_with_work(input, budgets).map(|(program, _work)| program)
    }
    pub fn decode_canonical_with_work(
        input: &[u8],
        budgets: MemoryBudgetsV2,
    ) -> Result<(Self, u64), MemoryModelErrorV2> {
        budgets
            .validate_hard_caps()
            .map_err(MemoryModelErrorV2::static_error)?;
        enforce(
            "canonical bytes",
            input.len() as u64,
            budgets.max_canonical_bytes as u64,
        )
        .map_err(MemoryModelErrorV2::static_error)?;
        let mut reader = ReaderV2::new(input);
        if reader.bytes(8)? != MAGIC {
            return Err(reader.error("bad magic"));
        }
        if reader.u16()? != VERSION || reader.u16()? != 0 {
            return Err(reader.error("unsupported version or flags"));
        }
        let mut work = WorkMeterV2::validation(budgets.max_validation_work);
        work.charge(input.len() as u64)
            .map_err(MemoryModelErrorV2::static_error)?;
        let target = decode_target(&mut reader, budgets, &mut work)?;
        let type_count = reader.count("types", budgets.max_types)?;
        let mut types = decode_collection::<MemoryTypeV2>(
            &reader,
            "types",
            type_count,
            MIN_TYPE_BYTES,
            budgets,
            DecodeCollectionKindV2::Ordinary,
            &mut work,
        )?;
        for _ in 0..type_count {
            types.push(decode_type(&mut reader, budgets, &mut work)?);
        }
        let action_count = reader.count("actions", budgets.max_actions)?;
        let mut actions = decode_collection::<MemoryActionV2>(
            &reader,
            "actions",
            action_count,
            MIN_ACTION_BYTES,
            budgets,
            DecodeCollectionKindV2::Ordinary,
            &mut work,
        )?;
        for _ in 0..action_count {
            actions.push(decode_action(&mut reader, budgets, &mut work)?);
        }
        if !reader.finished() {
            return Err(reader.error("trailing bytes"));
        }
        validate_program_envelope(&target, types.len(), actions.len(), budgets, &mut work)
            .map_err(MemoryModelErrorV2::static_error)?;
        let mut program = Self {
            target,
            types,
            actions,
            admission_validation_work: 0,
        };
        program
            .validate_types(budgets, &mut work)
            .map_err(MemoryModelErrorV2::static_error)?;
        program
            .validate_action_shapes(budgets, &mut work)
            .map_err(MemoryModelErrorV2::static_error)?;
        let reencoded = program
            .encode_unchecked(budgets, &mut work)
            .map_err(MemoryModelErrorV2::static_error)?;
        work.charge(input.len() as u64)
            .map_err(MemoryModelErrorV2::static_error)?;
        if reencoded.as_slice() != input {
            return Err(MemoryModelErrorV2::static_error(
                MemoryErrorReasonV2::NonCanonical,
            ));
        }
        program.admission_validation_work = work.used();
        Ok((program, work.used()))
    }

    fn type_index(
        &self,
        id: MemoryTypeIdV2,
        work: &mut WorkMeterV2,
    ) -> Result<usize, MemoryErrorReasonV2> {
        work.charge(binary_lookup_work(self.types.len() as u64))?;
        self.types
            .binary_search_by_key(&id, |ty| ty.id)
            .map_err(|_| MemoryErrorReasonV2::UnknownType(id))
    }

    fn find_type(
        &self,
        id: MemoryTypeIdV2,
        work: &mut WorkMeterV2,
    ) -> Result<&MemoryTypeV2, MemoryErrorReasonV2> {
        let index = self.type_index(id, work)?;
        Ok(&self.types[index])
    }

    fn validate_types(
        &self,
        budgets: MemoryBudgetsV2,
        work: &mut WorkMeterV2,
    ) -> Result<(), MemoryErrorReasonV2> {
        work.charge(self.types.len() as u64)?;
        for pair in self.types.windows(2) {
            if pair[0].id == pair[1].id {
                return Err(MemoryErrorReasonV2::DuplicateType);
            }
            if pair[0].id > pair[1].id {
                return Err(MemoryErrorReasonV2::NonCanonical);
            }
        }
        let mut edges = 0_u64;
        let mut ranges = 0_u64;
        for ty in &self.types {
            work.charge(1)?;
            if ty.alignment == 0 || !ty.alignment.is_power_of_two() {
                return Err(MemoryErrorReasonV2::InvalidType {
                    ty: ty.id,
                    detail: "alignment must be a nonzero power of two",
                });
            }
            match &ty.kind {
                MemoryTypeKindV2::Scalar {
                    bit_width,
                    validity,
                } => {
                    if *bit_width == 0
                        || *bit_width > 128
                        || u64::from((*bit_width).div_ceil(8)) != ty.size
                    {
                        return Err(MemoryErrorReasonV2::InvalidType {
                            ty: ty.id,
                            detail: "scalar width and size disagree",
                        });
                    }
                    if let BitValidityV2::Ranges(items) = validity {
                        work.charge(items.len() as u64)?;
                        ranges += items.len() as u64;
                    }
                    validate_validity(*bit_width, validity, ty.id)?;
                }
                MemoryTypeKindV2::Array {
                    element,
                    length,
                    stride,
                } => {
                    edges += 1;
                    let element = self.find_type(*element, work)?;
                    if *stride < element.size || *stride % element.alignment != 0 {
                        return Err(MemoryErrorReasonV2::InvalidType {
                            ty: ty.id,
                            detail: "array stride does not contain and align the element",
                        });
                    }
                    let expected = if *length == 0 {
                        0
                    } else {
                        stride
                            .checked_mul(length - 1)
                            .and_then(|n| n.checked_add(element.size))
                            .ok_or(MemoryErrorReasonV2::InvalidType {
                                ty: ty.id,
                                detail: "array layout overflows",
                            })?
                    };
                    if expected > ty.size {
                        return Err(MemoryErrorReasonV2::InvalidType {
                            ty: ty.id,
                            detail: "array elements exceed layout",
                        });
                    }
                }
                MemoryTypeKindV2::Aggregate { fields } => {
                    work.charge(fields.len() as u64)?;
                    edges += fields.len() as u64;
                    let mut last = None;
                    for field in fields {
                        let field_ty = self.find_type(field.ty, work)?;
                        if last.is_some_and(|offset| field.offset < offset) {
                            return Err(MemoryErrorReasonV2::InvalidType {
                                ty: ty.id,
                                detail: "fields are not in offset order",
                            });
                        }
                        let end = field.offset.checked_add(field_ty.size).ok_or(
                            MemoryErrorReasonV2::InvalidType {
                                ty: ty.id,
                                detail: "field layout overflows",
                            },
                        )?;
                        if end > ty.size || field.offset % field_ty.alignment != 0 {
                            return Err(MemoryErrorReasonV2::InvalidType {
                                ty: ty.id,
                                detail: "field is out of bounds or misaligned",
                            });
                        }
                        last = Some(field.offset);
                    }
                }
                MemoryTypeKindV2::OpaqueBytes => {}
            }
        }
        enforce("type edges", edges, budgets.max_type_edges as u64)?;
        enforce(
            "validity ranges",
            ranges,
            budgets.max_validity_ranges as u64,
        )?;
        self.reject_cycles(edges, work)
    }

    fn reject_cycles(&self, edges: u64, work: &mut WorkMeterV2) -> Result<(), MemoryErrorReasonV2> {
        let mut color = fallible_zeroed_bytes("type cycle colors", self.types.len())?;
        work.charge(self.types.len() as u64)?;
        let pending_capacity = u64::try_from(self.types.len())
            .ok()
            .and_then(|types| types.checked_add(edges))
            .and_then(|items| items.checked_add(1))
            .and_then(|items| usize::try_from(items).ok())
            .ok_or(MemoryErrorReasonV2::ResourceLimit {
                resource: "type cycle stack",
                actual: u64::MAX,
                max: u64::from(HARD_MAX_TYPES) + u64::from(HARD_MAX_TYPE_EDGES) + 1,
            })?;
        let mut pending = Vec::<(MemoryTypeIdV2, bool)>::new();
        pending.try_reserve_exact(pending_capacity).map_err(|_| {
            MemoryErrorReasonV2::AllocationFailed {
                resource: "type cycle stack",
            }
        })?;
        for root in 0..self.types.len() {
            if color[root] == 2 {
                continue;
            }
            pending.clear();
            pending.push((self.types[root].id, false));
            while let Some((id, exit)) = pending.pop() {
                work.charge(1)?;
                let index = self.type_index(id, work)?;
                if exit {
                    color[index] = 2;
                    continue;
                }
                match color[index] {
                    1 => return Err(MemoryErrorReasonV2::TypeCycle(id)),
                    2 => continue,
                    _ => {}
                }
                color[index] = 1;
                pending.push((id, true));
                let node = &self.types[index];
                match &node.kind {
                    MemoryTypeKindV2::Array { element, .. } => {
                        work.charge(1)?;
                        pending.push((*element, false));
                    }
                    MemoryTypeKindV2::Aggregate { fields } => {
                        work.charge(fields.len() as u64)?;
                        pending.extend(fields.iter().rev().map(|field| (field.ty, false)));
                    }
                    MemoryTypeKindV2::Scalar { .. } | MemoryTypeKindV2::OpaqueBytes => {}
                }
            }
        }
        Ok(())
    }

    fn validate_action_shapes(
        &self,
        budgets: MemoryBudgetsV2,
        work: &mut WorkMeterV2,
    ) -> Result<(), MemoryErrorReasonV2> {
        for action in &self.actions {
            work.charge(1)?;
            if let Some(place) = action.typed_place() {
                enforce(
                    "place projections",
                    place.projections.len() as u64,
                    budgets.max_projections_per_place as u64,
                )?;
                work.charge(place.projections.len() as u64)?;
            }
        }
        Ok(())
    }
}

fn validate_validity(
    width: u16,
    validity: &BitValidityV2,
    ty: MemoryTypeIdV2,
) -> Result<(), MemoryErrorReasonV2> {
    let max = if width == 128 {
        u128::MAX
    } else {
        (1_u128 << width) - 1
    };
    match validity {
        BitValidityV2::Any => {}
        BitValidityV2::Bool if width == 8 => {}
        BitValidityV2::Char if width == 32 => {}
        BitValidityV2::NonZero => {}
        BitValidityV2::Ranges(ranges) if !ranges.is_empty() => {
            let mut previous = None;
            for range in ranges {
                if range.start > range.end_inclusive
                    || range.end_inclusive > max
                    || previous.is_some_and(|end: u128| range.start <= end.saturating_add(1))
                {
                    return Err(MemoryErrorReasonV2::InvalidType {
                        ty,
                        detail: "validity ranges are noncanonical",
                    });
                }
                previous = Some(range.end_inclusive);
            }
            let duplicates_named_rule = matches!(ranges.as_slice(), [only]
                if (only.start == 0 || only.start == 1) && only.end_inclusive == max
                    || (width == 8 && only.start == 0 && only.end_inclusive == 1))
                || (width == 32
                    && matches!(ranges.as_slice(), [left, right]
                        if left.start == 0
                            && left.end_inclusive == 0xd7ff
                            && right.start == 0xe000
                            && right.end_inclusive == 0x10ffff));
            if duplicates_named_rule {
                return Err(MemoryErrorReasonV2::InvalidType {
                    ty,
                    detail: "validity ranges duplicate a named canonical rule",
                });
            }
        }
        _ => {
            return Err(MemoryErrorReasonV2::InvalidType {
                ty,
                detail: "validity rule is incompatible with scalar width",
            });
        }
    }
    Ok(())
}

fn validate_program_envelope(
    target: &TargetLayoutV2,
    type_count: usize,
    action_count: usize,
    budgets: MemoryBudgetsV2,
    work: &mut WorkMeterV2,
) -> Result<(), MemoryErrorReasonV2> {
    let target_work = u64::try_from(target.architecture.len())
        .ok()
        .and_then(|name| name.checked_add(target.address_spaces.len() as u64))
        .ok_or(MemoryErrorReasonV2::ResourceLimit {
            resource: "validation work",
            actual: u64::MAX,
            max: work.max,
        })?;
    work.charge(target_work)?;
    target.validate()?;
    enforce("types", type_count as u64, budgets.max_types as u64)?;
    enforce("actions", action_count as u64, budgets.max_actions as u64)
}

fn fallible_zeroed_bytes(
    resource: &'static str,
    len: usize,
) -> Result<Vec<u8>, MemoryErrorReasonV2> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(len)
        .map_err(|_| MemoryErrorReasonV2::AllocationFailed { resource })?;
    bytes.resize(len, 0);
    Ok(bytes)
}

fn charge_resource(
    resource: &'static str,
    current: u64,
    amount: u64,
    max: u64,
) -> Result<u64, MemoryErrorReasonV2> {
    let actual = current
        .checked_add(amount)
        .ok_or(MemoryErrorReasonV2::ResourceLimit {
            resource,
            actual: u64::MAX,
            max,
        })?;
    enforce(resource, actual, max)?;
    Ok(actual)
}

fn enforce(resource: &'static str, actual: u64, max: u64) -> Result<(), MemoryErrorReasonV2> {
    if actual > max {
        Err(MemoryErrorReasonV2::ResourceLimit {
            resource,
            actual,
            max,
        })
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MemoryObligationKindV2 {
    AllocationLive,
    ProvenanceGeneration,
    AddressRepresentable,
    InBounds,
    Aligned,
    LifetimeContainsEpoch,
    BorrowAuthorizesAccess,
    NoConflictingAlias,
    Initialized,
    BitValidityCompatible,
    ExplicitRawCapability,
    ExplicitAddressSpaceCastCapability,
    PointerDistanceSameAllocation,
    PointerDistanceElementDivisibility,
    NonOverlappingCopy,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ObligationBasisV2 {
    LocallyEstablished,
    ExplicitCapability,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MemoryObligationV2 {
    obligation_identity: MemoryObligationIdentityV2,
    obligation_index: u32,
    admitted_budgets: MemoryBudgetsV2,
    pub program_identity: UntrustedMemoryProgramIdentityV2,
    pub action_identity: MemoryActionIdentityV2,
    pub action_index: u32,
    pub kind: MemoryObligationKindV2,
    pub allocation: AllocationIdV2,
    pub allocation_generation: u64,
    pub range: ByteRangeV2,
    pub epoch: EpochV2,
    pub basis: ObligationBasisV2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransitionRecordV2 {
    transition_identity: MemoryTransitionIdentityV2,
    admitted_budgets: MemoryBudgetsV2,
    pub program_identity: UntrustedMemoryProgramIdentityV2,
    pub action_identity: MemoryActionIdentityV2,
    pub action_index: u32,
    pub obligations: Vec<MemoryObligationV2>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryExecutionV2 {
    final_epoch: EpochV2,
    records: Vec<TransitionRecordV2>,
    live_allocations: usize,
    program_identity: UntrustedMemoryProgramIdentityV2,
    report_identity: MemoryReportIdentityV2,
    validation_work: u64,
    execution_work: u64,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct UntrustedMemoryProgramIdentityV2([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MemoryActionIdentityV2([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MemoryObligationIdentityV2([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MemoryTransitionIdentityV2([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MemoryReportIdentityV2([u8; 32]);

impl UntrustedMemoryProgramIdentityV2 {
    pub const fn digest(&self) -> &[u8; 32] {
        &self.0
    }
}

impl MemoryActionIdentityV2 {
    pub const fn digest(&self) -> &[u8; 32] {
        &self.0
    }
}

impl MemoryObligationIdentityV2 {
    pub const fn digest(&self) -> &[u8; 32] {
        &self.0
    }
}

impl MemoryTransitionIdentityV2 {
    pub const fn digest(&self) -> &[u8; 32] {
        &self.0
    }
}

impl MemoryReportIdentityV2 {
    pub const fn digest(&self) -> &[u8; 32] {
        &self.0
    }
}

impl MemoryExecutionV2 {
    pub const fn final_epoch(&self) -> EpochV2 {
        self.final_epoch
    }
    pub fn records(&self) -> &[TransitionRecordV2] {
        &self.records
    }
    pub const fn live_allocations(&self) -> usize {
        self.live_allocations
    }
    pub const fn untrusted_program_identity(&self) -> &UntrustedMemoryProgramIdentityV2 {
        &self.program_identity
    }
    pub const fn report_identity(&self) -> &MemoryReportIdentityV2 {
        &self.report_identity
    }
    pub const fn validation_work(&self) -> u64 {
        self.validation_work
    }
    pub const fn execution_work(&self) -> u64 {
        self.execution_work
    }
    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }
    pub const fn grants_proof_authority(&self) -> bool {
        false
    }
    pub const fn proves_compiler_refinement(&self) -> bool {
        false
    }
    pub const fn proves_gpu_behavior(&self) -> bool {
        false
    }
    pub const fn proves_race_freedom(&self) -> bool {
        false
    }

    pub fn verify_identities(
        &self,
        program: &MemoryProgramV2,
        budgets: MemoryBudgetsV2,
    ) -> Result<bool, MemoryModelErrorV2> {
        budgets
            .validate_hard_caps()
            .map_err(MemoryModelErrorV2::static_error)?;
        let (canonical, mut validation_meter) =
            program.canonical_bytes_continuing_from(budgets, program.admission_validation_work)?;
        let program_identity =
            canonical_program_identity_v2(&canonical, budgets, &mut validation_meter)
                .map_err(MemoryModelErrorV2::static_error)?;
        if program_identity != self.program_identity
            || validation_meter.used() != self.validation_work
            || self.records.len() != program.actions.len()
            || self.live_allocations > budgets.max_allocations as usize
        {
            return Ok(false);
        }

        let mut work = WorkMeterV2::execution(budgets.max_execution_work);
        for (index, (record, action)) in self.records.iter().zip(&program.actions).enumerate() {
            work.charge(1).map_err(MemoryModelErrorV2::static_error)?;
            let action_identity =
                canonical_action_identity_v2(program_identity, index, action, budgets, &mut work)
                    .map_err(MemoryModelErrorV2::static_error)?;
            if !verify_transition_identity_v2(
                record,
                program_identity,
                action_identity,
                index as u32,
                budgets,
                &mut work,
            )
            .map_err(MemoryModelErrorV2::static_error)?
            {
                return Ok(false);
            }
        }
        let report_work = report_identity_work_v2(
            program_identity,
            self.validation_work,
            self.final_epoch,
            self.live_allocations,
            self.execution_work,
            &self.records,
        )
        .map_err(MemoryModelErrorV2::static_error)?;
        work.charge(report_work)
            .map_err(MemoryModelErrorV2::static_error)?;
        let report_identity = hash_report_identity_v2(
            program_identity,
            self.validation_work,
            self.final_epoch,
            self.live_allocations,
            self.execution_work,
            &self.records,
        )
        .map_err(MemoryModelErrorV2::static_error)?;
        Ok(report_identity == self.report_identity)
    }
}

impl MemoryObligationV2 {
    pub const fn obligation_identity(&self) -> &MemoryObligationIdentityV2 {
        &self.obligation_identity
    }

    pub const fn obligation_index(&self) -> u32 {
        self.obligation_index
    }

    pub fn verify_identity_in(
        &self,
        record: &TransitionRecordV2,
        budgets: MemoryBudgetsV2,
    ) -> Result<bool, MemoryModelErrorV2> {
        budgets
            .validate_hard_caps()
            .map_err(MemoryModelErrorV2::static_error)?;
        let mut work = WorkMeterV2::execution(budgets.max_execution_work);
        let index = self.obligation_index as usize;
        if self.admitted_budgets != budgets
            || record.admitted_budgets != budgets
            || record.obligations.get(index) != Some(self)
        {
            return Ok(false);
        }
        verify_transition_identity_v2(
            record,
            record.program_identity,
            record.action_identity,
            record.action_index,
            budgets,
            &mut work,
        )
        .map_err(MemoryModelErrorV2::static_error)
    }
}

impl TransitionRecordV2 {
    pub const fn transition_identity(&self) -> &MemoryTransitionIdentityV2 {
        &self.transition_identity
    }

    pub fn verify_identity_for(
        &self,
        program_identity: UntrustedMemoryProgramIdentityV2,
        action: &MemoryActionV2,
        action_index: u32,
        budgets: MemoryBudgetsV2,
    ) -> Result<bool, MemoryModelErrorV2> {
        budgets
            .validate_hard_caps()
            .map_err(MemoryModelErrorV2::static_error)?;
        let mut work = WorkMeterV2::execution(budgets.max_execution_work);
        let action_identity = canonical_action_identity_v2(
            program_identity,
            action_index as usize,
            action,
            budgets,
            &mut work,
        )
        .map_err(MemoryModelErrorV2::static_error)?;
        verify_transition_identity_v2(
            self,
            program_identity,
            action_identity,
            action_index,
            budgets,
            &mut work,
        )
        .map_err(MemoryModelErrorV2::static_error)
    }
}

fn verify_obligation_identity_v2(
    obligation: &MemoryObligationV2,
    record: &TransitionRecordV2,
    obligation_index: u32,
    work: &mut WorkMeterV2,
) -> Result<bool, MemoryErrorReasonV2> {
    if obligation.program_identity != record.program_identity
        || obligation.action_identity != record.action_identity
        || obligation.action_index != record.action_index
        || obligation.obligation_index != obligation_index
        || obligation.admitted_budgets != record.admitted_budgets
    {
        return Ok(false);
    }
    Ok(canonical_obligation_identity_v2(obligation, work)? == obligation.obligation_identity)
}

fn verify_transition_identity_v2(
    record: &TransitionRecordV2,
    program_identity: UntrustedMemoryProgramIdentityV2,
    action_identity: MemoryActionIdentityV2,
    action_index: u32,
    budgets: MemoryBudgetsV2,
    work: &mut WorkMeterV2,
) -> Result<bool, MemoryErrorReasonV2> {
    enforce(
        "actions",
        u64::from(action_index).saturating_add(1),
        u64::from(budgets.max_actions),
    )?;
    enforce(
        "obligations",
        record.obligations.len() as u64,
        u64::from(budgets.max_obligations),
    )?;
    if record.admitted_budgets != budgets
        || record.program_identity != program_identity
        || record.action_identity != action_identity
        || record.action_index != action_index
    {
        return Ok(false);
    }
    for (index, obligation) in record.obligations.iter().enumerate() {
        if !verify_obligation_identity_v2(obligation, record, index as u32, work)? {
            return Ok(false);
        }
    }
    Ok(canonical_transition_identity_v2(record, work)? == record.transition_identity)
}

#[derive(Clone, Debug)]
struct AllocationStateV2 {
    provenance: ProvenanceV2,
    owner: OwnerIdV2,
    address_space: AddressSpaceV2,
    base_address: u64,
    byte_len: u64,
    lifetime: LifetimeRegionV2,
    dead_at: Option<EpochV2>,
    next_borrow_epoch: u64,
    initialized: Vec<ByteRangeV2>,
    typed: Vec<(ByteRangeV2, MemoryTypeIdV2)>,
}

#[derive(Clone, Debug)]
struct LoanStateV2 {
    id: LoanIdV2,
    allocation: AllocationIdV2,
    owner: OwnerIdV2,
    range: ByteRangeV2,
    kind: BorrowKindV2,
    lifetime: LifetimeRegionV2,
    borrow_epoch: u64,
    active: bool,
}

#[derive(Clone, Copy, Debug)]
enum CapabilityKindV2 {
    Raw {
        access: RawAccessV2,
    },
    Cast {
        from: AddressSpaceV2,
        to: AddressSpaceV2,
    },
}

#[derive(Clone, Copy, Debug)]
struct CapabilityStateV2 {
    provenance: ProvenanceV2,
    scope: CapabilityScopeV2,
    range: ByteRangeV2,
    lifetime: LifetimeRegionV2,
    kind: CapabilityKindV2,
}

#[derive(Clone, Debug)]
struct BoundedStateMapV2<K, V> {
    entries: Vec<(K, V)>,
    resource: &'static str,
    max: u32,
}

impl<K: Ord, V> BoundedStateMapV2<K, V> {
    fn try_with_capacity(resource: &'static str, max: u32) -> Result<Self, MemoryErrorReasonV2> {
        let capacity = usize::try_from(max).map_err(|_| MemoryErrorReasonV2::ResourceLimit {
            resource,
            actual: u64::from(max),
            max: usize::MAX as u64,
        })?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(capacity)
            .map_err(|_| MemoryErrorReasonV2::AllocationFailed {
                resource: "runtime state map",
            })?;
        Ok(Self {
            entries,
            resource,
            max,
        })
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn contains_key(&self, key: &K) -> bool {
        self.entries
            .binary_search_by(|(item, _)| item.cmp(key))
            .is_ok()
    }

    fn get(&self, key: &K) -> Option<&V> {
        self.entries
            .binary_search_by(|(item, _)| item.cmp(key))
            .ok()
            .map(|index| &self.entries[index].1)
    }

    fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.entries
            .binary_search_by(|(item, _)| item.cmp(key))
            .ok()
            .map(|index| &mut self.entries[index].1)
    }

    fn insert(&mut self, key: K, value: V) -> Result<bool, MemoryErrorReasonV2> {
        match self.entries.binary_search_by(|(item, _)| item.cmp(&key)) {
            Ok(_) => Ok(false),
            Err(index) => {
                let actual = self.entries.len().checked_add(1).ok_or(
                    MemoryErrorReasonV2::ResourceLimit {
                        resource: self.resource,
                        actual: u64::MAX,
                        max: u64::from(self.max),
                    },
                )?;
                enforce(self.resource, actual as u64, u64::from(self.max))?;
                if self.entries.len() == self.entries.capacity() {
                    return Err(MemoryErrorReasonV2::AllocationFailed {
                        resource: "runtime state map invariant",
                    });
                }
                self.entries.insert(index, (key, value));
                Ok(true)
            }
        }
    }

    fn values(&self) -> impl Iterator<Item = &V> {
        self.entries.iter().map(|(_, value)| value)
    }

    fn retain(&mut self, mut keep: impl FnMut(&K, &V) -> bool) {
        self.entries.retain(|(key, value)| keep(key, value));
    }
}

struct MachineV2<'a> {
    target: &'a TargetLayoutV2,
    types: &'a [MemoryTypeV2],
    epoch: EpochV2,
    allocations: BoundedStateMapV2<AllocationIdV2, AllocationStateV2>,
    loans: BoundedStateMapV2<LoanIdV2, LoanStateV2>,
    capabilities: BoundedStateMapV2<CapabilityIdV2, CapabilityStateV2>,
    records: Vec<TransitionRecordV2>,
    obligation_count: u64,
    program_identity: UntrustedMemoryProgramIdentityV2,
    action_identity: MemoryActionIdentityV2,
    action_index: u32,
    execution_work: WorkMeterV2,
    budgets: MemoryBudgetsV2,
}

#[derive(Clone, Copy)]
struct ResolvedPlaceV2 {
    allocation: AllocationIdV2,
    range: ByteRangeV2,
    ty: MemoryTypeIdV2,
}

pub fn execute_memory_program_v2(
    program: &MemoryProgramV2,
    budgets: MemoryBudgetsV2,
) -> Result<MemoryExecutionV2, MemoryModelErrorV2> {
    budgets
        .validate_hard_caps()
        .map_err(MemoryModelErrorV2::static_error)?;
    let (canonical, mut validation_meter) =
        program.canonical_bytes_continuing_from(budgets, program.admission_validation_work)?;
    enforce(
        "canonical bytes",
        canonical.len() as u64,
        budgets.max_canonical_bytes as u64,
    )
    .map_err(MemoryModelErrorV2::static_error)?;
    let program_identity =
        canonical_program_identity_v2(&canonical, budgets, &mut validation_meter)
            .map_err(MemoryModelErrorV2::static_error)?;
    let validation_work = validation_meter.used();
    let mut records = Vec::new();
    records
        .try_reserve_exact(program.actions.len())
        .map_err(|_| {
            MemoryModelErrorV2::static_error(MemoryErrorReasonV2::AllocationFailed {
                resource: "transition records",
            })
        })?;
    let allocations = BoundedStateMapV2::try_with_capacity("allocations", budgets.max_allocations)
        .map_err(MemoryModelErrorV2::static_error)?;
    let loans = BoundedStateMapV2::try_with_capacity("loans", budgets.max_loans)
        .map_err(MemoryModelErrorV2::static_error)?;
    let capabilities =
        BoundedStateMapV2::try_with_capacity("capabilities", budgets.max_capabilities)
            .map_err(MemoryModelErrorV2::static_error)?;
    let mut initial_execution_work = WorkMeterV2::execution(budgets.max_execution_work);
    initial_execution_work
        .charge(
            program.types.len() as u64
                + program.actions.len() as u64
                + u64::from(budgets.max_allocations)
                + u64::from(budgets.max_loans)
                + u64::from(budgets.max_capabilities),
        )
        .map_err(MemoryModelErrorV2::static_error)?;
    let mut machine = MachineV2 {
        target: &program.target,
        types: &program.types,
        epoch: EpochV2(0),
        allocations,
        loans,
        capabilities,
        records,
        obligation_count: 0,
        program_identity,
        action_identity: MemoryActionIdentityV2([0; 32]),
        action_index: 0,
        execution_work: initial_execution_work,
        budgets,
    };
    for (index, action) in program.actions.iter().enumerate() {
        machine.action_index = index as u32;
        machine
            .charge_work(1)
            .map_err(|reason| MemoryModelErrorV2 {
                action_index: Some(index as u32),
                reason,
            })?;
        machine.action_identity = canonical_action_identity_v2(
            program_identity,
            index,
            action,
            budgets,
            &mut machine.execution_work,
        )
        .map_err(|reason| MemoryModelErrorV2 {
            action_index: Some(index as u32),
            reason,
        })?;
        let mut obligations = machine.apply(action).map_err(|reason| MemoryModelErrorV2 {
            action_index: Some(index as u32),
            reason,
        })?;
        let obligation_lookup_work = (obligations.len() as u64)
            .checked_mul(btree_lookup_work(machine.allocations.len() as u64))
            .ok_or(MemoryModelErrorV2 {
                action_index: Some(index as u32),
                reason: MemoryErrorReasonV2::ResourceLimit {
                    resource: "execution work",
                    actual: u64::MAX,
                    max: budgets.max_execution_work,
                },
            })?;
        machine
            .charge_work(obligation_lookup_work)
            .map_err(|reason| MemoryModelErrorV2 {
                action_index: Some(index as u32),
                reason,
            })?;
        for (obligation_index, obligation) in obligations.iter_mut().enumerate() {
            obligation.obligation_index = obligation_index as u32;
            obligation.obligation_identity =
                canonical_obligation_identity_v2(obligation, &mut machine.execution_work).map_err(
                    |reason| MemoryModelErrorV2 {
                        action_index: Some(index as u32),
                        reason,
                    },
                )?;
        }
        let total = machine
            .obligation_count
            .checked_add(obligations.len() as u64)
            .ok_or(MemoryModelErrorV2 {
                action_index: Some(index as u32),
                reason: MemoryErrorReasonV2::ResourceLimit {
                    resource: "obligations",
                    actual: u64::MAX,
                    max: budgets.max_obligations as u64,
                },
            })?;
        enforce("obligations", total, budgets.max_obligations as u64).map_err(|reason| {
            MemoryModelErrorV2 {
                action_index: Some(index as u32),
                reason,
            }
        })?;
        machine.obligation_count = total;
        let mut record = TransitionRecordV2 {
            transition_identity: MemoryTransitionIdentityV2([0; 32]),
            admitted_budgets: budgets,
            program_identity,
            action_identity: machine.action_identity,
            action_index: index as u32,
            obligations,
        };
        record.transition_identity =
            canonical_transition_identity_v2(&record, &mut machine.execution_work).map_err(
                |reason| MemoryModelErrorV2 {
                    action_index: Some(index as u32),
                    reason,
                },
            )?;
        machine.records.push(record);
    }
    machine
        .charge_work(machine.allocations.len() as u64)
        .map_err(MemoryModelErrorV2::static_error)?;
    let live_allocations = machine
        .allocations
        .values()
        .filter(|allocation| {
            allocation.dead_at.is_none() && allocation.lifetime.contains(machine.epoch)
        })
        .count();
    machine
        .charge_work(machine.records.len() as u64 + machine.obligation_count)
        .map_err(MemoryModelErrorV2::static_error)?;
    let report_identity = canonical_report_identity_v2(
        program_identity,
        validation_work,
        machine.epoch,
        live_allocations,
        machine.execution_work.used(),
        &machine.records,
        &mut machine.execution_work,
    )
    .map_err(MemoryModelErrorV2::static_error)?;
    Ok(MemoryExecutionV2 {
        final_epoch: machine.epoch,
        records: machine.records,
        live_allocations,
        program_identity,
        report_identity,
        validation_work,
        execution_work: machine.execution_work.used(),
    })
}

impl MachineV2<'_> {
    fn charge_work(&mut self, amount: u64) -> Result<(), MemoryErrorReasonV2> {
        self.execution_work.charge(amount)
    }

    fn apply(
        &mut self,
        action: &MemoryActionV2,
    ) -> Result<Vec<MemoryObligationV2>, MemoryErrorReasonV2> {
        match action {
            MemoryActionV2::Allocate {
                allocation,
                generation,
                owner,
                address_space,
                base_address,
                byte_len,
                alignment,
                lifetime,
            } => self.allocate(
                *allocation,
                *generation,
                *owner,
                *address_space,
                *base_address,
                *byte_len,
                *alignment,
                *lifetime,
            ),
            MemoryActionV2::AdvanceEpoch { to } => {
                if to.0 <= self.epoch.0 {
                    return Err(MemoryErrorReasonV2::EpochDidNotAdvance);
                }
                self.epoch = *to;
                Ok(Vec::new())
            }
            MemoryActionV2::BeginBorrow {
                loan,
                owner,
                place,
                kind,
                lifetime,
            } => self.begin_borrow(*loan, *owner, place, *kind, *lifetime),
            MemoryActionV2::EndBorrow { loan, owner } => self.end_borrow(*loan, *owner),
            MemoryActionV2::WriteTyped {
                actor,
                place,
                value,
            } => self.typed_access(*actor, place, Some(*value)),
            MemoryActionV2::ReadTyped { actor, place } => self.typed_access(*actor, place, None),
            MemoryActionV2::GrantRawCapability {
                capability,
                owner,
                provenance,
                scope,
                range,
                access,
                lifetime,
            } => self.grant_capability(
                *capability,
                *owner,
                *provenance,
                *scope,
                *range,
                *lifetime,
                CapabilityKindV2::Raw { access: *access },
            ),
            MemoryActionV2::GrantAddressSpaceCastCapability {
                capability,
                owner,
                provenance,
                scope,
                range,
                from,
                to,
                lifetime,
            } => {
                if from == to {
                    return Err(MemoryErrorReasonV2::InvalidCapability);
                }
                self.grant_capability(
                    *capability,
                    *owner,
                    *provenance,
                    *scope,
                    *range,
                    *lifetime,
                    CapabilityKindV2::Cast {
                        from: *from,
                        to: *to,
                    },
                )
            }
            MemoryActionV2::ReadRaw {
                actor,
                place,
                raw_capability,
                cast_capability,
            } => self.raw_access(*actor, *place, *raw_capability, *cast_capability, false),
            MemoryActionV2::WriteRaw {
                actor,
                place,
                raw_capability,
                cast_capability,
            } => self.raw_access(*actor, *place, *raw_capability, *cast_capability, true),
            MemoryActionV2::PointerDistance {
                actor,
                left,
                right,
                element_size,
                left_capability,
                right_capability,
                left_cast_capability,
                right_cast_capability,
            } => self.pointer_distance(
                *actor,
                *left,
                *right,
                *element_size,
                *left_capability,
                *right_capability,
                *left_cast_capability,
                *right_cast_capability,
            ),
            MemoryActionV2::CopyNonOverlapping {
                actor,
                source,
                destination,
                source_capability,
                destination_capability,
                source_cast_capability,
                destination_cast_capability,
            } => self.copy_nonoverlapping(
                *actor,
                *source,
                *destination,
                *source_capability,
                *destination_capability,
                *source_cast_capability,
                *destination_cast_capability,
            ),
            MemoryActionV2::Deallocate { allocation, owner } => {
                self.deallocate(*allocation, *owner)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn allocate(
        &mut self,
        id: AllocationIdV2,
        generation: u64,
        owner: OwnerIdV2,
        space: AddressSpaceV2,
        base: u64,
        len: u64,
        alignment: u64,
        lifetime: LifetimeRegionV2,
    ) -> Result<Vec<MemoryObligationV2>, MemoryErrorReasonV2> {
        enforce(
            "allocations",
            self.allocations.len() as u64 + 1,
            self.budgets.max_allocations as u64,
        )?;
        self.charge_work(btree_lookup_work(self.allocations.len() as u64))?;
        if self.allocations.contains_key(&id) {
            return Err(MemoryErrorReasonV2::DuplicateAllocation(id));
        }
        if generation == 0
            || alignment == 0
            || !alignment.is_power_of_two()
            || !base.is_multiple_of(alignment)
            || !lifetime.valid()
            || !lifetime.contains(self.epoch)
        {
            return Err(MemoryErrorReasonV2::InvalidAllocation);
        }
        let pointer_bits = self
            .target
            .pointer_bits(space)
            .ok_or(MemoryErrorReasonV2::UnsupportedAddressSpace(space))?;
        let end = base
            .checked_add(len)
            .ok_or(MemoryErrorReasonV2::AddressNotRepresentable)?;
        if pointer_bits < 64 {
            let pointer_max = (1_u64 << pointer_bits) - 1;
            let exclusive_bound = pointer_max + 1;
            if base > pointer_max || end > exclusive_bound {
                return Err(MemoryErrorReasonV2::AddressNotRepresentable);
            }
        }
        let candidate = ByteRangeV2 { start: base, len };
        self.charge_work(self.allocations.len() as u64)?;
        for existing in self.allocations.values() {
            let existing_is_live =
                existing.dead_at.is_none() && existing.lifetime.contains(self.epoch);
            let existing_range = ByteRangeV2 {
                start: existing.base_address,
                len: existing.byte_len,
            };
            let same_alias_domain = self
                .target
                .address_space_semantics(existing.address_space)
                .alias_domain
                == self.target.address_space_semantics(space).alias_domain;
            if existing_is_live && same_alias_domain && existing_range.overlaps(candidate) {
                return Err(MemoryErrorReasonV2::OverlappingLiveAllocation);
            }
        }
        self.charge_work(btree_lookup_work(self.allocations.len() as u64))?;
        self.charge_work(self.allocations.len() as u64)?;
        if !self.allocations.insert(
            id,
            AllocationStateV2 {
                provenance: ProvenanceV2 {
                    allocation: id,
                    generation,
                },
                owner,
                address_space: space,
                base_address: base,
                byte_len: len,
                lifetime,
                dead_at: None,
                next_borrow_epoch: 0,
                initialized: Vec::new(),
                typed: Vec::new(),
            },
        )? {
            return Err(MemoryErrorReasonV2::DuplicateAllocation(id));
        }
        let mut obligations = self.empty_obligations();
        self.push_obligation(
            &mut obligations,
            self.obligation(
                id,
                ByteRangeV2 { start: 0, len },
                MemoryObligationKindV2::AddressRepresentable,
                ObligationBasisV2::LocallyEstablished,
            ),
        )?;
        self.push_obligation(
            &mut obligations,
            self.obligation(
                id,
                ByteRangeV2 { start: 0, len },
                MemoryObligationKindV2::LifetimeContainsEpoch,
                ObligationBasisV2::LocallyEstablished,
            ),
        )?;
        Ok(obligations)
    }

    fn resolve_place(
        &mut self,
        place: &TypedPlaceV2,
    ) -> Result<ResolvedPlaceV2, MemoryErrorReasonV2> {
        let type_lookups = (place.projections.len() as u64)
            .checked_add(1)
            .and_then(|lookups| lookups.checked_mul(binary_lookup_work(self.types.len() as u64)))
            .ok_or(MemoryErrorReasonV2::ResourceLimit {
                resource: "execution work",
                actual: u64::MAX,
                max: self.budgets.max_execution_work,
            })?;
        self.charge_work(place.projections.len() as u64 + type_lookups)?;
        self.charge_work(btree_lookup_work(self.allocations.len() as u64))?;
        let allocation = self.live_allocation(place.provenance)?;
        let mut offset = place.base_offset;
        let mut ty = lookup_type(self.types, place.root_type)
            .ok_or(MemoryErrorReasonV2::UnknownType(place.root_type))?;
        if offset
            .checked_add(ty.size)
            .ok_or(MemoryErrorReasonV2::OutOfBounds)?
            > allocation.byte_len
        {
            return Err(MemoryErrorReasonV2::OutOfBounds);
        }
        if address(allocation, offset)? % ty.alignment != 0 {
            return Err(MemoryErrorReasonV2::Misaligned);
        }
        for projection in &place.projections {
            match (projection, &ty.kind) {
                (ProjectionV2::Field(index), MemoryTypeKindV2::Aggregate { fields }) => {
                    let field = fields
                        .get(*index as usize)
                        .ok_or(MemoryErrorReasonV2::InvalidProjection)?;
                    offset = offset
                        .checked_add(field.offset)
                        .ok_or(MemoryErrorReasonV2::OutOfBounds)?;
                    ty = lookup_type(self.types, field.ty)
                        .ok_or(MemoryErrorReasonV2::UnknownType(field.ty))?;
                }
                (
                    ProjectionV2::Index(index),
                    MemoryTypeKindV2::Array {
                        element,
                        length,
                        stride,
                    },
                ) if index < length => {
                    offset = offset
                        .checked_add(
                            index
                                .checked_mul(*stride)
                                .ok_or(MemoryErrorReasonV2::OutOfBounds)?,
                        )
                        .ok_or(MemoryErrorReasonV2::OutOfBounds)?;
                    ty = lookup_type(self.types, *element)
                        .ok_or(MemoryErrorReasonV2::UnknownType(*element))?;
                }
                _ => return Err(MemoryErrorReasonV2::InvalidProjection),
            }
        }
        let range = ByteRangeV2 {
            start: offset,
            len: ty.size,
        };
        if !(ByteRangeV2 {
            start: 0,
            len: allocation.byte_len,
        })
        .contains(range)
        {
            return Err(MemoryErrorReasonV2::OutOfBounds);
        }
        if address(allocation, offset)? % ty.alignment != 0 {
            return Err(MemoryErrorReasonV2::Misaligned);
        }
        Ok(ResolvedPlaceV2 {
            allocation: place.provenance.allocation,
            range,
            ty: ty.id,
        })
    }

    fn begin_borrow(
        &mut self,
        loan_id: LoanIdV2,
        owner: OwnerIdV2,
        place: &TypedPlaceV2,
        kind: BorrowKindV2,
        lifetime: LifetimeRegionV2,
    ) -> Result<Vec<MemoryObligationV2>, MemoryErrorReasonV2> {
        enforce(
            "loans",
            self.loans.len() as u64 + 1,
            self.budgets.max_loans as u64,
        )?;
        self.charge_work(btree_lookup_work(self.loans.len() as u64))?;
        if self.loans.contains_key(&loan_id) {
            return Err(MemoryErrorReasonV2::DuplicateLoan(loan_id));
        }
        let resolved = self.resolve_place(place)?;
        self.charge_work(btree_lookup_work(self.allocations.len() as u64))?;
        let allocation = self
            .allocations
            .get(&resolved.allocation)
            .ok_or(MemoryErrorReasonV2::UnknownAllocation(resolved.allocation))?;
        if allocation.owner != owner
            || !lifetime.valid()
            || !lifetime.contains(self.epoch)
            || lifetime.start.0 < allocation.lifetime.start.0
            || lifetime.end_inclusive.0 > allocation.lifetime.end_inclusive.0
        {
            return Err(MemoryErrorReasonV2::InvalidLifetimeOrOwner);
        }
        self.ensure_no_alias_conflict(resolved.allocation, resolved.range, kind, None)?;
        self.charge_work(btree_lookup_work(self.allocations.len() as u64))?;
        let allocation = self
            .allocations
            .get_mut(&resolved.allocation)
            .expect("resolved allocation exists");
        allocation.next_borrow_epoch = allocation
            .next_borrow_epoch
            .checked_add(1)
            .ok_or(MemoryErrorReasonV2::BorrowEpochOverflow)?;
        let borrow_epoch = allocation.next_borrow_epoch;
        self.charge_work(btree_lookup_work(self.loans.len() as u64))?;
        self.charge_work(self.loans.len() as u64)?;
        if !self.loans.insert(
            loan_id,
            LoanStateV2 {
                id: loan_id,
                allocation: resolved.allocation,
                owner,
                range: resolved.range,
                kind,
                lifetime,
                borrow_epoch,
                active: true,
            },
        )? {
            return Err(MemoryErrorReasonV2::DuplicateLoan(loan_id));
        }
        self.base_obligations(resolved, true)
    }

    fn end_borrow(
        &mut self,
        loan_id: LoanIdV2,
        owner: OwnerIdV2,
    ) -> Result<Vec<MemoryObligationV2>, MemoryErrorReasonV2> {
        self.charge_work(btree_lookup_work(self.loans.len() as u64))?;
        let loan = self
            .loans
            .get_mut(&loan_id)
            .ok_or(MemoryErrorReasonV2::UnknownLoan(loan_id))?;
        if !loan.active || loan.owner != owner {
            return Err(MemoryErrorReasonV2::StaleBorrow);
        }
        loan.active = false;
        Ok(Vec::new())
    }

    fn typed_access(
        &mut self,
        actor: AccessActorV2,
        place: &TypedPlaceV2,
        write: Option<TypedWriteValueV2>,
    ) -> Result<Vec<MemoryObligationV2>, MemoryErrorReasonV2> {
        let resolved = self.resolve_place(place)?;
        self.authorize_actor(actor, resolved.allocation, resolved.range, write.is_some())?;
        let constrained = type_has_constrained_validity(
            resolved.ty,
            self.types,
            &mut self.execution_work,
            self.budgets.max_type_edges,
        )?;
        if let Some(value) = write {
            self.ensure_mutable(resolved.allocation)?;
            self.charge_work(binary_lookup_work(self.types.len() as u64))?;
            let ty = lookup_type(self.types, resolved.ty).expect("resolved type exists");
            validate_typed_value(ty, value, constrained, &mut self.execution_work)?;
            self.charge_work(btree_lookup_work(self.allocations.len() as u64))?;
            let (initialized_len, typed_len) = {
                let allocation = self
                    .allocations
                    .get(&resolved.allocation)
                    .expect("resolved allocation exists");
                (allocation.initialized.len(), allocation.typed.len())
            };
            self.charge_work(
                2 * initialized_len as u64
                    + sort_work(initialized_len as u64 + 1)
                    + 2 * typed_len as u64
                    + sort_work(typed_len as u64 + 1),
            )?;
            self.charge_work(btree_lookup_work(self.allocations.len() as u64))?;
            let allocation = self
                .allocations
                .get_mut(&resolved.allocation)
                .expect("resolved allocation exists");
            insert_range(
                &mut allocation.initialized,
                resolved.range,
                self.budgets.max_state_ranges,
            )?;
            replace_typed_range(
                &mut allocation.typed,
                resolved.range,
                resolved.ty,
                self.budgets.max_state_ranges,
            )?;
        } else {
            self.charge_work(btree_lookup_work(self.allocations.len() as u64))?;
            let (initialized_len, typed_len) = {
                let allocation = self
                    .allocations
                    .get(&resolved.allocation)
                    .expect("resolved allocation exists");
                (allocation.initialized.len(), allocation.typed.len())
            };
            self.charge_work(initialized_len as u64 + u64::from(constrained) * typed_len as u64)?;
            self.charge_work(btree_lookup_work(self.allocations.len() as u64))?;
            let allocation = self
                .allocations
                .get(&resolved.allocation)
                .expect("resolved allocation exists");
            if !range_set_contains(&allocation.initialized, resolved.range) {
                return Err(MemoryErrorReasonV2::UninitializedRead);
            }
            if constrained && !allocation.typed.contains(&(resolved.range, resolved.ty)) {
                return Err(MemoryErrorReasonV2::IncompatibleBitValidity);
            }
        }
        let mut obligations = self.base_obligations(resolved, true)?;
        self.push_obligation(
            &mut obligations,
            self.obligation(
                resolved.allocation,
                resolved.range,
                MemoryObligationKindV2::BorrowAuthorizesAccess,
                ObligationBasisV2::LocallyEstablished,
            ),
        )?;
        self.push_obligation(
            &mut obligations,
            self.obligation(
                resolved.allocation,
                resolved.range,
                MemoryObligationKindV2::BitValidityCompatible,
                ObligationBasisV2::LocallyEstablished,
            ),
        )?;
        if write.is_none() {
            self.push_obligation(
                &mut obligations,
                self.obligation(
                    resolved.allocation,
                    resolved.range,
                    MemoryObligationKindV2::Initialized,
                    ObligationBasisV2::LocallyEstablished,
                ),
            )?;
        }
        Ok(obligations)
    }

    #[allow(clippy::too_many_arguments)]
    fn grant_capability(
        &mut self,
        id: CapabilityIdV2,
        owner: OwnerIdV2,
        provenance: ProvenanceV2,
        scope: CapabilityScopeV2,
        range: ByteRangeV2,
        lifetime: LifetimeRegionV2,
        kind: CapabilityKindV2,
    ) -> Result<Vec<MemoryObligationV2>, MemoryErrorReasonV2> {
        enforce(
            "capabilities",
            self.capabilities.len() as u64 + 1,
            self.budgets.max_capabilities as u64,
        )?;
        self.charge_work(btree_lookup_work(self.capabilities.len() as u64))?;
        if self.capabilities.contains_key(&id) {
            return Err(MemoryErrorReasonV2::DuplicateCapability(id));
        }
        self.charge_work(btree_lookup_work(self.allocations.len() as u64))?;
        let allocation = self.live_allocation(provenance)?;
        if allocation.owner != owner
            || !(ByteRangeV2 {
                start: 0,
                len: allocation.byte_len,
            })
            .contains(range)
            || !lifetime.valid()
            || !lifetime.contains(self.epoch)
            || lifetime.start.0 < allocation.lifetime.start.0
            || lifetime.end_inclusive.0 > allocation.lifetime.end_inclusive.0
        {
            return Err(MemoryErrorReasonV2::InvalidCapability);
        }
        if matches!(
            kind,
            CapabilityKindV2::Raw {
                access: RawAccessV2::Write | RawAccessV2::ReadWrite
            }
        ) {
            self.ensure_mutable(provenance.allocation)?;
        }
        self.authorize_actor(
            scope.actor(),
            provenance.allocation,
            range,
            matches!(
                kind,
                CapabilityKindV2::Raw {
                    access: RawAccessV2::Write | RawAccessV2::ReadWrite
                }
            ),
        )?;
        self.charge_work(btree_lookup_work(self.capabilities.len() as u64))?;
        self.charge_work(self.capabilities.len() as u64)?;
        if !self.capabilities.insert(
            id,
            CapabilityStateV2 {
                provenance,
                scope,
                range,
                lifetime,
                kind,
            },
        )? {
            return Err(MemoryErrorReasonV2::DuplicateCapability(id));
        }
        let mut obligations = self.empty_obligations();
        self.push_obligation(
            &mut obligations,
            self.obligation(
                provenance.allocation,
                range,
                MemoryObligationKindV2::LifetimeContainsEpoch,
                ObligationBasisV2::LocallyEstablished,
            ),
        )?;
        Ok(obligations)
    }

    fn raw_access(
        &mut self,
        actor: AccessActorV2,
        place: RawPlaceV2,
        raw_id: CapabilityIdV2,
        cast_id: Option<CapabilityIdV2>,
        write: bool,
    ) -> Result<Vec<MemoryObligationV2>, MemoryErrorReasonV2> {
        self.charge_work(btree_lookup_work(self.allocations.len() as u64))?;
        let allocation = self.live_allocation(place.provenance)?;
        let allocation_space = allocation.address_space;
        let range = ByteRangeV2 {
            start: place.byte_offset,
            len: place.byte_len,
        };
        if !(ByteRangeV2 {
            start: 0,
            len: allocation.byte_len,
        })
        .contains(range)
        {
            return Err(MemoryErrorReasonV2::OutOfBounds);
        }
        if place.alignment == 0
            || !place.alignment.is_power_of_two()
            || address(allocation, range.start)? % place.alignment != 0
        {
            return Err(MemoryErrorReasonV2::Misaligned);
        }
        self.ensure_pointer_range_representable(allocation, range, place.pointer_address_space)?;
        self.authorize_actor(actor, place.provenance.allocation, range, write)?;
        self.charge_work(btree_lookup_work(self.capabilities.len() as u64))?;
        let raw = self
            .capabilities
            .get(&raw_id)
            .ok_or(MemoryErrorReasonV2::MissingRawCapability)?;
        if raw.provenance != place.provenance
            || raw.scope.actor() != actor
            || !raw.range.contains(range)
            || !raw.lifetime.contains(self.epoch)
            || !matches!(raw.kind, CapabilityKindV2::Raw { access } if access.permits(write))
        {
            return Err(MemoryErrorReasonV2::InvalidCapability);
        }
        let mut obligations = self.base_raw_obligations(place.provenance.allocation, range)?;
        self.push_obligation(
            &mut obligations,
            self.obligation(
                place.provenance.allocation,
                range,
                MemoryObligationKindV2::ExplicitRawCapability,
                ObligationBasisV2::ExplicitCapability,
            ),
        )?;
        if place.pointer_address_space != allocation_space {
            self.charge_work(btree_lookup_work(self.capabilities.len() as u64))?;
            let cast = self
                .capabilities
                .get(&cast_id.ok_or(MemoryErrorReasonV2::MissingAddressSpaceCastCapability)?)
                .ok_or(MemoryErrorReasonV2::MissingAddressSpaceCastCapability)?;
            if cast.provenance != place.provenance
                || cast.scope.actor() != actor
                || !cast.range.contains(range)
                || !cast.lifetime.contains(self.epoch)
                || !matches!(cast.kind, CapabilityKindV2::Cast { from, to } if from == allocation_space && to == place.pointer_address_space)
            {
                return Err(MemoryErrorReasonV2::InvalidCapability);
            }
            self.push_obligation(
                &mut obligations,
                self.obligation(
                    place.provenance.allocation,
                    range,
                    MemoryObligationKindV2::ExplicitAddressSpaceCastCapability,
                    ObligationBasisV2::ExplicitCapability,
                ),
            )?;
        } else if cast_id.is_some() {
            return Err(MemoryErrorReasonV2::UnexpectedAddressSpaceCastCapability);
        }
        if write {
            self.ensure_mutable(place.provenance.allocation)?;
            self.charge_work(btree_lookup_work(self.allocations.len() as u64))?;
            let (initialized_len, typed_len) = {
                let allocation = self
                    .allocations
                    .get(&place.provenance.allocation)
                    .expect("live allocation exists");
                (allocation.initialized.len(), allocation.typed.len())
            };
            self.charge_work(
                2 * initialized_len as u64
                    + sort_work(initialized_len as u64 + 1)
                    + typed_len as u64,
            )?;
            self.charge_work(btree_lookup_work(self.allocations.len() as u64))?;
            let allocation = self
                .allocations
                .get_mut(&place.provenance.allocation)
                .expect("live allocation exists");
            insert_range(
                &mut allocation.initialized,
                range,
                self.budgets.max_state_ranges,
            )?;
            allocation
                .typed
                .retain(|(typed_range, _)| !typed_range.overlaps(range));
        } else {
            self.charge_work(btree_lookup_work(self.allocations.len() as u64))?;
            let initialized_len = self
                .allocations
                .get(&place.provenance.allocation)
                .expect("live allocation exists")
                .initialized
                .len();
            self.charge_work(initialized_len as u64)?;
            self.charge_work(btree_lookup_work(self.allocations.len() as u64))?;
            let allocation = self
                .allocations
                .get(&place.provenance.allocation)
                .expect("live allocation exists");
            if !range_set_contains(&allocation.initialized, range) {
                return Err(MemoryErrorReasonV2::UninitializedRead);
            }
            self.push_obligation(
                &mut obligations,
                self.obligation(
                    place.provenance.allocation,
                    range,
                    MemoryObligationKindV2::Initialized,
                    ObligationBasisV2::LocallyEstablished,
                ),
            )?;
        }
        Ok(obligations)
    }

    fn ensure_pointer_range_representable(
        &self,
        allocation: &AllocationStateV2,
        range: ByteRangeV2,
        pointer_space: AddressSpaceV2,
    ) -> Result<(), MemoryErrorReasonV2> {
        let pointer_bits = self
            .target
            .pointer_bits(pointer_space)
            .ok_or(MemoryErrorReasonV2::UnsupportedAddressSpace(pointer_space))?;
        let byte_start = allocation
            .base_address
            .checked_add(range.start)
            .ok_or(MemoryErrorReasonV2::AddressNotRepresentable)?;
        if pointer_bits < 64 {
            let pointer_max = (1_u64 << pointer_bits) - 1;
            if byte_start > pointer_max {
                return Err(MemoryErrorReasonV2::AddressNotRepresentable);
            }
            if range.len != 0 {
                let last_byte = byte_start
                    .checked_add(range.len - 1)
                    .ok_or(MemoryErrorReasonV2::AddressNotRepresentable)?;
                if last_byte > pointer_max {
                    return Err(MemoryErrorReasonV2::AddressNotRepresentable);
                }
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn pointer_distance(
        &mut self,
        actor: AccessActorV2,
        left: RawPlaceV2,
        right: RawPlaceV2,
        element_size: u64,
        left_capability: CapabilityIdV2,
        right_capability: CapabilityIdV2,
        left_cast_capability: Option<CapabilityIdV2>,
        right_cast_capability: Option<CapabilityIdV2>,
    ) -> Result<Vec<MemoryObligationV2>, MemoryErrorReasonV2> {
        if element_size == 0
            || left.byte_len != 0
            || right.byte_len != 0
            || left.provenance != right.provenance
        {
            return Err(MemoryErrorReasonV2::InvalidPointerDistance);
        }
        let mut obligations =
            self.raw_access(actor, left, left_capability, left_cast_capability, false)?;
        let right_obligations =
            self.raw_access(actor, right, right_capability, right_cast_capability, false)?;
        self.charge_work(right_obligations.len() as u64)?;
        self.append_obligations(&mut obligations, right_obligations)?;
        let distance = left.byte_offset.abs_diff(right.byte_offset);
        if !distance.is_multiple_of(element_size) {
            return Err(MemoryErrorReasonV2::InvalidPointerDistance);
        }
        let range = ByteRangeV2 {
            start: left.byte_offset.min(right.byte_offset),
            len: distance,
        };
        self.push_obligation(
            &mut obligations,
            self.obligation(
                left.provenance.allocation,
                range,
                MemoryObligationKindV2::PointerDistanceSameAllocation,
                ObligationBasisV2::LocallyEstablished,
            ),
        )?;
        self.push_obligation(
            &mut obligations,
            self.obligation(
                left.provenance.allocation,
                range,
                MemoryObligationKindV2::PointerDistanceElementDivisibility,
                ObligationBasisV2::LocallyEstablished,
            ),
        )?;
        Ok(obligations)
    }

    #[allow(clippy::too_many_arguments)]
    fn copy_nonoverlapping(
        &mut self,
        actor: AccessActorV2,
        source: RawPlaceV2,
        destination: RawPlaceV2,
        source_capability: CapabilityIdV2,
        destination_capability: CapabilityIdV2,
        source_cast_capability: Option<CapabilityIdV2>,
        destination_cast_capability: Option<CapabilityIdV2>,
    ) -> Result<Vec<MemoryObligationV2>, MemoryErrorReasonV2> {
        if source.byte_len != destination.byte_len {
            return Err(MemoryErrorReasonV2::InvalidCopy);
        }
        let source_range = ByteRangeV2 {
            start: source.byte_offset,
            len: source.byte_len,
        };
        let (source_domain, source_physical) = self.raw_physical_range(source)?;
        let (destination_domain, destination_physical) = self.raw_physical_range(destination)?;
        if source_domain == destination_domain && source_physical.overlaps(destination_physical) {
            return Err(MemoryErrorReasonV2::OverlappingCopy);
        }
        let mut obligations = self.raw_access(
            actor,
            source,
            source_capability,
            source_cast_capability,
            false,
        )?;
        let destination_obligations = self.raw_access(
            actor,
            destination,
            destination_capability,
            destination_cast_capability,
            true,
        )?;
        self.charge_work(destination_obligations.len() as u64)?;
        self.append_obligations(&mut obligations, destination_obligations)?;
        self.push_obligation(
            &mut obligations,
            self.obligation(
                source.provenance.allocation,
                source_range,
                MemoryObligationKindV2::NonOverlappingCopy,
                ObligationBasisV2::LocallyEstablished,
            ),
        )?;
        Ok(obligations)
    }

    fn raw_physical_range(
        &mut self,
        place: RawPlaceV2,
    ) -> Result<(PhysicalAliasDomainV2, ByteRangeV2), MemoryErrorReasonV2> {
        self.charge_work(btree_lookup_work(self.allocations.len() as u64))?;
        let allocation = self.live_allocation(place.provenance)?;
        let relative = ByteRangeV2 {
            start: place.byte_offset,
            len: place.byte_len,
        };
        if !(ByteRangeV2 {
            start: 0,
            len: allocation.byte_len,
        })
        .contains(relative)
        {
            return Err(MemoryErrorReasonV2::OutOfBounds);
        }
        Ok((
            self.target
                .address_space_semantics(allocation.address_space)
                .alias_domain,
            ByteRangeV2 {
                start: address(allocation, place.byte_offset)?,
                len: place.byte_len,
            },
        ))
    }

    fn deallocate(
        &mut self,
        id: AllocationIdV2,
        owner: OwnerIdV2,
    ) -> Result<Vec<MemoryObligationV2>, MemoryErrorReasonV2> {
        self.charge_work(btree_lookup_work(self.allocations.len() as u64))?;
        let (dead_at, allocation_owner, lifetime, len) = {
            let allocation = self
                .allocations
                .get(&id)
                .ok_or(MemoryErrorReasonV2::UnknownAllocation(id))?;
            (
                allocation.dead_at,
                allocation.owner,
                allocation.lifetime,
                allocation.byte_len,
            )
        };
        if dead_at.is_some() {
            return Err(MemoryErrorReasonV2::UseAfterFree);
        }
        if allocation_owner != owner {
            return Err(MemoryErrorReasonV2::InvalidLifetimeOrOwner);
        }
        if !lifetime.contains(self.epoch) {
            return Err(MemoryErrorReasonV2::UseAfterFree);
        }
        self.charge_work(self.loans.len() as u64)?;
        if self
            .loans
            .values()
            .any(|loan| loan.active && loan.allocation == id)
        {
            return Err(MemoryErrorReasonV2::ActiveBorrowAtDeallocation);
        }
        self.charge_work(btree_lookup_work(self.allocations.len() as u64))?;
        self.allocations
            .get_mut(&id)
            .expect("allocation exists")
            .dead_at = Some(self.epoch);
        self.charge_work(self.capabilities.len() as u64)?;
        self.capabilities
            .retain(|_, capability| capability.provenance.allocation != id);
        let mut obligations = self.empty_obligations();
        self.push_obligation(
            &mut obligations,
            self.obligation(
                id,
                ByteRangeV2 { start: 0, len },
                MemoryObligationKindV2::AllocationLive,
                ObligationBasisV2::LocallyEstablished,
            ),
        )?;
        Ok(obligations)
    }

    fn live_allocation(
        &self,
        provenance: ProvenanceV2,
    ) -> Result<&AllocationStateV2, MemoryErrorReasonV2> {
        let allocation = self.allocations.get(&provenance.allocation).ok_or(
            MemoryErrorReasonV2::UnknownAllocation(provenance.allocation),
        )?;
        if allocation.dead_at.is_some() || !allocation.lifetime.contains(self.epoch) {
            return Err(MemoryErrorReasonV2::UseAfterFree);
        }
        if allocation.provenance != provenance {
            return Err(MemoryErrorReasonV2::ProvenanceMismatch);
        }
        Ok(allocation)
    }

    fn authorize_actor(
        &mut self,
        actor: AccessActorV2,
        allocation: AllocationIdV2,
        range: ByteRangeV2,
        write: bool,
    ) -> Result<(), MemoryErrorReasonV2> {
        self.charge_work(btree_lookup_work(self.allocations.len() as u64))?;
        let state = self
            .allocations
            .get(&allocation)
            .ok_or(MemoryErrorReasonV2::UnknownAllocation(allocation))?;
        match actor {
            AccessActorV2::Owner(owner) => {
                if owner != state.owner {
                    return Err(MemoryErrorReasonV2::InvalidLifetimeOrOwner);
                }
                self.ensure_no_alias_conflict(
                    allocation,
                    range,
                    if write {
                        BorrowKindV2::Exclusive
                    } else {
                        BorrowKindV2::Shared
                    },
                    None,
                )
            }
            AccessActorV2::Loan { loan, borrow_epoch } => {
                self.charge_work(btree_lookup_work(self.loans.len() as u64))?;
                let loan_state = self
                    .loans
                    .get(&loan)
                    .ok_or(MemoryErrorReasonV2::UnknownLoan(loan))?;
                if !loan_state.active
                    || loan_state.borrow_epoch != borrow_epoch
                    || loan_state.allocation != allocation
                    || !loan_state.lifetime.contains(self.epoch)
                    || !loan_state.range.contains(range)
                    || (write && loan_state.kind != BorrowKindV2::Exclusive)
                {
                    return Err(MemoryErrorReasonV2::StaleBorrow);
                }
                self.ensure_no_alias_conflict(
                    allocation,
                    range,
                    if write {
                        BorrowKindV2::Exclusive
                    } else {
                        BorrowKindV2::Shared
                    },
                    Some(loan),
                )
            }
        }
    }

    fn ensure_mutable(&mut self, allocation: AllocationIdV2) -> Result<(), MemoryErrorReasonV2> {
        self.charge_work(btree_lookup_work(self.allocations.len() as u64))?;
        let allocation = self
            .allocations
            .get(&allocation)
            .ok_or(MemoryErrorReasonV2::UnknownAllocation(allocation))?;
        if self
            .target
            .address_space_semantics(allocation.address_space)
            .mutability
            == MemoryMutabilityV2::ReadOnly
        {
            Err(MemoryErrorReasonV2::ReadOnlyAddressSpace(
                allocation.address_space,
            ))
        } else {
            Ok(())
        }
    }

    fn ensure_no_alias_conflict(
        &mut self,
        allocation: AllocationIdV2,
        range: ByteRangeV2,
        requested: BorrowKindV2,
        except: Option<LoanIdV2>,
    ) -> Result<(), MemoryErrorReasonV2> {
        self.charge_work(self.loans.len() as u64)?;
        let conflict = self.loans.values().any(|loan| {
            loan.active
                && Some(loan.id) != except
                && loan.allocation == allocation
                && loan.range.overlaps(range)
                && (loan.kind == BorrowKindV2::Exclusive || requested == BorrowKindV2::Exclusive)
        });
        if conflict {
            Err(MemoryErrorReasonV2::AliasConflict)
        } else {
            Ok(())
        }
    }

    fn base_obligations(
        &self,
        resolved: ResolvedPlaceV2,
        include_alias: bool,
    ) -> Result<Vec<MemoryObligationV2>, MemoryErrorReasonV2> {
        let mut result = self.base_raw_obligations(resolved.allocation, resolved.range)?;
        self.push_obligation(
            &mut result,
            self.obligation(
                resolved.allocation,
                resolved.range,
                MemoryObligationKindV2::Aligned,
                ObligationBasisV2::LocallyEstablished,
            ),
        )?;
        if include_alias {
            self.push_obligation(
                &mut result,
                self.obligation(
                    resolved.allocation,
                    resolved.range,
                    MemoryObligationKindV2::NoConflictingAlias,
                    ObligationBasisV2::LocallyEstablished,
                ),
            )?;
        }
        Ok(result)
    }

    fn base_raw_obligations(
        &self,
        allocation: AllocationIdV2,
        range: ByteRangeV2,
    ) -> Result<Vec<MemoryObligationV2>, MemoryErrorReasonV2> {
        let mut result = self.empty_obligations();
        for kind in [
            MemoryObligationKindV2::AllocationLive,
            MemoryObligationKindV2::ProvenanceGeneration,
            MemoryObligationKindV2::AddressRepresentable,
            MemoryObligationKindV2::InBounds,
            MemoryObligationKindV2::LifetimeContainsEpoch,
        ] {
            self.push_obligation(
                &mut result,
                self.obligation(
                    allocation,
                    range,
                    kind,
                    ObligationBasisV2::LocallyEstablished,
                ),
            )?;
        }
        Ok(result)
    }

    fn empty_obligations(&self) -> Vec<MemoryObligationV2> {
        Vec::new()
    }

    fn push_obligation(
        &self,
        obligations: &mut Vec<MemoryObligationV2>,
        obligation: MemoryObligationV2,
    ) -> Result<(), MemoryErrorReasonV2> {
        let action_total = (obligations.len() as u64).checked_add(1).ok_or(
            MemoryErrorReasonV2::ResourceLimit {
                resource: "obligations",
                actual: u64::MAX,
                max: u64::from(self.budgets.max_obligations),
            },
        )?;
        let total = self.obligation_count.checked_add(action_total).ok_or(
            MemoryErrorReasonV2::ResourceLimit {
                resource: "obligations",
                actual: u64::MAX,
                max: u64::from(self.budgets.max_obligations),
            },
        )?;
        enforce(
            "obligations",
            total,
            u64::from(self.budgets.max_obligations),
        )?;
        obligations
            .try_reserve_exact(1)
            .map_err(|_| MemoryErrorReasonV2::AllocationFailed {
                resource: "action obligations",
            })?;
        obligations.push(obligation);
        Ok(())
    }

    fn append_obligations(
        &self,
        obligations: &mut Vec<MemoryObligationV2>,
        mut additional: Vec<MemoryObligationV2>,
    ) -> Result<(), MemoryErrorReasonV2> {
        let action_total = obligations
            .len()
            .checked_add(additional.len())
            .and_then(|total| u64::try_from(total).ok())
            .ok_or(MemoryErrorReasonV2::ResourceLimit {
                resource: "obligations",
                actual: u64::MAX,
                max: u64::from(self.budgets.max_obligations),
            })?;
        let total = self.obligation_count.checked_add(action_total).ok_or(
            MemoryErrorReasonV2::ResourceLimit {
                resource: "obligations",
                actual: u64::MAX,
                max: u64::from(self.budgets.max_obligations),
            },
        )?;
        enforce(
            "obligations",
            total,
            u64::from(self.budgets.max_obligations),
        )?;
        obligations
            .try_reserve_exact(additional.len())
            .map_err(|_| MemoryErrorReasonV2::AllocationFailed {
                resource: "action obligations",
            })?;
        obligations.append(&mut additional);
        Ok(())
    }

    fn obligation(
        &self,
        allocation: AllocationIdV2,
        range: ByteRangeV2,
        kind: MemoryObligationKindV2,
        basis: ObligationBasisV2,
    ) -> MemoryObligationV2 {
        let allocation_generation = self
            .allocations
            .get(&allocation)
            .expect("obligations only describe admitted allocations")
            .provenance
            .generation;
        MemoryObligationV2 {
            obligation_identity: MemoryObligationIdentityV2([0; 32]),
            obligation_index: 0,
            admitted_budgets: self.budgets,
            program_identity: self.program_identity,
            action_identity: self.action_identity,
            action_index: self.action_index,
            kind,
            allocation,
            allocation_generation,
            range,
            epoch: self.epoch,
            basis,
        }
    }
}

fn address(allocation: &AllocationStateV2, offset: u64) -> Result<u64, MemoryErrorReasonV2> {
    allocation
        .base_address
        .checked_add(offset)
        .ok_or(MemoryErrorReasonV2::AddressNotRepresentable)
}

fn lookup_type(types: &[MemoryTypeV2], id: MemoryTypeIdV2) -> Option<&MemoryTypeV2> {
    types
        .binary_search_by_key(&id, |ty| ty.id)
        .ok()
        .and_then(|index| types.get(index))
}

fn type_has_constrained_validity(
    root: MemoryTypeIdV2,
    types: &[MemoryTypeV2],
    work: &mut WorkMeterV2,
    max_type_edges: u32,
) -> Result<bool, MemoryErrorReasonV2> {
    let mut pending = Vec::new();
    let capacity = types.len().checked_add(max_type_edges as usize).ok_or(
        MemoryErrorReasonV2::ResourceLimit {
            resource: "validity walk stack",
            actual: u64::MAX,
            max: u64::from(HARD_MAX_TYPES) + u64::from(HARD_MAX_TYPE_EDGES),
        },
    )?;
    pending
        .try_reserve_exact(capacity)
        .map_err(|_| MemoryErrorReasonV2::AllocationFailed {
            resource: "validity walk stack",
        })?;
    pending.push(root);
    let mut seen = fallible_zeroed_bytes("validity walk seen", types.len())?;
    work.charge(types.len() as u64)?;
    while let Some(id) = pending.pop() {
        work.charge(1 + binary_lookup_work(types.len() as u64))?;
        let index = types
            .binary_search_by_key(&id, |ty| ty.id)
            .expect("validated type graph");
        if seen[index] != 0 {
            continue;
        }
        seen[index] = 1;
        match &types[index].kind {
            MemoryTypeKindV2::Scalar { validity, .. } => {
                if *validity != BitValidityV2::Any {
                    return Ok(true);
                }
            }
            MemoryTypeKindV2::Array { element, .. } => pending.push(*element),
            MemoryTypeKindV2::Aggregate { fields } => {
                work.charge(fields.len() as u64)?;
                pending.extend(fields.iter().map(|field| field.ty));
            }
            MemoryTypeKindV2::OpaqueBytes => {}
        }
    }
    Ok(false)
}

fn validate_typed_value(
    ty: &MemoryTypeV2,
    value: TypedWriteValueV2,
    constrained: bool,
    work: &mut WorkMeterV2,
) -> Result<(), MemoryErrorReasonV2> {
    let MemoryTypeKindV2::Scalar {
        bit_width,
        validity,
    } = &ty.kind
    else {
        return if value == TypedWriteValueV2::ValidOpaque && !constrained {
            Ok(())
        } else {
            Err(MemoryErrorReasonV2::InvalidBitPattern)
        };
    };
    let TypedWriteValueV2::KnownBits(bits) = value else {
        return if *validity == BitValidityV2::Any {
            Ok(())
        } else {
            Err(MemoryErrorReasonV2::InvalidBitPattern)
        };
    };
    if *bit_width < 128 && bits >= (1_u128 << *bit_width) {
        return Err(MemoryErrorReasonV2::InvalidBitPattern);
    }
    let valid = match validity {
        BitValidityV2::Any => true,
        BitValidityV2::Bool => bits <= 1,
        BitValidityV2::Char => bits <= 0x10ffff && !(0xd800..=0xdfff).contains(&(bits as u32)),
        BitValidityV2::NonZero => bits != 0,
        BitValidityV2::Ranges(ranges) => {
            work.charge(ranges.len() as u64)?;
            ranges
                .iter()
                .any(|range| range.start <= bits && bits <= range.end_inclusive)
        }
    };
    if valid {
        Ok(())
    } else {
        Err(MemoryErrorReasonV2::InvalidBitPattern)
    }
}

fn insert_range(
    ranges: &mut Vec<ByteRangeV2>,
    range: ByteRangeV2,
    max: u32,
) -> Result<(), MemoryErrorReasonV2> {
    if range.len == 0 {
        return Ok(());
    }
    let mut start = range.start;
    let mut end = range.end().ok_or(MemoryErrorReasonV2::OutOfBounds)?;
    let mut retained = 0_usize;
    for existing in ranges.iter() {
        let existing_end = existing.end().expect("validated state range");
        if existing_end < start || end < existing.start {
            retained += 1;
        } else {
            start = start.min(existing.start);
            end = end.max(existing_end);
        }
    }
    let final_len = retained
        .checked_add(1)
        .ok_or(MemoryErrorReasonV2::ResourceLimit {
            resource: "state ranges",
            actual: u64::MAX,
            max: u64::from(max),
        })?;
    enforce("state ranges", final_len as u64, u64::from(max))?;
    ranges
        .try_reserve_exact(final_len.saturating_sub(ranges.len()))
        .map_err(|_| MemoryErrorReasonV2::AllocationFailed {
            resource: "initialized state ranges",
        })?;
    ranges.retain(|existing| {
        let existing_end = existing.end().expect("validated state range");
        if existing_end < start || end < existing.start {
            true
        } else {
            start = start.min(existing.start);
            end = end.max(existing_end);
            false
        }
    });
    ranges.push(ByteRangeV2 {
        start,
        len: end - start,
    });
    ranges.sort_unstable();
    Ok(())
}

fn replace_typed_range(
    ranges: &mut Vec<(ByteRangeV2, MemoryTypeIdV2)>,
    range: ByteRangeV2,
    ty: MemoryTypeIdV2,
    max: u32,
) -> Result<(), MemoryErrorReasonV2> {
    let retained = ranges
        .iter()
        .filter(|(existing, _)| !existing.overlaps(range))
        .count();
    let final_len = retained
        .checked_add(1)
        .ok_or(MemoryErrorReasonV2::ResourceLimit {
            resource: "typed state ranges",
            actual: u64::MAX,
            max: u64::from(max),
        })?;
    enforce("typed state ranges", final_len as u64, u64::from(max))?;
    ranges
        .try_reserve_exact(final_len.saturating_sub(ranges.len()))
        .map_err(|_| MemoryErrorReasonV2::AllocationFailed {
            resource: "typed state ranges",
        })?;
    ranges.retain(|(existing, _)| !existing.overlaps(range));
    ranges.push((range, ty));
    ranges.sort_unstable();
    Ok(())
}

fn sort_work(items: u64) -> u64 {
    if items < 2 {
        return items;
    }
    let levels = u64::from(u64::BITS - (items - 1).leading_zeros());
    items.saturating_mul(levels)
}

fn binary_lookup_work(items: u64) -> u64 {
    if items == 0 {
        1
    } else {
        u64::from(u64::BITS - (items - 1).leading_zeros()) + 1
    }
}

fn btree_lookup_work(items: u64) -> u64 {
    binary_lookup_work(items).saturating_mul(2)
}

fn range_set_contains(ranges: &[ByteRangeV2], range: ByteRangeV2) -> bool {
    range.len == 0 || ranges.iter().any(|initialized| initialized.contains(range))
}

fn canonical_program_identity_v2(
    canonical_program: &[u8],
    budgets: MemoryBudgetsV2,
    work: &mut WorkMeterV2,
) -> Result<UntrustedMemoryProgramIdentityV2, MemoryErrorReasonV2> {
    let mut digest = MeteredSha256V2::new(work);
    identity_bytes(&mut digest, PROGRAM_IDENTITY_DOMAIN)?;
    identity_u16(&mut digest, VERSION)?;
    identity_u32(&mut digest, budgets.max_types)?;
    identity_u32(&mut digest, budgets.max_type_edges)?;
    identity_u32(&mut digest, budgets.max_validity_ranges)?;
    identity_u32(&mut digest, budgets.max_actions)?;
    identity_u32(&mut digest, budgets.max_projections_per_place)?;
    identity_u32(&mut digest, budgets.max_allocations)?;
    identity_u32(&mut digest, budgets.max_loans)?;
    identity_u32(&mut digest, budgets.max_capabilities)?;
    identity_u32(&mut digest, budgets.max_state_ranges)?;
    identity_u32(&mut digest, budgets.max_obligations)?;
    identity_u32(&mut digest, budgets.max_canonical_bytes)?;
    identity_u64(&mut digest, budgets.max_validation_work)?;
    identity_u64(&mut digest, budgets.max_execution_work)?;
    identity_bytes(&mut digest, canonical_program)?;
    Ok(UntrustedMemoryProgramIdentityV2(digest.finalize()?))
}

fn canonical_action_identity_v2(
    program_identity: UntrustedMemoryProgramIdentityV2,
    index: usize,
    action: &MemoryActionV2,
    budgets: MemoryBudgetsV2,
    work: &mut WorkMeterV2,
) -> Result<MemoryActionIdentityV2, MemoryErrorReasonV2> {
    let canonical_len = preflight_action_identity_v2(index, action, budgets, work)?;
    let mut digest = MeteredSha256V2::new(work);
    identity_bytes(&mut digest, ACTION_IDENTITY_DOMAIN)?;
    identity_bytes(&mut digest, program_identity.digest())?;
    identity_budget_fields_v2(&mut digest, budgets)?;
    identity_u64(&mut digest, index as u64)?;
    identity_u64(&mut digest, canonical_len)?;
    let mut writer = DigestWriterV2::new(&mut digest, budgets.max_canonical_bytes);
    encode_action(&mut writer, action)?;
    debug_assert_eq!(writer.written(), canonical_len);
    Ok(MemoryActionIdentityV2(digest.finalize()?))
}

fn preflight_action_identity_v2(
    index: usize,
    action: &MemoryActionV2,
    budgets: MemoryBudgetsV2,
    work: &mut WorkMeterV2,
) -> Result<u64, MemoryErrorReasonV2> {
    let action_count = index
        .checked_add(1)
        .ok_or(MemoryErrorReasonV2::ResourceLimit {
            resource: "actions",
            actual: u64::MAX,
            max: u64::from(budgets.max_actions),
        })?;
    enforce(
        "actions",
        action_count as u64,
        u64::from(budgets.max_actions),
    )?;
    if let Some(place) = action.typed_place() {
        enforce(
            "place projections",
            place.projections.len() as u64,
            u64::from(budgets.max_projections_per_place),
        )?;
    }
    work.charge(1)?;
    if let Some(place) = action.typed_place() {
        work.charge(place.projections.len() as u64)?;
    }
    let mut counter = CountingWriterV2::new(budgets.max_canonical_bytes);
    encode_action(&mut counter, action)?;
    Ok(counter.written())
}

fn canonical_obligation_identity_v2(
    obligation: &MemoryObligationV2,
    work: &mut WorkMeterV2,
) -> Result<MemoryObligationIdentityV2, MemoryErrorReasonV2> {
    let mut digest = MeteredSha256V2::new(work);
    identity_bytes(&mut digest, OBLIGATION_IDENTITY_DOMAIN)?;
    identity_obligation_fields(&mut digest, obligation)?;
    Ok(MemoryObligationIdentityV2(digest.finalize()?))
}

fn canonical_transition_identity_v2(
    record: &TransitionRecordV2,
    work: &mut WorkMeterV2,
) -> Result<MemoryTransitionIdentityV2, MemoryErrorReasonV2> {
    let mut digest = MeteredSha256V2::new(work);
    identity_bytes(&mut digest, TRANSITION_IDENTITY_DOMAIN)?;
    identity_transition_fields(&mut digest, record)?;
    Ok(MemoryTransitionIdentityV2(digest.finalize()?))
}

fn canonical_report_identity_v2(
    program_identity: UntrustedMemoryProgramIdentityV2,
    validation_work: u64,
    final_epoch: EpochV2,
    live_allocations: usize,
    execution_work: u64,
    records: &[TransitionRecordV2],
    work: &mut WorkMeterV2,
) -> Result<MemoryReportIdentityV2, MemoryErrorReasonV2> {
    let hash_work = report_identity_work_v2(
        program_identity,
        validation_work,
        final_epoch,
        live_allocations,
        execution_work,
        records,
    )?;
    if work.used() != execution_work {
        return Err(MemoryErrorReasonV2::IdentityMismatch {
            resource: "report work preimage",
        });
    }
    work.charge(hash_work)?;
    hash_report_identity_v2(
        program_identity,
        validation_work,
        final_epoch,
        live_allocations,
        work.used(),
        records,
    )
}

fn report_identity_work_v2(
    _program_identity: UntrustedMemoryProgramIdentityV2,
    _validation_work: u64,
    _final_epoch: EpochV2,
    _live_allocations: usize,
    _execution_work: u64,
    records: &[TransitionRecordV2],
) -> Result<u64, MemoryErrorReasonV2> {
    const REPORT_FIXED_BYTES: u64 = 8 + REPORT_IDENTITY_DOMAIN.len() as u64 + 40 + 8 + 32;
    const TRANSITION_FIXED_BYTES: u64 = 40 + 40 + 40 + 4 + 8;
    const OBLIGATION_BYTES: u64 = 40 + 4 + 40 + 40 + 4 + 1 + 4 + 8 + 8 + 8 + 8 + 1;
    let mut message_bytes = REPORT_FIXED_BYTES;
    for record in records {
        message_bytes = message_bytes.checked_add(TRANSITION_FIXED_BYTES).ok_or(
            MemoryErrorReasonV2::ResourceLimit {
                resource: "identity work",
                actual: u64::MAX,
                max: HARD_MAX_EXECUTION_WORK,
            },
        )?;
        message_bytes = message_bytes
            .checked_add(
                (record.obligations.len() as u64)
                    .checked_mul(OBLIGATION_BYTES)
                    .ok_or(MemoryErrorReasonV2::ResourceLimit {
                        resource: "identity work",
                        actual: u64::MAX,
                        max: HARD_MAX_EXECUTION_WORK,
                    })?,
            )
            .ok_or(MemoryErrorReasonV2::ResourceLimit {
                resource: "identity work",
                actual: u64::MAX,
                max: HARD_MAX_EXECUTION_WORK,
            })?;
    }
    identity_hash_work_v2(message_bytes)?
        .checked_add(records.len() as u64)
        .ok_or(MemoryErrorReasonV2::ResourceLimit {
            resource: "identity work",
            actual: u64::MAX,
            max: HARD_MAX_EXECUTION_WORK,
        })
}

fn hash_report_identity_v2(
    program_identity: UntrustedMemoryProgramIdentityV2,
    validation_work: u64,
    final_epoch: EpochV2,
    live_allocations: usize,
    execution_work: u64,
    records: &[TransitionRecordV2],
) -> Result<MemoryReportIdentityV2, MemoryErrorReasonV2> {
    let mut digest = UnmeteredSha256V2::new();
    identity_bytes(&mut digest, REPORT_IDENTITY_DOMAIN)?;
    identity_bytes(&mut digest, program_identity.digest())?;
    identity_u64(&mut digest, validation_work)?;
    identity_u64(&mut digest, final_epoch.0)?;
    identity_u64(&mut digest, live_allocations as u64)?;
    identity_u64(&mut digest, execution_work)?;
    identity_u64(&mut digest, records.len() as u64)?;
    for record in records {
        identity_bytes(&mut digest, record.transition_identity.digest())?;
        identity_transition_fields(&mut digest, record)?;
    }
    Ok(MemoryReportIdentityV2(digest.finalize()))
}

fn identity_obligation_fields<D: IdentitySinkV2>(
    digest: &mut D,
    obligation: &MemoryObligationV2,
) -> Result<(), MemoryErrorReasonV2> {
    identity_budget_fields_v2(digest, obligation.admitted_budgets)?;
    identity_u32(digest, obligation.obligation_index)?;
    identity_bytes(digest, obligation.program_identity.digest())?;
    identity_bytes(digest, obligation.action_identity.digest())?;
    identity_u32(digest, obligation.action_index)?;
    identity_u8(digest, obligation_kind_tag(obligation.kind))?;
    identity_u32(digest, obligation.allocation.get())?;
    identity_u64(digest, obligation.allocation_generation)?;
    identity_u64(digest, obligation.range.start)?;
    identity_u64(digest, obligation.range.len)?;
    identity_u64(digest, obligation.epoch.0)?;
    identity_u8(digest, obligation_basis_tag(obligation.basis))
}

fn identity_transition_fields<D: IdentitySinkV2>(
    digest: &mut D,
    record: &TransitionRecordV2,
) -> Result<(), MemoryErrorReasonV2> {
    identity_budget_fields_v2(digest, record.admitted_budgets)?;
    identity_bytes(digest, record.program_identity.digest())?;
    identity_bytes(digest, record.action_identity.digest())?;
    identity_u32(digest, record.action_index)?;
    identity_u64(digest, record.obligations.len() as u64)?;
    for obligation in &record.obligations {
        identity_bytes(digest, obligation.obligation_identity.digest())?;
        identity_obligation_fields(digest, obligation)?;
    }
    Ok(())
}

fn identity_budget_fields_v2<D: IdentitySinkV2>(
    digest: &mut D,
    budgets: MemoryBudgetsV2,
) -> Result<(), MemoryErrorReasonV2> {
    identity_u32(digest, budgets.max_types)?;
    identity_u32(digest, budgets.max_type_edges)?;
    identity_u32(digest, budgets.max_validity_ranges)?;
    identity_u32(digest, budgets.max_actions)?;
    identity_u32(digest, budgets.max_projections_per_place)?;
    identity_u32(digest, budgets.max_allocations)?;
    identity_u32(digest, budgets.max_loans)?;
    identity_u32(digest, budgets.max_capabilities)?;
    identity_u32(digest, budgets.max_state_ranges)?;
    identity_u32(digest, budgets.max_obligations)?;
    identity_u32(digest, budgets.max_canonical_bytes)?;
    identity_u64(digest, budgets.max_validation_work)?;
    identity_u64(digest, budgets.max_execution_work)
}

const fn obligation_kind_tag(kind: MemoryObligationKindV2) -> u8 {
    match kind {
        MemoryObligationKindV2::AllocationLive => 1,
        MemoryObligationKindV2::ProvenanceGeneration => 2,
        MemoryObligationKindV2::AddressRepresentable => 3,
        MemoryObligationKindV2::InBounds => 4,
        MemoryObligationKindV2::Aligned => 5,
        MemoryObligationKindV2::LifetimeContainsEpoch => 6,
        MemoryObligationKindV2::BorrowAuthorizesAccess => 7,
        MemoryObligationKindV2::NoConflictingAlias => 8,
        MemoryObligationKindV2::Initialized => 9,
        MemoryObligationKindV2::BitValidityCompatible => 10,
        MemoryObligationKindV2::ExplicitRawCapability => 11,
        MemoryObligationKindV2::ExplicitAddressSpaceCastCapability => 12,
        MemoryObligationKindV2::PointerDistanceSameAllocation => 13,
        MemoryObligationKindV2::PointerDistanceElementDivisibility => 14,
        MemoryObligationKindV2::NonOverlappingCopy => 15,
    }
}

const fn obligation_basis_tag(basis: ObligationBasisV2) -> u8 {
    match basis {
        ObligationBasisV2::LocallyEstablished => 1,
        ObligationBasisV2::ExplicitCapability => 2,
    }
}

trait IdentitySinkV2 {
    fn update(&mut self, bytes: &[u8]) -> Result<(), MemoryErrorReasonV2>;
}

fn identity_hash_work_v2(message_bytes: u64) -> Result<u64, MemoryErrorReasonV2> {
    let blocks = message_bytes
        .checked_add(9)
        .map(|bytes| bytes.div_ceil(64))
        .ok_or(MemoryErrorReasonV2::ResourceLimit {
            resource: "identity work",
            actual: u64::MAX,
            max: HARD_MAX_EXECUTION_WORK,
        })?;
    message_bytes
        .checked_add(
            blocks
                .checked_mul(64)
                .ok_or(MemoryErrorReasonV2::ResourceLimit {
                    resource: "identity work",
                    actual: u64::MAX,
                    max: HARD_MAX_EXECUTION_WORK,
                })?,
        )
        .ok_or(MemoryErrorReasonV2::ResourceLimit {
            resource: "identity work",
            actual: u64::MAX,
            max: HARD_MAX_EXECUTION_WORK,
        })
}

struct MeteredSha256V2<'a> {
    digest: Sha256V2,
    work: &'a mut WorkMeterV2,
    message_bytes: u64,
}

impl<'a> MeteredSha256V2<'a> {
    fn new(work: &'a mut WorkMeterV2) -> Self {
        Self {
            digest: Sha256V2::new(),
            work,
            message_bytes: 0,
        }
    }

    fn finalize(self) -> Result<[u8; 32], MemoryErrorReasonV2> {
        let blocks = self
            .message_bytes
            .checked_add(9)
            .and_then(|bytes| bytes.checked_add(63))
            .map(|bytes| bytes / 64)
            .ok_or(MemoryErrorReasonV2::ResourceLimit {
                resource: self.work.resource,
                actual: u64::MAX,
                max: self.work.max,
            })?;
        self.work.charge(
            blocks
                .checked_mul(64)
                .ok_or(MemoryErrorReasonV2::ResourceLimit {
                    resource: self.work.resource,
                    actual: u64::MAX,
                    max: self.work.max,
                })?,
        )?;
        Ok(self.digest.finalize())
    }
}

impl IdentitySinkV2 for MeteredSha256V2<'_> {
    fn update(&mut self, bytes: &[u8]) -> Result<(), MemoryErrorReasonV2> {
        self.work.charge(bytes.len() as u64)?;
        self.message_bytes = self.message_bytes.checked_add(bytes.len() as u64).ok_or(
            MemoryErrorReasonV2::ResourceLimit {
                resource: self.work.resource,
                actual: u64::MAX,
                max: self.work.max,
            },
        )?;
        self.digest.update(bytes);
        Ok(())
    }
}

struct UnmeteredSha256V2(Sha256V2);

impl UnmeteredSha256V2 {
    const fn new() -> Self {
        Self(Sha256V2::new())
    }

    fn finalize(self) -> [u8; 32] {
        self.0.finalize()
    }
}

impl IdentitySinkV2 for UnmeteredSha256V2 {
    fn update(&mut self, bytes: &[u8]) -> Result<(), MemoryErrorReasonV2> {
        self.0.update(bytes);
        Ok(())
    }
}

fn identity_bytes<D: IdentitySinkV2>(
    digest: &mut D,
    bytes: &[u8],
) -> Result<(), MemoryErrorReasonV2> {
    identity_u64(digest, bytes.len() as u64)?;
    digest.update(bytes)
}

fn identity_u8<D: IdentitySinkV2>(digest: &mut D, value: u8) -> Result<(), MemoryErrorReasonV2> {
    digest.update(&[value])
}

fn identity_u16<D: IdentitySinkV2>(digest: &mut D, value: u16) -> Result<(), MemoryErrorReasonV2> {
    digest.update(&value.to_le_bytes())
}

fn identity_u32<D: IdentitySinkV2>(digest: &mut D, value: u32) -> Result<(), MemoryErrorReasonV2> {
    digest.update(&value.to_le_bytes())
}

fn identity_u64<D: IdentitySinkV2>(digest: &mut D, value: u64) -> Result<(), MemoryErrorReasonV2> {
    digest.update(&value.to_le_bytes())
}

struct Sha256V2 {
    state: [u32; 8],
    buffer: [u8; 64],
    buffer_len: usize,
    message_len: u64,
}

impl Sha256V2 {
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
            .expect("hard-bounded memory identity length");
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
            let block: &[u8; 64] = bytes[..64].try_into().expect("exact block");
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
            .expect("hard-bounded memory identity bit length");
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
            words[index] = u32::from_be_bytes(chunk.try_into().expect("four-byte word"));
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
pub fn sha256_test_vector_v2(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256V2::new();
    digest.update(bytes);
    digest.finalize()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryModelErrorV2 {
    pub action_index: Option<u32>,
    pub reason: MemoryErrorReasonV2,
}
impl MemoryModelErrorV2 {
    fn static_error(reason: MemoryErrorReasonV2) -> Self {
        Self {
            action_index: None,
            reason,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemoryErrorReasonV2 {
    UnsupportedTargetLayout,
    UnsupportedAddressSpace(AddressSpaceV2),
    ResourceLimit {
        resource: &'static str,
        actual: u64,
        max: u64,
    },
    AllocationFailed {
        resource: &'static str,
    },
    IdentityMismatch {
        resource: &'static str,
    },
    DuplicateType,
    UnknownType(MemoryTypeIdV2),
    InvalidType {
        ty: MemoryTypeIdV2,
        detail: &'static str,
    },
    TypeCycle(MemoryTypeIdV2),
    DuplicateAllocation(AllocationIdV2),
    OverlappingLiveAllocation,
    UnknownAllocation(AllocationIdV2),
    InvalidAllocation,
    AddressNotRepresentable,
    UseAfterFree,
    ProvenanceMismatch,
    OutOfBounds,
    Misaligned,
    InvalidProjection,
    DuplicateLoan(LoanIdV2),
    UnknownLoan(LoanIdV2),
    AliasConflict,
    StaleBorrow,
    BorrowEpochOverflow,
    ActiveBorrowAtDeallocation,
    InvalidLifetimeOrOwner,
    UninitializedRead,
    InvalidBitPattern,
    IncompatibleBitValidity,
    DuplicateCapability(CapabilityIdV2),
    MissingRawCapability,
    MissingAddressSpaceCastCapability,
    UnexpectedAddressSpaceCastCapability,
    InvalidCapability,
    InvalidPointerDistance,
    InvalidCopy,
    OverlappingCopy,
    ReadOnlyAddressSpace(AddressSpaceV2),
    EpochDidNotAdvance,
    Decode {
        offset: usize,
        detail: &'static str,
    },
    NonCanonical,
}

impl fmt::Display for MemoryModelErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.action_index {
            Some(index) => write!(f, "memory V2 action {index} failed: {:?}", self.reason),
            None => write!(f, "memory V2 program rejected: {:?}", self.reason),
        }
    }
}
impl std::error::Error for MemoryModelErrorV2 {}

impl MemoryProgramV2 {
    fn encode_unchecked(
        &self,
        budgets: MemoryBudgetsV2,
        work: &mut WorkMeterV2,
    ) -> Result<Vec<u8>, MemoryErrorReasonV2> {
        let mut writer = WriterV2::new(budgets.max_canonical_bytes);
        writer.bytes(&MAGIC)?;
        writer.u16(VERSION)?;
        writer.u16(0)?;
        encode_target(&mut writer, &self.target)?;
        writer.u32(self.types.len() as u32)?;
        for ty in &self.types {
            encode_type(&mut writer, ty)?;
        }
        writer.u32(self.actions.len() as u32)?;
        for action in &self.actions {
            encode_action(&mut writer, action)?;
        }
        let bytes = writer.finish();
        work.charge(bytes.len() as u64)?;
        Ok(bytes)
    }
}

struct WriterV2 {
    bytes: Vec<u8>,
    max: u32,
}
impl WriterV2 {
    fn new(max: u32) -> Self {
        Self {
            bytes: Vec::new(),
            max,
        }
    }
    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

trait CanonicalWriterV2 {
    fn append(&mut self, bytes: &[u8]) -> Result<(), MemoryErrorReasonV2>;

    fn bytes(&mut self, value: &[u8]) -> Result<(), MemoryErrorReasonV2> {
        self.append(value)
    }
    fn u8(&mut self, value: u8) -> Result<(), MemoryErrorReasonV2> {
        self.append(&[value])
    }
    fn u16(&mut self, value: u16) -> Result<(), MemoryErrorReasonV2> {
        self.append(&value.to_le_bytes())
    }
    fn u32(&mut self, value: u32) -> Result<(), MemoryErrorReasonV2> {
        self.append(&value.to_le_bytes())
    }
    fn u64(&mut self, value: u64) -> Result<(), MemoryErrorReasonV2> {
        self.append(&value.to_le_bytes())
    }
    fn u128(&mut self, value: u128) -> Result<(), MemoryErrorReasonV2> {
        self.append(&value.to_le_bytes())
    }
}

impl CanonicalWriterV2 for WriterV2 {
    fn append(&mut self, bytes: &[u8]) -> Result<(), MemoryErrorReasonV2> {
        let actual = self.bytes.len().checked_add(bytes.len()).ok_or(
            MemoryErrorReasonV2::ResourceLimit {
                resource: "canonical bytes",
                actual: u64::MAX,
                max: self.max as u64,
            },
        )?;
        enforce("canonical bytes", actual as u64, self.max as u64)?;
        self.bytes
            .try_reserve(bytes.len())
            .map_err(|_| MemoryErrorReasonV2::AllocationFailed {
                resource: "canonical bytes",
            })?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
}

struct CountingWriterV2 {
    written: u64,
    max: u32,
}

impl CountingWriterV2 {
    const fn new(max: u32) -> Self {
        Self { written: 0, max }
    }

    const fn written(&self) -> u64 {
        self.written
    }
}

impl CanonicalWriterV2 for CountingWriterV2 {
    fn append(&mut self, bytes: &[u8]) -> Result<(), MemoryErrorReasonV2> {
        let actual = self.written.checked_add(bytes.len() as u64).ok_or(
            MemoryErrorReasonV2::ResourceLimit {
                resource: "canonical bytes",
                actual: u64::MAX,
                max: u64::from(self.max),
            },
        )?;
        enforce("canonical bytes", actual, u64::from(self.max))?;
        self.written = actual;
        Ok(())
    }
}

struct DigestWriterV2<'a, 'work> {
    digest: &'a mut MeteredSha256V2<'work>,
    written: u64,
    max: u32,
}

impl<'a, 'work> DigestWriterV2<'a, 'work> {
    const fn new(digest: &'a mut MeteredSha256V2<'work>, max: u32) -> Self {
        Self {
            digest,
            written: 0,
            max,
        }
    }

    const fn written(&self) -> u64 {
        self.written
    }
}

impl CanonicalWriterV2 for DigestWriterV2<'_, '_> {
    fn append(&mut self, bytes: &[u8]) -> Result<(), MemoryErrorReasonV2> {
        let actual = self.written.checked_add(bytes.len() as u64).ok_or(
            MemoryErrorReasonV2::ResourceLimit {
                resource: "canonical bytes",
                actual: u64::MAX,
                max: u64::from(self.max),
            },
        )?;
        enforce("canonical bytes", actual, u64::from(self.max))?;
        self.digest.update(bytes)?;
        self.written = actual;
        Ok(())
    }
}

struct ReaderV2<'a> {
    input: &'a [u8],
    offset: usize,
}
impl<'a> ReaderV2<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }
    fn error(&self, detail: &'static str) -> MemoryModelErrorV2 {
        MemoryModelErrorV2::static_error(MemoryErrorReasonV2::Decode {
            offset: self.offset,
            detail,
        })
    }
    fn bytes(&mut self, len: usize) -> Result<&'a [u8], MemoryModelErrorV2> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| self.error("offset overflow"))?;
        if end > self.input.len() {
            return Err(self.error("truncated input"));
        }
        let bytes = &self.input[self.offset..end];
        self.offset = end;
        Ok(bytes)
    }
    fn u8(&mut self) -> Result<u8, MemoryModelErrorV2> {
        Ok(self.bytes(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, MemoryModelErrorV2> {
        Ok(u16::from_le_bytes(
            self.bytes(2)?.try_into().expect("exact width"),
        ))
    }
    fn u32(&mut self) -> Result<u32, MemoryModelErrorV2> {
        Ok(u32::from_le_bytes(
            self.bytes(4)?.try_into().expect("exact width"),
        ))
    }
    fn u64(&mut self) -> Result<u64, MemoryModelErrorV2> {
        Ok(u64::from_le_bytes(
            self.bytes(8)?.try_into().expect("exact width"),
        ))
    }
    fn u128(&mut self) -> Result<u128, MemoryModelErrorV2> {
        Ok(u128::from_le_bytes(
            self.bytes(16)?.try_into().expect("exact width"),
        ))
    }
    fn count(&mut self, resource: &'static str, max: u32) -> Result<usize, MemoryModelErrorV2> {
        let count = self.u32()?;
        enforce(resource, count as u64, max as u64).map_err(MemoryModelErrorV2::static_error)?;
        usize::try_from(count).map_err(|_| self.error("count does not fit usize"))
    }
    fn remaining(&self) -> usize {
        self.input.len() - self.offset
    }
    fn preflight_collection(
        &self,
        count: usize,
        minimum_item_bytes: usize,
    ) -> Result<(), MemoryModelErrorV2> {
        let minimum = count
            .checked_mul(minimum_item_bytes)
            .ok_or_else(|| self.error("collection byte count overflow"))?;
        if minimum > self.remaining() {
            Err(self.error("collection count exceeds remaining input"))
        } else {
            Ok(())
        }
    }
    fn finished(&self) -> bool {
        self.offset == self.input.len()
    }
}

fn decode_collection<T>(
    reader: &ReaderV2<'_>,
    resource: &'static str,
    count: usize,
    minimum_item_bytes: usize,
    budgets: MemoryBudgetsV2,
    kind: DecodeCollectionKindV2,
    work: &mut WorkMeterV2,
) -> Result<Vec<T>, MemoryModelErrorV2> {
    reader.preflight_collection(count, minimum_item_bytes)?;
    work.admit_decoded_collection(kind, count, budgets)
        .map_err(MemoryModelErrorV2::static_error)?;
    let mut values = Vec::new();
    values.try_reserve_exact(count).map_err(|_| {
        MemoryModelErrorV2::static_error(MemoryErrorReasonV2::AllocationFailed { resource })
    })?;
    Ok(values)
}

fn encode_target(
    writer: &mut WriterV2,
    target: &TargetLayoutV2,
) -> Result<(), MemoryErrorReasonV2> {
    writer.u16(target.architecture.len() as u16)?;
    writer.bytes(target.architecture.as_bytes())?;
    writer.u8(target.xnack_disabled as u8)?;
    writer.u8(target.little_endian as u8)?;
    writer.u32(target.address_spaces.len() as u32)?;
    for entry in &target.address_spaces {
        writer.u8(entry.address_space.tag())?;
        writer.u16(entry.pointer_bits)?;
        writer.u16(entry.pointer_alignment)?;
    }
    Ok(())
}

fn decode_target(
    reader: &mut ReaderV2<'_>,
    budgets: MemoryBudgetsV2,
    work: &mut WorkMeterV2,
) -> Result<TargetLayoutV2, MemoryModelErrorV2> {
    let len = reader.u16()? as usize;
    if len > 32 {
        return Err(reader.error("target name too long"));
    }
    let name = std::str::from_utf8(reader.bytes(len)?)
        .map_err(|_| reader.error("target name is not UTF-8"))?;
    let mut architecture = String::new();
    architecture.try_reserve_exact(len).map_err(|_| {
        MemoryModelErrorV2::static_error(MemoryErrorReasonV2::AllocationFailed {
            resource: "target name",
        })
    })?;
    architecture.push_str(name);
    let xnack_disabled = decode_bool(reader)?;
    let little_endian = decode_bool(reader)?;
    let count = reader.count("target address spaces", 16)?;
    let mut address_spaces = decode_collection::<AddressSpaceLayoutV2>(
        reader,
        "target address spaces",
        count,
        MIN_TARGET_ENTRY_BYTES,
        budgets,
        DecodeCollectionKindV2::Ordinary,
        work,
    )?;
    for _ in 0..count {
        let address_space = AddressSpaceV2::from_tag(reader.u8()?)
            .ok_or_else(|| reader.error("unknown address space"))?;
        address_spaces.push(AddressSpaceLayoutV2 {
            address_space,
            pointer_bits: reader.u16()?,
            pointer_alignment: reader.u16()?,
        });
    }
    Ok(TargetLayoutV2 {
        architecture,
        xnack_disabled,
        little_endian,
        address_spaces,
    })
}

fn encode_type(writer: &mut WriterV2, ty: &MemoryTypeV2) -> Result<(), MemoryErrorReasonV2> {
    writer.u32(ty.id.get())?;
    writer.u64(ty.size)?;
    writer.u64(ty.alignment)?;
    match &ty.kind {
        MemoryTypeKindV2::Scalar {
            bit_width,
            validity,
        } => {
            writer.u8(1)?;
            writer.u16(*bit_width)?;
            encode_validity(writer, validity)?;
        }
        MemoryTypeKindV2::Array {
            element,
            length,
            stride,
        } => {
            writer.u8(2)?;
            writer.u32(element.get())?;
            writer.u64(*length)?;
            writer.u64(*stride)?;
        }
        MemoryTypeKindV2::Aggregate { fields } => {
            writer.u8(3)?;
            writer.u32(fields.len() as u32)?;
            for field in fields {
                writer.u64(field.offset)?;
                writer.u32(field.ty.get())?;
            }
        }
        MemoryTypeKindV2::OpaqueBytes => writer.u8(4)?,
    }
    Ok(())
}

fn decode_type(
    reader: &mut ReaderV2<'_>,
    budgets: MemoryBudgetsV2,
    work: &mut WorkMeterV2,
) -> Result<MemoryTypeV2, MemoryModelErrorV2> {
    let id = decode_type_id(reader)?;
    let size = reader.u64()?;
    let alignment = reader.u64()?;
    let kind = match reader.u8()? {
        1 => MemoryTypeKindV2::Scalar {
            bit_width: reader.u16()?,
            validity: decode_validity(reader, budgets, work)?,
        },
        2 => MemoryTypeKindV2::Array {
            element: decode_type_id(reader)?,
            length: reader.u64()?,
            stride: reader.u64()?,
        },
        3 => {
            let count = reader.count("type edges", budgets.max_type_edges)?;
            let mut fields = decode_collection::<MemoryFieldV2>(
                reader,
                "type fields",
                count,
                MIN_FIELD_BYTES,
                budgets,
                DecodeCollectionKindV2::TypeEdges,
                work,
            )?;
            for _ in 0..count {
                fields.push(MemoryFieldV2 {
                    offset: reader.u64()?,
                    ty: decode_type_id(reader)?,
                });
            }
            MemoryTypeKindV2::Aggregate { fields }
        }
        4 => MemoryTypeKindV2::OpaqueBytes,
        _ => return Err(reader.error("unknown type kind")),
    };
    Ok(MemoryTypeV2 {
        id,
        size,
        alignment,
        kind,
    })
}

fn encode_validity(
    writer: &mut WriterV2,
    validity: &BitValidityV2,
) -> Result<(), MemoryErrorReasonV2> {
    match validity {
        BitValidityV2::Any => writer.u8(0)?,
        BitValidityV2::Bool => writer.u8(1)?,
        BitValidityV2::Char => writer.u8(2)?,
        BitValidityV2::NonZero => writer.u8(3)?,
        BitValidityV2::Ranges(ranges) => {
            writer.u8(4)?;
            writer.u32(ranges.len() as u32)?;
            for range in ranges {
                writer.u128(range.start)?;
                writer.u128(range.end_inclusive)?;
            }
        }
    }
    Ok(())
}

fn decode_validity(
    reader: &mut ReaderV2<'_>,
    budgets: MemoryBudgetsV2,
    work: &mut WorkMeterV2,
) -> Result<BitValidityV2, MemoryModelErrorV2> {
    Ok(match reader.u8()? {
        0 => BitValidityV2::Any,
        1 => BitValidityV2::Bool,
        2 => BitValidityV2::Char,
        3 => BitValidityV2::NonZero,
        4 => {
            let count = reader.count("validity ranges", budgets.max_validity_ranges)?;
            let mut ranges = decode_collection::<BitValidityRangeV2>(
                reader,
                "validity ranges",
                count,
                MIN_VALIDITY_RANGE_BYTES,
                budgets,
                DecodeCollectionKindV2::ValidityRanges,
                work,
            )?;
            for _ in 0..count {
                ranges.push(BitValidityRangeV2 {
                    start: reader.u128()?,
                    end_inclusive: reader.u128()?,
                });
            }
            BitValidityV2::Ranges(ranges)
        }
        _ => return Err(reader.error("unknown validity rule")),
    })
}

fn encode_action<W: CanonicalWriterV2>(
    writer: &mut W,
    action: &MemoryActionV2,
) -> Result<(), MemoryErrorReasonV2> {
    match action {
        MemoryActionV2::Allocate {
            allocation,
            generation,
            owner,
            address_space,
            base_address,
            byte_len,
            alignment,
            lifetime,
        } => {
            writer.u8(1)?;
            writer.u32(allocation.get())?;
            writer.u64(*generation)?;
            writer.u32(owner.get())?;
            writer.u8(address_space.tag())?;
            writer.u64(*base_address)?;
            writer.u64(*byte_len)?;
            writer.u64(*alignment)?;
            encode_lifetime(writer, *lifetime)?;
        }
        MemoryActionV2::AdvanceEpoch { to } => {
            writer.u8(2)?;
            writer.u64(to.0)?;
        }
        MemoryActionV2::BeginBorrow {
            loan,
            owner,
            place,
            kind,
            lifetime,
        } => {
            writer.u8(3)?;
            writer.u32(loan.get())?;
            writer.u32(owner.get())?;
            encode_place(writer, place)?;
            writer.u8(match kind {
                BorrowKindV2::Shared => 0,
                BorrowKindV2::Exclusive => 1,
            })?;
            encode_lifetime(writer, *lifetime)?;
        }
        MemoryActionV2::EndBorrow { loan, owner } => {
            writer.u8(4)?;
            writer.u32(loan.get())?;
            writer.u32(owner.get())?;
        }
        MemoryActionV2::WriteTyped {
            actor,
            place,
            value,
        } => {
            writer.u8(5)?;
            encode_actor(writer, *actor)?;
            encode_place(writer, place)?;
            match value {
                TypedWriteValueV2::KnownBits(bits) => {
                    writer.u8(0)?;
                    writer.u128(*bits)?;
                }
                TypedWriteValueV2::ValidOpaque => writer.u8(1)?,
            }
        }
        MemoryActionV2::ReadTyped { actor, place } => {
            writer.u8(6)?;
            encode_actor(writer, *actor)?;
            encode_place(writer, place)?;
        }
        MemoryActionV2::GrantRawCapability {
            capability,
            owner,
            provenance,
            scope,
            range,
            access,
            lifetime,
        } => {
            writer.u8(7)?;
            writer.u32(capability.get())?;
            writer.u32(owner.get())?;
            encode_provenance(writer, *provenance)?;
            encode_scope(writer, *scope)?;
            encode_range(writer, *range)?;
            writer.u8(match access {
                RawAccessV2::Read => 0,
                RawAccessV2::Write => 1,
                RawAccessV2::ReadWrite => 2,
            })?;
            encode_lifetime(writer, *lifetime)?;
        }
        MemoryActionV2::GrantAddressSpaceCastCapability {
            capability,
            owner,
            provenance,
            scope,
            range,
            from,
            to,
            lifetime,
        } => {
            writer.u8(8)?;
            writer.u32(capability.get())?;
            writer.u32(owner.get())?;
            encode_provenance(writer, *provenance)?;
            encode_scope(writer, *scope)?;
            encode_range(writer, *range)?;
            writer.u8(from.tag())?;
            writer.u8(to.tag())?;
            encode_lifetime(writer, *lifetime)?;
        }
        MemoryActionV2::ReadRaw {
            actor,
            place,
            raw_capability,
            cast_capability,
        } => {
            writer.u8(9)?;
            encode_actor(writer, *actor)?;
            encode_raw_place(writer, *place)?;
            writer.u32(raw_capability.get())?;
            encode_optional_capability(writer, *cast_capability)?;
        }
        MemoryActionV2::WriteRaw {
            actor,
            place,
            raw_capability,
            cast_capability,
        } => {
            writer.u8(10)?;
            encode_actor(writer, *actor)?;
            encode_raw_place(writer, *place)?;
            writer.u32(raw_capability.get())?;
            encode_optional_capability(writer, *cast_capability)?;
        }
        MemoryActionV2::Deallocate { allocation, owner } => {
            writer.u8(11)?;
            writer.u32(allocation.get())?;
            writer.u32(owner.get())?;
        }
        MemoryActionV2::PointerDistance {
            actor,
            left,
            right,
            element_size,
            left_capability,
            right_capability,
            left_cast_capability,
            right_cast_capability,
        } => {
            writer.u8(12)?;
            encode_actor(writer, *actor)?;
            encode_raw_place(writer, *left)?;
            encode_raw_place(writer, *right)?;
            writer.u64(*element_size)?;
            writer.u32(left_capability.get())?;
            writer.u32(right_capability.get())?;
            encode_optional_capability(writer, *left_cast_capability)?;
            encode_optional_capability(writer, *right_cast_capability)?;
        }
        MemoryActionV2::CopyNonOverlapping {
            actor,
            source,
            destination,
            source_capability,
            destination_capability,
            source_cast_capability,
            destination_cast_capability,
        } => {
            writer.u8(13)?;
            encode_actor(writer, *actor)?;
            encode_raw_place(writer, *source)?;
            encode_raw_place(writer, *destination)?;
            writer.u32(source_capability.get())?;
            writer.u32(destination_capability.get())?;
            encode_optional_capability(writer, *source_cast_capability)?;
            encode_optional_capability(writer, *destination_cast_capability)?;
        }
    }
    Ok(())
}

fn decode_action(
    reader: &mut ReaderV2<'_>,
    budgets: MemoryBudgetsV2,
    work: &mut WorkMeterV2,
) -> Result<MemoryActionV2, MemoryModelErrorV2> {
    Ok(match reader.u8()? {
        1 => MemoryActionV2::Allocate {
            allocation: decode_allocation_id(reader)?,
            generation: reader.u64()?,
            owner: decode_owner_id(reader)?,
            address_space: decode_space(reader)?,
            base_address: reader.u64()?,
            byte_len: reader.u64()?,
            alignment: reader.u64()?,
            lifetime: decode_lifetime(reader)?,
        },
        2 => MemoryActionV2::AdvanceEpoch {
            to: EpochV2(reader.u64()?),
        },
        3 => MemoryActionV2::BeginBorrow {
            loan: decode_loan_id(reader)?,
            owner: decode_owner_id(reader)?,
            place: decode_place(reader, budgets, work)?,
            kind: match reader.u8()? {
                0 => BorrowKindV2::Shared,
                1 => BorrowKindV2::Exclusive,
                _ => return Err(reader.error("unknown borrow kind")),
            },
            lifetime: decode_lifetime(reader)?,
        },
        4 => MemoryActionV2::EndBorrow {
            loan: decode_loan_id(reader)?,
            owner: decode_owner_id(reader)?,
        },
        5 => MemoryActionV2::WriteTyped {
            actor: decode_actor(reader)?,
            place: decode_place(reader, budgets, work)?,
            value: match reader.u8()? {
                0 => TypedWriteValueV2::KnownBits(reader.u128()?),
                1 => TypedWriteValueV2::ValidOpaque,
                _ => return Err(reader.error("unknown typed value")),
            },
        },
        6 => MemoryActionV2::ReadTyped {
            actor: decode_actor(reader)?,
            place: decode_place(reader, budgets, work)?,
        },
        7 => MemoryActionV2::GrantRawCapability {
            capability: decode_capability_id(reader)?,
            owner: decode_owner_id(reader)?,
            provenance: decode_provenance(reader)?,
            scope: decode_scope(reader)?,
            range: decode_range(reader)?,
            access: match reader.u8()? {
                0 => RawAccessV2::Read,
                1 => RawAccessV2::Write,
                2 => RawAccessV2::ReadWrite,
                _ => return Err(reader.error("unknown raw access")),
            },
            lifetime: decode_lifetime(reader)?,
        },
        8 => MemoryActionV2::GrantAddressSpaceCastCapability {
            capability: decode_capability_id(reader)?,
            owner: decode_owner_id(reader)?,
            provenance: decode_provenance(reader)?,
            scope: decode_scope(reader)?,
            range: decode_range(reader)?,
            from: decode_space(reader)?,
            to: decode_space(reader)?,
            lifetime: decode_lifetime(reader)?,
        },
        9 => MemoryActionV2::ReadRaw {
            actor: decode_actor(reader)?,
            place: decode_raw_place(reader)?,
            raw_capability: decode_capability_id(reader)?,
            cast_capability: decode_optional_capability(reader)?,
        },
        10 => MemoryActionV2::WriteRaw {
            actor: decode_actor(reader)?,
            place: decode_raw_place(reader)?,
            raw_capability: decode_capability_id(reader)?,
            cast_capability: decode_optional_capability(reader)?,
        },
        11 => MemoryActionV2::Deallocate {
            allocation: decode_allocation_id(reader)?,
            owner: decode_owner_id(reader)?,
        },
        12 => MemoryActionV2::PointerDistance {
            actor: decode_actor(reader)?,
            left: decode_raw_place(reader)?,
            right: decode_raw_place(reader)?,
            element_size: reader.u64()?,
            left_capability: decode_capability_id(reader)?,
            right_capability: decode_capability_id(reader)?,
            left_cast_capability: decode_optional_capability(reader)?,
            right_cast_capability: decode_optional_capability(reader)?,
        },
        13 => MemoryActionV2::CopyNonOverlapping {
            actor: decode_actor(reader)?,
            source: decode_raw_place(reader)?,
            destination: decode_raw_place(reader)?,
            source_capability: decode_capability_id(reader)?,
            destination_capability: decode_capability_id(reader)?,
            source_cast_capability: decode_optional_capability(reader)?,
            destination_cast_capability: decode_optional_capability(reader)?,
        },
        _ => return Err(reader.error("unknown action")),
    })
}

fn encode_place<W: CanonicalWriterV2>(
    writer: &mut W,
    place: &TypedPlaceV2,
) -> Result<(), MemoryErrorReasonV2> {
    encode_provenance(writer, place.provenance)?;
    writer.u64(place.base_offset)?;
    writer.u32(place.root_type.get())?;
    writer.u32(place.projections.len() as u32)?;
    for projection in &place.projections {
        match projection {
            ProjectionV2::Field(index) => {
                writer.u8(0)?;
                writer.u32(*index)?;
            }
            ProjectionV2::Index(index) => {
                writer.u8(1)?;
                writer.u64(*index)?;
            }
        }
    }
    Ok(())
}

fn decode_place(
    reader: &mut ReaderV2<'_>,
    budgets: MemoryBudgetsV2,
    work: &mut WorkMeterV2,
) -> Result<TypedPlaceV2, MemoryModelErrorV2> {
    let provenance = decode_provenance(reader)?;
    let base_offset = reader.u64()?;
    let root_type = decode_type_id(reader)?;
    let count = reader.count("place projections", budgets.max_projections_per_place)?;
    let mut projections = decode_collection::<ProjectionV2>(
        reader,
        "place projections",
        count,
        MIN_PROJECTION_BYTES,
        budgets,
        DecodeCollectionKindV2::Ordinary,
        work,
    )?;
    for _ in 0..count {
        projections.push(match reader.u8()? {
            0 => ProjectionV2::Field(reader.u32()?),
            1 => ProjectionV2::Index(reader.u64()?),
            _ => return Err(reader.error("unknown projection")),
        });
    }
    Ok(TypedPlaceV2 {
        provenance,
        base_offset,
        root_type,
        projections,
    })
}

fn encode_raw_place<W: CanonicalWriterV2>(
    writer: &mut W,
    place: RawPlaceV2,
) -> Result<(), MemoryErrorReasonV2> {
    encode_provenance(writer, place.provenance)?;
    writer.u8(place.pointer_address_space.tag())?;
    writer.u64(place.byte_offset)?;
    writer.u64(place.byte_len)?;
    writer.u64(place.alignment)
}
fn decode_raw_place(reader: &mut ReaderV2<'_>) -> Result<RawPlaceV2, MemoryModelErrorV2> {
    Ok(RawPlaceV2 {
        provenance: decode_provenance(reader)?,
        pointer_address_space: decode_space(reader)?,
        byte_offset: reader.u64()?,
        byte_len: reader.u64()?,
        alignment: reader.u64()?,
    })
}

fn encode_actor<W: CanonicalWriterV2>(
    writer: &mut W,
    actor: AccessActorV2,
) -> Result<(), MemoryErrorReasonV2> {
    match actor {
        AccessActorV2::Owner(owner) => {
            writer.u8(0)?;
            writer.u32(owner.get())
        }
        AccessActorV2::Loan { loan, borrow_epoch } => {
            writer.u8(1)?;
            writer.u32(loan.get())?;
            writer.u64(borrow_epoch)
        }
    }
}
fn decode_actor(reader: &mut ReaderV2<'_>) -> Result<AccessActorV2, MemoryModelErrorV2> {
    match reader.u8()? {
        0 => Ok(AccessActorV2::Owner(decode_owner_id(reader)?)),
        1 => Ok(AccessActorV2::Loan {
            loan: decode_loan_id(reader)?,
            borrow_epoch: reader.u64()?,
        }),
        _ => Err(reader.error("unknown actor")),
    }
}

fn encode_scope<W: CanonicalWriterV2>(
    writer: &mut W,
    scope: CapabilityScopeV2,
) -> Result<(), MemoryErrorReasonV2> {
    match scope {
        CapabilityScopeV2::Owner(owner) => {
            writer.u8(0)?;
            writer.u32(owner.get())
        }
        CapabilityScopeV2::Loan { loan, borrow_epoch } => {
            writer.u8(1)?;
            writer.u32(loan.get())?;
            writer.u64(borrow_epoch)
        }
    }
}
fn decode_scope(reader: &mut ReaderV2<'_>) -> Result<CapabilityScopeV2, MemoryModelErrorV2> {
    match reader.u8()? {
        0 => Ok(CapabilityScopeV2::Owner(decode_owner_id(reader)?)),
        1 => Ok(CapabilityScopeV2::Loan {
            loan: decode_loan_id(reader)?,
            borrow_epoch: reader.u64()?,
        }),
        _ => Err(reader.error("unknown capability scope")),
    }
}

fn encode_provenance<W: CanonicalWriterV2>(
    writer: &mut W,
    provenance: ProvenanceV2,
) -> Result<(), MemoryErrorReasonV2> {
    writer.u32(provenance.allocation.get())?;
    writer.u64(provenance.generation)
}
fn decode_provenance(reader: &mut ReaderV2<'_>) -> Result<ProvenanceV2, MemoryModelErrorV2> {
    Ok(ProvenanceV2 {
        allocation: decode_allocation_id(reader)?,
        generation: reader.u64()?,
    })
}
fn encode_lifetime<W: CanonicalWriterV2>(
    writer: &mut W,
    lifetime: LifetimeRegionV2,
) -> Result<(), MemoryErrorReasonV2> {
    writer.u64(lifetime.start.0)?;
    writer.u64(lifetime.end_inclusive.0)
}
fn decode_lifetime(reader: &mut ReaderV2<'_>) -> Result<LifetimeRegionV2, MemoryModelErrorV2> {
    Ok(LifetimeRegionV2 {
        start: EpochV2(reader.u64()?),
        end_inclusive: EpochV2(reader.u64()?),
    })
}
fn encode_range<W: CanonicalWriterV2>(
    writer: &mut W,
    range: ByteRangeV2,
) -> Result<(), MemoryErrorReasonV2> {
    writer.u64(range.start)?;
    writer.u64(range.len)
}
fn decode_range(reader: &mut ReaderV2<'_>) -> Result<ByteRangeV2, MemoryModelErrorV2> {
    Ok(ByteRangeV2 {
        start: reader.u64()?,
        len: reader.u64()?,
    })
}

fn encode_optional_capability<W: CanonicalWriterV2>(
    writer: &mut W,
    capability: Option<CapabilityIdV2>,
) -> Result<(), MemoryErrorReasonV2> {
    match capability {
        Some(id) => {
            writer.u8(1)?;
            writer.u32(id.get())
        }
        None => writer.u8(0),
    }
}
fn decode_optional_capability(
    reader: &mut ReaderV2<'_>,
) -> Result<Option<CapabilityIdV2>, MemoryModelErrorV2> {
    match reader.u8()? {
        0 => Ok(None),
        1 => Ok(Some(decode_capability_id(reader)?)),
        _ => Err(reader.error("unknown optional capability tag")),
    }
}
fn decode_bool(reader: &mut ReaderV2<'_>) -> Result<bool, MemoryModelErrorV2> {
    match reader.u8()? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(reader.error("noncanonical boolean")),
    }
}
fn decode_space(reader: &mut ReaderV2<'_>) -> Result<AddressSpaceV2, MemoryModelErrorV2> {
    AddressSpaceV2::from_tag(reader.u8()?).ok_or_else(|| reader.error("unknown address space"))
}

macro_rules! decode_id {
    ($name:ident, $ty:ident) => {
        fn $name(reader: &mut ReaderV2<'_>) -> Result<$ty, MemoryModelErrorV2> {
            $ty::new(reader.u32()?).ok_or_else(|| reader.error("zero identity"))
        }
    };
}
decode_id!(decode_type_id, MemoryTypeIdV2);
decode_id!(decode_allocation_id, AllocationIdV2);
decode_id!(decode_owner_id, OwnerIdV2);
decode_id!(decode_loan_id, LoanIdV2);
decode_id!(decode_capability_id, CapabilityIdV2);

#[cfg(test)]
mod internal_tests {
    use super::*;

    fn allocation_state(id: u32, generation: u64) -> AllocationStateV2 {
        let allocation = AllocationIdV2::new(id).unwrap();
        AllocationStateV2 {
            provenance: ProvenanceV2 {
                allocation,
                generation,
            },
            owner: OwnerIdV2::new(1).unwrap(),
            address_space: AddressSpaceV2::Global,
            base_address: 0x1000,
            byte_len: 16,
            lifetime: LifetimeRegionV2 {
                start: EpochV2(0),
                end_inclusive: EpochV2(1),
            },
            dead_at: None,
            next_borrow_epoch: 0,
            initialized: Vec::new(),
            typed: Vec::new(),
        }
    }

    #[test]
    fn copy_rechecks_physical_overlap_across_distinct_provenance() {
        let target = TargetLayoutV2::gfx942_xnack_minus();
        let budgets = MemoryBudgetsV2::default();
        let mut allocations =
            BoundedStateMapV2::try_with_capacity("allocations", budgets.max_allocations).unwrap();
        assert!(
            allocations
                .insert(AllocationIdV2::new(1).unwrap(), allocation_state(1, 7))
                .unwrap()
        );
        assert!(
            allocations
                .insert(
                    AllocationIdV2::new(2).unwrap(),
                    AllocationStateV2 {
                        address_space: AddressSpaceV2::Flat,
                        ..allocation_state(2, 8)
                    },
                )
                .unwrap()
        );
        let mut machine = MachineV2 {
            target: &target,
            types: &[],
            epoch: EpochV2(0),
            allocations,
            loans: BoundedStateMapV2::try_with_capacity("loans", budgets.max_loans).unwrap(),
            capabilities: BoundedStateMapV2::try_with_capacity(
                "capabilities",
                budgets.max_capabilities,
            )
            .unwrap(),
            records: Vec::new(),
            obligation_count: 0,
            program_identity: UntrustedMemoryProgramIdentityV2([0; 32]),
            action_identity: MemoryActionIdentityV2([0; 32]),
            action_index: 0,
            execution_work: WorkMeterV2::execution(budgets.max_execution_work),
            budgets,
        };
        let raw = |id, generation| RawPlaceV2 {
            provenance: ProvenanceV2 {
                allocation: AllocationIdV2::new(id).unwrap(),
                generation,
            },
            pointer_address_space: AddressSpaceV2::Global,
            byte_offset: 0,
            byte_len: 4,
            alignment: 4,
        };

        assert_eq!(
            machine
                .copy_nonoverlapping(
                    AccessActorV2::Owner(OwnerIdV2::new(1).unwrap()),
                    raw(1, 7),
                    raw(2, 8),
                    CapabilityIdV2::new(1).unwrap(),
                    CapabilityIdV2::new(2).unwrap(),
                    None,
                    None,
                )
                .unwrap_err(),
            MemoryErrorReasonV2::OverlappingCopy
        );
    }

    #[test]
    fn bounded_runtime_collections_reject_before_mutation() {
        let first = ByteRangeV2 { start: 0, len: 1 };
        let second = ByteRangeV2 { start: 2, len: 1 };

        let mut initialized = Vec::new();
        insert_range(&mut initialized, first, 1).unwrap();
        let initialized_before = initialized.clone();
        assert_eq!(
            insert_range(&mut initialized, second, 1).unwrap_err(),
            MemoryErrorReasonV2::ResourceLimit {
                resource: "state ranges",
                actual: 2,
                max: 1,
            }
        );
        assert_eq!(initialized, initialized_before);

        let mut typed = Vec::new();
        replace_typed_range(&mut typed, first, MemoryTypeIdV2::new(1).unwrap(), 1).unwrap();
        let typed_before = typed.clone();
        assert_eq!(
            replace_typed_range(&mut typed, second, MemoryTypeIdV2::new(1).unwrap(), 1)
                .unwrap_err(),
            MemoryErrorReasonV2::ResourceLimit {
                resource: "typed state ranges",
                actual: 2,
                max: 1,
            }
        );
        assert_eq!(typed, typed_before);

        let mut map = BoundedStateMapV2::try_with_capacity("test state", 1).unwrap();
        assert!(map.insert(1_u32, 10_u32).unwrap());
        assert_eq!(
            map.insert(2, 20).unwrap_err(),
            MemoryErrorReasonV2::ResourceLimit {
                resource: "test state",
                actual: 2,
                max: 1,
            }
        );
        assert_eq!(map.len(), 1);
        assert_eq!(map.get(&1), Some(&10));
        assert_eq!(map.get(&2), None);
    }

    fn identity_test_action(projections: Vec<ProjectionV2>) -> MemoryActionV2 {
        MemoryActionV2::ReadTyped {
            actor: AccessActorV2::Owner(OwnerIdV2::new(1).unwrap()),
            place: TypedPlaceV2 {
                provenance: ProvenanceV2 {
                    allocation: AllocationIdV2::new(1).unwrap(),
                    generation: u64::MAX,
                },
                base_offset: u64::MAX,
                root_type: MemoryTypeIdV2::new(u32::MAX).unwrap(),
                projections,
            },
        }
    }

    #[test]
    fn action_identity_preflight_is_exact_streaming_and_retry_stable() {
        let identity = UntrustedMemoryProgramIdentityV2([0x5a; 32]);
        let action = identity_test_action(vec![
            ProjectionV2::Field(u32::MAX),
            ProjectionV2::Index(u64::MAX),
        ]);
        let mut counter = CountingWriterV2::new(HARD_MAX_CANONICAL_BYTES);
        encode_action(&mut counter, &action).unwrap();
        let exact_bytes = u32::try_from(counter.written()).unwrap();

        let exact_byte_budgets = MemoryBudgetsV2 {
            max_actions: 1,
            max_projections_per_place: 2,
            max_canonical_bytes: exact_bytes,
            ..MemoryBudgetsV2::default()
        };
        let mut work = WorkMeterV2::execution(exact_byte_budgets.max_execution_work);
        canonical_action_identity_v2(identity, 0, &action, exact_byte_budgets, &mut work).unwrap();
        let exact_work = work.used();

        let exact_work_budgets = MemoryBudgetsV2 {
            max_execution_work: exact_work,
            ..exact_byte_budgets
        };
        let mut work = WorkMeterV2::execution(exact_work);
        canonical_action_identity_v2(identity, 0, &action, exact_work_budgets, &mut work).unwrap();
        assert_eq!(work.used(), exact_work);

        let short_bytes = MemoryBudgetsV2 {
            max_canonical_bytes: exact_bytes - 1,
            ..exact_byte_budgets
        };
        let mut work = WorkMeterV2::execution(short_bytes.max_execution_work);
        assert_eq!(
            canonical_action_identity_v2(identity, 0, &action, short_bytes, &mut work).unwrap_err(),
            MemoryErrorReasonV2::ResourceLimit {
                resource: "canonical bytes",
                actual: u64::from(exact_bytes),
                max: u64::from(exact_bytes - 1),
            }
        );

        let short_work = MemoryBudgetsV2 {
            max_execution_work: exact_work - 1,
            ..exact_byte_budgets
        };
        let mut work = WorkMeterV2::execution(short_work.max_execution_work);
        assert!(matches!(
            canonical_action_identity_v2(identity, 0, &action, short_work, &mut work),
            Err(MemoryErrorReasonV2::ResourceLimit {
                resource: "execution work",
                actual,
                max,
            }) if actual == exact_work && max == exact_work - 1
        ));

        let projection_bomb = identity_test_action(vec![ProjectionV2::Field(u32::MAX); 1_000]);
        let hostile_budgets = MemoryBudgetsV2 {
            max_actions: 1,
            max_projections_per_place: 0,
            max_canonical_bytes: 1,
            max_execution_work: 1,
            ..MemoryBudgetsV2::default()
        };
        for _ in 0..8 {
            let mut work = WorkMeterV2::execution(hostile_budgets.max_execution_work);
            assert_eq!(
                canonical_action_identity_v2(
                    identity,
                    0,
                    &projection_bomb,
                    hostile_budgets,
                    &mut work,
                )
                .unwrap_err(),
                MemoryErrorReasonV2::ResourceLimit {
                    resource: "place projections",
                    actual: 1_000,
                    max: 0,
                }
            );
            assert_eq!(work.used(), 0, "projection rejection must precede encoding");
        }
    }
}
