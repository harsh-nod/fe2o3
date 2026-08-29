use alloc::{string::String, vec::Vec};
use core::{fmt, str};

use crate::model::{obligation_kind_tag, origin_kind_tag};
use crate::{
    AddressSpaceV1, CodeModelV1, CodeObjectVersionV1, DeviceLibraryInputV1, DeviceLibraryKindV1,
    FunctionAttributeV1, Gfx942HandoffInputV1, Gfx942HandoffV1, Gfx942TargetPolicyV1,
    HandoffDiagnosticV1, HandoffLimitV1, IdentityV1, KernelEntryV1, KernelParameterV1,
    KernelValueTypeV1, MAX_DEVICE_LIBRARIES_V1, MAX_FUNCTION_ATTRIBUTES_V1,
    MAX_KERNEL_PARAMETERS_V1, MAX_KERNELS_V1, MAX_MODULE_FLAGS_V1, MAX_NAMED_METADATA_V1,
    MAX_OBLIGATIONS_V1, MAX_ORIGINS_V1, MAX_PARAMETER_ATTRIBUTES_V1, MAX_SOURCE_PATH_BYTES_V1,
    MAX_SYMBOL_BYTES_V1, ModuleFlagV1, ModuleMetadataV1, NamedMetadataV1, ObligationIdentityV1,
    ObligationKindV1, ObligationV1, OptimizationLevelV1, OriginIdentityV1, OriginKindV1, OriginV1,
    ParameterAttributeV1, RelocationModelV1, ScalarTypeV1, SourceSpanV1, StageIdentitiesV1,
    TargetFeatureStateV1, TargetFeatureV1, WavesPerEuV1, WorkgroupSizeRangeV1,
};

pub const MAX_CANONICAL_HANDOFF_BYTES_V1: usize = 1024 * 1024;

const MAGIC_V1: &[u8; 8] = b"F2LLVMH1";
const VERSION_V1: u16 = 1;
const HEADER_BYTES_V1: usize = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalHandoffBytesV1 {
    bytes: Vec<u8>,
}

impl CanonicalHandoffBytesV1 {
    pub(crate) fn from_validated(bytes: Vec<u8>) -> Self {
        debug_assert!(bytes.len() <= MAX_CANONICAL_HANDOFF_BYTES_V1);
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

impl AsRef<[u8]> for CanonicalHandoffBytesV1 {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WireSectionV1 {
    Header,
    TargetTriple,
    DataLayout,
    Cpu,
    TargetFeature,
    CodeObjectPolicy,
    OptimizationLevel,
    RelocationModel,
    CodeModel,
    CallingConvention,
    ReturnType,
    ValueType,
    ScalarType,
    AddressSpace,
    ParameterAttribute,
    FunctionAttribute,
    ModuleFlag,
    NamedMetadata,
    DeviceLibrary,
    Origin,
    SourceSpan,
    Obligation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodeHandoffErrorV1 {
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
    InvalidUtf8(WireSectionV1),
    UnknownTag {
        section: WireSectionV1,
        tag: u8,
    },
    LimitExceeded {
        limit: HandoffLimitV1,
        observed: usize,
        maximum: usize,
    },
    InvalidModel(HandoffDiagnosticV1),
    NonCanonical,
}

impl fmt::Display for DecodeHandoffErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLong { observed, maximum } => write!(
                formatter,
                "canonical LLVM handoff has {observed} bytes, maximum is {maximum}"
            ),
            Self::Truncated { offset } => {
                write!(
                    formatter,
                    "canonical LLVM handoff is truncated at offset {offset}"
                )
            }
            Self::BadMagic => formatter.write_str("invalid canonical LLVM handoff magic"),
            Self::UnsupportedVersion(version) => {
                write!(
                    formatter,
                    "unsupported canonical LLVM handoff version {version}"
                )
            }
            Self::NonzeroReserved => {
                formatter.write_str("canonical LLVM handoff reserved bits are nonzero")
            }
            Self::LengthMismatch { declared, observed } => write!(
                formatter,
                "canonical LLVM handoff declares {declared} bytes but contains {observed}"
            ),
            Self::InvalidUtf8(section) => {
                write!(
                    formatter,
                    "canonical LLVM handoff {section:?} text is not UTF-8"
                )
            }
            Self::UnknownTag { section, tag } => {
                write!(
                    formatter,
                    "unknown canonical LLVM handoff {section:?} tag {tag}"
                )
            }
            Self::LimitExceeded {
                limit,
                observed,
                maximum,
            } => write!(
                formatter,
                "canonical LLVM handoff {limit:?} count {observed} exceeds {maximum}"
            ),
            Self::InvalidModel(error) => write!(formatter, "invalid LLVM handoff model: {error}"),
            Self::NonCanonical => {
                formatter.write_str("LLVM handoff bytes are valid but not canonical")
            }
        }
    }
}

impl core::error::Error for DecodeHandoffErrorV1 {}

pub(crate) fn encode_handoff_v1(handoff: &Gfx942HandoffV1) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(MAGIC_V1);
    put_u16(&mut bytes, VERSION_V1);
    put_u16(&mut bytes, 0);
    put_u32(&mut bytes, 0);

