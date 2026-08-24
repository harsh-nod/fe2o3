use std::error::Error;
use std::fmt;

use fe2o3_kernel_ir::{
    AccessMode, KernelId, KernelIrDecodeError, KernelIrEncodeError, Module, ScalarType,
    VerifiedCanonicalKernelIrIdentityV6, VerifiedCanonicalKernelIrV6, decode_module_v6,
    encode_module_v6,
};

const HARD_MAX_CANONICAL_BYTES_V1: usize = 16 * 1024 * 1024;
const HARD_MAX_REACHABLE_FUNCTIONS_V1: usize = 16_384;
const HARD_MAX_REACHABLE_OPERATIONS_V1: usize = 4 * 1024 * 1024;
const HARD_MAX_INVOCATIONS_V1: u64 = 1 << 32;
const HARD_MAX_WORKGROUPS_V1: u64 = 1 << 32;
const HARD_MAX_SCHEDULED_SLOTS_V1: u64 = 1 << 32;
const HARD_MAX_STEPS_V1: u64 = 1 << 40;
const HARD_MAX_CALL_DEPTH_V1: usize = 1_024;
const HARD_MAX_SSA_VALUES_V1: usize = 1 << 20;
const HARD_MAX_ALLOCATIONS_V1: usize = 1 << 20;
const HARD_MAX_ALLOCATION_BYTES_V1: usize = 1 << 32;
const HARD_MAX_TOTAL_BYTES_V1: usize = 1 << 32;
const HARD_MAX_RESIDENT_BYTES_V1: usize = 1 << 34;
const HARD_MAX_EVENTS_V1: u64 = 1 << 40;
const HARD_MAX_MEMORY_ACCESS_RECORDS_V1: usize = 1 << 20;

/// Explicit resource limits for admission, preflight, and one simulation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulationLimitsV1 {
    /// Maximum canonical KIR bytes accepted by simulator admission.
    ///
    /// Construction of the verified canonical owner occurs before simulator
    /// admission and is governed by the frozen KIR wire/count/depth caps.
    pub max_canonical_bytes: usize,
    /// Maximum functions in the selected kernel's reachable call graph.
    pub max_reachable_functions: usize,
    /// Maximum operations in that reachable call graph.
    pub max_reachable_operations: usize,
    /// Maximum logical invocations in one launch.
    pub max_invocations: u64,
    /// Maximum physical workgroups in one launch.
    pub max_workgroups: u64,
    /// Maximum workitems visited, including padded tail slots.
    pub max_scheduled_slots: u64,
    /// Maximum operations and terminators executed across the launch.
    pub max_steps: u64,
    /// Maximum nested internal calls.
    pub max_call_depth: usize,
    /// Maximum live SSA values in one function frame.
    pub max_ssa_values: usize,
    /// Maximum allocations created by one simulation.
    pub max_allocations: usize,
    /// Maximum bytes in one allocation.
    pub max_allocation_bytes: usize,
    /// Maximum bytes across all live allocations.
    pub max_total_bytes: usize,
    /// Maximum accounted bytes accepted for retained admission and for
    /// preflight/execution resident peaks, including inputs, state, and outputs.
    ///
    /// This is not a pre-decode allocator cap. Constructing the canonical owner
    /// and a simulator decode/re-encode that is later rejected may transiently
    /// exceed it; those phases are bounded by `max_canonical_bytes` and the
    /// frozen KIR wire/count/depth caps.
    pub max_resident_bytes: usize,
    /// Maximum ephemeral events delivered to a sink.
    pub max_events: u64,
    /// Maximum byte-granular access records retained for conflict assessment.
    pub max_memory_access_records: usize,
}

impl Default for SimulationLimitsV1 {
    fn default() -> Self {
        Self {
            max_canonical_bytes: 16 * 1024 * 1024,
            max_reachable_functions: 4_096,
            max_reachable_operations: 1 << 20,
            max_invocations: 1 << 20,
            max_workgroups: 1 << 20,
            max_scheduled_slots: 1 << 22,
            max_steps: 1 << 28,
            max_call_depth: 128,
            max_ssa_values: 65_536,
            max_allocations: 65_536,
            max_allocation_bytes: 1 << 30,
            max_total_bytes: 1 << 30,
            max_resident_bytes: 1 << 32,
            max_events: 1 << 24,
            max_memory_access_records: 65_536,
        }
    }
}

