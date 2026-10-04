//! Fixed per-occurrence MIR38 primitive descriptor. This is inert source data,
//! not an executable program or a source-authentication token.
use super::*;

pub const SEMANTIC_PHYSICAL_GLOBAL_COPY_MAX_OCCURRENCES_V38: usize = 34;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticPhysicalGlobalCopyInstructionV38 {
    descriptor: [u8; 8],
}
impl SemanticPhysicalGlobalCopyInstructionV38 {
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
            0 => spdst && s0 == 0 && s1 == 0 && matches!(imm, 0 | 8 | 16 | 24),
            1 | 9 | 14 => d == 0 && s0 == 0 && s1 == 0 && imm == 0,
            2 => sdst && s0 <= 63 && s1 == 0 && imm == 6,
            3 | 6 | 7 => vdst && s0 <= 63 && s1 <= 63 && imm == 0,
            4 => vdst && (s0 <= 63 || s0 == 255) && s1 == 0 && imm == 0,
            5 => vpdst && pair(s0) && s1 == 0 && imm == 2,
            8 => vdst && pair(s0) && s1 == 0 && imm == 0,
            10 => d == 0 && pair(s0) && pair(s1) && imm == 0,
            11 => spdst && s0 == 0 && s1 == 0 && imm == 0,
            12 => d == 0 && pair(s0) && s1 <= 63 && imm == 0,
            13 => d == 0 && pair(s0) && s1 == 0 && imm == 0,
            _ => false,
        };
        if !valid {
            return Err(SemanticMirErrorV1::InvalidPhysicalGlobalCopyV38);
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
        SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalGlobalCopyBegin
            | SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalGlobalCopyLabel(_)
            | SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalGlobalCopyStep(_)
    )
}
pub(super) fn uses_v38(request: &InertSemanticMirRequestV1) -> bool {
    request.callables.iter().any(|callable| {
        matches!(callable,
        SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } if is_physical(*operation))
    }) || request.functions.iter().any(|function| {
        function.blocks.iter().any(|block| {
            matches!(&block.terminator.kind, SemanticTerminatorKindV1::Call(call)
                if call.physical_global_copy_source_v38().is_some())
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
        SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalGlobalCopyBegin
    ) {
        2
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
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
            SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
        ]
        || !inputs.iter().zip(abi.arguments()).all(|(ty, argument)| {
            request.types.get(ty.0 as usize).is_some_and(|ty| {
                ty.rust_type_kind() == SemanticRustTypeKindV1::Ordinary
                    && ty.layout().size_bytes() == Some(16)
                    && ty.layout().alignment_bytes() == 8
                    && !ty.layout().is_uninhabited()
            }) && matches!(argument.value().mode(), SemanticAbiPassModeV1::Pair { .. })
        })
    {
        return false;
    }
    // Structural evidence only. The live compiler separately authenticates the
    // nominal output wrapper and exact actual root/marker type relationship.
    let SemanticTypeShapeV1::Pointer(input) = request.types[inputs[0].0 as usize].shape() else {
        return false;
    };
    if input.kind() != SemanticPointerKindV1::Reference
        || input.mutability() != SemanticMutabilityV1::Immutable
        || input.address_space() != 0
        || input.pointer_width_bits() != 64
        || input.metadata() != SemanticPointerMetadataV1::SliceLength
    {
        return false;
    }
    let Some(slice) = request.types.get(input.pointee().0 as usize) else {
        return false;
    };
    let SemanticTypeShapeV1::Slice { element } = slice.shape() else {
        return false;
    };
    exact_output_element(request, inputs[1], *element)
        && request.types.get(element.0 as usize).is_some_and(|ty| {
            ty.rust_type_kind() == SemanticRustTypeKindV1::Ordinary
                && is_unsigned_integer_with_bits(request, *element, 32)
                && ty.layout().size_bytes() == Some(4)
                && ty.layout().alignment_bytes() == 4
        })
}

/// Exact structural field and pointee join; this is not nominal source authority.
fn exact_output_element(
    request: &InertSemanticMirRequestV1,
    output: SemanticTypeIdV1,
    element: SemanticTypeIdV1,
) -> bool {
    let Some(declaration) = request.types.get(output.0 as usize) else {
        return false;
    };
    let SemanticTypeShapeV1::Aggregate(aggregate) = declaration.shape() else {
        return false;
    };
    let SemanticTypeLayoutDetailsV1::Aggregate(layout) = declaration.layout().details() else {
        return false;
    };
    let [pointer, length, phantom] = aggregate.fields() else {
        return false;
    };
    if layout.field_offsets() != [0, 8, 16] || !layout.padding().is_empty() {
        return false;
    }
    let Some(pointer) = request.types.get(pointer.0 as usize) else {
        return false;
    };
    let SemanticTypeShapeV1::Pointer(pointer) = pointer.shape() else {
        return false;
    };
    if pointer.pointee() != element
        || pointer.kind() != SemanticPointerKindV1::Raw
        || pointer.mutability() != SemanticMutabilityV1::Mutable
        || pointer.address_space() != 0
        || pointer.pointer_width_bits() != 64
        || pointer.metadata() != SemanticPointerMetadataV1::None
    {
        return false;
    }
    request.types.get(length.0 as usize).is_some_and(|ty| {
        ty.rust_type_kind() == SemanticRustTypeKindV1::Ordinary
            && is_unsigned_integer_with_bits(request, *length, 64)
            && ty.layout().size_bytes() == Some(8)
            && ty.layout().alignment_bytes() == 8
    }) && request.types.get(phantom.0 as usize).is_some_and(|ty| {
        ty.layout().size_bytes() == Some(0)
            && ty.layout().alignment_bytes() == 1
            && !ty.layout().is_uninhabited()
    })
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
    if !physical && call.physical_global_copy_source_v38().is_none() {
        return Ok(());
    }
    let refuse = || SemanticMirErrorV1::InvalidPhysicalGlobalCopyV38;
    let Some(SemanticCallableDeclV1::CompilerIntrinsic {
        operation, binding, ..
    }) = callee
    else {
        return Err(refuse());
    };
    let source = call.physical_global_copy_source_v38().ok_or_else(refuse)?;
    let count = if matches!(
        operation,
        SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalGlobalCopyBegin
    ) {
        2
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
        || (source.occurrence() == 0) != (count == 2)
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
        if !place.projections.is_empty() || (index == 1 && !moved) {
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
    if version != SemanticMirWireVersionV1::V38 {
        return Err(SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: version,
            required: SemanticMirWireVersionV1::V38,
        });
    }
    match operation {
        SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalGlobalCopyBegin => {
            writer.u8(96)?;
            writer.u8(0)
        }
        SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalGlobalCopyLabel(0) => {
            writer.u8(97)?;
            writer.u8(0)?;
            writer.u8(0)
        }
        SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalGlobalCopyStep(step) => {
            SemanticPhysicalGlobalCopyInstructionV38::from_descriptor(step.descriptor())?;
            writer.u8(98)?;
            writer.u8(0)?;
            for byte in step.descriptor() {
                writer.u8(byte)?;
            }
            Ok(())
        }
        _ => Err(SemanticMirErrorV1::InvalidPhysicalGlobalCopyV38),
    }
}
pub(super) fn encode_source(
    writer: &mut CanonicalWriterV1,
    source: Option<SemanticPhysicalGlobalCopySourceV38>,
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
