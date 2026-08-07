//! Rustc-derived descriptor input for the first typed Worker V2 profile.

use crate::collector::{CollectedFunction, TypedArgumentListV1, TypedKernelProfile};
use crate::kernel_ir_codegen::InertCompilerModuleTextV1;
use crate::rust_type_layout::{ExtractError, extract_exact_typed_vecadd_layout};
use crate::rust_type_layout_v3::{
    GeneralTypedArgumentKindV3, GeneralTypedExtractError, extract_general_typed_kernel_v3,
};
use fe2o3_artifacts::{
    RustLayoutEvidenceV1, TypeIdentity, derive_generated_host_contract_identity_v1,
};
use fe2o3_compiler_ffi::{
    CompilerDescriptorSourceErrorV1, CompilerDescriptorSourceV1, CompilerFfiEnvelopeV1,
};
use fe2o3_kernel_descriptor::{
    AccessMode, BlockSizeV1, BuildEvidenceV1, CanonicalCodeObjectDigest, CapabilityV1,
    CodeObjectVersion, CompilerIdentityV1, DeviceDescriptorTableV1, DeviceLayoutDescriptorV1,
    DeviceLayoutRecordV1, DimensionsV1, EvidenceDigest, EvidenceIdentity, KernelAbiLayoutV1,
    KernelDescriptorV1, KernelId, LaunchConstraintsV1, LogicalArgumentV1, ProducerIdentityV1,
    ScalarTypeV1, SourceTypeDescriptorV1, SourceTypeRecordV1, Text, ValidName, ValidationError,
};
use fe2o3_kernel_ir::{Module, TargetCapability, WaveWidth, WorkgroupSize};
use reserved_fe2o3_symbols::{KernelBindingIdV1, MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1};
use rustc_middle::ty::TyCtxt;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

const GFX942_PROCESSOR: &str = "gfx942";
const EXPLICIT_ARGUMENT_BYTES: u32 = 48;
const KERNARG_ALIGNMENT_BYTES: u32 = 8;
const WORKGROUP_X: u32 = 256;

const SOURCE_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/RUSTC-DESCRIPTOR-SOURCE-IDENTITY/V1\0";
const SOURCE_DIGEST_DOMAIN_V1: &[u8] = b"FE2O3/RUSTC-DESCRIPTOR-SOURCE-DIGEST/V1\0";
const IR_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/RUSTC-DESCRIPTOR-IR-IDENTITY/V1\0";
const IR_DIGEST_DOMAIN_V1: &[u8] = b"FE2O3/RUSTC-DESCRIPTOR-IR-DIGEST/V1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypedDescriptorRootV1 {
    logical_name: String,
    export_name: String,
    profile: TypedKernelProfile,
    kernel_binding: KernelBindingIdV1,
    arguments: TypedArgumentListV1<TypedDescriptorArgumentV1>,
    explicit_argument_bytes: u32,
    kernarg_alignment_bytes: u32,
}

