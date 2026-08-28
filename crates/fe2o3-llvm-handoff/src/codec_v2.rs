use alloc::{string::String, vec::Vec};
use core::{fmt, str};

use crate::{
    AddressSpaceV1, AxisV2, BasicBlockV2, BinaryOperationV2, BlockIdV2, CallTargetV2,
    CallingConventionV2, CastOperationV2, ComparePredicateV2, DecodeHandoffErrorV1, EvidenceV2,
    ExecutableModuleV2, FloatBinaryOperationV2, FunctionAttributeV2, FunctionIdV2, FunctionKindV2,
    FunctionParameterV2, FunctionV2, Gfx942HandoffV1, Gfx942HandoffV2, GlobalIdV2, GlobalLinkageV2,
    GlobalV2, HandoffDiagnosticV2, HandoffLimitV2, InstructionKindV2, InstructionV2,
    IntegerBinaryOperationV2, IntrinsicReferenceV2, IntrinsicV2, MAX_CANONICAL_HANDOFF_BYTES_V1,
    MAX_CANONICAL_HANDOFF_BYTES_V2, MAX_EVIDENCE_OBLIGATIONS_V2, MAX_FUNCTION_ATTRIBUTES_V2,
    MAX_FUNCTION_BLOCKS_V2, MAX_FUNCTION_PARAMETERS_V2, MAX_FUNCTIONS_V2, MAX_GEP_INDICES_V2,
    MAX_GLOBALS_V2, MAX_INSTRUCTIONS_PER_FUNCTION_V2, MAX_INTRINSICS_V2, MAX_MODULE_FLAGS_V2,
    MAX_NAMED_METADATA_V2, MAX_PARAMETER_ATTRIBUTES_V2, MAX_SYMBOL_BYTES_V2, ModuleFlagV1,
    NamedMetadataV1, ObligationIdentityV1, OriginIdentityV1, ParameterAttributeV1, ReturnTypeV2,
    ScalarConstantV2, ScalarTypeV1, TerminatorV2, TypedValueV2, ValueIdV2, ValueTypeV2,
    WavesPerEuV1, WorkgroupSizeRangeV1,
};

const MAGIC_V2: &[u8; 8] = b"F2LLVMH2";
const VERSION_V2: u16 = 2;
const HEADER_BYTES_V2: usize = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalHandoffBytesV2 {
    bytes: Vec<u8>,
}

impl CanonicalHandoffBytesV2 {
    pub(crate) fn from_validated(bytes: Vec<u8>) -> Self {
        debug_assert!(bytes.len() <= MAX_CANONICAL_HANDOFF_BYTES_V2);
        Self { bytes }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

impl AsRef<[u8]> for CanonicalHandoffBytesV2 {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WireSectionV2 {
    Header,
    EmbeddedV1,
    ModuleFlag,
    NamedMetadata,
    GlobalLinkage,
    AddressSpace,
    ScalarType,
    Intrinsic,
    Axis,
    FunctionKind,
    CallingConvention,
    ReturnType,
    ValueType,
    ParameterAttribute,
    FunctionAttribute,
    Instruction,
    BinaryOperation,
    IntegerBinaryOperation,
    FloatBinaryOperation,
    ComparePredicate,
    CastOperation,
    CallTarget,
    Terminator,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodeHandoffErrorV2 {
    TooLong {
        observed: usize,
        maximum: usize,
    },
    Truncated {
        offset: usize,
    },
    BadMagic,
    UnsupportedVersion(u16),
    NonzeroReserved,
    LengthMismatch {
        declared: usize,
        observed: usize,
    },
    InvalidUtf8(WireSectionV2),
    UnknownTag {
        section: WireSectionV2,
        tag: u8,
    },
    LimitExceeded {
        limit: HandoffLimitV2,
        observed: usize,
        maximum: usize,
    },
    InvalidBase(DecodeHandoffErrorV1),
    InvalidModel(HandoffDiagnosticV2),
    NonCanonical,
}

impl fmt::Display for DecodeHandoffErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLong { observed, maximum } => write!(
                formatter,
                "canonical LLVM V2 handoff has {observed} bytes, maximum is {maximum}"
            ),
            Self::Truncated { offset } => {
                write!(
                    formatter,
                    "canonical LLVM V2 handoff is truncated at offset {offset}"
                )
            }
            Self::BadMagic => formatter.write_str("invalid canonical LLVM V2 handoff magic"),
            Self::UnsupportedVersion(version) => {
                write!(
                    formatter,
                    "unsupported canonical LLVM V2 handoff version {version}"
                )
            }
            Self::NonzeroReserved => {
                formatter.write_str("canonical LLVM V2 handoff reserved bits are nonzero")
            }
            Self::LengthMismatch { declared, observed } => write!(
                formatter,
                "canonical LLVM V2 handoff declares {declared} bytes but contains {observed}"
            ),
            Self::InvalidUtf8(section) => {
                write!(formatter, "canonical LLVM V2 {section:?} text is not UTF-8")
            }
            Self::UnknownTag { section, tag } => {
                write!(formatter, "unknown canonical LLVM V2 {section:?} tag {tag}")
            }
            Self::LimitExceeded {
                limit,
                observed,
                maximum,
            } => write!(
                formatter,
                "canonical LLVM V2 {limit:?} count {observed} exceeds {maximum}"
            ),
            Self::InvalidBase(error) => write!(formatter, "invalid embedded V1 handoff: {error}"),
            Self::InvalidModel(error) => write!(formatter, "invalid LLVM V2 model: {error}"),
            Self::NonCanonical => formatter.write_str("LLVM V2 handoff bytes are not canonical"),
        }
    }
}

impl core::error::Error for DecodeHandoffErrorV2 {}

pub(crate) fn encode_handoff_v2(handoff: &Gfx942HandoffV2) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(MAGIC_V2);
    put_u16(&mut bytes, VERSION_V2);
    put_u16(&mut bytes, 0);
    put_u32(&mut bytes, 0);

    let base = handoff.base.encode_canonical();
    put_u32(
        &mut bytes,
        u32::try_from(base.len()).expect("bounded V1 handoff fits u32"),
    );
    bytes.extend_from_slice(base.as_bytes());
    bytes.extend_from_slice(handoff.module.identity().as_bytes());
    bytes.extend_from_slice(&encode_module_v2(&handoff.module));

    let total_len = u32::try_from(bytes.len()).expect("bounded V2 handoff fits u32");
    bytes[12..16].copy_from_slice(&total_len.to_le_bytes());
    bytes
}

