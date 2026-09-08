//! Canonical compiler-emitted LLVM-to-machine correspondence.
//!
//! The record is intentionally target-neutral and contains no disassembly text. A measured
//! backend emits native LLVM coordinates, typed semantic effects, and exact machine offsets. A
//! target adapter must still decode the final ISA independently and validate those claims. Merely
//! decoding this record never establishes instruction-selection semantics.

use core::fmt;
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};

use crate::{CheckedPostLlvmStageContentsV1, ExactCompilerStageContentIdentityV1};

/// Canonical transcript magic.
pub const COMPILER_INSTRUCTION_SELECTION_CORRESPONDENCE_MAGIC_V1: [u8; 8] = *b"F2ISEL01";
/// Canonical transcript version.
pub const COMPILER_INSTRUCTION_SELECTION_CORRESPONDENCE_VERSION_V1: u16 = 1;
/// Maximum canonical transcript bytes.
pub const MAX_COMPILER_INSTRUCTION_SELECTION_CORRESPONDENCE_BYTES_V1: usize = 64 * 1024 * 1024;
/// Maximum functions in one transcript.
pub const MAX_COMPILER_INSTRUCTION_SELECTION_FUNCTIONS_V1: usize = 1_024;
/// Maximum LLVM blocks in one transcript.
pub const MAX_COMPILER_INSTRUCTION_SELECTION_BLOCKS_V1: usize = 16_384;
/// Maximum LLVM operations in one transcript.
pub const MAX_COMPILER_INSTRUCTION_SELECTION_OPERATIONS_V1: usize = 1_048_576;
/// Maximum operands or phi inputs attached to one operation.
pub const MAX_COMPILER_INSTRUCTION_SELECTION_OPERATION_INPUTS_V1: usize = 256;
/// Maximum direct CFG successors attached to one block.
pub const MAX_COMPILER_INSTRUCTION_SELECTION_SUCCESSORS_V1: usize = 256;
/// Maximum machine instructions attributed to one LLVM operation.
pub const MAX_COMPILER_INSTRUCTION_SELECTION_MACHINE_OFFSETS_V1: usize = 4_096;
/// Maximum value-location bindings attached to one LLVM operation.
pub const MAX_COMPILER_INSTRUCTION_SELECTION_VALUE_BINDINGS_V1: usize = 8_192;
/// Maximum affine terms in one normalized effective-address expression.
pub const MAX_COMPILER_INSTRUCTION_SELECTION_ADDRESS_TERMS_V1: usize = 256;

const IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/COMPILER-INSTRUCTION-SELECTION-CORRESPONDENCE/V1\0";

/// Target architecture selected by the backend.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MachineRefinementArchitectureV1 {
    /// AMD GCN/CDNA machine code.
    AmdGcn,
}

/// One exact LLVM operation coordinate in the final post-optimization module.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerLlvmOperationCoordinateV1 {
    function: u32,
    block: u32,
    operation: u32,
}

impl CompilerLlvmOperationCoordinateV1 {
    /// Creates one coordinate.
    pub const fn new(function: u32, block: u32, operation: u32) -> Self {
        Self {
            function,
            block,
            operation,
        }
    }

    /// Returns the function ordinal.
    pub const fn function(self) -> u32 {
        self.function
    }

    /// Returns the block ordinal.
    pub const fn block(self) -> u32 {
        self.block
    }

    /// Returns the operation ordinal.
    pub const fn operation(self) -> u32 {
        self.operation
    }
}

/// Closed semantic operation classes admitted by the correspondence checker.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CompilerLlvmOperationKindV1 {
    /// Materialized or folded constant.
    Constant,
    /// Vector construction or extraction.
    Vector,
    /// Global or function address materialization.
    SymbolAddress,
    /// Integer addition.
    IntegerAdd,
    /// Integer subtraction.
    IntegerSubtract,
    /// Integer multiplication.
    IntegerMultiply,
    /// Signed or unsigned integer division.
    IntegerDivide,
    /// Signed or unsigned integer remainder.
    IntegerRemainder,
    /// Integer bitwise operation.
    IntegerBitwise,
    /// Integer shift.
    IntegerShift,
    /// Floating-point addition.
    FloatAdd,
    /// Floating-point subtraction.
    FloatSubtract,
    /// Floating-point multiplication.
    FloatMultiply,
    /// Floating-point division.
    FloatDivide,
    /// Fused multiply-add.
    FloatFma,
    /// Floating-point square root.
    FloatSqrt,
    /// Integer comparison.
    IntegerCompare,
    /// Floating-point comparison.
    FloatCompare,
    /// Integer, floating-point, vector, or pointer cast.
    Cast,
    /// SSA select.
    Select,
    /// Stack or private allocation.
    Allocate,
    /// Typed effective-address calculation.
    Address,
    /// Memory load.
    Load,
    /// Memory store.
    Store,
    /// Atomic memory operation.
    Atomic,
    /// Memory fence.
    Fence,
    /// Execution and memory barrier.
    Barrier,
    /// Direct function call.
    Call,
    /// Closed target intrinsic.
    Intrinsic,
    /// SSA phi.
    Phi,
    /// Unconditional branch.
    Branch,
    /// Conditional branch.
    ConditionalBranch,
    /// Value or integer switch.
    Switch,
    /// Return.
    Return,
    /// Unreachable terminator.
    Unreachable,
    /// Lane or invocation identity operation.
    InvocationIndex,
    /// Subgroup or workgroup collective.
    Collective,
    /// Matrix operation.
    Matrix,
    /// Closed target inline-assembly operation.
    InlineAssembly,
    /// Explicit trap.
    Trap,
}

/// Width and representation of one LLVM SSA value.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CompilerLlvmValueTypeV1 {
    /// No result.
    Void,
    /// Integer scalar with an exact bit width.
    Integer(u16),
    /// IEEE or target floating scalar with an exact bit width.
    Float(u16),
    /// Pointer with exact address space and representation width.
    Pointer {
        /// LLVM address-space number.
        address_space: u32,
        /// Pointer representation width.
        bits: u16,
    },
    /// Fixed vector.
    Vector {
        /// Element width in bits.
        element_bits: u16,
        /// Lane count.
        lanes: u16,
        /// Whether elements use floating representation.
        floating: bool,
    },
}

/// Memory effect kind for one exact LLVM operation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CompilerMemoryEffectKindV1 {
    /// No memory effect.
    None,
    /// Read only.
    Read,
    /// Write only.
    Write,
    /// Read and write one location atomically or non-atomically.
    ReadWrite,
    /// Fence without an address.
    Fence,
    /// Collective execution/memory barrier.
    Barrier,
}

/// LLVM memory ordering retained without target reinterpretation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CompilerMemoryOrderingV1 {
    /// Non-atomic access.
    NotAtomic,
    /// Relaxed/monotonic atomic ordering.
    Relaxed,
    /// Acquire ordering.
    Acquire,
    /// Release ordering.
    Release,
    /// Acquire-release ordering.
    AcquireRelease,
    /// Sequentially consistent ordering.
    SequentiallyConsistent,
}

/// Synchronization scope retained from LLVM.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CompilerMemoryScopeV1 {
    /// No synchronization scope.
    None,
    /// One invocation.
    Invocation,
    /// One subgroup/wave.
    Subgroup,
    /// One workgroup.
    Workgroup,
    /// One agent/device.
    Agent,
    /// System scope.
    System,
}

/// Exact typed memory contract for one LLVM operation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerMemoryEffectV1 {
    kind: CompilerMemoryEffectKindV1,
    address_space: u32,
    byte_width: u32,
    alignment: u32,
    ordering: CompilerMemoryOrderingV1,
    scope: CompilerMemoryScopeV1,
    volatile: bool,
}

impl CompilerMemoryEffectV1 {
    /// Creates and validates one memory contract.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        kind: CompilerMemoryEffectKindV1,
        address_space: u32,
        byte_width: u32,
        alignment: u32,
        ordering: CompilerMemoryOrderingV1,
        scope: CompilerMemoryScopeV1,
        volatile: bool,
    ) -> Result<Self, CompilerInstructionSelectionCorrespondenceErrorV1> {
        let effect = Self {
            kind,
            address_space,
            byte_width,
            alignment,
            ordering,
            scope,
            volatile,
        };
        validate_memory_effect(effect)?;
        Ok(effect)
    }

    /// Returns an effect-free contract.
    pub const fn none() -> Self {
        Self {
            kind: CompilerMemoryEffectKindV1::None,
            address_space: 0,
            byte_width: 0,
            alignment: 0,
            ordering: CompilerMemoryOrderingV1::NotAtomic,
            scope: CompilerMemoryScopeV1::None,
            volatile: false,
        }
    }

    /// Returns the effect kind.
    pub const fn kind(self) -> CompilerMemoryEffectKindV1 {
        self.kind
    }

    /// Returns the LLVM address-space number.
    pub const fn address_space(self) -> u32 {
        self.address_space
    }

    /// Returns the exact accessed byte width.
    pub const fn byte_width(self) -> u32 {
        self.byte_width
    }

    /// Returns the required alignment.
    pub const fn alignment(self) -> u32 {
        self.alignment
    }

    /// Returns the memory ordering.
    pub const fn ordering(self) -> CompilerMemoryOrderingV1 {
        self.ordering
    }

    /// Returns the synchronization scope.
    pub const fn scope(self) -> CompilerMemoryScopeV1 {
        self.scope
    }

    /// Reports volatile access semantics.
    pub const fn is_volatile(self) -> bool {
        self.volatile
    }
}

/// Floating-point semantics required by one operation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CompilerNumericalContractV1 {
    /// Exact integer, pointer, or control behavior.
    Exact,
    /// Strict IEEE binary32 with subnormals retained and no contraction.
    IeeeBinary32,
    /// Strict IEEE binary64 with subnormals retained and no contraction.
    IeeeBinary64,
    /// Exact fused binary32 operation with one final rounding.
    IeeeBinary32Fused,
    /// Target-defined matrix or narrow-float behavior requiring a target adapter.
    TargetDefined,
    /// BF16 products accumulated into IEEE binary32 lanes by an MFMA operation.
    MfmaBf16AccumulateF32,
    /// FP8 E4M3 products accumulated into IEEE binary32 lanes by a scaled MFMA operation.
    MfmaFp8E4M3AccumulateF32,
    /// FP8 E5M2/BF8 products accumulated into IEEE binary32 lanes by a scaled MFMA operation.
    MfmaFp8E5M2AccumulateF32,
    /// FP4 E2M1 products accumulated into IEEE binary32 lanes by a scaled MFMA operation.
    MfmaFp4E2M1AccumulateF32,
    /// FP4 E2M1 by FP8 E4M3 products accumulated into IEEE binary32 lanes.
    MfmaFp4E2M1ByFp8E4M3AccumulateF32,
}