impl SimulationLimitsV1 {
    /// Rejects zero limits and values above immutable implementation caps.
    pub fn validate(self) -> Result<Self, SimulationLimitsErrorV1> {
        macro_rules! check {
            ($field:ident, $hard:expr) => {
                if self.$field == 0 {
                    return Err(SimulationLimitsErrorV1::Zero(stringify!($field)));
                }
                if self.$field > $hard {
                    return Err(SimulationLimitsErrorV1::AboveHardCap(stringify!($field)));
                }
            };
        }
        check!(max_canonical_bytes, HARD_MAX_CANONICAL_BYTES_V1);
        check!(max_reachable_functions, HARD_MAX_REACHABLE_FUNCTIONS_V1);
        check!(max_reachable_operations, HARD_MAX_REACHABLE_OPERATIONS_V1);
        check!(max_invocations, HARD_MAX_INVOCATIONS_V1);
        check!(max_workgroups, HARD_MAX_WORKGROUPS_V1);
        check!(max_scheduled_slots, HARD_MAX_SCHEDULED_SLOTS_V1);
        check!(max_steps, HARD_MAX_STEPS_V1);
        check!(max_call_depth, HARD_MAX_CALL_DEPTH_V1);
        check!(max_ssa_values, HARD_MAX_SSA_VALUES_V1);
        check!(max_allocations, HARD_MAX_ALLOCATIONS_V1);
        check!(max_allocation_bytes, HARD_MAX_ALLOCATION_BYTES_V1);
        check!(max_total_bytes, HARD_MAX_TOTAL_BYTES_V1);
        check!(max_resident_bytes, HARD_MAX_RESIDENT_BYTES_V1);
        check!(max_events, HARD_MAX_EVENTS_V1);
        check!(max_memory_access_records, HARD_MAX_MEMORY_ACCESS_RECORDS_V1);
        Ok(self)
    }
}

/// Invalid simulation limit configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SimulationLimitsErrorV1 {
    /// A limit was configured as zero.
    Zero(&'static str),
    /// A limit exceeded its immutable hard cap.
    AboveHardCap(&'static str),
}

impl fmt::Display for SimulationLimitsErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero(field) => write!(formatter, "simulation limit {field} must be nonzero"),
            Self::AboveHardCap(field) => {
                write!(formatter, "simulation limit {field} exceeds its hard cap")
            }
        }
    }
}

impl Error for SimulationLimitsErrorV1 {}

/// Pointer-sized integer width used by the deterministic CPU target profile.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IndexWidthV1 {
    /// A 32-bit target index.
    Bits32,
    /// A 64-bit target index.
    Bits64,
}

impl IndexWidthV1 {
    pub(crate) const fn bits(self) -> u16 {
        match self {
            Self::Bits32 => 32,
            Self::Bits64 => 64,
        }
    }
}

/// Explicit scalar layout profile used by simulation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulationTargetV1 {
    index_width: IndexWidthV1,
}

impl SimulationTargetV1 {
    /// Constructs a little-endian profile with the selected index width.
    pub const fn little_endian(index_width: IndexWidthV1) -> Self {
        Self { index_width }
    }

    /// The production AMDGPU-compatible 64-bit little-endian scalar profile.
    pub const fn amdgpu_64() -> Self {
        Self::little_endian(IndexWidthV1::Bits64)
    }

    /// Returns the target index width.
    pub const fn index_width(self) -> IndexWidthV1 {
        self.index_width
    }

    /// Maximum target-legal workitems in one workgroup.
    pub const fn max_workgroup_invocations(self) -> u64 {
        1_024
    }