impl TypedDescriptorRootV1 {
    pub(crate) fn general_v3_semantic_identity(
        &self,
    ) -> Option<(
        KernelBindingIdV1,
        reserved_fe2o3_symbols::GeneratedHostContractIdV3,
    )> {
        match self.profile {
            TypedKernelProfile::GeneralScalarSliceRustcLayoutV3 {
                generated_host_contract_identity,
            } => Some((self.kernel_binding, generated_host_contract_identity)),
            TypedKernelProfile::VecAddRustcLayoutV2 => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum DescriptorArgumentKindV1 {
    SharedSlice(ScalarTypeV1),
    DisjointSlice(ScalarTypeV1),
    Scalar(ScalarTypeV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TypedDescriptorArgumentV1 {
    name: String,
    kind: DescriptorArgumentKindV1,
    access: AccessMode,
    offset: u32,
    layout: RustLayoutEvidenceV1,
}

/// Re-extracts exact rustc layout evidence instead of trusting retained identity bytes alone.
pub(crate) fn typed_descriptor_roots_from_collection<'tcx>(
    tcx: TyCtxt<'tcx>,
    functions: &[CollectedFunction<'tcx>],
) -> Result<Vec<TypedDescriptorRootV1>, CompilerDescriptorError> {
    functions
        .iter()
        .filter_map(|function| {
            function.typed_profile.map(|profile| {
                if !function.is_kernel_entry() {
                    return Err(CompilerDescriptorError::TypedProfileOnNonKernel(
                        function.export_name.clone(),
                    ));
                }
                let logical_name = function.logical_name.clone().ok_or_else(|| {
                    CompilerDescriptorError::MissingTypedField {
                        kernel: function.export_name.clone(),
                        field: "logical name",
                    }
                })?;
                let kernel_binding = function.kernel_binding.ok_or_else(|| {
                    CompilerDescriptorError::MissingTypedField {
                        kernel: function.export_name.clone(),
                        field: "kernel binding",
                    }
                })?;
                let retained = function.typed_layout_identities.as_ref().ok_or_else(|| {
                    CompilerDescriptorError::MissingTypedField {
                        kernel: function.export_name.clone(),
                        field: "rustc layout identities",
                    }
                })?;
                validate_profile_argument_count(profile, &function.export_name, retained.len())?;
                let (arguments, explicit_argument_bytes, kernarg_alignment_bytes) = match profile {
                    TypedKernelProfile::VecAddRustcLayoutV2 => {
                        let [input_a, input_b, output] =
                            extract_exact_typed_vecadd_layout(tcx, function.instance)
                                .map_err(CompilerDescriptorError::RustLayout)?;
                        (
                            TypedArgumentListV1::new(vec![
                                TypedDescriptorArgumentV1 {
                                    name: "input_a".to_owned(),
                                    kind: DescriptorArgumentKindV1::SharedSlice(ScalarTypeV1::F32),
                                    access: AccessMode::ReadOnly,
                                    offset: 0,
                                    layout: input_a,
                                },
                                TypedDescriptorArgumentV1 {
                                    name: "input_b".to_owned(),
                                    kind: DescriptorArgumentKindV1::SharedSlice(ScalarTypeV1::F32),
                                    access: AccessMode::ReadOnly,
                                    offset: 16,
                                    layout: input_b,
                                },
                                TypedDescriptorArgumentV1 {
                                    name: "output".to_owned(),
                                    kind: DescriptorArgumentKindV1::DisjointSlice(
                                        ScalarTypeV1::F32,
                                    ),
                                    access: AccessMode::WriteOnly,
                                    offset: 32,
                                    layout: output,
                                },
                            ]),
                            EXPLICIT_ARGUMENT_BYTES,
                            KERNARG_ALIGNMENT_BYTES,
                        )
                    }
                    TypedKernelProfile::GeneralScalarSliceRustcLayoutV3 {
                        generated_host_contract_identity,
                    } => {
                        let contract = extract_general_typed_kernel_v3(tcx, function.instance)
                            .map_err(CompilerDescriptorError::GeneralRustLayout)?;
                        let retained_contract = function
                            .general_typed_contract
                            .as_ref()
                            .ok_or_else(|| CompilerDescriptorError::MissingTypedField {
                                kernel: function.export_name.clone(),
                                field: "general rustc contract",
                            })?;
                        if retained_contract != &contract {
                            return Err(CompilerDescriptorError::RetainedGeneralContractMismatch(
                                function.export_name.clone(),
                            ));
                        }
                        let derived = derive_generated_host_contract_identity_v1(
                            MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
                            kernel_binding.as_bytes(),
                            &logical_name,
                            &function.export_name,
                            contract.abi(),
                            contract.launch(),
                        );
                        if derived.as_bytes() != &generated_host_contract_identity.as_bytes() {
                            return Err(CompilerDescriptorError::GeneratedHostContractMismatch(
                                function.export_name.clone(),
                            ));
                        }
                        let fields = contract.abi().fields();
                        let arguments = contract
                            .arguments()
                            .iter()
                            .zip(fields)
                            .enumerate()
                            .map(|(index, (argument, field))| {
                                Ok(TypedDescriptorArgumentV1 {
                                    name: field.name().as_str().to_owned(),
                                    kind: descriptor_argument_kind(argument.kind()),
                                    access: match argument.kind() {
                                        GeneralTypedArgumentKindV3::Scalar(_) => {
                                            AccessMode::ByValue
                                        }
                                        GeneralTypedArgumentKindV3::SharedSlice(_) => {
                                            AccessMode::ReadOnly
                                        }
                                        GeneralTypedArgumentKindV3::DisjointSlice(_) => {
                                            AccessMode::ReadWrite
                                        }
                                    },
                                    offset: u32::try_from(field.offset()).map_err(|_| {
                                        CompilerDescriptorError::ArgumentOffsetOverflow {
                                            kernel: function.export_name.clone(),
                                            index,
                                        }
                                    })?,
                                    layout: argument.layout().clone(),
                                })
                            })
                            .collect::<Result<Vec<_>, CompilerDescriptorError>>()?;
                        (
                            TypedArgumentListV1::new(arguments),
                            u32::try_from(contract.abi().size()).map_err(|_| {
                                CompilerDescriptorError::ExplicitArgumentSizeOverflow(
                                    function.export_name.clone(),
                                )
                            })?,
                            contract.abi().alignment(),
                        )
                    }
                };
                let arguments = arguments.map_err(|error| {
                    CompilerDescriptorError::InvalidArgumentCollection {
                        kernel: function.export_name.clone(),
                        reason: error.to_string(),
                    }
                })?;
                if retained.len() != arguments.len() {
                    return Err(
                        CompilerDescriptorError::RetainedLayoutArgumentCountMismatch {
                            kernel: function.export_name.clone(),
                            retained: retained.len(),
                            rederived: arguments.len(),
                        },
                    );
                }
                if !retained.as_slice().iter().copied().eq(arguments
                    .as_slice()
                    .iter()
                    .map(|argument| argument.layout.type_identity()))
                {
                    return Err(CompilerDescriptorError::RetainedLayoutIdentityMismatch(
                        function.export_name.clone(),
                    ));
                }
                Ok(TypedDescriptorRootV1 {
                    logical_name,
                    export_name: function.export_name.clone(),
                    profile,
                    kernel_binding,
                    arguments,
                    explicit_argument_bytes,
                    kernarg_alignment_bytes,
                })
            })
        })
        .collect()
}

fn descriptor_argument_kind(kind: GeneralTypedArgumentKindV3) -> DescriptorArgumentKindV1 {
    let scalar = descriptor_scalar(kind.scalar());
    match kind {
        GeneralTypedArgumentKindV3::Scalar(_) => DescriptorArgumentKindV1::Scalar(scalar),
        GeneralTypedArgumentKindV3::SharedSlice(_) => DescriptorArgumentKindV1::SharedSlice(scalar),
        GeneralTypedArgumentKindV3::DisjointSlice(_) => {
            DescriptorArgumentKindV1::DisjointSlice(scalar)
        }
    }
}

fn descriptor_scalar(value: fe2o3_artifacts::RustScalarElementTypeV1) -> ScalarTypeV1 {
    use fe2o3_artifacts::RustScalarElementTypeV1 as RustScalar;
    match value {
        RustScalar::I8 => ScalarTypeV1::I8,
        RustScalar::U8 => ScalarTypeV1::U8,
        RustScalar::I16 => ScalarTypeV1::I16,
        RustScalar::U16 => ScalarTypeV1::U16,
        RustScalar::I32 => ScalarTypeV1::I32,
        RustScalar::U32 => ScalarTypeV1::U32,
        RustScalar::I64 => ScalarTypeV1::I64,
        RustScalar::U64 => ScalarTypeV1::U64,
        RustScalar::F32 => ScalarTypeV1::F32,
        RustScalar::F64 => ScalarTypeV1::F64,
        RustScalar::F16 => unreachable!("general typed V3 rejects f16"),
        _ => unreachable!("unknown scalar schema is not admitted by general typed V3"),
    }
}

/// Constructs a zero-digest descriptor source for a complete typed gfx942/COV6 module.
///
/// An all-raw module returns `None`. A mixed typed/raw module is rejected because publishing an
/// incomplete descriptor table would create a misleading kernel closure.
pub(crate) fn construct_compiler_descriptor_source_v1(
    envelope: &CompilerFfiEnvelopeV1,
    module: &Module,
    compiler_module: &InertCompilerModuleTextV1,
    typed_roots: &[TypedDescriptorRootV1],
) -> Result<Option<CompilerDescriptorSourceV1>, CompilerDescriptorError> {
    if typed_roots.is_empty() {
        return Ok(None);
    }
    if typed_roots.len() != module.kernels.len() {
        return Err(CompilerDescriptorError::IncompleteTypedKernelClosure {
            typed: typed_roots.len(),
            total: module.kernels.len(),
        });
    }
    if envelope.target().as_amd_target_id().processor() != GFX942_PROCESSOR {
        return Err(CompilerDescriptorError::UnsupportedTarget(
            envelope.target().to_string(),
        ));
    }
    if envelope.code_object_version() != CodeObjectVersion::V6 {
        return Err(CompilerDescriptorError::UnsupportedCodeObjectVersion(
            envelope.code_object_version(),
        ));
    }

    let descriptor_kinds = typed_roots
        .iter()
        .flat_map(|root| {
            root.arguments
                .as_slice()
                .iter()
                .map(|argument| argument.kind)
        })
        .collect::<BTreeSet<_>>();
    let mut source_types = Vec::with_capacity(descriptor_kinds.len());
    let mut device_layouts = Vec::with_capacity(descriptor_kinds.len());
    let mut descriptor_indexes = BTreeMap::new();
    for kind in descriptor_kinds {
        let index = source_types.len();
        descriptor_indexes.insert(kind, index);
        let (source, layout) = descriptor_records(kind);
        source_types.push(source);
        device_layouts.push(layout);
    }

    let module_capabilities = descriptor_capabilities(module)?;
    let mut seen_exports = BTreeSet::new();
    let mut kernels = Vec::with_capacity(typed_roots.len());
    for root in typed_roots {
        validate_profile_argument_count(root.profile, &root.export_name, root.arguments.len())?;
        if !seen_exports.insert(root.export_name.as_str()) {
            return Err(CompilerDescriptorError::DuplicateTypedKernel(
                root.export_name.clone(),
            ));
        }
        let kernel = module
            .kernels
            .iter()
            .find(|kernel| kernel.id.as_str() == root.export_name)
            .ok_or_else(|| CompilerDescriptorError::MissingTypedKernel(root.export_name.clone()))?;
        if kernel.workgroup_size != Some(WorkgroupSize::new(WORKGROUP_X, 1, 1)) {
            return Err(CompilerDescriptorError::UnexpectedWorkgroupSize(
                root.export_name.clone(),
            ));
        }

        let arguments = root
            .arguments
            .as_slice()
            .iter()
            .enumerate()
            .map(|(index, argument)| {
                let descriptor_index = descriptor_indexes[&argument.kind];
                let source_type = &source_types[descriptor_index];
                let device_layout = &device_layouts[descriptor_index];
                let source_index = u16::try_from(index).map_err(|_| {
                    CompilerDescriptorError::ArgumentIndexOverflow {
                        kernel: root.export_name.clone(),
                        index,
                    }
                })?;
                let name = ValidName::new(argument.name.clone())?;
                match argument.kind {
                    DescriptorArgumentKindV1::Scalar(_) => LogicalArgumentV1::scalar(
                        source_index,
                        name,
                        source_type,
                        device_layout,
                        argument.offset,
                    ),
                    DescriptorArgumentKindV1::SharedSlice(_) => LogicalArgumentV1::shared_slice(
                        source_index,
                        name,
                        source_type,
                        device_layout,
                        argument.offset,
                    ),
                    DescriptorArgumentKindV1::DisjointSlice(_) => {
                        LogicalArgumentV1::disjoint_slice(
                            source_index,
                            name,
                            source_type,
                            device_layout,
                            argument.access,
                            argument.offset,
                        )
                    }
                }
                .map_err(CompilerDescriptorError::Validation)
            })
            .collect::<Result<Vec<_>, CompilerDescriptorError>>()?;
        let source_evidence = source_evidence(root);
        let ir_evidence = ir_evidence(envelope, compiler_module, root);
        let kernarg_segment_bytes =
            root.explicit_argument_bytes
                .checked_add(256)
                .ok_or_else(|| {
                    CompilerDescriptorError::KernargSizeOverflow(root.export_name.clone())
                })?;
        kernels.push(KernelDescriptorV1::new(
            KernelId::from_bytes(root.kernel_binding.as_bytes()),
            ValidName::new(root.logical_name.clone())?,
            ValidName::new(root.export_name.clone())?,
            ValidName::new(format!("{}.kd", root.export_name))?,
            source_evidence,
            ir_evidence,
            module_capabilities.clone(),
            KernelAbiLayoutV1::new(
                root.explicit_argument_bytes,
                kernarg_segment_bytes,
                root.kernarg_alignment_bytes,
            )?,
            LaunchConstraintsV1::new(
                1,
                BlockSizeV1::Exact(DimensionsV1::new(WORKGROUP_X, 1, 1)?),
                DimensionsV1::new(u32::MAX, 1, 1)?,
                WORKGROUP_X,
                0,
                0,
            )?,
            arguments,
        )?);
    }

    let producer_version = if typed_roots
        .iter()
        .all(|root| root.profile == TypedKernelProfile::VecAddRustcLayoutV2)
    {
        "typed-vecadd-gfx942-cov6-v1"
    } else {
        "typed-general-gfx942-cov6-v1"
    };
    let table = DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes([0; 32]),
        CodeObjectVersion::V6,
        CompilerIdentityV1::new(
            Text::new("rustc-codegen-fe2o3")?,
            Text::new(env!("CARGO_PKG_VERSION"))?,
            [0; 20],
        ),
        ProducerIdentityV1::new(
            Text::new("rustc-codegen-fe2o3-worker-v2")?,
            Text::new(producer_version)?,
        ),
        envelope.target(),
        source_types,
        device_layouts,
        kernels,
    )?;
    CompilerDescriptorSourceV1::new(table)
        .map(Some)
        .map_err(CompilerDescriptorError::Source)
}

fn descriptor_records(
    kind: DescriptorArgumentKindV1,
) -> (SourceTypeRecordV1, DeviceLayoutRecordV1) {
    match kind {
        DescriptorArgumentKindV1::Scalar(scalar) => (
            SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(scalar)),
            DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::scalar(scalar)),
        ),
        DescriptorArgumentKindV1::SharedSlice(scalar) => (
            SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(scalar)),
            DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(scalar)),
        ),
        DescriptorArgumentKindV1::DisjointSlice(scalar) => (
            SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(scalar)),
            DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(scalar)),
        ),
    }
}