/// Divergence class attached by LLVM uniformity analysis.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CompilerBranchDivergenceV1 {
    /// No branch condition.
    None,
    /// Uniform condition represented in scalar control state.
    Uniform,
    /// Per-lane condition requiring EXEC-mask control.
    Divergent,
}

/// One phi incoming value and its predecessor block.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerPhiInputV1 {
    predecessor: u32,
    value: u32,
}

/// Physical transport of one phi value across one predecessor edge.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerPhiEdgeTransportV1 {
    predecessor: u32,
    value: u32,
    incoming: CompilerMachineValueLocationV1,
    result: CompilerMachineRegisterV1,
    move_offset: Option<u64>,
}

/// Target-independent physical register class reported by instruction selection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CompilerMachineRegisterClassV1 {
    /// Scalar general-purpose register.
    Scalar,
    /// Vector general-purpose register.
    Vector,
    /// Matrix accumulator register.
    Accumulator,
    /// Target execution-mask register.
    ExecutionMask,
    /// Vector condition register.
    VectorCondition,
    /// Scalar condition register.
    ScalarCondition,
    /// Target memory-offset register.
    MemoryOffset,
    /// Target floating-point mode register.
    FloatingMode,
}

/// One physical register location emitted by instruction selection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerMachineRegisterV1 {
    class: CompilerMachineRegisterClassV1,
    index: u16,
}

impl CompilerMachineRegisterV1 {
    /// Creates a physical register location.
    pub const fn new(class: CompilerMachineRegisterClassV1, index: u16) -> Self {
        Self { class, index }
    }

    /// Returns the register class.
    pub const fn class(self) -> CompilerMachineRegisterClassV1 {
        self.class
    }

    /// Returns the register index; singleton special registers require zero.
    pub const fn index(self) -> u16 {
        self.index
    }
}

/// Whether a machine location defines or consumes one final-LLVM SSA value.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CompilerMachineValueAccessV1 {
    /// Machine instruction defines the LLVM value.
    Definition,
    /// Machine instruction consumes the LLVM value.
    Use,
}

/// Exact machine location of one final-LLVM SSA value occurrence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CompilerMachineValueLocationV1 {
    /// Value is carried in a physical register.
    Register(CompilerMachineRegisterV1),
    /// Value is encoded as an immediate with an exact little-endian bit pattern.
    Immediate {
        /// Significant immediate width.
        bit_width: u16,
        /// Zero-padded little-endian bits, supporting values through 128 bits.
        bits: [u8; 16],
    },
}

/// One exact LLVM-value-to-machine-location binding.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerMachineValueBindingV1 {
    machine_offset: u64,
    value: u32,
    access: CompilerMachineValueAccessV1,
    location: CompilerMachineValueLocationV1,
}

impl CompilerMachineValueBindingV1 {
    /// Creates one machine location binding.
    pub fn new(
        machine_offset: u64,
        value: u32,
        access: CompilerMachineValueAccessV1,
        location: CompilerMachineValueLocationV1,
    ) -> Result<Self, CompilerInstructionSelectionCorrespondenceErrorV1> {
        let binding = Self {
            machine_offset,
            value,
            access,
            location,
        };
        validate_machine_value_binding(binding)?;
        Ok(binding)
    }

    /// Returns the attributed final-HSACO instruction offset.
    pub const fn machine_offset(self) -> u64 {
        self.machine_offset
    }

    /// Returns the final-LLVM SSA value number.
    pub const fn value(self) -> u32 {
        self.value
    }

    /// Returns definition/use direction.
    pub const fn access(self) -> CompilerMachineValueAccessV1 {
        self.access
    }

    /// Returns the exact machine location.
    pub const fn location(self) -> CompilerMachineValueLocationV1 {
        self.location
    }
}

/// One affine value term in a normalized LLVM effective-address expression.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerAddressTermV1 {
    value: u32,
    scale: i64,
}

impl CompilerAddressTermV1 {
    /// Creates `value * scale`; zero scales are noncanonical.
    pub fn new(
        value: u32,
        scale: i64,
    ) -> Result<Self, CompilerInstructionSelectionCorrespondenceErrorV1> {
        if scale == 0 {
            return Err(CompilerInstructionSelectionCorrespondenceErrorV1::InvalidAddress);
        }
        Ok(Self { value, scale })
    }

    /// Returns the LLVM value number.
    pub const fn value(self) -> u32 {
        self.value
    }

    /// Returns the signed byte scale.
    pub const fn scale(self) -> i64 {
        self.scale
    }
}

/// Normalized effective address `sum(value * scale) + displacement`.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerEffectiveAddressV1 {
    address_space: u32,
    pointer_bits: u16,
    byte_width: u32,
    displacement: i64,
    terms: Box<[CompilerAddressTermV1]>,
}

impl CompilerEffectiveAddressV1 {
    /// Creates one bounded normalized affine address.
    pub fn new(
        address_space: u32,
        pointer_bits: u16,
        byte_width: u32,
        displacement: i64,
        terms: impl Into<Box<[CompilerAddressTermV1]>>,
    ) -> Result<Self, CompilerInstructionSelectionCorrespondenceErrorV1> {
        let address = Self {
            address_space,
            pointer_bits,
            byte_width,
            displacement,
            terms: terms.into(),
        };
        validate_effective_address(&address)?;
        Ok(address)
    }

    /// Returns the LLVM address space.
    pub const fn address_space(&self) -> u32 {
        self.address_space
    }

    /// Returns the pointer arithmetic width.
    pub const fn pointer_bits(&self) -> u16 {
        self.pointer_bits
    }

    /// Returns the accessed byte width.
    pub const fn byte_width(&self) -> u32 {
        self.byte_width
    }

    /// Returns the constant byte displacement.
    pub const fn displacement(&self) -> i64 {
        self.displacement
    }

    /// Returns normalized affine terms sorted by LLVM value number.
    pub fn terms(&self) -> &[CompilerAddressTermV1] {
        &self.terms
    }
}

impl CompilerPhiInputV1 {
    /// Creates one incoming edge/value pair.
    pub const fn new(predecessor: u32, value: u32) -> Self {
        Self { predecessor, value }
    }

    /// Returns the predecessor block ordinal.
    pub const fn predecessor(self) -> u32 {
        self.predecessor
    }

    /// Returns the LLVM value number.
    pub const fn value(self) -> u32 {
        self.value
    }
}

impl CompilerPhiEdgeTransportV1 {
    /// Creates one edge transport. A coalesced edge has no move and must retain one register.
    pub fn new(
        predecessor: u32,
        value: u32,
        incoming: CompilerMachineValueLocationV1,
        result: CompilerMachineRegisterV1,
        move_offset: Option<u64>,
    ) -> Result<Self, CompilerInstructionSelectionCorrespondenceErrorV1> {
        let transport = Self {
            predecessor,
            value,
            incoming,
            result,
            move_offset,
        };
        validate_phi_edge_transport(transport)?;
        Ok(transport)
    }

    /// Returns the LLVM predecessor block ordinal.
    pub const fn predecessor(self) -> u32 {
        self.predecessor
    }

    /// Returns the incoming LLVM SSA value number.
    pub const fn value(self) -> u32 {
        self.value
    }

    /// Returns the incoming physical location on the predecessor edge.
    pub const fn incoming(self) -> CompilerMachineValueLocationV1 {
        self.incoming
    }

    /// Returns the physical register holding the phi result in the destination block.
    pub const fn result(self) -> CompilerMachineRegisterV1 {
        self.result
    }

    /// Returns the edge move offset, or `None` when instruction selection coalesced the locations.
    pub const fn move_offset(self) -> Option<u64> {
        self.move_offset
    }
}

/// One exact final-LLVM basic block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerLlvmBlockV1 {
    function: u32,
    block: u32,
    successors: Box<[u32]>,
}

impl CompilerLlvmBlockV1 {
    /// Creates one block with successors in LLVM terminator order.
    pub fn new(
        function: u32,
        block: u32,
        successors: impl Into<Box<[u32]>>,
    ) -> Result<Self, CompilerInstructionSelectionCorrespondenceErrorV1> {
        let successors = successors.into();
        if successors.len() > MAX_COMPILER_INSTRUCTION_SELECTION_SUCCESSORS_V1
            || has_duplicate(successors.iter().copied())
        {
            return Err(CompilerInstructionSelectionCorrespondenceErrorV1::InvalidControlFlow);
        }
        Ok(Self {
            function,
            block,
            successors,
        })
    }

    /// Returns the function ordinal.
    pub const fn function(&self) -> u32 {
        self.function
    }

    /// Returns the block ordinal.
    pub const fn block(&self) -> u32 {
        self.block
    }

    /// Returns successors in terminator order.
    pub fn successors(&self) -> &[u32] {
        &self.successors
    }
}

/// One compiler-emitted final-LLVM operation and its exact machine attribution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerLlvmOperationV1 {
    coordinate: CompilerLlvmOperationCoordinateV1,
    llvm_opcode: u16,
    kind: CompilerLlvmOperationKindV1,
    semantic_discriminator: u32,
    result: Option<u32>,
    result_type: CompilerLlvmValueTypeV1,
    operands: Box<[u32]>,
    phi_inputs: Box<[CompilerPhiInputV1]>,
    phi_edge_transports: Box<[CompilerPhiEdgeTransportV1]>,
    successors: Box<[u32]>,
    divergence: CompilerBranchDivergenceV1,
    memory: CompilerMemoryEffectV1,
    numerical: CompilerNumericalContractV1,
    machine_offsets: Box<[u64]>,
    machine_value_bindings: Box<[CompilerMachineValueBindingV1]>,
    effective_address: Option<CompilerEffectiveAddressV1>,
}