pub(crate) fn encode_module_v2(module: &ExecutableModuleV2) -> Vec<u8> {
    let mut bytes = Vec::new();
    put_u8(
        &mut bytes,
        u8::try_from(module.flags.len()).expect("bounded module flag count fits u8"),
    );
    for flag in &module.flags {
        put_u8(&mut bytes, module_flag_tag(*flag));
    }
    put_u8(
        &mut bytes,
        u8::try_from(module.named_metadata.len()).expect("bounded metadata count fits u8"),
    );
    for metadata in &module.named_metadata {
        put_u8(&mut bytes, metadata.kind());
        if let NamedMetadataV1::ProducerIdentity(identity) = metadata {
            bytes.extend_from_slice(identity.as_bytes());
        }
    }

    put_u16(
        &mut bytes,
        u16::try_from(module.globals.len()).expect("bounded global count fits u16"),
    );
    for global in &module.globals {
        put_u32(&mut bytes, global.id.get());
        put_string(&mut bytes, &global.symbol);
        put_u8(&mut bytes, global_linkage_tag(global.linkage));
        put_u8(&mut bytes, address_space_tag(global.address_space));
        put_u8(&mut bytes, u8::from(global.mutable));
        put_u8(&mut bytes, scalar_type_tag(global.value_type));
        match (
            global.array_elements,
            global.initializer,
            global.byte_initializer.as_deref(),
        ) {
            (Some(elements), None, Some(initializer)) => {
                put_u8(&mut bytes, 3);
                put_u16(&mut bytes, elements);
                bytes.extend_from_slice(initializer);
                put_u16(&mut bytes, global.alignment);
                put_string(
                    &mut bytes,
                    global.section.as_deref().expect("validated byte section"),
                );
            }
            (Some(elements), None, None) => {
                put_u8(&mut bytes, 2);
                put_u16(&mut bytes, elements);
                put_u16(&mut bytes, global.alignment);
            }
            (None, None, None) => put_u8(&mut bytes, 0),
            (None, Some(initializer), None) => {
                put_u8(&mut bytes, 1);
                encode_scalar_constant(&mut bytes, initializer);
            }
            _ => unreachable!("validated globals have one storage form"),
        }
        encode_evidence(&mut bytes, &global.evidence);
    }

    put_u16(
        &mut bytes,
        u16::try_from(module.intrinsics.len()).expect("bounded intrinsic count fits u16"),
    );
    for intrinsic in &module.intrinsics {
        encode_intrinsic(&mut bytes, intrinsic.intrinsic);
        encode_evidence(&mut bytes, &intrinsic.evidence);
    }

    put_u16(
        &mut bytes,
        u16::try_from(module.functions.len()).expect("bounded function count fits u16"),
    );
    for function in &module.functions {
        encode_function(&mut bytes, function);
    }
    bytes
}

pub(crate) fn decode_handoff_v2(bytes: &[u8]) -> Result<Gfx942HandoffV2, DecodeHandoffErrorV2> {
    if bytes.len() > MAX_CANONICAL_HANDOFF_BYTES_V2 {
        return Err(DecodeHandoffErrorV2::TooLong {
            observed: bytes.len(),
            maximum: MAX_CANONICAL_HANDOFF_BYTES_V2,
        });
    }
    let mut reader = Reader::new(bytes);
    if reader.take(MAGIC_V2.len())? != MAGIC_V2 {
        return Err(DecodeHandoffErrorV2::BadMagic);
    }
    let version = reader.u16()?;
    if version != VERSION_V2 {
        return Err(DecodeHandoffErrorV2::UnsupportedVersion(version));
    }
    if reader.u16()? != 0 {
        return Err(DecodeHandoffErrorV2::NonzeroReserved);
    }
    let declared = usize::try_from(reader.u32()?).expect("u32 fits usize");
    if declared != bytes.len() {
        return Err(DecodeHandoffErrorV2::LengthMismatch {
            declared,
            observed: bytes.len(),
        });
    }
    let base_len = usize::try_from(reader.u32()?).expect("u32 fits usize");
    check_wire_limit(
        HandoffLimitV2::EmbeddedV1Bytes,
        base_len,
        MAX_CANONICAL_HANDOFF_BYTES_V1,
    )?;
    let base = Gfx942HandoffV1::decode_canonical(reader.take(base_len)?)
        .map_err(DecodeHandoffErrorV2::InvalidBase)?;
    let encoded_module_identity = reader.array_32()?;
    let module = decode_module_v2(&mut reader)?;
    if !reader.is_finished() {
        return Err(DecodeHandoffErrorV2::LengthMismatch {
            declared,
            observed: reader.offset,
        });
    }
    if module.identity().as_bytes() != &encoded_module_identity {
        return Err(DecodeHandoffErrorV2::NonCanonical);
    }
    let handoff = Gfx942HandoffV2::new(base, module).map_err(DecodeHandoffErrorV2::InvalidModel)?;
    if encode_handoff_v2(&handoff) != bytes {
        return Err(DecodeHandoffErrorV2::NonCanonical);
    }
    Ok(handoff)
}

fn decode_module_v2(reader: &mut Reader<'_>) -> Result<ExecutableModuleV2, DecodeHandoffErrorV2> {
    let flag_count = reader.bounded_u8(HandoffLimitV2::ModuleFlags, MAX_MODULE_FLAGS_V2)?;
    let mut flags = Vec::with_capacity(flag_count);
    for _ in 0..flag_count {
        flags.push(decode_module_flag(reader.u8()?)?);
    }
    let metadata_count = reader.bounded_u8(HandoffLimitV2::NamedMetadata, MAX_NAMED_METADATA_V2)?;
    let mut named_metadata = Vec::with_capacity(metadata_count);
    for _ in 0..metadata_count {
        named_metadata.push(match reader.u8()? {
            1 => NamedMetadataV1::OpenClVersion2_0,
            2 => NamedMetadataV1::OpenClSpirVersion2_0,
            3 => NamedMetadataV1::ProducerIdentity(
                crate::IdentityV1::new(reader.array_32()?)
                    .map_err(|error| DecodeHandoffErrorV2::InvalidModel(map_v1_error(error)))?,
            ),
            tag => {
                return Err(DecodeHandoffErrorV2::UnknownTag {
                    section: WireSectionV2::NamedMetadata,
                    tag,
                });
            }
        });
    }

    let global_count = reader.bounded_u16(HandoffLimitV2::Globals, MAX_GLOBALS_V2)?;
    let mut globals = Vec::with_capacity(global_count);
    for _ in 0..global_count {
        let id = GlobalIdV2::new(reader.u32()?);
        let symbol = reader.string(
            MAX_SYMBOL_BYTES_V2,
            HandoffLimitV2::SymbolBytes,
            WireSectionV2::GlobalLinkage,
        )?;
        let linkage = decode_global_linkage(reader.u8()?)?;
        let address_space = decode_address_space(reader.u8()?)?;
        let mutable = decode_bool(reader.u8()?, WireSectionV2::GlobalLinkage)?;
        let value_type = decode_scalar_type(reader.u8()?)?;
        let storage_tag = reader.u8()?;
        let initializer = match storage_tag {
            0 | 2 | 3 => None,
            1 => Some(decode_scalar_constant(reader)?),
            tag => {
                return Err(DecodeHandoffErrorV2::UnknownTag {
                    section: WireSectionV2::GlobalLinkage,
                    tag,
                });
            }
        };
        let array_shape = if storage_tag == 2 {
            Some((reader.u16()?, reader.u16()?))
        } else {
            None
        };
        let byte_shape = if storage_tag == 3 {
            let elements = usize::from(reader.u16()?);
            check_wire_limit(
                HandoffLimitV2::CanonicalBytes,
                elements,
                crate::MAX_CONSTANT_GLOBAL_BYTES_V2,
            )?;
            let bytes = reader.take(elements)?.to_vec();
            let alignment = reader.u16()?;
            let section = reader.string(
                MAX_SYMBOL_BYTES_V2,
                HandoffLimitV2::SymbolBytes,
                WireSectionV2::GlobalLinkage,
            )?;
            Some((bytes, alignment, section))
        } else {
            None
        };
        let evidence = decode_evidence(reader)?;
        let global = if let Some((bytes, alignment, section)) = byte_shape {
            if linkage != GlobalLinkageV2::Internal
                || address_space != AddressSpaceV1::Constant
                || mutable
                || value_type != ScalarTypeV1::I8
            {
                return Err(DecodeHandoffErrorV2::InvalidModel(
                    HandoffDiagnosticV2::UnsupportedInstruction,
                ));
            }
            GlobalV2::new_private_constant_bytes(id, &symbol, &section, bytes, alignment, evidence)
        } else if let Some((elements, alignment)) = array_shape {
            if linkage != GlobalLinkageV2::Internal
                || address_space != AddressSpaceV1::Local
                || !mutable
            {
                return Err(DecodeHandoffErrorV2::InvalidModel(
                    HandoffDiagnosticV2::UnsupportedInstruction,
                ));
            }
            GlobalV2::new_local_array(
                id,
                &symbol,
                value_type,
                usize::from(elements),
                alignment,
                evidence,
            )
        } else {
            GlobalV2::new(
                id,
                &symbol,
                linkage,
                address_space,
                mutable,
                value_type,
                initializer,
                evidence,
            )
        };
        globals.push(global.map_err(DecodeHandoffErrorV2::InvalidModel)?);
    }

    let intrinsic_count = reader.bounded_u16(HandoffLimitV2::Intrinsics, MAX_INTRINSICS_V2)?;
    let mut intrinsics = Vec::with_capacity(intrinsic_count);
    for _ in 0..intrinsic_count {
        intrinsics.push(IntrinsicReferenceV2::new(
            decode_intrinsic(reader)?,
            decode_evidence(reader)?,
        ));
    }

    let function_count = reader.bounded_u16(HandoffLimitV2::Functions, MAX_FUNCTIONS_V2)?;
    let mut functions = Vec::with_capacity(function_count);
    for _ in 0..function_count {
        functions.push(decode_function(reader)?);
    }
    ExecutableModuleV2::new(flags, named_metadata, globals, intrinsics, functions)
        .map_err(DecodeHandoffErrorV2::InvalidModel)
}

