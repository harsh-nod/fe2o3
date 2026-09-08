//! Bounded, authority-free structured KIR V13 to LLVM lowering derivations.

use core::fmt;
use sha2::{Digest, Sha256};

const IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/STRUCTURED-KIR-V13-TO-LLVM-DERIVATION/V1\0";
const MAX_SYMBOL_BYTES_V1: usize = 256;
const MAX_RECORD_OPERANDS_V1: usize = 32;
const MAX_RECORD_RESULTS_V1: usize = 16;
const MAX_RECORD_LLVM_INSTRUCTIONS_V1: usize = 32;

/// Maximum block records retained by one derivation.
pub const MAX_STRUCTURED_KIR_TO_LLVM_BLOCKS_V1: usize = 4_096;
/// Maximum operation records retained by one derivation.
pub const MAX_STRUCTURED_KIR_TO_LLVM_OPERATIONS_V1: usize = 262_144;
/// Maximum value records retained by one derivation.
pub const MAX_STRUCTURED_KIR_TO_LLVM_VALUES_V1: usize = 524_288;
/// Maximum control-edge records retained by one derivation.
pub const MAX_STRUCTURED_KIR_TO_LLVM_EDGES_V1: usize = 524_288;

/// Exact physical LLVM target selected by a structured derivation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StructuredLlvmTargetV1 {
    /// AMD gfx942, HSA ABI, wave64-capable, with XNACK disabled.
    AmdGfx942XnackMinus,
    /// AMD gfx950, HSA ABI, wave64-capable, with XNACK disabled.
    AmdGfx950XnackMinus,
}

/// Floating-point policy retained beside the structured lowering.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StructuredLlvmNumericalPolicyV1 {
    /// IEEE binary32 operations, preserved NaN/Inf/signed-zero behavior, and no contraction.
    IeeeBinary32SeparateMulAdd,
}

/// Closed type vocabulary for the first structured lowering slice.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StructuredKirValueTypeV1 {
    /// Logical kernel context erased from the physical ABI.
    KernelContext,
    /// One-bit boolean.
    Bool,
    /// Target-neutral index lowered to i64.
    Index,
    /// IEEE binary32 scalar.
    F32,
    /// Read-only global f32 slice lowered to pointer plus length.
    GlobalReadSliceF32,
    /// Write-only global f32 slice lowered to pointer plus length.
    GlobalWriteSliceF32,
    /// Read-only global f32 capability aliased to an admitted slice.
    GlobalReadCapabilityF32,
    /// Disjoint-write global f32 capability aliased to an admitted slice.
    GlobalWriteCapabilityF32,
    /// Read-only global f32 pointer.
    GlobalReadPointerF32,
    /// Write-only global f32 pointer.
    GlobalWritePointerF32,
}

/// Physical carrier used for one KIR SSA value.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StructuredKirValueCarrierV1 {
    /// Consecutive physical LLVM function arguments.
    Argument {
        /// First physical LLVM argument ordinal.
        first: u16,
        /// Number of consecutive physical arguments.
        count: u8,
    },
    /// One LLVM phi result in the named source block and parameter position.
    Phi {
        /// Source block receiving the value.
        block: u32,
        /// Block-parameter/phi ordinal.
        ordinal: u16,
    },
    /// Result of the named KIR operation's emitted LLVM instruction expansion.
    Instruction {
        /// Source block identifier.
        block: u32,
        /// Source operation ordinal within the block.
        operation: u32,
        /// Result-producing LLVM instruction ordinal within the operation expansion.
        ordinal: u16,
    },
    /// Compile-time immediate with no LLVM SSA definition.
    Immediate,
    /// Exact alias of another KIR value's physical carrier.
    Alias {
        /// Source KIR value whose physical carrier is reused.
        value: u32,
    },
    /// Proof-only value erased from the physical program.
    Erased,
}