impl CompilerLlvmOperationV1 {
    /// Creates one bounded typed operation record.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        coordinate: CompilerLlvmOperationCoordinateV1,
        llvm_opcode: u16,
        kind: CompilerLlvmOperationKindV1,
        semantic_discriminator: u32,
        result: Option<u32>,
        result_type: CompilerLlvmValueTypeV1,
        operands: impl Into<Box<[u32]>>,
        phi_inputs: impl Into<Box<[CompilerPhiInputV1]>>,
        phi_edge_transports: impl Into<Box<[CompilerPhiEdgeTransportV1]>>,
        successors: impl Into<Box<[u32]>>,
        divergence: CompilerBranchDivergenceV1,
        memory: CompilerMemoryEffectV1,
        numerical: CompilerNumericalContractV1,
        machine_offsets: impl Into<Box<[u64]>>,
        machine_value_bindings: impl Into<Box<[CompilerMachineValueBindingV1]>>,
        effective_address: Option<CompilerEffectiveAddressV1>,
    ) -> Result<Self, CompilerInstructionSelectionCorrespondenceErrorV1> {
        let operation = Self {
            coordinate,
            llvm_opcode,
            kind,
            semantic_discriminator,
            result,
            result_type,
            operands: operands.into(),
            phi_inputs: phi_inputs.into(),
            phi_edge_transports: phi_edge_transports.into(),
            successors: successors.into(),
            divergence,
            memory,
            numerical,
            machine_offsets: machine_offsets.into(),
            machine_value_bindings: machine_value_bindings.into(),
            effective_address,
        };
        validate_operation_local(&operation)?;
        Ok(operation)
    }

    /// Returns the exact LLVM coordinate.
    pub const fn coordinate(&self) -> CompilerLlvmOperationCoordinateV1 {
        self.coordinate
    }

    /// Returns LLVM's stable numeric opcode for hostile-substitution detection.
    pub const fn llvm_opcode(&self) -> u16 {
        self.llvm_opcode
    }

    /// Returns the semantic operation class.
    pub const fn kind(&self) -> CompilerLlvmOperationKindV1 {
        self.kind
    }

    /// Returns the compiler's closed semantic subkind number.
    ///
    /// Target adapters define and validate the numbers used for intrinsics, atomics,
    /// collectives, matrix operations, and inline assembly. It is zero for classes with no
    /// semantic subkind.
    pub const fn semantic_discriminator(&self) -> u32 {
        self.semantic_discriminator
    }

    /// Returns the result value number, if any.
    pub const fn result(&self) -> Option<u32> {
        self.result
    }

    /// Returns the exact result type.
    pub const fn result_type(&self) -> CompilerLlvmValueTypeV1 {
        self.result_type
    }

    /// Returns operand value numbers in LLVM operand order.
    pub fn operands(&self) -> &[u32] {
        &self.operands
    }

    /// Returns phi incoming pairs in predecessor order.
    pub fn phi_inputs(&self) -> &[CompilerPhiInputV1] {
        &self.phi_inputs
    }

    /// Returns physical transports in the same canonical order as [`Self::phi_inputs`].
    pub fn phi_edge_transports(&self) -> &[CompilerPhiEdgeTransportV1] {
        &self.phi_edge_transports
    }

    /// Returns terminator successors in LLVM order.
    pub fn successors(&self) -> &[u32] {
        &self.successors
    }

    /// Returns the branch divergence class.
    pub const fn divergence(&self) -> CompilerBranchDivergenceV1 {
        self.divergence
    }

    /// Returns the typed memory effect.
    pub const fn memory(&self) -> CompilerMemoryEffectV1 {
        self.memory
    }

    /// Returns the numerical contract.
    pub const fn numerical(&self) -> CompilerNumericalContractV1 {
        self.numerical
    }

    /// Returns exact final-HSACO instruction offsets attributed by instruction selection.
    pub fn machine_offsets(&self) -> &[u64] {
        &self.machine_offsets
    }

    /// Returns the complete LLVM-value-to-machine-location roster.
    pub fn machine_value_bindings(&self) -> &[CompilerMachineValueBindingV1] {
        &self.machine_value_bindings
    }

    /// Returns the normalized LLVM effective address for an addressed operation.
    pub const fn effective_address(&self) -> Option<&CompilerEffectiveAddressV1> {
        self.effective_address.as_ref()
    }
}

/// Public transcript parts for producer integration and hostile tests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerInstructionSelectionCorrespondencePartsV1 {
    /// Target architecture.
    pub architecture: MachineRefinementArchitectureV1,
    /// Exact compiler occurrence identity.
    pub compiler_occurrence_identity: [u8; 32],
    /// Exact measured worker request identity.
    pub worker_request_identity: [u8; 32],
    /// Exact measured worker response identity excluding this embedded field.
    pub worker_response_identity: [u8; 32],
    /// Exact final post-optimization bitcode identity.
    pub post_optimization_bitcode: ExactCompilerStageContentIdentityV1,
    /// Exact generated relocatable object identity.
    pub generated_object: ExactCompilerStageContentIdentityV1,
    /// Exact final code-object identity.
    pub final_code_object: ExactCompilerStageContentIdentityV1,
    /// Complete final-LLVM block roster.
    pub blocks: Box<[CompilerLlvmBlockV1]>,
    /// Complete final-LLVM operation roster.
    pub operations: Box<[CompilerLlvmOperationV1]>,
}

/// Canonical transcript identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerInstructionSelectionCorrespondenceIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl CompilerInstructionSelectionCorrespondenceIdentityV1 {
    /// Returns the domain-separated SHA-256 digest.
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    /// Returns canonical byte length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Reconstructs a nonzero correspondence identity decoded by another canonical schema.
///
/// This does not authenticate instruction selection; consumers must match it to the checked
/// correspondence owner.
pub fn decode_compiler_instruction_selection_correspondence_identity_v1(
    sha256: [u8; 32],
    byte_len: u64,
) -> Option<CompilerInstructionSelectionCorrespondenceIdentityV1> {
    (sha256 != [0; 32] && byte_len != 0)
        .then_some(CompilerInstructionSelectionCorrespondenceIdentityV1 { sha256, byte_len })
}

/// Canonical compiler-emitted operation-to-machine correspondence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerInstructionSelectionCorrespondenceV1 {
    parts: CompilerInstructionSelectionCorrespondencePartsV1,
    canonical: Box<[u8]>,
    identity: CompilerInstructionSelectionCorrespondenceIdentityV1,
}

impl CompilerInstructionSelectionCorrespondenceV1 {
    /// Constructs a canonical transcript from typed producer parts.
    pub fn from_parts(
        parts: CompilerInstructionSelectionCorrespondencePartsV1,
    ) -> Result<Self, CompilerInstructionSelectionCorrespondenceErrorV1> {
        validate_parts(&parts)?;
        let canonical = encode_parts(&parts)?;
        let identity = correspondence_identity(&canonical);
        Ok(Self {
            parts,
            canonical: canonical.into_boxed_slice(),
            identity,
        })
    }

    /// Independently decodes canonical producer bytes and rejects alternate encodings.
    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, CompilerInstructionSelectionCorrespondenceErrorV1> {
        if bytes.len() > MAX_COMPILER_INSTRUCTION_SELECTION_CORRESPONDENCE_BYTES_V1 {
            return Err(CompilerInstructionSelectionCorrespondenceErrorV1::ResourceLimit);
        }
        let mut input = Input::new(bytes);
        if input.take(8)? != COMPILER_INSTRUCTION_SELECTION_CORRESPONDENCE_MAGIC_V1 {
            return Err(CompilerInstructionSelectionCorrespondenceErrorV1::InvalidMagic);
        }
        if input.u16()? != COMPILER_INSTRUCTION_SELECTION_CORRESPONDENCE_VERSION_V1 {
            return Err(CompilerInstructionSelectionCorrespondenceErrorV1::UnsupportedVersion);
        }
        let architecture = match input.u8()? {
            1 => MachineRefinementArchitectureV1::AmdGcn,
            _ => return Err(CompilerInstructionSelectionCorrespondenceErrorV1::UnknownTag),
        };
        let compiler_occurrence_identity = input.array32()?;
        let worker_request_identity = input.array32()?;
        let worker_response_identity = input.array32()?;
        let post_optimization_bitcode = input.content_identity()?;
        let generated_object = input.content_identity()?;
        let final_code_object = input.content_identity()?;
        let block_count = input.count(MAX_COMPILER_INSTRUCTION_SELECTION_BLOCKS_V1)?;
        let mut blocks = Vec::new();
        blocks
            .try_reserve_exact(block_count)
            .map_err(|_| CompilerInstructionSelectionCorrespondenceErrorV1::AllocationFailure)?;
        for _ in 0..block_count {
            let function = input.u32()?;
            let block = input.u32()?;
            let count = input.count(MAX_COMPILER_INSTRUCTION_SELECTION_SUCCESSORS_V1)?;
            let mut successors = Vec::new();
            successors.try_reserve_exact(count).map_err(|_| {
                CompilerInstructionSelectionCorrespondenceErrorV1::AllocationFailure
            })?;
            for _ in 0..count {
                successors.push(input.u32()?);
            }
            blocks.push(CompilerLlvmBlockV1::new(function, block, successors)?);
        }
        let operation_count = input.count(MAX_COMPILER_INSTRUCTION_SELECTION_OPERATIONS_V1)?;
        let mut operations = Vec::new();
        operations
            .try_reserve_exact(operation_count)
            .map_err(|_| CompilerInstructionSelectionCorrespondenceErrorV1::AllocationFailure)?;
        for _ in 0..operation_count {
            operations.push(decode_operation(&mut input)?);
        }
        input.finish()?;
        let decoded = Self::from_parts(CompilerInstructionSelectionCorrespondencePartsV1 {
            architecture,
            compiler_occurrence_identity,
            worker_request_identity,
            worker_response_identity,
            post_optimization_bitcode,
            generated_object,
            final_code_object,
            blocks: blocks.into_boxed_slice(),
            operations: operations.into_boxed_slice(),
        })?;
        if decoded.canonical.as_ref() != bytes {
            return Err(CompilerInstructionSelectionCorrespondenceErrorV1::NonCanonical);
        }
        Ok(decoded)
    }

    /// Returns the target architecture.
    pub const fn architecture(&self) -> MachineRefinementArchitectureV1 {
        self.parts.architecture
    }

    /// Returns the compiler occurrence identity.
    pub const fn compiler_occurrence_identity(&self) -> [u8; 32] {
        self.parts.compiler_occurrence_identity
    }

    /// Returns the worker request identity.
    pub const fn worker_request_identity(&self) -> [u8; 32] {
        self.parts.worker_request_identity
    }

    /// Returns the worker response identity.
    pub const fn worker_response_identity(&self) -> [u8; 32] {
        self.parts.worker_response_identity
    }

    /// Returns the exact post-optimization bitcode identity.
    pub const fn post_optimization_bitcode(&self) -> ExactCompilerStageContentIdentityV1 {
        self.parts.post_optimization_bitcode
    }

    /// Returns the exact object identity.
    pub const fn generated_object(&self) -> ExactCompilerStageContentIdentityV1 {
        self.parts.generated_object
    }

    /// Returns the exact final code-object identity.
    pub const fn final_code_object(&self) -> ExactCompilerStageContentIdentityV1 {
        self.parts.final_code_object
    }

    /// Returns the complete final-LLVM block roster.
    pub fn blocks(&self) -> &[CompilerLlvmBlockV1] {
        &self.parts.blocks
    }

    /// Returns the complete final-LLVM operation roster.
    pub fn operations(&self) -> &[CompilerLlvmOperationV1] {
        &self.parts.operations
    }

