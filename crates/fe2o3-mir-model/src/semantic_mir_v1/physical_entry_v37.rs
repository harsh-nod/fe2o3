//! Fixed per-occurrence MIR37 primitive descriptor. This is inert source data,
//! not an executable program or a source-authentication token.
use super::*;

pub const SEMANTIC_PHYSICAL_ENTRY_MAX_OCCURRENCES_V37: usize = 73;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticPhysicalEntryInstructionV37 {
    descriptor: [u8; 8],
}
impl SemanticPhysicalEntryInstructionV37 {
    pub fn new(
        opcode: u8,
        destination: u8,
        source0: u8,
        source1: u8,
        immediate: u32,
    ) -> Result<Self, SemanticMirErrorV1> {
        let [a, b, c, d] = immediate.to_le_bytes();
        Self::from_descriptor([opcode, destination, source0, source1, a, b, c, d])
    }
    pub fn from_descriptor(descriptor: [u8; 8]) -> Result<Self, SemanticMirErrorV1> {
        let [op, d, s0, s1, _, _, _, _] = descriptor;
        let imm = u32::from_le_bytes([descriptor[4], descriptor[5], descriptor[6], descriptor[7]]);
        let sdst = (3..=63).contains(&d);
        let vdst = (1..=63).contains(&d);
        let pair = |value: u8| value <= 62 && value.is_multiple_of(2);
        let spdst = d >= 4 && pair(d);
        let vpdst = d >= 2 && pair(d);
        let valid = match op {
            0 => spdst && s0 == 0 && s1 == 0 && matches!(imm, 0 | 8),
            1 => sdst && s0 == 0 && s1 == 0 && matches!(imm, 16 | 20 | 24 | 28),
            2 | 15 | 17 => d == 0 && s0 == 0 && s1 == 0 && imm == 0,
            3 => sdst && s0 <= 63 && s1 == 0 && imm == 6,
            4 | 10 | 11 => vdst && s0 <= 63 && s1 <= 63 && imm == 0,
            5 => vdst && (s0 <= 63 || s0 == 255) && s1 == 0 && imm == 0,
            6 => d == 0 && s0 <= 63 && s1 == 0 && imm == 0,
            7 | 8 | 18 => d == 0 && s0 == 0 && s1 == 0 && imm <= u8::MAX.into(),
            9 => vpdst && pair(s0) && s1 == 0 && imm == 2,
            12 => d == 0 && pair(s0) && pair(s1) && imm == 0,
            13 => spdst && s0 == 0 && s1 == 0 && imm == 0,
            14 => d == 0 && pair(s0) && s1 <= 63 && imm == 0,
            16 => d == 0 && pair(s0) && s1 == 0 && imm == 0,
            _ => false,
        };
        if !valid {
            return Err(SemanticMirErrorV1::InvalidPhysicalEntryV37);
        }
        Ok(Self { descriptor })
    }
    pub const fn descriptor(self) -> [u8; 8] {
        self.descriptor
    }
    pub const fn opcode(self) -> u8 {
        self.descriptor[0]
    }
    pub const fn destination(self) -> u8 {
        self.descriptor[1]
    }
    pub const fn source0(self) -> u8 {
        self.descriptor[2]
    }
    pub const fn source1(self) -> u8 {
        self.descriptor[3]
    }
    pub const fn immediate(self) -> u32 {
        u32::from_le_bytes([
            self.descriptor[4],
            self.descriptor[5],
            self.descriptor[6],
            self.descriptor[7],
        ])
    }
}

pub(super) const VALIDATION_WORK: usize = 128;
pub(super) const fn is_physical(operation: SemanticCompilerIntrinsicOperationV1) -> bool {
    matches!(
        operation,
        SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalEntryBegin
            | SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalEntryLabel(_)
            | SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalEntryStep(_)
    )
}
pub(super) fn uses_v37(request: &InertSemanticMirRequestV1) -> bool {
    request.callables.iter().any(|callable| {
        matches!(callable,
        SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } if is_physical(*operation))
    }) || request.functions.iter().any(|function| {
        function.blocks.iter().any(|block| {
            matches!(&block.terminator.kind, SemanticTerminatorKindV1::Call(call)
                if call.physical_entry_source_v37().is_some() || call.has_mixed_physical_sources())
        })
    })
}

