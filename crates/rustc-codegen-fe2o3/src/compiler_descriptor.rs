//! Rustc-derived descriptor input for the first typed Worker V2 profile.

use crate::collector::{CollectedFunction, TypedKernelProfile};
use crate::kernel_ir_codegen::InertCompilerModuleTextV1;
use crate::rust_type_layout::{ExtractError, extract_exact_typed_vecadd_layout};
use fe2o3_artifacts::{RustLayoutEvidenceV1, TypeIdentity};
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
use reserved_fe2o3_symbols::KernelBindingIdV1;
use rustc_middle::ty::TyCtxt;
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fmt};

const GFX942_PROCESSOR: &str = "gfx942";
const EXPLICIT_ARGUMENT_BYTES: u32 = 48;
// ROCm LLVM COV6 metadata on gfx942 reports 48 explicit bytes followed by its 256-byte
// hidden-argument suffix. Finalization independently checks this against the linked HSACO.
const COV6_KERNARG_SEGMENT_BYTES: u32 = 304;
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
    kernel_binding: KernelBindingIdV1,
    layouts: [RustLayoutEvidenceV1; 3],
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
                if profile != TypedKernelProfile::VecAddRustcLayoutV2 {
                    return Err(CompilerDescriptorError::UnsupportedTypedProfile);
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
                let retained = function.typed_layout_identities.ok_or_else(|| {
                    CompilerDescriptorError::MissingTypedField {
                        kernel: function.export_name.clone(),
                        field: "rustc layout identities",
                    }
                })?;
                let layouts = extract_exact_typed_vecadd_layout(tcx, function.instance)
                    .map_err(CompilerDescriptorError::RustLayout)?;
                let rederived = layouts.each_ref().map(|layout| layout.type_identity());
                if retained != rederived {
                    return Err(CompilerDescriptorError::RetainedLayoutIdentityMismatch(
                        function.export_name.clone(),
                    ));
                }
                Ok(TypedDescriptorRootV1 {
                    logical_name,
                    export_name: function.export_name.clone(),
                    kernel_binding,
                    layouts,
                })
            })
        })
        .collect()
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

    let shared_type =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let disjoint_type =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::F32));
    let shared_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let disjoint_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::F32));

    let module_capabilities = descriptor_capabilities(module)?;
    let mut seen_exports = BTreeSet::new();
    let mut kernels = Vec::with_capacity(typed_roots.len());
    for root in typed_roots {
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

        let arguments = vec![
            LogicalArgumentV1::shared_slice(
                0,
                ValidName::new("input_a")?,
                &shared_type,
                &shared_layout,
                0,
            )?,
            LogicalArgumentV1::shared_slice(
                1,
                ValidName::new("input_b")?,
                &shared_type,
                &shared_layout,
                16,
            )?,
            LogicalArgumentV1::disjoint_slice(
                2,
                ValidName::new("output")?,
                &disjoint_type,
                &disjoint_layout,
                AccessMode::WriteOnly,
                32,
            )?,
        ];
        let source_evidence = source_evidence(root);
        let ir_evidence = ir_evidence(envelope, compiler_module, root);
        kernels.push(KernelDescriptorV1::new(
            KernelId::from_bytes(root.kernel_binding.as_bytes()),
            ValidName::new(root.logical_name.clone())?,
            ValidName::new(root.export_name.clone())?,
            ValidName::new(format!("{}.kd", root.export_name))?,
            source_evidence,
            ir_evidence,
            module_capabilities.clone(),
            KernelAbiLayoutV1::new(
                EXPLICIT_ARGUMENT_BYTES,
                COV6_KERNARG_SEGMENT_BYTES,
                KERNARG_ALIGNMENT_BYTES,
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
            Text::new("typed-vecadd-gfx942-cov6-v1")?,
        ),
        envelope.target(),
        vec![shared_type, disjoint_type],
        vec![shared_layout, disjoint_layout],
        kernels,
    )?;
    CompilerDescriptorSourceV1::new(table)
        .map(Some)
        .map_err(CompilerDescriptorError::Source)
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
    let identities = root.layouts.each_ref().map(|layout| layout.type_identity());
    let identity_bytes = identities.map(type_identity_bytes);
    for bytes in &identity_bytes {
        identity_frames.push(bytes.as_slice());
    }
    let canonical_layouts = root
        .layouts
        .each_ref()
        .map(|layout| layout.canonical_bytes());
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
    UnsupportedTypedProfile,
    MissingTypedField { kernel: String, field: &'static str },
    RustLayout(ExtractError),
    RetainedLayoutIdentityMismatch(String),
    IncompleteTypedKernelClosure { typed: usize, total: usize },
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
            Self::UnsupportedTypedProfile => {
                formatter.write_str("unsupported typed descriptor profile")
            }
            Self::MissingTypedField { kernel, field } => {
                write!(formatter, "typed kernel `{kernel}` has no {field}")
            }
            Self::RustLayout(error) => write!(formatter, "rustc layout extraction failed: {error}"),
            Self::RetainedLayoutIdentityMismatch(kernel) => write!(
                formatter,
                "typed kernel `{kernel}` retained layout identities do not match fresh rustc evidence"
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
        DeviceFfiContractFieldsV1, DeviceFfiDirectionV1, derive_device_ffi_contract_id_v1,
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

    fn root(binding: u8) -> TypedDescriptorRootV1 {
        TypedDescriptorRootV1 {
            logical_name: "add".to_owned(),
            export_name: "vecadd".to_owned(),
            kernel_binding: KernelBindingIdV1::from_bytes([binding; 32]),
            layouts: [layout(false), layout(false), layout(true)],
        }
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