    /// Returns canonical bytes suitable for embedding in a worker response.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }

    /// Returns the canonical transcript identity.
    pub const fn identity(&self) -> CompilerInstructionSelectionCorrespondenceIdentityV1 {
        self.identity
    }

    /// Decoding validates complete typed custody, not target instruction semantics.
    pub const fn proves_instruction_selection_semantics(&self) -> bool {
        false
    }

    /// This inert producer record grants no authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }

    /// Decomposes the transcript for hostile one-axis substitution tests.
    pub fn into_parts(self) -> CompilerInstructionSelectionCorrespondencePartsV1 {
        self.parts
    }
}

/// Checked correspondence bound to exact post-LLVM stage custody.
#[must_use = "dropping checked compiler correspondence abandons a machine-refinement premise"]
pub struct CheckedCompilerInstructionSelectionCorrespondenceV1 {
    correspondence: CompilerInstructionSelectionCorrespondenceV1,
}

impl fmt::Debug for CheckedCompilerInstructionSelectionCorrespondenceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CheckedCompilerInstructionSelectionCorrespondenceV1")
            .field("identity", &self.correspondence.identity())
            .field("operations", &self.correspondence.operations().len())
            .finish_non_exhaustive()
    }
}

impl CheckedCompilerInstructionSelectionCorrespondenceV1 {
    /// Returns the checked correspondence.
    pub const fn correspondence(&self) -> &CompilerInstructionSelectionCorrespondenceV1 {
        &self.correspondence
    }

    /// Reports exact post-LLVM/object/HSACO content binding and complete LLVM record structure.
    pub const fn retains_required_machine_refinement_input(&self) -> bool {
        true
    }

    /// Target-independent custody does not validate target ISA semantics.
    pub const fn proves_target_instruction_semantics(&self) -> bool {
        false
    }
}

/// Decodes compiler-emitted bytes and binds all artifact identities to checked stage custody.
pub fn check_compiler_emitted_instruction_selection_correspondence_v1(
    stages: &CheckedPostLlvmStageContentsV1,
    canonical_correspondence: &[u8],
    expected_compiler_occurrence_identity: [u8; 32],
    expected_worker_request_identity: [u8; 32],
    expected_worker_response_identity: [u8; 32],
) -> Result<
    CheckedCompilerInstructionSelectionCorrespondenceV1,
    CompilerInstructionSelectionCorrespondenceErrorV1,
> {
    let correspondence =
        CompilerInstructionSelectionCorrespondenceV1::decode_canonical(canonical_correspondence)?;
    if correspondence.compiler_occurrence_identity() != expected_compiler_occurrence_identity
        || correspondence.worker_request_identity() != expected_worker_request_identity
        || correspondence.worker_response_identity() != expected_worker_response_identity
    {
        return Err(CompilerInstructionSelectionCorrespondenceErrorV1::OccurrenceMismatch);
    }
    if !correspondence
        .post_optimization_bitcode()
        .matches(stages.post_optimization_bitcode())
    {
        return Err(CompilerInstructionSelectionCorrespondenceErrorV1::PostOptimizationMismatch);
    }
    if !correspondence
        .generated_object()
        .matches(stages.generated_object())
    {
        return Err(CompilerInstructionSelectionCorrespondenceErrorV1::GeneratedObjectMismatch);
    }
    if !correspondence
        .final_code_object()
        .matches(stages.final_code_object())
    {
        return Err(CompilerInstructionSelectionCorrespondenceErrorV1::FinalCodeObjectMismatch);
    }
    Ok(CheckedCompilerInstructionSelectionCorrespondenceV1 { correspondence })
}

/// Closed failures for canonical compiler instruction-selection correspondence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerInstructionSelectionCorrespondenceErrorV1 {
    /// Input ended before a complete record.
    Truncated,
    /// Magic did not match.
    InvalidMagic,
    /// Version is unsupported.
    UnsupportedVersion,
    /// A closed enum tag was unknown.
    UnknownTag,
    /// A collection or byte body exceeded its bound.
    ResourceLimit,
    /// A bounded allocation failed.
    AllocationFailure,
    /// Canonical ordering or unique-coordinate rules failed.
    NonCanonical,
    /// A value type was invalid.
    InvalidValueType,
    /// A memory effect was inconsistent.
    InvalidMemoryEffect,
    /// An operation's local semantic shape was inconsistent.
    InvalidOperation,
    /// CFG successors, terminators, phis, or reachability were inconsistent.
    InvalidControlFlow,
    /// A machine offset was missing, duplicated, or misordered.
    InvalidMachineMapping,
    /// An LLVM SSA value was not bound canonically to its machine definition or use.
    InvalidValueBinding,
    /// A normalized effective-address expression was malformed or incomplete.
    InvalidAddress,
    /// Compiler/worker occurrence identities differ.
    OccurrenceMismatch,
    /// Post-optimization bitcode differs.
    PostOptimizationMismatch,
    /// Generated object differs.
    GeneratedObjectMismatch,
    /// Final code object differs.
    FinalCodeObjectMismatch,
}

impl fmt::Display for CompilerInstructionSelectionCorrespondenceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid compiler instruction-selection correspondence: {self:?}"
        )
    }
}

impl std::error::Error for CompilerInstructionSelectionCorrespondenceErrorV1 {}

fn validate_parts(
    parts: &CompilerInstructionSelectionCorrespondencePartsV1,
) -> Result<(), CompilerInstructionSelectionCorrespondenceErrorV1> {
    if parts.compiler_occurrence_identity == [0; 32]
        || parts.worker_request_identity == [0; 32]
        || parts.worker_response_identity == [0; 32]
        || parts.blocks.is_empty()
        || parts.blocks.len() > MAX_COMPILER_INSTRUCTION_SELECTION_BLOCKS_V1
        || parts.operations.is_empty()
        || parts.operations.len() > MAX_COMPILER_INSTRUCTION_SELECTION_OPERATIONS_V1
    {
        return Err(CompilerInstructionSelectionCorrespondenceErrorV1::ResourceLimit);
    }
    if !parts
        .blocks
        .windows(2)
        .all(|pair| (pair[0].function, pair[0].block) < (pair[1].function, pair[1].block))
        || !parts
            .operations
            .windows(2)
            .all(|pair| pair[0].coordinate < pair[1].coordinate)
    {
        return Err(CompilerInstructionSelectionCorrespondenceErrorV1::NonCanonical);
    }
    let mut functions = BTreeSet::new();
    let mut blocks = BTreeMap::<(u32, u32), &CompilerLlvmBlockV1>::new();
    for block in &parts.blocks {
        functions.insert(block.function);
        if blocks
            .insert((block.function, block.block), block)
            .is_some()
        {
            return Err(CompilerInstructionSelectionCorrespondenceErrorV1::NonCanonical);
        }
    }
    if functions.len() > MAX_COMPILER_INSTRUCTION_SELECTION_FUNCTIONS_V1
        || functions.iter().copied().ne(0..functions.len() as u32)
    {
        return Err(CompilerInstructionSelectionCorrespondenceErrorV1::NonCanonical);
    }
    for function in &functions {
        let function_blocks = parts
            .blocks
            .iter()
            .filter(|block| block.function == *function)
            .map(|block| block.block);
        if function_blocks.ne(0..parts
            .blocks
            .iter()
            .filter(|block| block.function == *function)
            .count() as u32)
        {
            return Err(CompilerInstructionSelectionCorrespondenceErrorV1::NonCanonical);
        }
    }
    let mut predecessors = BTreeMap::<(u32, u32), BTreeSet<u32>>::new();
    for block in &parts.blocks {
        for successor in &block.successors {
            if !blocks.contains_key(&(block.function, *successor)) {
                return Err(CompilerInstructionSelectionCorrespondenceErrorV1::InvalidControlFlow);
            }
            predecessors
                .entry((block.function, *successor))
                .or_default()
                .insert(block.block);
        }
    }
    let mut operation_counts = BTreeMap::<(u32, u32), u32>::new();
    let mut all_machine_offsets = BTreeSet::new();
    for operation in &parts.operations {
        validate_operation_local(operation)?;
        let coordinate = operation.coordinate;
        let block = blocks
            .get(&(coordinate.function, coordinate.block))
            .ok_or(CompilerInstructionSelectionCorrespondenceErrorV1::InvalidControlFlow)?;
        let expected = operation_counts
            .entry((coordinate.function, coordinate.block))
            .or_insert(0);
        if coordinate.operation != *expected {
            return Err(CompilerInstructionSelectionCorrespondenceErrorV1::NonCanonical);
        }
        *expected += 1;
        if operation.kind == CompilerLlvmOperationKindV1::Phi {
            let observed = operation
                .phi_inputs
                .iter()
                .map(|input| input.predecessor)
                .collect::<BTreeSet<_>>();
            if observed
                != predecessors
                    .get(&(coordinate.function, coordinate.block))
                    .cloned()
                    .unwrap_or_default()
            {
                return Err(CompilerInstructionSelectionCorrespondenceErrorV1::InvalidControlFlow);
            }
        }
        if is_terminator(operation.kind)
            && operation.successors.as_ref() != block.successors.as_ref()
        {
            return Err(CompilerInstructionSelectionCorrespondenceErrorV1::InvalidControlFlow);
        }
        for offset in &operation.machine_offsets {
            if !all_machine_offsets.insert(*offset) {
                return Err(
                    CompilerInstructionSelectionCorrespondenceErrorV1::InvalidMachineMapping,
                );
            }
        }
    }
    for block in &parts.blocks {
        let operations = parts
            .operations
            .iter()
            .filter(|operation| {
                operation.coordinate.function == block.function
                    && operation.coordinate.block == block.block
            })
            .collect::<Vec<_>>();
        if operations.is_empty()
            || !is_terminator(operations.last().expect("nonempty").kind)
            || operations[..operations.len() - 1]
                .iter()
                .any(|operation| is_terminator(operation.kind))
        {
            return Err(CompilerInstructionSelectionCorrespondenceErrorV1::InvalidControlFlow);
        }
    }
    validate_reachability(&parts.blocks, &blocks)?;
    Ok(())
}

fn validate_reachability(
    source: &[CompilerLlvmBlockV1],
    blocks: &BTreeMap<(u32, u32), &CompilerLlvmBlockV1>,
) -> Result<(), CompilerInstructionSelectionCorrespondenceErrorV1> {
    for function in source
        .iter()
        .map(|block| block.function)
        .collect::<BTreeSet<_>>()
    {
        let mut pending = vec![0_u32];
        let mut reached = BTreeSet::new();
        while let Some(block) = pending.pop() {
            if !reached.insert(block) {
                continue;
            }
            let record = blocks
                .get(&(function, block))
                .ok_or(CompilerInstructionSelectionCorrespondenceErrorV1::InvalidControlFlow)?;
            pending.extend(record.successors.iter().copied());
            if reached.len() > source.len() {
                return Err(CompilerInstructionSelectionCorrespondenceErrorV1::ResourceLimit);
            }
        }
        let expected = source
            .iter()
            .filter(|block| block.function == function)
            .count();
        if reached.len() != expected {
            return Err(CompilerInstructionSelectionCorrespondenceErrorV1::InvalidControlFlow);
        }
    }
    Ok(())
}