fn encode_function(bytes: &mut Vec<u8>, function: &FunctionV2) {
    put_u32(bytes, function.id.get());
    put_string(bytes, &function.symbol);
    put_u8(bytes, function_kind_tag(function.kind));
    put_u8(bytes, calling_convention_tag(function.calling_convention));
    encode_return_type(bytes, function.return_type);
    encode_evidence(bytes, &function.evidence);
    put_u16(
        bytes,
        u16::try_from(function.parameters.len()).expect("bounded parameter count fits u16"),
    );
    for parameter in &function.parameters {
        encode_typed_value(bytes, parameter.value);
        put_string(bytes, &parameter.name);
        put_u8(
            bytes,
            u8::try_from(parameter.attributes.len())
                .expect("bounded parameter attribute count fits u8"),
        );
        for attribute in &parameter.attributes {
            encode_parameter_attribute(bytes, *attribute);
        }
    }
    put_u8(
        bytes,
        u8::try_from(function.attributes.len()).expect("bounded function attribute count fits u8"),
    );
    for attribute in &function.attributes {
        encode_function_attribute(bytes, *attribute);
    }
    put_u32(bytes, function.entry.get());
    put_u16(
        bytes,
        u16::try_from(function.blocks.len()).expect("bounded block count fits u16"),
    );
    for block in &function.blocks {
        put_u32(bytes, block.id.get());
        put_u32(
            bytes,
            u32::try_from(block.instructions.len()).expect("bounded instruction count fits u32"),
        );
        for instruction in &block.instructions {
            encode_instruction(bytes, instruction);
        }
        encode_terminator(bytes, &block.terminator);
    }
}

fn decode_function(reader: &mut Reader<'_>) -> Result<FunctionV2, DecodeHandoffErrorV2> {
    let id = FunctionIdV2::new(reader.u32()?);
    let symbol = reader.string(
        MAX_SYMBOL_BYTES_V2,
        HandoffLimitV2::SymbolBytes,
        WireSectionV2::FunctionKind,
    )?;
    let kind = decode_function_kind(reader.u8()?)?;
    let calling_convention = decode_calling_convention(reader.u8()?)?;
    let return_type = decode_return_type(reader)?;
    let evidence = decode_evidence(reader)?;
    let parameter_count = reader.bounded_u16(
        HandoffLimitV2::FunctionParameters,
        MAX_FUNCTION_PARAMETERS_V2,
    )?;
    let mut parameters = Vec::with_capacity(parameter_count);
    for _ in 0..parameter_count {
        let value = decode_typed_value(reader)?;
        let name = reader.string(
            MAX_SYMBOL_BYTES_V2,
            HandoffLimitV2::SymbolBytes,
            WireSectionV2::ParameterAttribute,
        )?;
        let attribute_count = reader.bounded_u8(
            HandoffLimitV2::ParameterAttributes,
            MAX_PARAMETER_ATTRIBUTES_V2,
        )?;
        let mut attributes = Vec::with_capacity(attribute_count);
        for _ in 0..attribute_count {
            attributes.push(decode_parameter_attribute(reader)?);
        }
        parameters.push(
            FunctionParameterV2::new(value, &name, attributes)
                .map_err(DecodeHandoffErrorV2::InvalidModel)?,
        );
    }
    let attribute_count = reader.bounded_u8(
        HandoffLimitV2::FunctionAttributes,
        MAX_FUNCTION_ATTRIBUTES_V2,
    )?;
    let mut attributes = Vec::with_capacity(attribute_count);
    for _ in 0..attribute_count {
        attributes.push(decode_function_attribute(reader)?);
    }
    let entry = BlockIdV2::new(reader.u32()?);
    let block_count = reader.bounded_u16(HandoffLimitV2::FunctionBlocks, MAX_FUNCTION_BLOCKS_V2)?;
    let mut blocks = Vec::with_capacity(block_count);
    let mut total_instructions = 0_usize;
    for _ in 0..block_count {
        let block_id = BlockIdV2::new(reader.u32()?);
        let instruction_count = reader.bounded_u32(
            HandoffLimitV2::FunctionInstructions,
            MAX_INSTRUCTIONS_PER_FUNCTION_V2,
        )?;
        total_instructions = total_instructions.checked_add(instruction_count).ok_or(
            DecodeHandoffErrorV2::LimitExceeded {
                limit: HandoffLimitV2::FunctionInstructions,
                observed: usize::MAX,
                maximum: MAX_INSTRUCTIONS_PER_FUNCTION_V2,
            },
        )?;
        check_wire_limit(
            HandoffLimitV2::FunctionInstructions,
            total_instructions,
            MAX_INSTRUCTIONS_PER_FUNCTION_V2,
        )?;
        let mut instructions = Vec::with_capacity(instruction_count);
        for _ in 0..instruction_count {
            instructions.push(decode_instruction(reader)?);
        }
        blocks.push(BasicBlockV2::new(
            block_id,
            instructions,
            decode_terminator(reader)?,
        ));
    }
    FunctionV2::new(
        id,
        &symbol,
        kind,
        calling_convention,
        return_type,
        parameters,
        attributes,
        entry,
        blocks,
        evidence,
    )
    .map_err(DecodeHandoffErrorV2::InvalidModel)
}