    put_u8(&mut bytes, 1); // amdgcn-amd-amdhsa
    put_u8(&mut bytes, 1); // canonical gfx942 data layout
    put_u8(&mut bytes, 1); // gfx942
    put_u8(
        &mut bytes,
        u8::try_from(handoff.target.features.len()).expect("bounded feature count fits u8"),
    );
    for feature in &handoff.target.features {
        put_u8(&mut bytes, target_feature_tag(feature.feature));
        put_u8(&mut bytes, u8::from(feature.enabled));
    }
    put_u8(&mut bytes, code_object_tag(handoff.target.code_object));
    put_u8(&mut bytes, optimization_tag(handoff.target.optimization));
    put_u8(&mut bytes, relocation_tag(handoff.target.relocation));
    put_u8(&mut bytes, code_model_tag(handoff.target.code_model));

    bytes.extend_from_slice(handoff.stage_identities.semantic.as_bytes());
    bytes.extend_from_slice(handoff.stage_identities.schedule.as_bytes());
    bytes.extend_from_slice(handoff.stage_identities.target_plan.as_bytes());

    put_u16(
        &mut bytes,
        u16::try_from(handoff.kernels.len()).expect("bounded kernel count fits u16"),
    );
    for kernel in &handoff.kernels {
        put_string(&mut bytes, &kernel.symbol);
        bytes.extend_from_slice(kernel.origin.as_bytes());
        put_u8(&mut bytes, 1); // amdgpu_kernel
        put_u8(&mut bytes, 1); // void
        put_u16(
            &mut bytes,
            u16::try_from(kernel.parameters.len()).expect("bounded parameter count fits u16"),
        );
        for parameter in &kernel.parameters {
            put_string(&mut bytes, &parameter.name);
            encode_value_type(&mut bytes, parameter.value_type);
            put_u8(
                &mut bytes,
                u8::try_from(parameter.attributes.len())
                    .expect("bounded parameter attribute count fits u8"),
            );
            for attribute in &parameter.attributes {
                encode_parameter_attribute(&mut bytes, *attribute);
            }
        }
        put_u8(
            &mut bytes,
            u8::try_from(kernel.function_attributes.len())
                .expect("bounded function attribute count fits u8"),
        );
        for attribute in &kernel.function_attributes {
            encode_function_attribute(&mut bytes, *attribute);
        }
    }

    put_u8(
        &mut bytes,
        u8::try_from(handoff.module.flags.len()).expect("bounded module flag count fits u8"),
    );
    for flag in &handoff.module.flags {
        put_u8(&mut bytes, module_flag_tag(*flag));
    }
    put_u8(
        &mut bytes,
        u8::try_from(handoff.module.named.len()).expect("bounded metadata count fits u8"),
    );
    for metadata in &handoff.module.named {
        put_u8(&mut bytes, metadata.kind());
        if let NamedMetadataV1::ProducerIdentity(identity) = metadata {
            bytes.extend_from_slice(identity.as_bytes());
        }
    }
    put_u8(
        &mut bytes,
        u8::try_from(handoff.module.device_libraries.len())
            .expect("bounded device-library count fits u8"),
    );
    for library in &handoff.module.device_libraries {
        put_u8(&mut bytes, device_library_tag(library.kind));
        bytes.extend_from_slice(library.sha256.as_bytes());
        put_u64(&mut bytes, library.byte_len);
    }