/// Closed operation vocabulary for the exact scalar-f32 structured slice.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StructuredKirOperationKindV1 {
    /// Kernel-context issuance, erased after capability checking.
    KernelContextIssue,
    /// Global capability binding, represented as a physical slice alias.
    GlobalCapabilityBind,
    /// Global capability index projection, represented as an index alias.
    GlobalCapabilityIndex,
    /// One-dimensional global invocation index expansion.
    GlobalId1d,
    /// Index immediate.
    ConstantIndex,
    /// Binary32 immediate.
    ConstantF32,
    /// Integer inequality comparison.
    IntegerNotEqual,
    /// Unsigned integer less-than comparison.
    IntegerLessThan,
    /// Scalar select.
    Select,
    /// Integer multiplication.
    IntegerMultiply,
    /// Integer addition.
    IntegerAdd,
    /// Boolean conjunction.
    BooleanAnd,
    /// Unsigned integer division.
    IntegerDivide,
    /// Unsigned integer remainder.
    IntegerRemainder,
    /// Slice-length projection.
    SliceLength,
    /// Slice-data projection.
    SliceData,
    /// Global pointer element addressing.
    GlobalGetElementPointer,
    /// Predicated global f32 load with fallback.
    GuardedLoadF32,
    /// Predicated global f32 store.
    GuardedStoreF32,
    /// Non-contracted binary32 multiplication.
    F32Multiply,
    /// Non-contracted binary32 addition.
    F32Add,
}

/// Closed LLVM instruction vocabulary emitted for the exact scalar-f32 slice.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StructuredLlvmOpcodeV1 {
    /// Call `llvm.amdgcn.workitem.id.x` with an i32 result.
    CallWorkitemIdX,
    /// Call `llvm.amdgcn.workgroup.id.x` with an i32 result.
    CallWorkgroupIdX,
    /// Zero-extend an i32 value to i64.
    ZeroExtendI32ToI64,
    /// Multiply two i64 values.
    MultiplyI64,
    /// Add two i64 values.
    AddI64,
    /// Compare two i64 values for inequality.
    CompareNotEqualI64,
    /// Compare two i64 values with unsigned less-than.
    CompareUnsignedLessThanI64,
    /// Select between two equally typed scalar values.
    Select,
    /// Compute the bitwise conjunction of two i1 values.
    AndI1,
    /// Divide two i64 values as unsigned integers.
    DivideUnsignedI64,
    /// Compute the unsigned i64 remainder.
    RemainderUnsignedI64,
    /// Copy a slice length with `add i64 length, 0`.
    CopySliceLengthI64,
    /// Project a global slice data pointer with zero-offset `getelementptr i8`.
    ProjectSliceDataGlobal,
    /// Compute a global f32 element pointer.
    GetElementPointerGlobalF32,
    /// Conditionally branch on an i1 value.
    ConditionalBranch,
    /// Unconditionally branch to one block.
    Branch,
    /// Load one f32 value from global address space.
    LoadGlobalF32,
    /// Store one f32 value to global address space.
    StoreGlobalF32,
    /// Merge f32 values with a phi instruction.
    PhiF32,
    /// Multiply two f32 values without contraction.
    MultiplyF32,
    /// Add two f32 values without contraction.
    AddF32,
}

/// One source block and its stable LLVM block ordinal.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StructuredKirBlockLoweringV1 {
    function: u32,
    block: u32,
    llvm_ordinal: u32,
}

impl StructuredKirBlockLoweringV1 {
    /// Creates one block mapping.
    pub const fn new(function: u32, block: u32, llvm_ordinal: u32) -> Self {
        Self {
            function,
            block,
            llvm_ordinal,
        }
    }

    /// Returns the source function ordinal.
    pub const fn function(self) -> u32 {
        self.function
    }
    /// Returns the source block identifier.
    pub const fn block(self) -> u32 {
        self.block
    }
    /// Returns the stable primary LLVM block ordinal.
    pub const fn llvm_ordinal(self) -> u32 {
        self.llvm_ordinal
    }
}

/// Complete mapping for one KIR operation.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StructuredKirOperationLoweringV1 {
    function: u32,
    block: u32,
    operation: u32,
    kind: StructuredKirOperationKindV1,
    operands: Box<[u32]>,
    results: Box<[u32]>,
    result_types: Box<[StructuredKirValueTypeV1]>,
    llvm_opcodes: Box<[StructuredLlvmOpcodeV1]>,
}