fn encode_instruction(bytes: &mut Vec<u8>, instruction: &InstructionV2) {
    match instruction.result {
        None => put_u8(bytes, 0),
        Some(result) => {
            put_u8(bytes, 1);
            encode_typed_value(bytes, result);
        }
    }
    match &instruction.kind {
        InstructionKindV2::Constant(value) => {
            put_u8(bytes, 1);
            encode_scalar_constant(bytes, *value);
        }
        InstructionKindV2::VectorZero { element_type } => {
            put_u8(bytes, 14);
            put_u8(bytes, scalar_type_tag(*element_type));
        }
        InstructionKindV2::GlobalAddress(global) => {
            put_u8(bytes, 2);
            put_u32(bytes, global.get());
        }
        InstructionKindV2::Binary {
            operation,
            left,
            right,
        } => {
            put_u8(bytes, 3);
            encode_binary_operation(bytes, *operation);
            put_u32(bytes, left.get());
            put_u32(bytes, right.get());
        }
        InstructionKindV2::Compare {
            predicate,
            left,
            right,
        } => {
            put_u8(bytes, 4);
            put_u8(bytes, compare_predicate_tag(*predicate));
            put_u32(bytes, left.get());
            put_u32(bytes, right.get());
        }
        InstructionKindV2::Cast {
            operation,
            value,
            to,
        } => {
            put_u8(bytes, 5);
            put_u8(bytes, cast_operation_tag(*operation));
            put_u32(bytes, value.get());
            encode_value_type(bytes, *to);
        }
        InstructionKindV2::GetElementPtr { base, indices } => {
            put_u8(bytes, 6);
            put_u32(bytes, base.get());
            put_u8(
                bytes,
                u8::try_from(indices.len()).expect("bounded GEP index count fits u8"),
            );
            for index in indices {
                put_u32(bytes, index.get());
            }
        }
        InstructionKindV2::Load {
            pointer,
            value_type,
            alignment,
        } => {
            put_u8(bytes, 7);
            put_u32(bytes, pointer.get());
            put_u8(bytes, scalar_type_tag(*value_type));
            put_u16(bytes, *alignment);
        }
        InstructionKindV2::Store {
            pointer,
            value,
            value_type,
            alignment,
        } => {
            put_u8(bytes, 8);
            put_u32(bytes, pointer.get());
            put_u32(bytes, value.get());
            put_u8(bytes, scalar_type_tag(*value_type));
            put_u16(bytes, *alignment);
        }
        InstructionKindV2::Call { target, arguments } => {
            put_u8(bytes, 9);
            encode_call_target(bytes, *target);
            put_u16(
                bytes,
                u16::try_from(arguments.len()).expect("bounded call argument count fits u16"),
            );
            for argument in arguments {
                put_u32(bytes, argument.get());
            }
        }
        InstructionKindV2::VectorLoad4 {
            pointer,
            element_type,
            alignment,
        } => {
            put_u8(bytes, 10);
            put_u32(bytes, pointer.get());
            put_u8(bytes, scalar_type_tag(*element_type));
            put_u16(bytes, *alignment);
        }
        InstructionKindV2::Phi { incoming } => {
            put_u8(bytes, 11);
            put_u16(
                bytes,
                u16::try_from(incoming.len()).expect("bounded phi incoming count fits u16"),
            );
            for (value, block) in incoming {
                put_u32(bytes, value.get());
                put_u32(bytes, block.get());
            }
        }
        InstructionKindV2::InsertElement {
            vector,
            element,
            index,
        } => {
            put_u8(bytes, 12);
            put_u32(bytes, vector.get());
            put_u32(bytes, element.get());
            put_u32(bytes, index.get());
        }
        InstructionKindV2::ExtractElement { vector, index } => {
            put_u8(bytes, 13);
            put_u32(bytes, vector.get());
            put_u32(bytes, index.get());
        }
    }
    encode_evidence(bytes, &instruction.evidence);
}

fn decode_instruction(reader: &mut Reader<'_>) -> Result<InstructionV2, DecodeHandoffErrorV2> {
    let result = match reader.u8()? {
        0 => None,
        1 => Some(decode_typed_value(reader)?),
        tag => {
            return Err(DecodeHandoffErrorV2::UnknownTag {
                section: WireSectionV2::Instruction,
                tag,
            });
        }
    };
    let kind = match reader.u8()? {
        1 => InstructionKindV2::Constant(decode_scalar_constant(reader)?),
        2 => InstructionKindV2::GlobalAddress(GlobalIdV2::new(reader.u32()?)),
        3 => InstructionKindV2::Binary {
            operation: decode_binary_operation(reader)?,
            left: ValueIdV2::new(reader.u32()?),
            right: ValueIdV2::new(reader.u32()?),
        },
        4 => InstructionKindV2::Compare {
            predicate: decode_compare_predicate(reader.u8()?)?,
            left: ValueIdV2::new(reader.u32()?),
            right: ValueIdV2::new(reader.u32()?),
        },
        5 => InstructionKindV2::Cast {
            operation: decode_cast_operation(reader.u8()?)?,
            value: ValueIdV2::new(reader.u32()?),
            to: decode_value_type(reader)?,
        },
        6 => {
            let base = ValueIdV2::new(reader.u32()?);
            let count =
                reader.bounded_u8(HandoffLimitV2::GetElementPtrIndices, MAX_GEP_INDICES_V2)?;
            let mut indices = Vec::with_capacity(count);
            for _ in 0..count {
                indices.push(ValueIdV2::new(reader.u32()?));
            }
            InstructionKindV2::GetElementPtr { base, indices }
        }
        7 => InstructionKindV2::Load {
            pointer: ValueIdV2::new(reader.u32()?),
            value_type: decode_scalar_type(reader.u8()?)?,
            alignment: reader.u16()?,
        },
        8 => InstructionKindV2::Store {
            pointer: ValueIdV2::new(reader.u32()?),
            value: ValueIdV2::new(reader.u32()?),
            value_type: decode_scalar_type(reader.u8()?)?,
            alignment: reader.u16()?,
        },
        9 => {
            let target = decode_call_target(reader)?;
            let count = reader.bounded_u16(
                HandoffLimitV2::FunctionParameters,
                MAX_FUNCTION_PARAMETERS_V2,
            )?;
            let mut arguments = Vec::with_capacity(count);
            for _ in 0..count {
                arguments.push(ValueIdV2::new(reader.u32()?));
            }
            InstructionKindV2::Call { target, arguments }
        }
        10 => InstructionKindV2::VectorLoad4 {
            pointer: ValueIdV2::new(reader.u32()?),
            element_type: decode_scalar_type(reader.u8()?)?,
            alignment: reader.u16()?,
        },
        11 => {
            let count =
                reader.bounded_u16(HandoffLimitV2::FunctionBlocks, MAX_FUNCTION_BLOCKS_V2)?;
            let mut incoming = Vec::with_capacity(count);
            for _ in 0..count {
                incoming.push((ValueIdV2::new(reader.u32()?), BlockIdV2::new(reader.u32()?)));
            }
            InstructionKindV2::Phi { incoming }
        }
        12 => InstructionKindV2::InsertElement {
            vector: ValueIdV2::new(reader.u32()?),
            element: ValueIdV2::new(reader.u32()?),
            index: ValueIdV2::new(reader.u32()?),
        },
        13 => InstructionKindV2::ExtractElement {
            vector: ValueIdV2::new(reader.u32()?),
            index: ValueIdV2::new(reader.u32()?),
        },
        14 => InstructionKindV2::VectorZero {
            element_type: decode_scalar_type(reader.u8()?)?,
        },
        tag => {
            return Err(DecodeHandoffErrorV2::UnknownTag {
                section: WireSectionV2::Instruction,
                tag,
            });
        }
    };
    let evidence = decode_evidence(reader)?;
    InstructionV2::new(result, kind, evidence).map_err(DecodeHandoffErrorV2::InvalidModel)
}

fn encode_terminator(bytes: &mut Vec<u8>, terminator: &TerminatorV2) {
    match terminator {
        TerminatorV2::Return(None) => put_u8(bytes, 1),
        TerminatorV2::Return(Some(value)) => {
            put_u8(bytes, 2);
            put_u32(bytes, value.get());
        }
        TerminatorV2::Branch(block) => {
            put_u8(bytes, 3);
            put_u32(bytes, block.get());
        }
        TerminatorV2::ConditionalBranch {
            condition,
            then_block,
            else_block,
        } => {
            put_u8(bytes, 4);
            put_u32(bytes, condition.get());
            put_u32(bytes, then_block.get());
            put_u32(bytes, else_block.get());
        }
        TerminatorV2::Unreachable => put_u8(bytes, 5),
    }
}