fn descriptor_capabilities(module: &Module) -> Result<Vec<CapabilityV1>, CompilerDescriptorError> {
    let mut result = BTreeSet::new();
    let mut effective = module.effective_capabilities();
    effective.extend(
        module
            .kernels
            .iter()
            .flat_map(|kernel| kernel.required_capabilities.iter().cloned()),
    );
    effective.extend(
        module
            .functions
            .iter()
            .flat_map(|function| function.effective_capabilities()),
    );
    for capability in effective {
        match capability {
            TargetCapability::Int64 => {}
            TargetCapability::Subgroups | TargetCapability::SubgroupSize(64) => {
                result.insert(CapabilityV1::Subgroup);
                result.insert(CapabilityV1::AmdWave);
            }
            TargetCapability::WaveWidth(WaveWidth::Wave64) => {
                result.insert(CapabilityV1::AmdWave);
            }
            unsupported => {
                return Err(CompilerDescriptorError::UnsupportedCapability(format!(
                    "{unsupported:?}"
                )));
            }
        }
    }
    Ok(result.into_iter().collect())
}

fn source_evidence(root: &TypedDescriptorRootV1) -> BuildEvidenceV1 {
    let binding = root.kernel_binding.as_bytes();
    let mut identity_frames = vec![
        binding.as_slice(),
        root.logical_name.as_bytes(),
        root.export_name.as_bytes(),
    ];
    let identity_bytes = root
        .arguments
        .as_slice()
        .iter()
        .map(|argument| type_identity_bytes(argument.layout.type_identity()))
        .collect::<Vec<_>>();
    for bytes in &identity_bytes {
        identity_frames.push(bytes.as_slice());
    }
    let canonical_layouts = root
        .arguments
        .as_slice()
        .iter()
        .map(|argument| argument.layout.canonical_bytes())
        .collect::<Vec<_>>();
    let digest_frames = canonical_layouts
        .iter()
        .map(Vec::as_slice)
        .collect::<Vec<_>>();
    BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes(domain_hash(
            SOURCE_IDENTITY_DOMAIN_V1,
            &identity_frames,
        )),
        EvidenceDigest::from_sha256_bytes(domain_hash(SOURCE_DIGEST_DOMAIN_V1, &digest_frames)),
    )
}

