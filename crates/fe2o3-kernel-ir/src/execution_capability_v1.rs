//! Target-neutral execution-capability types and operations introduced by KIR V13.
//!
//! These records are compiler-authenticated facts, not proof receipts. Local
//! verification checks their closed typestate and provenance transitions. The
//! unresolved obligation bits are retained for whole-kernel analyses.

use std::collections::BTreeSet;

use crate::{
    AccessMode, AddressSpace, AsyncCopyCompletionV1, AtomicKind, CollectiveCapabilityOperationV1,
    ExecutionCapabilityRequirementV1, FunctionId, MemoryEffect, MemoryOrdering,
    ResourceCapabilityRequirementV1, ScalarType, SynchronizationScope, TargetCapability, ValueId,
};

pub const MAX_EXECUTION_CAPABILITY_ARGUMENTS_V1: usize = 4;
pub const MAX_EXECUTION_CAPABILITY_OPERANDS_V1: usize = 32;
pub const MAX_EXECUTION_CAPABILITY_RESULTS_V1: usize = 16;
pub const MAX_EXECUTION_CAPABILITY_CONTRACT_BYTES_V1: usize = 2048;
pub const MAX_EXECUTION_CAPABILITY_TYPE_BYTES_V1: usize = 768;

/// Stable identity of one source-language type used by an execution contract.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExecutionTypeIdentityV1([u8; 32]);

impl ExecutionTypeIdentityV1 {
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn is_complete(self) -> bool {
        self.0 != [0; 32]
    }
}

/// Compiler-authenticated kernel provenance shared by every derived value.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExecutionCapabilityProvenanceV1 {
    pub root: FunctionId,
    pub kernel_binding: [u8; 32],
    pub frontend_unit: [u8; 32],
    pub kernel_marker: [u8; 32],
    pub target_brand: [u8; 32],
    pub launch_brand: [u8; 32],
    pub issuance: [u8; 32],
}