pub(super) fn signature_matches(
    request: &InertSemanticMirRequestV1,
    operation: SemanticCompilerIntrinsicOperationV1,
    abi: &SemanticFunctionAbiV1,
) -> bool {
    let inputs = abi.source_input_types();
    let count = if matches!(
        operation,
        SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalEntryBegin
    ) {
        5
    } else {
        0
    };
    if !is_physical(operation)
        || abi.canon_abi() != SemanticCanonAbiV1::Rust
        || abi.extern_abi() != SemanticExternAbiV1::Rust
        || abi.c_variadic()
        || abi.can_unwind()
        || inputs.len() != count
        || abi.arguments().len() != count
        || abi.fixed_count() as usize != count
        || !request
            .types
            .get(abi.source_output_type().0 as usize)
            .is_some_and(|ty| matches!(ty.shape(), SemanticTypeShapeV1::Unit))
        || !matches!(abi.return_value().mode(), SemanticAbiPassModeV1::Ignore)
        || abi.return_value().adjusted().is_some()
        || abi.return_value().pointee_override().is_some()
        || abi
            .arguments()
            .iter()
            .any(|arg| arg.value().adjusted().is_some() || arg.value().pointee_override().is_some())
    {
        return false;
    }
    if count == 0 {
        return true;
    }
    if abi.source_argument_ownership()
        != [
            SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
            SemanticSourceArgumentOwnershipV1::ByValue,
            SemanticSourceArgumentOwnershipV1::ByValue,
            SemanticSourceArgumentOwnershipV1::ByValue,
            SemanticSourceArgumentOwnershipV1::ByValue,
        ]
    {
        return false;
    }
    request.types.get(inputs[0].0 as usize).is_some()
        && matches!(
            abi.arguments()[0].value().mode(),
            SemanticAbiPassModeV1::Pair { .. }
        )
        && inputs[1..]
            .iter()
            .all(|ty| is_unsigned_integer_with_bits(request, *ty, 32))
        && abi.arguments()[1..]
            .iter()
            .all(|argument| matches!(argument.value().mode(), SemanticAbiPassModeV1::Direct(_)))
}

pub(super) fn validate_call_source(
    request: &InertSemanticMirRequestV1,
    function: &SemanticFunctionDeclV1,
    location: SemanticMirLocationV1,
    call: &SemanticDirectCallV1,
) -> Result<(), SemanticMirErrorV1> {
    let callee = request.callables.get(call.callee.0 as usize);
    let physical = matches!(callee,Some(SemanticCallableDeclV1::CompilerIntrinsic{operation,..})
        if is_physical(*operation));
    if !physical && call.physical_entry_source_v37().is_none() {
        return Ok(());
    }
    let refuse = || SemanticMirErrorV1::InvalidPhysicalEntryV37;
    let Some(SemanticCallableDeclV1::CompilerIntrinsic {
        operation, binding, ..
    }) = callee
    else {
        return Err(refuse());
    };
    let source = call.physical_entry_source_v37().ok_or_else(refuse)?;
    let count = if matches!(
        operation,
        SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalEntryBegin
    ) {
        5
    } else {
        0
    };
    if !source.matches_function(function)
        || !signature_matches(request, *operation, &binding.abi)
        || call.arguments.len() != count
        || !call.variadic_argument_abis.is_empty()
        || call.complete_body_source_vnext.is_some()
        || call.inline_assembly_source_v30.is_some()
        || call.ordered_region_source_v31.is_some()
        || call.ordered_program_source_v32.is_some()
        || !matches!(
            call.unwind,
            SemanticUnwindActionV1::Continue | SemanticUnwindActionV1::Unreachable
        )
        || (source.occurrence() == 0) != (count == 5)
    {
        return Err(refuse());
    }
    let SemanticMirLocationV1::Terminator { block, .. } = location else {
        return Err(refuse());
    };
    let block = function.blocks.get(block.0 as usize).ok_or_else(refuse)?;
    if source.raw_block() as usize >= function.blocks.len()
        || source.block_identity() != *block.identity.as_bytes()
    {
        return Err(refuse());
    }
    for (index, operand) in call.arguments.iter().enumerate() {
        if operand.ty() != binding.abi.source_input_types()[index] {
            return Err(refuse());
        }
        let (place, moved) = match operand {
            SemanticOperandV1::Move(place) => (place, true),
            SemanticOperandV1::Copy(place) => (place, false),
            _ => return Err(refuse()),
        };
        if !place.projections.is_empty() || (index == 0 && !moved) {
            return Err(refuse());
        }
    }
    Ok(())
}

pub(super) fn encode_operation(
    writer: &mut CanonicalWriterV1,
    operation: SemanticCompilerIntrinsicOperationV1,
    version: SemanticMirWireVersionV1,
) -> Result<(), SemanticMirErrorV1> {
    if version != SemanticMirWireVersionV1::V37 {
        return Err(SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: version,
            required: SemanticMirWireVersionV1::V37,
        });
    }
    match operation {
        SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalEntryBegin => {
            writer.u8(93)?;
            writer.u8(0)
        }
        SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalEntryLabel(label) => {
            writer.u8(94)?;
            writer.u8(0)?;
            writer.u8(label)
        }
        SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalEntryStep(step) => {
            SemanticPhysicalEntryInstructionV37::from_descriptor(step.descriptor())?;
            writer.u8(95)?;
            writer.u8(0)?;
            for byte in step.descriptor() {
                writer.u8(byte)?;
            }
            Ok(())
        }
        _ => Err(SemanticMirErrorV1::InvalidPhysicalEntryV37),
    }
}
pub(super) fn encode_source(
    writer: &mut CanonicalWriterV1,
    source: Option<SemanticPhysicalEntrySourceV37>,
) -> Result<(), SemanticMirErrorV1> {
    let Some(source) = source else {
        return writer.u8(0);
    };
    writer.u8(1)?;
    for hash in source.root_axes().into_iter().chain([
        source.mir_body(),
        source.block_identity(),
        source.source_signature(),
        source.rustc_fn_abi(),
        source.frontend_bytes_sha256(),
    ]) {
        writer.identity(hash)?;
    }
    writer.u32(source.raw_block())?;
    writer.u8(source.occurrence())
}