impl StructuredKirOperationLoweringV1 {
    /// Creates one bounded operation mapping.
    pub fn new(
        function: u32,
        block: u32,
        operation: u32,
        kind: StructuredKirOperationKindV1,
        operands: impl Into<Box<[u32]>>,
        results: impl Into<Box<[u32]>>,
        result_types: impl Into<Box<[StructuredKirValueTypeV1]>>,
        llvm_opcodes: impl Into<Box<[StructuredLlvmOpcodeV1]>>,
    ) -> Result<Self, StructuredKirToLlvmDerivationErrorV1> {
        let operands = operands.into();
        let results = results.into();
        let result_types = result_types.into();
        let llvm_opcodes = llvm_opcodes.into();
        if operands.len() > MAX_RECORD_OPERANDS_V1
            || results.len() > MAX_RECORD_RESULTS_V1
            || llvm_opcodes.len() > MAX_RECORD_LLVM_INSTRUCTIONS_V1
        {
            return Err(StructuredKirToLlvmDerivationErrorV1::RecordTooLarge);
        }
        if results.len() != result_types.len() {
            return Err(StructuredKirToLlvmDerivationErrorV1::ResultTypeArityMismatch);
        }
        Ok(Self {
            function,
            block,
            operation,
            kind,
            operands,
            results,
            result_types,
            llvm_opcodes,
        })
    }

    /// Returns the source function ordinal.
    pub const fn function(&self) -> u32 {
        self.function
    }
    /// Returns the source block identifier.
    pub const fn block(&self) -> u32 {
        self.block
    }
    /// Returns the source operation ordinal.
    pub const fn operation(&self) -> u32 {
        self.operation
    }
    /// Returns the checked operation class.
    pub const fn kind(&self) -> StructuredKirOperationKindV1 {
        self.kind
    }
    /// Returns source SSA operands in semantic order.
    pub fn operands(&self) -> &[u32] {
        &self.operands
    }
    /// Returns source SSA results in semantic order.
    pub fn results(&self) -> &[u32] {
        &self.results
    }
    /// Returns the result types aligned with `results`.
    pub fn result_types(&self) -> &[StructuredKirValueTypeV1] {
        &self.result_types
    }
    /// Returns the ordered structured LLVM instruction expansion.
    pub fn llvm_opcodes(&self) -> &[StructuredLlvmOpcodeV1] {
        &self.llvm_opcodes
    }
    /// Returns the exact number of LLVM instructions emitted for this source operation.
    pub fn llvm_instruction_count(&self) -> u16 {
        self.llvm_opcodes.len() as u16
    }
}

/// Complete mapping for one KIR value.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StructuredKirValueLoweringV1 {
    function: u32,
    value: u32,
    ty: StructuredKirValueTypeV1,
    carrier: StructuredKirValueCarrierV1,
}

impl StructuredKirValueLoweringV1 {
    /// Creates one value mapping.
    pub const fn new(
        function: u32,
        value: u32,
        ty: StructuredKirValueTypeV1,
        carrier: StructuredKirValueCarrierV1,
    ) -> Self {
        Self {
            function,
            value,
            ty,
            carrier,
        }
    }

    /// Returns the source function ordinal.
    pub const fn function(self) -> u32 {
        self.function
    }
    /// Returns the source value identifier.
    pub const fn value(self) -> u32 {
        self.value
    }
    /// Returns the closed source/physical type class.
    pub const fn ty(self) -> StructuredKirValueTypeV1 {
        self.ty
    }
    /// Returns the exact physical carrier relation.
    pub const fn carrier(self) -> StructuredKirValueCarrierV1 {
        self.carrier
    }
}

/// One source CFG edge and its physical branch/phi relation.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StructuredKirControlEdgeLoweringV1 {
    function: u32,
    predecessor: u32,
    ordinal: u16,
    successor: u32,
    arguments: Box<[u32]>,
    split_for_phi: bool,
}