fn decode_terminator(reader: &mut Reader<'_>) -> Result<TerminatorV2, DecodeHandoffErrorV2> {
    match reader.u8()? {
        1 => Ok(TerminatorV2::Return(None)),
        2 => Ok(TerminatorV2::Return(Some(ValueIdV2::new(reader.u32()?)))),
        3 => Ok(TerminatorV2::Branch(BlockIdV2::new(reader.u32()?))),
        4 => Ok(TerminatorV2::ConditionalBranch {
            condition: ValueIdV2::new(reader.u32()?),
            then_block: BlockIdV2::new(reader.u32()?),
            else_block: BlockIdV2::new(reader.u32()?),
        }),
        5 => Ok(TerminatorV2::Unreachable),
        tag => Err(DecodeHandoffErrorV2::UnknownTag {
            section: WireSectionV2::Terminator,
            tag,
        }),
    }
}

fn encode_evidence(bytes: &mut Vec<u8>, evidence: &EvidenceV2) {
    bytes.extend_from_slice(evidence.origin.as_bytes());
    put_u8(
        bytes,
        u8::try_from(evidence.obligations.len()).expect("bounded obligation count fits u8"),
    );
    for obligation in &evidence.obligations {
        bytes.extend_from_slice(obligation.as_bytes());
    }
}

fn decode_evidence(reader: &mut Reader<'_>) -> Result<EvidenceV2, DecodeHandoffErrorV2> {
    let origin = OriginIdentityV1(reader.array_32()?);
    let count = reader.bounded_u8(
        HandoffLimitV2::EvidenceObligations,
        MAX_EVIDENCE_OBLIGATIONS_V2,
    )?;
    let mut obligations = Vec::with_capacity(count);
    for _ in 0..count {
        obligations.push(ObligationIdentityV1(reader.array_32()?));
    }
    EvidenceV2::new(origin, obligations).map_err(DecodeHandoffErrorV2::InvalidModel)
}

fn encode_scalar_constant(bytes: &mut Vec<u8>, value: ScalarConstantV2) {
    put_u8(bytes, scalar_type_tag(value.scalar_type));
    put_u64(bytes, value.bits);
}

fn decode_scalar_constant(
    reader: &mut Reader<'_>,
) -> Result<ScalarConstantV2, DecodeHandoffErrorV2> {
    let scalar_type = decode_scalar_type(reader.u8()?)?;
    ScalarConstantV2::new(scalar_type, reader.u64()?).map_err(DecodeHandoffErrorV2::InvalidModel)
}

fn encode_intrinsic(bytes: &mut Vec<u8>, intrinsic: IntrinsicV2) {
    match intrinsic {
        IntrinsicV2::AmdGpuWorkitemId(axis) => {
            put_u8(bytes, 1);
            put_u8(bytes, axis_tag(axis));
        }
        IntrinsicV2::AmdGpuWorkgroupId(axis) => {
            put_u8(bytes, 2);
            put_u8(bytes, axis_tag(axis));
        }
        IntrinsicV2::AmdGpuBarrier => put_u8(bytes, 3),
        IntrinsicV2::FmaF32 => put_u8(bytes, 4),
        IntrinsicV2::SqrtF32 => put_u8(bytes, 5),
        IntrinsicV2::Trap => put_u8(bytes, 6),
        IntrinsicV2::AmdGpuMfmaF32_16x16x16Bf16_1k => put_u8(bytes, 7),
    }
}

fn decode_intrinsic(reader: &mut Reader<'_>) -> Result<IntrinsicV2, DecodeHandoffErrorV2> {
    match reader.u8()? {
        1 => Ok(IntrinsicV2::AmdGpuWorkitemId(decode_axis(reader.u8()?)?)),
        2 => Ok(IntrinsicV2::AmdGpuWorkgroupId(decode_axis(reader.u8()?)?)),
        3 => Ok(IntrinsicV2::AmdGpuBarrier),
        4 => Ok(IntrinsicV2::FmaF32),
        5 => Ok(IntrinsicV2::SqrtF32),
        6 => Ok(IntrinsicV2::Trap),
        7 => Ok(IntrinsicV2::AmdGpuMfmaF32_16x16x16Bf16_1k),
        tag => Err(DecodeHandoffErrorV2::UnknownTag {
            section: WireSectionV2::Intrinsic,
            tag,
        }),
    }
}

fn encode_typed_value(bytes: &mut Vec<u8>, value: TypedValueV2) {
    put_u32(bytes, value.id.get());
    encode_value_type(bytes, value.value_type);
}

fn decode_typed_value(reader: &mut Reader<'_>) -> Result<TypedValueV2, DecodeHandoffErrorV2> {
    Ok(TypedValueV2::new(
        ValueIdV2::new(reader.u32()?),
        decode_value_type(reader)?,
    ))
}

fn encode_value_type(bytes: &mut Vec<u8>, value_type: ValueTypeV2) {
    match value_type {
        ValueTypeV2::Scalar(scalar) => {
            put_u8(bytes, 1);
            put_u8(bytes, scalar_type_tag(scalar));
        }
        ValueTypeV2::Pointer {
            pointee,
            address_space,
        } => {
            put_u8(bytes, 2);
            put_u8(bytes, scalar_type_tag(pointee));
            put_u8(bytes, address_space_tag(address_space));
        }
        ValueTypeV2::Vector { element, lanes } => {
            put_u8(bytes, 3);
            put_u8(bytes, scalar_type_tag(element));
            put_u8(bytes, lanes);
        }
        ValueTypeV2::ArrayPointer {
            element,
            elements,
            address_space,
        } => {
            put_u8(bytes, 4);
            put_u8(bytes, scalar_type_tag(element));
            put_u16(bytes, elements);
            put_u8(bytes, address_space_tag(address_space));
        }
    }
}

fn decode_value_type(reader: &mut Reader<'_>) -> Result<ValueTypeV2, DecodeHandoffErrorV2> {
    match reader.u8()? {
        1 => Ok(ValueTypeV2::Scalar(decode_scalar_type(reader.u8()?)?)),
        2 => Ok(ValueTypeV2::Pointer {
            pointee: decode_scalar_type(reader.u8()?)?,
            address_space: decode_address_space(reader.u8()?)?,
        }),
        3 => {
            let element = decode_scalar_type(reader.u8()?)?;
            let lanes = reader.u8()?;
            ValueTypeV2::fixed_vector(element, usize::from(lanes))
                .map_err(DecodeHandoffErrorV2::InvalidModel)
        }
        4 => {
            let element = decode_scalar_type(reader.u8()?)?;
            let elements = reader.u16()?;
            let address_space = decode_address_space(reader.u8()?)?;
            ValueTypeV2::array_pointer(element, usize::from(elements), address_space)
                .map_err(DecodeHandoffErrorV2::InvalidModel)
        }
        tag => Err(DecodeHandoffErrorV2::UnknownTag {
            section: WireSectionV2::ValueType,
            tag,
        }),
    }
}

fn encode_return_type(bytes: &mut Vec<u8>, return_type: ReturnTypeV2) {
    match return_type {
        ReturnTypeV2::Void => put_u8(bytes, 1),
        ReturnTypeV2::Value(value_type) => {
            put_u8(bytes, 2);
            encode_value_type(bytes, value_type);
        }
    }
}