fn validate_profile_argument_count(
    profile: TypedKernelProfile,
    kernel: &str,
    actual: usize,
) -> Result<(), CompilerDescriptorError> {
    if let Some(expected) = profile.expected_argument_count()
        && actual != expected
    {
        return Err(CompilerDescriptorError::TypedProfileArgumentCountMismatch {
            kernel: kernel.to_owned(),
            expected,
            actual,
        });
    }
    if !profile.accepts_argument_count(actual) {
        return Err(CompilerDescriptorError::InvalidArgumentCollection {
            kernel: kernel.to_owned(),
            reason: format!("unsupported general typed argument count {actual}"),
        });
    }
    Ok(())
}

fn ir_evidence(
    envelope: &CompilerFfiEnvelopeV1,
    module: &InertCompilerModuleTextV1,
    root: &TypedDescriptorRootV1,
) -> BuildEvidenceV1 {
    let binding = root.kernel_binding.as_bytes();
    let envelope_identity = envelope.identity().as_bytes();
    let target = envelope.target().to_string();
    let identity = domain_hash(
        IR_IDENTITY_DOMAIN_V1,
        &[
            binding.as_slice(),
            envelope_identity.as_slice(),
            target.as_bytes(),
            root.export_name.as_bytes(),
        ],
    );
    let digest = domain_hash(
        IR_DIGEST_DOMAIN_V1,
        &[
            envelope.canonical_bytes(),
            module.llvm_ir().as_bytes(),
            root.export_name.as_bytes(),
        ],
    );
    BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes(identity),
        EvidenceDigest::from_sha256_bytes(digest),
    )
}

fn type_identity_bytes(identity: TypeIdentity) -> [u8; 64] {
    let mut bytes = [0_u8; 64];
    bytes[..32].copy_from_slice(identity.rust_type().bytes().as_bytes());
    bytes[32..].copy_from_slice(identity.layout().bytes().as_bytes());
    bytes
}

fn domain_hash(domain: &[u8], frames: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((frames.len() as u64).to_le_bytes());
    for frame in frames {
        hasher.update((frame.len() as u64).to_le_bytes());
        hasher.update(frame);
    }
    hasher.finalize().into()
}

#[derive(Debug)]
pub(crate) enum CompilerDescriptorError {
    TypedProfileOnNonKernel(String),
    MissingTypedField {
        kernel: String,
        field: &'static str,
    },
    InvalidArgumentCollection {
        kernel: String,
        reason: String,
    },
    TypedProfileArgumentCountMismatch {
        kernel: String,
        expected: usize,
        actual: usize,
    },
    RetainedLayoutArgumentCountMismatch {
        kernel: String,
        retained: usize,
        rederived: usize,
    },
    RustLayout(ExtractError),
    GeneralRustLayout(GeneralTypedExtractError),
    RetainedLayoutIdentityMismatch(String),
    RetainedGeneralContractMismatch(String),
    GeneratedHostContractMismatch(String),
    ArgumentOffsetOverflow {
        kernel: String,
        index: usize,
    },
    ArgumentIndexOverflow {
        kernel: String,
        index: usize,
    },
    ExplicitArgumentSizeOverflow(String),
    KernargSizeOverflow(String),
    IncompleteTypedKernelClosure {
        typed: usize,
        total: usize,
    },
    UnsupportedTarget(String),
    UnsupportedCodeObjectVersion(CodeObjectVersion),
    DuplicateTypedKernel(String),
    MissingTypedKernel(String),
    UnexpectedWorkgroupSize(String),
    UnsupportedCapability(String),
    Validation(ValidationError),
    Source(CompilerDescriptorSourceErrorV1),
}

impl From<ValidationError> for CompilerDescriptorError {
    fn from(value: ValidationError) -> Self {
        Self::Validation(value)
    }
}