impl StructuredKirControlEdgeLoweringV1 {
    /// Creates one bounded control-edge mapping.
    pub fn new(
        function: u32,
        predecessor: u32,
        ordinal: u16,
        successor: u32,
        arguments: impl Into<Box<[u32]>>,
        split_for_phi: bool,
    ) -> Result<Self, StructuredKirToLlvmDerivationErrorV1> {
        let arguments = arguments.into();
        if arguments.len() > MAX_RECORD_OPERANDS_V1 {
            return Err(StructuredKirToLlvmDerivationErrorV1::RecordTooLarge);
        }
        Ok(Self {
            function,
            predecessor,
            ordinal,
            successor,
            arguments,
            split_for_phi,
        })
    }

    /// Returns the source function ordinal.
    pub const fn function(&self) -> u32 {
        self.function
    }
    /// Returns the predecessor block identifier.
    pub const fn predecessor(&self) -> u32 {
        self.predecessor
    }
    /// Returns the terminator-edge ordinal.
    pub const fn ordinal(&self) -> u16 {
        self.ordinal
    }
    /// Returns the successor block identifier.
    pub const fn successor(&self) -> u32 {
        self.successor
    }
    /// Returns the branch arguments aligned with successor phis.
    pub fn arguments(&self) -> &[u32] {
        &self.arguments
    }
    /// Returns whether LLVM materializes a dedicated edge block.
    pub const fn split_for_phi(&self) -> bool {
        self.split_for_phi
    }
}

/// Public parts used to transport and hostile-test an inert derivation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredKirToLlvmDerivationPartsV1 {
    /// Canonical KIR V13 SHA-256.
    pub kir_sha256: [u8; 32],
    /// Canonical KIR V13 byte length.
    pub kir_byte_len: u64,
    /// Exact LLVM text SHA-256.
    pub llvm_sha256: [u8; 32],
    /// Exact LLVM text byte length.
    pub llvm_byte_len: u64,
    /// Selected physical target.
    pub target: StructuredLlvmTargetV1,
    /// Selected numerical policy.
    pub numerical_policy: StructuredLlvmNumericalPolicyV1,
    /// Exact emitted function symbol.
    pub function_symbol: String,
    /// Exact flat workgroup size.
    pub flat_workgroup_size: u32,
    /// Complete block roster.
    pub blocks: Box<[StructuredKirBlockLoweringV1]>,
    /// Complete operation roster.
    pub operations: Box<[StructuredKirOperationLoweringV1]>,
    /// Complete SSA/ABI carrier roster.
    pub values: Box<[StructuredKirValueLoweringV1]>,
    /// Complete CFG edge roster.
    pub edges: Box<[StructuredKirControlEdgeLoweringV1]>,
}

/// Identity of canonical structured derivation content.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StructuredKirToLlvmDerivationIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl StructuredKirToLlvmDerivationIdentityV1 {
    /// Returns the derivation SHA-256.
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }
    /// Returns the canonical derivation byte length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Bounded typed derivation. This value is inert and grants no compiler or runtime authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredKirToLlvmDerivationV1 {
    parts: StructuredKirToLlvmDerivationPartsV1,
    identity: StructuredKirToLlvmDerivationIdentityV1,
}

impl StructuredKirToLlvmDerivationV1 {
    /// Constructs a bounded inert derivation and binds it to exact LLVM bytes.
    pub fn new(
        kir_sha256: [u8; 32],
        kir_byte_len: u64,
        llvm_bytes: &[u8],
        target: StructuredLlvmTargetV1,
        numerical_policy: StructuredLlvmNumericalPolicyV1,
        function_symbol: impl Into<String>,
        flat_workgroup_size: u32,
        blocks: impl Into<Box<[StructuredKirBlockLoweringV1]>>,
        operations: impl Into<Box<[StructuredKirOperationLoweringV1]>>,
        values: impl Into<Box<[StructuredKirValueLoweringV1]>>,
        edges: impl Into<Box<[StructuredKirControlEdgeLoweringV1]>>,
    ) -> Result<Self, StructuredKirToLlvmDerivationErrorV1> {
        let llvm_byte_len = u64::try_from(llvm_bytes.len())
            .map_err(|_| StructuredKirToLlvmDerivationErrorV1::LengthOverflow)?;
        let llvm_sha256 = Sha256::digest(llvm_bytes).into();
        Self::from_parts(StructuredKirToLlvmDerivationPartsV1 {
            kir_sha256,
            kir_byte_len,
            llvm_sha256,
            llvm_byte_len,
            target,
            numerical_policy,
            function_symbol: function_symbol.into(),
            flat_workgroup_size,
            blocks: blocks.into(),
            operations: operations.into(),
            values: values.into(),
            edges: edges.into(),
        })
    }