    put_u16(
        &mut bytes,
        u16::try_from(handoff.origins.len()).expect("bounded origin count fits u16"),
    );
    for origin in &handoff.origins {
        bytes.extend_from_slice(origin.identity.as_bytes());
        put_u8(&mut bytes, origin_kind_tag(origin.kind));
        bytes.extend_from_slice(origin.source_identity.as_bytes());
        match &origin.span {
            None => put_u8(&mut bytes, 0),
            Some(span) => {
                put_u8(&mut bytes, 1);
                put_string(&mut bytes, &span.path);
                put_u32(&mut bytes, span.start_line);
                put_u32(&mut bytes, span.start_column);
                put_u32(&mut bytes, span.end_line);
                put_u32(&mut bytes, span.end_column);
            }
        }
    }

    put_u16(
        &mut bytes,
        u16::try_from(handoff.obligations.len()).expect("bounded obligation count fits u16"),
    );
    for obligation in &handoff.obligations {
        bytes.extend_from_slice(obligation.identity.as_bytes());
        put_u8(&mut bytes, obligation_kind_tag(obligation.kind));
        bytes.extend_from_slice(obligation.subject.as_bytes());
        bytes.extend_from_slice(obligation.origin.as_bytes());
    }

    let total_len = u32::try_from(bytes.len()).expect("bounded canonical handoff fits u32");
    bytes[12..16].copy_from_slice(&total_len.to_le_bytes());
    bytes
}

