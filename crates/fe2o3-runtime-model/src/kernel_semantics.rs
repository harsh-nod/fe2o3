//! Authority-free admission model for gfx942 atomic and collective semantics.
//!
//! KFD creates queues and publishes packets; it does not implement the atomic
//! or collective operations executed by a kernel. This model therefore binds
//! caller-declared semantics to one current runtime device, live code artifact,
//! and exact data mappings. It grants no compiler, code-object, coherence,
//! dispatch, convergence, LDS, machine-code, or hardware authority. A native
//! adapter must authenticate those facts independently before treating an
//! admitted value as executable evidence.

use alloc::vec::Vec;

use crate::*;

pub const GFX942_KERNEL_SEMANTICS_SCHEMA_VERSION_V1: u16 = 1;
pub const MAX_GFX942_KERNEL_SEMANTIC_OPERATIONS_V1: usize = 256;
pub const GFX942_WAVE64_LANES_V1: u32 = 64;
pub const MAX_GFX942_COLLECTIVE_WORKGROUP_SIZE_V1: u32 = 256;
pub const MAX_GFX942_MODELED_LDS_BYTES_V1: u32 = 64 * 1_024;

/// Standard integer atomic operations in the reviewed gfx942 model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942AtomicOperationV1 {
    Load,
    Store,
    Swap,
    CompareExchange,
    FetchAdd,
    FetchSub,
    FetchAnd,
    FetchNand,
    FetchOr,
    FetchXor,
    FetchMinSigned,
    FetchMinUnsigned,
    FetchMaxSigned,
    FetchMaxUnsigned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942AtomicWidthV1 {
    Bits32,
    Bits64,
}