    /// Reconstructs an inert derivation from explicit parts for transport and hostile replay.
    pub fn from_parts(
        parts: StructuredKirToLlvmDerivationPartsV1,
    ) -> Result<Self, StructuredKirToLlvmDerivationErrorV1> {
        validate_parts(&parts)?;
        let canonical = encode_parts(&parts)?;
        let identity = StructuredKirToLlvmDerivationIdentityV1 {
            sha256: Sha256::digest(&canonical).into(),
            byte_len: canonical.len() as u64,
        };
        Ok(Self { parts, identity })
    }

    /// Returns the exact KIR digest.
    pub const fn kir_sha256(&self) -> [u8; 32] {
        self.parts.kir_sha256
    }
    /// Returns the exact KIR byte length.
    pub const fn kir_byte_len(&self) -> u64 {
        self.parts.kir_byte_len
    }
    /// Returns the exact LLVM digest.
    pub const fn llvm_sha256(&self) -> [u8; 32] {
        self.parts.llvm_sha256
    }
    /// Returns the exact LLVM byte length.
    pub const fn llvm_byte_len(&self) -> u64 {
        self.parts.llvm_byte_len
    }
    /// Returns the physical target.
    pub const fn target(&self) -> StructuredLlvmTargetV1 {
        self.parts.target
    }
    /// Returns the numerical policy.
    pub const fn numerical_policy(&self) -> StructuredLlvmNumericalPolicyV1 {
        self.parts.numerical_policy
    }
    /// Returns the function symbol.
    pub fn function_symbol(&self) -> &str {
        &self.parts.function_symbol
    }
    /// Returns the flat workgroup size.
    pub const fn flat_workgroup_size(&self) -> u32 {
        self.parts.flat_workgroup_size
    }
    /// Returns block records.
    pub fn blocks(&self) -> &[StructuredKirBlockLoweringV1] {
        &self.parts.blocks
    }
    /// Returns operation records.
    pub fn operations(&self) -> &[StructuredKirOperationLoweringV1] {
        &self.parts.operations
    }
    /// Returns value records.
    pub fn values(&self) -> &[StructuredKirValueLoweringV1] {
        &self.parts.values
    }
    /// Returns control-edge records.
    pub fn edges(&self) -> &[StructuredKirControlEdgeLoweringV1] {
        &self.parts.edges
    }
    /// Returns the canonical content identity.
    pub const fn identity(&self) -> StructuredKirToLlvmDerivationIdentityV1 {
        self.identity
    }
    /// Returns false because this content carries no authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
    /// Decomposes the inert record.
    pub fn into_parts(self) -> StructuredKirToLlvmDerivationPartsV1 {
        self.parts
    }
}

/// Representation failure for a structured derivation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuredKirToLlvmDerivationErrorV1 {
    /// A top-level roster exceeded its fixed bound.
    RosterTooLarge,
    /// A record-local operand or result roster exceeded its fixed bound.
    RecordTooLarge,
    /// Result identifiers and result types were not aligned.
    ResultTypeArityMismatch,
    /// Function symbol was empty, unsafe, or too large.
    InvalidFunctionSymbol,
    /// A required identity, length, workgroup, or roster was empty.
    EmptyRequiredField,
    /// A host length could not be represented canonically.
    LengthOverflow,
}

impl fmt::Display for StructuredKirToLlvmDerivationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "structured KIR-to-LLVM derivation rejected: {self:?}"
        )
    }
}