pub(crate) fn decode_handoff_v1(bytes: &[u8]) -> Result<Gfx942HandoffV1, DecodeHandoffErrorV1> {
    if bytes.len() > MAX_CANONICAL_HANDOFF_BYTES_V1 {
        return Err(DecodeHandoffErrorV1::TooLong {
            observed: bytes.len(),
            maximum: MAX_CANONICAL_HANDOFF_BYTES_V1,
        });
    }
    let mut reader = Reader::new(bytes);
    if reader.take(MAGIC_V1.len())? != MAGIC_V1 {
        return Err(DecodeHandoffErrorV1::BadMagic);
    }
    let version = reader.u16()?;
    if version != VERSION_V1 {
        return Err(DecodeHandoffErrorV1::UnsupportedVersion(version));
    }
    if reader.u16()? != 0 {
        return Err(DecodeHandoffErrorV1::NonzeroReserved);
    }
    let declared = usize::try_from(reader.u32()?).expect("u32 fits usize");
    if declared != bytes.len() {
        return Err(DecodeHandoffErrorV1::LengthMismatch {
            declared,
            observed: bytes.len(),
        });
    }

    expect_tag(&mut reader, WireSectionV1::TargetTriple, 1)?;
    expect_tag(&mut reader, WireSectionV1::DataLayout, 1)?;
    expect_tag(&mut reader, WireSectionV1::Cpu, 1)?;
    let feature_count = reader.u8()? as usize;
    if feature_count > 3 {
        return Err(DecodeHandoffErrorV1::LimitExceeded {
            limit: HandoffLimitV1::FunctionAttributes,
            observed: feature_count,
            maximum: 3,
        });
    }
    let mut features = Vec::with_capacity(feature_count);
    for _ in 0..feature_count {
        let feature = decode_target_feature(reader.u8()?)?;
        let enabled = match reader.u8()? {
            0 => false,
            1 => true,
            tag => {
                return Err(DecodeHandoffErrorV1::UnknownTag {
                    section: WireSectionV1::TargetFeature,
                    tag,
                });
            }
        };
        features.push(TargetFeatureStateV1::new(feature, enabled));
    }
    let code_object = decode_code_object(reader.u8()?)?;
    let optimization = decode_optimization(reader.u8()?)?;
    let relocation = decode_relocation(reader.u8()?)?;
    let code_model = decode_code_model(reader.u8()?)?;
    let target = Gfx942TargetPolicyV1::from_parts(
        features,
        code_object,
        optimization,
        relocation,
        code_model,
    )
    .map_err(DecodeHandoffErrorV1::InvalidModel)?;

    let stage_identities =
        StageIdentitiesV1::new(reader.array_32()?, reader.array_32()?, reader.array_32()?)
            .map_err(DecodeHandoffErrorV1::InvalidModel)?;

    let kernel_count = reader.bounded_u16(HandoffLimitV1::Kernels, MAX_KERNELS_V1)?;
    let mut kernels = Vec::with_capacity(kernel_count);
    for _ in 0..kernel_count {
        let symbol = reader.string(
            MAX_SYMBOL_BYTES_V1,
            HandoffLimitV1::SymbolBytes,
            WireSectionV1::CallingConvention,
        )?;
        let origin = OriginIdentityV1(reader.array_32()?);
        expect_tag(&mut reader, WireSectionV1::CallingConvention, 1)?;
        expect_tag(&mut reader, WireSectionV1::ReturnType, 1)?;
        let parameter_count =
            reader.bounded_u16(HandoffLimitV1::KernelParameters, MAX_KERNEL_PARAMETERS_V1)?;
        let mut parameters = Vec::with_capacity(parameter_count);
        for _ in 0..parameter_count {
            let name = reader.string(
                MAX_SYMBOL_BYTES_V1,
                HandoffLimitV1::SymbolBytes,
                WireSectionV1::ValueType,
            )?;
            let value_type = decode_value_type(&mut reader)?;
            let attribute_count = reader.bounded_u8(
                HandoffLimitV1::ParameterAttributes,
                MAX_PARAMETER_ATTRIBUTES_V1,
            )?;
            let mut attributes = Vec::with_capacity(attribute_count);
            for _ in 0..attribute_count {
                attributes.push(decode_parameter_attribute(&mut reader)?);
            }
            parameters.push(
                KernelParameterV1::new(&name, value_type, attributes)
                    .map_err(DecodeHandoffErrorV1::InvalidModel)?,
            );
        }
        let function_attribute_count = reader.bounded_u8(
            HandoffLimitV1::FunctionAttributes,
            MAX_FUNCTION_ATTRIBUTES_V1,
        )?;
        let mut function_attributes = Vec::with_capacity(function_attribute_count);
        for _ in 0..function_attribute_count {
            function_attributes.push(decode_function_attribute(&mut reader)?);
        }
        kernels.push(
            KernelEntryV1::new(&symbol, parameters, function_attributes, origin)
                .map_err(DecodeHandoffErrorV1::InvalidModel)?,
        );
    }

    let flag_count = reader.bounded_u8(HandoffLimitV1::ModuleFlags, MAX_MODULE_FLAGS_V1)?;
    let mut flags = Vec::with_capacity(flag_count);
    for _ in 0..flag_count {
        flags.push(decode_module_flag(reader.u8()?)?);
    }
    let named_count = reader.bounded_u8(HandoffLimitV1::NamedMetadata, MAX_NAMED_METADATA_V1)?;
    let mut named = Vec::with_capacity(named_count);
    for _ in 0..named_count {
        named.push(match reader.u8()? {
            1 => NamedMetadataV1::OpenClVersion2_0,
            2 => NamedMetadataV1::OpenClSpirVersion2_0,
            3 => NamedMetadataV1::ProducerIdentity(
                IdentityV1::named(reader.array_32()?, "producer")
                    .map_err(DecodeHandoffErrorV1::InvalidModel)?,
            ),
            tag => {
                return Err(DecodeHandoffErrorV1::UnknownTag {
                    section: WireSectionV1::NamedMetadata,
                    tag,
                });
            }
        });
    }
    let library_count =
        reader.bounded_u8(HandoffLimitV1::DeviceLibraries, MAX_DEVICE_LIBRARIES_V1)?;
    let mut device_libraries = Vec::with_capacity(library_count);
    for _ in 0..library_count {
        let kind = decode_device_library(reader.u8()?)?;
        let sha256 = reader.array_32()?;
        let byte_len = reader.u64()?;
        device_libraries.push(
            DeviceLibraryInputV1::new(kind, sha256, byte_len)
                .map_err(DecodeHandoffErrorV1::InvalidModel)?,
        );
    }
    let module = ModuleMetadataV1::new(flags, named, device_libraries)
        .map_err(DecodeHandoffErrorV1::InvalidModel)?;

    let origin_count = reader.bounded_u16(HandoffLimitV1::Origins, MAX_ORIGINS_V1)?;
    let mut origins = Vec::with_capacity(origin_count);
    for _ in 0..origin_count {
        let encoded_identity = OriginIdentityV1(reader.array_32()?);
        let kind = decode_origin_kind(reader.u8()?)?;
        let source_identity = IdentityV1::named(reader.array_32()?, "origin source")
            .map_err(DecodeHandoffErrorV1::InvalidModel)?;
        let span = match reader.u8()? {
            0 => None,
            1 => {
                let path = reader.string(
                    MAX_SOURCE_PATH_BYTES_V1,
                    HandoffLimitV1::SourcePathBytes,
                    WireSectionV1::SourceSpan,
                )?;
                Some(
                    SourceSpanV1::new(
                        &path,
                        reader.u32()?,
                        reader.u32()?,
                        reader.u32()?,
                        reader.u32()?,
                    )
                    .map_err(DecodeHandoffErrorV1::InvalidModel)?,
                )
            }
            tag => {
                return Err(DecodeHandoffErrorV1::UnknownTag {
                    section: WireSectionV1::SourceSpan,
                    tag,
                });
            }
        };
        let origin = OriginV1::new(kind, source_identity, span);
        if origin.identity() != encoded_identity {
            return Err(DecodeHandoffErrorV1::NonCanonical);
        }
        origins.push(origin);
    }

    let obligation_count = reader.bounded_u16(HandoffLimitV1::Obligations, MAX_OBLIGATIONS_V1)?;
    let mut obligations = Vec::with_capacity(obligation_count);
    for _ in 0..obligation_count {
        let encoded_identity = ObligationIdentityV1(reader.array_32()?);
        let kind = decode_obligation_kind(reader.u8()?)?;
        let subject = IdentityV1::named(reader.array_32()?, "obligation subject")
            .map_err(DecodeHandoffErrorV1::InvalidModel)?;
        let origin = OriginIdentityV1(reader.array_32()?);
        let obligation = ObligationV1::new(kind, subject, origin);
        if obligation.identity() != encoded_identity {
            return Err(DecodeHandoffErrorV1::NonCanonical);
        }
        obligations.push(obligation);
    }
    if !reader.is_finished() {
        return Err(DecodeHandoffErrorV1::LengthMismatch {
            declared,
            observed: reader.offset,
        });
    }

    let handoff = Gfx942HandoffV1::new(Gfx942HandoffInputV1 {
        stage_identities,
        target,
        kernels,
        module,
        origins,
        obligations,
    })
    .map_err(DecodeHandoffErrorV1::InvalidModel)?;
    if encode_handoff_v1(&handoff) != bytes {
        return Err(DecodeHandoffErrorV1::NonCanonical);
    }
    Ok(handoff)
}