fn decode_return_type(reader: &mut Reader<'_>) -> Result<ReturnTypeV2, DecodeHandoffErrorV2> {
    match reader.u8()? {
        1 => Ok(ReturnTypeV2::Void),
        2 => Ok(ReturnTypeV2::Value(decode_value_type(reader)?)),
        tag => Err(DecodeHandoffErrorV2::UnknownTag {
            section: WireSectionV2::ReturnType,
            tag,
        }),
    }
}

fn encode_parameter_attribute(bytes: &mut Vec<u8>, attribute: ParameterAttributeV1) {
    put_u8(bytes, attribute.kind());
    match attribute {
        ParameterAttributeV1::Align(value) => put_u16(bytes, value),
        ParameterAttributeV1::Dereferenceable(value) => put_u32(bytes, value),
        _ => {}
    }
}

fn decode_parameter_attribute(
    reader: &mut Reader<'_>,
) -> Result<ParameterAttributeV1, DecodeHandoffErrorV2> {
    Ok(match reader.u8()? {
        1 => ParameterAttributeV1::NoAlias,
        2 => ParameterAttributeV1::NoCapture,
        3 => ParameterAttributeV1::NonNull,
        4 => ParameterAttributeV1::ReadOnly,
        5 => ParameterAttributeV1::WriteOnly,
        6 => ParameterAttributeV1::Align(reader.u16()?),
        7 => ParameterAttributeV1::Dereferenceable(reader.u32()?),
        tag => {
            return Err(DecodeHandoffErrorV2::UnknownTag {
                section: WireSectionV2::ParameterAttribute,
                tag,
            });
        }
    })
}

fn encode_function_attribute(bytes: &mut Vec<u8>, attribute: FunctionAttributeV2) {
    put_u8(bytes, attribute.kind());
    match attribute {
        FunctionAttributeV2::FlatWorkgroupSize(range) => {
            put_u16(bytes, range.minimum());
            put_u16(bytes, range.maximum());
        }
        FunctionAttributeV2::WavesPerEu(range) => {
            put_u8(bytes, range.minimum());
            put_u8(bytes, range.maximum());
        }
        FunctionAttributeV2::RequiredWorkgroupSize(shape) => {
            for extent in shape {
                put_u16(bytes, extent);
            }
        }
        _ => {}
    }
}

fn decode_function_attribute(
    reader: &mut Reader<'_>,
) -> Result<FunctionAttributeV2, DecodeHandoffErrorV2> {
    Ok(match reader.u8()? {
        1 => FunctionAttributeV2::NoUnwind,
        2 => FunctionAttributeV2::AlwaysInline,
        3 => FunctionAttributeV2::NoInline,
        4 => FunctionAttributeV2::ReadNone,
        5 => FunctionAttributeV2::WillReturn,
        6 => FunctionAttributeV2::FlatWorkgroupSize(
            WorkgroupSizeRangeV1::new(reader.u16()?, reader.u16()?)
                .map_err(|error| DecodeHandoffErrorV2::InvalidModel(map_v1_error(error)))?,
        ),
        7 => FunctionAttributeV2::WavesPerEu(
            WavesPerEuV1::new(reader.u8()?, reader.u8()?)
                .map_err(|error| DecodeHandoffErrorV2::InvalidModel(map_v1_error(error)))?,
        ),
        8 => FunctionAttributeV2::DenormalFpMathF32Ieee,
        9 => FunctionAttributeV2::UnsafeFpMathDisabled,
        10 => FunctionAttributeV2::NoInfsFpMathDisabled,
        11 => FunctionAttributeV2::NoNansFpMathDisabled,
        12 => FunctionAttributeV2::NoSignedZerosFpMathDisabled,
        13 => FunctionAttributeV2::ApproxFuncFpMathDisabled,
        14 => FunctionAttributeV2::FpContractOff,
        15 => FunctionAttributeV2::RequiredWorkgroupSize([
            reader.u16()?,
            reader.u16()?,
            reader.u16()?,
        ]),
        16 => FunctionAttributeV2::NoCompletionAction,
        17 => FunctionAttributeV2::NoDefaultQueue,
        18 => FunctionAttributeV2::NoHeapPointer,
        19 => FunctionAttributeV2::NoHostcallPointer,
        20 => FunctionAttributeV2::NoMultigridSyncArgument,
        21 => FunctionAttributeV2::NoQueuePointer,
        tag => {
            return Err(DecodeHandoffErrorV2::UnknownTag {
                section: WireSectionV2::FunctionAttribute,
                tag,
            });
        }
    })
}

fn encode_binary_operation(bytes: &mut Vec<u8>, operation: BinaryOperationV2) {
    match operation {
        BinaryOperationV2::Integer(operation) => {
            put_u8(bytes, 1);
            put_u8(bytes, integer_binary_tag(operation));
        }
        BinaryOperationV2::Float(operation) => {
            put_u8(bytes, 2);
            put_u8(bytes, float_binary_tag(operation));
        }
    }
}

fn decode_binary_operation(
    reader: &mut Reader<'_>,
) -> Result<BinaryOperationV2, DecodeHandoffErrorV2> {
    match reader.u8()? {
        1 => Ok(BinaryOperationV2::Integer(decode_integer_binary(
            reader.u8()?,
        )?)),
        2 => Ok(BinaryOperationV2::Float(decode_float_binary(reader.u8()?)?)),
        tag => Err(DecodeHandoffErrorV2::UnknownTag {
            section: WireSectionV2::BinaryOperation,
            tag,
        }),
    }
}

fn encode_call_target(bytes: &mut Vec<u8>, target: CallTargetV2) {
    match target {
        CallTargetV2::Function(function) => {
            put_u8(bytes, 1);
            put_u32(bytes, function.get());
        }
        CallTargetV2::Intrinsic(intrinsic) => {
            put_u8(bytes, 2);
            encode_intrinsic(bytes, intrinsic);
        }
    }
}

fn decode_call_target(reader: &mut Reader<'_>) -> Result<CallTargetV2, DecodeHandoffErrorV2> {
    match reader.u8()? {
        1 => Ok(CallTargetV2::Function(FunctionIdV2::new(reader.u32()?))),
        2 => Ok(CallTargetV2::Intrinsic(decode_intrinsic(reader)?)),
        tag => Err(DecodeHandoffErrorV2::UnknownTag {
            section: WireSectionV2::CallTarget,
            tag,
        }),
    }
}

fn decode_module_flag(tag: u8) -> Result<ModuleFlagV1, DecodeHandoffErrorV2> {
    match tag {
        1 => Ok(ModuleFlagV1::CodeObjectVersion6),
        2 => Ok(ModuleFlagV1::PicLevel2),
        3 => Ok(ModuleFlagV1::WcharSize4),
        tag => Err(DecodeHandoffErrorV2::UnknownTag {
            section: WireSectionV2::ModuleFlag,
            tag,
        }),
    }
}

fn decode_global_linkage(tag: u8) -> Result<GlobalLinkageV2, DecodeHandoffErrorV2> {
    match tag {
        1 => Ok(GlobalLinkageV2::Internal),
        2 => Ok(GlobalLinkageV2::External),
        tag => Err(DecodeHandoffErrorV2::UnknownTag {
            section: WireSectionV2::GlobalLinkage,
            tag,
        }),
    }
}

fn decode_function_kind(tag: u8) -> Result<FunctionKindV2, DecodeHandoffErrorV2> {
    match tag {
        1 => Ok(FunctionKindV2::Kernel),
        2 => Ok(FunctionKindV2::Helper),
        tag => Err(DecodeHandoffErrorV2::UnknownTag {
            section: WireSectionV2::FunctionKind,
            tag,
        }),
    }
}