impl std::error::Error for StructuredKirToLlvmDerivationErrorV1 {}

fn validate_parts(
    parts: &StructuredKirToLlvmDerivationPartsV1,
) -> Result<(), StructuredKirToLlvmDerivationErrorV1> {
    if parts.blocks.len() > MAX_STRUCTURED_KIR_TO_LLVM_BLOCKS_V1
        || parts.operations.len() > MAX_STRUCTURED_KIR_TO_LLVM_OPERATIONS_V1
        || parts.values.len() > MAX_STRUCTURED_KIR_TO_LLVM_VALUES_V1
        || parts.edges.len() > MAX_STRUCTURED_KIR_TO_LLVM_EDGES_V1
    {
        return Err(StructuredKirToLlvmDerivationErrorV1::RosterTooLarge);
    }
    if parts.kir_sha256 == [0; 32]
        || parts.llvm_sha256 == [0; 32]
        || parts.kir_byte_len == 0
        || parts.llvm_byte_len == 0
        || parts.flat_workgroup_size == 0
        || parts.blocks.is_empty()
        || parts.operations.is_empty()
        || parts.values.is_empty()
    {
        return Err(StructuredKirToLlvmDerivationErrorV1::EmptyRequiredField);
    }
    if parts.function_symbol.is_empty()
        || parts.function_symbol.len() > MAX_SYMBOL_BYTES_V1
        || !parts
            .function_symbol
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(StructuredKirToLlvmDerivationErrorV1::InvalidFunctionSymbol);
    }
    Ok(())
}

fn encode_parts(
    parts: &StructuredKirToLlvmDerivationPartsV1,
) -> Result<Vec<u8>, StructuredKirToLlvmDerivationErrorV1> {
    let mut out = Vec::new();
    out.extend_from_slice(IDENTITY_DOMAIN_V1);
    out.extend_from_slice(&parts.kir_sha256);
    push_u64(&mut out, parts.kir_byte_len);
    out.extend_from_slice(&parts.llvm_sha256);
    push_u64(&mut out, parts.llvm_byte_len);
    out.push(parts.target as u8);
    out.push(parts.numerical_policy as u8);
    push_bytes(&mut out, parts.function_symbol.as_bytes())?;
    push_u32(&mut out, parts.flat_workgroup_size);
    push_u32(&mut out, parts.blocks.len() as u32);
    for block in &parts.blocks {
        push_u32(&mut out, block.function);
        push_u32(&mut out, block.block);
        push_u32(&mut out, block.llvm_ordinal);
    }
    push_u32(&mut out, parts.operations.len() as u32);
    for op in &parts.operations {
        push_u32(&mut out, op.function);
        push_u32(&mut out, op.block);
        push_u32(&mut out, op.operation);
        out.push(op.kind as u8);
        push_u16(&mut out, op.llvm_opcodes.len() as u16);
        out.extend(op.llvm_opcodes.iter().map(|opcode| *opcode as u8));
        push_u32_slice(&mut out, &op.operands)?;
        push_u32_slice(&mut out, &op.results)?;
        push_u32(&mut out, op.result_types.len() as u32);
        out.extend(op.result_types.iter().map(|ty| *ty as u8));
    }
    push_u32(&mut out, parts.values.len() as u32);
    for value in &parts.values {
        push_u32(&mut out, value.function);
        push_u32(&mut out, value.value);
        out.push(value.ty as u8);
        match value.carrier {
            StructuredKirValueCarrierV1::Argument { first, count } => {
                out.push(0);
                push_u16(&mut out, first);
                out.push(count);
            }
            StructuredKirValueCarrierV1::Phi { block, ordinal } => {
                out.push(1);
                push_u32(&mut out, block);
                push_u16(&mut out, ordinal);
            }
            StructuredKirValueCarrierV1::Instruction {
                block,
                operation,
                ordinal,
            } => {
                out.push(2);
                push_u32(&mut out, block);
                push_u32(&mut out, operation);
                push_u16(&mut out, ordinal);
            }
            StructuredKirValueCarrierV1::Immediate => out.push(3),
            StructuredKirValueCarrierV1::Alias { value } => {
                out.push(4);
                push_u32(&mut out, value);
            }
            StructuredKirValueCarrierV1::Erased => out.push(5),
        }
    }
    push_u32(&mut out, parts.edges.len() as u32);
    for edge in &parts.edges {
        push_u32(&mut out, edge.function);
        push_u32(&mut out, edge.predecessor);
        push_u16(&mut out, edge.ordinal);
        push_u32(&mut out, edge.successor);
        out.push(u8::from(edge.split_for_phi));
        push_u32_slice(&mut out, &edge.arguments)?;
    }
    Ok(out)
}