    pub(crate) const fn scalar_bits(self, ty: ScalarType) -> Option<u16> {
        match ty {
            ScalarType::Index => Some(self.index_width.bits()),
            ScalarType::Bool
            | ScalarType::I8
            | ScalarType::I16
            | ScalarType::I32
            | ScalarType::I64
            | ScalarType::I128
            | ScalarType::U8
            | ScalarType::U16
            | ScalarType::U32
            | ScalarType::U64
            | ScalarType::U128 => ty.bit_width(),
            ScalarType::F16 | ScalarType::Bf16 | ScalarType::F32 | ScalarType::F64 => None,
        }
    }

    pub(crate) const fn scalar_bytes(self, ty: ScalarType) -> Option<usize> {
        match self.scalar_bits(ty) {
            Some(1) => Some(1),
            Some(bits) => Some((bits / 8) as usize),
            None => None,
        }
    }
}

/// Exact boolean or integer scalar bits tagged with their KIR type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ScalarBitsV1 {
    ty: ScalarType,
    bits: u128,
    index_width: Option<IndexWidthV1>,
}

impl ScalarBitsV1 {
    /// Constructs a scalar and rejects floats or high bits outside its target width.
    pub fn new(
        ty: ScalarType,
        bits: u128,
        target: SimulationTargetV1,
    ) -> Result<Self, ScalarBitsErrorV1> {
        let width = target
            .scalar_bits(ty)
            .ok_or(ScalarBitsErrorV1::UnsupportedType(ty))?;
        let mask = mask(width);
        if bits & !mask != 0 || (ty == ScalarType::Bool && bits > 1) {
            return Err(ScalarBitsErrorV1::OutOfRange { ty, bits });
        }
        Ok(Self {
            ty,
            bits,
            index_width: (ty == ScalarType::Index).then_some(target.index_width()),
        })
    }

    /// Constructs a boolean scalar.
    pub const fn boolean(value: bool) -> Self {
        Self {
            ty: ScalarType::Bool,
            bits: value as u128,
            index_width: None,
        }
    }

    /// Constructs an `i32` scalar preserving two's-complement bits.
    pub const fn i32(value: i32) -> Self {
        Self {
            ty: ScalarType::I32,
            bits: value as u32 as u128,
            index_width: None,
        }
    }

    /// Constructs a `u32` scalar.
    pub const fn u32(value: u32) -> Self {
        Self {
            ty: ScalarType::U32,
            bits: value as u128,
            index_width: None,
        }
    }

    /// Constructs an index scalar after checking the selected target width.
    pub fn index(value: u64, target: SimulationTargetV1) -> Result<Self, ScalarBitsErrorV1> {
        Self::new(ScalarType::Index, value as u128, target)
    }

    /// Returns the exact KIR scalar type.
    pub const fn ty(self) -> ScalarType {
        self.ty
    }

    /// Returns normalized raw scalar bits.
    pub const fn bits(self) -> u128 {
        self.bits
    }

    /// Returns the boolean value when this scalar is a boolean.
    pub const fn as_bool(self) -> Option<bool> {
        match self.ty {
            ScalarType::Bool => Some(self.bits != 0),
            _ => None,
        }
    }

    pub(crate) fn matches_target(self, target: SimulationTargetV1) -> bool {
        match self.index_width {
            Some(width) => width == target.index_width(),
            None => true,
        }
    }
}

/// Invalid construction of typed simulation scalar bits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScalarBitsErrorV1 {
    /// The type has no scalar semantics in this simulation profile.
    UnsupportedType(ScalarType),
    /// Bits were not normalized to the type's target width.
    OutOfRange { ty: ScalarType, bits: u128 },
}

impl fmt::Display for ScalarBitsErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedType(ty) => write!(formatter, "unsupported scalar type {ty:?}"),
            Self::OutOfRange { ty, bits } => {
                write!(formatter, "scalar bits {bits:#x} are outside {ty:?}")
            }
        }
    }
}

impl Error for ScalarBitsErrorV1 {}

/// One byte-addressed host buffer copied into a provenance-tracked allocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BufferArgumentV1 {
    element: ScalarType,
    access: AccessMode,
    alignment: u32,
    bytes: Vec<u8>,
    initialized: Vec<bool>,
    index_width: Option<IndexWidthV1>,
}