fn decode_calling_convention(tag: u8) -> Result<CallingConventionV2, DecodeHandoffErrorV2> {
    match tag {
        1 => Ok(CallingConventionV2::C),
        2 => Ok(CallingConventionV2::AmdGpuKernel),
        tag => Err(DecodeHandoffErrorV2::UnknownTag {
            section: WireSectionV2::CallingConvention,
            tag,
        }),
    }
}

fn decode_axis(tag: u8) -> Result<AxisV2, DecodeHandoffErrorV2> {
    match tag {
        1 => Ok(AxisV2::X),
        2 => Ok(AxisV2::Y),
        3 => Ok(AxisV2::Z),
        tag => Err(DecodeHandoffErrorV2::UnknownTag {
            section: WireSectionV2::Axis,
            tag,
        }),
    }
}

fn decode_scalar_type(tag: u8) -> Result<ScalarTypeV1, DecodeHandoffErrorV2> {
    match tag {
        1 => Ok(ScalarTypeV1::I1),
        2 => Ok(ScalarTypeV1::I8),
        3 => Ok(ScalarTypeV1::I16),
        4 => Ok(ScalarTypeV1::I32),
        5 => Ok(ScalarTypeV1::I64),
        6 => Ok(ScalarTypeV1::F16),
        7 => Ok(ScalarTypeV1::Bf16),
        8 => Ok(ScalarTypeV1::F32),
        9 => Ok(ScalarTypeV1::F64),
        tag => Err(DecodeHandoffErrorV2::UnknownTag {
            section: WireSectionV2::ScalarType,
            tag,
        }),
    }
}

fn decode_address_space(tag: u8) -> Result<AddressSpaceV1, DecodeHandoffErrorV2> {
    match tag {
        0 => Ok(AddressSpaceV1::Flat),
        1 => Ok(AddressSpaceV1::Global),
        2 => Ok(AddressSpaceV1::Region),
        3 => Ok(AddressSpaceV1::Local),
        4 => Ok(AddressSpaceV1::Constant),
        5 => Ok(AddressSpaceV1::Private),
        tag => Err(DecodeHandoffErrorV2::UnknownTag {
            section: WireSectionV2::AddressSpace,
            tag,
        }),
    }
}

fn decode_integer_binary(tag: u8) -> Result<IntegerBinaryOperationV2, DecodeHandoffErrorV2> {
    match tag {
        1 => Ok(IntegerBinaryOperationV2::Add),
        2 => Ok(IntegerBinaryOperationV2::Subtract),
        3 => Ok(IntegerBinaryOperationV2::Multiply),
        4 => Ok(IntegerBinaryOperationV2::And),
        5 => Ok(IntegerBinaryOperationV2::Or),
        6 => Ok(IntegerBinaryOperationV2::Xor),
        7 => Ok(IntegerBinaryOperationV2::ShiftLeft),
        8 => Ok(IntegerBinaryOperationV2::LogicalShiftRight),
        9 => Ok(IntegerBinaryOperationV2::ArithmeticShiftRight),
        tag => Err(DecodeHandoffErrorV2::UnknownTag {
            section: WireSectionV2::IntegerBinaryOperation,
            tag,
        }),
    }
}

fn decode_float_binary(tag: u8) -> Result<FloatBinaryOperationV2, DecodeHandoffErrorV2> {
    match tag {
        1 => Ok(FloatBinaryOperationV2::Add),
        2 => Ok(FloatBinaryOperationV2::Subtract),
        3 => Ok(FloatBinaryOperationV2::Multiply),
        4 => Ok(FloatBinaryOperationV2::Divide),
        tag => Err(DecodeHandoffErrorV2::UnknownTag {
            section: WireSectionV2::FloatBinaryOperation,
            tag,
        }),
    }
}

fn decode_compare_predicate(tag: u8) -> Result<ComparePredicateV2, DecodeHandoffErrorV2> {
    match tag {
        1 => Ok(ComparePredicateV2::IntegerEqual),
        2 => Ok(ComparePredicateV2::IntegerNotEqual),
        3 => Ok(ComparePredicateV2::UnsignedLessThan),
        4 => Ok(ComparePredicateV2::UnsignedLessOrEqual),
        5 => Ok(ComparePredicateV2::SignedLessThan),
        6 => Ok(ComparePredicateV2::SignedLessOrEqual),
        7 => Ok(ComparePredicateV2::OrderedEqual),
        8 => Ok(ComparePredicateV2::OrderedNotEqual),
        9 => Ok(ComparePredicateV2::OrderedLessThan),
        10 => Ok(ComparePredicateV2::OrderedLessOrEqual),
        tag => Err(DecodeHandoffErrorV2::UnknownTag {
            section: WireSectionV2::ComparePredicate,
            tag,
        }),
    }
}

fn decode_cast_operation(tag: u8) -> Result<CastOperationV2, DecodeHandoffErrorV2> {
    match tag {
        1 => Ok(CastOperationV2::ZeroExtend),
        2 => Ok(CastOperationV2::SignExtend),
        3 => Ok(CastOperationV2::Truncate),
        4 => Ok(CastOperationV2::FloatExtend),
        5 => Ok(CastOperationV2::FloatTruncate),
        6 => Ok(CastOperationV2::UnsignedIntToFloat),
        7 => Ok(CastOperationV2::SignedIntToFloat),
        8 => Ok(CastOperationV2::FloatToUnsignedInt),
        9 => Ok(CastOperationV2::FloatToSignedInt),
        10 => Ok(CastOperationV2::PointerToInt),
        tag => Err(DecodeHandoffErrorV2::UnknownTag {
            section: WireSectionV2::CastOperation,
            tag,
        }),
    }
}

const fn module_flag_tag(flag: ModuleFlagV1) -> u8 {
    match flag {
        ModuleFlagV1::CodeObjectVersion6 => 1,
        ModuleFlagV1::PicLevel2 => 2,
        ModuleFlagV1::WcharSize4 => 3,
    }
}

const fn global_linkage_tag(linkage: GlobalLinkageV2) -> u8 {
    match linkage {
        GlobalLinkageV2::Internal => 1,
        GlobalLinkageV2::External => 2,
    }
}

const fn function_kind_tag(kind: FunctionKindV2) -> u8 {
    match kind {
        FunctionKindV2::Kernel => 1,
        FunctionKindV2::Helper => 2,
    }
}

const fn calling_convention_tag(convention: CallingConventionV2) -> u8 {
    match convention {
        CallingConventionV2::C => 1,
        CallingConventionV2::AmdGpuKernel => 2,
    }
}

const fn axis_tag(axis: AxisV2) -> u8 {
    match axis {
        AxisV2::X => 1,
        AxisV2::Y => 2,
        AxisV2::Z => 3,
    }
}

const fn scalar_type_tag(scalar: ScalarTypeV1) -> u8 {
    match scalar {
        ScalarTypeV1::I1 => 1,
        ScalarTypeV1::I8 => 2,
        ScalarTypeV1::I16 => 3,
        ScalarTypeV1::I32 => 4,
        ScalarTypeV1::I64 => 5,
        ScalarTypeV1::F16 => 6,
        ScalarTypeV1::Bf16 => 7,
        ScalarTypeV1::F32 => 8,
        ScalarTypeV1::F64 => 9,
    }
}

const fn address_space_tag(address_space: AddressSpaceV1) -> u8 {
    match address_space {
        AddressSpaceV1::Flat => 0,
        AddressSpaceV1::Global => 1,
        AddressSpaceV1::Region => 2,
        AddressSpaceV1::Local => 3,
        AddressSpaceV1::Constant => 4,
        AddressSpaceV1::Private => 5,
    }
}