fn validate_operation_local(
    operation: &CompilerLlvmOperationV1,
) -> Result<(), CompilerInstructionSelectionCorrespondenceErrorV1> {
    if operation.operands.len() > MAX_COMPILER_INSTRUCTION_SELECTION_OPERATION_INPUTS_V1
        || operation.phi_inputs.len() > MAX_COMPILER_INSTRUCTION_SELECTION_OPERATION_INPUTS_V1
        || operation.phi_edge_transports.len()
            > MAX_COMPILER_INSTRUCTION_SELECTION_OPERATION_INPUTS_V1
        || operation.successors.len() > MAX_COMPILER_INSTRUCTION_SELECTION_SUCCESSORS_V1
        || operation.machine_offsets.len() > MAX_COMPILER_INSTRUCTION_SELECTION_MACHINE_OFFSETS_V1
        || operation.machine_value_bindings.len()
            > MAX_COMPILER_INSTRUCTION_SELECTION_VALUE_BINDINGS_V1
        || !operation
            .machine_offsets
            .windows(2)
            .all(|pair| pair[0] < pair[1])
        || !operation
            .machine_value_bindings
            .windows(2)
            .all(|pair| pair[0] < pair[1])
        || has_duplicate(operation.successors.iter().copied())
    {
        return Err(CompilerInstructionSelectionCorrespondenceErrorV1::ResourceLimit);
    }
    validate_value_type(operation.result_type)?;
    validate_memory_effect(operation.memory)?;
    let has_result = operation.result.is_some();
    if has_result == (operation.result_type == CompilerLlvmValueTypeV1::Void) {
        return Err(CompilerInstructionSelectionCorrespondenceErrorV1::InvalidValueType);
    }
    let phi = operation.kind == CompilerLlvmOperationKindV1::Phi;
    if phi != !operation.phi_inputs.is_empty()
        || phi != !operation.phi_edge_transports.is_empty()
        || operation.phi_inputs.len() != operation.phi_edge_transports.len()
        || operation
            .phi_inputs
            .iter()
            .zip(operation.phi_edge_transports.iter())
            .any(|(input, transport)| {
                input.predecessor != transport.predecessor || input.value != transport.value
            })
        || (phi && (!operation.operands.is_empty() || operation.successors.len() > 0))
        || (!is_terminator(operation.kind) && !operation.successors.is_empty())
        || (operation.kind == CompilerLlvmOperationKindV1::ConditionalBranch
            && operation.divergence == CompilerBranchDivergenceV1::None)
        || (operation.kind != CompilerLlvmOperationKindV1::ConditionalBranch
            && operation.divergence != CompilerBranchDivergenceV1::None)
    {
        return Err(CompilerInstructionSelectionCorrespondenceErrorV1::InvalidOperation);
    }
    let memory_valid = match operation.kind {
        CompilerLlvmOperationKindV1::Load => {
            operation.memory.kind == CompilerMemoryEffectKindV1::Read
        }
        CompilerLlvmOperationKindV1::Store => {
            operation.memory.kind == CompilerMemoryEffectKindV1::Write
        }
        CompilerLlvmOperationKindV1::Atomic => {
            matches!(
                operation.memory.kind,
                CompilerMemoryEffectKindV1::Read
                    | CompilerMemoryEffectKindV1::Write
                    | CompilerMemoryEffectKindV1::ReadWrite
            ) && operation.memory.ordering != CompilerMemoryOrderingV1::NotAtomic
        }
        CompilerLlvmOperationKindV1::Fence => {
            operation.memory.kind == CompilerMemoryEffectKindV1::Fence
        }
        CompilerLlvmOperationKindV1::Barrier => {
            operation.memory.kind == CompilerMemoryEffectKindV1::Barrier
        }
        _ => operation.memory.kind == CompilerMemoryEffectKindV1::None,
    };
    if !memory_valid {
        return Err(CompilerInstructionSelectionCorrespondenceErrorV1::InvalidMemoryEffect);
    }
    if let Some(address) = &operation.effective_address {
        if !matches!(
            operation.kind,
            CompilerLlvmOperationKindV1::Address
                | CompilerLlvmOperationKindV1::Load
                | CompilerLlvmOperationKindV1::Store
                | CompilerLlvmOperationKindV1::Atomic
        ) {
            return Err(CompilerInstructionSelectionCorrespondenceErrorV1::InvalidAddress);
        }
        validate_effective_address(address)?;
        if !address
            .terms
            .iter()
            .all(|term| operation.operands.contains(&term.value))
            || (operation.kind != CompilerLlvmOperationKindV1::Address
                && (address.address_space != operation.memory.address_space
                    || address.byte_width != operation.memory.byte_width))
        {
            return Err(CompilerInstructionSelectionCorrespondenceErrorV1::InvalidAddress);
        }
    }
    let numerical_valid = match operation.kind {
        CompilerLlvmOperationKindV1::FloatAdd
        | CompilerLlvmOperationKindV1::FloatSubtract
        | CompilerLlvmOperationKindV1::FloatMultiply
        | CompilerLlvmOperationKindV1::FloatDivide
        | CompilerLlvmOperationKindV1::FloatSqrt
        | CompilerLlvmOperationKindV1::FloatCompare => matches!(
            operation.numerical,
            CompilerNumericalContractV1::IeeeBinary32 | CompilerNumericalContractV1::IeeeBinary64
        ),
        CompilerLlvmOperationKindV1::FloatFma => matches!(
            operation.numerical,
            CompilerNumericalContractV1::IeeeBinary32Fused
                | CompilerNumericalContractV1::TargetDefined
        ),
        CompilerLlvmOperationKindV1::Collective | CompilerLlvmOperationKindV1::Matrix => true,
        _ => operation.numerical == CompilerNumericalContractV1::Exact,
    };
    if !numerical_valid {
        return Err(CompilerInstructionSelectionCorrespondenceErrorV1::InvalidOperation);
    }
    if operation.machine_offsets.is_empty()
        && !matches!(
            operation.kind,
            CompilerLlvmOperationKindV1::Constant
                | CompilerLlvmOperationKindV1::Vector
                | CompilerLlvmOperationKindV1::SymbolAddress
                | CompilerLlvmOperationKindV1::Phi
                | CompilerLlvmOperationKindV1::Cast
        )
    {
        return Err(CompilerInstructionSelectionCorrespondenceErrorV1::InvalidMachineMapping);
    }
    if operation.machine_offsets.is_empty() && !operation.machine_value_bindings.is_empty() {
        return Err(CompilerInstructionSelectionCorrespondenceErrorV1::InvalidValueBinding);
    }
    for binding in &operation.machine_value_bindings {
        validate_machine_value_binding(*binding)?;
        if !operation.machine_offsets.contains(&binding.machine_offset) {
            return Err(CompilerInstructionSelectionCorrespondenceErrorV1::InvalidValueBinding);
        }
        let valid_value = match binding.access {
            CompilerMachineValueAccessV1::Definition => {
                operation.result == Some(binding.value)
                    && matches!(
                        binding.location,
                        CompilerMachineValueLocationV1::Register(_)
                    )
            }
            CompilerMachineValueAccessV1::Use => {
                operation.operands.contains(&binding.value)
                    || operation
                        .phi_inputs
                        .iter()
                        .any(|input| input.value == binding.value)
            }
        };
        if !valid_value {
            return Err(CompilerInstructionSelectionCorrespondenceErrorV1::InvalidValueBinding);
        }
    }
    for transport in &operation.phi_edge_transports {
        validate_phi_edge_transport(*transport)?;
        if transport
            .move_offset
            .is_some_and(|offset| !operation.machine_offsets.contains(&offset))
        {
            return Err(CompilerInstructionSelectionCorrespondenceErrorV1::InvalidValueBinding);
        }
    }
    Ok(())
}

fn validate_phi_edge_transport(
    transport: CompilerPhiEdgeTransportV1,
) -> Result<(), CompilerInstructionSelectionCorrespondenceErrorV1> {
    validate_machine_value_binding(CompilerMachineValueBindingV1 {
        machine_offset: transport.move_offset.unwrap_or(0),
        value: transport.value,
        access: CompilerMachineValueAccessV1::Use,
        location: transport.incoming,
    })?;
    validate_machine_value_binding(CompilerMachineValueBindingV1 {
        machine_offset: transport.move_offset.unwrap_or(0),
        value: transport.value,
        access: CompilerMachineValueAccessV1::Definition,
        location: CompilerMachineValueLocationV1::Register(transport.result),
    })?;
    if transport.move_offset.is_none()
        && transport.incoming != CompilerMachineValueLocationV1::Register(transport.result)
    {
        return Err(CompilerInstructionSelectionCorrespondenceErrorV1::InvalidValueBinding);
    }
    Ok(())
}

fn validate_machine_value_binding(
    binding: CompilerMachineValueBindingV1,
) -> Result<(), CompilerInstructionSelectionCorrespondenceErrorV1> {
    match binding.location {
        CompilerMachineValueLocationV1::Register(register) => {
            if !matches!(
                register.class,
                CompilerMachineRegisterClassV1::Scalar
                    | CompilerMachineRegisterClassV1::Vector
                    | CompilerMachineRegisterClassV1::Accumulator
            ) && register.index != 0
            {
                return Err(CompilerInstructionSelectionCorrespondenceErrorV1::InvalidValueBinding);
            }
        }
        CompilerMachineValueLocationV1::Immediate { bit_width, bits } => {
            if binding.access != CompilerMachineValueAccessV1::Use
                || bit_width == 0
                || bit_width > 128
            {
                return Err(CompilerInstructionSelectionCorrespondenceErrorV1::InvalidValueBinding);
            }
            let full_bytes = usize::from(bit_width.div_ceil(8));
            if bits[full_bytes..].iter().any(|byte| *byte != 0)
                || (bit_width % 8 != 0
                    && bits[full_bytes - 1] & !((1_u8 << (bit_width % 8)) - 1) != 0)
            {
                return Err(CompilerInstructionSelectionCorrespondenceErrorV1::InvalidValueBinding);
            }
        }
    }
    Ok(())
}