impl BufferArgumentV1 {
    /// Constructs a buffer with explicit initialization state for every byte.
    pub fn new(
        element: ScalarType,
        access: AccessMode,
        alignment: u32,
        bytes: Vec<u8>,
        initialized: Vec<bool>,
        target: SimulationTargetV1,
    ) -> Result<Self, BufferArgumentErrorV1> {
        let element_bytes = target
            .scalar_bytes(element)
            .ok_or(BufferArgumentErrorV1::UnsupportedElement(element))?;
        if alignment == 0 || !alignment.is_power_of_two() {
            return Err(BufferArgumentErrorV1::InvalidAlignment(alignment));
        }
        if bytes.len() != initialized.len() {
            return Err(BufferArgumentErrorV1::InitializationLength {
                bytes: bytes.len(),
                initialized: initialized.len(),
            });
        }
        if !bytes.len().is_multiple_of(element_bytes) {
            return Err(BufferArgumentErrorV1::PartialElement {
                bytes: bytes.len(),
                element_bytes,
            });
        }
        Ok(Self {
            element,
            access,
            alignment,
            bytes,
            initialized,
            index_width: (element == ScalarType::Index).then_some(target.index_width()),
        })
    }

    /// Constructs a fully initialized buffer from exact little-endian scalars.
    pub fn from_scalars(
        access: AccessMode,
        alignment: u32,
        values: &[ScalarBitsV1],
        target: SimulationTargetV1,
    ) -> Result<Self, BufferArgumentErrorV1> {
        let element = values
            .first()
            .map(|value| value.ty())
            .ok_or(BufferArgumentErrorV1::EmptyScalarInput)?;
        if values.iter().any(|value| value.ty() != element) {
            return Err(BufferArgumentErrorV1::MixedElementTypes);
        }
        if values.iter().any(|value| !value.matches_target(target)) {
            return Err(BufferArgumentErrorV1::TargetLayoutMismatch);
        }
        let element_bytes = target
            .scalar_bytes(element)
            .ok_or(BufferArgumentErrorV1::UnsupportedElement(element))?;
        let byte_len = values
            .len()
            .checked_mul(element_bytes)
            .ok_or(BufferArgumentErrorV1::ByteLengthOverflow)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(byte_len)
            .map_err(|_| BufferArgumentErrorV1::AllocationFailure)?;
        for value in values {
            bytes.extend_from_slice(&value.bits().to_le_bytes()[..element_bytes]);
        }
        let mut initialized = Vec::new();
        initialized
            .try_reserve_exact(byte_len)
            .map_err(|_| BufferArgumentErrorV1::AllocationFailure)?;
        initialized.resize(byte_len, true);
        Self::new(element, access, alignment, bytes, initialized, target)
    }

    /// Returns the exact scalar element type.
    pub const fn element(&self) -> ScalarType {
        self.element
    }

    /// Returns the allocation access mode.
    pub const fn access(&self) -> AccessMode {
        self.access
    }

    /// Returns the promised base alignment.
    pub const fn alignment(&self) -> u32 {
        self.alignment
    }

    /// Returns exact allocation bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns byte initialization state.
    pub fn initialized(&self) -> &[bool] {
        &self.initialized
    }

    /// Returns the number of scalar elements.
    pub fn element_count(
        &self,
        target: SimulationTargetV1,
    ) -> Result<usize, BufferArgumentErrorV1> {
        if !self.matches_target(target) {
            return Err(BufferArgumentErrorV1::TargetLayoutMismatch);
        }
        let element_bytes = target
            .scalar_bytes(self.element)
            .ok_or(BufferArgumentErrorV1::UnsupportedElement(self.element))?;
        Ok(self.bytes.len() / element_bytes)
    }

    pub(crate) fn with_contents(&self, bytes: Vec<u8>, initialized: Vec<bool>) -> Self {
        Self {
            element: self.element,
            access: self.access,
            alignment: self.alignment,
            bytes,
            initialized,
            index_width: self.index_width,
        }
    }

    pub(crate) fn matches_target(&self, target: SimulationTargetV1) -> bool {
        match self.index_width {
            Some(width) => width == target.index_width(),
            None => true,
        }
    }