impl fmt::Display for CompilerDescriptorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TypedProfileOnNonKernel(kernel) => {
                write!(
                    formatter,
                    "typed descriptor profile is attached to non-kernel `{kernel}`"
                )
            }
            Self::MissingTypedField { kernel, field } => {
                write!(formatter, "typed kernel `{kernel}` has no {field}")
            }
            Self::InvalidArgumentCollection { kernel, reason } => {
                write!(
                    formatter,
                    "typed kernel `{kernel}` has invalid arguments: {reason}"
                )
            }
            Self::TypedProfileArgumentCountMismatch {
                kernel,
                expected,
                actual,
            } => write!(
                formatter,
                "typed kernel `{kernel}` profile requires {expected} argument(s), found {actual}"
            ),
            Self::RetainedLayoutArgumentCountMismatch {
                kernel,
                retained,
                rederived,
            } => write!(
                formatter,
                "typed kernel `{kernel}` retained {retained} layout identity/identities but rustc rederived {rederived}"
            ),
            Self::RustLayout(error) => write!(formatter, "rustc layout extraction failed: {error}"),
            Self::GeneralRustLayout(error) => {
                write!(formatter, "general rustc layout extraction failed: {error}")
            }
            Self::RetainedLayoutIdentityMismatch(kernel) => write!(
                formatter,
                "typed kernel `{kernel}` retained layout identities do not match fresh rustc evidence"
            ),
            Self::RetainedGeneralContractMismatch(kernel) => write!(
                formatter,
                "typed kernel `{kernel}` retained general contract does not match fresh rustc evidence"
            ),
            Self::GeneratedHostContractMismatch(kernel) => write!(
                formatter,
                "typed kernel `{kernel}` generated host-contract identity does not match fresh rustc evidence"
            ),
            Self::ArgumentOffsetOverflow { kernel, index } => write!(
                formatter,
                "typed kernel `{kernel}` argument {index} offset exceeds u32"
            ),
            Self::ArgumentIndexOverflow { kernel, index } => write!(
                formatter,
                "typed kernel `{kernel}` argument index {index} exceeds u16"
            ),
            Self::ExplicitArgumentSizeOverflow(kernel) => write!(
                formatter,
                "typed kernel `{kernel}` explicit argument size exceeds u32"
            ),
            Self::KernargSizeOverflow(kernel) => write!(
                formatter,
                "typed kernel `{kernel}` COV6 kernarg size overflows u32"
            ),
            Self::IncompleteTypedKernelClosure { typed, total } => write!(
                formatter,
                "typed descriptor closure has {typed} typed kernel(s) for {total} module kernel(s)"
            ),
            Self::UnsupportedTarget(target) => {
                write!(
                    formatter,
                    "typed descriptor source currently requires gfx942, found {target}"
                )
            }
            Self::UnsupportedCodeObjectVersion(version) => write!(
                formatter,
                "typed descriptor source currently requires code object V6, found {version:?}"
            ),
            Self::DuplicateTypedKernel(kernel) => {
                write!(formatter, "duplicate typed descriptor kernel `{kernel}`")
            }
            Self::MissingTypedKernel(kernel) => {
                write!(
                    formatter,
                    "typed descriptor kernel `{kernel}` is absent from kernel IR"
                )
            }
            Self::UnexpectedWorkgroupSize(kernel) => write!(
                formatter,
                "typed descriptor kernel `{kernel}` does not have the exact 256x1x1 workgroup"
            ),
            Self::UnsupportedCapability(capability) => write!(
                formatter,
                "typed vecadd descriptor cannot represent capability {capability}"
            ),
            Self::Validation(error) => write!(formatter, "invalid typed descriptor: {error}"),
            Self::Source(error) => write!(formatter, "invalid compiler descriptor source: {error}"),
        }
    }
}