fn validate_effective_address(
    address: &CompilerEffectiveAddressV1,
) -> Result<(), CompilerInstructionSelectionCorrespondenceErrorV1> {
    if !matches!(address.pointer_bits, 32 | 64)
        || address.byte_width == 0
        || address.terms.is_empty()
        || address.terms.len() > MAX_COMPILER_INSTRUCTION_SELECTION_ADDRESS_TERMS_V1
        || !address.terms.windows(2).all(|pair| pair[0] < pair[1])
        || address.terms.iter().any(|term| term.scale == 0)
    {
        return Err(CompilerInstructionSelectionCorrespondenceErrorV1::InvalidAddress);
    }
    Ok(())
}

fn validate_value_type(
    value_type: CompilerLlvmValueTypeV1,
) -> Result<(), CompilerInstructionSelectionCorrespondenceErrorV1> {
    let valid = match value_type {
        CompilerLlvmValueTypeV1::Void => true,
        CompilerLlvmValueTypeV1::Integer(bits) => matches!(bits, 1 | 8 | 16 | 32 | 64 | 128),
        CompilerLlvmValueTypeV1::Float(bits) => matches!(bits, 16 | 32 | 64),
        CompilerLlvmValueTypeV1::Pointer { bits, .. } => matches!(bits, 32 | 64),
        CompilerLlvmValueTypeV1::Vector {
            element_bits,
            lanes,
            ..
        } => matches!(element_bits, 1 | 8 | 16 | 32 | 64) && (1..=256).contains(&lanes),
    };
    valid
        .then_some(())
        .ok_or(CompilerInstructionSelectionCorrespondenceErrorV1::InvalidValueType)
}

fn validate_memory_effect(
    effect: CompilerMemoryEffectV1,
) -> Result<(), CompilerInstructionSelectionCorrespondenceErrorV1> {
    let no_effect = effect.kind == CompilerMemoryEffectKindV1::None;
    let fence = effect.kind == CompilerMemoryEffectKindV1::Fence;
    let barrier = effect.kind == CompilerMemoryEffectKindV1::Barrier;
    let addressed = matches!(
        effect.kind,
        CompilerMemoryEffectKindV1::Read
            | CompilerMemoryEffectKindV1::Write
            | CompilerMemoryEffectKindV1::ReadWrite
    );
    let alignment_valid = effect.alignment == 0
        || (effect.alignment.is_power_of_two() && effect.alignment <= (1 << 20));
    let valid = alignment_valid
        && if no_effect {
            effect.address_space == 0
                && effect.byte_width == 0
                && effect.alignment == 0
                && effect.ordering == CompilerMemoryOrderingV1::NotAtomic
                && effect.scope == CompilerMemoryScopeV1::None
                && !effect.volatile
        } else if addressed {
            effect.byte_width != 0
                && effect.alignment != 0
                && ((effect.ordering == CompilerMemoryOrderingV1::NotAtomic
                    && effect.scope == CompilerMemoryScopeV1::None)
                    || (effect.ordering != CompilerMemoryOrderingV1::NotAtomic
                        && effect.scope != CompilerMemoryScopeV1::None))
        } else if fence || barrier {
            effect.byte_width == 0
                && effect.alignment == 0
                && effect.ordering != CompilerMemoryOrderingV1::NotAtomic
                && effect.scope != CompilerMemoryScopeV1::None
                && !effect.volatile
        } else {
            false
        };
    valid
        .then_some(())
        .ok_or(CompilerInstructionSelectionCorrespondenceErrorV1::InvalidMemoryEffect)
}

fn is_terminator(kind: CompilerLlvmOperationKindV1) -> bool {
    matches!(
        kind,
        CompilerLlvmOperationKindV1::Branch
            | CompilerLlvmOperationKindV1::ConditionalBranch
            | CompilerLlvmOperationKindV1::Switch
            | CompilerLlvmOperationKindV1::Return
            | CompilerLlvmOperationKindV1::Unreachable
    )
}

fn has_duplicate<T: Ord>(values: impl Iterator<Item = T>) -> bool {
    let mut seen = BTreeSet::new();
    values.into_iter().any(|value| !seen.insert(value))
}

fn correspondence_identity(bytes: &[u8]) -> CompilerInstructionSelectionCorrespondenceIdentityV1 {
    let mut digest = Sha256::new();
    digest.update((IDENTITY_DOMAIN_V1.len() as u32).to_le_bytes());
    digest.update(IDENTITY_DOMAIN_V1);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    CompilerInstructionSelectionCorrespondenceIdentityV1 {
        sha256: digest.finalize().into(),
        byte_len: bytes.len() as u64,
    }
}

fn encode_parts(
    parts: &CompilerInstructionSelectionCorrespondencePartsV1,
) -> Result<Vec<u8>, CompilerInstructionSelectionCorrespondenceErrorV1> {
    let mut output = Vec::new();
    output
        .try_reserve(256)
        .map_err(|_| CompilerInstructionSelectionCorrespondenceErrorV1::AllocationFailure)?;
    output.extend_from_slice(&COMPILER_INSTRUCTION_SELECTION_CORRESPONDENCE_MAGIC_V1);
    put_u16(
        &mut output,
        COMPILER_INSTRUCTION_SELECTION_CORRESPONDENCE_VERSION_V1,
    );
    output.push(match parts.architecture {
        MachineRefinementArchitectureV1::AmdGcn => 1,
    });
    output.extend_from_slice(&parts.compiler_occurrence_identity);
    output.extend_from_slice(&parts.worker_request_identity);
    output.extend_from_slice(&parts.worker_response_identity);
    for identity in [
        parts.post_optimization_bitcode,
        parts.generated_object,
        parts.final_code_object,
    ] {
        output.extend_from_slice(&identity.sha256());
        put_u64(&mut output, identity.byte_len());
    }
    put_u32(&mut output, parts.blocks.len() as u32);
    for block in &parts.blocks {
        put_u32(&mut output, block.function);
        put_u32(&mut output, block.block);
        put_u32(&mut output, block.successors.len() as u32);
        for successor in &block.successors {
            put_u32(&mut output, *successor);
        }
    }
    put_u32(&mut output, parts.operations.len() as u32);
    for operation in &parts.operations {
        encode_operation(&mut output, operation);
    }
    if output.len() > MAX_COMPILER_INSTRUCTION_SELECTION_CORRESPONDENCE_BYTES_V1 {
        return Err(CompilerInstructionSelectionCorrespondenceErrorV1::ResourceLimit);
    }
    Ok(output)
}

fn encode_operation(output: &mut Vec<u8>, operation: &CompilerLlvmOperationV1) {
    put_u32(output, operation.coordinate.function);
    put_u32(output, operation.coordinate.block);
    put_u32(output, operation.coordinate.operation);
    put_u16(output, operation.llvm_opcode);
    output.push(operation_kind_tag(operation.kind));
    put_u32(output, operation.semantic_discriminator);
    match operation.result {
        Some(result) => {
            output.push(1);
            put_u32(output, result);
        }
        None => output.push(0),
    }
    encode_value_type(output, operation.result_type);
    put_u32(output, operation.operands.len() as u32);
    for operand in &operation.operands {
        put_u32(output, *operand);
    }
    put_u32(output, operation.phi_inputs.len() as u32);
    for input in &operation.phi_inputs {
        put_u32(output, input.predecessor);
        put_u32(output, input.value);
    }
    put_u32(output, operation.phi_edge_transports.len() as u32);
    for transport in &operation.phi_edge_transports {
        put_u32(output, transport.predecessor);
        put_u32(output, transport.value);
        encode_machine_value_location(output, transport.incoming);
        output.push(machine_register_class_tag(transport.result.class));
        put_u16(output, transport.result.index);
        match transport.move_offset {
            Some(offset) => {
                output.push(1);
                put_u64(output, offset);
            }
            None => output.push(0),
        }
    }
    put_u32(output, operation.successors.len() as u32);
    for successor in &operation.successors {
        put_u32(output, *successor);
    }
    output.push(divergence_tag(operation.divergence));
    encode_memory(output, operation.memory);
    output.push(numerical_tag(operation.numerical));
    put_u32(output, operation.machine_offsets.len() as u32);
    for offset in &operation.machine_offsets {
        put_u64(output, *offset);
    }
    put_u32(output, operation.machine_value_bindings.len() as u32);
    for binding in &operation.machine_value_bindings {
        put_u64(output, binding.machine_offset);
        put_u32(output, binding.value);
        output.push(match binding.access {
            CompilerMachineValueAccessV1::Definition => 1,
            CompilerMachineValueAccessV1::Use => 2,
        });
        encode_machine_value_location(output, binding.location);
    }
    match &operation.effective_address {
        Some(address) => {
            output.push(1);
            put_u32(output, address.address_space);
            put_u16(output, address.pointer_bits);
            put_u32(output, address.byte_width);
            put_i64(output, address.displacement);
            put_u32(output, address.terms.len() as u32);
            for term in &address.terms {
                put_u32(output, term.value);
                put_i64(output, term.scale);
            }
        }
        None => output.push(0),
    }
}

fn encode_machine_value_location(output: &mut Vec<u8>, location: CompilerMachineValueLocationV1) {
    match location {
        CompilerMachineValueLocationV1::Register(register) => {
            output.push(1);
            output.push(machine_register_class_tag(register.class));
            put_u16(output, register.index);
        }
        CompilerMachineValueLocationV1::Immediate { bit_width, bits } => {
            output.push(2);
            put_u16(output, bit_width);
            output.extend_from_slice(&bits);
        }
    }
}