fn encode_value_type(bytes: &mut Vec<u8>, value_type: KernelValueTypeV1) {
    match value_type {
        KernelValueTypeV1::Scalar(scalar) => {
            put_u8(bytes, 1);
            put_u8(bytes, scalar_type_tag(scalar));
        }
        KernelValueTypeV1::Pointer {
            pointee,
            address_space,
        } => {
            put_u8(bytes, 2);
            put_u8(bytes, scalar_type_tag(pointee));
            put_u8(bytes, address_space_tag(address_space));
        }
    }
}

fn decode_value_type(reader: &mut Reader<'_>) -> Result<KernelValueTypeV1, DecodeHandoffErrorV1> {
    match reader.u8()? {
        1 => Ok(KernelValueTypeV1::Scalar(decode_scalar_type(reader.u8()?)?)),
        2 => Ok(KernelValueTypeV1::Pointer {
            pointee: decode_scalar_type(reader.u8()?)?,
            address_space: decode_address_space(reader.u8()?)?,
        }),
        tag => Err(DecodeHandoffErrorV1::UnknownTag {
            section: WireSectionV1::ValueType,
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
) -> Result<ParameterAttributeV1, DecodeHandoffErrorV1> {
    Ok(match reader.u8()? {
        1 => ParameterAttributeV1::NoAlias,
        2 => ParameterAttributeV1::NoCapture,
        3 => ParameterAttributeV1::NonNull,
        4 => ParameterAttributeV1::ReadOnly,
        5 => ParameterAttributeV1::WriteOnly,
        6 => ParameterAttributeV1::Align(reader.u16()?),
        7 => ParameterAttributeV1::Dereferenceable(reader.u32()?),
        tag => {
            return Err(DecodeHandoffErrorV1::UnknownTag {
                section: WireSectionV1::ParameterAttribute,
                tag,
            });
        }
    })
}

fn encode_function_attribute(bytes: &mut Vec<u8>, attribute: FunctionAttributeV1) {
    put_u8(bytes, attribute.kind());
    match attribute {
        FunctionAttributeV1::FlatWorkgroupSize(range) => {
            put_u16(bytes, range.minimum);
            put_u16(bytes, range.maximum);
        }
        FunctionAttributeV1::WavesPerEu(range) => {
            put_u8(bytes, range.minimum);
            put_u8(bytes, range.maximum);
        }
        _ => {}
    }
}

fn decode_function_attribute(
    reader: &mut Reader<'_>,
) -> Result<FunctionAttributeV1, DecodeHandoffErrorV1> {
    Ok(match reader.u8()? {
        1 => FunctionAttributeV1::NoUnwind,
        2 => FunctionAttributeV1::FlatWorkgroupSize(
            WorkgroupSizeRangeV1::new(reader.u16()?, reader.u16()?)
                .map_err(DecodeHandoffErrorV1::InvalidModel)?,
        ),
        3 => FunctionAttributeV1::WavesPerEu(
            WavesPerEuV1::new(reader.u8()?, reader.u8()?)
                .map_err(DecodeHandoffErrorV1::InvalidModel)?,
        ),
        4 => FunctionAttributeV1::DenormalFpMathF32Ieee,
        5 => FunctionAttributeV1::UnsafeFpMathDisabled,
        6 => FunctionAttributeV1::NoInfsFpMathDisabled,
        7 => FunctionAttributeV1::NoNansFpMathDisabled,
        8 => FunctionAttributeV1::NoSignedZerosFpMathDisabled,
        9 => FunctionAttributeV1::ApproxFuncFpMathDisabled,
        10 => FunctionAttributeV1::FpContractOff,
        11 => FunctionAttributeV1::NoCompletionAction,
        12 => FunctionAttributeV1::NoDefaultQueue,
        13 => FunctionAttributeV1::NoHeapPointer,
        14 => FunctionAttributeV1::NoHostcallPointer,
        15 => FunctionAttributeV1::NoMultigridSyncArgument,
        16 => FunctionAttributeV1::NoQueuePointer,
        tag => {
            return Err(DecodeHandoffErrorV1::UnknownTag {
                section: WireSectionV1::FunctionAttribute,
                tag,
            });
        }
    })
}

fn expect_tag(
    reader: &mut Reader<'_>,
    section: WireSectionV1,
    expected: u8,
) -> Result<(), DecodeHandoffErrorV1> {
    let tag = reader.u8()?;
    if tag != expected {
        return Err(DecodeHandoffErrorV1::UnknownTag { section, tag });
    }
    Ok(())
}

fn decode_target_feature(tag: u8) -> Result<TargetFeatureV1, DecodeHandoffErrorV1> {
    match tag {
        1 => Ok(TargetFeatureV1::WavefrontSize32),
        2 => Ok(TargetFeatureV1::WavefrontSize64),
        3 => Ok(TargetFeatureV1::Xnack),
        tag => Err(DecodeHandoffErrorV1::UnknownTag {
            section: WireSectionV1::TargetFeature,
            tag,
        }),
    }
}

fn decode_code_object(tag: u8) -> Result<CodeObjectVersionV1, DecodeHandoffErrorV1> {
    match tag {
        6 => Ok(CodeObjectVersionV1::V6),
        tag => Err(DecodeHandoffErrorV1::UnknownTag {
            section: WireSectionV1::CodeObjectPolicy,
            tag,
        }),
    }
}

fn decode_optimization(tag: u8) -> Result<OptimizationLevelV1, DecodeHandoffErrorV1> {
    match tag {
        2 => Ok(OptimizationLevelV1::O2),
        tag => Err(DecodeHandoffErrorV1::UnknownTag {
            section: WireSectionV1::OptimizationLevel,
            tag,
        }),
    }
}

fn decode_relocation(tag: u8) -> Result<RelocationModelV1, DecodeHandoffErrorV1> {
    match tag {
        1 => Ok(RelocationModelV1::Pic),
        tag => Err(DecodeHandoffErrorV1::UnknownTag {
            section: WireSectionV1::RelocationModel,
            tag,
        }),
    }
}

fn decode_code_model(tag: u8) -> Result<CodeModelV1, DecodeHandoffErrorV1> {
    match tag {
        1 => Ok(CodeModelV1::Small),
        tag => Err(DecodeHandoffErrorV1::UnknownTag {
            section: WireSectionV1::CodeModel,
            tag,
        }),
    }
}

fn decode_scalar_type(tag: u8) -> Result<ScalarTypeV1, DecodeHandoffErrorV1> {
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
        tag => Err(DecodeHandoffErrorV1::UnknownTag {
            section: WireSectionV1::ScalarType,
            tag,
        }),
    }
}

fn decode_address_space(tag: u8) -> Result<AddressSpaceV1, DecodeHandoffErrorV1> {
    match tag {
        0 => Ok(AddressSpaceV1::Flat),
        1 => Ok(AddressSpaceV1::Global),
        2 => Ok(AddressSpaceV1::Region),
        3 => Ok(AddressSpaceV1::Local),
        4 => Ok(AddressSpaceV1::Constant),
        5 => Ok(AddressSpaceV1::Private),
        tag => Err(DecodeHandoffErrorV1::UnknownTag {
            section: WireSectionV1::AddressSpace,
            tag,
        }),
    }
}

fn decode_module_flag(tag: u8) -> Result<ModuleFlagV1, DecodeHandoffErrorV1> {
    match tag {
        1 => Ok(ModuleFlagV1::CodeObjectVersion6),
        2 => Ok(ModuleFlagV1::PicLevel2),
        3 => Ok(ModuleFlagV1::WcharSize4),
        tag => Err(DecodeHandoffErrorV1::UnknownTag {
            section: WireSectionV1::ModuleFlag,
            tag,
        }),
    }
}

fn decode_device_library(tag: u8) -> Result<DeviceLibraryKindV1, DecodeHandoffErrorV1> {
    match tag {
        1 => Ok(DeviceLibraryKindV1::Ocml),
        2 => Ok(DeviceLibraryKindV1::Ockl),
        3 => Ok(DeviceLibraryKindV1::OpenCl),
        4 => Ok(DeviceLibraryKindV1::OclcIsaVersion942),
        5 => Ok(DeviceLibraryKindV1::OclcWavefrontSize64On),
        6 => Ok(DeviceLibraryKindV1::OclcFiniteOnlyOff),
        7 => Ok(DeviceLibraryKindV1::OclcUnsafeMathOff),
        8 => Ok(DeviceLibraryKindV1::OclcCorrectlyRoundedSqrtOn),
        9 => Ok(DeviceLibraryKindV1::OclcDazOff),
        tag => Err(DecodeHandoffErrorV1::UnknownTag {
            section: WireSectionV1::DeviceLibrary,
            tag,
        }),
    }
}

fn decode_origin_kind(tag: u8) -> Result<OriginKindV1, DecodeHandoffErrorV1> {
    match tag {
        1 => Ok(OriginKindV1::RustSource),
        2 => Ok(OriginKindV1::Mir),
        3 => Ok(OriginKindV1::KernelIr),
        4 => Ok(OriginKindV1::ScheduleIr),
        5 => Ok(OriginKindV1::AmdgcnIr),
        tag => Err(DecodeHandoffErrorV1::UnknownTag {
            section: WireSectionV1::Origin,
            tag,
        }),
    }
}

fn decode_obligation_kind(tag: u8) -> Result<ObligationKindV1, DecodeHandoffErrorV1> {
    match tag {
        1 => Ok(ObligationKindV1::PreserveKernelAbi),
        2 => Ok(ObligationKindV1::PreserveAddressSpaces),
        3 => Ok(ObligationKindV1::PreserveTargetFeatures),
        4 => Ok(ObligationKindV1::PreserveCallingConvention),
        5 => Ok(ObligationKindV1::PreserveFunctionAttributes),
        6 => Ok(ObligationKindV1::PreserveModuleMetadata),
        7 => Ok(ObligationKindV1::AuthenticateDeviceLibraries),
        8 => Ok(ObligationKindV1::MaintainOriginCoverage),
        tag => Err(DecodeHandoffErrorV1::UnknownTag {
            section: WireSectionV1::Obligation,
            tag,
        }),
    }
}

const fn target_feature_tag(feature: TargetFeatureV1) -> u8 {
    match feature {
        TargetFeatureV1::WavefrontSize32 => 1,
        TargetFeatureV1::WavefrontSize64 => 2,
        TargetFeatureV1::Xnack => 3,
    }
}

const fn code_object_tag(version: CodeObjectVersionV1) -> u8 {
    match version {
        CodeObjectVersionV1::V6 => 6,
    }
}

const fn optimization_tag(level: OptimizationLevelV1) -> u8 {
    match level {
        OptimizationLevelV1::O2 => 2,
    }
}

const fn relocation_tag(model: RelocationModelV1) -> u8 {
    match model {
        RelocationModelV1::Pic => 1,
    }
}

const fn code_model_tag(model: CodeModelV1) -> u8 {
    match model {
        CodeModelV1::Small => 1,
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

const fn module_flag_tag(flag: ModuleFlagV1) -> u8 {
    match flag {
        ModuleFlagV1::CodeObjectVersion6 => 1,
        ModuleFlagV1::PicLevel2 => 2,
        ModuleFlagV1::WcharSize4 => 3,
    }
}

const fn device_library_tag(kind: DeviceLibraryKindV1) -> u8 {
    match kind {
        DeviceLibraryKindV1::Ocml => 1,
        DeviceLibraryKindV1::Ockl => 2,
        DeviceLibraryKindV1::OpenCl => 3,
        DeviceLibraryKindV1::OclcIsaVersion942 => 4,
        DeviceLibraryKindV1::OclcWavefrontSize64On => 5,
        DeviceLibraryKindV1::OclcFiniteOnlyOff => 6,
        DeviceLibraryKindV1::OclcUnsafeMathOff => 7,
        DeviceLibraryKindV1::OclcCorrectlyRoundedSqrtOn => 8,
        DeviceLibraryKindV1::OclcDazOff => 9,
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

    fn take(&mut self, count: usize) -> Result<&'a [u8], DecodeHandoffErrorV1> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(DecodeHandoffErrorV1::Truncated {
                offset: self.offset,
            })?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(DecodeHandoffErrorV1::Truncated {
                offset: self.offset,
            })?;
        self.offset = end;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, DecodeHandoffErrorV1> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, DecodeHandoffErrorV1> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn u32(&mut self) -> Result<u32, DecodeHandoffErrorV1> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn u64(&mut self) -> Result<u64, DecodeHandoffErrorV1> {
        let bytes = self.take(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn array_32(&mut self) -> Result<[u8; 32], DecodeHandoffErrorV1> {
        let mut output = [0; 32];
        output.copy_from_slice(self.take(32)?);
        Ok(output)
    }

    fn bounded_u8(
        &mut self,
        limit: HandoffLimitV1,
        maximum: usize,
    ) -> Result<usize, DecodeHandoffErrorV1> {
        let observed = self.u8()? as usize;
        check_wire_limit(limit, observed, maximum)?;
        Ok(observed)
    }

    fn bounded_u16(
        &mut self,
        limit: HandoffLimitV1,
        maximum: usize,
    ) -> Result<usize, DecodeHandoffErrorV1> {
        let observed = self.u16()? as usize;
        check_wire_limit(limit, observed, maximum)?;
        Ok(observed)
    }

    fn string(
        &mut self,
        maximum: usize,
        limit: HandoffLimitV1,
        section: WireSectionV1,
    ) -> Result<String, DecodeHandoffErrorV1> {
        let observed = self.u16()? as usize;
        check_wire_limit(limit, observed, maximum)?;
        let bytes = self.take(observed)?;
        let text = str::from_utf8(bytes).map_err(|_| DecodeHandoffErrorV1::InvalidUtf8(section))?;
        Ok(String::from(text))
    }

    fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

fn check_wire_limit(
    limit: HandoffLimitV1,
    observed: usize,
    maximum: usize,
) -> Result<(), DecodeHandoffErrorV1> {
    if observed > maximum {
        return Err(DecodeHandoffErrorV1::LimitExceeded {
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
        u16::try_from(value.len()).expect("bounded handoff string fits u16"),
    );
    bytes.extend_from_slice(value.as_bytes());
}

const _: () = assert!(HEADER_BYTES_V1 == MAGIC_V1.len() + 2 + 2 + 4);

#[cfg(test)]
mod tests {
    use super::*;

    const ABI_ATTRIBUTES: [FunctionAttributeV1; 6] = [
        FunctionAttributeV1::NoCompletionAction,
        FunctionAttributeV1::NoDefaultQueue,
        FunctionAttributeV1::NoHeapPointer,
        FunctionAttributeV1::NoHostcallPointer,
        FunctionAttributeV1::NoMultigridSyncArgument,
        FunctionAttributeV1::NoQueuePointer,
    ];

    #[test]
    fn abi_function_attribute_tags_are_stable_and_closed() {
        for (index, attribute) in ABI_ATTRIBUTES.into_iter().enumerate() {
            let tag = 11 + u8::try_from(index).unwrap();
            let mut bytes = Vec::new();
            encode_function_attribute(&mut bytes, attribute);
            assert_eq!(bytes, [tag]);
            assert_eq!(
                decode_function_attribute(&mut Reader::new(&bytes)).unwrap(),
                attribute
            );
        }

        assert_eq!(
            decode_function_attribute(&mut Reader::new(&[17])),
            Err(DecodeHandoffErrorV1::UnknownTag {
                section: WireSectionV1::FunctionAttribute,
                tag: 17,
            })
        );
    }
}
