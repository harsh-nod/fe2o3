//! Independent gfx942/gfx950 validation of a closed LLVM-to-machine recurrence subset.
//!
//! Compiler correspondence and ISA decoding are separate inputs. The implemented decoder covers
//! exact SOPP control/barrier, SOP1 EXEC save/restore, selected SOP2 scalar ALU, VOP2 binary32
//! arithmetic, GLOBAL/DS dword memory, DPP binary32 add, and BF16 MFMA bytes. It checks complete
//! executable-section coverage, CFG/backedges, mapped registers, effective addresses, and
//! kernel-descriptor binary32 MODE. The admitted target-specific subset includes closed DPP lane
//! controls, gfx942/gfx950 BF16 MFMA, and gfx950 scaled FP8/FP4 MFMA encodings. Unknown operation
//! or ISA bytes fail closed.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use fe2o3_amd_target::{
    AmdTargetId, PRODUCTION_GFX942_DEVICE_TARGET_V1, PRODUCTION_GFX950_DEVICE_TARGET_V1,
};
use fe2o3_compiler_lineage::{
    CheckedCompilerInstructionSelectionCorrespondenceV1, CompilerBranchDivergenceV1,
    CompilerInstructionSelectionCorrespondenceIdentityV1,
    CompilerInstructionSelectionCorrespondenceV1, CompilerLlvmBlockV1, CompilerLlvmOperationKindV1,
    CompilerLlvmOperationV1, CompilerMachineRegisterClassV1, CompilerMachineRegisterV1,
    CompilerMachineValueAccessV1, CompilerMachineValueLocationV1, CompilerMemoryEffectKindV1,
    CompilerMemoryEffectV1, CompilerMemoryOrderingV1, CompilerMemoryScopeV1,
    CompilerNumericalContractV1, ExactCompilerStageContentIdentityV1,
    MachineRefinementArchitectureV1, MachineRefinementContentIdentityV1, MachineRefinementFamilyV1,
    TargetMachineRefinementReceiptErrorV1, TargetMachineRefinementReceiptPartsV1,
    TargetMachineRefinementTargetV1,
};
use sha2::{Digest as _, Sha256};

use crate::{CheckedGfx942ObjectToHsacoPreservationV1, CheckedGfx950ObjectToHsacoPreservationV1};

/// Canonical independent gfx942 ISA transcript magic.
pub const GFX942_DECODED_ISA_TRANSCRIPT_MAGIC_V1: [u8; 8] = *b"F2G9ISA1";
/// Canonical independent gfx942 ISA transcript version.
pub const GFX942_DECODED_ISA_TRANSCRIPT_VERSION_V1: u16 = 1;
/// Maximum canonical transcript bytes.
pub const MAX_GFX942_DECODED_ISA_TRANSCRIPT_BYTES_V1: usize = 64 * 1024 * 1024;
/// Maximum machine basic blocks.
pub const MAX_GFX942_DECODED_ISA_BLOCKS_V1: usize = 65_536;
/// Maximum decoded instructions.
pub const MAX_GFX942_DECODED_ISA_INSTRUCTIONS_V1: usize = 2_097_152;
/// Maximum instruction bytes retained per object or HSACO encoding.
pub const MAX_GFX942_INSTRUCTION_BYTES_V1: usize = 32;
/// Maximum explicit and implicit register facts per instruction.
pub const MAX_GFX942_INSTRUCTION_REGISTERS_V1: usize = 128;
/// Maximum machine CFG successors per block.
pub const MAX_GFX942_MACHINE_SUCCESSORS_V1: usize = 256;

const ISA_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/GFX942-DECODED-ISA-TRANSCRIPT/V1\0";
const REFINEMENT_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/GFX942-MACHINE-REFINEMENT/V1\0";
const GFX950_REFINEMENT_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/GFX950-MACHINE-REFINEMENT/V1\0";
const COMPUTE_PGM_RSRC1_FLOAT_ROUND_MODE_32_SHIFT: u32 = 12;
const COMPUTE_PGM_RSRC1_FLOAT_DENORM_MODE_32_SHIFT: u32 = 16;
const COMPUTE_PGM_RSRC1_ENABLE_DX10_CLAMP_SHIFT: u32 = 21;
const COMPUTE_PGM_RSRC1_ENABLE_IEEE_MODE_SHIFT: u32 = 23;
const COMPUTE_PGM_RSRC1_TWO_BIT_MASK: u32 = 0b11;
const FLOAT_ROUND_MODE_NEAR_EVEN: u32 = 0;
const FLOAT_DENORM_MODE_FLUSH_NONE: u32 = 3;

const DISCRIMINATOR_BITWISE_AND: u32 = 1;
const DISCRIMINATOR_BITWISE_OR: u32 = 2;
const DISCRIMINATOR_BITWISE_XOR: u32 = 3;
const DISCRIMINATOR_ATOMIC_ADD: u32 = 1;
const DISCRIMINATOR_MFMA_F32_16X16X16_BF16: u32 = 1;
const DISCRIMINATOR_MFMA_F32_16X16X128_FP8_E4M3: u32 = 2;
const DISCRIMINATOR_MFMA_F32_16X16X128_FP8_E5M2: u32 = 3;
const DISCRIMINATOR_MFMA_F32_16X16X128_FP4_E2M1: u32 = 4;
const DISCRIMINATOR_MFMA_F32_16X16X128_FP4_E2M1_BY_FP8_E4M3: u32 = 5;

/// Authenticated AMD target class understood by the independent byte decoder.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AmdMachineRefinementTargetV1 {
    /// CDNA3 gfx942 with the exact production feature state.
    Gfx942,
    /// CDNA4 gfx950 with the exact production feature state.
    Gfx950,
}

impl AmdMachineRefinementTargetV1 {
    /// Selects a decoder only from a parsed target identity, never from artifact names.
    pub fn from_authenticated_target(
        target: AmdTargetId,
    ) -> Result<Self, AmdMachineRefinementTargetErrorV1> {
        let gfx942 = AmdTargetId::parse(PRODUCTION_GFX942_DEVICE_TARGET_V1)
            .map_err(|_| AmdMachineRefinementTargetErrorV1::InvalidProductionTargetConstant)?;
        let gfx950 = AmdTargetId::parse(PRODUCTION_GFX950_DEVICE_TARGET_V1)
            .map_err(|_| AmdMachineRefinementTargetErrorV1::InvalidProductionTargetConstant)?;
        if target == gfx942 {
            Ok(Self::Gfx942)
        } else if target == gfx950 {
            Ok(Self::Gfx950)
        } else {
            Err(AmdMachineRefinementTargetErrorV1::UnsupportedTarget)
        }
    }
}

/// Fail-closed target selection failures for independent AMD decoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AmdMachineRefinementTargetErrorV1 {
    /// A pinned production target constant was internally invalid.
    InvalidProductionTargetConstant,
    /// The authenticated target has no decoder in this revision.
    UnsupportedTarget,
}

impl fmt::Display for AmdMachineRefinementTargetErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "AMD machine-refinement target rejected: {self:?}"
        )
    }
}

impl std::error::Error for AmdMachineRefinementTargetErrorV1 {}

/// One independently auditable component of AMD LLVM-to-machine refinement.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AmdMachineRefinementComponentV1 {
    /// gfx942 SOPP direct control instructions.
    Gfx942SoppControl,
    /// gfx942 SOP1 EXEC save/restore instructions.
    Gfx942Sop1Exec,
    /// gfx942 VOP2 binary32 add and multiply with VGPR operands.
    Gfx942Vop2Float32AddMultiply,
    /// gfx942 GLOBAL dword load/store with an off-mode vector address pair.
    Gfx942GlobalDwordLoadStore,
    /// Selected direct-register SOP2 and VOP2 scalar/vector ALU encodings.
    Gfx942GeneralScalarVectorAlu,
    /// Selected gfx942 DS/LDS dword addressing and data movement.
    Gfx942DsLds,
    /// Exact `s_barrier` workgroup synchronization; waits and standalone fences remain excluded.
    Gfx942Synchronization,
    /// Relaxed workgroup `ds_add_u32`; other atomics and scopes remain excluded.
    Gfx942Atomics,
    /// Multi-instruction subgroup/workgroup collective protocols.
    Gfx942Collectives,
    /// gfx942 MFMA instruction and accumulator semantics.
    Gfx942Mfma,
    /// gfx950 instruction decoding and target-specific semantics.
    Gfx950InstructionSet,
    /// Complete final-LLVM SSA/phi value transport through machine locations.
    FinalLlvmValueTransport,
    /// Semantic application of object relocations into final HSACO instructions and data.
    ObjectRelocationApplication,
    /// Required consumption by the protected production finalization/publication path.
    ProductionPublicationConsumption,
}

/// Honest implementation state for one machine-refinement component.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AmdMachineRefinementComponentDispositionV1 {
    /// Exact bytes are decoded locally and unsupported encodings fail closed.
    IndependentlyDecodedClosedSubset,
    /// No checker currently establishes this component.
    Unavailable,
}

/// One explicit row in the machine-refinement support boundary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AmdMachineRefinementComponentStatusV1 {
    component: AmdMachineRefinementComponentV1,
    disposition: AmdMachineRefinementComponentDispositionV1,
}

impl AmdMachineRefinementComponentStatusV1 {
    const fn new(
        component: AmdMachineRefinementComponentV1,
        disposition: AmdMachineRefinementComponentDispositionV1,
    ) -> Self {
        Self {
            component,
            disposition,
        }
    }

    /// Returns the independently auditable component.
    pub const fn component(self) -> AmdMachineRefinementComponentV1 {
        self.component
    }

    /// Returns its current fail-closed implementation state.
    pub const fn disposition(self) -> AmdMachineRefinementComponentDispositionV1 {
        self.disposition
    }
}

const AMD_MACHINE_REFINEMENT_COMPONENT_STATUS_V1: [AmdMachineRefinementComponentStatusV1; 14] = [
    AmdMachineRefinementComponentStatusV1::new(
        AmdMachineRefinementComponentV1::Gfx942SoppControl,
        AmdMachineRefinementComponentDispositionV1::IndependentlyDecodedClosedSubset,
    ),
    AmdMachineRefinementComponentStatusV1::new(
        AmdMachineRefinementComponentV1::Gfx942Sop1Exec,
        AmdMachineRefinementComponentDispositionV1::IndependentlyDecodedClosedSubset,
    ),
    AmdMachineRefinementComponentStatusV1::new(
        AmdMachineRefinementComponentV1::Gfx942Vop2Float32AddMultiply,
        AmdMachineRefinementComponentDispositionV1::IndependentlyDecodedClosedSubset,
    ),
    AmdMachineRefinementComponentStatusV1::new(
        AmdMachineRefinementComponentV1::Gfx942GlobalDwordLoadStore,
        AmdMachineRefinementComponentDispositionV1::IndependentlyDecodedClosedSubset,
    ),
    AmdMachineRefinementComponentStatusV1::new(
        AmdMachineRefinementComponentV1::Gfx942GeneralScalarVectorAlu,
        AmdMachineRefinementComponentDispositionV1::IndependentlyDecodedClosedSubset,
    ),
    AmdMachineRefinementComponentStatusV1::new(
        AmdMachineRefinementComponentV1::Gfx942DsLds,
        AmdMachineRefinementComponentDispositionV1::IndependentlyDecodedClosedSubset,
    ),
    AmdMachineRefinementComponentStatusV1::new(
        AmdMachineRefinementComponentV1::Gfx942Synchronization,
        AmdMachineRefinementComponentDispositionV1::IndependentlyDecodedClosedSubset,
    ),
    AmdMachineRefinementComponentStatusV1::new(
        AmdMachineRefinementComponentV1::Gfx942Atomics,
        AmdMachineRefinementComponentDispositionV1::IndependentlyDecodedClosedSubset,
    ),
    AmdMachineRefinementComponentStatusV1::new(
        AmdMachineRefinementComponentV1::Gfx942Collectives,
        AmdMachineRefinementComponentDispositionV1::Unavailable,
    ),
    AmdMachineRefinementComponentStatusV1::new(
        AmdMachineRefinementComponentV1::Gfx942Mfma,
        AmdMachineRefinementComponentDispositionV1::Unavailable,
    ),
    AmdMachineRefinementComponentStatusV1::new(
        AmdMachineRefinementComponentV1::Gfx950InstructionSet,
        AmdMachineRefinementComponentDispositionV1::IndependentlyDecodedClosedSubset,
    ),
    AmdMachineRefinementComponentStatusV1::new(
        AmdMachineRefinementComponentV1::FinalLlvmValueTransport,
        AmdMachineRefinementComponentDispositionV1::Unavailable,
    ),
    AmdMachineRefinementComponentStatusV1::new(
        AmdMachineRefinementComponentV1::ObjectRelocationApplication,
        AmdMachineRefinementComponentDispositionV1::Unavailable,
    ),
    AmdMachineRefinementComponentStatusV1::new(
        AmdMachineRefinementComponentV1::ProductionPublicationConsumption,
        AmdMachineRefinementComponentDispositionV1::Unavailable,
    ),
];

/// Returns the complete, fixed support boundary for this machine-refinement adapter.
pub const fn amd_machine_refinement_component_status_v1()
-> &'static [AmdMachineRefinementComponentStatusV1] {
    &AMD_MACHINE_REFINEMENT_COMPONENT_STATUS_V1
}

/// gfx942 register unit used by the operational validator.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Gfx942SemanticRegisterV1 {
    /// Scalar general-purpose register.
    Scalar(u16),
    /// Vector general-purpose register.
    Vector(u16),
    /// Matrix accumulator register.
    Accumulator(u16),
    /// Wave execution mask.
    Exec,
    /// Vector condition mask.
    Vcc,
    /// Scalar condition bit.
    Scc,
    /// Memory offset register.
    M0,
    /// Floating-point MODE register.
    Mode,
}

/// Closed semantic opcode classes produced by an independent gfx942 decoder.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Gfx942SemanticOpcodeV1 {
    /// Copy or constant materialization.
    Copy,
    /// Integer addition.
    IntegerAdd,
    /// Integer subtraction.
    IntegerSubtract,
    /// Integer multiplication.
    IntegerMultiply,
    /// Integer division.
    IntegerDivide,
    /// Integer remainder.
    IntegerRemainder,
    /// Integer bitwise operation.
    IntegerBitwise,
    /// Integer shift.
    IntegerShift,
    /// Floating binary32 addition.
    FloatAdd32,
    /// Floating binary32 subtraction.
    FloatSubtract32,
    /// Floating binary32 multiplication.
    FloatMultiply32,
    /// Floating binary32 division.
    FloatDivide32,
    /// Fused binary32 multiply-add.
    FloatFma32,
    /// Floating binary32 square root.
    FloatSqrt32,
    /// Integer comparison.
    IntegerCompare,
    /// Floating binary32 comparison.
    FloatCompare32,
    /// Scalar or pointer cast.
    Cast,
    /// Value selection.
    Select,
    /// Address calculation.
    Address,
    /// Memory load.
    Load,
    /// Memory store.
    Store,
    /// Atomic read/modify/write.
    Atomic,
    /// Memory fence.
    Fence,
    /// Workgroup or subgroup barrier.
    Barrier,
    /// Direct call.
    Call,
    /// Unconditional branch.
    Branch,
    /// Scalar-condition branch.
    BranchScc,
    /// Vector-condition branch.
    BranchVcc,
    /// Save EXEC and intersect it with a lane predicate.
    ExecAndSave,
    /// Save EXEC and intersect it with the inverse lane predicate.
    ExecAndNotSave,
    /// Restore EXEC from a saved mask.
    ExecRestore,
    /// Branch when EXEC is zero.
    BranchExecZero,
    /// Branch when EXEC is nonzero.
    BranchExecNonZero,
    /// Function return/end-program.
    Return,
    /// Invocation, workitem, workgroup, or lane index.
    InvocationIndex,
    /// Closed subgroup/workgroup collective.
    Collective,
    /// Closed matrix instruction.
    Matrix,
    /// Closed inline-assembly instruction.
    InlineAssembly,
    /// Explicit trap.
    Trap,
}

/// Exact effective-address expression decoded from one memory instruction.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Gfx942EffectiveAddressV1 {
    /// No effective address.
    None,
    /// `base + index * scale + displacement`, evaluated with the stated pointer width.
    BaseIndex {
        /// Base register.
        base: Gfx942SemanticRegisterV1,
        /// Optional index register.
        index: Option<Gfx942SemanticRegisterV1>,
        /// Index scale in bytes.
        scale: u32,
        /// Signed byte displacement.
        displacement: i64,
        /// LLVM/AMDGPU address-space number.
        address_space: u32,
        /// Pointer arithmetic width.
        pointer_bits: u16,
        /// Accessed byte width.
        byte_width: u32,
    },
}

/// Exact static gfx942 floating-point MODE facts for the decoded kernel.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Gfx942FloatingModeV1 {
    ieee: bool,
    fp32_denormals: bool,
    round_to_nearest_even: bool,
    dx10_clamp: bool,
}

impl Gfx942FloatingModeV1 {
    /// Creates one explicit MODE contract.
    pub const fn new(
        ieee: bool,
        fp32_denormals: bool,
        round_to_nearest_even: bool,
        dx10_clamp: bool,
    ) -> Self {
        Self {
            ieee,
            fp32_denormals,
            round_to_nearest_even,
            dx10_clamp,
        }
    }

    /// Strict fe2o3 binary32 mode.
    pub const fn strict_ieee_binary32() -> Self {
        Self::new(true, true, true, false)
    }

    const fn from_compute_pgm_rsrc1(word: u32) -> Self {
        Self::new(
            word & (1 << COMPUTE_PGM_RSRC1_ENABLE_IEEE_MODE_SHIFT) != 0,
            (word >> COMPUTE_PGM_RSRC1_FLOAT_DENORM_MODE_32_SHIFT) & COMPUTE_PGM_RSRC1_TWO_BIT_MASK
                == FLOAT_DENORM_MODE_FLUSH_NONE,
            (word >> COMPUTE_PGM_RSRC1_FLOAT_ROUND_MODE_32_SHIFT) & COMPUTE_PGM_RSRC1_TWO_BIT_MASK
                == FLOAT_ROUND_MODE_NEAR_EVEN,
            word & (1 << COMPUTE_PGM_RSRC1_ENABLE_DX10_CLAMP_SHIFT) != 0,
        )
    }

    /// Reports IEEE mode.
    pub const fn ieee(self) -> bool {
        self.ieee
    }

    /// Reports preserved binary32 subnormals.
    pub const fn fp32_denormals(self) -> bool {
        self.fp32_denormals
    }

    /// Reports round-to-nearest, ties-to-even.
    pub const fn round_to_nearest_even(self) -> bool {
        self.round_to_nearest_even
    }

    /// Reports DX10 NaN/zero clamping.
    pub const fn dx10_clamp(self) -> bool {
        self.dx10_clamp
    }

    const fn supports_strict_binary32(self) -> bool {
        self.ieee && self.fp32_denormals && self.round_to_nearest_even && !self.dx10_clamp
    }
}

/// One decoded machine block with an exact final-LLVM block attribution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942DecodedMachineBlockV1 {
    function: u32,
    machine_block: u32,
    llvm_block: u32,
    first_instruction: u64,
    instruction_count: u32,
    successors: Box<[u32]>,
}

impl Gfx942DecodedMachineBlockV1 {
    /// Creates one bounded block.
    pub fn new(
        function: u32,
        machine_block: u32,
        llvm_block: u32,
        first_instruction: u64,
        instruction_count: u32,
        successors: impl Into<Box<[u32]>>,
    ) -> Result<Self, Gfx942MachineRefinementErrorV1> {
        let successors = successors.into();
        if instruction_count == 0
            || successors.len() > MAX_GFX942_MACHINE_SUCCESSORS_V1
            || has_duplicate(successors.iter().copied())
        {
            return Err(Gfx942MachineRefinementErrorV1::InvalidMachineCfg);
        }
        Ok(Self {
            function,
            machine_block,
            llvm_block,
            first_instruction,
            instruction_count,
            successors,
        })
    }

    /// Returns the function ordinal.
    pub const fn function(&self) -> u32 {
        self.function
    }

    /// Returns the machine block ordinal.
    pub const fn machine_block(&self) -> u32 {
        self.machine_block
    }

    /// Returns the attributed final-LLVM block ordinal.
    pub const fn llvm_block(&self) -> u32 {
        self.llvm_block
    }

    /// Returns the first final-HSACO instruction offset.
    pub const fn first_instruction(&self) -> u64 {
        self.first_instruction
    }

    /// Returns decoded instruction count.
    pub const fn instruction_count(&self) -> u32 {
        self.instruction_count
    }

    /// Returns machine-block successors.
    pub fn successors(&self) -> &[u32] {
        &self.successors
    }
}

/// One instruction decoded independently from both object and final HSACO bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942DecodedInstructionV1 {
    function: u32,
    machine_block: u32,
    object_offset: u64,
    hsaco_offset: u64,
    native_opcode: u32,
    semantics: Gfx942SemanticOpcodeV1,
    semantic_discriminator: u32,
    object_encoding: Box<[u8]>,
    hsaco_encoding: Box<[u8]>,
    definitions: Box<[Gfx942SemanticRegisterV1]>,
    uses: Box<[Gfx942SemanticRegisterV1]>,
    branch_target: Option<u64>,
    memory: CompilerMemoryEffectV1,
    effective_address: Gfx942EffectiveAddressV1,
}

impl Gfx942DecodedInstructionV1 {
    /// Creates one bounded decoded instruction.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        function: u32,
        machine_block: u32,
        object_offset: u64,
        hsaco_offset: u64,
        native_opcode: u32,
        semantics: Gfx942SemanticOpcodeV1,
        semantic_discriminator: u32,
        object_encoding: impl Into<Box<[u8]>>,
        hsaco_encoding: impl Into<Box<[u8]>>,
        definitions: impl Into<Box<[Gfx942SemanticRegisterV1]>>,
        uses: impl Into<Box<[Gfx942SemanticRegisterV1]>>,
        branch_target: Option<u64>,
        memory: CompilerMemoryEffectV1,
        effective_address: Gfx942EffectiveAddressV1,
    ) -> Result<Self, Gfx942MachineRefinementErrorV1> {
        let instruction = Self {
            function,
            machine_block,
            object_offset,
            hsaco_offset,
            native_opcode,
            semantics,
            semantic_discriminator,
            object_encoding: object_encoding.into(),
            hsaco_encoding: hsaco_encoding.into(),
            definitions: definitions.into(),
            uses: uses.into(),
            branch_target,
            memory,
            effective_address,
        };
        validate_instruction_local(&instruction)?;
        Ok(instruction)
    }

    /// Returns the function ordinal.
    pub const fn function(&self) -> u32 {
        self.function
    }

    /// Returns the machine block ordinal.
    pub const fn machine_block(&self) -> u32 {
        self.machine_block
    }

    /// Returns the absolute generated-object byte offset.
    pub const fn object_offset(&self) -> u64 {
        self.object_offset
    }

    /// Returns the absolute final-HSACO byte offset.
    pub const fn hsaco_offset(&self) -> u64 {
        self.hsaco_offset
    }

    /// Returns the target decoder's closed native opcode number.
    pub const fn native_opcode(&self) -> u32 {
        self.native_opcode
    }

    /// Returns independently decoded semantics.
    pub const fn semantics(&self) -> Gfx942SemanticOpcodeV1 {
        self.semantics
    }

    /// Returns the target adapter's closed semantic subkind.
    pub const fn semantic_discriminator(&self) -> u32 {
        self.semantic_discriminator
    }

    /// Returns exact object instruction bytes.
    pub fn object_encoding(&self) -> &[u8] {
        &self.object_encoding
    }

    /// Returns exact final-HSACO instruction bytes.
    pub fn hsaco_encoding(&self) -> &[u8] {
        &self.hsaco_encoding
    }

    /// Returns register definitions.
    pub fn definitions(&self) -> &[Gfx942SemanticRegisterV1] {
        &self.definitions
    }

    /// Returns register uses.
    pub fn uses(&self) -> &[Gfx942SemanticRegisterV1] {
        &self.uses
    }

    /// Returns the final-HSACO direct branch target.
    pub const fn branch_target(&self) -> Option<u64> {
        self.branch_target
    }

    /// Returns the independently decoded memory effect.
    pub const fn memory(&self) -> CompilerMemoryEffectV1 {
        self.memory
    }

    /// Returns the decoded effective-address expression.
    pub const fn effective_address(&self) -> Gfx942EffectiveAddressV1 {
        self.effective_address
    }
}