    pub(crate) fn retained_payload_capacity_bytes(&self) -> Option<usize> {
        self.bytes
            .capacity()
            .checked_mul(std::mem::size_of::<u8>())?
            .checked_add(crate::resident::bool_vec_storage_bytes(
                self.initialized.capacity(),
            )?)
    }
}

/// Invalid byte-addressed buffer argument.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BufferArgumentErrorV1 {
    UnsupportedElement(ScalarType),
    InvalidAlignment(u32),
    InitializationLength { bytes: usize, initialized: usize },
    PartialElement { bytes: usize, element_bytes: usize },
    EmptyScalarInput,
    MixedElementTypes,
    ByteLengthOverflow,
    AllocationFailure,
    TargetLayoutMismatch,
}

impl fmt::Display for BufferArgumentErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid simulation buffer: {self:?}")
    }
}

impl Error for BufferArgumentErrorV1 {}

/// One typed kernel argument.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SimulationArgumentV1 {
    /// A by-value boolean or integer scalar.
    Scalar(ScalarBitsV1),
    /// A global buffer supplied as a KIR pointer or slice.
    Buffer(BufferArgumentV1),
    /// An ABI pointer or slice view into a named shared backing allocation.
    BufferView(BufferViewArgumentV1),
}

/// Stable request-local identity for one shared backing allocation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BufferBackingIdV1(pub u32);

/// One named buffer allocation that may be viewed by multiple ABI arguments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SharedBufferV1 {
    pub id: BufferBackingIdV1,
    pub buffer: BufferArgumentV1,
}

/// A bounded element view into a request-local shared backing allocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BufferViewArgumentV1 {
    backing: BufferBackingIdV1,
    element: ScalarType,
    access: AccessMode,
    alignment: u32,
    byte_offset: usize,
    elements: usize,
    index_width: Option<IndexWidthV1>,
}

impl BufferViewArgumentV1 {
    pub fn new(
        backing: BufferBackingIdV1,
        element: ScalarType,
        access: AccessMode,
        alignment: u32,
        byte_offset: usize,
        elements: usize,
        target: SimulationTargetV1,
    ) -> Result<Self, BufferArgumentErrorV1> {
        if alignment == 0
            || !alignment.is_power_of_two()
            || !byte_offset.is_multiple_of(alignment as usize)
        {
            return Err(BufferArgumentErrorV1::InvalidAlignment(alignment));
        }
        let element_bytes = target
            .scalar_bytes(element)
            .ok_or(BufferArgumentErrorV1::UnsupportedElement(element))?;
        elements
            .checked_mul(element_bytes)
            .and_then(|bytes| byte_offset.checked_add(bytes))
            .ok_or(BufferArgumentErrorV1::ByteLengthOverflow)?;
        Ok(Self {
            backing,
            element,
            access,
            alignment,
            byte_offset,
            elements,
            index_width: (element == ScalarType::Index).then_some(target.index_width()),
        })
    }

    pub const fn backing(&self) -> BufferBackingIdV1 {
        self.backing
    }

    pub const fn element(&self) -> ScalarType {
        self.element
    }

    pub const fn access(&self) -> AccessMode {
        self.access
    }

    pub const fn alignment(&self) -> u32 {
        self.alignment
    }

    pub const fn byte_offset(&self) -> usize {
        self.byte_offset
    }

    pub const fn elements(&self) -> usize {
        self.elements
    }

    pub(crate) fn matches_target(&self, target: SimulationTargetV1) -> bool {
        match self.index_width {
            Some(width) => width == target.index_width(),
            None => true,
        }
    }

    pub(crate) fn byte_len(
        &self,
        target: SimulationTargetV1,
    ) -> Result<usize, BufferArgumentErrorV1> {
        if !self.matches_target(target) {
            return Err(BufferArgumentErrorV1::TargetLayoutMismatch);
        }
        self.elements
            .checked_mul(
                target
                    .scalar_bytes(self.element)
                    .ok_or(BufferArgumentErrorV1::UnsupportedElement(self.element))?,
            )
            .ok_or(BufferArgumentErrorV1::ByteLengthOverflow)
    }
}