fn push_bytes(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), StructuredKirToLlvmDerivationErrorV1> {
    push_u32(
        out,
        u32::try_from(bytes.len())
            .map_err(|_| StructuredKirToLlvmDerivationErrorV1::LengthOverflow)?,
    );
    out.extend_from_slice(bytes);
    Ok(())
}
fn push_u32_slice(
    out: &mut Vec<u8>,
    values: &[u32],
) -> Result<(), StructuredKirToLlvmDerivationErrorV1> {
    push_u32(
        out,
        u32::try_from(values.len())
            .map_err(|_| StructuredKirToLlvmDerivationErrorV1::LengthOverflow)?,
    );
    for value in values {
        push_u32(out, *value);
    }
    Ok(())
}
fn push_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn push_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate() -> StructuredKirToLlvmDerivationV1 {
        StructuredKirToLlvmDerivationV1::new(
            [1; 32],
            7,
            b"llvm",
            StructuredLlvmTargetV1::AmdGfx942XnackMinus,
            StructuredLlvmNumericalPolicyV1::IeeeBinary32SeparateMulAdd,
            "kernel",
            256,
            vec![StructuredKirBlockLoweringV1::new(0, 0, 0)],
            vec![
                StructuredKirOperationLoweringV1::new(
                    0,
                    0,
                    0,
                    StructuredKirOperationKindV1::F32Add,
                    vec![1, 2],
                    vec![3],
                    vec![StructuredKirValueTypeV1::F32],
                    vec![StructuredLlvmOpcodeV1::AddF32],
                )
                .unwrap(),
            ],
            vec![StructuredKirValueLoweringV1::new(
                0,
                3,
                StructuredKirValueTypeV1::F32,
                StructuredKirValueCarrierV1::Instruction {
                    block: 0,
                    operation: 0,
                    ordinal: 0,
                },
            )],
            Vec::<StructuredKirControlEdgeLoweringV1>::new(),
        )
        .unwrap()
    }

    #[test]
    fn identity_binds_every_structured_field() {
        let baseline = candidate();
        let mut parts = baseline.clone().into_parts();
        parts.operations[0] = StructuredKirOperationLoweringV1::new(
            0,
            0,
            0,
            StructuredKirOperationKindV1::F32Multiply,
            vec![1, 2],
            vec![3],
            vec![StructuredKirValueTypeV1::F32],
            vec![StructuredLlvmOpcodeV1::MultiplyF32],
        )
        .unwrap();
        let mutated = StructuredKirToLlvmDerivationV1::from_parts(parts).unwrap();
        assert_ne!(baseline.identity(), mutated.identity());
        assert!(!baseline.grants_authority());
    }

    #[test]
    fn bounds_and_result_type_arity_fail_closed() {
        assert_eq!(
            StructuredKirOperationLoweringV1::new(
                0,
                0,
                0,
                StructuredKirOperationKindV1::F32Add,
                vec![],
                vec![1],
                vec![],
                vec![StructuredLlvmOpcodeV1::AddF32]
            )
            .unwrap_err(),
            StructuredKirToLlvmDerivationErrorV1::ResultTypeArityMismatch
        );
        let mut parts = candidate().into_parts();
        parts.function_symbol.clear();
        assert_eq!(
            StructuredKirToLlvmDerivationV1::from_parts(parts).unwrap_err(),
            StructuredKirToLlvmDerivationErrorV1::InvalidFunctionSymbol
        );
    }
}
