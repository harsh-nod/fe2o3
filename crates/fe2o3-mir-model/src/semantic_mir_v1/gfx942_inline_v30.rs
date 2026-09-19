//! Closed, inert scalar ISA descriptions and per-call source references.
//!
//! These records validate shape, not provenance. Only the compiler's independent
//! source/occurrence replay can authenticate the referenced records. The V30
//! suffix is the historical record API, not the current standalone wire schema.
//! V34 encodes these records at intrinsic 91; frozen diagnostic V31/V32 retain
//! their scalar intrinsic 87. All three extend V28 without V29 capabilities.

use super::*;

/// The six vector integer instructions exposed by the typed source macro.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticGfx942InlineInstructionV30 {
    VMovB32,
    VAddU32,
    VSubU32,
    VAndB32,
    VOrB32,
    VXorB32,
}

impl SemanticGfx942InlineInstructionV30 {
    pub const fn input_count(self) -> usize {
        match self {
            Self::VMovB32 => 1,
            _ => 2,
        }
    }

    pub const fn mnemonic(self) -> &'static str {
        match self {
            Self::VMovB32 => "v_mov_b32",
            Self::VAddU32 => "v_add_u32",
            Self::VSubU32 => "v_sub_u32",
            Self::VAndB32 => "v_and_b32",
            Self::VOrB32 => "v_or_b32",
            Self::VXorB32 => "v_xor_b32",
        }
    }

    pub(super) const fn wire_tag(self) -> u8 {
        match self {
            Self::VMovB32 => 0,
            Self::VAddU32 => 1,
            Self::VSubU32 => 2,
            Self::VAndB32 => 3,
            Self::VOrB32 => 4,
            Self::VXorB32 => 5,
        }
    }

    pub(super) const fn from_wire_tag(tag: u8) -> Option<Self> {
        Some(match tag {
            0 => Self::VMovB32,
            1 => Self::VAddU32,
            2 => Self::VSubU32,
            3 => Self::VAndB32,
            4 => Self::VOrB32,
            5 => Self::VXorB32,
            _ => return None,
        })
    }
}

/// Fixed gfx942, VGPR32, `u32` input/output, effect-free source contract.
///
/// The sole admitted option is NoMemory. In particular, this record does not
/// silently add Pure, PreservesFlags, or NoStack to the source macro's promise.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticGfx942InlineU32V30 {
    instruction: SemanticGfx942InlineInstructionV30,
    option_bits: u16,
}

impl SemanticGfx942InlineU32V30 {
    pub const NOMEM_OPTION_BITS: u16 = 0x0001;

    pub fn new(
        instruction: SemanticGfx942InlineInstructionV30,
        option_bits: u16,
    ) -> Result<Self, SemanticMirErrorV1> {
        if option_bits != Self::NOMEM_OPTION_BITS {
            return Err(SemanticMirErrorV1::InvalidInlineAssemblyV30);
        }
        Ok(Self {
            instruction,
            option_bits,
        })
    }

    pub const fn instruction(self) -> SemanticGfx942InlineInstructionV30 {
        self.instruction
    }

    pub const fn option_bits(self) -> u16 {
        self.option_bits
    }

    pub const fn input_count(self) -> usize {
        self.instruction.input_count()
    }
}

/// Inert references for one original call occurrence, never a shared callee.
///
/// Nonzero identities and caller equality establish structural consistency
/// only. Construction and canonical decoding grant no source or launch authority.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticInlineAssemblySourceV30 {
    frontend_unit: [u8; 32],
    function: SemanticFunctionIdentityV1,
    contract: [u8; 32],
    statement: [u8; 32],
}

impl SemanticInlineAssemblySourceV30 {
    pub fn new(
        frontend_unit: [u8; 32],
        function: SemanticFunctionIdentityV1,
        contract: [u8; 32],
        statement: [u8; 32],
    ) -> Result<Self, SemanticMirErrorV1> {
        if [frontend_unit, *function.as_bytes(), contract, statement].contains(&[0; 32]) {
            return Err(SemanticMirErrorV1::InvalidInlineAssemblyV30);
        }
        Ok(Self {
            frontend_unit,
            function,
            contract,
            statement,
        })
    }

    pub const fn frontend_unit(self) -> [u8; 32] {
        self.frontend_unit
    }

    pub const fn function(self) -> SemanticFunctionIdentityV1 {
        self.function
    }

    pub const fn contract(self) -> [u8; 32] {
        self.contract
    }

    pub const fn statement(self) -> [u8; 32] {
        self.statement
    }
}

pub(super) fn uses_scalar_authoring(request: &InertSemanticMirRequestV1) -> bool {
    request.callables.iter().any(|callable| {
        matches!(
            callable,
            SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::Gfx942InlineU32(_),
                ..
            }
        )
    }) || request.functions.iter().any(|function| {
        function.blocks.iter().any(|block| {
            matches!(&block.terminator.kind, SemanticTerminatorKindV1::Call(call)
            if call.inline_assembly_source_v30.is_some())
        })
    })
}

pub(super) fn validate_call_source(
    request: &InertSemanticMirRequestV1,
    function: &SemanticFunctionDeclV1,
    call: &SemanticDirectCallV1,
) -> Result<(), SemanticMirErrorV1> {
    let is_assembly = matches!(
        request.callables.get(call.callee.0 as usize),
        Some(SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::Gfx942InlineU32(_),
            ..
        })
    );
    match (is_assembly, call.inline_assembly_source_v30) {
        (false, None) => Ok(()),
        (true, Some(source))
            if source.function == function.identity
                && call.ordered_program_source_v32.is_none() =>
        {
            Ok(())
        }
        _ => Err(SemanticMirErrorV1::InvalidInlineAssemblyV30),
    }
}