/// Three-dimensional global launch extent. Inactive axes must be one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GridShapeV1(pub [u64; 3]);

/// Three-dimensional workgroup extent. Inactive axes must be one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkgroupShapeV1(pub [u32; 3]);

/// Whether ephemeral execution events are delivered to the selected sink.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventPolicyV1 {
    Disabled,
    Enabled,
}

/// One immutable simulation request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationRequestV1 {
    /// Selected KIR kernel identity.
    pub kernel: KernelId,
    /// Exact global launch dimensions.
    pub grid: GridShapeV1,
    /// Exact workgroup dimensions.
    pub workgroup: WorkgroupShapeV1,
    /// Typed scalar and buffer arguments.
    pub arguments: Vec<SimulationArgumentV1>,
    /// Named backing allocations used by `BufferView` arguments.
    pub shared_buffers: Vec<SharedBufferV1>,
    /// Ephemeral diagnostic event policy.
    pub events: EventPolicyV1,
}

impl SimulationRequestV1 {
    /// Constructs a request with events disabled.
    pub fn new(
        kernel: impl Into<KernelId>,
        grid: [u64; 3],
        workgroup: [u32; 3],
        arguments: Vec<SimulationArgumentV1>,
    ) -> Self {
        Self {
            kernel: kernel.into(),
            grid: GridShapeV1(grid),
            workgroup: WorkgroupShapeV1(workgroup),
            arguments,
            shared_buffers: Vec::new(),
            events: EventPolicyV1::Disabled,
        }
    }

    /// Adds named backing allocations for aliased or offset argument views.
    pub fn with_shared_buffers(mut self, shared_buffers: Vec<SharedBufferV1>) -> Self {
        self.shared_buffers = shared_buffers;
        self
    }
}

/// Exact execution hierarchy coordinates for one logical invocation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SimulationInvocationV1 {
    pub global: [u64; 3],
    pub workgroup: [u64; 3],
    pub local: [u32; 3],
    pub workgroup_size: [u32; 3],
    pub workgroup_count: [u64; 3],
    pub launch_extent: [u64; 3],
}

/// Stable in-memory KIR site used by diagnostics and ephemeral events.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SimulationSiteV1 {
    pub function: fe2o3_kernel_ir::FunctionId,
    pub block: fe2o3_kernel_ir::BlockId,
    pub operation: Option<u32>,
}

/// Exact V6 owner admitted for simulation. This owner is intentionally not `Clone`.
#[derive(Debug)]
pub struct AdmittedSimulationModuleV1 {
    pub(crate) identity: VerifiedCanonicalKernelIrIdentityV6,
    pub(crate) module: Module,
    pub(crate) admitted_resident_bytes: usize,
}

impl AdmittedSimulationModuleV1 {
    /// Consumes exact verified V6 custody and retains a bounded decoded execution view.
    ///
    /// `max_resident_bytes` is evaluated after canonical decode/re-encode, when
    /// retained container capacities are known. A rejected attempt may therefore
    /// transiently exceed that setting, but remains bounded by the canonical-byte
    /// limit and frozen KIR wire/count/depth caps.
    pub fn admit(
        canonical: VerifiedCanonicalKernelIrV6,
        limits: SimulationLimitsV1,
    ) -> Result<Self, SimulationAdmissionErrorV1> {
        let limits = limits
            .validate()
            .map_err(SimulationAdmissionErrorV1::InvalidLimits)?;
        if canonical.canonical_bytes().len() > limits.max_canonical_bytes {
            return Err(SimulationAdmissionErrorV1::CanonicalBytesLimit {
                actual: canonical.canonical_bytes().len(),
                limit: limits.max_canonical_bytes,
            });
        }
        let identity = *canonical.identity();
        let bytes = canonical.into_canonical_bytes();
        let module =
            decode_module_v6(&bytes).map_err(SimulationAdmissionErrorV1::DecodeAfterAdmission)?;
        let reencoded =
            encode_module_v6(&module).map_err(SimulationAdmissionErrorV1::EncodeAfterAdmission)?;
        let admitted_resident_bytes = std::mem::size_of::<Self>()
            .checked_add(
                crate::resident::module_retained_heap_bytes(&module)
                    .ok_or(SimulationAdmissionErrorV1::ResidentBytesOverflow)?,
            )
            .ok_or(SimulationAdmissionErrorV1::ResidentBytesOverflow)?;
        let decode_peak = admitted_resident_bytes
            .checked_add(bytes.capacity())
            .and_then(|bytes| bytes.checked_add(reencoded.capacity()))
            .and_then(|bytes| bytes.checked_add(2 * std::mem::size_of::<Vec<u8>>()))
            .ok_or(SimulationAdmissionErrorV1::ResidentBytesOverflow)?;
        if decode_peak > limits.max_resident_bytes {
            return Err(SimulationAdmissionErrorV1::ResidentBytesLimit {
                phase: "post-decode canonical admission",
                actual: decode_peak,
                limit: limits.max_resident_bytes,
            });
        }
        Ok(Self {
            identity,
            module,
            admitted_resident_bytes,
        })
    }