/// Public canonical decoded-ISA transcript parts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942DecodedIsaTranscriptPartsV1 {
    /// Exact generated object identity.
    pub generated_object: ExactCompilerStageContentIdentityV1,
    /// Exact final HSACO identity.
    pub final_code_object: ExactCompilerStageContentIdentityV1,
    /// Static kernel MODE facts.
    pub mode: Gfx942FloatingModeV1,
    /// Complete machine-block roster.
    pub blocks: Box<[Gfx942DecodedMachineBlockV1]>,
    /// Complete decoded instruction roster for compiler-generated code.
    pub instructions: Box<[Gfx942DecodedInstructionV1]>,
}

/// Canonical decoded-ISA identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Gfx942DecodedIsaTranscriptIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl Gfx942DecodedIsaTranscriptIdentityV1 {
    /// Returns the domain-separated digest.
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    /// Returns canonical byte length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Canonical independently decoded gfx942 object/final-HSACO ISA facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942DecodedIsaTranscriptV1 {
    parts: Gfx942DecodedIsaTranscriptPartsV1,
    canonical: Box<[u8]>,
    identity: Gfx942DecodedIsaTranscriptIdentityV1,
}

impl Gfx942DecodedIsaTranscriptV1 {
    /// Constructs canonical decoded-ISA facts.
    pub fn from_parts(
        parts: Gfx942DecodedIsaTranscriptPartsV1,
    ) -> Result<Self, Gfx942MachineRefinementErrorV1> {
        validate_isa_parts(&parts)?;
        let canonical = encode_isa_parts(&parts)?;
        let identity = isa_identity(&canonical);
        Ok(Self {
            parts,
            canonical: canonical.into_boxed_slice(),
            identity,
        })
    }

    /// Independently decodes canonical typed facts.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, Gfx942MachineRefinementErrorV1> {
        if bytes.len() > MAX_GFX942_DECODED_ISA_TRANSCRIPT_BYTES_V1 {
            return Err(Gfx942MachineRefinementErrorV1::ResourceLimit);
        }
        let mut input = Input::new(bytes);
        if input.take(8)? != GFX942_DECODED_ISA_TRANSCRIPT_MAGIC_V1 {
            return Err(Gfx942MachineRefinementErrorV1::InvalidMagic);
        }
        if input.u16()? != GFX942_DECODED_ISA_TRANSCRIPT_VERSION_V1 {
            return Err(Gfx942MachineRefinementErrorV1::UnsupportedVersion);
        }
        let generated_object = input.content_identity()?;
        let final_code_object = input.content_identity()?;
        let mode = Gfx942FloatingModeV1::new(
            input.boolean()?,
            input.boolean()?,
            input.boolean()?,
            input.boolean()?,
        );
        let block_count = input.count(MAX_GFX942_DECODED_ISA_BLOCKS_V1)?;
        let mut blocks = Vec::with_capacity(block_count);
        for _ in 0..block_count {
            let function = input.u32()?;
            let machine_block = input.u32()?;
            let llvm_block = input.u32()?;
            let first_instruction = input.u64()?;
            let instruction_count = input.u32()?;
            let successor_count = input.count(MAX_GFX942_MACHINE_SUCCESSORS_V1)?;
            let mut successors = Vec::with_capacity(successor_count);
            for _ in 0..successor_count {
                successors.push(input.u32()?);
            }
            blocks.push(Gfx942DecodedMachineBlockV1::new(
                function,
                machine_block,
                llvm_block,
                first_instruction,
                instruction_count,
                successors,
            )?);
        }
        let instruction_count = input.count(MAX_GFX942_DECODED_ISA_INSTRUCTIONS_V1)?;
        let mut instructions = Vec::with_capacity(instruction_count);
        for _ in 0..instruction_count {
            instructions.push(decode_instruction(&mut input)?);
        }
        input.finish()?;
        let decoded = Self::from_parts(Gfx942DecodedIsaTranscriptPartsV1 {
            generated_object,
            final_code_object,
            mode,
            blocks: blocks.into_boxed_slice(),
            instructions: instructions.into_boxed_slice(),
        })?;
        if decoded.canonical.as_ref() != bytes {
            return Err(Gfx942MachineRefinementErrorV1::NonCanonical);
        }
        Ok(decoded)
    }

    /// Returns exact object identity.
    pub const fn generated_object(&self) -> ExactCompilerStageContentIdentityV1 {
        self.parts.generated_object
    }

    /// Returns exact final-HSACO identity.
    pub const fn final_code_object(&self) -> ExactCompilerStageContentIdentityV1 {
        self.parts.final_code_object
    }

    /// Returns static MODE facts.
    pub const fn mode(&self) -> Gfx942FloatingModeV1 {
        self.parts.mode
    }

    /// Returns complete machine blocks.
    pub fn blocks(&self) -> &[Gfx942DecodedMachineBlockV1] {
        &self.parts.blocks
    }

    /// Returns complete decoded instructions.
    pub fn instructions(&self) -> &[Gfx942DecodedInstructionV1] {
        &self.parts.instructions
    }

    /// Returns canonical bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }

    /// Returns canonical identity.
    pub const fn identity(&self) -> Gfx942DecodedIsaTranscriptIdentityV1 {
        self.identity
    }

    /// Typed decode facts do not establish compiler correspondence by themselves.
    pub const fn proves_compiler_refinement(&self) -> bool {
        false
    }

    /// Decomposes the transcript for hostile tests.
    pub fn into_parts(self) -> Gfx942DecodedIsaTranscriptPartsV1 {
        self.parts
    }
}

/// Identity of the complete independently checked gfx942 machine-refinement input.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Gfx942MachineRefinementIdentityV1 {
    sha256: [u8; 32],
}

impl Gfx942MachineRefinementIdentityV1 {
    /// Returns the domain-separated digest.
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }
}

/// Identity of the complete independently checked gfx950 machine-refinement input.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Gfx950MachineRefinementIdentityV1 {
    sha256: [u8; 32],
}

impl Gfx950MachineRefinementIdentityV1 {
    /// Returns the gfx950-domain-separated digest.
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }
}

/// Target-discriminated identity of one independently checked AMD refinement owner.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AmdMachineRefinementIdentityV1 {
    /// Exact gfx942 owner identity.
    Gfx942(Gfx942MachineRefinementIdentityV1),
    /// Exact gfx950 owner identity.
    Gfx950(Gfx950MachineRefinementIdentityV1),
}

impl AmdMachineRefinementIdentityV1 {
    /// Returns the target-domain-separated digest.
    pub const fn sha256(self) -> [u8; 32] {
        match self {
            Self::Gfx942(identity) => identity.sha256(),
            Self::Gfx950(identity) => identity.sha256(),
        }
    }
}

/// Move-only conjunction of the implemented closed-subset machine-refinement premises.
#[must_use = "dropping checked gfx942 refinement abandons a required machine-refinement input"]
pub struct CheckedGfx942MachineRefinementV1 {
    preservation: CheckedGfx942ObjectToHsacoPreservationV1,
    compiler: CheckedCompilerInstructionSelectionCorrespondenceV1,
    isa: Gfx942DecodedIsaTranscriptV1,
    identity: Gfx942MachineRefinementIdentityV1,
    llvm_backedges: u32,
    required_families: u64,
    established_families: u64,
}

impl fmt::Debug for CheckedGfx942MachineRefinementV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CheckedGfx942MachineRefinementV1")
            .field("identity", &self.identity)
            .field("llvm_backedges", &self.llvm_backedges)
            .field("required_families", &self.required_families)
            .field("established_families", &self.established_families)
            .finish_non_exhaustive()
    }
}

impl CheckedGfx942MachineRefinementV1 {
    /// Returns the checked identity.
    pub const fn identity(&self) -> Gfx942MachineRefinementIdentityV1 {
        self.identity
    }

    /// Returns the compiler correspondence premise.
    pub const fn compiler(&self) -> &CheckedCompilerInstructionSelectionCorrespondenceV1 {
        &self.compiler
    }

    /// Returns independent decoded ISA facts.
    pub const fn isa(&self) -> &Gfx942DecodedIsaTranscriptV1 {
        &self.isa
    }

    /// Returns dominance-qualified final-LLVM backedge count.
    pub const fn llvm_backedges(&self) -> u32 {
        self.llvm_backedges
    }

    /// Returns the machine-family set required by the exact compiler correspondence.
    pub const fn required_families(&self) -> u64 {
        self.required_families
    }

    /// Returns the machine-family set independently established by exact-byte decoding.
    pub const fn established_families(&self) -> u64 {
        self.established_families
    }

    /// Produces exact coordinates for the typed #214 receipt checked by the sealed verifier.
    pub fn target_machine_refinement_receipt_parts_v1(
        &self,
    ) -> Result<TargetMachineRefinementReceiptPartsV1, TargetMachineRefinementReceiptErrorV1> {
        let isa = self.isa.identity();
        Ok(TargetMachineRefinementReceiptPartsV1 {
            target: TargetMachineRefinementTargetV1::Gfx942,
            post_llvm_custody: self.preservation.contents().record().identity(),
            instruction_selection: self.compiler.correspondence().identity(),
            decoded_isa: MachineRefinementContentIdentityV1::new(isa.sha256(), isa.byte_len())?,
            final_code_object: self.isa.final_code_object(),
            machine_refinement_sha256: self.identity.sha256(),
            required_families: self.required_families,
            established_families: self.established_families,
        })
    }

    /// Reports exact object/HSACO replay and section/symbol/relocation roster preservation.
    pub const fn retains_object_to_hsaco_preservation(&self) -> bool {
        self.preservation
            .retains_object_to_hsaco_structural_and_replay_premises()
    }

    /// Returns the explicit support boundary; unavailable rows prohibit a production claim.
    pub const fn component_status(&self) -> &'static [AmdMachineRefinementComponentStatusV1] {
        amd_machine_refinement_component_status_v1()
    }

    /// This proof input grants no publication, load, or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }

    /// Decomposes all move-only premises.
    pub fn into_parts(
        self,
    ) -> (
        CheckedGfx942ObjectToHsacoPreservationV1,
        CheckedCompilerInstructionSelectionCorrespondenceV1,
        Gfx942DecodedIsaTranscriptV1,
    ) {
        (self.preservation, self.compiler, self.isa)
    }
}

/// Move-only conjunction of independently checked gfx950 common-subset premises.
///
/// The retained ISA transcript uses the versioned common gfx9 wire grammar, but every executable
/// byte is re-decoded through the authenticated gfx950 target branch before this owner exists.
/// Target-specific scaled MFMA and transpose forms remain unavailable and fail construction.
#[must_use = "dropping checked gfx950 refinement abandons a required machine-refinement input"]
pub struct CheckedGfx950MachineRefinementV1 {
    preservation: CheckedGfx950ObjectToHsacoPreservationV1,
    compiler: CheckedCompilerInstructionSelectionCorrespondenceV1,
    isa: Gfx942DecodedIsaTranscriptV1,
    identity: Gfx950MachineRefinementIdentityV1,
    llvm_backedges: u32,
    required_families: u64,
    established_families: u64,
}

impl fmt::Debug for CheckedGfx950MachineRefinementV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CheckedGfx950MachineRefinementV1")
            .field("identity", &self.identity)
            .field("llvm_backedges", &self.llvm_backedges)
            .field("required_families", &self.required_families)
            .field("established_families", &self.established_families)
            .finish_non_exhaustive()
    }
}

impl CheckedGfx950MachineRefinementV1 {
    /// Returns the checked gfx950 identity.
    pub const fn identity(&self) -> Gfx950MachineRefinementIdentityV1 {
        self.identity
    }

    /// Returns the compiler correspondence premise.
    pub const fn compiler(&self) -> &CheckedCompilerInstructionSelectionCorrespondenceV1 {
        &self.compiler
    }

    /// Returns independently decoded ISA facts.
    pub const fn isa(&self) -> &Gfx942DecodedIsaTranscriptV1 {
        &self.isa
    }

    /// Returns dominance-qualified final-LLVM backedge count.
    pub const fn llvm_backedges(&self) -> u32 {
        self.llvm_backedges
    }

    /// Returns the machine-family set required by the exact compiler correspondence.
    pub const fn required_families(&self) -> u64 {
        self.required_families
    }

    /// Returns the machine-family set independently established by exact-byte decoding.
    pub const fn established_families(&self) -> u64 {
        self.established_families
    }

    /// Produces exact gfx950 coordinates for the typed #214 receipt.
    pub fn target_machine_refinement_receipt_parts_v1(
        &self,
    ) -> Result<TargetMachineRefinementReceiptPartsV1, TargetMachineRefinementReceiptErrorV1> {
        let isa = self.isa.identity();
        Ok(TargetMachineRefinementReceiptPartsV1 {
            target: TargetMachineRefinementTargetV1::Gfx950,
            post_llvm_custody: self.preservation.contents().record().identity(),
            instruction_selection: self.compiler.correspondence().identity(),
            decoded_isa: MachineRefinementContentIdentityV1::new(isa.sha256(), isa.byte_len())?,
            final_code_object: self.isa.final_code_object(),
            machine_refinement_sha256: self.identity.sha256(),
            required_families: self.required_families,
            established_families: self.established_families,
        })
    }

    /// Reports exact replay and structural section/symbol/relocation custody.
    pub const fn retains_object_to_hsaco_preservation(&self) -> bool {
        self.preservation
            .retains_object_to_hsaco_structural_and_replay_premises()
    }

    /// Returns the explicit per-family support boundary.
    pub const fn component_status(&self) -> &'static [AmdMachineRefinementComponentStatusV1] {
        amd_machine_refinement_component_status_v1()
    }

    /// This proof input grants no publication, load, or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }

    /// Decomposes all move-only gfx950 premises.
    pub fn into_parts(
        self,
    ) -> (
        CheckedGfx950ObjectToHsacoPreservationV1,
        CheckedCompilerInstructionSelectionCorrespondenceV1,
        Gfx942DecodedIsaTranscriptV1,
    ) {
        (self.preservation, self.compiler, self.isa)
    }
}

/// Move-only target-selected AMD machine-refinement owner.
#[must_use = "dropping checked AMD refinement abandons a required publication input"]
pub enum CheckedAmdMachineRefinementV1 {
    /// Exact gfx942 refinement owner.
    Gfx942(CheckedGfx942MachineRefinementV1),
    /// Exact gfx950 refinement owner.
    Gfx950(CheckedGfx950MachineRefinementV1),
}

impl fmt::Debug for CheckedAmdMachineRefinementV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gfx942(value) => formatter.debug_tuple("Gfx942").field(value).finish(),
            Self::Gfx950(value) => formatter.debug_tuple("Gfx950").field(value).finish(),
        }
    }
}

impl CheckedAmdMachineRefinementV1 {
    /// Returns the authenticated target selected by construction.
    pub const fn target(&self) -> AmdMachineRefinementTargetV1 {
        match self {
            Self::Gfx942(_) => AmdMachineRefinementTargetV1::Gfx942,
            Self::Gfx950(_) => AmdMachineRefinementTargetV1::Gfx950,
        }
    }

    /// Returns the target-discriminated refinement identity.
    pub const fn identity(&self) -> AmdMachineRefinementIdentityV1 {
        match self {
            Self::Gfx942(value) => AmdMachineRefinementIdentityV1::Gfx942(value.identity()),
            Self::Gfx950(value) => AmdMachineRefinementIdentityV1::Gfx950(value.identity()),
        }
    }

    /// Returns the exact final code-object coordinate.
    pub const fn final_code_object(&self) -> ExactCompilerStageContentIdentityV1 {
        match self {
            Self::Gfx942(value) => value.isa().final_code_object(),
            Self::Gfx950(value) => value.isa().final_code_object(),
        }
    }

    /// Produces the exact target-specific #214 receipt coordinates.
    pub fn target_machine_refinement_receipt_parts_v1(
        &self,
    ) -> Result<TargetMachineRefinementReceiptPartsV1, TargetMachineRefinementReceiptErrorV1> {
        match self {
            Self::Gfx942(value) => value.target_machine_refinement_receipt_parts_v1(),
            Self::Gfx950(value) => value.target_machine_refinement_receipt_parts_v1(),
        }
    }

    /// Returns the machine-family set required by the exact compiler correspondence.
    pub const fn required_families(&self) -> u64 {
        match self {
            Self::Gfx942(value) => value.required_families(),
            Self::Gfx950(value) => value.required_families(),
        }
    }

    /// Returns the machine-family set independently established by exact-byte decoding.
    pub const fn established_families(&self) -> u64 {
        match self {
            Self::Gfx942(value) => value.established_families(),
            Self::Gfx950(value) => value.established_families(),
        }
    }