impl ExecutionCapabilityProvenanceV1 {
    pub fn is_complete(&self) -> bool {
        !self.root.as_str().is_empty()
            && [
                self.kernel_binding,
                self.frontend_unit,
                self.kernel_marker,
                self.target_brand,
                self.launch_brand,
                self.issuance,
            ]
            .into_iter()
            .all(|identity| identity != [0; 32])
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExecutionMemoryScopeV1 {
    System,
    Device,
    Workgroup,
    Subgroup,
}

impl ExecutionMemoryScopeV1 {
    pub const fn synchronization_scope(self) -> SynchronizationScope {
        match self {
            Self::System => SynchronizationScope::System,
            Self::Device => SynchronizationScope::Device,
            Self::Workgroup => SynchronizationScope::Workgroup,
            Self::Subgroup => SynchronizationScope::Subgroup,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExecutionMemoryOrderingV1 {
    Relaxed,
    Acquire,
    Release,
    AcquireRelease,
    SequentiallyConsistent,
}

impl ExecutionMemoryOrderingV1 {
    pub const fn memory_ordering(self) -> MemoryOrdering {
        match self {
            Self::Relaxed => MemoryOrdering::Relaxed,
            Self::Acquire => MemoryOrdering::Acquire,
            Self::Release => MemoryOrdering::Release,
            Self::AcquireRelease => MemoryOrdering::AcquireRelease,
            Self::SequentiallyConsistent => MemoryOrdering::SequentiallyConsistent,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExecutionMemorySpacesV1 {
    Global,
    Workgroup,
    GlobalAndWorkgroup,
}

impl ExecutionMemorySpacesV1 {
    pub fn address_spaces(self) -> BTreeSet<AddressSpace> {
        match self {
            Self::Global => BTreeSet::from([AddressSpace::Global]),
            Self::Workgroup => BTreeSet::from([AddressSpace::Workgroup]),
            Self::GlobalAndWorkgroup => {
                BTreeSet::from([AddressSpace::Global, AddressSpace::Workgroup])
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExecutionMemorySemanticsV1 {
    pub scope: ExecutionMemoryScopeV1,
    pub ordering: ExecutionMemoryOrderingV1,
    pub spaces: ExecutionMemorySpacesV1,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExecutionMemoryAddressSpaceV1 {
    Private,
    Workgroup,
    Global,
}

impl ExecutionMemoryAddressSpaceV1 {
    pub const fn address_space(self) -> AddressSpace {
        match self {
            Self::Private => AddressSpace::Private,
            Self::Workgroup => AddressSpace::Workgroup,
            Self::Global => AddressSpace::Global,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExecutionMemoryAccessV1 {
    ReadOnly,
    ExclusiveReadWrite,
    DisjointWrite,
    AtomicReadWrite,
}

impl ExecutionMemoryAccessV1 {
    pub const fn access_mode(self) -> AccessMode {
        match self {
            Self::ReadOnly => AccessMode::ReadOnly,
            Self::DisjointWrite => AccessMode::WriteOnly,
            Self::ExclusiveReadWrite | Self::AtomicReadWrite => AccessMode::ReadWrite,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExecutionMemoryInitializationV1 {
    Uninitialized,
    InvocationInitialized,
    Published,
    FullyInitialized,
    SelectedWriteInitializes,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExecutionElementLayoutV1 {
    pub byte_size: u32,
    pub byte_alignment: u16,
}

impl ExecutionElementLayoutV1 {
    pub const fn is_complete(self) -> bool {
        self.byte_size != 0 && self.byte_alignment != 0 && self.byte_alignment.is_power_of_two()
    }

    pub const fn checked_footprint(self, elements: u64) -> Option<u64> {
        elements.checked_mul(self.byte_size as u64)
    }
}

/// A dynamic extent names its live operand position and a checked resource
/// ceiling. Operand positions survive SSA renaming in the exact Pliron bridge.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExecutionDynamicExtentV1 {
    pub operand: u8,
    pub source_argument: u8,
    pub source_type: ExecutionTypeIdentityV1,
    pub value_type: ScalarType,
    pub upper_bound: u64,
    pub bound_check_operand: u8,
    pub nonnegative_check_operand: Option<u8>,
}

impl ExecutionDynamicExtentV1 {
    pub fn is_complete(self) -> bool {
        self.source_type.is_complete()
            && self.upper_bound != 0
            && match self.value_type {
                ScalarType::U8
                | ScalarType::U16
                | ScalarType::U32
                | ScalarType::U64
                | ScalarType::Index => self.nonnegative_check_operand.is_none(),
                ScalarType::I8 => {
                    self.nonnegative_check_operand.is_some() && self.upper_bound <= i8::MAX as u64
                }
                ScalarType::I16 => {
                    self.nonnegative_check_operand.is_some() && self.upper_bound <= i16::MAX as u64
                }
                ScalarType::I32 => {
                    self.nonnegative_check_operand.is_some() && self.upper_bound <= i32::MAX as u64
                }
                ScalarType::I64 => {
                    self.nonnegative_check_operand.is_some() && self.upper_bound <= i64::MAX as u64
                }
                ScalarType::Bool
                | ScalarType::I128
                | ScalarType::U128
                | ScalarType::F16
                | ScalarType::Bf16
                | ScalarType::F32
                | ScalarType::F64 => false,
            }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExecutionMemoryExtentV1 {
    Static(u64),
    Dynamic(ExecutionDynamicExtentV1),
}

impl ExecutionMemoryExtentV1 {
    pub fn is_complete(self) -> bool {
        match self {
            Self::Static(elements) => elements != 0,
            Self::Dynamic(extent) => extent.is_complete(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExecutionLdsStateV1 {
    Uninitialized,
    InvocationInitialized,
    Published,
    PendingAsyncCopy,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExecutionCollectiveKindV1 {
    ReduceSum,
    InclusiveScanSum,
    ExclusiveScanSum,
}

impl ExecutionCollectiveKindV1 {
    pub const fn requirement(self) -> CollectiveCapabilityOperationV1 {
        match self {
            Self::ReduceSum => CollectiveCapabilityOperationV1::ReduceAdd,
            Self::InclusiveScanSum => CollectiveCapabilityOperationV1::InclusiveScanAdd,
            Self::ExclusiveScanSum => CollectiveCapabilityOperationV1::ExclusiveScanAdd,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExecutionAtomicKindV1 {
    BindGlobalLocation,
    Load,
    Store,
    FetchAdd,
    CompareExchange,
    BindGlobalView,
}

impl ExecutionAtomicKindV1 {
    pub const fn atomic_kind(self) -> Option<AtomicKind> {
        match self {
            Self::Load => Some(AtomicKind::Load),
            Self::Store => Some(AtomicKind::Store),
            Self::FetchAdd => Some(AtomicKind::Add),
            Self::CompareExchange => Some(AtomicKind::CompareExchange),
            Self::BindGlobalLocation | Self::BindGlobalView => None,
        }
    }
}

/// Logical SSA role. This is deliberately richer than the physical ABI.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExecutionCapabilityRoleV1 {
    KernelAuthority,
    Workgroup,
    Subgroup {
        width: u32,
    },
    Lds {
        element: ExecutionTypeIdentityV1,
        layout: ExecutionElementLayoutV1,
        elements: u64,
        state: ExecutionLdsStateV1,
    },
    ScopedAtomic {
        element: ExecutionTypeIdentityV1,
        space: ExecutionMemoryAddressSpaceV1,
        scope: ExecutionMemoryScopeV1,
    },
    Matrix {
        subgroup_brand: [u8; 32],
        width: u32,
    },
    PendingAsyncCopy {
        element: ExecutionTypeIdentityV1,
        layout: ExecutionElementLayoutV1,
        elements: u64,
    },
    MemoryView {
        element: ExecutionTypeIdentityV1,
        layout: ExecutionElementLayoutV1,
        space: ExecutionMemoryAddressSpaceV1,
        access: ExecutionMemoryAccessV1,
        extent: ExecutionMemoryExtentV1,
        initialization: ExecutionMemoryInitializationV1,
        index_space: Option<ExecutionTypeIdentityV1>,
        atomic_scope: Option<ExecutionMemoryScopeV1>,
    },
    WorkgroupMemoryIndex,
    EpochTransition,
    UnsafeRawMemoryObligation,
}

impl ExecutionCapabilityRoleV1 {
    pub fn is_complete(&self) -> bool {
        match self {
            Self::Subgroup { width } => *width != 0 && width.is_power_of_two(),
            Self::Lds {
                element,
                layout,
                elements,
                ..
            }
            | Self::PendingAsyncCopy {
                element,
                layout,
                elements,
            } => {
                element.is_complete()
                    && layout.is_complete()
                    && layout.checked_footprint(*elements).is_some()
            }
            Self::ScopedAtomic { element, .. } => element.is_complete(),
            Self::MemoryView {
                element,
                layout,
                extent,
                access,
                index_space,
                atomic_scope,
                ..
            } => {
                let disjoint_index = matches!(access, ExecutionMemoryAccessV1::DisjointWrite);
                let scoped_atomic = matches!(access, ExecutionMemoryAccessV1::AtomicReadWrite);
                element.is_complete()
                    && layout.is_complete()
                    && extent.is_complete()
                    && disjoint_index == index_space.is_some()
                    && scoped_atomic == atomic_scope.is_some()
                    && index_space.is_none_or(|identity| identity.is_complete())
            }
            Self::Matrix {
                subgroup_brand,
                width,
            } => *subgroup_brand != [0; 32] && *width != 0 && width.is_power_of_two(),
            Self::KernelAuthority
            | Self::Workgroup
            | Self::WorkgroupMemoryIndex
            | Self::EpochTransition
            | Self::UnsafeRawMemoryObligation => true,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExecutionCapabilityTypeV1 {
    pub source_type: ExecutionTypeIdentityV1,
    pub provenance: ExecutionCapabilityProvenanceV1,
    pub workgroup_brand: Option<[u8; 32]>,
    pub epoch: Option<[u8; 32]>,
    pub role: ExecutionCapabilityRoleV1,
}

impl ExecutionCapabilityTypeV1 {
    pub fn is_complete(&self) -> bool {
        self.source_type.is_complete()
            && self.provenance.is_complete()
            && self.role.is_complete()
            && self.workgroup_brand.is_some() == self.epoch.is_some()
            && self
                .workgroup_brand
                .is_none_or(|identity| identity != [0; 32])
            && self.epoch.is_none_or(|identity| identity != [0; 32])
    }
}

/// Closed operation payload. Type references are stable source type identities,
/// never table ordinals from Semantic MIR.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExecutionCapabilityOperationV1 {
    WorkgroupDerive {
        context: ExecutionTypeIdentityV1,
        workgroup: ExecutionTypeIdentityV1,
    },
    SubgroupDerive {
        workgroup: ExecutionTypeIdentityV1,
        subgroup: ExecutionTypeIdentityV1,
        width: u32,
    },
    LdsAllocate {
        workgroup: ExecutionTypeIdentityV1,
        lds: ExecutionTypeIdentityV1,
        element: ExecutionTypeIdentityV1,
        layout: ExecutionElementLayoutV1,
        elements: u64,
    },
    LdsInitializeByInvocation {
        input_lds: ExecutionTypeIdentityV1,
        workgroup: ExecutionTypeIdentityV1,
        output_lds: ExecutionTypeIdentityV1,
        element: ExecutionTypeIdentityV1,
        layout: ExecutionElementLayoutV1,
        elements: u64,
    },
    LdsPublish {
        input_workgroup: ExecutionTypeIdentityV1,
        input_lds: ExecutionTypeIdentityV1,
        output_lds: ExecutionTypeIdentityV1,
        transition: ExecutionTypeIdentityV1,
        element: ExecutionTypeIdentityV1,
        layout: ExecutionElementLayoutV1,
        elements: u64,
    },
    LdsReadPublished {
        lds_reference: ExecutionTypeIdentityV1,
        lds: ExecutionTypeIdentityV1,
        workgroup: ExecutionTypeIdentityV1,
        index: ExecutionTypeIdentityV1,
        option: ExecutionTypeIdentityV1,
        element: ExecutionTypeIdentityV1,
        layout: ExecutionElementLayoutV1,
        elements: u64,
    },
    WorkgroupBarrier {
        input_workgroup: ExecutionTypeIdentityV1,
        output_workgroup: ExecutionTypeIdentityV1,
        semantics: ExecutionMemorySemanticsV1,
    },
    SubgroupBarrier {
        input_workgroup: ExecutionTypeIdentityV1,
        semantics: ExecutionMemorySemanticsV1,
        subgroup: ExecutionTypeIdentityV1,
        transition: ExecutionTypeIdentityV1,
        width: u32,
    },
    WorkgroupFence {
        workgroup: ExecutionTypeIdentityV1,
        result: ExecutionTypeIdentityV1,
        semantics: ExecutionMemorySemanticsV1,
    },
    SubgroupFence {
        semantics: ExecutionMemorySemanticsV1,
        subgroup_reference: ExecutionTypeIdentityV1,
        subgroup: ExecutionTypeIdentityV1,
        epoch: ExecutionTypeIdentityV1,
        result: ExecutionTypeIdentityV1,
        width: u32,
    },
    Atomic {
        kind: ExecutionAtomicKindV1,
        authority: ExecutionTypeIdentityV1,
        location_input: ExecutionTypeIdentityV1,
        location: ExecutionTypeIdentityV1,
        element: ExecutionTypeIdentityV1,
        operand: Option<ExecutionTypeIdentityV1>,
        replacement: Option<ExecutionTypeIdentityV1>,
        result: ExecutionTypeIdentityV1,
        value_type: ScalarType,
        address_space: ExecutionMemoryAddressSpaceV1,
        scope: ExecutionMemoryScopeV1,
        success: Option<ExecutionMemoryOrderingV1>,
        failure: Option<ExecutionMemoryOrderingV1>,
    },
    WorkgroupCollective {
        kind: ExecutionCollectiveKindV1,
        input_workgroup: ExecutionTypeIdentityV1,
        scratch: ExecutionTypeIdentityV1,
        element: ExecutionTypeIdentityV1,
        transition: ExecutionTypeIdentityV1,
        value_type: ScalarType,
        layout: ExecutionElementLayoutV1,
        elements: u64,
    },
    SubgroupCollective {
        kind: ExecutionCollectiveKindV1,
        subgroup_reference: ExecutionTypeIdentityV1,
        subgroup: ExecutionTypeIdentityV1,
        epoch: ExecutionTypeIdentityV1,
        element: ExecutionTypeIdentityV1,
        value_type: ScalarType,
        width: u32,
    },
    MatrixAccess {
        subgroup: ExecutionTypeIdentityV1,
        epoch: ExecutionTypeIdentityV1,
        matrix: ExecutionTypeIdentityV1,
        subgroup_brand: [u8; 32],
        width: u32,
    },
    AsyncCopy {
        workgroup: ExecutionTypeIdentityV1,
        source_reference: ExecutionTypeIdentityV1,
        source: ExecutionTypeIdentityV1,
        index: ExecutionTypeIdentityV1,
        destination: ExecutionTypeIdentityV1,
        pending: ExecutionTypeIdentityV1,
        element: ExecutionTypeIdentityV1,
        layout: ExecutionElementLayoutV1,
        elements: u64,
    },
    AsyncWait {
        input_workgroup: ExecutionTypeIdentityV1,
        pending: ExecutionTypeIdentityV1,
        output_lds: ExecutionTypeIdentityV1,
        transition: ExecutionTypeIdentityV1,
        element: ExecutionTypeIdentityV1,
        layout: ExecutionElementLayoutV1,
        elements: u64,
    },
    RawMemoryBind {
        authority: ExecutionTypeIdentityV1,
        pointer: ExecutionTypeIdentityV1,
        length: ExecutionTypeIdentityV1,
        extent: ExecutionDynamicExtentV1,
        view: ExecutionTypeIdentityV1,
        element: ExecutionTypeIdentityV1,
        layout: ExecutionElementLayoutV1,
        space: ExecutionMemoryAddressSpaceV1,
        access: ExecutionMemoryAccessV1,
        index_space: Option<ExecutionTypeIdentityV1>,
        atomic_scope: Option<ExecutionMemoryScopeV1>,
        unsafe_obligation: ExecutionTypeIdentityV1,
    },
    PrivateMemoryAllocate {
        context: ExecutionTypeIdentityV1,
        view: ExecutionTypeIdentityV1,
        element: ExecutionTypeIdentityV1,
        layout: ExecutionElementLayoutV1,
        elements: u64,
    },
    WorkgroupMemoryIndex {
        workgroup: ExecutionTypeIdentityV1,
        witness: ExecutionTypeIdentityV1,
    },
    WorkgroupMemoryAllocate {
        workgroup: ExecutionTypeIdentityV1,
        view: ExecutionTypeIdentityV1,
        element: ExecutionTypeIdentityV1,
        layout: ExecutionElementLayoutV1,
        elements: u64,
        index_space: ExecutionTypeIdentityV1,
    },
    WorkgroupMemoryPublish {
        input_workgroup: ExecutionTypeIdentityV1,
        input_view: ExecutionTypeIdentityV1,
        output_view: ExecutionTypeIdentityV1,
        transition: ExecutionTypeIdentityV1,
        element: ExecutionTypeIdentityV1,
        layout: ExecutionElementLayoutV1,
    },
    MemoryLoad {
        view: ExecutionTypeIdentityV1,
        workgroup: Option<ExecutionTypeIdentityV1>,
        index: ExecutionTypeIdentityV1,
        option: ExecutionTypeIdentityV1,
        element: ExecutionTypeIdentityV1,
        layout: ExecutionElementLayoutV1,
        space: ExecutionMemoryAddressSpaceV1,
        access: ExecutionMemoryAccessV1,
    },
    MemoryStore {
        view: ExecutionTypeIdentityV1,
        workgroup: Option<ExecutionTypeIdentityV1>,
        index: ExecutionTypeIdentityV1,
        element: ExecutionTypeIdentityV1,
        layout: ExecutionElementLayoutV1,
        result: ExecutionTypeIdentityV1,
        space: ExecutionMemoryAddressSpaceV1,
        access: ExecutionMemoryAccessV1,
    },
}

impl ExecutionCapabilityOperationV1 {
    pub fn type_references(&self) -> Vec<ExecutionTypeIdentityV1> {
        use ExecutionCapabilityOperationV1 as Op;
        match self {
            Op::WorkgroupDerive { context, workgroup } => vec![*context, *workgroup],
            Op::SubgroupDerive {
                workgroup,
                subgroup,
                ..
            } => vec![*workgroup, *subgroup],
            Op::LdsAllocate {
                workgroup,
                lds,
                element,
                ..
            } => vec![*workgroup, *lds, *element],
            Op::LdsInitializeByInvocation {
                input_lds,
                workgroup,
                output_lds,
                element,
                ..
            } => vec![*input_lds, *workgroup, *output_lds, *element],
            Op::LdsPublish {
                input_workgroup,
                input_lds,
                output_lds,
                transition,
                element,
                ..
            } => vec![
                *input_workgroup,
                *input_lds,
                *output_lds,
                *transition,
                *element,
            ],
            Op::LdsReadPublished {
                lds_reference,
                lds,
                workgroup,
                index,
                option,
                element,
                ..
            } => vec![*lds_reference, *lds, *workgroup, *index, *option, *element],
            Op::WorkgroupBarrier {
                input_workgroup,
                output_workgroup,
                ..
            } => vec![*input_workgroup, *output_workgroup],
            Op::SubgroupBarrier {
                input_workgroup,
                subgroup,
                transition,
                ..
            } => vec![*input_workgroup, *subgroup, *transition],
            Op::WorkgroupFence {
                workgroup, result, ..
            } => vec![*workgroup, *result],
            Op::SubgroupFence {
                subgroup_reference,
                subgroup,
                epoch,
                result,
                ..
            } => vec![*subgroup_reference, *subgroup, *epoch, *result],
            Op::Atomic {
                authority,
                location_input,
                location,
                element,
                operand,
                replacement,
                result,
                ..
            } => {
                let mut values = vec![*authority, *location_input, *location, *element, *result];
                values.extend(operand);
                values.extend(replacement);
                values
            }
            Op::WorkgroupCollective {
                input_workgroup,
                scratch,
                element,
                transition,
                ..
            } => vec![*input_workgroup, *scratch, *element, *transition],
            Op::SubgroupCollective {
                subgroup_reference,
                subgroup,
                epoch,
                element,
                ..
            } => vec![*subgroup_reference, *subgroup, *epoch, *element],
            Op::MatrixAccess {
                subgroup,
                epoch,
                matrix,
                ..
            } => vec![*subgroup, *epoch, *matrix],
            Op::AsyncCopy {
                workgroup,
                source_reference,
                source,
                index,
                destination,
                pending,
                element,
                ..
            } => vec![
                *workgroup,
                *source_reference,
                *source,
                *index,
                *destination,
                *pending,
                *element,
            ],
            Op::AsyncWait {
                input_workgroup,
                pending,
                output_lds,
                transition,
                element,
                ..
            } => vec![
                *input_workgroup,
                *pending,
                *output_lds,
                *transition,
                *element,
            ],
            Op::RawMemoryBind {
                authority,
                pointer,
                length,
                extent,
                view,
                element,
                index_space,
                unsafe_obligation,
                ..
            } => {
                let mut values = vec![
                    *authority,
                    *pointer,
                    *length,
                    extent.source_type,
                    *view,
                    *element,
                    *unsafe_obligation,
                ];
                values.extend(index_space);
                values
            }
            Op::PrivateMemoryAllocate {
                context,
                view,
                element,
                ..
            } => vec![*context, *view, *element],
            Op::WorkgroupMemoryIndex { workgroup, witness } => vec![*workgroup, *witness],
            Op::WorkgroupMemoryAllocate {
                workgroup,
                view,
                element,
                index_space,
                ..
            } => vec![*workgroup, *view, *element, *index_space],
            Op::WorkgroupMemoryPublish {
                input_workgroup,
                input_view,
                output_view,
                transition,
                element,
                ..
            } => vec![
                *input_workgroup,
                *input_view,
                *output_view,
                *transition,
                *element,
            ],
            Op::MemoryLoad {
                view,
                workgroup,
                index,
                option,
                element,
                ..
            } => {
                let mut values = vec![*view, *index, *option, *element];
                values.extend(workgroup);
                values
            }
            Op::MemoryStore {
                view,
                workgroup,
                index,
                element,
                result,
                ..
            } => {
                let mut values = vec![*view, *index, *element, *result];
                values.extend(workgroup);
                values
            }
        }
    }

    pub const fn is_kernel_scoped(&self) -> bool {
        matches!(
            self,
            Self::Atomic {
                kind: ExecutionAtomicKindV1::BindGlobalView,
                ..
            } | Self::RawMemoryBind {
                space: ExecutionMemoryAddressSpaceV1::Private,
                ..
            } | Self::PrivateMemoryAllocate { .. }
                | Self::MemoryLoad {
                    space: ExecutionMemoryAddressSpaceV1::Private,
                    ..
                }
                | Self::MemoryStore {
                    space: ExecutionMemoryAddressSpaceV1::Private,
                    ..
                }
        )
    }

    pub const fn transitions_epoch(&self) -> bool {
        matches!(
            self,
            Self::LdsPublish { .. }
                | Self::WorkgroupBarrier { .. }
                | Self::SubgroupBarrier { .. }
                | Self::WorkgroupCollective { .. }
                | Self::WorkgroupMemoryPublish { .. }
                | Self::AsyncWait { .. }
        )
    }

    pub fn is_well_formed(&self) -> bool {
        use ExecutionAtomicKindV1 as Atomic;
        use ExecutionMemoryOrderingV1 as Ordering;
        use ExecutionMemoryScopeV1 as Scope;
        match self {
            Self::WorkgroupDerive { .. } => true,
            Self::SubgroupDerive { width, .. } | Self::SubgroupCollective { width, .. } => {
                matches!(width, 32 | 64)
            }
            Self::MatrixAccess {
                subgroup_brand,
                width,
                ..
            } => *width != 0 && width.is_power_of_two() && *subgroup_brand != [0; 32],
            Self::LdsAllocate {
                layout, elements, ..
            }
            | Self::LdsInitializeByInvocation {
                layout, elements, ..
            }
            | Self::LdsPublish {
                layout, elements, ..
            }
            | Self::LdsReadPublished {
                layout, elements, ..
            }
            | Self::WorkgroupCollective {
                layout, elements, ..
            }
            | Self::AsyncWait {
                layout, elements, ..
            } => layout.is_complete() && layout.checked_footprint(*elements).is_some(),
            Self::AsyncCopy {
                elements, layout, ..
            } => {
                layout.is_complete()
                    && layout
                        .checked_footprint(*elements)
                        .is_some_and(|bytes| u32::try_from(bytes).is_ok())
            }
            Self::Atomic {
                kind,
                operand,
                replacement,
                result,
                element,
                location,
                address_space,
                scope,
                success,
                failure,
                ..
            } => {
                matches!(
                    address_space,
                    ExecutionMemoryAddressSpaceV1::Global
                        | ExecutionMemoryAddressSpaceV1::Workgroup
                ) && !matches!(scope, Scope::Subgroup)
                    && match kind {
                        Atomic::BindGlobalLocation | Atomic::BindGlobalView => {
                            *address_space == ExecutionMemoryAddressSpaceV1::Global
                                && success.is_none()
                                && failure.is_none()
                                && match kind {
                                    Atomic::BindGlobalLocation => {
                                        operand.is_some() && replacement.is_none()
                                    }
                                    Atomic::BindGlobalView => {
                                        operand.is_none()
                                            && replacement.is_none()
                                            && result == location
                                    }
                                    _ => false,
                                }
                        }
                        Atomic::Load => {
                            operand.is_none()
                                && replacement.is_none()
                                && result == element
                                && failure.is_none()
                                && matches!(
                                    success,
                                    Some(
                                        Ordering::Relaxed
                                            | Ordering::Acquire
                                            | Ordering::SequentiallyConsistent
                                    )
                                )
                        }
                        Atomic::Store => {
                            operand == &Some(*element)
                                && replacement.is_none()
                                && failure.is_none()
                                && matches!(
                                    success,
                                    Some(
                                        Ordering::Relaxed
                                            | Ordering::Release
                                            | Ordering::SequentiallyConsistent
                                    )
                                )
                        }
                        Atomic::FetchAdd => {
                            operand == &Some(*element)
                                && replacement.is_none()
                                && result == element
                                && failure.is_none()
                                && success.is_some()
                        }
                        Atomic::CompareExchange => {
                            operand == &Some(*element)
                                && replacement == &Some(*element)
                                && valid_compare_exchange_order_pair(*success, *failure)
                        }
                    }
            }
            Self::WorkgroupBarrier { semantics, .. } => {
                semantics.scope == Scope::Workgroup
                    && semantics.ordering == Ordering::AcquireRelease
            }
            Self::SubgroupBarrier {
                semantics, width, ..
            } => {
                matches!(width, 32 | 64)
                    && semantics.scope == Scope::Subgroup
                    && semantics.ordering == Ordering::AcquireRelease
            }
            Self::WorkgroupFence { semantics, .. } => {
                semantics.scope == Scope::Workgroup && semantics.ordering != Ordering::Relaxed
            }
            Self::SubgroupFence {
                semantics, width, ..
            } => {
                matches!(width, 32 | 64)
                    && semantics.scope == Scope::Subgroup
                    && semantics.ordering != Ordering::Relaxed
            }
            Self::RawMemoryBind {
                access,
                extent,
                layout,
                index_space,
                atomic_scope,
                ..
            } => {
                extent.is_complete()
                    && layout.is_complete()
                    && matches!(access, ExecutionMemoryAccessV1::DisjointWrite)
                        == index_space.is_some()
                    && matches!(access, ExecutionMemoryAccessV1::AtomicReadWrite)
                        == atomic_scope.is_some()
            }
            Self::PrivateMemoryAllocate {
                elements, layout, ..
            }
            | Self::WorkgroupMemoryAllocate {
                elements, layout, ..
            } => layout.is_complete() && layout.checked_footprint(*elements).is_some(),
            Self::WorkgroupMemoryIndex { .. } => true,
            Self::WorkgroupMemoryPublish { layout, .. } => layout.is_complete(),
            Self::MemoryLoad {
                workgroup,
                space,
                access,
                layout,
                ..
            } => {
                layout.is_complete()
                    && workgroup.is_some()
                        == matches!(space, ExecutionMemoryAddressSpaceV1::Workgroup)
                    && matches!(
                        access,
                        ExecutionMemoryAccessV1::ReadOnly
                            | ExecutionMemoryAccessV1::ExclusiveReadWrite
                    )
            }
            Self::MemoryStore {
                workgroup,
                space,
                access,
                layout,
                ..
            } => {
                layout.is_complete()
                    && workgroup.is_some()
                        == matches!(space, ExecutionMemoryAddressSpaceV1::Workgroup)
                    && matches!(
                        access,
                        ExecutionMemoryAccessV1::ExclusiveReadWrite
                            | ExecutionMemoryAccessV1::DisjointWrite
                    )
            }
        }
    }

    pub fn signature_matches(&self, signature: ExecutionCapabilitySignatureV1) -> bool {
        let matches = |arguments: &[ExecutionTypeIdentityV1], output| {
            signature.arguments().eq(arguments.iter().copied()) && signature.output() == output
        };
        match self {
            Self::WorkgroupDerive { context, workgroup } => matches(&[*context], *workgroup),
            Self::SubgroupDerive {
                workgroup,
                subgroup,
                ..
            } => matches(&[*workgroup], *subgroup),
            Self::LdsAllocate { workgroup, lds, .. } => matches(&[*workgroup], *lds),
            Self::LdsInitializeByInvocation {
                input_lds,
                workgroup,
                output_lds,
                element,
                ..
            } => matches(&[*input_lds, *workgroup, *element], *output_lds),
            Self::LdsPublish {
                input_workgroup,
                input_lds,
                transition,
                ..
            } => matches(&[*input_workgroup, *input_lds], *transition),
            Self::LdsReadPublished {
                lds_reference,
                workgroup,
                index,
                option,
                ..
            } => matches(&[*lds_reference, *workgroup, *index], *option),
            Self::WorkgroupBarrier {
                input_workgroup,
                output_workgroup,
                ..
            } => matches(&[*input_workgroup], *output_workgroup),
            Self::SubgroupBarrier {
                input_workgroup,
                subgroup,
                transition,
                ..
            } => matches(&[*input_workgroup, *subgroup], *transition),
            Self::WorkgroupFence {
                workgroup, result, ..
            } => matches(&[*workgroup], *result),
            Self::SubgroupFence {
                subgroup_reference,
                epoch,
                result,
                ..
            } => matches(&[*subgroup_reference, *epoch], *result),
            Self::Atomic {
                kind,
                authority,
                location_input,
                operand,
                replacement,
                result,
                ..
            } => {
                match kind {
                    ExecutionAtomicKindV1::BindGlobalView | ExecutionAtomicKindV1::Load => {
                        matches(&[*authority, *location_input], *result)
                    }
                    ExecutionAtomicKindV1::BindGlobalLocation
                    | ExecutionAtomicKindV1::Store
                    | ExecutionAtomicKindV1::FetchAdd => operand.is_some_and(|operand| {
                        replacement.is_none()
                            && matches(&[*authority, *location_input, operand], *result)
                    }),
                    ExecutionAtomicKindV1::CompareExchange => operand
                        .zip(*replacement)
                        .is_some_and(|(operand, replacement)| {
                            matches(
                                &[*authority, *location_input, operand, replacement],
                                *result,
                            )
                        }),
                }
            }
            Self::WorkgroupCollective {
                input_workgroup,
                scratch,
                element,
                transition,
                ..
            } => matches(&[*input_workgroup, *scratch, *element], *transition),
            Self::SubgroupCollective {
                subgroup_reference,
                epoch,
                element,
                ..
            } => matches(&[*subgroup_reference, *epoch, *element], *element),
            Self::MatrixAccess {
                subgroup,
                epoch,
                matrix,
                ..
            } => matches(&[*subgroup, *epoch], *matrix),
            Self::AsyncCopy {
                workgroup,
                source_reference,
                index,
                destination,
                pending,
                ..
            } => matches(
                &[*workgroup, *source_reference, *index, *destination],
                *pending,
            ),
            Self::AsyncWait {
                input_workgroup,
                pending,
                transition,
                ..
            } => matches(&[*input_workgroup, *pending], *transition),
            Self::RawMemoryBind {
                authority,
                pointer,
                length,
                view,
                unsafe_obligation,
                ..
            } => matches(&[*authority, *pointer, *length, *unsafe_obligation], *view),
            Self::PrivateMemoryAllocate { context, view, .. } => matches(&[*context], *view),
            Self::WorkgroupMemoryIndex { workgroup, witness } => matches(&[*workgroup], *witness),
            Self::WorkgroupMemoryAllocate {
                workgroup, view, ..
            } => matches(&[*workgroup], *view),
            Self::WorkgroupMemoryPublish {
                input_workgroup,
                input_view,
                transition,
                ..
            } => matches(&[*input_workgroup, *input_view], *transition),
            Self::MemoryLoad {
                view,
                workgroup,
                index,
                option,
                ..
            } => match workgroup {
                None => matches(&[*view, *index], *option),
                Some(workgroup) => matches(&[*view, *workgroup, *index], *option),
            },
            Self::MemoryStore {
                view,
                workgroup,
                index,
                element,
                result,
                ..
            } => match workgroup {
                None => matches(&[*view, *index, *element], *result),
                Some(workgroup) => matches(&[*view, *workgroup, *index, *element], *result),
            },
        }
    }

    pub fn memory_effects(&self) -> Vec<MemoryEffect> {
        match self {
            Self::LdsAllocate { .. } | Self::WorkgroupMemoryAllocate { .. } => {
                vec![MemoryEffect::Allocate(AddressSpace::Workgroup)]
            }
            Self::PrivateMemoryAllocate { .. } => {
                vec![MemoryEffect::Allocate(AddressSpace::Private)]
            }
            Self::LdsInitializeByInvocation { .. }
            | Self::LdsPublish { .. }
            | Self::WorkgroupMemoryPublish { .. } => {
                vec![MemoryEffect::Write(AddressSpace::Workgroup)]
            }
            Self::LdsReadPublished { .. } => vec![MemoryEffect::Read(AddressSpace::Workgroup)],
            Self::WorkgroupBarrier { semantics, .. } | Self::SubgroupBarrier { semantics, .. } => {
                vec![MemoryEffect::Synchronize {
                    execution_scope: semantics.scope.synchronization_scope(),
                    memory_scope: semantics.scope.synchronization_scope(),
                    address_spaces: semantics.spaces.address_spaces(),
                }]
            }
            Self::WorkgroupFence { semantics, .. } | Self::SubgroupFence { semantics, .. } => {
                vec![MemoryEffect::Fence {
                    memory_scope: semantics.scope.synchronization_scope(),
                    ordering: semantics.ordering.memory_ordering(),
                    address_spaces: semantics.spaces.address_spaces(),
                }]
            }
            Self::Atomic {
                kind,
                address_space,
                scope,
                success,
                ..
            } => kind.atomic_kind().map_or_else(Vec::new, |_| {
                vec![MemoryEffect::Atomic {
                    address_space: address_space.address_space(),
                    scope: scope.synchronization_scope(),
                    ordering: success
                        .unwrap_or(ExecutionMemoryOrderingV1::Relaxed)
                        .memory_ordering(),
                }]
            }),
            Self::AsyncCopy { .. } => vec![
                MemoryEffect::Read(AddressSpace::Global),
                MemoryEffect::Write(AddressSpace::Workgroup),
            ],
            Self::MemoryLoad { space, .. } => vec![MemoryEffect::Read(space.address_space())],
            Self::MemoryStore { space, .. } => vec![MemoryEffect::Write(space.address_space())],
            Self::WorkgroupCollective { .. } => vec![
                MemoryEffect::Read(AddressSpace::Workgroup),
                MemoryEffect::Write(AddressSpace::Workgroup),
            ],
            Self::WorkgroupDerive { .. }
            | Self::SubgroupDerive { .. }
            | Self::SubgroupCollective { .. }
            | Self::MatrixAccess { .. }
            | Self::AsyncWait { .. }
            | Self::RawMemoryBind { .. }
            | Self::WorkgroupMemoryIndex { .. } => Vec::new(),
        }
    }

    pub fn required_capabilities(&self) -> BTreeSet<TargetCapability> {
        let mut required = BTreeSet::new();
        if let Self::SubgroupDerive { width, .. }
        | Self::SubgroupBarrier { width, .. }
        | Self::SubgroupFence { width, .. }
        | Self::SubgroupCollective { width, .. }
        | Self::MatrixAccess { width, .. } = self
        {
            required.insert(TargetCapability::Subgroups);
            required.insert(TargetCapability::SubgroupSize(*width));
        }
        let mut address = |space: AddressSpace, access: AccessMode| {
            required.insert(TargetCapability::Execution(
                ExecutionCapabilityRequirementV1::AddressSpace {
                    address_space: space,
                    access,
                },
            ));
        };
        match self {
            Self::SubgroupDerive { .. } => {}
            Self::LdsAllocate {
                layout, elements, ..
            }
            | Self::LdsInitializeByInvocation {
                layout, elements, ..
            }
            | Self::LdsPublish {
                layout, elements, ..
            }
            | Self::LdsReadPublished {
                layout, elements, ..
            }
            | Self::AsyncWait {
                layout, elements, ..
            } => {
                required.insert(TargetCapability::WorkgroupMemory);
                if let Some(bytes) = layout.checked_footprint(*elements) {
                    required.insert(TargetCapability::Execution(
                        ExecutionCapabilityRequirementV1::Resource(
                            ResourceCapabilityRequirementV1::StaticWorkgroupMemoryBytesAtMost(
                                bytes,
                            ),
                        ),
                    ));
                }
            }
            Self::WorkgroupBarrier { semantics, .. } | Self::SubgroupBarrier { semantics, .. } => {
                required.insert(TargetCapability::Execution(
                    ExecutionCapabilityRequirementV1::Barrier {
                        execution_scope: semantics.scope.synchronization_scope(),
                        memory_scope: semantics.scope.synchronization_scope(),
                        ordering: semantics.ordering.memory_ordering(),
                        address_spaces: semantics.spaces.address_spaces(),
                    },
                ));
            }
            Self::Atomic {
                kind,
                value_type,
                address_space,
                scope,
                success,
                failure,
                ..
            } => {
                if let Some(operation) = kind.atomic_kind() {
                    required.insert(TargetCapability::Execution(
                        ExecutionCapabilityRequirementV1::Atomic {
                            value_type: *value_type,
                            operation,
                            ordering: success
                                .unwrap_or(ExecutionMemoryOrderingV1::Relaxed)
                                .memory_ordering(),
                            failure_ordering: failure
                                .map(ExecutionMemoryOrderingV1::memory_ordering),
                            scope: scope.synchronization_scope(),
                            address_space: address_space.address_space(),
                        },
                    ));
                }
            }
            Self::WorkgroupCollective {
                kind,
                value_type,
                elements,
                ..
            } => {
                required.insert(TargetCapability::Execution(
                    ExecutionCapabilityRequirementV1::Collective {
                        execution_scope: SynchronizationScope::Workgroup,
                        operation: kind.requirement(),
                        value_type: *value_type,
                        participants: u32::try_from(*elements).unwrap_or(u32::MAX),
                    },
                ));
            }
            Self::SubgroupCollective {
                kind,
                value_type,
                width,
                ..
            } => {
                required.insert(TargetCapability::Execution(
                    ExecutionCapabilityRequirementV1::Collective {
                        execution_scope: SynchronizationScope::Subgroup,
                        operation: kind.requirement(),
                        value_type: *value_type,
                        participants: *width,
                    },
                ));
            }
            Self::AsyncCopy {
                layout, elements, ..
            } => {
                if let Some(bytes) = layout
                    .checked_footprint(*elements)
                    .and_then(|bytes| u32::try_from(bytes).ok())
                {
                    required.insert(TargetCapability::Execution(
                        ExecutionCapabilityRequirementV1::AsyncCopy {
                            source: AddressSpace::Global,
                            destination: AddressSpace::Workgroup,
                            bytes,
                            alignment: layout.byte_alignment,
                            completion: AsyncCopyCompletionV1::WorkgroupBarrier,
                        },
                    ));
                }
            }
            Self::PrivateMemoryAllocate {
                layout, elements, ..
            } => {
                if let Some(bytes) = layout.checked_footprint(*elements) {
                    required.insert(TargetCapability::Execution(
                        ExecutionCapabilityRequirementV1::Resource(
                            ResourceCapabilityRequirementV1::PrivateMemoryBytesPerInvocationAtMost(
                                bytes,
                            ),
                        ),
                    ));
                }
            }
            Self::WorkgroupMemoryAllocate {
                layout, elements, ..
            } => {
                required.insert(TargetCapability::WorkgroupMemory);
                if let Some(bytes) = layout.checked_footprint(*elements) {
                    required.insert(TargetCapability::Execution(
                        ExecutionCapabilityRequirementV1::Resource(
                            ResourceCapabilityRequirementV1::StaticWorkgroupMemoryBytesAtMost(
                                bytes,
                            ),
                        ),
                    ));
                }
            }
            Self::RawMemoryBind { space, access, .. }
            | Self::MemoryLoad { space, access, .. }
            | Self::MemoryStore { space, access, .. } => {
                address(space.address_space(), access.access_mode())
            }
            Self::WorkgroupDerive { .. }
            | Self::WorkgroupFence { .. }
            | Self::SubgroupFence { .. }
            | Self::MatrixAccess { .. }
            | Self::WorkgroupMemoryPublish { .. }
            | Self::WorkgroupMemoryIndex { .. } => {}
        }
        required
    }
}

const fn valid_compare_exchange_order_pair(
    success: Option<ExecutionMemoryOrderingV1>,
    failure: Option<ExecutionMemoryOrderingV1>,
) -> bool {
    use ExecutionMemoryOrderingV1 as Ordering;
    matches!(
        (success, failure),
        (Some(Ordering::Relaxed), Some(Ordering::Relaxed))
            | (
                Some(Ordering::Acquire),
                Some(Ordering::Relaxed | Ordering::Acquire)
            )
            | (Some(Ordering::Release), Some(Ordering::Relaxed))
            | (
                Some(Ordering::AcquireRelease),
                Some(Ordering::Relaxed | Ordering::Acquire)
            )
            | (
                Some(Ordering::SequentiallyConsistent),
                Some(Ordering::Relaxed | Ordering::Acquire | Ordering::SequentiallyConsistent)
            )
    )
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExecutionCapabilitySignatureV1 {
    arguments: [Option<ExecutionTypeIdentityV1>; MAX_EXECUTION_CAPABILITY_ARGUMENTS_V1],
    output: ExecutionTypeIdentityV1,
}

impl ExecutionCapabilitySignatureV1 {
    pub fn new(
        arguments: &[ExecutionTypeIdentityV1],
        output: ExecutionTypeIdentityV1,
    ) -> Option<Self> {
        if arguments.len() > MAX_EXECUTION_CAPABILITY_ARGUMENTS_V1 {
            return None;
        }
        let mut bounded = [None; MAX_EXECUTION_CAPABILITY_ARGUMENTS_V1];
        for (slot, argument) in bounded.iter_mut().zip(arguments) {
            *slot = Some(*argument);
        }
        Some(Self {
            arguments: bounded,
            output,
        })
    }

    pub fn arguments(self) -> impl Iterator<Item = ExecutionTypeIdentityV1> {
        self.arguments.into_iter().flatten()
    }

    pub const fn output(self) -> ExecutionTypeIdentityV1 {
        self.output
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExecutionCapabilitySourceV1 {
    pub function: [u8; 32],
    pub operation: [u8; 32],
    pub block: u32,
}

impl ExecutionCapabilitySourceV1 {
    pub fn is_complete(self) -> bool {
        self.function != [0; 32] && self.operation != [0; 32]
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExecutionSafetyObligationsV1(u16);

impl ExecutionSafetyObligationsV1 {
    pub const TARGET_SUPPORT: u16 = 1 << 0;
    pub const DYNAMIC_WORKGROUP_IDENTITY: u16 = 1 << 1;
    pub const WORKGROUP_CONVERGENCE: u16 = 1 << 2;
    pub const SUBGROUP_CONVERGENCE: u16 = 1 << 3;
    pub const BOUNDS: u16 = 1 << 4;
    pub const DISJOINT_LDS_ALLOCATION: u16 = 1 << 5;
    pub const INITIALIZATION: u16 = 1 << 6;
    pub const RACE_FREEDOM: u16 = 1 << 7;
    pub const EXACT_PARTICIPATION: u16 = 1 << 8;
    pub const ASYNC_COMPLETION: u16 = 1 << 9;
    pub const MEMORY_MODEL: u16 = 1 << 10;
    pub const MATRIX_LEGALITY: u16 = 1 << 11;
    pub const RAW_POINTER_VALIDITY: u16 = 1 << 12;
    pub const LIFETIME_VALIDITY: u16 = 1 << 13;
    pub const ADDRESS_SPACE_VALIDITY: u16 = 1 << 14;
    pub const ALIASING_VALIDITY: u16 = 1 << 15;

    pub const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionCapabilityOpV1 {
    pub operands: Vec<ValueId>,
    pub operation: ExecutionCapabilityOperationV1,
    pub signature: ExecutionCapabilitySignatureV1,
    pub provenance: ExecutionCapabilityProvenanceV1,
    pub workgroup_brand: Option<[u8; 32]>,
    pub epoch_before: Option<[u8; 32]>,
    pub epoch_after: Option<[u8; 32]>,
    pub obligations: ExecutionSafetyObligationsV1,
    pub source: ExecutionCapabilitySourceV1,
}

impl ExecutionCapabilityOpV1 {
    pub fn is_complete(&self) -> bool {
        self.operands.len() <= MAX_EXECUTION_CAPABILITY_OPERANDS_V1
            && self.provenance.is_complete()
            && self.source.is_complete()
            && self.operation.is_well_formed()
            && self.operation.signature_matches(self.signature)
            && self
                .operation
                .type_references()
                .into_iter()
                .all(ExecutionTypeIdentityV1::is_complete)
            && self
                .signature
                .arguments()
                .all(ExecutionTypeIdentityV1::is_complete)
            && self.signature.output().is_complete()
            && self.operation.is_kernel_scoped() == self.workgroup_brand.is_none()
            && self.workgroup_brand.is_some() == self.epoch_before.is_some()
            && self
                .workgroup_brand
                .is_none_or(|identity| identity != [0; 32])
            && self.epoch_before.is_none_or(|identity| identity != [0; 32])
            && self.operation.transitions_epoch() == self.epoch_after.is_some()
            && self
                .epoch_after
                .is_none_or(|identity| identity != [0; 32] && Some(identity) != self.epoch_before)
            && self.obligations.bits() == required_execution_obligations_v1(&self.operation)
            && match &self.operation {
                ExecutionCapabilityOperationV1::RawMemoryBind { extent, .. } => {
                    usize::from(extent.operand) < self.operands.len()
                        && usize::from(extent.bound_check_operand) < self.operands.len()
                        && extent
                            .nonnegative_check_operand
                            .is_none_or(|operand| usize::from(operand) < self.operands.len())
                        && self
                            .signature
                            .arguments()
                            .nth(usize::from(extent.source_argument))
                            == Some(extent.source_type)
                }
                _ => true,
            }
    }
}

pub const fn required_execution_obligations_v1(operation: &ExecutionCapabilityOperationV1) -> u16 {
    let kernel = ExecutionSafetyObligationsV1::TARGET_SUPPORT;
    let base = kernel | ExecutionSafetyObligationsV1::DYNAMIC_WORKGROUP_IDENTITY;
    match operation {
        ExecutionCapabilityOperationV1::WorkgroupDerive { .. }
        | ExecutionCapabilityOperationV1::SubgroupDerive { .. } => base,
        ExecutionCapabilityOperationV1::LdsAllocate { .. } => {
            base | ExecutionSafetyObligationsV1::DISJOINT_LDS_ALLOCATION
        }
        ExecutionCapabilityOperationV1::LdsInitializeByInvocation { .. } => {
            base | ExecutionSafetyObligationsV1::BOUNDS
                | ExecutionSafetyObligationsV1::INITIALIZATION
                | ExecutionSafetyObligationsV1::RACE_FREEDOM
                | ExecutionSafetyObligationsV1::EXACT_PARTICIPATION
        }
        ExecutionCapabilityOperationV1::LdsPublish { .. }
        | ExecutionCapabilityOperationV1::WorkgroupBarrier { .. } => {
            base | ExecutionSafetyObligationsV1::WORKGROUP_CONVERGENCE
                | ExecutionSafetyObligationsV1::MEMORY_MODEL
        }
        ExecutionCapabilityOperationV1::LdsReadPublished { .. } => {
            base | ExecutionSafetyObligationsV1::BOUNDS
                | ExecutionSafetyObligationsV1::INITIALIZATION
                | ExecutionSafetyObligationsV1::RACE_FREEDOM
        }
        ExecutionCapabilityOperationV1::SubgroupBarrier { .. } => {
            base | ExecutionSafetyObligationsV1::SUBGROUP_CONVERGENCE
                | ExecutionSafetyObligationsV1::MEMORY_MODEL
        }
        ExecutionCapabilityOperationV1::WorkgroupFence { .. }
        | ExecutionCapabilityOperationV1::SubgroupFence { .. } => {
            base | ExecutionSafetyObligationsV1::MEMORY_MODEL
        }
        ExecutionCapabilityOperationV1::Atomic {
            kind: ExecutionAtomicKindV1::BindGlobalView,
            ..
        } => {
            kernel
                | ExecutionSafetyObligationsV1::BOUNDS
                | ExecutionSafetyObligationsV1::RACE_FREEDOM
                | ExecutionSafetyObligationsV1::MEMORY_MODEL
                | ExecutionSafetyObligationsV1::ADDRESS_SPACE_VALIDITY
                | ExecutionSafetyObligationsV1::ALIASING_VALIDITY
        }
        ExecutionCapabilityOperationV1::Atomic { .. } => {
            base | ExecutionSafetyObligationsV1::BOUNDS
                | ExecutionSafetyObligationsV1::MEMORY_MODEL
                | ExecutionSafetyObligationsV1::RACE_FREEDOM
        }
        ExecutionCapabilityOperationV1::WorkgroupCollective { .. } => {
            base | ExecutionSafetyObligationsV1::WORKGROUP_CONVERGENCE
                | ExecutionSafetyObligationsV1::EXACT_PARTICIPATION
                | ExecutionSafetyObligationsV1::RACE_FREEDOM
        }
        ExecutionCapabilityOperationV1::SubgroupCollective { .. } => {
            base | ExecutionSafetyObligationsV1::SUBGROUP_CONVERGENCE
                | ExecutionSafetyObligationsV1::EXACT_PARTICIPATION
        }
        ExecutionCapabilityOperationV1::MatrixAccess { .. } => {
            base | ExecutionSafetyObligationsV1::SUBGROUP_CONVERGENCE
                | ExecutionSafetyObligationsV1::MATRIX_LEGALITY
        }
        ExecutionCapabilityOperationV1::AsyncCopy { .. } => {
            base | ExecutionSafetyObligationsV1::BOUNDS
                | ExecutionSafetyObligationsV1::DISJOINT_LDS_ALLOCATION
                | ExecutionSafetyObligationsV1::ASYNC_COMPLETION
        }
        ExecutionCapabilityOperationV1::AsyncWait { .. } => {
            base | ExecutionSafetyObligationsV1::WORKGROUP_CONVERGENCE
                | ExecutionSafetyObligationsV1::ASYNC_COMPLETION
                | ExecutionSafetyObligationsV1::MEMORY_MODEL
        }
        ExecutionCapabilityOperationV1::RawMemoryBind { space, access, .. } => {
            let scoped = if matches!(space, ExecutionMemoryAddressSpaceV1::Private) {
                kernel
            } else {
                base
            };
            let initialization = if matches!(access, ExecutionMemoryAccessV1::DisjointWrite) {
                0
            } else {
                ExecutionSafetyObligationsV1::INITIALIZATION
            };
            scoped
                | ExecutionSafetyObligationsV1::BOUNDS
                | ExecutionSafetyObligationsV1::RACE_FREEDOM
                | ExecutionSafetyObligationsV1::RAW_POINTER_VALIDITY
                | ExecutionSafetyObligationsV1::LIFETIME_VALIDITY
                | ExecutionSafetyObligationsV1::ADDRESS_SPACE_VALIDITY
                | ExecutionSafetyObligationsV1::ALIASING_VALIDITY
                | initialization
        }
        ExecutionCapabilityOperationV1::PrivateMemoryAllocate { .. } => {
            kernel
                | ExecutionSafetyObligationsV1::BOUNDS
                | ExecutionSafetyObligationsV1::INITIALIZATION
        }
        ExecutionCapabilityOperationV1::WorkgroupMemoryIndex { .. } => {
            base | ExecutionSafetyObligationsV1::BOUNDS
                | ExecutionSafetyObligationsV1::EXACT_PARTICIPATION
        }
        ExecutionCapabilityOperationV1::WorkgroupMemoryAllocate { .. } => {
            base | ExecutionSafetyObligationsV1::BOUNDS
                | ExecutionSafetyObligationsV1::DISJOINT_LDS_ALLOCATION
                | ExecutionSafetyObligationsV1::RACE_FREEDOM
                | ExecutionSafetyObligationsV1::EXACT_PARTICIPATION
        }
        ExecutionCapabilityOperationV1::WorkgroupMemoryPublish { .. } => {
            base | ExecutionSafetyObligationsV1::WORKGROUP_CONVERGENCE
                | ExecutionSafetyObligationsV1::INITIALIZATION
                | ExecutionSafetyObligationsV1::RACE_FREEDOM
                | ExecutionSafetyObligationsV1::EXACT_PARTICIPATION
                | ExecutionSafetyObligationsV1::MEMORY_MODEL
        }
        ExecutionCapabilityOperationV1::MemoryLoad { space, .. } => {
            let scoped = if matches!(space, ExecutionMemoryAddressSpaceV1::Private) {
                kernel
            } else {
                base
            };
            scoped
                | ExecutionSafetyObligationsV1::BOUNDS
                | ExecutionSafetyObligationsV1::INITIALIZATION
                | ExecutionSafetyObligationsV1::RACE_FREEDOM
        }
        ExecutionCapabilityOperationV1::MemoryStore { space, access, .. } => {
            let scoped = if matches!(space, ExecutionMemoryAddressSpaceV1::Private) {
                kernel
            } else {
                base
            };
            let participation = if matches!(access, ExecutionMemoryAccessV1::DisjointWrite) {
                ExecutionSafetyObligationsV1::EXACT_PARTICIPATION
            } else {
                0
            };
            scoped
                | ExecutionSafetyObligationsV1::BOUNDS
                | ExecutionSafetyObligationsV1::RACE_FREEDOM
                | participation
        }
    }
}

/// Encodes only the immutable semantic contract. SSA operands remain live graph
/// edges and are encoded by the enclosing KIR operation.
pub fn encode_execution_capability_contract_v1(
    contract: &ExecutionCapabilityOpV1,
) -> Option<Vec<u8>> {
    if !contract.is_complete() {
        return None;
    }
    let mut writer = ContractWriter::default();
    writer.u8(1);
    encode_execution_operation(&mut writer, &contract.operation);
    let arguments = contract.signature.arguments().collect::<Vec<_>>();
    writer.u8(u8::try_from(arguments.len()).ok()?);
    for argument in arguments {
        writer.identity(argument);
    }
    writer.identity(contract.signature.output());
    encode_execution_provenance(&mut writer, &contract.provenance)?;
    writer.optional_digest(contract.workgroup_brand);
    writer.optional_digest(contract.epoch_before);
    writer.optional_digest(contract.epoch_after);
    writer.u16(contract.obligations.bits());
    writer.digest(contract.source.function);
    writer.digest(contract.source.operation);
    writer.u32(contract.source.block);
    (writer.bytes.len() <= MAX_EXECUTION_CAPABILITY_CONTRACT_BYTES_V1).then_some(writer.bytes)
}

pub fn decode_execution_capability_contract_v1(
    bytes: &[u8],
    operands: Vec<ValueId>,
) -> Option<ExecutionCapabilityOpV1> {
    if bytes.len() > MAX_EXECUTION_CAPABILITY_CONTRACT_BYTES_V1 {
        return None;
    }
    let mut reader = ContractReader::new(bytes);
    (reader.u8()? == 1).then_some(())?;
    let operation = decode_execution_operation(&mut reader)?;
    let count = usize::from(reader.u8()?);
    (count <= MAX_EXECUTION_CAPABILITY_ARGUMENTS_V1).then_some(())?;
    let mut arguments = Vec::with_capacity(count);
    for _ in 0..count {
        arguments.push(reader.identity()?);
    }
    let signature = ExecutionCapabilitySignatureV1::new(&arguments, reader.identity()?)?;
    let provenance = decode_execution_provenance(&mut reader)?;
    let workgroup_brand = reader.optional_digest()?;
    let epoch_before = reader.optional_digest()?;
    let epoch_after = reader.optional_digest()?;
    let obligations = ExecutionSafetyObligationsV1::from_bits(reader.u16()?);
    let source = ExecutionCapabilitySourceV1 {
        function: reader.digest()?,
        operation: reader.digest()?,
        block: reader.u32()?,
    };
    reader.finished().then_some(())?;
    let contract = ExecutionCapabilityOpV1 {
        operands,
        operation,
        signature,
        provenance,
        workgroup_brand,
        epoch_before,
        epoch_after,
        obligations,
        source,
    };
    contract.is_complete().then_some(contract)
}

pub fn encode_execution_capability_type_v1(
    capability: &ExecutionCapabilityTypeV1,
) -> Option<Vec<u8>> {
    if !capability.is_complete() {
        return None;
    }
    let mut writer = ContractWriter::default();
    writer.u8(1);
    writer.identity(capability.source_type);
    encode_execution_provenance(&mut writer, &capability.provenance)?;
    writer.optional_digest(capability.workgroup_brand);
    writer.optional_digest(capability.epoch);
    encode_execution_role(&mut writer, &capability.role);
    (writer.bytes.len() <= MAX_EXECUTION_CAPABILITY_TYPE_BYTES_V1).then_some(writer.bytes)
}

pub fn decode_execution_capability_type_v1(bytes: &[u8]) -> Option<ExecutionCapabilityTypeV1> {
    if bytes.len() > MAX_EXECUTION_CAPABILITY_TYPE_BYTES_V1 {
        return None;
    }
    let mut reader = ContractReader::new(bytes);
    (reader.u8()? == 1).then_some(())?;
    let capability = ExecutionCapabilityTypeV1 {
        source_type: reader.identity()?,
        provenance: decode_execution_provenance(&mut reader)?,
        workgroup_brand: reader.optional_digest()?,
        epoch: reader.optional_digest()?,
        role: decode_execution_role(&mut reader)?,
    };
    (reader.finished() && capability.is_complete()).then_some(capability)
}

#[derive(Default)]
struct ContractWriter {
    bytes: Vec<u8>,
}

impl ContractWriter {
    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn digest(&mut self, value: [u8; 32]) {
        self.bytes.extend_from_slice(&value);
    }

    fn identity(&mut self, value: ExecutionTypeIdentityV1) {
        self.digest(value.bytes());
    }

    fn optional_digest(&mut self, value: Option<[u8; 32]>) {
        match value {
            None => self.u8(0),
            Some(value) => {
                self.u8(1);
                self.digest(value);
            }
        }
    }
}

struct ContractReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ContractReader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let end = self.offset.checked_add(N)?;
        let value = self.bytes.get(self.offset..end)?.try_into().ok()?;
        self.offset = end;
        Some(value)
    }

    fn u8(&mut self) -> Option<u8> {
        Some(self.take::<1>()?[0])
    }

    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take()?))
    }

    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take()?))
    }

    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take()?))
    }

    fn digest(&mut self) -> Option<[u8; 32]> {
        self.take()
    }

    fn identity(&mut self) -> Option<ExecutionTypeIdentityV1> {
        Some(ExecutionTypeIdentityV1::new(self.digest()?))
    }

    fn optional_digest(&mut self) -> Option<Option<[u8; 32]>> {
        match self.u8()? {
            0 => Some(None),
            1 => Some(Some(self.digest()?)),
            _ => None,
        }
    }

    const fn finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

fn encode_execution_provenance(
    writer: &mut ContractWriter,
    provenance: &ExecutionCapabilityProvenanceV1,
) -> Option<()> {
    let root = provenance.root.as_str().as_bytes();
    let length = u16::try_from(root.len()).ok()?;
    (length != 0 && length <= 256).then_some(())?;
    writer.u16(length);
    writer.bytes.extend_from_slice(root);
    writer.digest(provenance.kernel_binding);
    writer.digest(provenance.frontend_unit);
    writer.digest(provenance.kernel_marker);
    writer.digest(provenance.target_brand);
    writer.digest(provenance.launch_brand);
    writer.digest(provenance.issuance);
    Some(())
}

fn decode_execution_provenance(
    reader: &mut ContractReader<'_>,
) -> Option<ExecutionCapabilityProvenanceV1> {
    let length = usize::from(reader.u16()?);
    (length != 0 && length <= 256).then_some(())?;
    let end = reader.offset.checked_add(length)?;
    let root = std::str::from_utf8(reader.bytes.get(reader.offset..end)?).ok()?;
    reader.offset = end;
    let provenance = ExecutionCapabilityProvenanceV1 {
        root: FunctionId::new(root),
        kernel_binding: reader.digest()?,
        frontend_unit: reader.digest()?,
        kernel_marker: reader.digest()?,
        target_brand: reader.digest()?,
        launch_brand: reader.digest()?,
        issuance: reader.digest()?,
    };
    provenance.is_complete().then_some(provenance)
}

fn encode_execution_role(writer: &mut ContractWriter, role: &ExecutionCapabilityRoleV1) {
    match role {
        ExecutionCapabilityRoleV1::KernelAuthority => writer.u8(0),
        ExecutionCapabilityRoleV1::Workgroup => writer.u8(1),
        ExecutionCapabilityRoleV1::Subgroup { width } => {
            writer.u8(2);
            writer.u32(*width);
        }
        ExecutionCapabilityRoleV1::Lds {
            element,
            layout,
            elements,
            state,
        } => {
            writer.u8(3);
            writer.identity(*element);
            put_layout(writer, *layout);
            writer.u64(*elements);
            writer.u8(lds_state_tag(*state));
        }
        ExecutionCapabilityRoleV1::ScopedAtomic {
            element,
            space,
            scope,
        } => {
            writer.u8(4);
            writer.identity(*element);
            writer.u8(address_space_tag(*space));
            writer.u8(scope_tag(*scope));
        }
        ExecutionCapabilityRoleV1::Matrix {
            subgroup_brand,
            width,
        } => {
            writer.u8(5);
            writer.digest(*subgroup_brand);
            writer.u32(*width);
        }
        ExecutionCapabilityRoleV1::PendingAsyncCopy {
            element,
            layout,
            elements,
        } => {
            writer.u8(6);
            writer.identity(*element);
            put_layout(writer, *layout);
            writer.u64(*elements);
        }
        ExecutionCapabilityRoleV1::MemoryView {
            element,
            layout,
            space,
            access,
            extent,
            initialization,
            index_space,
            atomic_scope,
        } => {
            writer.u8(7);
            writer.identity(*element);
            put_layout(writer, *layout);
            writer.u8(address_space_tag(*space));
            writer.u8(access_tag(*access));
            match extent {
                ExecutionMemoryExtentV1::Static(elements) => {
                    writer.u8(0);
                    writer.u64(*elements);
                }
                ExecutionMemoryExtentV1::Dynamic(extent) => {
                    writer.u8(1);
                    put_dynamic_extent(writer, *extent);
                }
            }
            writer.u8(initialization_tag(*initialization));
            put_optional_identity(writer, *index_space);
            put_optional_scope(writer, *atomic_scope);
        }
        ExecutionCapabilityRoleV1::WorkgroupMemoryIndex => writer.u8(8),
        ExecutionCapabilityRoleV1::EpochTransition => writer.u8(9),
        ExecutionCapabilityRoleV1::UnsafeRawMemoryObligation => writer.u8(10),
    }
}

fn decode_execution_role(reader: &mut ContractReader<'_>) -> Option<ExecutionCapabilityRoleV1> {
    Some(match reader.u8()? {
        0 => ExecutionCapabilityRoleV1::KernelAuthority,
        1 => ExecutionCapabilityRoleV1::Workgroup,
        2 => ExecutionCapabilityRoleV1::Subgroup {
            width: reader.u32()?,
        },
        3 => ExecutionCapabilityRoleV1::Lds {
            element: reader.identity()?,
            layout: get_layout(reader)?,
            elements: reader.u64()?,
            state: decode_lds_state(reader.u8()?)?,
        },
        4 => ExecutionCapabilityRoleV1::ScopedAtomic {
            element: reader.identity()?,
            space: decode_address_space(reader.u8()?)?,
            scope: decode_scope(reader.u8()?)?,
        },
        5 => ExecutionCapabilityRoleV1::Matrix {
            subgroup_brand: reader.digest()?,
            width: reader.u32()?,
        },
        6 => ExecutionCapabilityRoleV1::PendingAsyncCopy {
            element: reader.identity()?,
            layout: get_layout(reader)?,
            elements: reader.u64()?,
        },
        7 => ExecutionCapabilityRoleV1::MemoryView {
            element: reader.identity()?,
            layout: get_layout(reader)?,
            space: decode_address_space(reader.u8()?)?,
            access: decode_access(reader.u8()?)?,
            extent: match reader.u8()? {
                0 => ExecutionMemoryExtentV1::Static(reader.u64()?),
                1 => ExecutionMemoryExtentV1::Dynamic(get_dynamic_extent(reader)?),
                _ => return None,
            },
            initialization: decode_initialization(reader.u8()?)?,
            index_space: get_optional_identity(reader)?,
            atomic_scope: get_optional_scope(reader)?,
        },
        8 => ExecutionCapabilityRoleV1::WorkgroupMemoryIndex,
        9 => ExecutionCapabilityRoleV1::EpochTransition,
        10 => ExecutionCapabilityRoleV1::UnsafeRawMemoryObligation,
        _ => return None,
    })
}

fn encode_execution_operation(
    writer: &mut ContractWriter,
    operation: &ExecutionCapabilityOperationV1,
) {
    use ExecutionCapabilityOperationV1 as Op;
    match operation {
        Op::WorkgroupDerive { context, workgroup } => {
            writer.u8(0);
            writer.identity(*context);
            writer.identity(*workgroup);
        }
        Op::SubgroupDerive {
            workgroup,
            subgroup,
            width,
        } => {
            writer.u8(1);
            writer.identity(*workgroup);
            writer.identity(*subgroup);
            writer.u32(*width);
        }
        Op::LdsAllocate {
            workgroup,
            lds,
            element,
            layout,
            elements,
        } => {
            writer.u8(2);
            writer.identity(*workgroup);
            writer.identity(*lds);
            writer.identity(*element);
            put_layout(writer, *layout);
            writer.u64(*elements);
        }
        Op::LdsInitializeByInvocation {
            input_lds,
            workgroup,
            output_lds,
            element,
            layout,
            elements,
        } => {
            writer.u8(3);
            writer.identity(*input_lds);
            writer.identity(*workgroup);
            writer.identity(*output_lds);
            writer.identity(*element);
            put_layout(writer, *layout);
            writer.u64(*elements);
        }
        Op::LdsPublish {
            input_workgroup,
            input_lds,
            output_lds,
            transition,
            element,
            layout,
            elements,
        } => {
            writer.u8(4);
            writer.identity(*input_workgroup);
            writer.identity(*input_lds);
            writer.identity(*output_lds);
            writer.identity(*transition);
            writer.identity(*element);
            put_layout(writer, *layout);
            writer.u64(*elements);
        }
        Op::LdsReadPublished {
            lds_reference,
            lds,
            workgroup,
            index,
            option,
            element,
            layout,
            elements,
        } => {
            writer.u8(5);
            writer.identity(*lds_reference);
            writer.identity(*lds);
            writer.identity(*workgroup);
            writer.identity(*index);
            writer.identity(*option);
            writer.identity(*element);
            put_layout(writer, *layout);
            writer.u64(*elements);
        }
        Op::WorkgroupBarrier {
            input_workgroup,
            output_workgroup,
            semantics,
        } => {
            writer.u8(6);
            writer.identity(*input_workgroup);
            writer.identity(*output_workgroup);
            put_semantics(writer, *semantics);
        }
        Op::SubgroupBarrier {
            input_workgroup,
            semantics,
            subgroup,
            transition,
            width,
        } => {
            writer.u8(7);
            writer.identity(*input_workgroup);
            put_semantics(writer, *semantics);
            writer.identity(*subgroup);
            writer.identity(*transition);
            writer.u32(*width);
        }
        Op::WorkgroupFence {
            workgroup,
            result,
            semantics,
        } => {
            writer.u8(8);
            writer.identity(*workgroup);
            writer.identity(*result);
            put_semantics(writer, *semantics);
        }
        Op::SubgroupFence {
            semantics,
            subgroup_reference,
            subgroup,
            epoch,
            result,
            width,
        } => {
            writer.u8(9);
            put_semantics(writer, *semantics);
            writer.identity(*subgroup_reference);
            writer.identity(*subgroup);
            writer.identity(*epoch);
            writer.identity(*result);
            writer.u32(*width);
        }
        Op::Atomic {
            kind,
            authority,
            location_input,
            location,
            element,
            operand,
            replacement,
            result,
            value_type,
            address_space,
            scope,
            success,
            failure,
        } => {
            writer.u8(10);
            writer.u8(atomic_kind_tag(*kind));
            writer.identity(*authority);
            writer.identity(*location_input);
            writer.identity(*location);
            writer.identity(*element);
            put_optional_identity(writer, *operand);
            put_optional_identity(writer, *replacement);
            writer.identity(*result);
            writer.u8(scalar_tag(*value_type));
            writer.u8(address_space_tag(*address_space));
            writer.u8(scope_tag(*scope));
            put_optional_ordering(writer, *success);
            put_optional_ordering(writer, *failure);
        }
        Op::WorkgroupCollective {
            kind,
            input_workgroup,
            scratch,
            element,
            transition,
            value_type,
            layout,
            elements,
        } => {
            writer.u8(11);
            writer.u8(collective_tag(*kind));
            writer.identity(*input_workgroup);
            writer.identity(*scratch);
            writer.identity(*element);
            writer.identity(*transition);
            writer.u8(scalar_tag(*value_type));
            put_layout(writer, *layout);
            writer.u64(*elements);
        }
        Op::SubgroupCollective {
            kind,
            subgroup_reference,
            subgroup,
            epoch,
            element,
            value_type,
            width,
        } => {
            writer.u8(12);
            writer.u8(collective_tag(*kind));
            writer.identity(*subgroup_reference);
            writer.identity(*subgroup);
            writer.identity(*epoch);
            writer.identity(*element);
            writer.u8(scalar_tag(*value_type));
            writer.u32(*width);
        }
        Op::MatrixAccess {
            subgroup,
            epoch,
            matrix,
            subgroup_brand,
            width,
        } => {
            writer.u8(13);
            writer.identity(*subgroup);
            writer.identity(*epoch);
            writer.identity(*matrix);
            writer.digest(*subgroup_brand);
            writer.u32(*width);
        }
        Op::AsyncCopy {
            workgroup,
            source_reference,
            source,
            index,
            destination,
            pending,
            element,
            layout,
            elements,
        } => {
            writer.u8(14);
            writer.identity(*workgroup);
            writer.identity(*source_reference);
            writer.identity(*source);
            writer.identity(*index);
            writer.identity(*destination);
            writer.identity(*pending);
            writer.identity(*element);
            put_layout(writer, *layout);
            writer.u64(*elements);
        }
        Op::AsyncWait {
            input_workgroup,
            pending,
            output_lds,
            transition,
            element,
            layout,
            elements,
        } => {
            writer.u8(15);
            writer.identity(*input_workgroup);
            writer.identity(*pending);
            writer.identity(*output_lds);
            writer.identity(*transition);
            writer.identity(*element);
            put_layout(writer, *layout);
            writer.u64(*elements);
        }
        Op::RawMemoryBind {
            authority,
            pointer,
            length,
            extent,
            view,
            element,
            layout,
            space,
            access,
            index_space,
            atomic_scope,
            unsafe_obligation,
        } => {
            writer.u8(16);
            writer.identity(*authority);
            writer.identity(*pointer);
            writer.identity(*length);
            put_dynamic_extent(writer, *extent);
            writer.identity(*view);
            writer.identity(*element);
            put_layout(writer, *layout);
            writer.u8(address_space_tag(*space));
            writer.u8(access_tag(*access));
            put_optional_identity(writer, *index_space);
            put_optional_scope(writer, *atomic_scope);
            writer.identity(*unsafe_obligation);
        }
        Op::PrivateMemoryAllocate {
            context,
            view,
            element,
            layout,
            elements,
        } => {
            writer.u8(17);
            writer.identity(*context);
            writer.identity(*view);
            writer.identity(*element);
            put_layout(writer, *layout);
            writer.u64(*elements);
        }
        Op::WorkgroupMemoryIndex { workgroup, witness } => {
            writer.u8(18);
            writer.identity(*workgroup);
            writer.identity(*witness);
        }
        Op::WorkgroupMemoryAllocate {
            workgroup,
            view,
            element,
            layout,
            elements,
            index_space,
        } => {
            writer.u8(19);
            writer.identity(*workgroup);
            writer.identity(*view);
            writer.identity(*element);
            put_layout(writer, *layout);
            writer.u64(*elements);
            writer.identity(*index_space);
        }
        Op::WorkgroupMemoryPublish {
            input_workgroup,
            input_view,
            output_view,
            transition,
            element,
            layout,
        } => {
            writer.u8(20);
            writer.identity(*input_workgroup);
            writer.identity(*input_view);
            writer.identity(*output_view);
            writer.identity(*transition);
            writer.identity(*element);
            put_layout(writer, *layout);
        }
        Op::MemoryLoad {
            view,
            workgroup,
            index,
            option,
            element,
            layout,
            space,
            access,
        } => {
            writer.u8(21);
            writer.identity(*view);
            put_optional_identity(writer, *workgroup);
            writer.identity(*index);
            writer.identity(*option);
            writer.identity(*element);
            put_layout(writer, *layout);
            writer.u8(address_space_tag(*space));
            writer.u8(access_tag(*access));
        }
        Op::MemoryStore {
            view,
            workgroup,
            index,
            element,
            layout,
            result,
            space,
            access,
        } => {
            writer.u8(22);
            writer.identity(*view);
            put_optional_identity(writer, *workgroup);
            writer.identity(*index);
            writer.identity(*element);
            put_layout(writer, *layout);
            writer.identity(*result);
            writer.u8(address_space_tag(*space));
            writer.u8(access_tag(*access));
        }
    }
}

fn decode_execution_operation(
    reader: &mut ContractReader<'_>,
) -> Option<ExecutionCapabilityOperationV1> {
    use ExecutionCapabilityOperationV1 as Op;
    Some(match reader.u8()? {
        0 => Op::WorkgroupDerive {
            context: reader.identity()?,
            workgroup: reader.identity()?,
        },
        1 => Op::SubgroupDerive {
            workgroup: reader.identity()?,
            subgroup: reader.identity()?,
            width: reader.u32()?,
        },
        2 => Op::LdsAllocate {
            workgroup: reader.identity()?,
            lds: reader.identity()?,
            element: reader.identity()?,
            layout: get_layout(reader)?,
            elements: reader.u64()?,
        },
        3 => Op::LdsInitializeByInvocation {
            input_lds: reader.identity()?,
            workgroup: reader.identity()?,
            output_lds: reader.identity()?,
            element: reader.identity()?,
            layout: get_layout(reader)?,
            elements: reader.u64()?,
        },
        4 => Op::LdsPublish {
            input_workgroup: reader.identity()?,
            input_lds: reader.identity()?,
            output_lds: reader.identity()?,
            transition: reader.identity()?,
            element: reader.identity()?,
            layout: get_layout(reader)?,
            elements: reader.u64()?,
        },
        5 => Op::LdsReadPublished {
            lds_reference: reader.identity()?,
            lds: reader.identity()?,
            workgroup: reader.identity()?,
            index: reader.identity()?,
            option: reader.identity()?,
            element: reader.identity()?,
            layout: get_layout(reader)?,
            elements: reader.u64()?,
        },
        6 => Op::WorkgroupBarrier {
            input_workgroup: reader.identity()?,
            output_workgroup: reader.identity()?,
            semantics: get_semantics(reader)?,
        },
        7 => Op::SubgroupBarrier {
            input_workgroup: reader.identity()?,
            semantics: get_semantics(reader)?,
            subgroup: reader.identity()?,
            transition: reader.identity()?,
            width: reader.u32()?,
        },
        8 => Op::WorkgroupFence {
            workgroup: reader.identity()?,
            result: reader.identity()?,
            semantics: get_semantics(reader)?,
        },
        9 => Op::SubgroupFence {
            semantics: get_semantics(reader)?,
            subgroup_reference: reader.identity()?,
            subgroup: reader.identity()?,
            epoch: reader.identity()?,
            result: reader.identity()?,
            width: reader.u32()?,
        },
        10 => Op::Atomic {
            kind: decode_atomic_kind(reader.u8()?)?,
            authority: reader.identity()?,
            location_input: reader.identity()?,
            location: reader.identity()?,
            element: reader.identity()?,
            operand: get_optional_identity(reader)?,
            replacement: get_optional_identity(reader)?,
            result: reader.identity()?,
            value_type: decode_scalar(reader.u8()?)?,
            address_space: decode_address_space(reader.u8()?)?,
            scope: decode_scope(reader.u8()?)?,
            success: get_optional_ordering(reader)?,
            failure: get_optional_ordering(reader)?,
        },
        11 => Op::WorkgroupCollective {
            kind: decode_collective(reader.u8()?)?,
            input_workgroup: reader.identity()?,
            scratch: reader.identity()?,
            element: reader.identity()?,
            transition: reader.identity()?,
            value_type: decode_scalar(reader.u8()?)?,
            layout: get_layout(reader)?,
            elements: reader.u64()?,
        },
        12 => Op::SubgroupCollective {
            kind: decode_collective(reader.u8()?)?,
            subgroup_reference: reader.identity()?,
            subgroup: reader.identity()?,
            epoch: reader.identity()?,
            element: reader.identity()?,
            value_type: decode_scalar(reader.u8()?)?,
            width: reader.u32()?,
        },
        13 => Op::MatrixAccess {
            subgroup: reader.identity()?,
            epoch: reader.identity()?,
            matrix: reader.identity()?,
            subgroup_brand: reader.digest()?,
            width: reader.u32()?,
        },
        14 => Op::AsyncCopy {
            workgroup: reader.identity()?,
            source_reference: reader.identity()?,
            source: reader.identity()?,
            index: reader.identity()?,
            destination: reader.identity()?,
            pending: reader.identity()?,
            element: reader.identity()?,
            layout: get_layout(reader)?,
            elements: reader.u64()?,
        },
        15 => Op::AsyncWait {
            input_workgroup: reader.identity()?,
            pending: reader.identity()?,
            output_lds: reader.identity()?,
            transition: reader.identity()?,
            element: reader.identity()?,
            layout: get_layout(reader)?,
            elements: reader.u64()?,
        },
        16 => Op::RawMemoryBind {
            authority: reader.identity()?,
            pointer: reader.identity()?,
            length: reader.identity()?,
            extent: get_dynamic_extent(reader)?,
            view: reader.identity()?,
            element: reader.identity()?,
            layout: get_layout(reader)?,
            space: decode_address_space(reader.u8()?)?,
            access: decode_access(reader.u8()?)?,
            index_space: get_optional_identity(reader)?,
            atomic_scope: get_optional_scope(reader)?,
            unsafe_obligation: reader.identity()?,
        },
        17 => Op::PrivateMemoryAllocate {
            context: reader.identity()?,
            view: reader.identity()?,
            element: reader.identity()?,
            layout: get_layout(reader)?,
            elements: reader.u64()?,
        },
        18 => Op::WorkgroupMemoryIndex {
            workgroup: reader.identity()?,
            witness: reader.identity()?,
        },
        19 => Op::WorkgroupMemoryAllocate {
            workgroup: reader.identity()?,
            view: reader.identity()?,
            element: reader.identity()?,
            layout: get_layout(reader)?,
            elements: reader.u64()?,
            index_space: reader.identity()?,
        },
        20 => Op::WorkgroupMemoryPublish {
            input_workgroup: reader.identity()?,
            input_view: reader.identity()?,
            output_view: reader.identity()?,
            transition: reader.identity()?,
            element: reader.identity()?,
            layout: get_layout(reader)?,
        },
        21 => Op::MemoryLoad {
            view: reader.identity()?,
            workgroup: get_optional_identity(reader)?,
            index: reader.identity()?,
            option: reader.identity()?,
            element: reader.identity()?,
            layout: get_layout(reader)?,
            space: decode_address_space(reader.u8()?)?,
            access: decode_access(reader.u8()?)?,
        },
        22 => Op::MemoryStore {
            view: reader.identity()?,
            workgroup: get_optional_identity(reader)?,
            index: reader.identity()?,
            element: reader.identity()?,
            layout: get_layout(reader)?,
            result: reader.identity()?,
            space: decode_address_space(reader.u8()?)?,
            access: decode_access(reader.u8()?)?,
        },
        _ => return None,
    })
}

fn put_semantics(writer: &mut ContractWriter, semantics: ExecutionMemorySemanticsV1) {
    writer.u8(scope_tag(semantics.scope));
    writer.u8(ordering_tag(semantics.ordering));
    writer.u8(spaces_tag(semantics.spaces));
}

fn get_semantics(reader: &mut ContractReader<'_>) -> Option<ExecutionMemorySemanticsV1> {
    Some(ExecutionMemorySemanticsV1 {
        scope: decode_scope(reader.u8()?)?,
        ordering: decode_ordering(reader.u8()?)?,
        spaces: decode_spaces(reader.u8()?)?,
    })
}

fn put_optional_identity(writer: &mut ContractWriter, value: Option<ExecutionTypeIdentityV1>) {
    match value {
        None => writer.u8(0),
        Some(value) => {
            writer.u8(1);
            writer.identity(value);
        }
    }
}

fn get_optional_identity(
    reader: &mut ContractReader<'_>,
) -> Option<Option<ExecutionTypeIdentityV1>> {
    match reader.u8()? {
        0 => Some(None),
        1 => Some(Some(reader.identity()?)),
        _ => None,
    }
}

fn put_optional_scope(writer: &mut ContractWriter, value: Option<ExecutionMemoryScopeV1>) {
    match value {
        None => writer.u8(0),
        Some(value) => {
            writer.u8(1);
            writer.u8(scope_tag(value));
        }
    }
}

fn get_optional_scope(reader: &mut ContractReader<'_>) -> Option<Option<ExecutionMemoryScopeV1>> {
    match reader.u8()? {
        0 => Some(None),
        1 => Some(Some(decode_scope(reader.u8()?)?)),
        _ => None,
    }
}

fn put_optional_ordering(writer: &mut ContractWriter, value: Option<ExecutionMemoryOrderingV1>) {
    match value {
        None => writer.u8(0),
        Some(value) => {
            writer.u8(1);
            writer.u8(ordering_tag(value));
        }
    }
}

fn get_optional_ordering(
    reader: &mut ContractReader<'_>,
) -> Option<Option<ExecutionMemoryOrderingV1>> {
    match reader.u8()? {
        0 => Some(None),
        1 => Some(Some(decode_ordering(reader.u8()?)?)),
        _ => None,
    }
}

fn put_layout(writer: &mut ContractWriter, layout: ExecutionElementLayoutV1) {
    writer.u32(layout.byte_size);
    writer.u16(layout.byte_alignment);
}

fn get_layout(reader: &mut ContractReader<'_>) -> Option<ExecutionElementLayoutV1> {
    let layout = ExecutionElementLayoutV1 {
        byte_size: reader.u32()?,
        byte_alignment: reader.u16()?,
    };
    layout.is_complete().then_some(layout)
}

fn put_dynamic_extent(writer: &mut ContractWriter, extent: ExecutionDynamicExtentV1) {
    writer.u8(extent.operand);
    writer.u8(extent.source_argument);
    writer.identity(extent.source_type);
    writer.u8(scalar_tag(extent.value_type));
    writer.u64(extent.upper_bound);
    writer.u8(extent.bound_check_operand);
    match extent.nonnegative_check_operand {
        None => writer.u8(0),
        Some(operand) => {
            writer.u8(1);
            writer.u8(operand);
        }
    }
}

fn get_dynamic_extent(reader: &mut ContractReader<'_>) -> Option<ExecutionDynamicExtentV1> {
    let extent = ExecutionDynamicExtentV1 {
        operand: reader.u8()?,
        source_argument: reader.u8()?,
        source_type: reader.identity()?,
        value_type: decode_scalar(reader.u8()?)?,
        upper_bound: reader.u64()?,
        bound_check_operand: reader.u8()?,
        nonnegative_check_operand: match reader.u8()? {
            0 => None,
            1 => Some(reader.u8()?),
            _ => return None,
        },
    };
    extent.is_complete().then_some(extent)
}

fn scope_tag(value: ExecutionMemoryScopeV1) -> u8 {
    match value {
        ExecutionMemoryScopeV1::System => 0,
        ExecutionMemoryScopeV1::Device => 1,
        ExecutionMemoryScopeV1::Workgroup => 2,
        ExecutionMemoryScopeV1::Subgroup => 3,
    }
}
fn decode_scope(value: u8) -> Option<ExecutionMemoryScopeV1> {
    Some(match value {
        0 => ExecutionMemoryScopeV1::System,
        1 => ExecutionMemoryScopeV1::Device,
        2 => ExecutionMemoryScopeV1::Workgroup,
        3 => ExecutionMemoryScopeV1::Subgroup,
        _ => return None,
    })
}
fn ordering_tag(value: ExecutionMemoryOrderingV1) -> u8 {
    match value {
        ExecutionMemoryOrderingV1::Relaxed => 0,
        ExecutionMemoryOrderingV1::Acquire => 1,
        ExecutionMemoryOrderingV1::Release => 2,
        ExecutionMemoryOrderingV1::AcquireRelease => 3,
        ExecutionMemoryOrderingV1::SequentiallyConsistent => 4,
    }
}
fn decode_ordering(value: u8) -> Option<ExecutionMemoryOrderingV1> {
    Some(match value {
        0 => ExecutionMemoryOrderingV1::Relaxed,
        1 => ExecutionMemoryOrderingV1::Acquire,
        2 => ExecutionMemoryOrderingV1::Release,
        3 => ExecutionMemoryOrderingV1::AcquireRelease,
        4 => ExecutionMemoryOrderingV1::SequentiallyConsistent,
        _ => return None,
    })
}
fn spaces_tag(value: ExecutionMemorySpacesV1) -> u8 {
    match value {
        ExecutionMemorySpacesV1::Global => 0,
        ExecutionMemorySpacesV1::Workgroup => 1,
        ExecutionMemorySpacesV1::GlobalAndWorkgroup => 2,
    }
}
fn decode_spaces(value: u8) -> Option<ExecutionMemorySpacesV1> {
    Some(match value {
        0 => ExecutionMemorySpacesV1::Global,
        1 => ExecutionMemorySpacesV1::Workgroup,
        2 => ExecutionMemorySpacesV1::GlobalAndWorkgroup,
        _ => return None,
    })
}
fn address_space_tag(value: ExecutionMemoryAddressSpaceV1) -> u8 {
    match value {
        ExecutionMemoryAddressSpaceV1::Private => 0,
        ExecutionMemoryAddressSpaceV1::Workgroup => 1,
        ExecutionMemoryAddressSpaceV1::Global => 2,
    }
}
fn decode_address_space(value: u8) -> Option<ExecutionMemoryAddressSpaceV1> {
    Some(match value {
        0 => ExecutionMemoryAddressSpaceV1::Private,
        1 => ExecutionMemoryAddressSpaceV1::Workgroup,
        2 => ExecutionMemoryAddressSpaceV1::Global,
        _ => return None,
    })
}
fn access_tag(value: ExecutionMemoryAccessV1) -> u8 {
    match value {
        ExecutionMemoryAccessV1::ReadOnly => 0,
        ExecutionMemoryAccessV1::ExclusiveReadWrite => 1,
        ExecutionMemoryAccessV1::DisjointWrite => 2,
        ExecutionMemoryAccessV1::AtomicReadWrite => 3,
    }
}
fn decode_access(value: u8) -> Option<ExecutionMemoryAccessV1> {
    Some(match value {
        0 => ExecutionMemoryAccessV1::ReadOnly,
        1 => ExecutionMemoryAccessV1::ExclusiveReadWrite,
        2 => ExecutionMemoryAccessV1::DisjointWrite,
        3 => ExecutionMemoryAccessV1::AtomicReadWrite,
        _ => return None,
    })
}
fn lds_state_tag(value: ExecutionLdsStateV1) -> u8 {
    match value {
        ExecutionLdsStateV1::Uninitialized => 0,
        ExecutionLdsStateV1::InvocationInitialized => 1,
        ExecutionLdsStateV1::Published => 2,
        ExecutionLdsStateV1::PendingAsyncCopy => 3,
    }
}
fn decode_lds_state(value: u8) -> Option<ExecutionLdsStateV1> {
    Some(match value {
        0 => ExecutionLdsStateV1::Uninitialized,
        1 => ExecutionLdsStateV1::InvocationInitialized,
        2 => ExecutionLdsStateV1::Published,
        3 => ExecutionLdsStateV1::PendingAsyncCopy,
        _ => return None,
    })
}
fn collective_tag(value: ExecutionCollectiveKindV1) -> u8 {
    match value {
        ExecutionCollectiveKindV1::ReduceSum => 0,
        ExecutionCollectiveKindV1::InclusiveScanSum => 1,
        ExecutionCollectiveKindV1::ExclusiveScanSum => 2,
    }
}
fn decode_collective(value: u8) -> Option<ExecutionCollectiveKindV1> {
    Some(match value {
        0 => ExecutionCollectiveKindV1::ReduceSum,
        1 => ExecutionCollectiveKindV1::InclusiveScanSum,
        2 => ExecutionCollectiveKindV1::ExclusiveScanSum,
        _ => return None,
    })
}
fn atomic_kind_tag(value: ExecutionAtomicKindV1) -> u8 {
    match value {
        ExecutionAtomicKindV1::BindGlobalLocation => 0,
        ExecutionAtomicKindV1::Load => 1,
        ExecutionAtomicKindV1::Store => 2,
        ExecutionAtomicKindV1::FetchAdd => 3,
        ExecutionAtomicKindV1::CompareExchange => 4,
        ExecutionAtomicKindV1::BindGlobalView => 5,
    }
}
fn decode_atomic_kind(value: u8) -> Option<ExecutionAtomicKindV1> {
    Some(match value {
        0 => ExecutionAtomicKindV1::BindGlobalLocation,
        1 => ExecutionAtomicKindV1::Load,
        2 => ExecutionAtomicKindV1::Store,
        3 => ExecutionAtomicKindV1::FetchAdd,
        4 => ExecutionAtomicKindV1::CompareExchange,
        5 => ExecutionAtomicKindV1::BindGlobalView,
        _ => return None,
    })
}
fn initialization_tag(value: ExecutionMemoryInitializationV1) -> u8 {
    match value {
        ExecutionMemoryInitializationV1::Uninitialized => 0,
        ExecutionMemoryInitializationV1::InvocationInitialized => 1,
        ExecutionMemoryInitializationV1::Published => 2,
        ExecutionMemoryInitializationV1::FullyInitialized => 3,
        ExecutionMemoryInitializationV1::SelectedWriteInitializes => 4,
    }
}
fn decode_initialization(value: u8) -> Option<ExecutionMemoryInitializationV1> {
    Some(match value {
        0 => ExecutionMemoryInitializationV1::Uninitialized,
        1 => ExecutionMemoryInitializationV1::InvocationInitialized,
        2 => ExecutionMemoryInitializationV1::Published,
        3 => ExecutionMemoryInitializationV1::FullyInitialized,
        4 => ExecutionMemoryInitializationV1::SelectedWriteInitializes,
        _ => return None,
    })
}
fn scalar_tag(value: ScalarType) -> u8 {
    match value {
        ScalarType::Bool => 0,
        ScalarType::I8 => 1,
        ScalarType::I16 => 2,
        ScalarType::I32 => 3,
        ScalarType::I64 => 4,
        ScalarType::I128 => 5,
        ScalarType::U8 => 6,
        ScalarType::U16 => 7,
        ScalarType::U32 => 8,
        ScalarType::U64 => 9,
        ScalarType::U128 => 10,
        ScalarType::F16 => 11,
        ScalarType::Bf16 => 12,
        ScalarType::F32 => 13,
        ScalarType::F64 => 14,
        ScalarType::Index => 15,
    }
}
fn decode_scalar(value: u8) -> Option<ScalarType> {
    Some(match value {
        0 => ScalarType::Bool,
        1 => ScalarType::I8,
        2 => ScalarType::I16,
        3 => ScalarType::I32,
        4 => ScalarType::I64,
        5 => ScalarType::I128,
        6 => ScalarType::U8,
        7 => ScalarType::U16,
        8 => ScalarType::U32,
        9 => ScalarType::U64,
        10 => ScalarType::U128,
        11 => ScalarType::F16,
        12 => ScalarType::Bf16,
        13 => ScalarType::F32,
        14 => ScalarType::F64,
        15 => ScalarType::Index,
        _ => return None,
    })
}