fn decode_operation(
    input: &mut Input<'_>,
) -> Result<CompilerLlvmOperationV1, CompilerInstructionSelectionCorrespondenceErrorV1> {
    let coordinate =
        CompilerLlvmOperationCoordinateV1::new(input.u32()?, input.u32()?, input.u32()?);
    let llvm_opcode = input.u16()?;
    let kind = decode_operation_kind(input.u8()?)?;
    let semantic_discriminator = input.u32()?;
    let result = match input.u8()? {
        0 => None,
        1 => Some(input.u32()?),
        _ => return Err(CompilerInstructionSelectionCorrespondenceErrorV1::UnknownTag),
    };
    let result_type = decode_value_type(input)?;
    let operand_count = input.count(MAX_COMPILER_INSTRUCTION_SELECTION_OPERATION_INPUTS_V1)?;
    let mut operands = Vec::with_capacity(operand_count);
    for _ in 0..operand_count {
        operands.push(input.u32()?);
    }
    let phi_count = input.count(MAX_COMPILER_INSTRUCTION_SELECTION_OPERATION_INPUTS_V1)?;
    let mut phi_inputs = Vec::with_capacity(phi_count);
    for _ in 0..phi_count {
        phi_inputs.push(CompilerPhiInputV1::new(input.u32()?, input.u32()?));
    }
    let transport_count = input.count(MAX_COMPILER_INSTRUCTION_SELECTION_OPERATION_INPUTS_V1)?;
    let mut phi_edge_transports = Vec::with_capacity(transport_count);
    for _ in 0..transport_count {
        let predecessor = input.u32()?;
        let value = input.u32()?;
        let incoming = decode_machine_value_location(input)?;
        let result = CompilerMachineRegisterV1::new(
            decode_machine_register_class(input.u8()?)?,
            input.u16()?,
        );
        let move_offset = match input.u8()? {
            0 => None,
            1 => Some(input.u64()?),
            _ => return Err(CompilerInstructionSelectionCorrespondenceErrorV1::UnknownTag),
        };
        phi_edge_transports.push(CompilerPhiEdgeTransportV1::new(
            predecessor,
            value,
            incoming,
            result,
            move_offset,
        )?);
    }
    let successor_count = input.count(MAX_COMPILER_INSTRUCTION_SELECTION_SUCCESSORS_V1)?;
    let mut successors = Vec::with_capacity(successor_count);
    for _ in 0..successor_count {
        successors.push(input.u32()?);
    }
    let divergence = match input.u8()? {
        0 => CompilerBranchDivergenceV1::None,
        1 => CompilerBranchDivergenceV1::Uniform,
        2 => CompilerBranchDivergenceV1::Divergent,
        _ => return Err(CompilerInstructionSelectionCorrespondenceErrorV1::UnknownTag),
    };
    let memory = decode_memory(input)?;
    let numerical = match input.u8()? {
        0 => CompilerNumericalContractV1::Exact,
        1 => CompilerNumericalContractV1::IeeeBinary32,
        2 => CompilerNumericalContractV1::IeeeBinary64,
        3 => CompilerNumericalContractV1::IeeeBinary32Fused,
        4 => CompilerNumericalContractV1::TargetDefined,
        5 => CompilerNumericalContractV1::MfmaBf16AccumulateF32,
        6 => CompilerNumericalContractV1::MfmaFp8E4M3AccumulateF32,
        7 => CompilerNumericalContractV1::MfmaFp8E5M2AccumulateF32,
        8 => CompilerNumericalContractV1::MfmaFp4E2M1AccumulateF32,
        9 => CompilerNumericalContractV1::MfmaFp4E2M1ByFp8E4M3AccumulateF32,
        _ => return Err(CompilerInstructionSelectionCorrespondenceErrorV1::UnknownTag),
    };
    let offset_count = input.count(MAX_COMPILER_INSTRUCTION_SELECTION_MACHINE_OFFSETS_V1)?;
    let mut machine_offsets = Vec::with_capacity(offset_count);
    for _ in 0..offset_count {
        machine_offsets.push(input.u64()?);
    }
    let binding_count = input.count(MAX_COMPILER_INSTRUCTION_SELECTION_VALUE_BINDINGS_V1)?;
    let mut machine_value_bindings = Vec::with_capacity(binding_count);
    for _ in 0..binding_count {
        let machine_offset = input.u64()?;
        let value = input.u32()?;
        let access = match input.u8()? {
            1 => CompilerMachineValueAccessV1::Definition,
            2 => CompilerMachineValueAccessV1::Use,
            _ => return Err(CompilerInstructionSelectionCorrespondenceErrorV1::UnknownTag),
        };
        let location = decode_machine_value_location(input)?;
        machine_value_bindings.push(CompilerMachineValueBindingV1::new(
            machine_offset,
            value,
            access,
            location,
        )?);
    }
    let effective_address = match input.u8()? {
        0 => None,
        1 => {
            let address_space = input.u32()?;
            let pointer_bits = input.u16()?;
            let byte_width = input.u32()?;
            let displacement = input.i64()?;
            let term_count = input.count(MAX_COMPILER_INSTRUCTION_SELECTION_ADDRESS_TERMS_V1)?;
            let mut terms = Vec::with_capacity(term_count);
            for _ in 0..term_count {
                terms.push(CompilerAddressTermV1::new(input.u32()?, input.i64()?)?);
            }
            Some(CompilerEffectiveAddressV1::new(
                address_space,
                pointer_bits,
                byte_width,
                displacement,
                terms,
            )?)
        }
        _ => return Err(CompilerInstructionSelectionCorrespondenceErrorV1::UnknownTag),
    };
    CompilerLlvmOperationV1::new(
        coordinate,
        llvm_opcode,
        kind,
        semantic_discriminator,
        result,
        result_type,
        operands,
        phi_inputs,
        phi_edge_transports,
        successors,
        divergence,
        memory,
        numerical,
        machine_offsets,
        machine_value_bindings,
        effective_address,
    )
}

fn decode_machine_value_location(
    input: &mut Input<'_>,
) -> Result<CompilerMachineValueLocationV1, CompilerInstructionSelectionCorrespondenceErrorV1> {
    match input.u8()? {
        1 => Ok(CompilerMachineValueLocationV1::Register(
            CompilerMachineRegisterV1::new(
                decode_machine_register_class(input.u8()?)?,
                input.u16()?,
            ),
        )),
        2 => Ok(CompilerMachineValueLocationV1::Immediate {
            bit_width: input.u16()?,
            bits: input.array16()?,
        }),
        _ => Err(CompilerInstructionSelectionCorrespondenceErrorV1::UnknownTag),
    }
}

fn machine_register_class_tag(class: CompilerMachineRegisterClassV1) -> u8 {
    match class {
        CompilerMachineRegisterClassV1::Scalar => 1,
        CompilerMachineRegisterClassV1::Vector => 2,
        CompilerMachineRegisterClassV1::Accumulator => 3,
        CompilerMachineRegisterClassV1::ExecutionMask => 4,
        CompilerMachineRegisterClassV1::VectorCondition => 5,
        CompilerMachineRegisterClassV1::ScalarCondition => 6,
        CompilerMachineRegisterClassV1::MemoryOffset => 7,
        CompilerMachineRegisterClassV1::FloatingMode => 8,
    }
}

fn decode_machine_register_class(
    tag: u8,
) -> Result<CompilerMachineRegisterClassV1, CompilerInstructionSelectionCorrespondenceErrorV1> {
    Ok(match tag {
        1 => CompilerMachineRegisterClassV1::Scalar,
        2 => CompilerMachineRegisterClassV1::Vector,
        3 => CompilerMachineRegisterClassV1::Accumulator,
        4 => CompilerMachineRegisterClassV1::ExecutionMask,
        5 => CompilerMachineRegisterClassV1::VectorCondition,
        6 => CompilerMachineRegisterClassV1::ScalarCondition,
        7 => CompilerMachineRegisterClassV1::MemoryOffset,
        8 => CompilerMachineRegisterClassV1::FloatingMode,
        _ => return Err(CompilerInstructionSelectionCorrespondenceErrorV1::UnknownTag),
    })
}

fn operation_kind_tag(kind: CompilerLlvmOperationKindV1) -> u8 {
    use CompilerLlvmOperationKindV1 as Kind;
    match kind {
        Kind::Constant => 1,
        Kind::Vector => 2,
        Kind::SymbolAddress => 3,
        Kind::IntegerAdd => 4,
        Kind::IntegerSubtract => 5,
        Kind::IntegerMultiply => 6,
        Kind::IntegerDivide => 7,
        Kind::IntegerRemainder => 8,
        Kind::IntegerBitwise => 9,
        Kind::IntegerShift => 10,
        Kind::FloatAdd => 11,
        Kind::FloatSubtract => 12,
        Kind::FloatMultiply => 13,
        Kind::FloatDivide => 14,
        Kind::FloatFma => 15,
        Kind::FloatSqrt => 16,
        Kind::IntegerCompare => 17,
        Kind::FloatCompare => 18,
        Kind::Cast => 19,
        Kind::Select => 20,
        Kind::Allocate => 21,
        Kind::Address => 22,
        Kind::Load => 23,
        Kind::Store => 24,
        Kind::Atomic => 25,
        Kind::Fence => 26,
        Kind::Barrier => 27,
        Kind::Call => 28,
        Kind::Intrinsic => 29,
        Kind::Phi => 30,
        Kind::Branch => 31,
        Kind::ConditionalBranch => 32,
        Kind::Switch => 33,
        Kind::Return => 34,
        Kind::Unreachable => 35,
        Kind::InvocationIndex => 36,
        Kind::Collective => 37,
        Kind::Matrix => 38,
        Kind::InlineAssembly => 39,
        Kind::Trap => 40,
    }
}

fn decode_operation_kind(
    tag: u8,
) -> Result<CompilerLlvmOperationKindV1, CompilerInstructionSelectionCorrespondenceErrorV1> {
    use CompilerLlvmOperationKindV1 as Kind;
    Ok(match tag {
        1 => Kind::Constant,
        2 => Kind::Vector,
        3 => Kind::SymbolAddress,
        4 => Kind::IntegerAdd,
        5 => Kind::IntegerSubtract,
        6 => Kind::IntegerMultiply,
        7 => Kind::IntegerDivide,
        8 => Kind::IntegerRemainder,
        9 => Kind::IntegerBitwise,
        10 => Kind::IntegerShift,
        11 => Kind::FloatAdd,
        12 => Kind::FloatSubtract,
        13 => Kind::FloatMultiply,
        14 => Kind::FloatDivide,
        15 => Kind::FloatFma,
        16 => Kind::FloatSqrt,
        17 => Kind::IntegerCompare,
        18 => Kind::FloatCompare,
        19 => Kind::Cast,
        20 => Kind::Select,
        21 => Kind::Allocate,
        22 => Kind::Address,
        23 => Kind::Load,
        24 => Kind::Store,
        25 => Kind::Atomic,
        26 => Kind::Fence,
        27 => Kind::Barrier,
        28 => Kind::Call,
        29 => Kind::Intrinsic,
        30 => Kind::Phi,
        31 => Kind::Branch,
        32 => Kind::ConditionalBranch,
        33 => Kind::Switch,
        34 => Kind::Return,
        35 => Kind::Unreachable,
        36 => Kind::InvocationIndex,
        37 => Kind::Collective,
        38 => Kind::Matrix,
        39 => Kind::InlineAssembly,
        40 => Kind::Trap,
        _ => return Err(CompilerInstructionSelectionCorrespondenceErrorV1::UnknownTag),
    })
}