impl Gfx942AtomicWidthV1 {
    pub const fn byte_len(self) -> u64 {
        match self {
            Self::Bits32 => 4,
            Self::Bits64 => 8,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942AtomicScopeV1 {
    Workgroup,
    Device,
    System,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942MemoryOrderingV1 {
    Relaxed,
    Acquire,
    Release,
    AcquireRelease,
    SequentiallyConsistent,
}

/// Caller-declared global-storage coherence class.
///
/// `SystemCoherent` is not a native receipt. A sealed adapter must relate it
/// to an eligible allocation and mapping before system-scope execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclaredAtomicCoherenceV1 {
    DeviceOnly,
    SystemCoherent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942AtomicStorageV1 {
    Global {
        mapping: MappingKeyV1,
        byte_offset: u64,
        coherence: DeclaredAtomicCoherenceV1,
    },
    Workgroup {
        byte_offset: u32,
        lds_byte_len: u32,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942AtomicSemanticV1 {
    pub operation_id: u16,
    pub operation: Gfx942AtomicOperationV1,
    pub width: Gfx942AtomicWidthV1,
    pub storage: Gfx942AtomicStorageV1,
    pub scope: Gfx942AtomicScopeV1,
    pub success_ordering: Gfx942MemoryOrderingV1,
    pub failure_ordering: Option<Gfx942MemoryOrderingV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942CollectiveElementV1 {
    U32,
    I32,
    F32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942CollectiveOperationV1 {
    Wave64ReduceSum,
    Wave64InclusiveScanSum,
    Wave64ExclusiveScanSum,
    Wave64ReduceActiveU32,
    SubgroupReduceSumF32,
    SubgroupReduceMaxF32,
    WorkgroupReduceSum,
    WorkgroupInclusiveScanSum,
    WorkgroupExclusiveScanSum,
    Workgroup256ReduceActiveU32,
}

/// Exact static geometry claimed for one collective call site.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942CollectiveSemanticV1 {
    pub operation_id: u16,
    pub operation: Gfx942CollectiveOperationV1,
    pub element: Gfx942CollectiveElementV1,
    /// Physical lanes/work-items that must execute the call convergently.
    ///
    /// Wave and subgroup operations require all 64 physical wave lanes. For a
    /// workgroup operation this is the exact one-dimensional workgroup size.
    pub physical_participant_count: u32,
    /// Static contiguous tile width for subgroup operations; `None` for wave
    /// and workgroup operations.
    pub subgroup_width: Option<u32>,
    /// One scalar LDS slot per work-item for workgroup algorithms; zero for
    /// wave/subgroup algorithms.
    pub lds_slots: u32,
    /// Declares that every physical participant reaches the call in the same
    /// dynamic order. This is an input to the model, not a CFG proof.
    pub convergent: bool,
    /// Declares an exact `[physical_participant_count, 1, 1]` workgroup launch.
    pub exact_one_dimensional_launch: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942KernelSemanticOperationV1 {
    Atomic(Gfx942AtomicSemanticV1),
    Collective(Gfx942CollectiveSemanticV1),
}

impl Gfx942KernelSemanticOperationV1 {
    pub const fn operation_id(self) -> u16 {
        match self {
            Self::Atomic(operation) => operation.operation_id,
            Self::Collective(operation) => operation.operation_id,
        }
    }
}

/// Caller-constructed contract presented for model-only admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UntrustedGfx942KernelSemanticContractV1 {
    pub schema_version: u16,
    pub contract_identity: IdentityDigestV1,
    pub device: DeviceKeyV1,
    pub artifact: RuntimeArtifactIdV1,
    pub kernel_identity: IdentityDigestV1,
    pub operations: Vec<Gfx942KernelSemanticOperationV1>,
}

/// Exact admitted model value. It remains in [`AuthorityDomainV1::ModelOnly`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelGfx942KernelSemanticsV1 {
    contract_identity: IdentityDigestV1,
    device: DeviceKeyV1,
    code: LoadedCodeKeyV1,
    artifact: RuntimeArtifactIdV1,
    kernel_identity: IdentityDigestV1,
    resources: AsyncOperationResourcesV1,
    operations: Vec<Gfx942KernelSemanticOperationV1>,
}

impl ModelGfx942KernelSemanticsV1 {
    pub const fn authority_domain(&self) -> AuthorityDomainV1 {
        AuthorityDomainV1::ModelOnly
    }

    pub const fn contract_identity(&self) -> IdentityDigestV1 {
        self.contract_identity
    }

    pub const fn device(&self) -> DeviceKeyV1 {
        self.device
    }

    pub const fn code(&self) -> LoadedCodeKeyV1 {
        self.code
    }

    pub const fn artifact(&self) -> RuntimeArtifactIdV1 {
        self.artifact
    }

    pub const fn kernel_identity(&self) -> IdentityDigestV1 {
        self.kernel_identity
    }

    pub const fn resources(&self) -> &AsyncOperationResourcesV1 {
        &self.resources
    }

    pub fn operations(&self) -> &[Gfx942KernelSemanticOperationV1] {
        &self.operations
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942KernelSemanticAdmissionErrorV1 {
    InvalidSchema,
    InvalidIdentity,
    InvalidRuntimeState,
    DeviceNotCurrent,
    CodeNotLive,
    CodeDeviceMismatch,
    ArtifactMismatch,
    EmptyOperationRoster,
    OperationCapacityExceeded,
    NonCanonicalOperationRoster,
    MappingNotLive,
    MappingDeviceMismatch,
    MappingResourceMissing,
    MappingAccessInsufficient,
    ResourceRoleCollision,
    ExecutableStorageCollision,
    AtomicRangeInvalid,
    AtomicStorageScopeMismatch,
    SystemCoherenceRequired,
    AtomicOrderingInvalid,
    AtomicFailureOrderingInvalid,
    IncompatibleAtomicObject,
    CollectiveConvergenceRequired,
    CollectiveGeometryInvalid,
    CollectiveElementInvalid,
    CollectiveScratchInvalid,
}

/// Admits caller-declared gfx942 semantics against exact runtime resources.
///
/// This function checks identities, lifetimes, ranges, access, ordering, scope,
/// collective geometry, and LDS cardinality. It does not inspect a code object
/// or establish that the declared calls occur in the loaded artifact.
pub fn admit_gfx942_kernel_semantics_model_only_v1(
    runtime: &RuntimeStateV1,
    resources: &AsyncOperationResourcesV1,
    contract: UntrustedGfx942KernelSemanticContractV1,
) -> Result<ModelGfx942KernelSemanticsV1, Gfx942KernelSemanticAdmissionErrorV1> {
    runtime
        .validate_global_invariants()
        .map_err(|_| Gfx942KernelSemanticAdmissionErrorV1::InvalidRuntimeState)?;
    if contract.schema_version != GFX942_KERNEL_SEMANTICS_SCHEMA_VERSION_V1 {
        return Err(Gfx942KernelSemanticAdmissionErrorV1::InvalidSchema);
    }
    if digest_is_zero(contract.contract_identity)
        || digest_is_zero(contract.artifact.digest())
        || digest_is_zero(contract.kernel_identity)
        || contract.device.generation.0 == 0
    {
        return Err(Gfx942KernelSemanticAdmissionErrorV1::InvalidIdentity);
    }
    if !runtime
        .devices()
        .iter()
        .any(|record| record.key == contract.device && record.state == DeviceStateV1::Ready)
    {
        return Err(Gfx942KernelSemanticAdmissionErrorV1::DeviceNotCurrent);
    }
    let code = runtime
        .loaded_code()
        .iter()
        .find(|record| record.key == resources.code() && record.state == ResourceStateV1::Live)
        .ok_or(Gfx942KernelSemanticAdmissionErrorV1::CodeNotLive)?;
    if code.key.vm.device != contract.device {
        return Err(Gfx942KernelSemanticAdmissionErrorV1::CodeDeviceMismatch);
    }
    if code.artifact_id != contract.artifact {
        return Err(Gfx942KernelSemanticAdmissionErrorV1::ArtifactMismatch);
    }
    let resource_ranges = validate_async_resources(runtime, resources, code.key.vm)?;
    if contract.operations.is_empty() {
        return Err(Gfx942KernelSemanticAdmissionErrorV1::EmptyOperationRoster);
    }
    if contract.operations.len() > MAX_GFX942_KERNEL_SEMANTIC_OPERATIONS_V1 {
        return Err(Gfx942KernelSemanticAdmissionErrorV1::OperationCapacityExceeded);
    }
    if contract.operations[0].operation_id() == 0
        || contract
            .operations
            .windows(2)
            .any(|pair| pair[0].operation_id() >= pair[1].operation_id())
    {
        return Err(Gfx942KernelSemanticAdmissionErrorV1::NonCanonicalOperationRoster);
    }
    for operation in &contract.operations {
        match operation {
            Gfx942KernelSemanticOperationV1::Atomic(atomic) => {
                validate_atomic(runtime, resources, code.key.vm, *atomic)?;
            }
            Gfx942KernelSemanticOperationV1::Collective(collective) => {
                validate_collective(*collective)?;
            }
        }
    }
    validate_atomic_object_compatibility(runtime, &contract.operations)?;
    validate_resource_role_aliases(&resource_ranges)?;
    validate_writable_executable_aliases(runtime, &resource_ranges)?;
    Ok(ModelGfx942KernelSemanticsV1 {
        contract_identity: contract.contract_identity,
        device: contract.device,
        code: code.key,
        artifact: contract.artifact,
        kernel_identity: contract.kernel_identity,
        resources: resources.clone(),
        operations: contract.operations,
    })
}

fn validate_async_resources(
    runtime: &RuntimeStateV1,
    resources: &AsyncOperationResourcesV1,
    vm: VmKeyV1,
) -> Result<Vec<PhysicalMappingRangeV1>, Gfx942KernelSemanticAdmissionErrorV1> {
    let mut ranges = Vec::with_capacity(resources.data().len() + 2);
    ranges.push(validate_runtime_mapping(
        runtime,
        resources.kernarg(),
        vm,
        MemoryAccessV1::ReadWrite,
    )?);
    ranges.push(validate_runtime_mapping(
        runtime,
        resources.completion_signal(),
        vm,
        MemoryAccessV1::ReadWrite,
    )?);
    for resource in resources.data() {
        ranges.push(validate_runtime_mapping(
            runtime,
            resource.mapping,
            vm,
            resource.required_access,
        )?);
    }
    Ok(ranges)
}

#[derive(Clone, Copy)]
struct PhysicalMappingRangeV1 {
    allocation: AllocationKeyV1,
    byte_offset: u64,
    byte_len: u64,
    required_access: MemoryAccessV1,
}

fn validate_runtime_mapping(
    runtime: &RuntimeStateV1,
    mapping: MappingKeyV1,
    vm: VmKeyV1,
    required_access: MemoryAccessV1,
) -> Result<PhysicalMappingRangeV1, Gfx942KernelSemanticAdmissionErrorV1> {
    let record = runtime
        .mappings()
        .iter()
        .find(|record| record.key == mapping && record.state == ResourceStateV1::Live)
        .ok_or(Gfx942KernelSemanticAdmissionErrorV1::MappingNotLive)?;
    if mapping.allocation.vm != vm {
        return Err(Gfx942KernelSemanticAdmissionErrorV1::MappingDeviceMismatch);
    }
    if !record.access.permits(required_access) {
        return Err(Gfx942KernelSemanticAdmissionErrorV1::MappingAccessInsufficient);
    }
    Ok(PhysicalMappingRangeV1 {
        allocation: mapping.allocation,
        byte_offset: record.allocation_offset,
        byte_len: record.byte_len,
        required_access,
    })
}

fn validate_resource_role_aliases(
    resources: &[PhysicalMappingRangeV1],
) -> Result<(), Gfx942KernelSemanticAdmissionErrorV1> {
    for left_index in 0..resources.len() {
        let left = resources[left_index];
        for right in &resources[left_index + 1..] {
            if left.allocation == right.allocation
                && ranges_overlap_u64(
                    left.byte_offset,
                    left.byte_len,
                    right.byte_offset,
                    right.byte_len,
                )
            {
                return Err(Gfx942KernelSemanticAdmissionErrorV1::ResourceRoleCollision);
            }
        }
    }
    Ok(())
}

fn validate_writable_executable_aliases(
    runtime: &RuntimeStateV1,
    resources: &[PhysicalMappingRangeV1],
) -> Result<(), Gfx942KernelSemanticAdmissionErrorV1> {
    for code in runtime
        .loaded_code()
        .iter()
        .filter(|code| code.state == ResourceStateV1::Live)
    {
        let executable = runtime
            .mappings()
            .iter()
            .find(|mapping| {
                mapping.key == code.executable_mapping && mapping.state == ResourceStateV1::Live
            })
            .ok_or(Gfx942KernelSemanticAdmissionErrorV1::InvalidRuntimeState)?;
        if resources.iter().any(|resource| {
            resource.required_access == MemoryAccessV1::ReadWrite
                && resource.allocation == executable.key.allocation
                && ranges_overlap_u64(
                    resource.byte_offset,
                    resource.byte_len,
                    executable.allocation_offset,
                    executable.byte_len,
                )
        }) {
            return Err(Gfx942KernelSemanticAdmissionErrorV1::ExecutableStorageCollision);
        }
    }
    Ok(())
}

fn validate_atomic(
    runtime: &RuntimeStateV1,
    resources: &AsyncOperationResourcesV1,
    vm: VmKeyV1,
    atomic: Gfx942AtomicSemanticV1,
) -> Result<(), Gfx942KernelSemanticAdmissionErrorV1> {
    if !atomic_success_ordering_is_valid(atomic.operation, atomic.success_ordering) {
        return Err(Gfx942KernelSemanticAdmissionErrorV1::AtomicOrderingInvalid);
    }
    match (atomic.operation, atomic.failure_ordering) {
        (Gfx942AtomicOperationV1::CompareExchange, Some(failure))
            if compare_exchange_failure_is_valid(atomic.success_ordering, failure) => {}
        (Gfx942AtomicOperationV1::CompareExchange, _) => {
            return Err(Gfx942KernelSemanticAdmissionErrorV1::AtomicFailureOrderingInvalid);
        }
        (_, None) => {}
        (_, Some(_)) => {
            return Err(Gfx942KernelSemanticAdmissionErrorV1::AtomicFailureOrderingInvalid);
        }
    }

    match atomic.storage {
        Gfx942AtomicStorageV1::Global {
            mapping,
            byte_offset,
            coherence,
        } => {
            let record = runtime
                .mappings()
                .iter()
                .find(|record| record.key == mapping && record.state == ResourceStateV1::Live)
                .ok_or(Gfx942KernelSemanticAdmissionErrorV1::MappingNotLive)?;
            if mapping.allocation.vm != vm {
                return Err(Gfx942KernelSemanticAdmissionErrorV1::MappingDeviceMismatch);
            }
            let required_access = if atomic.operation == Gfx942AtomicOperationV1::Load {
                MemoryAccessV1::Read
            } else {
                MemoryAccessV1::ReadWrite
            };
            if !resources.data().iter().any(|resource| {
                resource.mapping == mapping && resource.required_access.permits(required_access)
            }) {
                return Err(Gfx942KernelSemanticAdmissionErrorV1::MappingResourceMissing);
            }
            if !record.access.permits(required_access) {
                return Err(Gfx942KernelSemanticAdmissionErrorV1::MappingAccessInsufficient);
            }
            let width = atomic.width.byte_len();
            let address = record
                .gpu_va
                .checked_add(byte_offset)
                .ok_or(Gfx942KernelSemanticAdmissionErrorV1::AtomicRangeInvalid)?;
            if !address.is_multiple_of(width)
                || byte_offset
                    .checked_add(width)
                    .is_none_or(|end| end > record.byte_len)
            {
                return Err(Gfx942KernelSemanticAdmissionErrorV1::AtomicRangeInvalid);
            }
            if atomic.scope == Gfx942AtomicScopeV1::System
                && coherence != DeclaredAtomicCoherenceV1::SystemCoherent
            {
                return Err(Gfx942KernelSemanticAdmissionErrorV1::SystemCoherenceRequired);
            }
        }
        Gfx942AtomicStorageV1::Workgroup {
            byte_offset,
            lds_byte_len,
        } => {
            if atomic.scope != Gfx942AtomicScopeV1::Workgroup {
                return Err(Gfx942KernelSemanticAdmissionErrorV1::AtomicStorageScopeMismatch);
            }
            let width = u32::try_from(atomic.width.byte_len())
                .map_err(|_| Gfx942KernelSemanticAdmissionErrorV1::AtomicRangeInvalid)?;
            if lds_byte_len == 0
                || lds_byte_len > MAX_GFX942_MODELED_LDS_BYTES_V1
                || !byte_offset.is_multiple_of(width)
                || byte_offset
                    .checked_add(width)
                    .is_none_or(|end| end > lds_byte_len)
            {
                return Err(Gfx942KernelSemanticAdmissionErrorV1::AtomicRangeInvalid);
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum AtomicObjectLocationV1 {
    Global {
        allocation: AllocationKeyV1,
        byte_offset: u64,
    },
    Workgroup {
        byte_offset: u32,
    },
}

#[derive(Clone, Copy)]
struct AtomicObjectV1 {
    location: AtomicObjectLocationV1,
    width: u64,
}

fn validate_atomic_object_compatibility(
    runtime: &RuntimeStateV1,
    operations: &[Gfx942KernelSemanticOperationV1],
) -> Result<(), Gfx942KernelSemanticAdmissionErrorV1> {
    let atomic_objects = operations
        .iter()
        .filter_map(|operation| match operation {
            Gfx942KernelSemanticOperationV1::Atomic(atomic) => Some(*atomic),
            Gfx942KernelSemanticOperationV1::Collective(_) => None,
        })
        .map(|atomic| atomic_object(runtime, atomic))
        .collect::<Result<Vec<_>, _>>()?;

    for left_index in 0..atomic_objects.len() {
        let left = atomic_objects[left_index];
        for right in &atomic_objects[left_index + 1..] {
            if atomic_objects_overlap(left, *right)
                && !atomic_objects_are_exactly_compatible(left, *right)
            {
                return Err(Gfx942KernelSemanticAdmissionErrorV1::IncompatibleAtomicObject);
            }
        }
    }
    Ok(())
}

fn atomic_object(
    runtime: &RuntimeStateV1,
    atomic: Gfx942AtomicSemanticV1,
) -> Result<AtomicObjectV1, Gfx942KernelSemanticAdmissionErrorV1> {
    let width = atomic.width.byte_len();
    let location = match atomic.storage {
        Gfx942AtomicStorageV1::Global {
            mapping,
            byte_offset,
            ..
        } => {
            let record = runtime
                .mappings()
                .iter()
                .find(|record| record.key == mapping && record.state == ResourceStateV1::Live)
                .ok_or(Gfx942KernelSemanticAdmissionErrorV1::MappingNotLive)?;
            let allocation_byte_offset = record
                .allocation_offset
                .checked_add(byte_offset)
                .ok_or(Gfx942KernelSemanticAdmissionErrorV1::AtomicRangeInvalid)?;
            AtomicObjectLocationV1::Global {
                allocation: mapping.allocation,
                byte_offset: allocation_byte_offset,
            }
        }
        Gfx942AtomicStorageV1::Workgroup { byte_offset, .. } => {
            AtomicObjectLocationV1::Workgroup { byte_offset }
        }
    };
    Ok(AtomicObjectV1 { location, width })
}

fn atomic_objects_overlap(left: AtomicObjectV1, right: AtomicObjectV1) -> bool {
    match (left.location, right.location) {
        (
            AtomicObjectLocationV1::Global {
                allocation: left_allocation,
                byte_offset: left_start,
            },
            AtomicObjectLocationV1::Global {
                allocation: right_allocation,
                byte_offset: right_start,
            },
        ) if left_allocation == right_allocation => {
            ranges_overlap_u64(left_start, left.width, right_start, right.width)
        }
        (
            AtomicObjectLocationV1::Workgroup {
                byte_offset: left_start,
            },
            AtomicObjectLocationV1::Workgroup {
                byte_offset: right_start,
            },
        ) => ranges_overlap_u64(
            u64::from(left_start),
            left.width,
            u64::from(right_start),
            right.width,
        ),
        _ => false,
    }
}

fn atomic_objects_are_exactly_compatible(left: AtomicObjectV1, right: AtomicObjectV1) -> bool {
    left.width == right.width
        && match (left.location, right.location) {
            (
                AtomicObjectLocationV1::Global {
                    allocation: left_allocation,
                    byte_offset: left_offset,
                },
                AtomicObjectLocationV1::Global {
                    allocation: right_allocation,
                    byte_offset: right_offset,
                },
            ) => left_allocation == right_allocation && left_offset == right_offset,
            (
                AtomicObjectLocationV1::Workgroup {
                    byte_offset: left_offset,
                },
                AtomicObjectLocationV1::Workgroup {
                    byte_offset: right_offset,
                },
            ) => left_offset == right_offset,
            _ => false,
        }
}

fn ranges_overlap_u64(left_start: u64, left_len: u64, right_start: u64, right_len: u64) -> bool {
    let Some(left_end) = left_start.checked_add(left_len) else {
        return true;
    };
    let Some(right_end) = right_start.checked_add(right_len) else {
        return true;
    };
    left_start < right_end && right_start < left_end
}

fn validate_collective(
    collective: Gfx942CollectiveSemanticV1,
) -> Result<(), Gfx942KernelSemanticAdmissionErrorV1> {
    if !collective.convergent {
        return Err(Gfx942KernelSemanticAdmissionErrorV1::CollectiveConvergenceRequired);
    }
    let wave_operation = matches!(
        collective.operation,
        Gfx942CollectiveOperationV1::Wave64ReduceSum
            | Gfx942CollectiveOperationV1::Wave64InclusiveScanSum
            | Gfx942CollectiveOperationV1::Wave64ExclusiveScanSum
            | Gfx942CollectiveOperationV1::Wave64ReduceActiveU32
    );
    let subgroup_operation = matches!(
        collective.operation,
        Gfx942CollectiveOperationV1::SubgroupReduceSumF32
            | Gfx942CollectiveOperationV1::SubgroupReduceMaxF32
    );
    let workgroup_operation = !wave_operation && !subgroup_operation;

    if wave_operation
        && (collective.physical_participant_count != GFX942_WAVE64_LANES_V1
            || collective.subgroup_width.is_some()
            || collective.lds_slots != 0)
    {
        return Err(Gfx942KernelSemanticAdmissionErrorV1::CollectiveGeometryInvalid);
    }
    if subgroup_operation
        && (collective.physical_participant_count != GFX942_WAVE64_LANES_V1
            || collective
                .subgroup_width
                .is_none_or(|width| !supported_power_of_two(width, GFX942_WAVE64_LANES_V1))
            || collective.lds_slots != 0)
    {
        return Err(Gfx942KernelSemanticAdmissionErrorV1::CollectiveGeometryInvalid);
    }
    if workgroup_operation
        && (!collective.exact_one_dimensional_launch
            || collective.subgroup_width.is_some()
            || !supported_power_of_two(
                collective.physical_participant_count,
                MAX_GFX942_COLLECTIVE_WORKGROUP_SIZE_V1,
            ))
    {
        return Err(Gfx942KernelSemanticAdmissionErrorV1::CollectiveGeometryInvalid);
    }
    if workgroup_operation && collective.lds_slots != collective.physical_participant_count {
        return Err(Gfx942KernelSemanticAdmissionErrorV1::CollectiveScratchInvalid);
    }

    match collective.operation {
        Gfx942CollectiveOperationV1::Wave64ReduceActiveU32
        | Gfx942CollectiveOperationV1::Workgroup256ReduceActiveU32
            if collective.element != Gfx942CollectiveElementV1::U32 =>
        {
            return Err(Gfx942KernelSemanticAdmissionErrorV1::CollectiveElementInvalid);
        }
        Gfx942CollectiveOperationV1::SubgroupReduceSumF32
        | Gfx942CollectiveOperationV1::SubgroupReduceMaxF32
            if collective.element != Gfx942CollectiveElementV1::F32 =>
        {
            return Err(Gfx942KernelSemanticAdmissionErrorV1::CollectiveElementInvalid);
        }
        Gfx942CollectiveOperationV1::Workgroup256ReduceActiveU32
            if collective.physical_participant_count != MAX_GFX942_COLLECTIVE_WORKGROUP_SIZE_V1 =>
        {
            return Err(Gfx942KernelSemanticAdmissionErrorV1::CollectiveGeometryInvalid);
        }
        _ => {}
    }
    Ok(())
}

const fn atomic_success_ordering_is_valid(
    operation: Gfx942AtomicOperationV1,
    ordering: Gfx942MemoryOrderingV1,
) -> bool {
    match operation {
        Gfx942AtomicOperationV1::Load => matches!(
            ordering,
            Gfx942MemoryOrderingV1::Relaxed
                | Gfx942MemoryOrderingV1::Acquire
                | Gfx942MemoryOrderingV1::SequentiallyConsistent
        ),
        Gfx942AtomicOperationV1::Store => matches!(
            ordering,
            Gfx942MemoryOrderingV1::Relaxed
                | Gfx942MemoryOrderingV1::Release
                | Gfx942MemoryOrderingV1::SequentiallyConsistent
        ),
        _ => true,
    }
}

const fn compare_exchange_failure_is_valid(
    success: Gfx942MemoryOrderingV1,
    failure: Gfx942MemoryOrderingV1,
) -> bool {
    match success {
        Gfx942MemoryOrderingV1::Relaxed => matches!(failure, Gfx942MemoryOrderingV1::Relaxed),
        Gfx942MemoryOrderingV1::Acquire => matches!(
            failure,
            Gfx942MemoryOrderingV1::Relaxed | Gfx942MemoryOrderingV1::Acquire
        ),
        Gfx942MemoryOrderingV1::Release => matches!(failure, Gfx942MemoryOrderingV1::Relaxed),
        Gfx942MemoryOrderingV1::AcquireRelease => matches!(
            failure,
            Gfx942MemoryOrderingV1::Relaxed | Gfx942MemoryOrderingV1::Acquire
        ),
        Gfx942MemoryOrderingV1::SequentiallyConsistent => matches!(
            failure,
            Gfx942MemoryOrderingV1::Relaxed
                | Gfx942MemoryOrderingV1::Acquire
                | Gfx942MemoryOrderingV1::SequentiallyConsistent
        ),
    }
}

const fn supported_power_of_two(value: u32, maximum: u32) -> bool {
    value != 0 && value <= maximum && value.is_power_of_two()
}

fn digest_is_zero(digest: IdentityDigestV1) -> bool {
    digest.as_bytes() == &[0; IDENTITY_DIGEST_BYTES_V1]
}