impl std::error::Error for CompilerDescriptorError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel_ir_codegen::{
        bind_compiler_descriptor_source_v1, construct_inert_compiler_module_text_v1,
    };
    use fe2o3_artifacts::{
        PointerWidth, RustDisjointIndexSpaceV1, RustPhysicalComponentKindV1,
        RustPhysicalComponentV1, RustPointerMutabilityV1, RustScalarElementTypeV1,
        RustSourceTypeShapeV1, RustTypeEvidenceV1, RustcAbiClassV1,
    };
    use fe2o3_compiler_ffi::{
        CompilerFfiContractV1, CompilerFfiEnvelopeBuilderV1, CompilerFfiLinkRoleV1,
        CompilerFfiSourceOwnerV1, DeviceTargetV1,
    };
    use fe2o3_kernel_ir::{
        BasicBlock, BlockId, Function, Kernel, LaunchDomain, LaunchExtent, Signature, Terminator,
    };
    use reserved_fe2o3_symbols::{
        DeviceFfiContractFieldsV1, DeviceFfiDirectionV1, GeneratedHostContractIdV3,
        derive_device_ffi_contract_id_v1,
    };

    const TEST_ABI: &str = "C(mut_ptr<global,u32>[size=8,align=8,as=global])->unit[size=0,align=1]";

    fn envelope(version: CodeObjectVersion) -> CompilerFfiEnvelopeV1 {
        let target = DeviceTargetV1::parse("gfx942:xnack-").unwrap();
        let version_number = match version {
            CodeObjectVersion::V4 => 4,
            CodeObjectVersion::V5 => 5,
            CodeObjectVersion::V6 => 6,
        };
        let semantic_identity = [0x55; 32];
        let semantic_text = semantic_identity
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let id = derive_device_ffi_contract_id_v1(DeviceFfiContractFieldsV1 {
            direction: DeviceFfiDirectionV1::Import.tag(),
            symbol: "external_test",
            calling_convention: "C",
            code_object_version: version_number,
            target: "gfx942:xnack-",
            physical_abi: TEST_ABI,
            effects: "read_global",
            semantic_identity: &semantic_text,
        });
        let contract = CompilerFfiContractV1::new(
            id,
            DeviceFfiDirectionV1::Import,
            CompilerFfiLinkRoleV1::RequiresExternalDefinition,
            target,
            version,
            CompilerFfiSourceOwnerV1::new(
                "descriptor_test",
                "descriptor_test::external_test",
                [0x44; 16],
                "_RNvCs1234_descriptor_test13external_test",
            )
            .unwrap(),
            "external_test",
            TEST_ABI,
            "read_global",
            semantic_identity,
        )
        .unwrap();
        let mut builder = CompilerFfiEnvelopeBuilderV1::new(target, version, 1).unwrap();
        builder.push(contract).unwrap();
        builder.finish().unwrap()
    }

    fn layout(disjoint: bool) -> RustLayoutEvidenceV1 {
        let (shape, mutability) = if disjoint {
            (
                RustSourceTypeShapeV1::disjoint_slice(
                    RustScalarElementTypeV1::F32,
                    RustDisjointIndexSpaceV1::Index1D,
                ),
                RustPointerMutabilityV1::Mut,
            )
        } else {
            (
                RustSourceTypeShapeV1::shared_slice(RustScalarElementTypeV1::F32),
                RustPointerMutabilityV1::Const,
            )
        };
        RustLayoutEvidenceV1::new(
            RustTypeEvidenceV1::new(shape),
            RustcAbiClassV1::ScalarPair,
            PointerWidth::Bits64,
            16,
            8,
            vec![
                RustPhysicalComponentV1::new(
                    0,
                    8,
                    8,
                    RustPhysicalComponentKindV1::Pointer {
                        mutability,
                        pointee: RustScalarElementTypeV1::F32,
                    },
                )
                .unwrap(),
                RustPhysicalComponentV1::new(8, 8, 8, RustPhysicalComponentKindV1::Usize).unwrap(),
            ],
        )
        .unwrap()
    }

    fn root_with_layouts(binding: u8, layouts: Vec<RustLayoutEvidenceV1>) -> TypedDescriptorRootV1 {
        let arguments = layouts
            .into_iter()
            .enumerate()
            .map(|(index, layout)| {
                let disjoint = matches!(
                    layout.rust_type().source_type(),
                    RustSourceTypeShapeV1::DisjointSlice { .. }
                );
                TypedDescriptorArgumentV1 {
                    name: if index == 0 {
                        "input_a".to_owned()
                    } else if index == 1 {
                        "input_b".to_owned()
                    } else {
                        "output".to_owned()
                    },
                    kind: if disjoint {
                        DescriptorArgumentKindV1::DisjointSlice(ScalarTypeV1::F32)
                    } else {
                        DescriptorArgumentKindV1::SharedSlice(ScalarTypeV1::F32)
                    },
                    access: if disjoint {
                        AccessMode::WriteOnly
                    } else {
                        AccessMode::ReadOnly
                    },
                    offset: u32::try_from(index * 16).unwrap(),
                    layout,
                }
            })
            .collect();
        TypedDescriptorRootV1 {
            logical_name: "add".to_owned(),
            export_name: "vecadd".to_owned(),
            profile: TypedKernelProfile::VecAddRustcLayoutV2,
            kernel_binding: KernelBindingIdV1::from_bytes([binding; 32]),
            arguments: TypedArgumentListV1::new(arguments).unwrap(),
            explicit_argument_bytes: 48,
            kernarg_alignment_bytes: 8,
        }
    }

    fn root(binding: u8) -> TypedDescriptorRootV1 {
        root_with_layouts(binding, vec![layout(false), layout(false), layout(true)])
    }

    fn scalar_layout() -> RustLayoutEvidenceV1 {
        RustLayoutEvidenceV1::new(
            RustTypeEvidenceV1::new(RustSourceTypeShapeV1::scalar(RustScalarElementTypeV1::F32)),
            RustcAbiClassV1::Scalar,
            PointerWidth::Bits64,
            4,
            4,
            vec![
                RustPhysicalComponentV1::new(
                    0,
                    4,
                    4,
                    RustPhysicalComponentKindV1::Scalar {
                        scalar: RustScalarElementTypeV1::F32,
                    },
                )
                .unwrap(),
            ],
        )
        .unwrap()
    }

    fn descriptor_argument(
        index: usize,
        kind: DescriptorArgumentKindV1,
        offset: u32,
    ) -> TypedDescriptorArgumentV1 {
        let (layout, access) = match kind {
            DescriptorArgumentKindV1::Scalar(_) => (scalar_layout(), AccessMode::ByValue),
            DescriptorArgumentKindV1::SharedSlice(_) => (layout(false), AccessMode::ReadOnly),
            DescriptorArgumentKindV1::DisjointSlice(_) => (layout(true), AccessMode::ReadWrite),
        };
        TypedDescriptorArgumentV1 {
            name: format!("arg{index}"),
            kind,
            access,
            offset,
            layout,
        }
    }

    fn named_descriptor_argument(
        name: &str,
        index: usize,
        kind: DescriptorArgumentKindV1,
        offset: u32,
    ) -> TypedDescriptorArgumentV1 {
        let mut argument = descriptor_argument(index, kind, offset);
        argument.name = name.to_owned();
        argument
    }

    fn general_root(
        logical_name: &str,
        binding: u8,
        explicit_argument_bytes: u32,
        arguments: Vec<TypedDescriptorArgumentV1>,
    ) -> TypedDescriptorRootV1 {
        TypedDescriptorRootV1 {
            logical_name: logical_name.to_owned(),
            export_name: logical_name.to_owned(),
            profile: TypedKernelProfile::GeneralScalarSliceRustcLayoutV3 {
                generated_host_contract_identity: GeneratedHostContractIdV3::from_bytes(
                    [binding; 32],
                ),
            },
            kernel_binding: KernelBindingIdV1::from_bytes([binding; 32]),
            arguments: TypedArgumentListV1::new(arguments).unwrap(),
            explicit_argument_bytes,
            kernarg_alignment_bytes: 8,
        }
    }

    fn alpha_root() -> TypedDescriptorRootV1 {
        general_root(
            "alpha",
            0x61,
            40,
            vec![
                named_descriptor_argument(
                    "scale",
                    0,
                    DescriptorArgumentKindV1::Scalar(ScalarTypeV1::F32),
                    0,
                ),
                named_descriptor_argument(
                    "input",
                    1,
                    DescriptorArgumentKindV1::SharedSlice(ScalarTypeV1::F32),
                    8,
                ),
                named_descriptor_argument(
                    "output",
                    2,
                    DescriptorArgumentKindV1::DisjointSlice(ScalarTypeV1::F32),
                    24,
                ),
            ],
        )
    }

    fn zeta_root() -> TypedDescriptorRootV1 {
        general_root(
            "zeta",
            0x7a,
            56,
            vec![
                named_descriptor_argument(
                    "a",
                    0,
                    DescriptorArgumentKindV1::SharedSlice(ScalarTypeV1::F32),
                    0,
                ),
                named_descriptor_argument(
                    "b",
                    1,
                    DescriptorArgumentKindV1::SharedSlice(ScalarTypeV1::F32),
                    16,
                ),
                named_descriptor_argument(
                    "bias",
                    2,
                    DescriptorArgumentKindV1::Scalar(ScalarTypeV1::F32),
                    32,
                ),
                named_descriptor_argument(
                    "output",
                    3,
                    DescriptorArgumentKindV1::DisjointSlice(ScalarTypeV1::F32),
                    40,
                ),
            ],
        )
    }

    fn module() -> Module {
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let entry = Function::kernel_entry(
            "vecadd_impl",
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        );
        let mut kernel = Kernel::new(
            "vecadd",
            "vecadd_impl",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(256, 1, 1));
        let mut module = Module::new("typed_descriptor_test");
        module.functions.push(entry);
        module.kernels.push(kernel);
        module
            .required_capabilities
            .insert(TargetCapability::WaveWidth(WaveWidth::Wave64));
        module
    }

    fn module_for(exports: &[&str]) -> Module {
        let mut module = Module::new("typed_descriptor_test");
        for export in exports {
            let implementation = format!("{export}_impl");
            let mut block = BasicBlock::new(BlockId(0));
            block.terminator = Some(Terminator::Return { values: vec![] });
            module.functions.push(Function::kernel_entry(
                implementation.clone(),
                Signature::new(vec![], vec![]),
                vec![],
                vec![block],
            ));
            let mut kernel = Kernel::new(
                *export,
                implementation,
                LaunchDomain::D1 {
                    x: LaunchExtent::Dynamic,
                },
            );
            kernel.workgroup_size = Some(WorkgroupSize::new(256, 1, 1));
            module.kernels.push(kernel);
        }
        module
            .required_capabilities
            .insert(TargetCapability::WaveWidth(WaveWidth::Wave64));
        module
    }

    #[test]
    fn constructs_exact_gfx942_cov6_vecadd_descriptor() {
        let envelope = envelope(CodeObjectVersion::V6);
        let module = module();
        let llvm = construct_inert_compiler_module_text_v1(&module).unwrap();
        let source =
            construct_compiler_descriptor_source_v1(&envelope, &module, &llvm, &[root(0x42)])
                .unwrap()
                .unwrap();
        let table = source.table();
        assert_eq!(table.code_object_version(), CodeObjectVersion::V6);
        assert_eq!(table.device_target(), envelope.target());
        assert_eq!(table.canonical_code_object_digest().as_bytes(), &[0; 32]);
        assert_eq!(table.type_records().len(), 2);
        assert_eq!(table.layout_records().len(), 2);
        assert_eq!(table.kernels().len(), 1);
        let kernel = &table.kernels()[0];
        assert_eq!(kernel.kernel_id().as_bytes(), &[0x42; 32]);
        assert_eq!(kernel.logical_name().as_str(), "add");
        assert_eq!(kernel.entry_name().as_str(), "vecadd");
        assert_eq!(kernel.descriptor_symbol().as_str(), "vecadd.kd");
        assert_eq!(kernel.abi_layout().explicit_argument_size(), 48);
        assert_eq!(kernel.abi_layout().kernarg_segment_size(), 304);
        assert_eq!(kernel.abi_layout().kernarg_segment_alignment(), 8);
        assert_eq!(kernel.arguments().len(), 3);
        assert_eq!(kernel.arguments()[2].access(), AccessMode::WriteOnly);
        assert_eq!(kernel.capabilities(), &[CapabilityV1::AmdWave]);
        assert!(!source.authenticates_compiler_origin());
        assert!(!source.grants_link_authority());
        assert!(!source.grants_load_authority());
        assert!(!source.grants_launch_authority());

        let source_identity = source.identity();
        let bound = bind_compiler_descriptor_source_v1(llvm, &source).unwrap();
        assert_eq!(bound.descriptor_source_identity(), Some(source_identity));
        assert!(bound.llvm_ir().contains(".section .fe2o3.kd.v1"));
    }

    #[test]
    fn exact_vecadd_descriptor_bytes_match_the_v2_compatibility_golden() {
        let envelope = envelope(CodeObjectVersion::V6);
        let module = module();
        let llvm = construct_inert_compiler_module_text_v1(&module).unwrap();
        let source =
            construct_compiler_descriptor_source_v1(&envelope, &module, &llvm, &[root(0x42)])
                .unwrap()
                .unwrap();
        let digest: [u8; 32] = Sha256::digest(source.canonical_bytes()).into();
        let actual = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(
            actual,
            "92d88cdc6a13474ac5a988bb2f3afa196985c4454343a9886d80c068442445ba"
        );
    }

    #[test]
    fn constructs_two_differing_general_v3_descriptors_and_mixed_v2_v3() {
        let envelope = envelope(CodeObjectVersion::V6);
        let module = module_for(&["alpha", "zeta"]);
        let llvm = construct_inert_compiler_module_text_v1(&module).unwrap();
        let source = construct_compiler_descriptor_source_v1(
            &envelope,
            &module,
            &llvm,
            &[alpha_root(), zeta_root()],
        )
        .unwrap()
        .unwrap();
        let kernels = source.table().kernels();
        assert_eq!(kernels.len(), 2);
        assert_eq!(kernels[0].entry_name().as_str(), "alpha");
        assert_eq!(kernels[0].abi_layout().explicit_argument_size(), 40);
        assert_eq!(kernels[0].abi_layout().kernarg_segment_size(), 296);
        assert_eq!(kernels[0].arguments().len(), 3);
        assert_eq!(
            kernels[0]
                .arguments()
                .iter()
                .map(|argument| argument.name().as_str())
                .collect::<Vec<_>>(),
            ["scale", "input", "output"]
        );
        assert_eq!(
            kernels[0]
                .arguments()
                .iter()
                .map(|argument| {
                    argument
                        .physical_components()
                        .next()
                        .expect("argument has a physical component")
                        .1
                })
                .collect::<Vec<_>>(),
            [0, 8, 24]
        );
        assert_eq!(kernels[0].arguments()[0].access(), AccessMode::ByValue);
        assert_eq!(kernels[0].arguments()[2].access(), AccessMode::ReadWrite);
        assert_eq!(kernels[1].entry_name().as_str(), "zeta");
        assert_eq!(kernels[1].abi_layout().explicit_argument_size(), 56);
        assert_eq!(kernels[1].abi_layout().kernarg_segment_size(), 312);
        assert_eq!(kernels[1].arguments().len(), 4);
        assert_eq!(
            kernels[1]
                .arguments()
                .iter()
                .map(|argument| argument.name().as_str())
                .collect::<Vec<_>>(),
            ["a", "b", "bias", "output"]
        );
        assert_eq!(
            kernels[1]
                .arguments()
                .iter()
                .map(|argument| {
                    argument
                        .physical_components()
                        .next()
                        .expect("argument has a physical component")
                        .1
                })
                .collect::<Vec<_>>(),
            [0, 16, 32, 40]
        );
        assert_eq!(kernels[1].arguments()[2].access(), AccessMode::ByValue);
        assert_eq!(kernels[1].arguments()[3].access(), AccessMode::ReadWrite);

        let mixed_module = module_for(&["vecadd", "alpha"]);
        let mixed_llvm = construct_inert_compiler_module_text_v1(&mixed_module).unwrap();
        let mixed = construct_compiler_descriptor_source_v1(
            &envelope,
            &mixed_module,
            &mixed_llvm,
            &[root(0x42), alpha_root()],
        )
        .unwrap()
        .unwrap();
        assert_eq!(mixed.table().kernels().len(), 2);
        assert_eq!(
            mixed.table().kernels()[0].arguments()[2].access(),
            AccessMode::WriteOnly
        );
        assert_eq!(
            mixed.table().kernels()[1].arguments()[2].access(),
            AccessMode::ReadWrite
        );
    }

    #[test]
    fn general_v3_contract_field_names_are_identity_bound_and_lookalikes_stay_positional() {
        let positional_alpha_arguments = || {
            vec![
                descriptor_argument(0, DescriptorArgumentKindV1::Scalar(ScalarTypeV1::F32), 0),
                descriptor_argument(
                    1,
                    DescriptorArgumentKindV1::SharedSlice(ScalarTypeV1::F32),
                    8,
                ),
                descriptor_argument(
                    2,
                    DescriptorArgumentKindV1::DisjointSlice(ScalarTypeV1::F32),
                    24,
                ),
            ]
        };
        let envelope = envelope(CodeObjectVersion::V6);
        let alpha_module = module_for(&["alpha"]);
        let alpha_llvm = construct_inert_compiler_module_text_v1(&alpha_module).unwrap();
        let exact = construct_compiler_descriptor_source_v1(
            &envelope,
            &alpha_module,
            &alpha_llvm,
            &[alpha_root()],
        )
        .unwrap()
        .unwrap();
        let positional = construct_compiler_descriptor_source_v1(
            &envelope,
            &alpha_module,
            &alpha_llvm,
            &[general_root(
                "alpha",
                0x61,
                40,
                positional_alpha_arguments(),
            )],
        )
        .unwrap()
        .unwrap();
        assert_ne!(exact.identity(), positional.identity());

        let lookalike_module = module_for(&["alpha_lookalike"]);
        let lookalike_llvm = construct_inert_compiler_module_text_v1(&lookalike_module).unwrap();
        let lookalike = construct_compiler_descriptor_source_v1(
            &envelope,
            &lookalike_module,
            &lookalike_llvm,
            &[general_root(
                "alpha_lookalike",
                0x62,
                40,
                positional_alpha_arguments(),
            )],
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            lookalike.table().kernels()[0]
                .arguments()
                .iter()
                .map(|argument| argument.name().as_str())
                .collect::<Vec<_>>(),
            ["arg0", "arg1", "arg2"]
        );
    }

    #[test]
    fn semantic_witness_plans_select_only_general_v3_roots_in_binding_order() {
        assert!(
            crate::semantic_witness::plans_from_descriptor_roots(&[root(0x42)])
                .unwrap()
                .is_empty()
        );

        let plans = crate::semantic_witness::plans_from_descriptor_roots(&[
            zeta_root(),
            root(0x42),
            alpha_root(),
        ])
        .unwrap();
        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].kernel_binding().as_bytes(), [0x61; 32]);
        assert_eq!(plans[1].kernel_binding().as_bytes(), [0x7a; 32]);
        assert_ne!(plans[0].payload(), plans[1].payload());
    }

    #[test]
    fn synthetic_roots_retain_distinct_argument_counts_but_vecadd_rejects_them() {
        let one = root_with_layouts(1, vec![layout(false)]);
        let two = root_with_layouts(2, vec![layout(false), layout(true)]);
        assert_eq!(one.arguments.len(), 1);
        assert_eq!(two.arguments.len(), 2);

        let envelope = envelope(CodeObjectVersion::V6);
        let module = module();
        let llvm = construct_inert_compiler_module_text_v1(&module).unwrap();
        for (root, actual) in [(one, 1), (two, 2)] {
            assert!(matches!(
                construct_compiler_descriptor_source_v1(&envelope, &module, &llvm, &[root]),
                Err(CompilerDescriptorError::TypedProfileArgumentCountMismatch {
                    expected: 3,
                    actual: found,
                    ..
                }) if found == actual
            ));
        }
    }

    #[test]
    fn rejects_partial_closures_wrong_cov_and_unrepresentable_capabilities() {
        let mut two_kernels = module();
        let llvm = construct_inert_compiler_module_text_v1(&two_kernels).unwrap();
        let mut second = two_kernels.kernels[0].clone();
        second.id = "other".into();
        two_kernels.kernels.push(second);
        assert!(matches!(
            construct_compiler_descriptor_source_v1(
                &envelope(CodeObjectVersion::V6),
                &two_kernels,
                &llvm,
                &[root(1)],
            ),
            Err(CompilerDescriptorError::IncompleteTypedKernelClosure { typed: 1, total: 2 })
        ));

        let one = module();
        let llvm = construct_inert_compiler_module_text_v1(&one).unwrap();
        assert!(matches!(
            construct_compiler_descriptor_source_v1(
                &envelope(CodeObjectVersion::V5),
                &one,
                &llvm,
                &[root(1)],
            ),
            Err(CompilerDescriptorError::UnsupportedCodeObjectVersion(
                CodeObjectVersion::V5
            ))
        ));

        let mut unsupported = module();
        let llvm = construct_inert_compiler_module_text_v1(&unsupported).unwrap();
        unsupported
            .required_capabilities
            .insert(TargetCapability::Float64);
        assert!(matches!(
            construct_compiler_descriptor_source_v1(
                &envelope(CodeObjectVersion::V6),
                &unsupported,
                &llvm,
                &[root(1)],
            ),
            Err(CompilerDescriptorError::UnsupportedCapability(_))
        ));
    }

    #[test]
    fn raw_modules_stay_unbound_and_exact_inputs_change_evidence() {
        let envelope = envelope(CodeObjectVersion::V6);
        let module = module();
        let llvm = construct_inert_compiler_module_text_v1(&module).unwrap();
        assert!(
            construct_compiler_descriptor_source_v1(&envelope, &module, &llvm, &[])
                .unwrap()
                .is_none()
        );

        let first = construct_compiler_descriptor_source_v1(&envelope, &module, &llvm, &[root(1)])
            .unwrap()
            .unwrap();
        let second = construct_compiler_descriptor_source_v1(&envelope, &module, &llvm, &[root(2)])
            .unwrap()
            .unwrap();
        assert_ne!(first.identity(), second.identity());
        assert_ne!(
            first.table().kernels()[0].source_evidence(),
            second.table().kernels()[0].source_evidence()
        );
        assert_ne!(
            first.table().kernels()[0].executable_ir_evidence(),
            second.table().kernels()[0].executable_ir_evidence()
        );
    }
}