const fn integer_binary_tag(operation: IntegerBinaryOperationV2) -> u8 {
    match operation {
        IntegerBinaryOperationV2::Add => 1,
        IntegerBinaryOperationV2::Subtract => 2,
        IntegerBinaryOperationV2::Multiply => 3,
        IntegerBinaryOperationV2::And => 4,
        IntegerBinaryOperationV2::Or => 5,
        IntegerBinaryOperationV2::Xor => 6,
        IntegerBinaryOperationV2::ShiftLeft => 7,
        IntegerBinaryOperationV2::LogicalShiftRight => 8,
        IntegerBinaryOperationV2::ArithmeticShiftRight => 9,
    }
}

const fn float_binary_tag(operation: FloatBinaryOperationV2) -> u8 {
    match operation {
        FloatBinaryOperationV2::Add => 1,
        FloatBinaryOperationV2::Subtract => 2,
        FloatBinaryOperationV2::Multiply => 3,
        FloatBinaryOperationV2::Divide => 4,
    }
}

const fn compare_predicate_tag(predicate: ComparePredicateV2) -> u8 {
    match predicate {
        ComparePredicateV2::IntegerEqual => 1,
        ComparePredicateV2::IntegerNotEqual => 2,
        ComparePredicateV2::UnsignedLessThan => 3,
        ComparePredicateV2::UnsignedLessOrEqual => 4,
        ComparePredicateV2::SignedLessThan => 5,
        ComparePredicateV2::SignedLessOrEqual => 6,
        ComparePredicateV2::OrderedEqual => 7,
        ComparePredicateV2::OrderedNotEqual => 8,
        ComparePredicateV2::OrderedLessThan => 9,
        ComparePredicateV2::OrderedLessOrEqual => 10,
    }
}

const fn cast_operation_tag(operation: CastOperationV2) -> u8 {
    match operation {
        CastOperationV2::ZeroExtend => 1,
        CastOperationV2::SignExtend => 2,
        CastOperationV2::Truncate => 3,
        CastOperationV2::FloatExtend => 4,
        CastOperationV2::FloatTruncate => 5,
        CastOperationV2::UnsignedIntToFloat => 6,
        CastOperationV2::SignedIntToFloat => 7,
        CastOperationV2::FloatToUnsignedInt => 8,
        CastOperationV2::FloatToSignedInt => 9,
        CastOperationV2::PointerToInt => 10,
    }
}

fn decode_bool(tag: u8, section: WireSectionV2) -> Result<bool, DecodeHandoffErrorV2> {
    match tag {
        0 => Ok(false),
        1 => Ok(true),
        tag => Err(DecodeHandoffErrorV2::UnknownTag { section, tag }),
    }
}

fn map_v1_error(error: crate::HandoffDiagnosticV1) -> HandoffDiagnosticV2 {
    match error {
        crate::HandoffDiagnosticV1::ZeroIdentity(_) => HandoffDiagnosticV2::InvalidIdentity,
        crate::HandoffDiagnosticV1::InvalidWorkgroupSizeRange
        | crate::HandoffDiagnosticV1::InvalidWavesPerEu => {
            HandoffDiagnosticV2::InvalidFunctionAttribute
        }
        _ => HandoffDiagnosticV2::UnsupportedInstruction,
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], DecodeHandoffErrorV2> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(DecodeHandoffErrorV2::Truncated {
                offset: self.offset,
            })?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(DecodeHandoffErrorV2::Truncated {
                offset: self.offset,
            })?;
        self.offset = end;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, DecodeHandoffErrorV2> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, DecodeHandoffErrorV2> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn u32(&mut self) -> Result<u32, DecodeHandoffErrorV2> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn u64(&mut self) -> Result<u64, DecodeHandoffErrorV2> {
        let bytes = self.take(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn array_32(&mut self) -> Result<[u8; 32], DecodeHandoffErrorV2> {
        let mut output = [0; 32];
        output.copy_from_slice(self.take(32)?);
        Ok(output)
    }

    fn bounded_u8(
        &mut self,
        limit: HandoffLimitV2,
        maximum: usize,
    ) -> Result<usize, DecodeHandoffErrorV2> {
        let observed = self.u8()? as usize;
        check_wire_limit(limit, observed, maximum)?;
        Ok(observed)
    }

    fn bounded_u16(
        &mut self,
        limit: HandoffLimitV2,
        maximum: usize,
    ) -> Result<usize, DecodeHandoffErrorV2> {
        let observed = self.u16()? as usize;
        check_wire_limit(limit, observed, maximum)?;
        Ok(observed)
    }

    fn bounded_u32(
        &mut self,
        limit: HandoffLimitV2,
        maximum: usize,
    ) -> Result<usize, DecodeHandoffErrorV2> {
        let observed = usize::try_from(self.u32()?).expect("u32 fits usize");
        check_wire_limit(limit, observed, maximum)?;
        Ok(observed)
    }

    fn string(
        &mut self,
        maximum: usize,
        limit: HandoffLimitV2,
        section: WireSectionV2,
    ) -> Result<String, DecodeHandoffErrorV2> {
        let observed = self.u16()? as usize;
        check_wire_limit(limit, observed, maximum)?;
        let bytes = self.take(observed)?;
        let text = str::from_utf8(bytes).map_err(|_| DecodeHandoffErrorV2::InvalidUtf8(section))?;
        Ok(String::from(text))
    }

    fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

fn check_wire_limit(
    limit: HandoffLimitV2,
    observed: usize,
    maximum: usize,
) -> Result<(), DecodeHandoffErrorV2> {
    if observed > maximum {
        return Err(DecodeHandoffErrorV2::LimitExceeded {
            limit,
            observed,
            maximum,
        });
    }
    Ok(())
}

fn put_u8(bytes: &mut Vec<u8>, value: u8) {
    bytes.push(value);
}

fn put_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn put_string(bytes: &mut Vec<u8>, value: &str) {
    put_u16(
        bytes,
        u16::try_from(value.len()).expect("bounded V2 handoff string fits u16"),
    );
    bytes.extend_from_slice(value.as_bytes());
}

const _: () = assert!(HEADER_BYTES_V2 == MAGIC_V2.len() + 2 + 2 + 4);

#[cfg(test)]
mod tests {
    use super::*;

    const ABI_ATTRIBUTES: [FunctionAttributeV2; 6] = [
        FunctionAttributeV2::NoCompletionAction,
        FunctionAttributeV2::NoDefaultQueue,
        FunctionAttributeV2::NoHeapPointer,
        FunctionAttributeV2::NoHostcallPointer,
        FunctionAttributeV2::NoMultigridSyncArgument,
        FunctionAttributeV2::NoQueuePointer,
    ];

    #[test]
    fn abi_function_attribute_tags_are_stable_and_closed() {
        for (index, attribute) in ABI_ATTRIBUTES.into_iter().enumerate() {
            let tag = 16 + u8::try_from(index).unwrap();
            let mut bytes = Vec::new();
            encode_function_attribute(&mut bytes, attribute);
            assert_eq!(bytes, [tag]);
            assert_eq!(
                decode_function_attribute(&mut Reader::new(&bytes)).unwrap(),
                attribute
            );
        }

        assert_eq!(
            decode_function_attribute(&mut Reader::new(&[22])),
            Err(DecodeHandoffErrorV2::UnknownTag {
                section: WireSectionV2::FunctionAttribute,
                tag: 22,
            })
        );
    }
}