    /// This proof input grants no publication, load, or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

impl From<CheckedGfx942MachineRefinementV1> for CheckedAmdMachineRefinementV1 {
    fn from(value: CheckedGfx942MachineRefinementV1) -> Self {
        Self::Gfx942(value)
    }
}

impl From<CheckedGfx950MachineRefinementV1> for CheckedAmdMachineRefinementV1 {
    fn from(value: CheckedGfx950MachineRefinementV1) -> Self {
        Self::Gfx950(value)
    }
}

const UNSUPPORTED_MACHINE_FAMILY_V1: u64 = 1_u64 << 63;

fn required_machine_families(compiler: &CompilerInstructionSelectionCorrespondenceV1) -> u64 {
    let mut required = MachineRefinementFamilyV1::ObjectToHsaco.bit();
    for operation in compiler.operations() {
        required |= required_operation_families(operation);
    }
    required
}

fn required_operation_families(operation: &CompilerLlvmOperationV1) -> u64 {
    use CompilerLlvmOperationKindV1 as Operation;

    let mut required = match operation.kind() {
        Operation::Load | Operation::Store => memory_space_family(operation.memory()),
        Operation::Atomic => {
            MachineRefinementFamilyV1::Atomic.bit() | memory_space_family(operation.memory())
        }
        Operation::Fence | Operation::Barrier => MachineRefinementFamilyV1::Barrier.bit(),
        Operation::Branch
        | Operation::Switch
        | Operation::Return
        | Operation::Unreachable
        | Operation::Trap
        | Operation::Call => MachineRefinementFamilyV1::ControlFlow.bit(),
        Operation::ConditionalBranch => {
            MachineRefinementFamilyV1::ControlFlow.bit()
                | if operation.divergence() == CompilerBranchDivergenceV1::Divergent {
                    MachineRefinementFamilyV1::ExecutionMask.bit()
                } else {
                    0
                }
        }
        Operation::Collective => MachineRefinementFamilyV1::Dpp.bit(),
        Operation::Matrix => MachineRefinementFamilyV1::Matrix.bit(),
        Operation::Intrinsic => match operation.semantic_discriminator() {
            8 => MachineRefinementFamilyV1::Barrier.bit(),
            12 => MachineRefinementFamilyV1::Dpp.bit(),
            13 => MachineRefinementFamilyV1::Matrix.bit(),
            _ => register_families(operation),
        },
        Operation::InlineAssembly => UNSUPPORTED_MACHINE_FAMILY_V1,
        _ => register_families(operation),
    };
    required |= match operation.numerical() {
        CompilerNumericalContractV1::IeeeBinary32
        | CompilerNumericalContractV1::IeeeBinary32Fused => {
            MachineRefinementFamilyV1::FloatingMode.bit()
        }
        CompilerNumericalContractV1::IeeeBinary64 => UNSUPPORTED_MACHINE_FAMILY_V1,
        CompilerNumericalContractV1::MfmaBf16AccumulateF32
        | CompilerNumericalContractV1::MfmaFp8E4M3AccumulateF32
        | CompilerNumericalContractV1::MfmaFp8E5M2AccumulateF32
        | CompilerNumericalContractV1::MfmaFp4E2M1AccumulateF32
        | CompilerNumericalContractV1::MfmaFp4E2M1ByFp8E4M3AccumulateF32 => {
            MachineRefinementFamilyV1::Matrix.bit() | MachineRefinementFamilyV1::FloatingMode.bit()
        }
        CompilerNumericalContractV1::TargetDefined => UNSUPPORTED_MACHINE_FAMILY_V1,
        CompilerNumericalContractV1::Exact => 0,
    };
    required
}

fn register_families(operation: &CompilerLlvmOperationV1) -> u64 {
    operation
        .machine_value_bindings()
        .iter()
        .fold(0, |families, binding| {
            families
                | match binding.location() {
                    CompilerMachineValueLocationV1::Immediate { .. } => 0,
                    CompilerMachineValueLocationV1::Register(register) => {
                        register_class_family(register.class())
                    }
                }
        })
}

const fn register_class_family(class: CompilerMachineRegisterClassV1) -> u64 {
    match class {
        CompilerMachineRegisterClassV1::Scalar
        | CompilerMachineRegisterClassV1::ScalarCondition
        | CompilerMachineRegisterClassV1::MemoryOffset => {
            MachineRefinementFamilyV1::ScalarAlu.bit()
        }
        CompilerMachineRegisterClassV1::Vector
        | CompilerMachineRegisterClassV1::VectorCondition => {
            MachineRefinementFamilyV1::VectorAlu.bit()
        }
        CompilerMachineRegisterClassV1::Accumulator => MachineRefinementFamilyV1::Matrix.bit(),
        CompilerMachineRegisterClassV1::ExecutionMask => {
            MachineRefinementFamilyV1::ExecutionMask.bit()
        }
        CompilerMachineRegisterClassV1::FloatingMode => {
            MachineRefinementFamilyV1::FloatingMode.bit()
        }
    }
}

const fn memory_space_family(memory: CompilerMemoryEffectV1) -> u64 {
    match memory.address_space() {
        1 => MachineRefinementFamilyV1::GlobalMemory.bit(),
        3 => MachineRefinementFamilyV1::LocalMemory.bit(),
        _ => UNSUPPORTED_MACHINE_FAMILY_V1,
    }
}

fn established_machine_families(
    isa: &Gfx942DecodedIsaTranscriptV1,
    retains_object_to_hsaco_preservation: bool,
) -> u64 {
    let mut established = 0;
    if retains_object_to_hsaco_preservation {
        established |= MachineRefinementFamilyV1::ObjectToHsaco.bit();
    }
    if isa.mode().supports_strict_binary32() {
        established |= MachineRefinementFamilyV1::FloatingMode.bit();
    }
    for instruction in isa.instructions() {
        established |= established_instruction_families(instruction);
    }
    established
}

fn established_instruction_families(instruction: &Gfx942DecodedInstructionV1) -> u64 {
    use Gfx942SemanticOpcodeV1 as Opcode;

    let register_families = instruction
        .definitions()
        .iter()
        .chain(instruction.uses())
        .fold(0, |families, register| {
            families
                | match register {
                    Gfx942SemanticRegisterV1::Scalar(_)
                    | Gfx942SemanticRegisterV1::Scc
                    | Gfx942SemanticRegisterV1::M0 => MachineRefinementFamilyV1::ScalarAlu.bit(),
                    Gfx942SemanticRegisterV1::Vector(_) | Gfx942SemanticRegisterV1::Vcc => {
                        MachineRefinementFamilyV1::VectorAlu.bit()
                    }
                    Gfx942SemanticRegisterV1::Accumulator(_) => 0,
                    Gfx942SemanticRegisterV1::Exec => {
                        MachineRefinementFamilyV1::ExecutionMask.bit()
                    }
                    Gfx942SemanticRegisterV1::Mode => MachineRefinementFamilyV1::FloatingMode.bit(),
                }
        });
    match instruction.semantics() {
        Opcode::Copy
        | Opcode::IntegerAdd
        | Opcode::IntegerSubtract
        | Opcode::IntegerMultiply
        | Opcode::IntegerDivide
        | Opcode::IntegerRemainder
        | Opcode::IntegerBitwise
        | Opcode::IntegerShift
        | Opcode::IntegerCompare
        | Opcode::FloatCompare32
        | Opcode::Cast
        | Opcode::Select
        | Opcode::Address
        | Opcode::InvocationIndex => register_families,
        Opcode::FloatAdd32
        | Opcode::FloatSubtract32
        | Opcode::FloatMultiply32
        | Opcode::FloatDivide32
        | Opcode::FloatFma32
        | Opcode::FloatSqrt32 => MachineRefinementFamilyV1::VectorAlu.bit(),
        Opcode::Load | Opcode::Store => memory_space_family(instruction.memory()),
        Opcode::Atomic => {
            MachineRefinementFamilyV1::Atomic.bit() | memory_space_family(instruction.memory())
        }
        Opcode::Fence | Opcode::Barrier => MachineRefinementFamilyV1::Barrier.bit(),
        Opcode::Branch
        | Opcode::BranchScc
        | Opcode::BranchVcc
        | Opcode::Return
        | Opcode::Call
        | Opcode::Trap => MachineRefinementFamilyV1::ControlFlow.bit(),
        Opcode::ExecAndSave | Opcode::ExecAndNotSave | Opcode::ExecRestore => {
            MachineRefinementFamilyV1::ExecutionMask.bit()
        }
        Opcode::BranchExecZero | Opcode::BranchExecNonZero => {
            MachineRefinementFamilyV1::ControlFlow.bit()
                | MachineRefinementFamilyV1::ExecutionMask.bit()
        }
        Opcode::Collective if dpp_discriminator_admitted(instruction.semantic_discriminator()) => {
            MachineRefinementFamilyV1::Dpp.bit() | register_families
        }
        Opcode::Matrix if matrix_discriminator_admitted(instruction.semantic_discriminator()) => {
            MachineRefinementFamilyV1::Matrix.bit() | MachineRefinementFamilyV1::FloatingMode.bit()
        }
        Opcode::Collective | Opcode::Matrix | Opcode::InlineAssembly => 0,
    }
}

/// Checks compiler correspondence against the independently decoded gfx942 recurrence subset.
pub fn check_gfx942_machine_refinement_v1(
    preservation: CheckedGfx942ObjectToHsacoPreservationV1,
    compiler: CheckedCompilerInstructionSelectionCorrespondenceV1,
    decoded_isa_bytes: &[u8],
) -> Result<CheckedGfx942MachineRefinementV1, Gfx942MachineRefinementErrorV1> {
    let isa = Gfx942DecodedIsaTranscriptV1::decode_canonical(decoded_isa_bytes)?;
    let stages = preservation.contents();
    if compiler.correspondence().architecture() != MachineRefinementArchitectureV1::AmdGcn {
        return Err(Gfx942MachineRefinementErrorV1::TargetMismatch);
    }
    for (identity, bytes, mismatch) in [
        (
            isa.generated_object(),
            stages.generated_object(),
            Gfx942MachineRefinementErrorV1::GeneratedObjectMismatch,
        ),
        (
            isa.final_code_object(),
            stages.final_code_object(),
            Gfx942MachineRefinementErrorV1::FinalCodeObjectMismatch,
        ),
    ] {
        if !identity.matches(bytes) {
            return Err(mismatch);
        }
    }
    if isa.generated_object() != compiler.correspondence().generated_object()
        || isa.final_code_object() != compiler.correspondence().final_code_object()
    {
        return Err(Gfx942MachineRefinementErrorV1::CompilerIsaArtifactMismatch);
    }
    let mode_words = crate::post_llvm_stage_custody_v1::decode_gfx942_hsaco_kernel_mode_words_v1(
        stages.final_code_object(),
    )
    .map_err(|()| Gfx942MachineRefinementErrorV1::KernelModeUnavailable)?;
    let mut decoded_modes = mode_words
        .iter()
        .copied()
        .map(Gfx942FloatingModeV1::from_compute_pgm_rsrc1);
    let hsaco_mode = decoded_modes
        .next()
        .ok_or(Gfx942MachineRefinementErrorV1::KernelModeUnavailable)?;
    if decoded_modes.any(|mode| mode != hsaco_mode) || isa.mode() != hsaco_mode {
        return Err(Gfx942MachineRefinementErrorV1::ModeOrIeeeMismatch);
    }
    validate_instruction_bytes(
        AmdMachineRefinementTargetV1::Gfx942,
        stages.generated_object(),
        stages.final_code_object(),
        isa.instructions(),
    )?;
    let llvm_backedges = validate_correspondence(compiler.correspondence(), &isa)?;
    let required_families = required_machine_families(compiler.correspondence());
    let established_families = established_machine_families(
        &isa,
        preservation.retains_object_to_hsaco_structural_and_replay_premises(),
    );
    if required_families == 0 || required_families & !established_families != 0 {
        return Err(Gfx942MachineRefinementErrorV1::UnsupportedMachineFamily);
    }
    let identity = refinement_identity(compiler.correspondence().identity(), isa.identity());
    Ok(CheckedGfx942MachineRefinementV1 {
        preservation,
        compiler,
        isa,
        identity,
        llvm_backedges,
        required_families,
        established_families,
    })
}

/// Checks compiler correspondence against exact gfx950 common-subset machine bytes.
///
/// The target is established by the gfx950 stage owner, and every executable byte is decoded
/// through the gfx950 branch. Any target-specific or semantically unsupported family fails closed.
pub fn check_gfx950_machine_refinement_v1(
    preservation: CheckedGfx950ObjectToHsacoPreservationV1,
    compiler: CheckedCompilerInstructionSelectionCorrespondenceV1,
    decoded_isa_bytes: &[u8],
) -> Result<CheckedGfx950MachineRefinementV1, Gfx942MachineRefinementErrorV1> {
    let isa = Gfx942DecodedIsaTranscriptV1::decode_canonical(decoded_isa_bytes)?;
    let stages = preservation.contents();
    if compiler.correspondence().architecture() != MachineRefinementArchitectureV1::AmdGcn {
        return Err(Gfx942MachineRefinementErrorV1::TargetMismatch);
    }
    for (identity, bytes, mismatch) in [
        (
            isa.generated_object(),
            stages.generated_object(),
            Gfx942MachineRefinementErrorV1::GeneratedObjectMismatch,
        ),
        (
            isa.final_code_object(),
            stages.final_code_object(),
            Gfx942MachineRefinementErrorV1::FinalCodeObjectMismatch,
        ),
    ] {
        if !identity.matches(bytes) {
            return Err(mismatch);
        }
    }
    if isa.generated_object() != compiler.correspondence().generated_object()
        || isa.final_code_object() != compiler.correspondence().final_code_object()
    {
        return Err(Gfx942MachineRefinementErrorV1::CompilerIsaArtifactMismatch);
    }
    let mode_words = crate::post_llvm_stage_custody_v1::decode_gfx950_hsaco_kernel_mode_words_v1(
        stages.final_code_object(),
    )
    .map_err(|()| Gfx942MachineRefinementErrorV1::KernelModeUnavailable)?;
    let mut decoded_modes = mode_words
        .iter()
        .copied()
        .map(Gfx942FloatingModeV1::from_compute_pgm_rsrc1);
    let hsaco_mode = decoded_modes
        .next()
        .ok_or(Gfx942MachineRefinementErrorV1::KernelModeUnavailable)?;
    if decoded_modes.any(|mode| mode != hsaco_mode) || isa.mode() != hsaco_mode {
        return Err(Gfx942MachineRefinementErrorV1::ModeOrIeeeMismatch);
    }
    validate_instruction_bytes(
        AmdMachineRefinementTargetV1::Gfx950,
        stages.generated_object(),
        stages.final_code_object(),
        isa.instructions(),
    )?;
    let llvm_backedges = validate_correspondence(compiler.correspondence(), &isa)?;
    let required_families = required_machine_families(compiler.correspondence());
    let established_families = established_machine_families(
        &isa,
        preservation.retains_object_to_hsaco_structural_and_replay_premises(),
    );
    if required_families == 0 || required_families & !established_families != 0 {
        return Err(Gfx942MachineRefinementErrorV1::UnsupportedMachineFamily);
    }
    let identity = gfx950_refinement_identity(compiler.correspondence().identity(), isa.identity());
    Ok(CheckedGfx950MachineRefinementV1 {
        preservation,
        compiler,
        isa,
        identity,
        llvm_backedges,
        required_families,
        established_families,
    })
}

/// Closed failures for independent gfx942 machine refinement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942MachineRefinementErrorV1 {
    /// Canonical input was truncated.
    Truncated,
    /// Canonical magic differed.
    InvalidMagic,
    /// Canonical version is unsupported.
    UnsupportedVersion,
    /// A closed tag was unknown.
    UnknownTag,
    /// A count or body exceeded its bound.
    ResourceLimit,
    /// Canonical ordering or uniqueness failed.
    NonCanonical,
    /// Generated-object identity or bytes differed.
    GeneratedObjectMismatch,
    /// Final-HSACO identity or bytes differed.
    FinalCodeObjectMismatch,
    /// Compiler and independent ISA evidence name different artifacts.
    CompilerIsaArtifactMismatch,
    /// The compiler architecture is not AMD GCN.
    TargetMismatch,
    /// Machine CFG, branch target, or reachability is invalid.
    InvalidMachineCfg,
    /// LLVM and machine branch/backedge structures differ.
    ControlFlowMismatch,
    /// EXEC save/update/branch/restore semantics are incomplete.
    ExecSemanticsMismatch,
    /// Register def/use facts are inconsistent.
    RegisterSemanticsMismatch,
    /// Effective-address expression or memory shape differs.
    AddressOrMemoryMismatch,
    /// Barrier/fence/atomic ordering or scope differs.
    SynchronizationMismatch,
    /// MODE cannot implement the required IEEE behavior.
    ModeOrIeeeMismatch,
    /// The final HSACO has no unambiguous kernel-descriptor MODE value.
    KernelModeUnavailable,
    /// Exact object or HSACO executable sections cannot be completely enumerated.
    ExecutableTextUnavailable,
    /// The target-defined numerical contract has no admitted operational model.
    UnsupportedNumericalContract,
    /// Compiler semantic subkind is not admitted by the target adapter.
    UnsupportedSemanticDiscriminator,
    /// Compiler operation and decoded opcode classes differ.
    OperationSemanticsMismatch,
    /// Exact bytes do not belong to the closed gfx942 binary subset decoded by this adapter.
    UnsupportedMachineOpcode,
    /// Caller-supplied instruction facts differ from the facts decoded from exact bytes.
    InstructionDecodeMismatch,
    /// Compiler mapping omitted or duplicated a decoded instruction.
    IncompleteInstructionMapping,
    /// A mapped LLVM result or operand has no complete decoded machine location.
    IncompleteValueBinding,
    /// The closed decoder cannot validate this machine value location representation.
    UnsupportedMachineValueLocation,
    /// A required instruction family has byte custody but no admitted semantic checker.
    UnsupportedMachineFamily,
}

impl fmt::Display for Gfx942MachineRefinementErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "gfx942 machine refinement rejected: {self:?}")
    }
}

impl std::error::Error for Gfx942MachineRefinementErrorV1 {}

fn validate_isa_parts(
    parts: &Gfx942DecodedIsaTranscriptPartsV1,
) -> Result<(), Gfx942MachineRefinementErrorV1> {
    if parts.blocks.is_empty()
        || parts.blocks.len() > MAX_GFX942_DECODED_ISA_BLOCKS_V1
        || parts.instructions.is_empty()
        || parts.instructions.len() > MAX_GFX942_DECODED_ISA_INSTRUCTIONS_V1
        || !parts.blocks.windows(2).all(|pair| {
            (pair[0].function, pair[0].machine_block) < (pair[1].function, pair[1].machine_block)
        })
        || !parts.instructions.windows(2).all(|pair| {
            (pair[0].function, pair[0].hsaco_offset) < (pair[1].function, pair[1].hsaco_offset)
        })
    {
        return Err(Gfx942MachineRefinementErrorV1::NonCanonical);
    }
    let block_map = parts
        .blocks
        .iter()
        .map(|block| ((block.function, block.machine_block), block))
        .collect::<BTreeMap<_, _>>();
    if block_map.len() != parts.blocks.len() {
        return Err(Gfx942MachineRefinementErrorV1::NonCanonical);
    }
    for function in parts
        .blocks
        .iter()
        .map(|block| block.function)
        .collect::<BTreeSet<_>>()
    {
        let blocks = parts
            .blocks
            .iter()
            .filter(|block| block.function == function)
            .collect::<Vec<_>>();
        if blocks
            .iter()
            .map(|block| block.machine_block)
            .ne(0..blocks.len() as u32)
            || !blocks
                .windows(2)
                .all(|pair| pair[0].first_instruction < pair[1].first_instruction)
        {
            return Err(Gfx942MachineRefinementErrorV1::InvalidMachineCfg);
        }
        let mut reached = BTreeSet::new();
        let mut pending = vec![0_u32];
        while let Some(block) = pending.pop() {
            if !reached.insert(block) {
                continue;
            }
            let record = block_map
                .get(&(function, block))
                .ok_or(Gfx942MachineRefinementErrorV1::InvalidMachineCfg)?;
            for successor in &record.successors {
                if !block_map.contains_key(&(function, *successor)) {
                    return Err(Gfx942MachineRefinementErrorV1::InvalidMachineCfg);
                }
                pending.push(*successor);
            }
        }
        if reached.len() != blocks.len() {
            return Err(Gfx942MachineRefinementErrorV1::InvalidMachineCfg);
        }
    }
    for instruction in &parts.instructions {
        validate_instruction_local(instruction)?;
        if !block_map.contains_key(&(instruction.function, instruction.machine_block)) {
            return Err(Gfx942MachineRefinementErrorV1::InvalidMachineCfg);
        }
    }
    for block in &parts.blocks {
        let instructions = parts
            .instructions
            .iter()
            .filter(|instruction| {
                instruction.function == block.function
                    && instruction.machine_block == block.machine_block
            })
            .collect::<Vec<_>>();
        if instructions.len() != block.instruction_count as usize
            || instructions
                .first()
                .map(|instruction| instruction.hsaco_offset)
                != Some(block.first_instruction)
        {
            return Err(Gfx942MachineRefinementErrorV1::InvalidMachineCfg);
        }
        if instructions.windows(2).any(|pair| {
            pair[0]
                .hsaco_offset
                .checked_add(pair[0].hsaco_encoding.len() as u64)
                != Some(pair[1].hsaco_offset)
                || is_machine_terminator(pair[0].semantics)
        }) {
            return Err(Gfx942MachineRefinementErrorV1::InvalidMachineCfg);
        }
        let last = instructions
            .last()
            .ok_or(Gfx942MachineRefinementErrorV1::InvalidMachineCfg)?;
        let next = parts.blocks.iter().find(|candidate| {
            candidate.function == block.function
                && candidate.machine_block == block.machine_block.saturating_add(1)
        });
        let target_block = |target| {
            parts.blocks.iter().find(|candidate| {
                candidate.function == block.function && candidate.first_instruction == target
            })
        };
        let mut derived_successors = BTreeSet::new();
        match last.semantics {
            Gfx942SemanticOpcodeV1::Return | Gfx942SemanticOpcodeV1::Trap => {}
            Gfx942SemanticOpcodeV1::Branch => {
                let target = last
                    .branch_target
                    .and_then(target_block)
                    .ok_or(Gfx942MachineRefinementErrorV1::InvalidMachineCfg)?;
                derived_successors.insert(target.machine_block);
            }
            Gfx942SemanticOpcodeV1::BranchScc
            | Gfx942SemanticOpcodeV1::BranchVcc
            | Gfx942SemanticOpcodeV1::BranchExecZero
            | Gfx942SemanticOpcodeV1::BranchExecNonZero => {
                let target = last
                    .branch_target
                    .and_then(target_block)
                    .ok_or(Gfx942MachineRefinementErrorV1::InvalidMachineCfg)?;
                let fallthrough = next.ok_or(Gfx942MachineRefinementErrorV1::InvalidMachineCfg)?;
                derived_successors.insert(target.machine_block);
                derived_successors.insert(fallthrough.machine_block);
            }
            _ => {
                let fallthrough = next.ok_or(Gfx942MachineRefinementErrorV1::InvalidMachineCfg)?;
                derived_successors.insert(fallthrough.machine_block);
            }
        }
        if block.successors.iter().copied().collect::<BTreeSet<_>>() != derived_successors {
            return Err(Gfx942MachineRefinementErrorV1::InvalidMachineCfg);
        }
    }
    Ok(())
}

const fn is_machine_terminator(semantics: Gfx942SemanticOpcodeV1) -> bool {
    matches!(
        semantics,
        Gfx942SemanticOpcodeV1::Branch
            | Gfx942SemanticOpcodeV1::BranchScc
            | Gfx942SemanticOpcodeV1::BranchVcc
            | Gfx942SemanticOpcodeV1::BranchExecZero
            | Gfx942SemanticOpcodeV1::BranchExecNonZero
            | Gfx942SemanticOpcodeV1::Return
            | Gfx942SemanticOpcodeV1::Trap
    )
}

fn validate_instruction_local(
    instruction: &Gfx942DecodedInstructionV1,
) -> Result<(), Gfx942MachineRefinementErrorV1> {
    if instruction.native_opcode == 0
        || instruction.object_encoding.is_empty()
        || instruction.hsaco_encoding.is_empty()
        || instruction.object_encoding.len() > MAX_GFX942_INSTRUCTION_BYTES_V1
        || instruction.hsaco_encoding.len() > MAX_GFX942_INSTRUCTION_BYTES_V1
        || instruction.definitions.len() > MAX_GFX942_INSTRUCTION_REGISTERS_V1
        || instruction.uses.len() > MAX_GFX942_INSTRUCTION_REGISTERS_V1
        || has_duplicate(instruction.definitions.iter().copied())
        || has_duplicate(instruction.uses.iter().copied())
    {
        return Err(Gfx942MachineRefinementErrorV1::NonCanonical);
    }
    let branch = matches!(
        instruction.semantics,
        Gfx942SemanticOpcodeV1::Branch
            | Gfx942SemanticOpcodeV1::BranchScc
            | Gfx942SemanticOpcodeV1::BranchVcc
            | Gfx942SemanticOpcodeV1::BranchExecZero
            | Gfx942SemanticOpcodeV1::BranchExecNonZero
            | Gfx942SemanticOpcodeV1::Call
    );
    if branch != instruction.branch_target.is_some() {
        return Err(Gfx942MachineRefinementErrorV1::InvalidMachineCfg);
    }
    let memory_kind = instruction.memory.kind();
    let address_required = matches!(
        instruction.semantics,
        Gfx942SemanticOpcodeV1::Load
            | Gfx942SemanticOpcodeV1::Store
            | Gfx942SemanticOpcodeV1::Atomic
    );
    if address_required
        != !matches!(
            instruction.effective_address,
            Gfx942EffectiveAddressV1::None
        )
    {
        return Err(Gfx942MachineRefinementErrorV1::AddressOrMemoryMismatch);
    }
    let memory_valid = match instruction.semantics {
        Gfx942SemanticOpcodeV1::Load => memory_kind == CompilerMemoryEffectKindV1::Read,
        Gfx942SemanticOpcodeV1::Store => memory_kind == CompilerMemoryEffectKindV1::Write,
        Gfx942SemanticOpcodeV1::Atomic => matches!(
            memory_kind,
            CompilerMemoryEffectKindV1::Read
                | CompilerMemoryEffectKindV1::Write
                | CompilerMemoryEffectKindV1::ReadWrite
        ),
        Gfx942SemanticOpcodeV1::Fence => memory_kind == CompilerMemoryEffectKindV1::Fence,
        Gfx942SemanticOpcodeV1::Barrier => memory_kind == CompilerMemoryEffectKindV1::Barrier,
        _ => memory_kind == CompilerMemoryEffectKindV1::None,
    };
    if !memory_valid {
        return Err(Gfx942MachineRefinementErrorV1::AddressOrMemoryMismatch);
    }
    if let Gfx942EffectiveAddressV1::BaseIndex {
        scale,
        pointer_bits,
        byte_width,
        address_space,
        ..
    } = instruction.effective_address
        && (scale == 0
            || !scale.is_power_of_two()
            || !matches!(pointer_bits, 32 | 64)
            || byte_width != instruction.memory.byte_width()
            || address_space != instruction.memory.address_space())
    {
        return Err(Gfx942MachineRefinementErrorV1::AddressOrMemoryMismatch);
    }
    validate_control_registers(instruction)?;
    let decoded = decode_gfx942_instruction_v1(
        instruction.hsaco_offset,
        instruction.hsaco_encoding.as_ref(),
    )?;
    if decoded.native_opcode != instruction.native_opcode
        || decoded.semantics != instruction.semantics
        || decoded.semantic_discriminator != instruction.semantic_discriminator
        || decoded.definitions.as_slice() != instruction.definitions.as_ref()
        || decoded.uses.as_slice() != instruction.uses.as_ref()
        || decoded.branch_target != instruction.branch_target
        || decoded.memory != instruction.memory
        || decoded.effective_address != instruction.effective_address
    {
        return Err(Gfx942MachineRefinementErrorV1::InstructionDecodeMismatch);
    }
    Ok(())
}

fn validate_control_registers(
    instruction: &Gfx942DecodedInstructionV1,
) -> Result<(), Gfx942MachineRefinementErrorV1> {
    let contains = |values: &[Gfx942SemanticRegisterV1], expected| values.contains(&expected);
    let valid = match instruction.semantics {
        Gfx942SemanticOpcodeV1::ExecAndSave | Gfx942SemanticOpcodeV1::ExecAndNotSave => {
            contains(&instruction.uses, Gfx942SemanticRegisterV1::Exec)
                && contains(&instruction.definitions, Gfx942SemanticRegisterV1::Exec)
                && instruction
                    .definitions
                    .iter()
                    .any(|register| matches!(register, Gfx942SemanticRegisterV1::Scalar(_)))
        }
        Gfx942SemanticOpcodeV1::ExecRestore => {
            contains(&instruction.definitions, Gfx942SemanticRegisterV1::Exec)
                && instruction
                    .uses
                    .iter()
                    .any(|register| matches!(register, Gfx942SemanticRegisterV1::Scalar(_)))
        }
        Gfx942SemanticOpcodeV1::BranchExecZero | Gfx942SemanticOpcodeV1::BranchExecNonZero => {
            contains(&instruction.uses, Gfx942SemanticRegisterV1::Exec)
        }
        Gfx942SemanticOpcodeV1::BranchScc => {
            contains(&instruction.uses, Gfx942SemanticRegisterV1::Scc)
        }
        Gfx942SemanticOpcodeV1::BranchVcc => {
            contains(&instruction.uses, Gfx942SemanticRegisterV1::Vcc)
        }
        _ => !contains(&instruction.definitions, Gfx942SemanticRegisterV1::Mode),
    };
    valid
        .then_some(())
        .ok_or(Gfx942MachineRefinementErrorV1::RegisterSemanticsMismatch)
}

struct BinaryDecodedGfx942InstructionV1 {
    native_opcode: u32,
    semantics: Gfx942SemanticOpcodeV1,
    semantic_discriminator: u32,
    definitions: Vec<Gfx942SemanticRegisterV1>,
    uses: Vec<Gfx942SemanticRegisterV1>,
    branch_target: Option<u64>,
    memory: CompilerMemoryEffectV1,
    effective_address: Gfx942EffectiveAddressV1,
}

/// Independently decoded facts derived only from an exact instruction encoding and typed target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndependentlyDecodedAmdInstructionV1 {
    target: AmdMachineRefinementTargetV1,
    native_opcode: u32,
    semantics: Gfx942SemanticOpcodeV1,
    semantic_discriminator: u32,
    definitions: Box<[Gfx942SemanticRegisterV1]>,
    uses: Box<[Gfx942SemanticRegisterV1]>,
    branch_target: Option<u64>,
    memory: CompilerMemoryEffectV1,
    effective_address: Gfx942EffectiveAddressV1,
}