fn encode_value_type(output: &mut Vec<u8>, value_type: CompilerLlvmValueTypeV1) {
    match value_type {
        CompilerLlvmValueTypeV1::Void => output.push(0),
        CompilerLlvmValueTypeV1::Integer(bits) => {
            output.push(1);
            put_u16(output, bits);
        }
        CompilerLlvmValueTypeV1::Float(bits) => {
            output.push(2);
            put_u16(output, bits);
        }
        CompilerLlvmValueTypeV1::Pointer {
            address_space,
            bits,
        } => {
            output.push(3);
            put_u32(output, address_space);
            put_u16(output, bits);
        }
        CompilerLlvmValueTypeV1::Vector {
            element_bits,
            lanes,
            floating,
        } => {
            output.push(4);
            put_u16(output, element_bits);
            put_u16(output, lanes);
            output.push(u8::from(floating));
        }
    }
}

fn decode_value_type(
    input: &mut Input<'_>,
) -> Result<CompilerLlvmValueTypeV1, CompilerInstructionSelectionCorrespondenceErrorV1> {
    let value_type = match input.u8()? {
        0 => CompilerLlvmValueTypeV1::Void,
        1 => CompilerLlvmValueTypeV1::Integer(input.u16()?),
        2 => CompilerLlvmValueTypeV1::Float(input.u16()?),
        3 => CompilerLlvmValueTypeV1::Pointer {
            address_space: input.u32()?,
            bits: input.u16()?,
        },
        4 => CompilerLlvmValueTypeV1::Vector {
            element_bits: input.u16()?,
            lanes: input.u16()?,
            floating: input.boolean()?,
        },
        _ => return Err(CompilerInstructionSelectionCorrespondenceErrorV1::UnknownTag),
    };
    validate_value_type(value_type)?;
    Ok(value_type)
}

fn encode_memory(output: &mut Vec<u8>, memory: CompilerMemoryEffectV1) {
    output.push(match memory.kind {
        CompilerMemoryEffectKindV1::None => 0,
        CompilerMemoryEffectKindV1::Read => 1,
        CompilerMemoryEffectKindV1::Write => 2,
        CompilerMemoryEffectKindV1::ReadWrite => 3,
        CompilerMemoryEffectKindV1::Fence => 4,
        CompilerMemoryEffectKindV1::Barrier => 5,
    });
    put_u32(output, memory.address_space);
    put_u32(output, memory.byte_width);
    put_u32(output, memory.alignment);
    output.push(match memory.ordering {
        CompilerMemoryOrderingV1::NotAtomic => 0,
        CompilerMemoryOrderingV1::Relaxed => 1,
        CompilerMemoryOrderingV1::Acquire => 2,
        CompilerMemoryOrderingV1::Release => 3,
        CompilerMemoryOrderingV1::AcquireRelease => 4,
        CompilerMemoryOrderingV1::SequentiallyConsistent => 5,
    });
    output.push(match memory.scope {
        CompilerMemoryScopeV1::None => 0,
        CompilerMemoryScopeV1::Invocation => 1,
        CompilerMemoryScopeV1::Subgroup => 2,
        CompilerMemoryScopeV1::Workgroup => 3,
        CompilerMemoryScopeV1::Agent => 4,
        CompilerMemoryScopeV1::System => 5,
    });
    output.push(u8::from(memory.volatile));
}

fn decode_memory(
    input: &mut Input<'_>,
) -> Result<CompilerMemoryEffectV1, CompilerInstructionSelectionCorrespondenceErrorV1> {
    let kind = match input.u8()? {
        0 => CompilerMemoryEffectKindV1::None,
        1 => CompilerMemoryEffectKindV1::Read,
        2 => CompilerMemoryEffectKindV1::Write,
        3 => CompilerMemoryEffectKindV1::ReadWrite,
        4 => CompilerMemoryEffectKindV1::Fence,
        5 => CompilerMemoryEffectKindV1::Barrier,
        _ => return Err(CompilerInstructionSelectionCorrespondenceErrorV1::UnknownTag),
    };
    let address_space = input.u32()?;
    let byte_width = input.u32()?;
    let alignment = input.u32()?;
    let ordering = match input.u8()? {
        0 => CompilerMemoryOrderingV1::NotAtomic,
        1 => CompilerMemoryOrderingV1::Relaxed,
        2 => CompilerMemoryOrderingV1::Acquire,
        3 => CompilerMemoryOrderingV1::Release,
        4 => CompilerMemoryOrderingV1::AcquireRelease,
        5 => CompilerMemoryOrderingV1::SequentiallyConsistent,
        _ => return Err(CompilerInstructionSelectionCorrespondenceErrorV1::UnknownTag),
    };
    let scope = match input.u8()? {
        0 => CompilerMemoryScopeV1::None,
        1 => CompilerMemoryScopeV1::Invocation,
        2 => CompilerMemoryScopeV1::Subgroup,
        3 => CompilerMemoryScopeV1::Workgroup,
        4 => CompilerMemoryScopeV1::Agent,
        5 => CompilerMemoryScopeV1::System,
        _ => return Err(CompilerInstructionSelectionCorrespondenceErrorV1::UnknownTag),
    };
    CompilerMemoryEffectV1::new(
        kind,
        address_space,
        byte_width,
        alignment,
        ordering,
        scope,
        input.boolean()?,
    )
}

fn divergence_tag(divergence: CompilerBranchDivergenceV1) -> u8 {
    match divergence {
        CompilerBranchDivergenceV1::None => 0,
        CompilerBranchDivergenceV1::Uniform => 1,
        CompilerBranchDivergenceV1::Divergent => 2,
    }
}

fn numerical_tag(numerical: CompilerNumericalContractV1) -> u8 {
    match numerical {
        CompilerNumericalContractV1::Exact => 0,
        CompilerNumericalContractV1::IeeeBinary32 => 1,
        CompilerNumericalContractV1::IeeeBinary64 => 2,
        CompilerNumericalContractV1::IeeeBinary32Fused => 3,
        CompilerNumericalContractV1::TargetDefined => 4,
        CompilerNumericalContractV1::MfmaBf16AccumulateF32 => 5,
        CompilerNumericalContractV1::MfmaFp8E4M3AccumulateF32 => 6,
        CompilerNumericalContractV1::MfmaFp8E5M2AccumulateF32 => 7,
        CompilerNumericalContractV1::MfmaFp4E2M1AccumulateF32 => 8,
        CompilerNumericalContractV1::MfmaFp4E2M1ByFp8E4M3AccumulateF32 => 9,
    }
}

fn put_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn put_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn put_u64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn put_i64(output: &mut Vec<u8>, value: i64) {
    output.extend_from_slice(&value.to_le_bytes());
}

struct Input<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Input<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take(
        &mut self,
        length: usize,
    ) -> Result<&'a [u8], CompilerInstructionSelectionCorrespondenceErrorV1> {
        let end = self
            .cursor
            .checked_add(length)
            .ok_or(CompilerInstructionSelectionCorrespondenceErrorV1::Truncated)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(CompilerInstructionSelectionCorrespondenceErrorV1::Truncated)?;
        self.cursor = end;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, CompilerInstructionSelectionCorrespondenceErrorV1> {
        Ok(self.take(1)?[0])
    }

    fn boolean(&mut self) -> Result<bool, CompilerInstructionSelectionCorrespondenceErrorV1> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(CompilerInstructionSelectionCorrespondenceErrorV1::UnknownTag),
        }
    }

    fn u16(&mut self) -> Result<u16, CompilerInstructionSelectionCorrespondenceErrorV1> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().map_err(
            |_| CompilerInstructionSelectionCorrespondenceErrorV1::Truncated,
        )?))
    }

    fn u32(&mut self) -> Result<u32, CompilerInstructionSelectionCorrespondenceErrorV1> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().map_err(
            |_| CompilerInstructionSelectionCorrespondenceErrorV1::Truncated,
        )?))
    }

    fn u64(&mut self) -> Result<u64, CompilerInstructionSelectionCorrespondenceErrorV1> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().map_err(
            |_| CompilerInstructionSelectionCorrespondenceErrorV1::Truncated,
        )?))
    }

    fn i64(&mut self) -> Result<i64, CompilerInstructionSelectionCorrespondenceErrorV1> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().map_err(
            |_| CompilerInstructionSelectionCorrespondenceErrorV1::Truncated,
        )?))
    }

    fn array16(&mut self) -> Result<[u8; 16], CompilerInstructionSelectionCorrespondenceErrorV1> {
        self.take(16)?
            .try_into()
            .map_err(|_| CompilerInstructionSelectionCorrespondenceErrorV1::Truncated)
    }

    fn array32(&mut self) -> Result<[u8; 32], CompilerInstructionSelectionCorrespondenceErrorV1> {
        self.take(32)?
            .try_into()
            .map_err(|_| CompilerInstructionSelectionCorrespondenceErrorV1::Truncated)
    }

    fn count(
        &mut self,
        maximum: usize,
    ) -> Result<usize, CompilerInstructionSelectionCorrespondenceErrorV1> {
        let count = usize::try_from(self.u32()?)
            .map_err(|_| CompilerInstructionSelectionCorrespondenceErrorV1::ResourceLimit)?;
        if count > maximum {
            return Err(CompilerInstructionSelectionCorrespondenceErrorV1::ResourceLimit);
        }
        Ok(count)
    }

    fn content_identity(
        &mut self,
    ) -> Result<
        ExactCompilerStageContentIdentityV1,
        CompilerInstructionSelectionCorrespondenceErrorV1,
    > {
        let digest = self.array32()?;
        let byte_len = self.u64()?;
        if digest == [0; 32] || byte_len == 0 {
            return Err(CompilerInstructionSelectionCorrespondenceErrorV1::NonCanonical);
        }
        // The stage identity deliberately has no unchecked public constructor. Reconstruct it by
        // retaining its wire components locally, then compare canonical bytes through a tiny
        // identity carrier encoded by a private helper in the owning module.
        exact_identity_from_wire(digest, byte_len)
    }

    fn finish(self) -> Result<(), CompilerInstructionSelectionCorrespondenceErrorV1> {
        if self.cursor == self.bytes.len() {
            Ok(())
        } else {
            Err(CompilerInstructionSelectionCorrespondenceErrorV1::NonCanonical)
        }
    }
}

fn exact_identity_from_wire(
    digest: [u8; 32],
    byte_len: u64,
) -> Result<ExactCompilerStageContentIdentityV1, CompilerInstructionSelectionCorrespondenceErrorV1>
{
    ExactCompilerStageContentIdentityV1::from_checked_parts(digest, byte_len)
        .ok_or(CompilerInstructionSelectionCorrespondenceErrorV1::NonCanonical)
}

/// Reconstructs a validated exact content identity for another canonical refinement schema.
///
/// This is not an identity assertion: consumers must still match the result against retained
/// bytes before admitting evidence.
pub fn decode_exact_compiler_stage_content_identity_v1(
    digest: [u8; 32],
    byte_len: u64,
) -> Option<ExactCompilerStageContentIdentityV1> {
    ExactCompilerStageContentIdentityV1::from_checked_parts(digest, byte_len)
}