    /// Returns the exact verified canonical KIR identity.
    pub const fn identity(&self) -> &VerifiedCanonicalKernelIrIdentityV6 {
        &self.identity
    }

    /// Returns the decoded, already verified inspection view.
    pub const fn module(&self) -> &Module {
        &self.module
    }

    /// Returns the conservative accounted upper bound retained by this admitted
    /// decoded module.
    ///
    /// Standard-container payload capacities are counted. Allocator metadata
    /// and page rounding are outside the stable resident-byte contract.
    pub const fn admitted_resident_bytes(&self) -> usize {
        self.admitted_resident_bytes
    }

    /// Simulation admission never grants compiler, proof, artifact, load, or launch authority.
    pub const fn grants_execution_authority(&self) -> bool {
        false
    }
}

/// Failure to admit exact canonical KIR V6 for simulation.
#[derive(Debug)]
pub enum SimulationAdmissionErrorV1 {
    InvalidLimits(SimulationLimitsErrorV1),
    CanonicalBytesLimit {
        actual: usize,
        limit: usize,
    },
    DecodeAfterAdmission(KernelIrDecodeError),
    EncodeAfterAdmission(KernelIrEncodeError),
    ResidentBytesOverflow,
    /// The fully measured admission peak exceeded the successful-admission cap.
    ///
    /// This is a post-decode rejection: canonical construction and this rejected
    /// decode/re-encode may already have transiently exceeded `limit`.
    ResidentBytesLimit {
        phase: &'static str,
        actual: usize,
        limit: usize,
    },
}

impl fmt::Display for SimulationAdmissionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLimits(error) => error.fmt(formatter),
            Self::CanonicalBytesLimit { actual, limit } => {
                write!(
                    formatter,
                    "canonical KIR bytes {actual} exceed simulation limit {limit}"
                )
            }
            Self::DecodeAfterAdmission(error) => {
                write!(
                    formatter,
                    "admitted canonical KIR failed to decode: {error}"
                )
            }
            Self::EncodeAfterAdmission(error) => {
                write!(
                    formatter,
                    "admitted decoded KIR failed canonical re-encoding: {error}"
                )
            }
            Self::ResidentBytesOverflow => {
                write!(
                    formatter,
                    "admitted module resident-byte accounting overflowed"
                )
            }
            Self::ResidentBytesLimit {
                phase,
                actual,
                limit,
            } => write!(
                formatter,
                "{phase} accounted resident bytes {actual} exceed successful-admission limit {limit}; the rejected decode/re-encode was bounded by canonical KIR limits, not by this post-decode setting"
            ),
        }
    }
}

impl Error for SimulationAdmissionErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidLimits(error) => Some(error),
            Self::DecodeAfterAdmission(error) => Some(error),
            Self::EncodeAfterAdmission(error) => Some(error),
            Self::CanonicalBytesLimit { .. }
            | Self::ResidentBytesOverflow
            | Self::ResidentBytesLimit { .. } => None,
        }
    }
}

pub(crate) const fn mask(width: u16) -> u128 {
    if width == 128 {
        u128::MAX
    } else {
        (1_u128 << width) - 1
    }
}