impl IndependentlyDecodedAmdInstructionV1 {
    /// Returns the typed target that selected the decoder.
    pub const fn target(&self) -> AmdMachineRefinementTargetV1 {
        self.target
    }

    /// Returns the decoder-owned native opcode number.
    pub const fn native_opcode(&self) -> u32 {
        self.native_opcode
    }

    /// Returns the operation semantics derived from the instruction bits.
    pub const fn semantics(&self) -> Gfx942SemanticOpcodeV1 {
        self.semantics
    }

    /// Returns the target-adapter discriminator derived from the instruction bits.
    pub const fn semantic_discriminator(&self) -> u32 {
        self.semantic_discriminator
    }

    /// Returns every explicitly or implicitly defined register.
    pub fn definitions(&self) -> &[Gfx942SemanticRegisterV1] {
        &self.definitions
    }

    /// Returns every explicitly or implicitly used register.
    pub fn uses(&self) -> &[Gfx942SemanticRegisterV1] {
        &self.uses
    }

    /// Returns the decoded direct branch target, when present.
    pub const fn branch_target(&self) -> Option<u64> {
        self.branch_target
    }

    /// Returns the decoded memory shape and minimum ordering/scope contract.
    pub const fn memory(&self) -> CompilerMemoryEffectV1 {
        self.memory
    }

    /// Returns the decoded effective address.
    pub const fn effective_address(&self) -> Gfx942EffectiveAddressV1 {
        self.effective_address
    }
}

/// Decodes one exact instruction using a decoder selected from authenticated target data.
///
/// gfx950 currently admits only encodings independently observed to be identical on the pinned
/// gfx942 and gfx950 toolchains. Target-specific gfx950 scaled MFMA and transpose encodings remain
/// unsupported and therefore fail closed.
pub fn independently_decode_amd_instruction_v1(
    target: AmdTargetId,
    instruction_offset: u64,
    bytes: &[u8],
) -> Result<IndependentlyDecodedAmdInstructionV1, Gfx942MachineRefinementErrorV1> {
    let target = AmdMachineRefinementTargetV1::from_authenticated_target(target)
        .map_err(|_| Gfx942MachineRefinementErrorV1::TargetMismatch)?;
    let decoded = match target {
        AmdMachineRefinementTargetV1::Gfx942 => {
            decode_gfx942_instruction_v1(instruction_offset, bytes)
        }
        AmdMachineRefinementTargetV1::Gfx950 => {
            decode_gfx950_instruction_v1(instruction_offset, bytes)
        }
    }?;
    Ok(IndependentlyDecodedAmdInstructionV1 {
        target,
        native_opcode: decoded.native_opcode,
        semantics: decoded.semantics,
        semantic_discriminator: decoded.semantic_discriminator,
        definitions: decoded.definitions.into_boxed_slice(),
        uses: decoded.uses.into_boxed_slice(),
        branch_target: decoded.branch_target,
        memory: decoded.memory,
        effective_address: decoded.effective_address,
    })
}

fn decode_gfx942_instruction_v1(
    instruction_offset: u64,
    bytes: &[u8],
) -> Result<BinaryDecodedGfx942InstructionV1, Gfx942MachineRefinementErrorV1> {
    decode_gfx9_common_instruction_v1(instruction_offset, bytes)
}

fn decode_gfx950_instruction_v1(
    instruction_offset: u64,
    bytes: &[u8],
) -> Result<BinaryDecodedGfx942InstructionV1, Gfx942MachineRefinementErrorV1> {
    if bytes.len() == 8 {
        let word = u32::from_le_bytes(
            bytes[..4]
                .try_into()
                .map_err(|_| Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode)?,
        );
        if word & 0xffff_f800 == 0xd3ad_8000 {
            return decode_gfx950_scaled_mfma_v1(word, second_instruction_word(bytes)?);
        }
    }
    decode_gfx9_common_instruction_v1(instruction_offset, bytes)
}

fn decode_gfx9_common_instruction_v1(
    instruction_offset: u64,
    bytes: &[u8],
) -> Result<BinaryDecodedGfx942InstructionV1, Gfx942MachineRefinementErrorV1> {
    let word = u32::from_le_bytes(
        bytes
            .get(..4)
            .ok_or(Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode)?
            .try_into()
            .map_err(|_| Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode)?,
    );
    if bytes.len() == 4 && word & 0xff80_0000 == 0xbf80_0000 {
        return decode_sopp_v1(instruction_offset, word);
    }
    if bytes.len() == 4 && word & 0xff80_0000 == 0xbe80_0000 {
        return decode_sop1_v1(word);
    }
    if bytes.len() == 4 && word & 0xc000_0000 == 0x8000_0000 {
        return decode_sop2_v1(word);
    }
    if bytes.len() == 4 && word >> 31 == 0 {
        return decode_vop2_v1(word);
    }
    if bytes.len() == 8 && word >> 31 == 0 && word & 0x1ff == 0xfa {
        let extension = second_instruction_word(bytes)?;
        return decode_vop2_dpp_v1(word, extension);
    }
    if bytes.len() == 8 && word & 0xfe00_0000 == 0xd800_0000 {
        let extension = second_instruction_word(bytes)?;
        return decode_ds_v1(word, extension);
    }
    if bytes.len() == 8 && word & 0xffff_7f00 == 0xd3e1_0000 {
        let extension = second_instruction_word(bytes)?;
        return decode_mfma_f32_16x16x16_bf16_v1(word, extension);
    }
    if bytes.len() == 8 && word & 0xfc00_0000 == 0xdc00_0000 {
        let extension = second_instruction_word(bytes)?;
        return decode_global_v1(word, extension);
    }
    Err(Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode)
}

fn second_instruction_word(bytes: &[u8]) -> Result<u32, Gfx942MachineRefinementErrorV1> {
    Ok(u32::from_le_bytes(
        bytes
            .get(4..8)
            .ok_or(Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode)?
            .try_into()
            .map_err(|_| Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode)?,
    ))
}

fn decoded_no_memory(
    native_opcode: u32,
    semantics: Gfx942SemanticOpcodeV1,
    definitions: impl Into<Vec<Gfx942SemanticRegisterV1>>,
    uses: impl Into<Vec<Gfx942SemanticRegisterV1>>,
    branch_target: Option<u64>,
) -> BinaryDecodedGfx942InstructionV1 {
    BinaryDecodedGfx942InstructionV1 {
        native_opcode,
        semantics,
        semantic_discriminator: 0,
        definitions: definitions.into(),
        uses: uses.into(),
        branch_target,
        memory: CompilerMemoryEffectV1::none(),
        effective_address: Gfx942EffectiveAddressV1::None,
    }
}

fn decode_sopp_v1(
    instruction_offset: u64,
    word: u32,
) -> Result<BinaryDecodedGfx942InstructionV1, Gfx942MachineRefinementErrorV1> {
    let operation = ((word >> 16) & 0x7f) as u8;
    let displacement = i64::from(word as u16 as i16) * 4;
    let branch_target = || {
        instruction_offset
            .checked_add(4)
            .and_then(|next| next.checked_add_signed(displacement))
            .ok_or(Gfx942MachineRefinementErrorV1::InvalidMachineCfg)
    };
    match operation {
        1 if word as u16 == 0 => Ok(decoded_no_memory(
            1,
            Gfx942SemanticOpcodeV1::Return,
            [],
            [],
            None,
        )),
        2 => Ok(decoded_no_memory(
            2,
            Gfx942SemanticOpcodeV1::Branch,
            [],
            [],
            Some(branch_target()?),
        )),
        4 | 5 => Ok(decoded_no_memory(
            u32::from(operation),
            Gfx942SemanticOpcodeV1::BranchScc,
            [],
            [Gfx942SemanticRegisterV1::Scc],
            Some(branch_target()?),
        )),
        6 | 7 => Ok(decoded_no_memory(
            u32::from(operation),
            Gfx942SemanticOpcodeV1::BranchVcc,
            [],
            [Gfx942SemanticRegisterV1::Vcc],
            Some(branch_target()?),
        )),
        8 => Ok(decoded_no_memory(
            8,
            Gfx942SemanticOpcodeV1::BranchExecZero,
            [],
            [Gfx942SemanticRegisterV1::Exec],
            Some(branch_target()?),
        )),
        9 => Ok(decoded_no_memory(
            9,
            Gfx942SemanticOpcodeV1::BranchExecNonZero,
            [],
            [Gfx942SemanticRegisterV1::Exec],
            Some(branch_target()?),
        )),
        10 if word as u16 == 0 => decode_s_barrier_v1(),
        _ => Err(Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode),
    }
}

fn decode_s_barrier_v1() -> Result<BinaryDecodedGfx942InstructionV1, Gfx942MachineRefinementErrorV1>
{
    let memory = CompilerMemoryEffectV1::new(
        CompilerMemoryEffectKindV1::Barrier,
        3,
        0,
        0,
        CompilerMemoryOrderingV1::AcquireRelease,
        CompilerMemoryScopeV1::Workgroup,
        false,
    )
    .map_err(|_| Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode)?;
    Ok(BinaryDecodedGfx942InstructionV1 {
        native_opcode: 0x18a,
        semantics: Gfx942SemanticOpcodeV1::Barrier,
        semantic_discriminator: 0,
        definitions: Vec::new(),
        uses: Vec::new(),
        branch_target: None,
        memory,
        effective_address: Gfx942EffectiveAddressV1::None,
    })
}

fn decode_sop1_v1(
    word: u32,
) -> Result<BinaryDecodedGfx942InstructionV1, Gfx942MachineRefinementErrorV1> {
    let source = (word & 0xff) as u8;
    let operation = ((word >> 8) & 0xff) as u8;
    let destination = ((word >> 16) & 0x7f) as u8;
    match (operation, destination, source) {
        (0x20, destination, 0x6a) if destination <= 104 => Ok(decoded_no_memory(
            0x120,
            Gfx942SemanticOpcodeV1::ExecAndSave,
            [
                Gfx942SemanticRegisterV1::Exec,
                Gfx942SemanticRegisterV1::Scalar(u16::from(destination)),
            ],
            [
                Gfx942SemanticRegisterV1::Exec,
                Gfx942SemanticRegisterV1::Vcc,
            ],
            None,
        )),
        (1, 0x7e, source) if source <= 104 => Ok(decoded_no_memory(
            0x101,
            Gfx942SemanticOpcodeV1::ExecRestore,
            [Gfx942SemanticRegisterV1::Exec],
            [Gfx942SemanticRegisterV1::Scalar(u16::from(source))],
            None,
        )),
        _ => Err(Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode),
    }
}

fn decode_sop2_v1(
    word: u32,
) -> Result<BinaryDecodedGfx942InstructionV1, Gfx942MachineRefinementErrorV1> {
    let source0 = (word & 0xff) as u16;
    let source1 = ((word >> 8) & 0xff) as u16;
    let destination = ((word >> 16) & 0x7f) as u16;
    if source0 > 104 || source1 > 104 || destination > 104 {
        return Err(Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode);
    }
    let operation = (word >> 23) & 0x7f;
    let (semantics, discriminator) = match operation {
        0 => (Gfx942SemanticOpcodeV1::IntegerAdd, 0),
        1 => (Gfx942SemanticOpcodeV1::IntegerSubtract, 0),
        12 => (
            Gfx942SemanticOpcodeV1::IntegerBitwise,
            DISCRIMINATOR_BITWISE_AND,
        ),
        14 => (
            Gfx942SemanticOpcodeV1::IntegerBitwise,
            DISCRIMINATOR_BITWISE_OR,
        ),
        16 => (
            Gfx942SemanticOpcodeV1::IntegerBitwise,
            DISCRIMINATOR_BITWISE_XOR,
        ),
        _ => return Err(Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode),
    };
    Ok(BinaryDecodedGfx942InstructionV1 {
        native_opcode: 0x400 + operation,
        semantics,
        semantic_discriminator: discriminator,
        definitions: vec![
            Gfx942SemanticRegisterV1::Scalar(destination),
            Gfx942SemanticRegisterV1::Scc,
        ],
        uses: vec![
            Gfx942SemanticRegisterV1::Scalar(source0),
            Gfx942SemanticRegisterV1::Scalar(source1),
        ],
        branch_target: None,
        memory: CompilerMemoryEffectV1::none(),
        effective_address: Gfx942EffectiveAddressV1::None,
    })
}

fn decode_vop2_v1(
    word: u32,
) -> Result<BinaryDecodedGfx942InstructionV1, Gfx942MachineRefinementErrorV1> {
    let source0 = (word & 0x1ff) as u16;
    if source0 < 256 {
        return Err(Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode);
    }
    let source0 = Gfx942SemanticRegisterV1::Vector(source0 - 256);
    let source1 = Gfx942SemanticRegisterV1::Vector(((word >> 9) & 0xff) as u16);
    let destination = Gfx942SemanticRegisterV1::Vector(((word >> 17) & 0xff) as u16);
    let (native_opcode, semantics) = match (word >> 25) & 0x3f {
        1 => (0x201, Gfx942SemanticOpcodeV1::FloatAdd32),
        2 => (0x202, Gfx942SemanticOpcodeV1::FloatSubtract32),
        5 => (0x205, Gfx942SemanticOpcodeV1::FloatMultiply32),
        _ => return Err(Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode),
    };
    Ok(decoded_no_memory(
        native_opcode,
        semantics,
        [destination],
        [source0, source1],
        None,
    ))
}

fn decode_vop2_dpp_v1(
    word: u32,
    extension: u32,
) -> Result<BinaryDecodedGfx942InstructionV1, Gfx942MachineRefinementErrorV1> {
    if (word >> 25) & 0x3f != 1 || extension & 0x0006_0000 != 0 {
        return Err(Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode);
    }
    let control = (extension >> 8) & 0x1ff;
    if !matches!(control, 0x111..=0x11f | 0x121..=0x12f) {
        return Err(Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode);
    }
    let source0 = Gfx942SemanticRegisterV1::Vector((extension & 0xff) as u16);
    let source1 = Gfx942SemanticRegisterV1::Vector(((word >> 9) & 0xff) as u16);
    let destination = Gfx942SemanticRegisterV1::Vector(((word >> 17) & 0xff) as u16);
    let bound_control = (extension >> 19) & 1;
    let row_mask = (extension >> 24) & 0xf;
    let bank_mask = (extension >> 28) & 0xf;
    let discriminator = control | (bound_control << 9) | (row_mask << 10) | (bank_mask << 14);
    Ok(BinaryDecodedGfx942InstructionV1 {
        native_opcode: 0x501,
        semantics: Gfx942SemanticOpcodeV1::Collective,
        semantic_discriminator: discriminator,
        definitions: vec![destination],
        uses: vec![source0, source1],
        branch_target: None,
        memory: CompilerMemoryEffectV1::none(),
        effective_address: Gfx942EffectiveAddressV1::None,
    })
}

fn decode_ds_v1(
    word: u32,
    extension: u32,
) -> Result<BinaryDecodedGfx942InstructionV1, Gfx942MachineRefinementErrorV1> {
    let offset = i64::from(word & 0xff);
    let offset1 = (word >> 8) & 0xff;
    let gds = (word >> 16) & 1;
    let operation = (word >> 17) & 0xff;
    if offset1 != 0 || gds != 0 {
        return Err(Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode);
    }
    let address_register = Gfx942SemanticRegisterV1::Vector((extension & 0xff) as u16);
    let data_register = Gfx942SemanticRegisterV1::Vector(((extension >> 8) & 0xff) as u16);
    let destination_register = Gfx942SemanticRegisterV1::Vector(((extension >> 24) & 0xff) as u16);
    let address = Gfx942EffectiveAddressV1::BaseIndex {
        base: address_register,
        index: None,
        scale: 1,
        displacement: offset,
        address_space: 3,
        pointer_bits: 32,
        byte_width: 4,
    };
    let memory = |kind, ordering, scope| {
        CompilerMemoryEffectV1::new(kind, 3, 4, 4, ordering, scope, false)
            .map_err(|_| Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode)
    };
    match operation {
        54 if extension & 0x00ff_ff00 == 0 => Ok(BinaryDecodedGfx942InstructionV1 {
            native_opcode: 0x636,
            semantics: Gfx942SemanticOpcodeV1::Load,
            semantic_discriminator: 0,
            definitions: vec![destination_register],
            uses: vec![address_register],
            branch_target: None,
            memory: memory(
                CompilerMemoryEffectKindV1::Read,
                CompilerMemoryOrderingV1::NotAtomic,
                CompilerMemoryScopeV1::None,
            )?,
            effective_address: address,
        }),
        13 if extension & 0xffff_0000 == 0 => Ok(BinaryDecodedGfx942InstructionV1 {
            native_opcode: 0x60d,
            semantics: Gfx942SemanticOpcodeV1::Store,
            semantic_discriminator: 0,
            definitions: Vec::new(),
            uses: vec![address_register, data_register],
            branch_target: None,
            memory: memory(
                CompilerMemoryEffectKindV1::Write,
                CompilerMemoryOrderingV1::NotAtomic,
                CompilerMemoryScopeV1::None,
            )?,
            effective_address: address,
        }),
        0 if extension & 0xffff_0000 == 0 => Ok(BinaryDecodedGfx942InstructionV1 {
            native_opcode: 0x600,
            semantics: Gfx942SemanticOpcodeV1::Atomic,
            semantic_discriminator: DISCRIMINATOR_ATOMIC_ADD,
            definitions: Vec::new(),
            uses: vec![address_register, data_register],
            branch_target: None,
            memory: memory(
                CompilerMemoryEffectKindV1::ReadWrite,
                CompilerMemoryOrderingV1::Relaxed,
                CompilerMemoryScopeV1::Workgroup,
            )?,
            effective_address: address,
        }),
        _ => Err(Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode),
    }
}

fn decode_mfma_f32_16x16x16_bf16_v1(
    word: u32,
    extension: u32,
) -> Result<BinaryDecodedGfx942InstructionV1, Gfx942MachineRefinementErrorV1> {
    if extension >> 27 != 0 {
        return Err(Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode);
    }
    let source0 = (extension & 0x1ff) as u16;
    let source1 = ((extension >> 9) & 0x1ff) as u16;
    let source2 = ((extension >> 18) & 0x1ff) as u16;
    if source0 < 256 || source1 < 256 || source2 < 256 {
        return Err(Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode);
    }
    let source0 = source0 - 256;
    let source1 = source1 - 256;
    let source2 = source2 - 256;
    let destination = (word & 0xff) as u16;
    if source0 > 254 || source1 > 254 || source2 > 252 || destination > 252 {
        return Err(Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode);
    }
    let accumulator_file = word & (1 << 15) != 0;
    let register = |index| {
        if accumulator_file {
            Gfx942SemanticRegisterV1::Accumulator(index)
        } else {
            Gfx942SemanticRegisterV1::Vector(index)
        }
    };
    let mut definitions = Vec::with_capacity(4);
    let mut uses = Vec::with_capacity(8);
    for index in destination..destination + 4 {
        definitions.push(register(index));
    }
    for index in source0..source0 + 2 {
        uses.push(Gfx942SemanticRegisterV1::Vector(index));
    }
    for index in source1..source1 + 2 {
        uses.push(Gfx942SemanticRegisterV1::Vector(index));
    }
    for index in source2..source2 + 4 {
        uses.push(register(index));
    }
    Ok(BinaryDecodedGfx942InstructionV1 {
        native_opcode: 0x7e1,
        semantics: Gfx942SemanticOpcodeV1::Matrix,
        semantic_discriminator: DISCRIMINATOR_MFMA_F32_16X16X16_BF16,
        definitions,
        uses,
        branch_target: None,
        memory: CompilerMemoryEffectV1::none(),
        effective_address: Gfx942EffectiveAddressV1::None,
    })
}

fn decode_gfx950_scaled_mfma_v1(
    word: u32,
    extension: u32,
) -> Result<BinaryDecodedGfx942InstructionV1, Gfx942MachineRefinementErrorV1> {
    let lhs_format = (word >> 8) & 0x7;
    let rhs_format = extension >> 29;
    let discriminator = match (lhs_format, rhs_format) {
        (0, 0) => DISCRIMINATOR_MFMA_F32_16X16X128_FP8_E4M3,
        (1, 1) => DISCRIMINATOR_MFMA_F32_16X16X128_FP8_E5M2,
        (4, 4) => DISCRIMINATOR_MFMA_F32_16X16X128_FP4_E2M1,
        (4, 0) => DISCRIMINATOR_MFMA_F32_16X16X128_FP4_E2M1_BY_FP8_E4M3,
        _ => return Err(Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode),
    };
    let decode_vgpr = |encoded: u32| {
        u16::try_from(encoded.checked_sub(256)?)
            .ok()
            .filter(|register| *register <= 255)
    };
    let source0 = decode_vgpr(extension & 0x1ff)
        .ok_or(Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode)?;
    let source1 = decode_vgpr((extension >> 9) & 0x1ff)
        .ok_or(Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode)?;
    let source2 = u16::try_from((extension >> 18) & 0x1ff)
        .map_err(|_| Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode)?;
    let destination = (word & 0xff) as u16;
    if source2 < 256 || source2 > 511 || destination > 252 {
        return Err(Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode);
    }
    let source2 = source2 - 256;
    if source2 > 252 {
        return Err(Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode);
    }
    let lhs_registers = if lhs_format == 4 { 4 } else { 8 };
    let rhs_registers = if rhs_format == 4 { 4 } else { 8 };
    if usize::from(source0) + lhs_registers > 256 || usize::from(source1) + rhs_registers > 256 {
        return Err(Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode);
    }
    let definitions = (destination..destination + 4)
        .map(Gfx942SemanticRegisterV1::Accumulator)
        .collect();
    let uses = (source0..source0 + lhs_registers as u16)
        .chain(source1..source1 + rhs_registers as u16)
        .map(Gfx942SemanticRegisterV1::Vector)
        .chain((source2..source2 + 4).map(Gfx942SemanticRegisterV1::Accumulator))
        .collect();
    Ok(BinaryDecodedGfx942InstructionV1 {
        native_opcode: 0x8ad,
        semantics: Gfx942SemanticOpcodeV1::Matrix,
        semantic_discriminator: discriminator,
        definitions,
        uses,
        branch_target: None,
        memory: CompilerMemoryEffectV1::none(),
        effective_address: Gfx942EffectiveAddressV1::None,
    })
}

fn decode_global_v1(
    word: u32,
    extension: u32,
) -> Result<BinaryDecodedGfx942InstructionV1, Gfx942MachineRefinementErrorV1> {
    let encoded_displacement = i64::from(word & 0x1fff);
    let displacement = if encoded_displacement & 0x1000 != 0 {
        encoded_displacement - 0x2000
    } else {
        encoded_displacement
    };
    let vector_address = (extension & 0xff) as u16;
    let Some(vector_address_high) = vector_address.checked_add(1) else {
        return Err(Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode);
    };
    if (extension >> 16) & 0x7f != 0x7f {
        return Err(Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode);
    }
    let memory = |kind| {
        CompilerMemoryEffectV1::new(
            kind,
            1,
            4,
            4,
            CompilerMemoryOrderingV1::NotAtomic,
            CompilerMemoryScopeV1::None,
            false,
        )
        .map_err(|_| Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode)
    };
    let address = Gfx942EffectiveAddressV1::BaseIndex {
        base: Gfx942SemanticRegisterV1::Vector(vector_address),
        index: None,
        scale: 1,
        displacement,
        address_space: 1,
        pointer_bits: 64,
        byte_width: 4,
    };
    match word & !0x1fff {
        0xdc50_8000 if extension & 0x00ff_ff00 == 0x007f_0000 => {
            let destination = Gfx942SemanticRegisterV1::Vector((extension >> 24) as u16);
            Ok(BinaryDecodedGfx942InstructionV1 {
                native_opcode: 0x314,
                semantics: Gfx942SemanticOpcodeV1::Load,
                semantic_discriminator: 0,
                definitions: vec![destination],
                uses: vec![
                    Gfx942SemanticRegisterV1::Vector(vector_address),
                    Gfx942SemanticRegisterV1::Vector(vector_address_high),
                ],
                branch_target: None,
                memory: memory(CompilerMemoryEffectKindV1::Read)?,
                effective_address: address,
            })
        }
        0xdc70_8000 if extension & 0xff00_0000 == 0 => {
            let value = Gfx942SemanticRegisterV1::Vector(((extension >> 8) & 0xff) as u16);
            Ok(BinaryDecodedGfx942InstructionV1 {
                native_opcode: 0x31c,
                semantics: Gfx942SemanticOpcodeV1::Store,
                semantic_discriminator: 0,
                definitions: Vec::new(),
                uses: vec![
                    Gfx942SemanticRegisterV1::Vector(vector_address),
                    Gfx942SemanticRegisterV1::Vector(vector_address_high),
                    value,
                ],
                branch_target: None,
                memory: memory(CompilerMemoryEffectKindV1::Write)?,
                effective_address: address,
            })
        }
        _ => Err(Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode),
    }
}

fn validate_instruction_bytes(
    target: AmdMachineRefinementTargetV1,
    object: &[u8],
    hsaco: &[u8],
    instructions: &[Gfx942DecodedInstructionV1],
) -> Result<(), Gfx942MachineRefinementErrorV1> {
    let object_decoded = decode_complete_executable_text_v1(target, object, false)?;
    let hsaco_decoded = decode_complete_executable_text_v1(target, hsaco, true)?;
    if object_decoded.len() != instructions.len() || hsaco_decoded.len() != instructions.len() {
        return Err(Gfx942MachineRefinementErrorV1::IncompleteInstructionMapping);
    }
    let object_by_offset = object_decoded
        .iter()
        .map(|instruction| (instruction.offset, instruction))
        .collect::<BTreeMap<_, _>>();
    let hsaco_by_offset = hsaco_decoded
        .iter()
        .map(|instruction| (instruction.offset, instruction))
        .collect::<BTreeMap<_, _>>();
    for instruction in instructions {
        let object_instruction = object_by_offset
            .get(&instruction.object_offset)
            .ok_or(Gfx942MachineRefinementErrorV1::IncompleteInstructionMapping)?;
        let hsaco_instruction = hsaco_by_offset
            .get(&instruction.hsaco_offset)
            .ok_or(Gfx942MachineRefinementErrorV1::IncompleteInstructionMapping)?;
        if object_instruction.encoding != instruction.object_encoding.as_ref()
            || !same_decoded_operation(&object_instruction.decoded, instruction)
        {
            return Err(Gfx942MachineRefinementErrorV1::GeneratedObjectMismatch);
        }
        if hsaco_instruction.encoding != instruction.hsaco_encoding.as_ref()
            || !same_decoded_instruction(&hsaco_instruction.decoded, instruction)
        {
            return Err(Gfx942MachineRefinementErrorV1::FinalCodeObjectMismatch);
        }
    }
    Ok(())
}

struct CompleteDecodedGfx942InstructionV1<'a> {
    offset: u64,
    encoding: &'a [u8],
    decoded: BinaryDecodedGfx942InstructionV1,
}

fn decode_complete_executable_text_v1(
    target: AmdMachineRefinementTargetV1,
    artifact: &[u8],
    retain_final_branch_targets: bool,
) -> Result<Vec<CompleteDecodedGfx942InstructionV1<'_>>, Gfx942MachineRefinementErrorV1> {
    let sections = match target {
        AmdMachineRefinementTargetV1::Gfx942 => {
            crate::post_llvm_stage_custody_v1::decode_gfx942_executable_sections_v1(artifact)
        }
        AmdMachineRefinementTargetV1::Gfx950 => {
            crate::post_llvm_stage_custody_v1::decode_gfx950_executable_sections_v1(artifact)
        }
    }
    .map_err(|()| Gfx942MachineRefinementErrorV1::ExecutableTextUnavailable)?;
    let mut result = Vec::new();
    for section in &sections {
        let mut cursor = 0_usize;
        while cursor < section.bytes.len() {
            let first_word = u32::from_le_bytes(
                section
                    .bytes
                    .get(cursor..cursor.saturating_add(4))
                    .ok_or(Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode)?
                    .try_into()
                    .map_err(|_| Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode)?,
            );
            let instruction_len = if first_word & 0xfc00_0000 == 0xdc00_0000
                || first_word & 0xfe00_0000 == 0xd800_0000
                || first_word & 0xffff_7f00 == 0xd3e1_0000
                || first_word & 0xffff_f800 == 0xd3ad_8000
                || first_word >> 31 == 0 && first_word & 0x1ff == 0xfa
            {
                8
            } else {
                4
            };
            let encoding = section
                .bytes
                .get(cursor..cursor.saturating_add(instruction_len))
                .ok_or(Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode)?;
            let offset = section
                .file_offset
                .checked_add(cursor as u64)
                .ok_or(Gfx942MachineRefinementErrorV1::ResourceLimit)?;
            let mut decoded = match target {
                AmdMachineRefinementTargetV1::Gfx942 => {
                    decode_gfx942_instruction_v1(offset, encoding)
                }
                AmdMachineRefinementTargetV1::Gfx950 => {
                    decode_gfx950_instruction_v1(offset, encoding)
                }
            }?;
            if !retain_final_branch_targets {
                decoded.branch_target = None;
            }
            result.push(CompleteDecodedGfx942InstructionV1 {
                offset,
                encoding,
                decoded,
            });
            if result.len() > MAX_GFX942_DECODED_ISA_INSTRUCTIONS_V1 {
                return Err(Gfx942MachineRefinementErrorV1::ResourceLimit);
            }
            cursor = cursor
                .checked_add(instruction_len)
                .ok_or(Gfx942MachineRefinementErrorV1::ResourceLimit)?;
        }
    }
    Ok(result)
}

fn same_decoded_operation(
    decoded: &BinaryDecodedGfx942InstructionV1,
    claimed: &Gfx942DecodedInstructionV1,
) -> bool {
    decoded.native_opcode == claimed.native_opcode
        && decoded.semantics == claimed.semantics
        && decoded.semantic_discriminator == claimed.semantic_discriminator
        && decoded.definitions.as_slice() == claimed.definitions.as_ref()
        && decoded.uses.as_slice() == claimed.uses.as_ref()
        && decoded.memory == claimed.memory
        && decoded.effective_address == claimed.effective_address
}

fn same_decoded_instruction(
    decoded: &BinaryDecodedGfx942InstructionV1,
    claimed: &Gfx942DecodedInstructionV1,
) -> bool {
    same_decoded_operation(decoded, claimed) && decoded.branch_target == claimed.branch_target
}

fn validate_correspondence(
    compiler: &fe2o3_compiler_lineage::CompilerInstructionSelectionCorrespondenceV1,
    isa: &Gfx942DecodedIsaTranscriptV1,
) -> Result<u32, Gfx942MachineRefinementErrorV1> {
    let by_offset = isa
        .instructions()
        .iter()
        .map(|instruction| (instruction.hsaco_offset, instruction))
        .collect::<BTreeMap<_, _>>();
    if by_offset.len() != isa.instructions().len() {
        return Err(Gfx942MachineRefinementErrorV1::NonCanonical);
    }
    let mapped = compiler
        .operations()
        .iter()
        .flat_map(|operation| operation.machine_offsets().iter().copied())
        .collect::<BTreeSet<_>>();
    if mapped.len() != isa.instructions().len()
        || mapped.iter().copied().ne(by_offset.keys().copied())
    {
        return Err(Gfx942MachineRefinementErrorV1::IncompleteInstructionMapping);
    }
    let machine_blocks = isa
        .blocks()
        .iter()
        .map(|block| ((block.function, block.machine_block), block))
        .collect::<BTreeMap<_, _>>();
    for operation in compiler.operations() {
        let instructions = operation
            .machine_offsets()
            .iter()
            .map(|offset| {
                by_offset
                    .get(offset)
                    .copied()
                    .ok_or(Gfx942MachineRefinementErrorV1::IncompleteInstructionMapping)
            })
            .collect::<Result<Vec<_>, _>>()?;
        validate_operation_mapping(operation, &instructions, isa.mode(), &machine_blocks)?;
    }
    validate_llvm_machine_cfg(
        compiler.blocks(),
        compiler.operations(),
        isa.blocks(),
        &by_offset,
    )
}

fn validate_operation_mapping(
    operation: &CompilerLlvmOperationV1,
    instructions: &[&Gfx942DecodedInstructionV1],
    mode: Gfx942FloatingModeV1,
    machine_blocks: &BTreeMap<(u32, u32), &Gfx942DecodedMachineBlockV1>,
) -> Result<(), Gfx942MachineRefinementErrorV1> {
    validate_operation_numerics(operation, mode)?;
    if matches!(
        operation.numerical(),
        CompilerNumericalContractV1::IeeeBinary32 | CompilerNumericalContractV1::IeeeBinary32Fused
    ) && !mode.supports_strict_binary32()
    {
        return Err(Gfx942MachineRefinementErrorV1::ModeOrIeeeMismatch);
    }
    validate_machine_value_bindings(operation, instructions)?;
    let primary = expected_primary_semantics(operation)?;
    if let Some(primary) = primary {
        let matched = instructions
            .iter()
            .find(|instruction| instruction.semantics == primary)
            .ok_or(Gfx942MachineRefinementErrorV1::OperationSemanticsMismatch)?;
        if matched.semantic_discriminator != operation.semantic_discriminator() {
            return Err(Gfx942MachineRefinementErrorV1::UnsupportedSemanticDiscriminator);
        }
        if operation.memory() != matched.memory {
            return Err(
                if matches!(
                    operation.kind(),
                    CompilerLlvmOperationKindV1::Atomic
                        | CompilerLlvmOperationKindV1::Fence
                        | CompilerLlvmOperationKindV1::Barrier
                ) {
                    Gfx942MachineRefinementErrorV1::SynchronizationMismatch
                } else {
                    Gfx942MachineRefinementErrorV1::AddressOrMemoryMismatch
                },
            );
        }
        if matches!(
            operation.kind(),
            CompilerLlvmOperationKindV1::Load
                | CompilerLlvmOperationKindV1::Store
                | CompilerLlvmOperationKindV1::Atomic
        ) {
            validate_address_against_memory(matched.effective_address, operation.memory())?;
            validate_compiler_effective_address(operation, matched)?;
        }
    } else if !instructions
        .iter()
        .all(|instruction| allowed_non_primary_semantics(operation, instruction.semantics))
    {
        return Err(Gfx942MachineRefinementErrorV1::OperationSemanticsMismatch);
    }
    match operation.kind() {
        CompilerLlvmOperationKindV1::ConditionalBranch => match operation.divergence() {
            CompilerBranchDivergenceV1::Divergent => {
                validate_exec_sequence(instructions, operation.successors(), machine_blocks)?
            }
            CompilerBranchDivergenceV1::Uniform => {
                if !instructions
                    .iter()
                    .any(|instruction| instruction.semantics == Gfx942SemanticOpcodeV1::BranchScc)
                {
                    return Err(Gfx942MachineRefinementErrorV1::ControlFlowMismatch);
                }
            }
            CompilerBranchDivergenceV1::None => {
                return Err(Gfx942MachineRefinementErrorV1::ControlFlowMismatch);
            }
        },
        CompilerLlvmOperationKindV1::Switch => {
            validate_switch_sequence(instructions, operation.successors(), machine_blocks)?;
        }
        CompilerLlvmOperationKindV1::Branch | CompilerLlvmOperationKindV1::Return => {}
        _ => {
            if instructions.iter().any(|instruction| {
                matches!(
                    instruction.semantics,
                    Gfx942SemanticOpcodeV1::Branch
                        | Gfx942SemanticOpcodeV1::BranchScc
                        | Gfx942SemanticOpcodeV1::BranchVcc
                        | Gfx942SemanticOpcodeV1::BranchExecZero
                        | Gfx942SemanticOpcodeV1::BranchExecNonZero
                        | Gfx942SemanticOpcodeV1::Return
                )
            }) {
                return Err(Gfx942MachineRefinementErrorV1::ControlFlowMismatch);
            }
        }
    }
    Ok(())
}

fn validate_operation_numerics(
    operation: &CompilerLlvmOperationV1,
    mode: Gfx942FloatingModeV1,
) -> Result<(), Gfx942MachineRefinementErrorV1> {
    use CompilerNumericalContractV1 as Numerical;
    let expected_matrix = match operation.numerical() {
        Numerical::MfmaBf16AccumulateF32 => Some(DISCRIMINATOR_MFMA_F32_16X16X16_BF16),
        Numerical::MfmaFp8E4M3AccumulateF32 => Some(DISCRIMINATOR_MFMA_F32_16X16X128_FP8_E4M3),
        Numerical::MfmaFp8E5M2AccumulateF32 => Some(DISCRIMINATOR_MFMA_F32_16X16X128_FP8_E5M2),
        Numerical::MfmaFp4E2M1AccumulateF32 => Some(DISCRIMINATOR_MFMA_F32_16X16X128_FP4_E2M1),
        Numerical::MfmaFp4E2M1ByFp8E4M3AccumulateF32 => {
            Some(DISCRIMINATOR_MFMA_F32_16X16X128_FP4_E2M1_BY_FP8_E4M3)
        }
        Numerical::TargetDefined | Numerical::IeeeBinary64 => {
            return Err(Gfx942MachineRefinementErrorV1::UnsupportedNumericalContract);
        }
        Numerical::IeeeBinary32 | Numerical::IeeeBinary32Fused => {
            if !mode.supports_strict_binary32() {
                return Err(Gfx942MachineRefinementErrorV1::ModeOrIeeeMismatch);
            }
            None
        }
        Numerical::Exact => None,
    };
    if let Some(expected) = expected_matrix {
        if operation.kind() != CompilerLlvmOperationKindV1::Matrix
            || operation.semantic_discriminator() != expected
            || !mode.supports_strict_binary32()
        {
            return Err(Gfx942MachineRefinementErrorV1::UnsupportedNumericalContract);
        }
    }
    Ok(())
}

fn validate_machine_value_bindings(
    operation: &CompilerLlvmOperationV1,
    instructions: &[&Gfx942DecodedInstructionV1],
) -> Result<(), Gfx942MachineRefinementErrorV1> {
    validate_phi_edge_transports(operation, instructions)?;
    if instructions.is_empty() {
        return Ok(());
    }
    let by_offset = instructions
        .iter()
        .map(|instruction| (instruction.hsaco_offset, *instruction))
        .collect::<BTreeMap<_, _>>();
    for binding in operation.machine_value_bindings() {
        let instruction = by_offset
            .get(&binding.machine_offset())
            .ok_or(Gfx942MachineRefinementErrorV1::IncompleteValueBinding)?;
        let register = match binding.location() {
            CompilerMachineValueLocationV1::Register(register) => {
                translate_compiler_register(register)
            }
            CompilerMachineValueLocationV1::Immediate { .. } => {
                return Err(Gfx942MachineRefinementErrorV1::UnsupportedMachineValueLocation);
            }
        };
        let registers = match binding.access() {
            CompilerMachineValueAccessV1::Definition => instruction.definitions(),
            CompilerMachineValueAccessV1::Use => instruction.uses(),
        };
        if !registers.contains(&register) {
            return Err(Gfx942MachineRefinementErrorV1::RegisterSemanticsMismatch);
        }
    }
    if operation.result().is_some()
        && !operation
            .machine_value_bindings()
            .iter()
            .any(|binding| binding.access() == CompilerMachineValueAccessV1::Definition)
    {
        return Err(Gfx942MachineRefinementErrorV1::IncompleteValueBinding);
    }
    for operand in operation.operands() {
        if !operation.machine_value_bindings().iter().any(|binding| {
            binding.value() == *operand && binding.access() == CompilerMachineValueAccessV1::Use
        }) {
            return Err(Gfx942MachineRefinementErrorV1::IncompleteValueBinding);
        }
    }
    Ok(())
}

fn validate_phi_edge_transports(
    operation: &CompilerLlvmOperationV1,
    instructions: &[&Gfx942DecodedInstructionV1],
) -> Result<(), Gfx942MachineRefinementErrorV1> {
    if operation.kind() != CompilerLlvmOperationKindV1::Phi {
        return operation
            .phi_edge_transports()
            .is_empty()
            .then_some(())
            .ok_or(Gfx942MachineRefinementErrorV1::IncompleteValueBinding);
    }
    let by_offset = instructions
        .iter()
        .map(|instruction| (instruction.hsaco_offset(), *instruction))
        .collect::<BTreeMap<_, _>>();
    for transport in operation.phi_edge_transports() {
        let result = translate_compiler_register(transport.result());
        let incoming = match transport.incoming() {
            CompilerMachineValueLocationV1::Register(register) => {
                translate_compiler_register(register)
            }
            CompilerMachineValueLocationV1::Immediate { .. } => {
                return Err(Gfx942MachineRefinementErrorV1::UnsupportedMachineValueLocation);
            }
        };
        match transport.move_offset() {
            None if incoming == result => {}
            None => return Err(Gfx942MachineRefinementErrorV1::RegisterSemanticsMismatch),
            Some(offset) => {
                let instruction = by_offset
                    .get(&offset)
                    .ok_or(Gfx942MachineRefinementErrorV1::IncompleteValueBinding)?;
                if !instruction.uses().contains(&incoming)
                    || !instruction.definitions().contains(&result)
                {
                    return Err(Gfx942MachineRefinementErrorV1::RegisterSemanticsMismatch);
                }
            }
        }
    }
    Ok(())
}

const fn translate_compiler_register(
    register: CompilerMachineRegisterV1,
) -> Gfx942SemanticRegisterV1 {
    match register.class() {
        CompilerMachineRegisterClassV1::Scalar => {
            Gfx942SemanticRegisterV1::Scalar(register.index())
        }
        CompilerMachineRegisterClassV1::Vector => {
            Gfx942SemanticRegisterV1::Vector(register.index())
        }
        CompilerMachineRegisterClassV1::Accumulator => {
            Gfx942SemanticRegisterV1::Accumulator(register.index())
        }
        CompilerMachineRegisterClassV1::ExecutionMask => Gfx942SemanticRegisterV1::Exec,
        CompilerMachineRegisterClassV1::VectorCondition => Gfx942SemanticRegisterV1::Vcc,
        CompilerMachineRegisterClassV1::ScalarCondition => Gfx942SemanticRegisterV1::Scc,
        CompilerMachineRegisterClassV1::MemoryOffset => Gfx942SemanticRegisterV1::M0,
        CompilerMachineRegisterClassV1::FloatingMode => Gfx942SemanticRegisterV1::Mode,
    }
}

fn validate_compiler_effective_address(
    operation: &CompilerLlvmOperationV1,
    instruction: &Gfx942DecodedInstructionV1,
) -> Result<(), Gfx942MachineRefinementErrorV1> {
    let compiler = operation
        .effective_address()
        .ok_or(Gfx942MachineRefinementErrorV1::IncompleteValueBinding)?;
    let Gfx942EffectiveAddressV1::BaseIndex {
        base,
        index,
        scale,
        displacement,
        address_space,
        pointer_bits,
        byte_width,
    } = instruction.effective_address
    else {
        return Err(Gfx942MachineRefinementErrorV1::AddressOrMemoryMismatch);
    };
    if index.is_some()
        || scale != 1
        || compiler.address_space() != address_space
        || compiler.pointer_bits() != pointer_bits
        || compiler.byte_width() != byte_width
        || compiler.displacement() != displacement
        || compiler.terms().len() != 1
        || compiler.terms()[0].scale() != 1
    {
        return Err(Gfx942MachineRefinementErrorV1::AddressOrMemoryMismatch);
    }
    let value = compiler.terms()[0].value();
    let mut registers = operation
        .machine_value_bindings()
        .iter()
        .filter(|binding| {
            binding.machine_offset() == instruction.hsaco_offset
                && binding.value() == value
                && binding.access() == CompilerMachineValueAccessV1::Use
        })
        .map(|binding| match binding.location() {
            CompilerMachineValueLocationV1::Register(register) => {
                Ok(translate_compiler_register(register))
            }
            CompilerMachineValueLocationV1::Immediate { .. } => {
                Err(Gfx942MachineRefinementErrorV1::UnsupportedMachineValueLocation)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    registers.sort_unstable();
    let mut expected = vec![base];
    if pointer_bits == 64 {
        let Gfx942SemanticRegisterV1::Vector(base) = base else {
            return Err(Gfx942MachineRefinementErrorV1::AddressOrMemoryMismatch);
        };
        expected.push(Gfx942SemanticRegisterV1::Vector(
            base.checked_add(1)
                .ok_or(Gfx942MachineRefinementErrorV1::AddressOrMemoryMismatch)?,
        ));
    }
    expected.sort_unstable();
    if registers != expected {
        return Err(Gfx942MachineRefinementErrorV1::AddressOrMemoryMismatch);
    }
    Ok(())
}

fn expected_primary_semantics(
    operation: &CompilerLlvmOperationV1,
) -> Result<Option<Gfx942SemanticOpcodeV1>, Gfx942MachineRefinementErrorV1> {
    use CompilerLlvmOperationKindV1 as Llvm;
    use Gfx942SemanticOpcodeV1 as Isa;
    let kind = operation.kind();
    let discriminator = operation.semantic_discriminator();
    let semantics = match kind {
        Llvm::Constant | Llvm::Vector | Llvm::SymbolAddress | Llvm::Phi | Llvm::Allocate => None,
        Llvm::IntegerAdd => Some(Isa::IntegerAdd),
        Llvm::IntegerSubtract => Some(Isa::IntegerSubtract),
        Llvm::IntegerMultiply => Some(Isa::IntegerMultiply),
        Llvm::IntegerDivide => Some(Isa::IntegerDivide),
        Llvm::IntegerRemainder => Some(Isa::IntegerRemainder),
        Llvm::IntegerBitwise => Some(Isa::IntegerBitwise),
        Llvm::IntegerShift => Some(Isa::IntegerShift),
        Llvm::FloatAdd => Some(Isa::FloatAdd32),
        Llvm::FloatSubtract => Some(Isa::FloatSubtract32),
        Llvm::FloatMultiply => Some(Isa::FloatMultiply32),
        Llvm::FloatDivide => Some(Isa::FloatDivide32),
        Llvm::FloatFma => Some(Isa::FloatFma32),
        Llvm::FloatSqrt => Some(Isa::FloatSqrt32),
        Llvm::IntegerCompare => Some(Isa::IntegerCompare),
        Llvm::FloatCompare => Some(Isa::FloatCompare32),
        Llvm::Cast => Some(Isa::Cast),
        Llvm::Select => Some(Isa::Select),
        Llvm::Address => Some(Isa::Address),
        Llvm::Load => Some(Isa::Load),
        Llvm::Store => Some(Isa::Store),
        Llvm::Atomic => Some(Isa::Atomic),
        Llvm::Fence => Some(Isa::Fence),
        Llvm::Barrier => Some(Isa::Barrier),
        Llvm::Call => Some(Isa::Call),
        Llvm::Intrinsic => Some(match discriminator {
            1..=7 => Isa::InvocationIndex,
            8 => Isa::Barrier,
            9 => Isa::FloatFma32,
            10 => Isa::FloatSqrt32,
            11 => Isa::Trap,
            12 => Isa::Collective,
            13 => Isa::Matrix,
            _ => {
                return Err(Gfx942MachineRefinementErrorV1::UnsupportedSemanticDiscriminator);
            }
        }),
        Llvm::Branch => Some(Isa::Branch),
        Llvm::ConditionalBranch => None,
        Llvm::Switch => None,
        Llvm::Return => Some(Isa::Return),
        Llvm::Unreachable | Llvm::Trap => Some(Isa::Trap),
        Llvm::InvocationIndex => Some(Isa::InvocationIndex),
        Llvm::Collective => Some(Isa::Collective),
        Llvm::Matrix => Some(Isa::Matrix),
        Llvm::InlineAssembly => Some(Isa::InlineAssembly),
    };
    if discriminator_admitted(kind, discriminator) {
        Ok(semantics)
    } else {
        Err(Gfx942MachineRefinementErrorV1::UnsupportedSemanticDiscriminator)
    }
}

fn discriminator_admitted(kind: CompilerLlvmOperationKindV1, discriminator: u32) -> bool {
    use CompilerLlvmOperationKindV1 as Llvm;
    match kind {
        Llvm::IntegerDivide | Llvm::IntegerRemainder => matches!(discriminator, 1 | 2),
        Llvm::IntegerBitwise | Llvm::IntegerShift => matches!(discriminator, 1..=3),
        Llvm::IntegerCompare => matches!(discriminator, 1..=10),
        Llvm::FloatCompare => matches!(discriminator, 1..=16),
        Llvm::Cast => matches!(discriminator, 1..=13),
        Llvm::Atomic => matches!(discriminator, 1..=13),
        Llvm::InvocationIndex => matches!(discriminator, 1..=7),
        Llvm::Collective => dpp_discriminator_admitted(discriminator),
        Llvm::Matrix => matrix_discriminator_admitted(discriminator),
        Llvm::Intrinsic => matches!(discriminator, 1..=13),
        // Arbitrary inline assembly has no admitted operational model in V1.
        Llvm::InlineAssembly => false,
        _ => discriminator == 0,
    }
}

fn matrix_discriminator_admitted(discriminator: u32) -> bool {
    matches!(
        discriminator,
        DISCRIMINATOR_MFMA_F32_16X16X16_BF16
            | DISCRIMINATOR_MFMA_F32_16X16X128_FP8_E4M3
            | DISCRIMINATOR_MFMA_F32_16X16X128_FP8_E5M2
            | DISCRIMINATOR_MFMA_F32_16X16X128_FP4_E2M1
            | DISCRIMINATOR_MFMA_F32_16X16X128_FP4_E2M1_BY_FP8_E4M3
    )
}

fn dpp_discriminator_admitted(discriminator: u32) -> bool {
    if discriminator >> 18 != 0 {
        return false;
    }
    let control = discriminator & 0x1ff;
    let bound_control = (discriminator >> 9) & 1 != 0;
    let row_mask = (discriminator >> 10) & 0xf;
    let bank_mask = (discriminator >> 14) & 0xf;
    if !matches!(control, 0x111..=0x11f | 0x121..=0x12f) || row_mask == 0 || bank_mask == 0 {
        return false;
    }
    (0_u8..64)
        .all(|lane| dpp_source_lane(control, bound_control, row_mask, bank_mask, lane).is_some())
}

fn dpp_source_lane(
    control: u32,
    bound_control: bool,
    row_mask: u32,
    bank_mask: u32,
    lane: u8,
) -> Option<Option<u8>> {
    let row = u32::from(lane / 16);
    let bank = u32::from((lane % 16) / 4);
    if row_mask & (1 << row) == 0 || bank_mask & (1 << bank) == 0 {
        return Some(Some(lane));
    }
    let lane_in_row = u32::from(lane % 16);
    let source_in_row = match control {
        0x111..=0x11f => lane_in_row.checked_sub(control - 0x110),
        0x121..=0x12f => Some((lane_in_row + 16 - (control - 0x120)) % 16),
        _ => return None,
    };
    match source_in_row {
        Some(source) => Some(Some((row * 16 + source) as u8)),
        None if bound_control => Some(None),
        None => Some(Some(lane)),
    }
}

fn allowed_non_primary_semantics(
    operation: &CompilerLlvmOperationV1,
    semantics: Gfx942SemanticOpcodeV1,
) -> bool {
    use CompilerBranchDivergenceV1 as Divergence;
    use CompilerLlvmOperationKindV1 as Llvm;
    use Gfx942SemanticOpcodeV1 as Isa;
    match operation.kind() {
        Llvm::Constant | Llvm::Vector | Llvm::Phi => semantics == Isa::Copy,
        Llvm::SymbolAddress | Llvm::Allocate => matches!(semantics, Isa::Address | Isa::Copy),
        Llvm::ConditionalBranch if operation.divergence() == Divergence::Divergent => matches!(
            semantics,
            Isa::Copy
                | Isa::IntegerCompare
                | Isa::FloatCompare32
                | Isa::ExecAndSave
                | Isa::ExecAndNotSave
                | Isa::BranchExecZero
                | Isa::BranchExecNonZero
                | Isa::ExecRestore
                | Isa::Branch
        ),
        Llvm::ConditionalBranch if operation.divergence() == Divergence::Uniform => matches!(
            semantics,
            Isa::Copy | Isa::IntegerCompare | Isa::FloatCompare32 | Isa::BranchScc | Isa::Branch
        ),
        Llvm::Switch => matches!(
            semantics,
            Isa::Copy | Isa::IntegerCompare | Isa::Branch | Isa::BranchScc | Isa::BranchVcc
        ),
        _ => false,
    }
}

fn validate_switch_sequence(
    instructions: &[&Gfx942DecodedInstructionV1],
    llvm_successors: &[u32],
    machine_blocks: &BTreeMap<(u32, u32), &Gfx942DecodedMachineBlockV1>,
) -> Result<(), Gfx942MachineRefinementErrorV1> {
    let branches = instructions
        .iter()
        .filter(|instruction| {
            matches!(
                instruction.semantics,
                Gfx942SemanticOpcodeV1::Branch
                    | Gfx942SemanticOpcodeV1::BranchScc
                    | Gfx942SemanticOpcodeV1::BranchVcc
            )
        })
        .collect::<Vec<_>>();
    if branches.is_empty() {
        return Err(Gfx942MachineRefinementErrorV1::ControlFlowMismatch);
    }
    for branch in branches {
        let target = branch
            .branch_target
            .ok_or(Gfx942MachineRefinementErrorV1::ControlFlowMismatch)?;
        let target_block = machine_blocks
            .values()
            .find(|block| block.function == branch.function && block.first_instruction == target)
            .ok_or(Gfx942MachineRefinementErrorV1::ControlFlowMismatch)?;
        if !llvm_successors.contains(&target_block.llvm_block) {
            return Err(Gfx942MachineRefinementErrorV1::ControlFlowMismatch);
        }
    }
    Ok(())
}

fn validate_address_against_memory(
    address: Gfx942EffectiveAddressV1,
    memory: CompilerMemoryEffectV1,
) -> Result<(), Gfx942MachineRefinementErrorV1> {
    match address {
        Gfx942EffectiveAddressV1::BaseIndex {
            address_space,
            byte_width,
            pointer_bits,
            ..
        } if address_space == memory.address_space()
            && byte_width == memory.byte_width()
            && matches!(pointer_bits, 32 | 64) =>
        {
            Ok(())
        }
        _ => Err(Gfx942MachineRefinementErrorV1::AddressOrMemoryMismatch),
    }
}

fn validate_exec_sequence(
    instructions: &[&Gfx942DecodedInstructionV1],
    llvm_successors: &[u32],
    machine_blocks: &BTreeMap<(u32, u32), &Gfx942DecodedMachineBlockV1>,
) -> Result<(), Gfx942MachineRefinementErrorV1> {
    let saves = instructions
        .iter()
        .enumerate()
        .filter(|(_, instruction)| {
            matches!(
                instruction.semantics,
                Gfx942SemanticOpcodeV1::ExecAndSave | Gfx942SemanticOpcodeV1::ExecAndNotSave
            )
        })
        .collect::<Vec<_>>();
    let branches = instructions
        .iter()
        .enumerate()
        .filter(|(_, instruction)| {
            matches!(
                instruction.semantics,
                Gfx942SemanticOpcodeV1::BranchExecZero | Gfx942SemanticOpcodeV1::BranchExecNonZero
            )
        })
        .collect::<Vec<_>>();
    let restores = instructions
        .iter()
        .enumerate()
        .filter(|(_, instruction)| instruction.semantics == Gfx942SemanticOpcodeV1::ExecRestore)
        .collect::<Vec<_>>();
    let ([(save_index, save)], [(branch_index, branch)], [(restore_index, restore)]) =
        (saves.as_slice(), branches.as_slice(), restores.as_slice())
    else {
        return Err(Gfx942MachineRefinementErrorV1::ExecSemanticsMismatch);
    };
    if !(save_index < branch_index && branch_index < restore_index) {
        return Err(Gfx942MachineRefinementErrorV1::ExecSemanticsMismatch);
    }
    let saved_registers = save
        .definitions
        .iter()
        .filter(|register| matches!(register, Gfx942SemanticRegisterV1::Scalar(_)))
        .copied()
        .collect::<Vec<_>>();
    let restored_registers = restore
        .uses
        .iter()
        .filter(|register| matches!(register, Gfx942SemanticRegisterV1::Scalar(_)))
        .copied()
        .collect::<Vec<_>>();
    if saved_registers.len() != 1 || saved_registers != restored_registers {
        return Err(Gfx942MachineRefinementErrorV1::ExecSemanticsMismatch);
    }
    let target = branch
        .branch_target
        .ok_or(Gfx942MachineRefinementErrorV1::ExecSemanticsMismatch)?;
    if target != restore.hsaco_offset {
        return Err(Gfx942MachineRefinementErrorV1::ExecSemanticsMismatch);
    }
    let target_block = machine_blocks
        .values()
        .find(|block| block.function == branch.function && block.first_instruction == target)
        .ok_or(Gfx942MachineRefinementErrorV1::ControlFlowMismatch)?;
    if !llvm_successors.contains(&target_block.llvm_block) {
        return Err(Gfx942MachineRefinementErrorV1::ControlFlowMismatch);
    }
    Ok(())
}

fn validate_llvm_machine_cfg(
    llvm_blocks: &[CompilerLlvmBlockV1],
    operations: &[CompilerLlvmOperationV1],
    machine_blocks: &[Gfx942DecodedMachineBlockV1],
    instructions: &BTreeMap<u64, &Gfx942DecodedInstructionV1>,
) -> Result<u32, Gfx942MachineRefinementErrorV1> {
    let mut llvm_by_function = BTreeMap::<u32, Vec<&CompilerLlvmBlockV1>>::new();
    for block in llvm_blocks {
        llvm_by_function
            .entry(block.function())
            .or_default()
            .push(block);
    }
    let mut backedges = 0_u32;
    for (function, blocks) in llvm_by_function {
        let dominators = dominators(&blocks)?;
        for block in &blocks {
            for successor in block.successors() {
                if dominators[block.block() as usize].contains(successor) {
                    backedges = backedges
                        .checked_add(1)
                        .ok_or(Gfx942MachineRefinementErrorV1::ResourceLimit)?;
                    let terminator = operations
                        .iter()
                        .find(|operation| {
                            operation.coordinate().function() == function
                                && operation.coordinate().block() == block.block()
                                && matches!(
                                    operation.kind(),
                                    CompilerLlvmOperationKindV1::Branch
                                        | CompilerLlvmOperationKindV1::ConditionalBranch
                                        | CompilerLlvmOperationKindV1::Switch
                                )
                        })
                        .ok_or(Gfx942MachineRefinementErrorV1::ControlFlowMismatch)?;
                    let has_machine_backedge = terminator.machine_offsets().iter().any(|offset| {
                        instructions.get(offset).is_some_and(|instruction| {
                            instruction.branch_target.is_some_and(|target| {
                                machine_blocks.iter().any(|machine| {
                                    machine.function == function
                                        && machine.llvm_block == *successor
                                        && machine.first_instruction == target
                                })
                            })
                        })
                    });
                    if !has_machine_backedge {
                        return Err(Gfx942MachineRefinementErrorV1::ControlFlowMismatch);
                    }
                }
            }
        }
    }
    Ok(backedges)
}

fn dominators(
    blocks: &[&CompilerLlvmBlockV1],
) -> Result<Vec<BTreeSet<u32>>, Gfx942MachineRefinementErrorV1> {
    if blocks.is_empty()
        || blocks
            .iter()
            .map(|block| block.block())
            .ne(0..blocks.len() as u32)
    {
        return Err(Gfx942MachineRefinementErrorV1::ControlFlowMismatch);
    }
    let all = (0..blocks.len() as u32).collect::<BTreeSet<_>>();
    let mut predecessors = vec![Vec::new(); blocks.len()];
    for block in blocks {
        for successor in block.successors() {
            predecessors[*successor as usize].push(block.block());
        }
    }
    let mut result = vec![all; blocks.len()];
    result[0] = [0].into_iter().collect();
    let mut work = 0_usize;
    loop {
        let snapshot = result.clone();
        let mut changed = false;
        for block in 1..blocks.len() {
            work = work
                .checked_add(predecessors[block].len().max(1) * blocks.len())
                .ok_or(Gfx942MachineRefinementErrorV1::ResourceLimit)?;
            if work > MAX_GFX942_DECODED_ISA_BLOCKS_V1 * MAX_GFX942_DECODED_ISA_BLOCKS_V1 {
                return Err(Gfx942MachineRefinementErrorV1::ResourceLimit);
            }
            let mut next = predecessors[block]
                .first()
                .map_or_else(BTreeSet::new, |predecessor| {
                    snapshot[*predecessor as usize].clone()
                });
            for predecessor in predecessors[block].iter().skip(1) {
                next.retain(|candidate| snapshot[*predecessor as usize].contains(candidate));
            }
            next.insert(block as u32);
            if next != result[block] {
                result[block] = next;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    Ok(result)
}

fn refinement_identity(
    compiler: CompilerInstructionSelectionCorrespondenceIdentityV1,
    isa: Gfx942DecodedIsaTranscriptIdentityV1,
) -> Gfx942MachineRefinementIdentityV1 {
    let mut digest = Sha256::new();
    digest.update((REFINEMENT_IDENTITY_DOMAIN_V1.len() as u32).to_le_bytes());
    digest.update(REFINEMENT_IDENTITY_DOMAIN_V1);
    digest.update(compiler.sha256());
    digest.update(compiler.byte_len().to_le_bytes());
    digest.update(isa.sha256());
    digest.update(isa.byte_len().to_le_bytes());
    Gfx942MachineRefinementIdentityV1 {
        sha256: digest.finalize().into(),
    }
}

fn gfx950_refinement_identity(
    compiler: CompilerInstructionSelectionCorrespondenceIdentityV1,
    isa: Gfx942DecodedIsaTranscriptIdentityV1,
) -> Gfx950MachineRefinementIdentityV1 {
    let mut digest = Sha256::new();
    digest.update((GFX950_REFINEMENT_IDENTITY_DOMAIN_V1.len() as u32).to_le_bytes());
    digest.update(GFX950_REFINEMENT_IDENTITY_DOMAIN_V1);
    digest.update(compiler.sha256());
    digest.update(compiler.byte_len().to_le_bytes());
    digest.update(isa.sha256());
    digest.update(isa.byte_len().to_le_bytes());
    Gfx950MachineRefinementIdentityV1 {
        sha256: digest.finalize().into(),
    }
}

fn isa_identity(bytes: &[u8]) -> Gfx942DecodedIsaTranscriptIdentityV1 {
    let mut digest = Sha256::new();
    digest.update((ISA_IDENTITY_DOMAIN_V1.len() as u32).to_le_bytes());
    digest.update(ISA_IDENTITY_DOMAIN_V1);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    Gfx942DecodedIsaTranscriptIdentityV1 {
        sha256: digest.finalize().into(),
        byte_len: bytes.len() as u64,
    }
}

fn encode_isa_parts(
    parts: &Gfx942DecodedIsaTranscriptPartsV1,
) -> Result<Vec<u8>, Gfx942MachineRefinementErrorV1> {
    let mut output = Vec::new();
    output.extend_from_slice(&GFX942_DECODED_ISA_TRANSCRIPT_MAGIC_V1);
    put_u16(&mut output, GFX942_DECODED_ISA_TRANSCRIPT_VERSION_V1);
    for identity in [parts.generated_object, parts.final_code_object] {
        output.extend_from_slice(&identity.sha256());
        put_u64(&mut output, identity.byte_len());
    }
    for value in [
        parts.mode.ieee,
        parts.mode.fp32_denormals,
        parts.mode.round_to_nearest_even,
        parts.mode.dx10_clamp,
    ] {
        output.push(u8::from(value));
    }
    put_u32(&mut output, parts.blocks.len() as u32);
    for block in &parts.blocks {
        put_u32(&mut output, block.function);
        put_u32(&mut output, block.machine_block);
        put_u32(&mut output, block.llvm_block);
        put_u64(&mut output, block.first_instruction);
        put_u32(&mut output, block.instruction_count);
        put_u32(&mut output, block.successors.len() as u32);
        for successor in &block.successors {
            put_u32(&mut output, *successor);
        }
    }
    put_u32(&mut output, parts.instructions.len() as u32);
    for instruction in &parts.instructions {
        encode_instruction(&mut output, instruction);
    }
    if output.len() > MAX_GFX942_DECODED_ISA_TRANSCRIPT_BYTES_V1 {
        return Err(Gfx942MachineRefinementErrorV1::ResourceLimit);
    }
    Ok(output)
}

fn encode_instruction(output: &mut Vec<u8>, instruction: &Gfx942DecodedInstructionV1) {
    put_u32(output, instruction.function);
    put_u32(output, instruction.machine_block);
    put_u64(output, instruction.object_offset);
    put_u64(output, instruction.hsaco_offset);
    put_u32(output, instruction.native_opcode);
    output.push(opcode_tag(instruction.semantics));
    put_u32(output, instruction.semantic_discriminator);
    put_bytes(output, &instruction.object_encoding);
    put_bytes(output, &instruction.hsaco_encoding);
    put_u16(output, instruction.definitions.len() as u16);
    for register in &instruction.definitions {
        encode_register(output, *register);
    }
    put_u16(output, instruction.uses.len() as u16);
    for register in &instruction.uses {
        encode_register(output, *register);
    }
    match instruction.branch_target {
        Some(target) => {
            output.push(1);
            put_u64(output, target);
        }
        None => output.push(0),
    }
    encode_memory(output, instruction.memory);
    encode_address(output, instruction.effective_address);
}

fn decode_instruction(
    input: &mut Input<'_>,
) -> Result<Gfx942DecodedInstructionV1, Gfx942MachineRefinementErrorV1> {
    let function = input.u32()?;
    let machine_block = input.u32()?;
    let object_offset = input.u64()?;
    let hsaco_offset = input.u64()?;
    let native_opcode = input.u32()?;
    let semantics = decode_opcode(input.u8()?)?;
    let semantic_discriminator = input.u32()?;
    let object_encoding = input.bytes(MAX_GFX942_INSTRUCTION_BYTES_V1)?;
    let hsaco_encoding = input.bytes(MAX_GFX942_INSTRUCTION_BYTES_V1)?;
    let definition_count = input.short_count(MAX_GFX942_INSTRUCTION_REGISTERS_V1)?;
    let mut definitions = Vec::with_capacity(definition_count);
    for _ in 0..definition_count {
        definitions.push(decode_register(input)?);
    }
    let use_count = input.short_count(MAX_GFX942_INSTRUCTION_REGISTERS_V1)?;
    let mut uses = Vec::with_capacity(use_count);
    for _ in 0..use_count {
        uses.push(decode_register(input)?);
    }
    let branch_target = match input.u8()? {
        0 => None,
        1 => Some(input.u64()?),
        _ => return Err(Gfx942MachineRefinementErrorV1::UnknownTag),
    };
    let memory = decode_memory(input)?;
    let effective_address = decode_address(input)?;
    Gfx942DecodedInstructionV1::new(
        function,
        machine_block,
        object_offset,
        hsaco_offset,
        native_opcode,
        semantics,
        semantic_discriminator,
        object_encoding,
        hsaco_encoding,
        definitions,
        uses,
        branch_target,
        memory,
        effective_address,
    )
}

fn opcode_tag(opcode: Gfx942SemanticOpcodeV1) -> u8 {
    use Gfx942SemanticOpcodeV1 as Opcode;
    match opcode {
        Opcode::Copy => 1,
        Opcode::IntegerAdd => 2,
        Opcode::IntegerSubtract => 3,
        Opcode::IntegerMultiply => 4,
        Opcode::IntegerDivide => 5,
        Opcode::IntegerRemainder => 6,
        Opcode::IntegerBitwise => 7,
        Opcode::IntegerShift => 8,
        Opcode::FloatAdd32 => 9,
        Opcode::FloatSubtract32 => 10,
        Opcode::FloatMultiply32 => 11,
        Opcode::FloatDivide32 => 12,
        Opcode::FloatFma32 => 13,
        Opcode::FloatSqrt32 => 14,
        Opcode::IntegerCompare => 15,
        Opcode::FloatCompare32 => 16,
        Opcode::Cast => 17,
        Opcode::Select => 18,
        Opcode::Address => 19,
        Opcode::Load => 20,
        Opcode::Store => 21,
        Opcode::Atomic => 22,
        Opcode::Fence => 23,
        Opcode::Barrier => 24,
        Opcode::Call => 25,
        Opcode::Branch => 26,
        Opcode::BranchScc => 27,
        Opcode::BranchVcc => 28,
        Opcode::ExecAndSave => 29,
        Opcode::ExecAndNotSave => 30,
        Opcode::ExecRestore => 31,
        Opcode::BranchExecZero => 32,
        Opcode::BranchExecNonZero => 33,
        Opcode::Return => 34,
        Opcode::InvocationIndex => 35,
        Opcode::Collective => 36,
        Opcode::Matrix => 37,
        Opcode::InlineAssembly => 38,
        Opcode::Trap => 39,
    }
}

fn decode_opcode(tag: u8) -> Result<Gfx942SemanticOpcodeV1, Gfx942MachineRefinementErrorV1> {
    use Gfx942SemanticOpcodeV1 as Opcode;
    Ok(match tag {
        1 => Opcode::Copy,
        2 => Opcode::IntegerAdd,
        3 => Opcode::IntegerSubtract,
        4 => Opcode::IntegerMultiply,
        5 => Opcode::IntegerDivide,
        6 => Opcode::IntegerRemainder,
        7 => Opcode::IntegerBitwise,
        8 => Opcode::IntegerShift,
        9 => Opcode::FloatAdd32,
        10 => Opcode::FloatSubtract32,
        11 => Opcode::FloatMultiply32,
        12 => Opcode::FloatDivide32,
        13 => Opcode::FloatFma32,
        14 => Opcode::FloatSqrt32,
        15 => Opcode::IntegerCompare,
        16 => Opcode::FloatCompare32,
        17 => Opcode::Cast,
        18 => Opcode::Select,
        19 => Opcode::Address,
        20 => Opcode::Load,
        21 => Opcode::Store,
        22 => Opcode::Atomic,
        23 => Opcode::Fence,
        24 => Opcode::Barrier,
        25 => Opcode::Call,
        26 => Opcode::Branch,
        27 => Opcode::BranchScc,
        28 => Opcode::BranchVcc,
        29 => Opcode::ExecAndSave,
        30 => Opcode::ExecAndNotSave,
        31 => Opcode::ExecRestore,
        32 => Opcode::BranchExecZero,
        33 => Opcode::BranchExecNonZero,
        34 => Opcode::Return,
        35 => Opcode::InvocationIndex,
        36 => Opcode::Collective,
        37 => Opcode::Matrix,
        38 => Opcode::InlineAssembly,
        39 => Opcode::Trap,
        _ => return Err(Gfx942MachineRefinementErrorV1::UnknownTag),
    })
}

fn encode_register(output: &mut Vec<u8>, register: Gfx942SemanticRegisterV1) {
    match register {
        Gfx942SemanticRegisterV1::Scalar(index) => {
            output.push(1);
            put_u16(output, index);
        }
        Gfx942SemanticRegisterV1::Vector(index) => {
            output.push(2);
            put_u16(output, index);
        }
        Gfx942SemanticRegisterV1::Accumulator(index) => {
            output.push(3);
            put_u16(output, index);
        }
        Gfx942SemanticRegisterV1::Exec => output.push(4),
        Gfx942SemanticRegisterV1::Vcc => output.push(5),
        Gfx942SemanticRegisterV1::Scc => output.push(6),
        Gfx942SemanticRegisterV1::M0 => output.push(7),
        Gfx942SemanticRegisterV1::Mode => output.push(8),
    }
}

fn decode_register(
    input: &mut Input<'_>,
) -> Result<Gfx942SemanticRegisterV1, Gfx942MachineRefinementErrorV1> {
    Ok(match input.u8()? {
        1 => Gfx942SemanticRegisterV1::Scalar(input.u16()?),
        2 => Gfx942SemanticRegisterV1::Vector(input.u16()?),
        3 => Gfx942SemanticRegisterV1::Accumulator(input.u16()?),
        4 => Gfx942SemanticRegisterV1::Exec,
        5 => Gfx942SemanticRegisterV1::Vcc,
        6 => Gfx942SemanticRegisterV1::Scc,
        7 => Gfx942SemanticRegisterV1::M0,
        8 => Gfx942SemanticRegisterV1::Mode,
        _ => return Err(Gfx942MachineRefinementErrorV1::UnknownTag),
    })
}

fn encode_memory(output: &mut Vec<u8>, memory: CompilerMemoryEffectV1) {
    output.push(match memory.kind() {
        CompilerMemoryEffectKindV1::None => 0,
        CompilerMemoryEffectKindV1::Read => 1,
        CompilerMemoryEffectKindV1::Write => 2,
        CompilerMemoryEffectKindV1::ReadWrite => 3,
        CompilerMemoryEffectKindV1::Fence => 4,
        CompilerMemoryEffectKindV1::Barrier => 5,
    });
    put_u32(output, memory.address_space());
    put_u32(output, memory.byte_width());
    put_u32(output, memory.alignment());
    output.push(memory_ordering_tag(memory.ordering()));
    output.push(memory_scope_tag(memory.scope()));
    output.push(u8::from(memory.is_volatile()));
}

fn decode_memory(
    input: &mut Input<'_>,
) -> Result<CompilerMemoryEffectV1, Gfx942MachineRefinementErrorV1> {
    use fe2o3_compiler_lineage::{
        CompilerMemoryOrderingV1 as Ordering, CompilerMemoryScopeV1 as Scope,
    };
    let kind = match input.u8()? {
        0 => CompilerMemoryEffectKindV1::None,
        1 => CompilerMemoryEffectKindV1::Read,
        2 => CompilerMemoryEffectKindV1::Write,
        3 => CompilerMemoryEffectKindV1::ReadWrite,
        4 => CompilerMemoryEffectKindV1::Fence,
        5 => CompilerMemoryEffectKindV1::Barrier,
        _ => return Err(Gfx942MachineRefinementErrorV1::UnknownTag),
    };
    let address_space = input.u32()?;
    let byte_width = input.u32()?;
    let alignment = input.u32()?;
    let ordering = match input.u8()? {
        0 => Ordering::NotAtomic,
        1 => Ordering::Relaxed,
        2 => Ordering::Acquire,
        3 => Ordering::Release,
        4 => Ordering::AcquireRelease,
        5 => Ordering::SequentiallyConsistent,
        _ => return Err(Gfx942MachineRefinementErrorV1::UnknownTag),
    };
    let scope = match input.u8()? {
        0 => Scope::None,
        1 => Scope::Invocation,
        2 => Scope::Subgroup,
        3 => Scope::Workgroup,
        4 => Scope::Agent,
        5 => Scope::System,
        _ => return Err(Gfx942MachineRefinementErrorV1::UnknownTag),
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
    .map_err(|_| Gfx942MachineRefinementErrorV1::AddressOrMemoryMismatch)
}

fn memory_ordering_tag(ordering: fe2o3_compiler_lineage::CompilerMemoryOrderingV1) -> u8 {
    use fe2o3_compiler_lineage::CompilerMemoryOrderingV1 as Ordering;
    match ordering {
        Ordering::NotAtomic => 0,
        Ordering::Relaxed => 1,
        Ordering::Acquire => 2,
        Ordering::Release => 3,
        Ordering::AcquireRelease => 4,
        Ordering::SequentiallyConsistent => 5,
    }
}

fn memory_scope_tag(scope: fe2o3_compiler_lineage::CompilerMemoryScopeV1) -> u8 {
    use fe2o3_compiler_lineage::CompilerMemoryScopeV1 as Scope;
    match scope {
        Scope::None => 0,
        Scope::Invocation => 1,
        Scope::Subgroup => 2,
        Scope::Workgroup => 3,
        Scope::Agent => 4,
        Scope::System => 5,
    }
}

fn encode_address(output: &mut Vec<u8>, address: Gfx942EffectiveAddressV1) {
    match address {
        Gfx942EffectiveAddressV1::None => output.push(0),
        Gfx942EffectiveAddressV1::BaseIndex {
            base,
            index,
            scale,
            displacement,
            address_space,
            pointer_bits,
            byte_width,
        } => {
            output.push(1);
            encode_register(output, base);
            match index {
                Some(index) => {
                    output.push(1);
                    encode_register(output, index);
                }
                None => output.push(0),
            }
            put_u32(output, scale);
            output.extend_from_slice(&displacement.to_le_bytes());
            put_u32(output, address_space);
            put_u16(output, pointer_bits);
            put_u32(output, byte_width);
        }
    }
}

fn decode_address(
    input: &mut Input<'_>,
) -> Result<Gfx942EffectiveAddressV1, Gfx942MachineRefinementErrorV1> {
    Ok(match input.u8()? {
        0 => Gfx942EffectiveAddressV1::None,
        1 => {
            let base = decode_register(input)?;
            let index = match input.u8()? {
                0 => None,
                1 => Some(decode_register(input)?),
                _ => return Err(Gfx942MachineRefinementErrorV1::UnknownTag),
            };
            Gfx942EffectiveAddressV1::BaseIndex {
                base,
                index,
                scale: input.u32()?,
                displacement: input.i64()?,
                address_space: input.u32()?,
                pointer_bits: input.u16()?,
                byte_width: input.u32()?,
            }
        }
        _ => return Err(Gfx942MachineRefinementErrorV1::UnknownTag),
    })
}

fn put_bytes(output: &mut Vec<u8>, bytes: &[u8]) {
    put_u16(output, bytes.len() as u16);
    output.extend_from_slice(bytes);
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

fn has_duplicate<T: Ord>(values: impl Iterator<Item = T>) -> bool {
    let mut seen = BTreeSet::new();
    values.into_iter().any(|value| !seen.insert(value))
}

struct Input<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Input<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], Gfx942MachineRefinementErrorV1> {
        let end = self
            .cursor
            .checked_add(length)
            .ok_or(Gfx942MachineRefinementErrorV1::Truncated)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(Gfx942MachineRefinementErrorV1::Truncated)?;
        self.cursor = end;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, Gfx942MachineRefinementErrorV1> {
        Ok(self.take(1)?[0])
    }

    fn boolean(&mut self) -> Result<bool, Gfx942MachineRefinementErrorV1> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Gfx942MachineRefinementErrorV1::UnknownTag),
        }
    }

    fn u16(&mut self) -> Result<u16, Gfx942MachineRefinementErrorV1> {
        Ok(u16::from_le_bytes(
            self.take(2)?
                .try_into()
                .map_err(|_| Gfx942MachineRefinementErrorV1::Truncated)?,
        ))
    }

    fn u32(&mut self) -> Result<u32, Gfx942MachineRefinementErrorV1> {
        Ok(u32::from_le_bytes(
            self.take(4)?
                .try_into()
                .map_err(|_| Gfx942MachineRefinementErrorV1::Truncated)?,
        ))
    }

    fn u64(&mut self) -> Result<u64, Gfx942MachineRefinementErrorV1> {
        Ok(u64::from_le_bytes(
            self.take(8)?
                .try_into()
                .map_err(|_| Gfx942MachineRefinementErrorV1::Truncated)?,
        ))
    }

    fn i64(&mut self) -> Result<i64, Gfx942MachineRefinementErrorV1> {
        Ok(i64::from_le_bytes(
            self.take(8)?
                .try_into()
                .map_err(|_| Gfx942MachineRefinementErrorV1::Truncated)?,
        ))
    }

    fn count(&mut self, maximum: usize) -> Result<usize, Gfx942MachineRefinementErrorV1> {
        let count = usize::try_from(self.u32()?)
            .map_err(|_| Gfx942MachineRefinementErrorV1::ResourceLimit)?;
        if count > maximum {
            return Err(Gfx942MachineRefinementErrorV1::ResourceLimit);
        }
        Ok(count)
    }

    fn short_count(&mut self, maximum: usize) -> Result<usize, Gfx942MachineRefinementErrorV1> {
        let count = usize::from(self.u16()?);
        if count > maximum {
            return Err(Gfx942MachineRefinementErrorV1::ResourceLimit);
        }
        Ok(count)
    }

    fn bytes(&mut self, maximum: usize) -> Result<Box<[u8]>, Gfx942MachineRefinementErrorV1> {
        let length = self.short_count(maximum)?;
        if length == 0 {
            return Err(Gfx942MachineRefinementErrorV1::NonCanonical);
        }
        Ok(self.take(length)?.to_vec().into_boxed_slice())
    }

    fn content_identity(
        &mut self,
    ) -> Result<ExactCompilerStageContentIdentityV1, Gfx942MachineRefinementErrorV1> {
        let digest: [u8; 32] = self
            .take(32)?
            .try_into()
            .map_err(|_| Gfx942MachineRefinementErrorV1::Truncated)?;
        let byte_len = self.u64()?;
        fe2o3_compiler_lineage::decode_exact_compiler_stage_content_identity_v1(digest, byte_len)
            .ok_or(Gfx942MachineRefinementErrorV1::NonCanonical)
    }

    fn finish(self) -> Result<(), Gfx942MachineRefinementErrorV1> {
        if self.cursor == self.bytes.len() {
            Ok(())
        } else {
            Err(Gfx942MachineRefinementErrorV1::NonCanonical)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_compiler_lineage::{
        CompilerAddressTermV1, CompilerEffectiveAddressV1,
        CompilerInstructionSelectionCorrespondencePartsV1,
        CompilerInstructionSelectionCorrespondenceV1, CompilerLlvmOperationCoordinateV1,
        CompilerLlvmValueTypeV1, CompilerMachineValueBindingV1, CompilerMemoryOrderingV1,
        CompilerMemoryScopeV1, CompilerPhiEdgeTransportV1, CompilerPhiInputV1,
    };

    fn memory(kind: CompilerMemoryEffectKindV1) -> CompilerMemoryEffectV1 {
        CompilerMemoryEffectV1::new(
            kind,
            1,
            4,
            4,
            fe2o3_compiler_lineage::CompilerMemoryOrderingV1::NotAtomic,
            fe2o3_compiler_lineage::CompilerMemoryScopeV1::None,
            false,
        )
        .unwrap()
    }

    #[allow(clippy::too_many_arguments)]
    fn llvm_operation(
        block: u32,
        ordinal: u32,
        kind: CompilerLlvmOperationKindV1,
        result: Option<u32>,
        operands: &[u32],
        phi: &[CompilerPhiInputV1],
        successors: &[u32],
        divergence: CompilerBranchDivergenceV1,
        memory: CompilerMemoryEffectV1,
        numerical: CompilerNumericalContractV1,
        offsets: &[u64],
    ) -> CompilerLlvmOperationV1 {
        let register = |class, index| {
            CompilerMachineValueLocationV1::Register(CompilerMachineRegisterV1::new(class, index))
        };
        let binding = |offset, value, access, location| {
            CompilerMachineValueBindingV1::new(offset, value, access, location).unwrap()
        };
        let (mut bindings, effective_address) = match (block, ordinal, kind) {
            (1, 1, CompilerLlvmOperationKindV1::Load) => (
                vec![
                    binding(
                        20,
                        1,
                        CompilerMachineValueAccessV1::Use,
                        register(CompilerMachineRegisterClassV1::Vector, 4),
                    ),
                    binding(
                        20,
                        1,
                        CompilerMachineValueAccessV1::Use,
                        register(CompilerMachineRegisterClassV1::Vector, 5),
                    ),
                    binding(
                        20,
                        2,
                        CompilerMachineValueAccessV1::Definition,
                        register(CompilerMachineRegisterClassV1::Vector, 1),
                    ),
                ],
                Some(
                    CompilerEffectiveAddressV1::new(
                        1,
                        64,
                        4,
                        0,
                        [CompilerAddressTermV1::new(1, 1).unwrap()],
                    )
                    .unwrap(),
                ),
            ),
            (1, 2, CompilerLlvmOperationKindV1::ConditionalBranch) => (
                vec![binding(
                    28,
                    2,
                    CompilerMachineValueAccessV1::Use,
                    register(CompilerMachineRegisterClassV1::VectorCondition, 0),
                )],
                None,
            ),
            (2, 0, CompilerLlvmOperationKindV1::FloatAdd) => (
                vec![
                    binding(
                        36,
                        1,
                        CompilerMachineValueAccessV1::Use,
                        register(CompilerMachineRegisterClassV1::Vector, 2),
                    ),
                    binding(
                        36,
                        2,
                        CompilerMachineValueAccessV1::Use,
                        register(CompilerMachineRegisterClassV1::Vector, 1),
                    ),
                    binding(
                        36,
                        3,
                        CompilerMachineValueAccessV1::Definition,
                        register(CompilerMachineRegisterClassV1::Vector, 2),
                    ),
                ],
                None,
            ),
            (2, 1, CompilerLlvmOperationKindV1::Store) => (
                vec![
                    binding(
                        40,
                        1,
                        CompilerMachineValueAccessV1::Use,
                        register(CompilerMachineRegisterClassV1::Vector, 4),
                    ),
                    binding(
                        40,
                        1,
                        CompilerMachineValueAccessV1::Use,
                        register(CompilerMachineRegisterClassV1::Vector, 5),
                    ),
                    binding(
                        40,
                        3,
                        CompilerMachineValueAccessV1::Use,
                        register(CompilerMachineRegisterClassV1::Vector, 2),
                    ),
                ],
                Some(
                    CompilerEffectiveAddressV1::new(
                        1,
                        64,
                        4,
                        0,
                        [CompilerAddressTermV1::new(1, 1).unwrap()],
                    )
                    .unwrap(),
                ),
            ),
            _ => (Vec::new(), None),
        };
        bindings.sort_unstable();
        let phi_result = CompilerMachineRegisterV1::new(
            CompilerMachineRegisterClassV1::Vector,
            result.unwrap_or_default() as u16,
        );
        let phi_transports = phi
            .iter()
            .map(|input| {
                CompilerPhiEdgeTransportV1::new(
                    input.predecessor(),
                    input.value(),
                    CompilerMachineValueLocationV1::Register(phi_result),
                    phi_result,
                    None,
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        CompilerLlvmOperationV1::new(
            CompilerLlvmOperationCoordinateV1::new(0, block, ordinal),
            200 + ordinal as u16,
            kind,
            0,
            result,
            result.map_or(CompilerLlvmValueTypeV1::Void, |_| {
                CompilerLlvmValueTypeV1::Float(32)
            }),
            operands,
            phi,
            phi_transports,
            successors,
            divergence,
            memory,
            numerical,
            offsets,
            bindings,
            effective_address,
        )
        .unwrap()
    }

    fn compiler() -> CompilerInstructionSelectionCorrespondenceV1 {
        let blocks = vec![
            CompilerLlvmBlockV1::new(0, 0, [1]).unwrap(),
            CompilerLlvmBlockV1::new(0, 1, [2, 3]).unwrap(),
            CompilerLlvmBlockV1::new(0, 2, [1]).unwrap(),
            CompilerLlvmBlockV1::new(0, 3, []).unwrap(),
        ];
        let operations = vec![
            llvm_operation(
                0,
                0,
                CompilerLlvmOperationKindV1::Constant,
                Some(0),
                &[],
                &[],
                &[],
                CompilerBranchDivergenceV1::None,
                CompilerMemoryEffectV1::none(),
                CompilerNumericalContractV1::Exact,
                &[],
            ),
            llvm_operation(
                0,
                1,
                CompilerLlvmOperationKindV1::Branch,
                None,
                &[],
                &[],
                &[1],
                CompilerBranchDivergenceV1::None,
                CompilerMemoryEffectV1::none(),
                CompilerNumericalContractV1::Exact,
                &[16],
            ),
            llvm_operation(
                1,
                0,
                CompilerLlvmOperationKindV1::Phi,
                Some(1),
                &[],
                &[CompilerPhiInputV1::new(0, 0), CompilerPhiInputV1::new(2, 3)],
                &[],
                CompilerBranchDivergenceV1::None,
                CompilerMemoryEffectV1::none(),
                CompilerNumericalContractV1::Exact,
                &[],
            ),
            llvm_operation(
                1,
                1,
                CompilerLlvmOperationKindV1::Load,
                Some(2),
                &[1],
                &[],
                &[],
                CompilerBranchDivergenceV1::None,
                memory(CompilerMemoryEffectKindV1::Read),
                CompilerNumericalContractV1::Exact,
                &[20],
            ),
            llvm_operation(
                1,
                2,
                CompilerLlvmOperationKindV1::ConditionalBranch,
                None,
                &[2],
                &[],
                &[2, 3],
                CompilerBranchDivergenceV1::Divergent,
                CompilerMemoryEffectV1::none(),
                CompilerNumericalContractV1::Exact,
                &[28, 32, 52],
            ),
            llvm_operation(
                2,
                0,
                CompilerLlvmOperationKindV1::FloatAdd,
                Some(3),
                &[2, 1],
                &[],
                &[],
                CompilerBranchDivergenceV1::None,
                CompilerMemoryEffectV1::none(),
                CompilerNumericalContractV1::IeeeBinary32,
                &[36],
            ),
            llvm_operation(
                2,
                1,
                CompilerLlvmOperationKindV1::Store,
                None,
                &[1, 3],
                &[],
                &[],
                CompilerBranchDivergenceV1::None,
                memory(CompilerMemoryEffectKindV1::Write),
                CompilerNumericalContractV1::Exact,
                &[40],
            ),
            llvm_operation(
                2,
                2,
                CompilerLlvmOperationKindV1::Branch,
                None,
                &[],
                &[],
                &[1],
                CompilerBranchDivergenceV1::None,
                CompilerMemoryEffectV1::none(),
                CompilerNumericalContractV1::Exact,
                &[48],
            ),
            llvm_operation(
                3,
                0,
                CompilerLlvmOperationKindV1::Return,
                None,
                &[],
                &[],
                &[],
                CompilerBranchDivergenceV1::None,
                CompilerMemoryEffectV1::none(),
                CompilerNumericalContractV1::Exact,
                &[56],
            ),
        ];
        CompilerInstructionSelectionCorrespondenceV1::from_parts(
            CompilerInstructionSelectionCorrespondencePartsV1 {
                architecture: MachineRefinementArchitectureV1::AmdGcn,
                compiler_occurrence_identity: [1; 32],
                worker_request_identity: [2; 32],
                worker_response_identity: [3; 32],
                post_optimization_bitcode: ExactCompilerStageContentIdentityV1::calculate(
                    b"BC\xc0\xde-post",
                )
                .unwrap(),
                generated_object: ExactCompilerStageContentIdentityV1::calculate(&artifact_bytes())
                    .unwrap(),
                final_code_object:
                    ExactCompilerStageContentIdentityV1::calculate(&artifact_bytes()).unwrap(),
                blocks: blocks.into_boxed_slice(),
                operations: operations.into_boxed_slice(),
            },
        )
        .unwrap()
    }

    fn artifact_bytes() -> Vec<u8> {
        let mut bytes = vec![0_u8; 64];
        for (offset, encoding) in [
            (16, &[0x00, 0x00, 0x82, 0xbf][..]),
            (20, &[0x00, 0x80, 0x50, 0xdc, 0x04, 0x00, 0x7f, 0x01][..]),
            (28, &[0x6a, 0x20, 0x88, 0xbe][..]),
            (32, &[0x04, 0x00, 0x88, 0xbf][..]),
            (36, &[0x01, 0x05, 0x04, 0x02][..]),
            (40, &[0x00, 0x80, 0x70, 0xdc, 0x04, 0x02, 0x7f, 0x00][..]),
            (48, &[0xf8, 0xff, 0x82, 0xbf][..]),
            (52, &[0x08, 0x01, 0xfe, 0xbe][..]),
            (56, &[0x00, 0x00, 0x81, 0xbf][..]),
        ] {
            bytes[offset..offset + encoding.len()].copy_from_slice(encoding);
        }
        bytes
    }

    fn executable_elf(code: &[u8]) -> Vec<u8> {
        let section_names = b"\0.text\0.shstrtab\0";
        let text_offset = 64_usize;
        let names_offset = text_offset + code.len();
        let section_table = (names_offset + section_names.len() + 7) & !7;
        let mut bytes = vec![0_u8; section_table + 3 * 64];
        bytes[..4].copy_from_slice(b"\x7fELF");
        bytes[4] = 2;
        bytes[5] = 1;
        bytes[6] = 1;
        bytes[40..48].copy_from_slice(&(section_table as u64).to_le_bytes());
        bytes[58..60].copy_from_slice(&64_u16.to_le_bytes());
        bytes[60..62].copy_from_slice(&3_u16.to_le_bytes());
        bytes[62..64].copy_from_slice(&2_u16.to_le_bytes());
        bytes[text_offset..text_offset + code.len()].copy_from_slice(code);
        bytes[names_offset..names_offset + section_names.len()].copy_from_slice(section_names);

        let text = section_table + 64;
        bytes[text..text + 4].copy_from_slice(&1_u32.to_le_bytes());
        bytes[text + 4..text + 8].copy_from_slice(&1_u32.to_le_bytes());
        bytes[text + 8..text + 16].copy_from_slice(&6_u64.to_le_bytes());
        bytes[text + 24..text + 32].copy_from_slice(&(text_offset as u64).to_le_bytes());
        bytes[text + 32..text + 40].copy_from_slice(&(code.len() as u64).to_le_bytes());
        bytes[text + 48..text + 56].copy_from_slice(&4_u64.to_le_bytes());

        let names = section_table + 128;
        bytes[names..names + 4].copy_from_slice(&7_u32.to_le_bytes());
        bytes[names + 4..names + 8].copy_from_slice(&3_u32.to_le_bytes());
        bytes[names + 24..names + 32].copy_from_slice(&(names_offset as u64).to_le_bytes());
        bytes[names + 32..names + 40].copy_from_slice(&(section_names.len() as u64).to_le_bytes());
        bytes[names + 48..names + 56].copy_from_slice(&1_u64.to_le_bytes());
        bytes
    }

    fn address() -> Gfx942EffectiveAddressV1 {
        Gfx942EffectiveAddressV1::BaseIndex {
            base: Gfx942SemanticRegisterV1::Vector(4),
            index: None,
            scale: 1,
            displacement: 0,
            address_space: 1,
            pointer_bits: 64,
            byte_width: 4,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn instruction(
        block: u32,
        offset: u64,
        semantics: Gfx942SemanticOpcodeV1,
        definitions: &[Gfx942SemanticRegisterV1],
        uses: &[Gfx942SemanticRegisterV1],
        target: Option<u64>,
        memory: CompilerMemoryEffectV1,
        address: Gfx942EffectiveAddressV1,
    ) -> Gfx942DecodedInstructionV1 {
        let bytes = artifact_bytes();
        let word = u32::from_le_bytes(
            bytes[offset as usize..offset as usize + 4]
                .try_into()
                .unwrap(),
        );
        let encoding_len = if word & 0xfc00_0000 == 0xdc00_0000 {
            8
        } else {
            4
        };
        let encoding = &bytes[offset as usize..offset as usize + encoding_len];
        let native_opcode = decode_gfx942_instruction_v1(offset, encoding)
            .unwrap()
            .native_opcode;
        Gfx942DecodedInstructionV1::new(
            0,
            block,
            offset,
            offset,
            native_opcode,
            semantics,
            0,
            encoding,
            encoding,
            definitions,
            uses,
            target,
            memory,
            address,
        )
        .unwrap()
    }

    fn isa_parts() -> Gfx942DecodedIsaTranscriptPartsV1 {
        let blocks = vec![
            Gfx942DecodedMachineBlockV1::new(0, 0, 0, 16, 1, [1]).unwrap(),
            Gfx942DecodedMachineBlockV1::new(0, 1, 1, 20, 3, [2, 3]).unwrap(),
            Gfx942DecodedMachineBlockV1::new(0, 2, 2, 36, 3, [1]).unwrap(),
            Gfx942DecodedMachineBlockV1::new(0, 3, 3, 52, 2, []).unwrap(),
        ];
        let instructions = vec![
            instruction(
                0,
                16,
                Gfx942SemanticOpcodeV1::Branch,
                &[],
                &[],
                Some(20),
                CompilerMemoryEffectV1::none(),
                Gfx942EffectiveAddressV1::None,
            ),
            instruction(
                1,
                20,
                Gfx942SemanticOpcodeV1::Load,
                &[Gfx942SemanticRegisterV1::Vector(1)],
                &[
                    Gfx942SemanticRegisterV1::Vector(4),
                    Gfx942SemanticRegisterV1::Vector(5),
                ],
                None,
                memory(CompilerMemoryEffectKindV1::Read),
                address(),
            ),
            instruction(
                1,
                28,
                Gfx942SemanticOpcodeV1::ExecAndSave,
                &[
                    Gfx942SemanticRegisterV1::Exec,
                    Gfx942SemanticRegisterV1::Scalar(8),
                ],
                &[
                    Gfx942SemanticRegisterV1::Exec,
                    Gfx942SemanticRegisterV1::Vcc,
                ],
                None,
                CompilerMemoryEffectV1::none(),
                Gfx942EffectiveAddressV1::None,
            ),
            instruction(
                1,
                32,
                Gfx942SemanticOpcodeV1::BranchExecZero,
                &[],
                &[Gfx942SemanticRegisterV1::Exec],
                Some(52),
                CompilerMemoryEffectV1::none(),
                Gfx942EffectiveAddressV1::None,
            ),
            instruction(
                2,
                36,
                Gfx942SemanticOpcodeV1::FloatAdd32,
                &[Gfx942SemanticRegisterV1::Vector(2)],
                &[
                    Gfx942SemanticRegisterV1::Vector(1),
                    Gfx942SemanticRegisterV1::Vector(2),
                ],
                None,
                CompilerMemoryEffectV1::none(),
                Gfx942EffectiveAddressV1::None,
            ),
            instruction(
                2,
                40,
                Gfx942SemanticOpcodeV1::Store,
                &[],
                &[
                    Gfx942SemanticRegisterV1::Vector(4),
                    Gfx942SemanticRegisterV1::Vector(5),
                    Gfx942SemanticRegisterV1::Vector(2),
                ],
                None,
                memory(CompilerMemoryEffectKindV1::Write),
                address(),
            ),
            instruction(
                2,
                48,
                Gfx942SemanticOpcodeV1::Branch,
                &[],
                &[],
                Some(20),
                CompilerMemoryEffectV1::none(),
                Gfx942EffectiveAddressV1::None,
            ),
            instruction(
                3,
                52,
                Gfx942SemanticOpcodeV1::ExecRestore,
                &[Gfx942SemanticRegisterV1::Exec],
                &[Gfx942SemanticRegisterV1::Scalar(8)],
                None,
                CompilerMemoryEffectV1::none(),
                Gfx942EffectiveAddressV1::None,
            ),
            instruction(
                3,
                56,
                Gfx942SemanticOpcodeV1::Return,
                &[],
                &[],
                None,
                CompilerMemoryEffectV1::none(),
                Gfx942EffectiveAddressV1::None,
            ),
        ];
        let bytes = artifact_bytes();
        Gfx942DecodedIsaTranscriptPartsV1 {
            generated_object: ExactCompilerStageContentIdentityV1::calculate(&bytes).unwrap(),
            final_code_object: ExactCompilerStageContentIdentityV1::calculate(&bytes).unwrap(),
            mode: Gfx942FloatingModeV1::strict_ieee_binary32(),
            blocks: blocks.into_boxed_slice(),
            instructions: instructions.into_boxed_slice(),
        }
    }

    #[test]
    fn canonical_decode_and_semantic_validation_cover_exec_loop_address_and_ieee() {
        let isa = Gfx942DecodedIsaTranscriptV1::from_parts(isa_parts()).unwrap();
        let decoded =
            Gfx942DecodedIsaTranscriptV1::decode_canonical(isa.canonical_bytes()).unwrap();
        assert_eq!(decoded, isa);
        assert_eq!(validate_correspondence(&compiler(), &decoded).unwrap(), 1);
        assert!(decoded.mode().supports_strict_binary32());
        assert!(!decoded.proves_compiler_refinement());
    }

    #[test]
    fn mode_exec_address_and_branch_mutations_fail_closed() {
        let compiler = compiler();

        let mut bad_mode = isa_parts();
        bad_mode.mode = Gfx942FloatingModeV1::new(true, false, true, false);
        let bad_mode = Gfx942DecodedIsaTranscriptV1::from_parts(bad_mode).unwrap();
        assert_eq!(
            validate_correspondence(&compiler, &bad_mode).unwrap_err(),
            Gfx942MachineRefinementErrorV1::ModeOrIeeeMismatch
        );

        let mut bad_exec = isa_parts();
        bad_exec.instructions[7].semantics = Gfx942SemanticOpcodeV1::Copy;
        assert_eq!(
            Gfx942DecodedIsaTranscriptV1::from_parts(bad_exec).unwrap_err(),
            Gfx942MachineRefinementErrorV1::InstructionDecodeMismatch
        );

        let mut bad_address = isa_parts();
        bad_address.instructions[1].memory = CompilerMemoryEffectV1::new(
            CompilerMemoryEffectKindV1::Read,
            3,
            4,
            4,
            CompilerMemoryOrderingV1::NotAtomic,
            CompilerMemoryScopeV1::None,
            false,
        )
        .unwrap();
        bad_address.instructions[1].effective_address = Gfx942EffectiveAddressV1::BaseIndex {
            base: Gfx942SemanticRegisterV1::Vector(4),
            index: None,
            scale: 1,
            displacement: 0,
            address_space: 3,
            pointer_bits: 64,
            byte_width: 4,
        };
        assert_eq!(
            Gfx942DecodedIsaTranscriptV1::from_parts(bad_address).unwrap_err(),
            Gfx942MachineRefinementErrorV1::InstructionDecodeMismatch
        );

        let mut bad_branch = isa_parts();
        bad_branch.instructions[6].branch_target = Some(52);
        assert_eq!(
            Gfx942DecodedIsaTranscriptV1::from_parts(bad_branch).unwrap_err(),
            Gfx942MachineRefinementErrorV1::InstructionDecodeMismatch
        );
    }

    #[test]
    fn caller_semantic_relabel_without_byte_change_is_rejected() {
        let mut relabeled = isa_parts();
        relabeled.instructions[4].semantics = Gfx942SemanticOpcodeV1::IntegerAdd;
        assert_eq!(
            Gfx942DecodedIsaTranscriptV1::from_parts(relabeled).unwrap_err(),
            Gfx942MachineRefinementErrorV1::InstructionDecodeMismatch
        );
    }

    #[test]
    fn real_gfx942_and_gfx950_instruction_bytes_drive_the_common_decoder() {
        // Captured independently with ROCm LLVM 22 llvm-mc --show-encoding for both targets.
        let gfx942 = AmdTargetId::parse(PRODUCTION_GFX942_DEVICE_TARGET_V1).unwrap();
        let gfx950 = AmdTargetId::parse(PRODUCTION_GFX950_DEVICE_TARGET_V1).unwrap();
        let corpus: &[(&[u8], Gfx942SemanticOpcodeV1, u32)] = &[
            (
                &[0x05, 0x06, 0x04, 0x80],
                Gfx942SemanticOpcodeV1::IntegerAdd,
                0,
            ),
            (
                &[0x08, 0x09, 0x87, 0x80],
                Gfx942SemanticOpcodeV1::IntegerSubtract,
                0,
            ),
            (
                &[0x0b, 0x0c, 0x0a, 0x86],
                Gfx942SemanticOpcodeV1::IntegerBitwise,
                DISCRIMINATOR_BITWISE_AND,
            ),
            (
                &[0x05, 0x0d, 0x08, 0x04],
                Gfx942SemanticOpcodeV1::FloatSubtract32,
                0,
            ),
            (
                &[0x10, 0x00, 0x6c, 0xd8, 0x0b, 0x00, 0x00, 0x0a],
                Gfx942SemanticOpcodeV1::Load,
                0,
            ),
            (
                &[0x14, 0x00, 0x1a, 0xd8, 0x0c, 0x0d, 0x00, 0x00],
                Gfx942SemanticOpcodeV1::Store,
                0,
            ),
            (
                &[0x18, 0x00, 0x00, 0xd8, 0x0e, 0x0f, 0x00, 0x00],
                Gfx942SemanticOpcodeV1::Atomic,
                DISCRIMINATOR_ATOMIC_ADD,
            ),
            (
                &[0x00, 0x00, 0x8a, 0xbf],
                Gfx942SemanticOpcodeV1::Barrier,
                0,
            ),
            (
                &[0xfa, 0x24, 0x20, 0x02, 0x11, 0x11, 0x01, 0xff],
                Gfx942SemanticOpcodeV1::Collective,
                0x3fd11,
            ),
            (
                &[0x14, 0x00, 0xe1, 0xd3, 0x18, 0x35, 0x52, 0x04],
                Gfx942SemanticOpcodeV1::Matrix,
                DISCRIMINATOR_MFMA_F32_16X16X16_BF16,
            ),
        ];
        for target in [gfx942, gfx950] {
            for (encoding, semantics, discriminator) in corpus {
                let decoded =
                    independently_decode_amd_instruction_v1(target, 64, encoding).unwrap();
                assert_eq!(decoded.semantics(), *semantics);
                assert_eq!(decoded.semantic_discriminator(), *discriminator);
            }
        }

        let lds_read = independently_decode_amd_instruction_v1(gfx942, 64, corpus[4].0).unwrap();
        assert_eq!(
            lds_read.definitions(),
            &[Gfx942SemanticRegisterV1::Vector(10)]
        );
        assert_eq!(lds_read.uses(), &[Gfx942SemanticRegisterV1::Vector(11)]);
        assert_eq!(lds_read.memory().address_space(), 3);
        assert_eq!(
            lds_read.effective_address(),
            Gfx942EffectiveAddressV1::BaseIndex {
                base: Gfx942SemanticRegisterV1::Vector(11),
                index: None,
                scale: 1,
                displacement: 16,
                address_space: 3,
                pointer_bits: 32,
                byte_width: 4,
            }
        );

        let mfma = independently_decode_amd_instruction_v1(gfx942, 64, corpus[9].0).unwrap();
        assert_eq!(
            mfma.definitions(),
            &[
                Gfx942SemanticRegisterV1::Vector(20),
                Gfx942SemanticRegisterV1::Vector(21),
                Gfx942SemanticRegisterV1::Vector(22),
                Gfx942SemanticRegisterV1::Vector(23),
            ]
        );
        assert_eq!(mfma.uses().len(), 8);
        assert_eq!(mfma.uses()[0], Gfx942SemanticRegisterV1::Vector(24));
        assert_eq!(mfma.uses()[7], Gfx942SemanticRegisterV1::Vector(23));

        // Captured with ROCm LLVM 22 for gfx942; the displacement is signed 13-bit data.
        let negative_global = independently_decode_amd_instruction_v1(
            gfx942,
            64,
            &[0xfc, 0x9f, 0x50, 0xdc, 0x04, 0x00, 0x7f, 0x01],
        )
        .unwrap();
        assert_eq!(
            negative_global.effective_address(),
            Gfx942EffectiveAddressV1::BaseIndex {
                base: Gfx942SemanticRegisterV1::Vector(4),
                index: None,
                scale: 1,
                displacement: -4,
                address_space: 1,
                pointer_bits: 64,
                byte_width: 4,
            }
        );
    }

    #[test]
    fn real_lds_byte_substitution_and_unsupported_target_fail_closed() {
        let target = AmdTargetId::parse(PRODUCTION_GFX942_DEVICE_TARGET_V1).unwrap();
        let original = [0x10, 0x00, 0x6c, 0xd8, 0x0b, 0x00, 0x00, 0x0a];
        let mut substituted = original;
        substituted[0] = 0x14;
        let original = independently_decode_amd_instruction_v1(target, 64, &original).unwrap();
        let substituted =
            independently_decode_amd_instruction_v1(target, 64, &substituted).unwrap();
        assert_ne!(
            original.effective_address(),
            substituted.effective_address()
        );

        let unsupported = AmdTargetId::parse("gfx90a:xnack-").unwrap();
        assert_eq!(
            independently_decode_amd_instruction_v1(unsupported, 64, &[0, 0, 0, 0]).unwrap_err(),
            Gfx942MachineRefinementErrorV1::TargetMismatch
        );
    }

    #[test]
    fn real_gfx950_scaled_mfma_bytes_determine_formats_and_register_spans() {
        let gfx942 = AmdTargetId::parse(PRODUCTION_GFX942_DEVICE_TARGET_V1).unwrap();
        let gfx950 = AmdTargetId::parse(PRODUCTION_GFX950_DEVICE_TARGET_V1).unwrap();
        let cases: &[(&[u8], u32, usize)] = &[
            (
                &[0x00, 0x80, 0xad, 0xd3, 0x00, 0x11, 0x02, 0x04],
                DISCRIMINATOR_MFMA_F32_16X16X128_FP8_E4M3,
                20,
            ),
            (
                &[0x00, 0x81, 0xad, 0xd3, 0x00, 0x11, 0x02, 0x24],
                DISCRIMINATOR_MFMA_F32_16X16X128_FP8_E5M2,
                20,
            ),
            (
                &[0x00, 0x84, 0xad, 0xd3, 0x00, 0x09, 0x02, 0x84],
                DISCRIMINATOR_MFMA_F32_16X16X128_FP4_E2M1,
                12,
            ),
            (
                &[0x00, 0x84, 0xad, 0xd3, 0x08, 0x01, 0x02, 0x04],
                DISCRIMINATOR_MFMA_F32_16X16X128_FP4_E2M1_BY_FP8_E4M3,
                16,
            ),
        ];
        for (encoding, discriminator, use_count) in cases {
            let decoded = independently_decode_amd_instruction_v1(gfx950, 64, encoding).unwrap();
            assert_eq!(decoded.semantics(), Gfx942SemanticOpcodeV1::Matrix);
            assert_eq!(decoded.semantic_discriminator(), *discriminator);
            assert_eq!(decoded.definitions().len(), 4);
            assert_eq!(decoded.uses().len(), *use_count);
            assert_eq!(
                independently_decode_amd_instruction_v1(gfx942, 64, encoding).unwrap_err(),
                Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode
            );
        }

        let mut relabeled_format = cases[0].0.to_vec();
        relabeled_format[1] = 4;
        assert_eq!(
            independently_decode_amd_instruction_v1(gfx950, 64, &relabeled_format).unwrap_err(),
            Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode
        );
    }

    #[test]
    fn dpp_lane_controls_are_closed_and_mask_aware() {
        let full_row_shift = 0x111 | (1 << 9) | (0xf << 10) | (0xf << 14);
        assert!(dpp_discriminator_admitted(full_row_shift));
        assert_eq!(dpp_source_lane(0x111, true, 0xf, 0xf, 0), Some(None));
        assert_eq!(dpp_source_lane(0x111, true, 0xf, 0xf, 17), Some(Some(16)));
        assert_eq!(dpp_source_lane(0x121, true, 0xf, 0xf, 0), Some(Some(15)));
        assert_eq!(dpp_source_lane(0x111, true, 0b1110, 0xf, 0), Some(Some(0)));
        assert!(!dpp_discriminator_admitted(full_row_shift | (1 << 20)));
        assert!(!dpp_discriminator_admitted(
            0x101 | (0xf << 10) | (0xf << 14)
        ));
    }

    #[test]
    fn complete_executable_text_roster_rejects_omission_and_unknown_word() {
        let end_program = [0x00, 0x00, 0x81, 0xbf];
        let artifact = executable_elf(&end_program);
        let instruction = Gfx942DecodedInstructionV1::new(
            0,
            0,
            64,
            64,
            1,
            Gfx942SemanticOpcodeV1::Return,
            0,
            end_program,
            end_program,
            [],
            [],
            None,
            CompilerMemoryEffectV1::none(),
            Gfx942EffectiveAddressV1::None,
        )
        .unwrap();
        validate_instruction_bytes(
            AmdMachineRefinementTargetV1::Gfx942,
            &artifact,
            &artifact,
            &[instruction],
        )
        .unwrap();
        assert_eq!(
            validate_instruction_bytes(
                AmdMachineRefinementTargetV1::Gfx942,
                &artifact,
                &artifact,
                &[],
            )
            .unwrap_err(),
            Gfx942MachineRefinementErrorV1::IncompleteInstructionMapping
        );

        let unknown = executable_elf(&[0, 0, 0, 0]);
        assert_eq!(
            validate_instruction_bytes(
                AmdMachineRefinementTargetV1::Gfx942,
                &unknown,
                &unknown,
                &[],
            )
            .unwrap_err(),
            Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode
        );
    }

    #[test]
    fn support_boundary_names_advanced_and_production_gaps() {
        let status = amd_machine_refinement_component_status_v1();
        let disposition = |component| {
            status
                .iter()
                .find(|entry| entry.component() == component)
                .unwrap()
                .disposition()
        };
        assert_eq!(
            disposition(AmdMachineRefinementComponentV1::Gfx942SoppControl),
            AmdMachineRefinementComponentDispositionV1::IndependentlyDecodedClosedSubset
        );
        for implemented_subset in [
            AmdMachineRefinementComponentV1::Gfx942GeneralScalarVectorAlu,
            AmdMachineRefinementComponentV1::Gfx942DsLds,
            AmdMachineRefinementComponentV1::Gfx942Synchronization,
            AmdMachineRefinementComponentV1::Gfx942Atomics,
            AmdMachineRefinementComponentV1::Gfx950InstructionSet,
        ] {
            assert_eq!(
                disposition(implemented_subset),
                AmdMachineRefinementComponentDispositionV1::IndependentlyDecodedClosedSubset
            );
        }
        for unavailable in [
            AmdMachineRefinementComponentV1::Gfx942Collectives,
            AmdMachineRefinementComponentV1::Gfx942Mfma,
            AmdMachineRefinementComponentV1::FinalLlvmValueTransport,
            AmdMachineRefinementComponentV1::ObjectRelocationApplication,
            AmdMachineRefinementComponentV1::ProductionPublicationConsumption,
        ] {
            assert_eq!(
                disposition(unavailable),
                AmdMachineRefinementComponentDispositionV1::Unavailable
            );
        }
    }

    #[test]
    fn caller_cfg_successor_substitution_is_rejected() {
        let mut substituted = isa_parts();
        substituted.blocks[1].successors = vec![3].into_boxed_slice();
        assert_eq!(
            Gfx942DecodedIsaTranscriptV1::from_parts(substituted).unwrap_err(),
            Gfx942MachineRefinementErrorV1::InvalidMachineCfg
        );
    }

    #[test]
    fn compiler_value_binding_substitution_is_rejected_against_decoded_registers() {
        let operation = llvm_operation(
            2,
            0,
            CompilerLlvmOperationKindV1::FloatAdd,
            Some(3),
            &[2, 1],
            &[],
            &[],
            CompilerBranchDivergenceV1::None,
            CompilerMemoryEffectV1::none(),
            CompilerNumericalContractV1::IeeeBinary32,
            &[36],
        );
        let mut bindings = operation.machine_value_bindings().to_vec();
        let definition = bindings
            .iter()
            .position(|binding| binding.access() == CompilerMachineValueAccessV1::Definition)
            .unwrap();
        bindings[definition] = CompilerMachineValueBindingV1::new(
            36,
            3,
            CompilerMachineValueAccessV1::Definition,
            CompilerMachineValueLocationV1::Register(CompilerMachineRegisterV1::new(
                CompilerMachineRegisterClassV1::Vector,
                3,
            )),
        )
        .unwrap();
        bindings.sort_unstable();
        let substituted = CompilerLlvmOperationV1::new(
            operation.coordinate(),
            operation.llvm_opcode(),
            operation.kind(),
            operation.semantic_discriminator(),
            operation.result(),
            operation.result_type(),
            operation.operands(),
            operation.phi_inputs(),
            operation.phi_edge_transports(),
            operation.successors(),
            operation.divergence(),
            operation.memory(),
            operation.numerical(),
            operation.machine_offsets(),
            bindings,
            operation.effective_address().cloned(),
        )
        .unwrap();
        let isa = Gfx942DecodedIsaTranscriptV1::from_parts(isa_parts()).unwrap();
        let instruction = isa
            .instructions()
            .iter()
            .find(|instruction| instruction.hsaco_offset() == 36)
            .unwrap();
        let blocks = isa
            .blocks()
            .iter()
            .map(|block| ((block.function(), block.machine_block()), block))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            validate_operation_mapping(&substituted, &[instruction], isa.mode(), &blocks)
                .unwrap_err(),
            Gfx942MachineRefinementErrorV1::RegisterSemanticsMismatch
        );
    }

    #[test]
    fn unsupported_atomic_bytes_and_invalid_fence_shape_fail_closed() {
        let atomic = CompilerMemoryEffectV1::new(
            CompilerMemoryEffectKindV1::ReadWrite,
            1,
            4,
            4,
            CompilerMemoryOrderingV1::AcquireRelease,
            CompilerMemoryScopeV1::Agent,
            false,
        )
        .unwrap();
        let decoded = Gfx942DecodedInstructionV1::new(
            0,
            0,
            0,
            0,
            1,
            Gfx942SemanticOpcodeV1::Atomic,
            3,
            [1, 2, 3, 4],
            [1, 2, 3, 4],
            [Gfx942SemanticRegisterV1::Vector(0)],
            [Gfx942SemanticRegisterV1::Scalar(0)],
            None,
            atomic,
            Gfx942EffectiveAddressV1::BaseIndex {
                base: Gfx942SemanticRegisterV1::Scalar(0),
                index: None,
                scale: 1,
                displacement: 0,
                address_space: 1,
                pointer_bits: 64,
                byte_width: 4,
            },
        );
        assert_eq!(
            decoded.unwrap_err(),
            Gfx942MachineRefinementErrorV1::UnsupportedMachineOpcode
        );

        let bad = CompilerMemoryEffectV1::new(
            CompilerMemoryEffectKindV1::Fence,
            0,
            0,
            0,
            CompilerMemoryOrderingV1::NotAtomic,
            CompilerMemoryScopeV1::None,
            false,
        );
        assert_eq!(
            bad.unwrap_err(),
            fe2o3_compiler_lineage::CompilerInstructionSelectionCorrespondenceErrorV1::InvalidMemoryEffect
        );
    }

    #[test]
    fn canonical_unknown_tag_trailing_bytes_and_instruction_omission_are_rejected() {
        let isa = Gfx942DecodedIsaTranscriptV1::from_parts(isa_parts()).unwrap();
        let mut trailing = isa.canonical_bytes().to_vec();
        trailing.push(0);
        assert_eq!(
            Gfx942DecodedIsaTranscriptV1::decode_canonical(&trailing).unwrap_err(),
            Gfx942MachineRefinementErrorV1::NonCanonical
        );

        let mut unknown = isa.canonical_bytes().to_vec();
        let marker = [
            0x14,
            0x03,
            0x00,
            0x00,
            opcode_tag(Gfx942SemanticOpcodeV1::Load),
        ];
        let native_opcode = unknown
            .windows(marker.len())
            .position(|window| window == marker)
            .unwrap();
        unknown[native_opcode + marker.len() - 1] = u8::MAX;
        assert_eq!(
            Gfx942DecodedIsaTranscriptV1::decode_canonical(&unknown).unwrap_err(),
            Gfx942MachineRefinementErrorV1::UnknownTag
        );

        let mut omitted = isa_parts();
        omitted.instructions = omitted.instructions[..omitted.instructions.len() - 1]
            .to_vec()
            .into_boxed_slice();
        omitted.blocks[3] = Gfx942DecodedMachineBlockV1::new(0, 3, 3, 52, 1, []).unwrap();
        assert_eq!(
            Gfx942DecodedIsaTranscriptV1::from_parts(omitted).unwrap_err(),
            Gfx942MachineRefinementErrorV1::InvalidMachineCfg
        );
    }
}
