//! Source correspondence for the attributed LDS tiled-GEMM Slice 1 profile.
//!
//! This is deliberately separate from `collected_tiled_gemm_v1`, whose
//! four-argument register-only ABI and Worker V2 evidence remain unchanged.
//! The receipt produced here may prepare and publish an inert Worker V2 module
//! bound to the exact verified LDS Kernel IR. It grants no worker, link, load,
//! or launch authority.

use std::error::Error;
use std::fmt;

use fe2o3_artifacts::{
    AbiKind, Access, AddressSpace, AliasClass, ArgumentOwnership, BlockSize, Capability,
    Dimensions, Endianness, IdentityText, LaunchContract, Mutability as ArtifactMutability,
    PointerWidth, RustScalarElementTypeV1, TargetIdentity,
};
use fe2o3_compiler_ffi::{
    CodeObjectVersion, CompilerDescriptorSourceV1, CompilerFfiEnvelopeV1, DeviceTargetV1,
};
use fe2o3_kernel_descriptor::{
    AccessMode, AliasSemantics, BlockSizeV1, CapabilityV1, OwnershipSemantics,
};
use fe2o3_kernel_ir::{
    Module, TiledGemmLdsV1Profile, tiled_gemm_lds_v1_module, verify_tiled_gemm_lds_v1_module,
};
use reserved_fe2o3_symbols::{
    CrateBindingIdV1, MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1, derive_kernel_binding_id_v1,
};
use rustc_abi::{CanonAbi, ExternAbi};
use rustc_hir::{Mutability, Safety};
use rustc_middle::ty::{FloatTy, InstanceKind, Ty, TyCtxt, TyKind, TypingEnv, UintTy};
use rustc_target::callconv::{ArgAttributes, ArgExtension, PassMode};
use sha2::{Digest as _, Sha256};

use crate::AmdGpuTarget;
use crate::collected_tiled_gemm_v1::{
    admit_execution_context, exact_collected_root, is_kernel_root_build_identity,
    observe_compiler_semantics, require_compiler_semantics,
};
use crate::collector::{CollectionResult, TypedKernelProfile};
use crate::rust_type_layout_v3::GeneralTypedArgumentKindV3;
use crate::trusted_device_items::{self, TrustedDeviceItem};

pub(crate) const LDS_SLICE1_KERNEL_EXPORT_V1: &str = "tiled_gemm_lds_slice1";
pub(crate) const LDS_SLICE1_EXPLICIT_KERNARG_BYTES_V1: u64 = 48;
pub(crate) const LDS_SLICE1_COMPLETE_KERNARG_BYTES_V1: u64 = 304;
pub(crate) const LDS_SLICE1_CODE_OBJECT_VERSION_V1: u16 = 6;

const AUTHORITY_DOMAIN_V1: &[u8] = b"fe2o3.tiled-gemm-lds-slice1.authority.v1";
const FN_ABI_DOMAIN_V1: &[u8] = b"fe2o3.tiled-gemm-lds-slice1.rustc-fn-abi.v1";
const TRUSTED_DEFINITIONS_DOMAIN_V1: &[u8] = b"fe2o3.tiled-gemm-lds-slice1.trusted-definitions.v1";
const RESOURCE_TRANSCRIPT_DOMAIN_V1: &[u8] = b"fe2o3.tiled-gemm-lds-slice1.worker-v2-resources.v1";
const CANONICAL_IR_DOMAIN_V1: &[u8] = b"fe2o3.tiled-gemm-lds-slice1.compiler-structural-ir.v1";
const ABI_BINDING_V1: &[u8] = b"ptr64;size=48;align=8;a@0:16:8:slice-u16:shared-readonly:bfloat16-bit-carrier;b@16:16:8:slice-u16:shared-readonly:bfloat16-bit-carrier;c@32:16:8:slice-f32:exclusive-readwrite";
const SOURCE_GEOMETRY_BINDING_V1: &[u8] = b"rank=1;block=exact(64,1,1);max-grid=(4294967295,1,1);user-static-shared=0;max-dynamic-shared=0";
const DERIVED_RESOURCE_BINDING_V1: &[u8] = b"rank=1;block=exact(64,1,1);max-grid=(1,1,1);compiler-static-shared=1024;allocation-count=2;allocation-bytes=512;allocation-alignment=16;wave=64;cov=6";
const CORRESPONDENCE_V1: &[u8] = b"exact attributed tiled_gemm_lds_slice1 portable-MIR selects fe2o3::tiled_gemm_lds_v1;two distinct aligned BF16 LDS tiles;XOR4 stage-barrier-read;bounded reviewed correspondence only;not a compiler-refinement proof";
const SOURCE_NAMESPACE_V1: &str =
    "c09558e16157fec495e78bc32a23b082213fa4a6ddabe48445a54cb3de591295";
const EXACT_FRONTEND_CONTRACT_V1: &[u8] = &[
    70, 69, 50, 79, 51, 75, 70, 0, 1, 0, 1, 0, 52, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 64, 0, 0, 0, 1,
    0, 0, 0, 1, 0, 0, 0, 64, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0,
];

const PORTABLE_MIR_SEMANTIC_IDENTITY_V1: [u8; 32] = [
    0x04, 0x67, 0xcd, 0x6d, 0xaa, 0xd4, 0x14, 0xde, 0x74, 0xb6, 0x69, 0xcc, 0x22, 0x3c, 0x26, 0xf3,
    0x8d, 0x36, 0xe2, 0x8b, 0x8a, 0x67, 0x72, 0x4d, 0x24, 0x5f, 0xea, 0x6e, 0x2f, 0x18, 0xd4, 0xfa,
];
const RUSTC_FN_ABI_IDENTITY_V1: [u8; 32] = [
    0xb8, 0x27, 0x0e, 0x78, 0xe7, 0x28, 0xce, 0x0c, 0xe1, 0xc8, 0x16, 0x85, 0x71, 0x85, 0xf7, 0x14,
    0x8c, 0x36, 0x78, 0xa9, 0x69, 0x47, 0xee, 0x95, 0xa5, 0x04, 0x8c, 0xd4, 0xec, 0x12, 0x3d, 0x8d,
];

const ARGUMENT_KINDS_V1: [GeneralTypedArgumentKindV3; 3] = [
    GeneralTypedArgumentKindV3::SharedSlice(RustScalarElementTypeV1::U16),
    GeneralTypedArgumentKindV3::SharedSlice(RustScalarElementTypeV1::U16),
    GeneralTypedArgumentKindV3::DisjointSlice(RustScalarElementTypeV1::F32),
];

const REQUIRED_TRUSTED_ITEMS_V1: [TrustedDeviceItem; 14] = [
    TrustedDeviceItem::ThreadIndex1d,
    TrustedDeviceItem::ThreadIndexGet,
    TrustedDeviceItem::Bf16MfmaFragmentFromBits,
    TrustedDeviceItem::WaveLaneFromRaw,
    TrustedDeviceItem::Gfx942LdsBf16TilePairM16x16,
    TrustedDeviceItem::LdsTile16x16WriteMfmaBf16,
    TrustedDeviceItem::WorkgroupSyncthreads,
    TrustedDeviceItem::LdsTile16x16AssumeInit,
    TrustedDeviceItem::LdsTile16x16ReadMfmaBf16,
    TrustedDeviceItem::DeviceMatrixFromCompiler,
    TrustedDeviceItem::DeviceMatrixMultiplyAccumulate,
    TrustedDeviceItem::F32AccumulatorFragmentIntoValues,
    TrustedDeviceItem::DisjointSliceLen,
    TrustedDeviceItem::DisjointSliceGetMutAt,
];

#[derive(Debug, Eq, PartialEq)]
struct LdsSlice1AuthorityV1 {
    target: String,
    code_object_version: u16,
    explicit_kernarg_bytes: u64,
    complete_kernarg_bytes: u64,
    root_instance_identity: String,
    kernel_export: String,
    portable_mir_identity: [u8; 32],
    compiler_semantics_identity: [u8; 32],
    fn_abi_identity: [u8; 32],
    trusted_definitions_identity: [u8; 32],
    frontend_contract_identity: [u8; 32],
    abi_binding_identity: [u8; 32],
    source_geometry_identity: [u8; 32],
    derived_resource_identity: [u8; 32],
    correspondence_identity: [u8; 32],
    canonical_ir_identity: [u8; 32],
    descriptor_source_identity: [u8; 32],
    authority_commitment: [u8; 32],
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct LdsSlice1FrontendReceiptV1 {
    authority: Option<LdsSlice1AuthorityV1>,
    prepared_resources: Option<PreparedLdsSlice1ResourceMetadataV1>,
    descriptor_source: Option<CompilerDescriptorSourceV1>,
    compiler_module: Option<crate::kernel_ir_codegen::InertCompilerModuleTextV1>,
}

impl LdsSlice1FrontendReceiptV1 {
    fn authority(&self) -> &LdsSlice1AuthorityV1 {
        self.authority
            .as_ref()
            .expect("unconsumed LDS Slice 1 authority")
    }

    pub(crate) fn root_instance_identity(&self) -> &str {
        &self.authority().root_instance_identity
    }

    pub(crate) fn portable_mir_semantic_hex(&self) -> String {
        encode_hex(&self.authority().portable_mir_identity)
    }

    pub(crate) fn authority_hex(&self) -> String {
        encode_hex(&self.authority().authority_commitment)
    }

    pub(crate) const fn authority_commitment(&self) -> &[u8; 32] {
        &self
            .authority
            .as_ref()
            .expect("unconsumed LDS Slice 1 authority")
            .authority_commitment
    }

    pub(crate) fn consume(
        &mut self,
    ) -> Result<AuthenticatedLdsSlice1ModuleV1, CollectedLdsSlice1ErrorV1> {
        let authority = self
            .authority
            .take()
            .ok_or(CollectedLdsSlice1ErrorV1::ReceiptAlreadyConsumed)?;
        let prepared_resources = self
            .prepared_resources
            .take()
            .ok_or(CollectedLdsSlice1ErrorV1::ReceiptAlreadyConsumed)?;
        let descriptor_source = self
            .descriptor_source
            .take()
            .ok_or(CollectedLdsSlice1ErrorV1::ReceiptAlreadyConsumed)?;
        let compiler_module = self
            .compiler_module
            .take()
            .ok_or(CollectedLdsSlice1ErrorV1::ReceiptAlreadyConsumed)?;
        validate_authority(&authority)?;
        let module = tiled_gemm_lds_v1_module();
        verify_tiled_gemm_lds_v1_module(
            &module,
            &TiledGemmLdsV1Profile::exact_gfx942_xnack_minus_cov6(),
        )
        .map_err(|error| CollectedLdsSlice1ErrorV1::CanonicalIr(error.to_string()))?;
        let canonical_ir_identity = canonical_ir_identity(&module)?;
        if canonical_ir_identity != authority.canonical_ir_identity {
            return Err(CollectedLdsSlice1ErrorV1::ReceiptBinding(
                "canonical Kernel IR",
            ));
        }
        if descriptor_source.identity().sha256() != &authority.descriptor_source_identity {
            return Err(CollectedLdsSlice1ErrorV1::ReceiptBinding(
                "compiler descriptor source",
            ));
        }
        validate_descriptor_source(&descriptor_source)?;
        let resources = finalize_resource_metadata(prepared_resources, &module)?;
        let resource_transcript = resource_transcript(&authority, &resources);
        Ok(AuthenticatedLdsSlice1ModuleV1 {
            module,
            descriptor_source,
            compiler_module,
            resources,
            source_authority_commitment: authority.authority_commitment,
            canonical_ir_identity,
            resource_transcript,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PreparedLdsSlice1ResourceMetadataV1 {
    source_launch: LaunchContract,
    compiler_launch: LaunchContract,
    lds_allocations: u32,
    lds_bytes_per_allocation: u32,
    lds_alignment: u32,
}

#[derive(Debug, Eq, PartialEq)]
struct FinalLdsSlice1ResourceMetadataV1 {
    source_launch: LaunchContract,
    compiler_launch: LaunchContract,
    lds_allocations: u32,
    lds_bytes_per_allocation: u32,
    lds_alignment: u32,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedLdsSlice1ModuleV1 {
    module: Module,
    descriptor_source: CompilerDescriptorSourceV1,
    compiler_module: crate::kernel_ir_codegen::InertCompilerModuleTextV1,
    resources: FinalLdsSlice1ResourceMetadataV1,
    source_authority_commitment: [u8; 32],
    canonical_ir_identity: [u8; 32],
    resource_transcript: Vec<u8>,
}

impl AuthenticatedLdsSlice1ModuleV1 {
    pub(crate) fn module(&self) -> &Module {
        &self.module
    }

    pub(crate) const fn source_authority_commitment(&self) -> &[u8; 32] {
        &self.source_authority_commitment
    }

    pub(crate) const fn canonical_ir_identity(&self) -> &[u8; 32] {
        &self.canonical_ir_identity
    }

    pub(crate) fn descriptor_source(&self) -> &CompilerDescriptorSourceV1 {
        &self.descriptor_source
    }

    #[cfg(test)]
    pub(crate) fn compiler_module(&self) -> &crate::kernel_ir_codegen::InertCompilerModuleTextV1 {
        &self.compiler_module
    }

    pub(crate) fn resource_transcript(&self) -> &[u8] {
        &self.resource_transcript
    }

    pub(crate) const fn source_static_shared_memory_bytes(&self) -> u32 {
        self.resources.source_launch.static_shared_memory_bytes()
    }

    pub(crate) const fn compiler_static_shared_memory_bytes(&self) -> u32 {
        self.resources.compiler_launch.static_shared_memory_bytes()
    }

    pub(crate) const fn lds_allocations(&self) -> u32 {
        self.resources.lds_allocations
    }

    pub(crate) const fn lds_bytes_per_allocation(&self) -> u32 {
        self.resources.lds_bytes_per_allocation
    }

    pub(crate) const fn lds_alignment(&self) -> u32 {
        self.resources.lds_alignment
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        Module,
        CompilerDescriptorSourceV1,
        crate::kernel_ir_codegen::InertCompilerModuleTextV1,
        [u8; 32],
        [u8; 32],
        Vec<u8>,
    ) {
        (
            self.module,
            self.descriptor_source,
            self.compiler_module,
            self.source_authority_commitment,
            self.canonical_ir_identity,
            self.resource_transcript,
        )
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum CollectedLdsSlice1ErrorV1 {
    Admission(String),
    Abi(String),
    Layout(String),
    PortableMir(String),
    PortableMirIdentity {
        expected: [u8; 32],
        actual: [u8; 32],
    },
    FnAbiIdentity {
        expected: [u8; 32],
        actual: [u8; 32],
    },
    TrustedDefinitions(String),
    ReceiptAlreadyConsumed,
    ReceiptBinding(&'static str),
    CanonicalIr(String),
    Descriptor(String),
}

impl fmt::Display for CollectedLdsSlice1ErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission(detail) => {
                write!(formatter, "LDS Slice 1 admission rejected: {detail}")
            }
            Self::Abi(detail) => write!(formatter, "LDS Slice 1 ABI mismatch: {detail}"),
            Self::Layout(detail) => write!(formatter, "LDS Slice 1 layout mismatch: {detail}"),
            Self::PortableMir(detail) => {
                write!(formatter, "LDS Slice 1 portable MIR rejected: {detail}")
            }
            Self::PortableMirIdentity { expected, actual } => write!(
                formatter,
                "LDS Slice 1 portable MIR identity mismatch: expected {}, found {}",
                encode_hex(expected),
                encode_hex(actual)
            ),
            Self::FnAbiIdentity { expected, actual } => write!(
                formatter,
                "LDS Slice 1 rustc FnAbi identity mismatch: expected {}, found {}",
                encode_hex(expected),
                encode_hex(actual)
            ),
            Self::TrustedDefinitions(detail) => {
                write!(
                    formatter,
                    "LDS Slice 1 trusted definitions rejected: {detail}"
                )
            }
            Self::ReceiptAlreadyConsumed => {
                formatter.write_str("LDS Slice 1 frontend receipt was already consumed")
            }
            Self::ReceiptBinding(field) => {
                write!(
                    formatter,
                    "LDS Slice 1 frontend receipt binding mismatch: {field}"
                )
            }
            Self::CanonicalIr(detail) => {
                write!(
                    formatter,
                    "canonical LDS Slice 1 Kernel IR rejected: {detail}"
                )
            }
            Self::Descriptor(detail) => {
                write!(
                    formatter,
                    "LDS Slice 1 compiler descriptor rejected: {detail}"
                )
            }
        }
    }
}

impl Error for CollectedLdsSlice1ErrorV1 {}

pub(crate) fn is_lds_slice1_collection(collection: &CollectionResult<'_>) -> bool {
    collection.functions.len() == 1
        && collection.functions[0].export_name == LDS_SLICE1_KERNEL_EXPORT_V1
}

pub(crate) fn authenticate_collected_lds_slice1_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    collection: &CollectionResult<'tcx>,
    target: &AmdGpuTarget,
    custom_llvm_pipeline: bool,
) -> Result<LdsSlice1FrontendReceiptV1, CollectedLdsSlice1ErrorV1> {
    admit_execution_context(target.as_str(), custom_llvm_pipeline)
        .map_err(|error| CollectedLdsSlice1ErrorV1::Admission(error.to_string()))?;
    let compiler_semantics = observe_compiler_semantics(tcx);
    let compiler_semantics_identity = require_compiler_semantics(&compiler_semantics)
        .map_err(|error| CollectedLdsSlice1ErrorV1::Admission(error.to_string()))?;
    let root = exact_collected_root(&collection.functions)
        .map_err(|error| CollectedLdsSlice1ErrorV1::Admission(error.to_string()))?;
    require_registration(root)?;
    require_signature(tcx, root.instance)?;
    require_layout(root)?;
    let fn_abi_identity = require_fn_abi(tcx, root.instance)?;
    let trusted_definitions_identity = trusted_definitions_identity(tcx)?;

    let target_identity = target_identity()?;
    let launch = exact_launch()?;
    let contract = root
        .general_typed_contract
        .as_ref()
        .ok_or_else(|| CollectedLdsSlice1ErrorV1::Layout("General V3 contract is absent".into()))?;
    let prepared_resources = prepare_resource_metadata(contract.launch())?;
    let imported = crate::mir_import::import_collection(tcx, collection)
        .map_err(|error| CollectedLdsSlice1ErrorV1::PortableMir(error.to_string()))?;
    let portable_mir_identity = imported
        .portable_semantic_digest_v2(crate::mir_import::MirSemanticAdmissionInputsV2::new(
            LDS_SLICE1_KERNEL_EXPORT_V1,
            &target_identity,
            contract.abi(),
            &launch,
        ))
        .map_err(|error| CollectedLdsSlice1ErrorV1::PortableMir(error.to_string()))?;
    let portable_mir_identity = *portable_mir_identity.as_bytes();
    if portable_mir_identity != PORTABLE_MIR_SEMANTIC_IDENTITY_V1 {
        return Err(CollectedLdsSlice1ErrorV1::PortableMirIdentity {
            expected: PORTABLE_MIR_SEMANTIC_IDENTITY_V1,
            actual: portable_mir_identity,
        });
    }

    let root_instance_identity = tcx.def_path_str(root.instance.def_id());
    if !is_kernel_root_build_identity(&root_instance_identity) {
        return Err(CollectedLdsSlice1ErrorV1::Admission(format!(
            "root instance has a noncanonical generated identity `{root_instance_identity}`"
        )));
    }
    let descriptor_roots = crate::compiler_descriptor::typed_descriptor_roots_from_collection(
        tcx,
        &collection.functions,
    )
    .map_err(|error| {
        CollectedLdsSlice1ErrorV1::Descriptor(format!("typed source evidence rejected: {error}"))
    })?;
    let module = tiled_gemm_lds_v1_module();
    verify_tiled_gemm_lds_v1_module(
        &module,
        &TiledGemmLdsV1Profile::exact_gfx942_xnack_minus_cov6(),
    )
    .map_err(|error| CollectedLdsSlice1ErrorV1::CanonicalIr(error.to_string()))?;
    let canonical_ir_identity = canonical_ir_identity(&module)?;
    let compiler_module =
        crate::kernel_ir_codegen::construct_inert_tiled_gemm_lds_slice1_module_text(&module)
            .map_err(|error| {
                CollectedLdsSlice1ErrorV1::CanonicalIr(format!(
                    "dedicated upstream-LLVM lowering rejected: {error}"
                ))
            })?;
    let device_target =
        DeviceTargetV1::parse(crate::collected_tiled_gemm_v1::EXACT_TILED_GEMM_TARGET_V1)
            .expect("fixed LDS Slice 1 target is valid");
    let envelope =
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(device_target, CodeObjectVersion::V6)
            .map_err(|error| {
                CollectedLdsSlice1ErrorV1::Descriptor(format!(
                    "compiler envelope rejected: {error}"
                ))
            })?;
    let descriptor_source =
        crate::compiler_descriptor::construct_tiled_gemm_lds_slice1_compiler_descriptor_source_v1(
            &envelope,
            &module,
            &compiler_module,
            &descriptor_roots,
        )
        .map_err(|error| CollectedLdsSlice1ErrorV1::Descriptor(error.to_string()))?
        .ok_or_else(|| {
            CollectedLdsSlice1ErrorV1::Descriptor(
                "exact compiler descriptor source is absent".to_owned(),
            )
        })?;
    validate_descriptor_source(&descriptor_source)?;
    let descriptor_source_identity = *descriptor_source.identity().sha256();
    let frontend_contract_identity = sha256(
        root.frontend_contract
            .as_ref()
            .expect("registration admission requires a frontend contract")
            .canonical_bytes(),
    );
    let mut authority = LdsSlice1AuthorityV1 {
        target: target.as_str().to_owned(),
        code_object_version: LDS_SLICE1_CODE_OBJECT_VERSION_V1,
        explicit_kernarg_bytes: LDS_SLICE1_EXPLICIT_KERNARG_BYTES_V1,
        complete_kernarg_bytes: LDS_SLICE1_COMPLETE_KERNARG_BYTES_V1,
        root_instance_identity,
        kernel_export: root.export_name.clone(),
        portable_mir_identity,
        compiler_semantics_identity,
        fn_abi_identity,
        trusted_definitions_identity,
        frontend_contract_identity,
        abi_binding_identity: sha256(ABI_BINDING_V1),
        source_geometry_identity: sha256(SOURCE_GEOMETRY_BINDING_V1),
        derived_resource_identity: sha256(DERIVED_RESOURCE_BINDING_V1),
        correspondence_identity: sha256(CORRESPONDENCE_V1),
        canonical_ir_identity,
        descriptor_source_identity,
        authority_commitment: [0; 32],
    };
    authority.authority_commitment = authority_identity(&authority);
    Ok(LdsSlice1FrontendReceiptV1 {
        authority: Some(authority),
        prepared_resources: Some(prepared_resources),
        descriptor_source: Some(descriptor_source),
        compiler_module: Some(compiler_module),
    })
}

fn require_registration(
    root: &crate::collector::CollectedFunction<'_>,
) -> Result<(), CollectedLdsSlice1ErrorV1> {
    let namespace = CrateBindingIdV1::from_hex(SOURCE_NAMESPACE_V1)
        .expect("reviewed Slice 1 namespace is canonical");
    let expected_binding = derive_kernel_binding_id_v1(
        namespace,
        MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
        LDS_SLICE1_KERNEL_EXPORT_V1,
        LDS_SLICE1_KERNEL_EXPORT_V1,
    );
    if root.export_name != LDS_SLICE1_KERNEL_EXPORT_V1
        || root.logical_name.as_deref() != Some(LDS_SLICE1_KERNEL_EXPORT_V1)
        || !matches!(
            root.typed_profile,
            Some(TypedKernelProfile::GeneralScalarSliceRustcLayoutV3 { .. })
        )
        || root.kernel_binding != Some(expected_binding)
        || root.frontend_contract.is_none()
    {
        return Err(CollectedLdsSlice1ErrorV1::Admission(
            "expected the unique attributed General V3 tiled_gemm_lds_slice1 root with its reviewed namespace binding".into(),
        ));
    }
    Ok(())
}

fn require_signature<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: rustc_middle::ty::Instance<'tcx>,
) -> Result<(), CollectedLdsSlice1ErrorV1> {
    if !matches!(instance.def, InstanceKind::Item(_)) || !instance.args.is_empty() {
        return Err(CollectedLdsSlice1ErrorV1::Abi(
            "kernel must be one nongeneric ordinary function item".into(),
        ));
    }
    let signature = tcx
        .try_instantiate_and_normalize_erasing_regions(
            instance.args,
            TypingEnv::fully_monomorphized(),
            tcx.fn_sig(instance.def_id()),
        )
        .map_err(|_| CollectedLdsSlice1ErrorV1::Abi("signature normalization failed".into()))?;
    let signature = tcx.instantiate_bound_regions_with_erased(signature);
    if signature.safety != Safety::Safe
        || signature.abi != ExternAbi::Rust
        || signature.c_variadic
        || signature.output() != tcx.types.unit
        || signature.inputs().len() != 3
    {
        return Err(CollectedLdsSlice1ErrorV1::Abi(format!(
            "expected safe Rust `(&[u16], &[u16], DisjointSlice<f32>) -> ()`, found `{signature}`"
        )));
    }
    let inputs = signature.inputs();
    if !is_shared_u16_slice(inputs[0])
        || !is_shared_u16_slice(inputs[1])
        || !is_disjoint_f32_slice(tcx, inputs[2])
    {
        return Err(CollectedLdsSlice1ErrorV1::Abi(format!(
            "expected A:&[u16], B:&[u16], C:DisjointSlice<f32>, found `{signature}`"
        )));
    }
    Ok(())
}

fn is_shared_u16_slice(ty: Ty<'_>) -> bool {
    matches!(
        ty.kind(),
        TyKind::Ref(_, pointee, Mutability::Not)
            if matches!(pointee.kind(), TyKind::Slice(element) if matches!(element.kind(), TyKind::Uint(UintTy::U16)))
    )
}

fn is_disjoint_f32_slice(tcx: TyCtxt<'_>, ty: Ty<'_>) -> bool {
    let TyKind::Adt(definition, args) = ty.kind() else {
        return false;
    };
    trusted_device_items::classify(tcx, definition.did()) == Some(TrustedDeviceItem::DisjointSlice)
        && args.len() == 2
        && args
            .first()
            .and_then(|argument| argument.as_type())
            .is_some_and(|element| matches!(element.kind(), TyKind::Float(FloatTy::F32)))
}

fn require_layout(
    root: &crate::collector::CollectedFunction<'_>,
) -> Result<(), CollectedLdsSlice1ErrorV1> {
    let identities = root.typed_layout_identities.as_ref().ok_or_else(|| {
        CollectedLdsSlice1ErrorV1::Layout("per-argument identities are absent".into())
    })?;
    if identities.len() != 3 {
        return Err(CollectedLdsSlice1ErrorV1::Layout(format!(
            "expected three argument identities, found {}",
            identities.len()
        )));
    }
    let contract = root
        .general_typed_contract
        .as_ref()
        .ok_or_else(|| CollectedLdsSlice1ErrorV1::Layout("General V3 contract is absent".into()))?;
    let actual = contract
        .arguments()
        .iter()
        .map(|argument| argument.kind())
        .collect::<Vec<_>>();
    if actual != ARGUMENT_KINDS_V1 {
        return Err(CollectedLdsSlice1ErrorV1::Layout(format!(
            "expected argument kinds {ARGUMENT_KINDS_V1:?}, found {actual:?}"
        )));
    }
    let abi = contract.abi();
    if abi.size() != 48 || abi.alignment() != 8 || abi.pointer_width() != PointerWidth::Bits64 {
        return Err(CollectedLdsSlice1ErrorV1::Layout(format!(
            "expected 48-byte, align-8, ptr64 ABI; found {} bytes, align {}, {:?}",
            abi.size(),
            abi.alignment(),
            abi.pointer_width()
        )));
    }
    let expected_names = ["arg0", "arg1", "arg2"];
    let expected_offsets = [0, 16, 32];
    if abi.fields().len() != 3 {
        return Err(CollectedLdsSlice1ErrorV1::Layout(format!(
            "expected three ABI fields, found {}",
            abi.fields().len()
        )));
    }
    for (index, field) in abi.fields().iter().enumerate() {
        if field.name().as_str() != expected_names[index]
            || field.offset() != expected_offsets[index]
            || field.size() != 16
            || field.alignment() != 8
            || field.type_identity() != contract.arguments()[index].type_identity()
        {
            return Err(CollectedLdsSlice1ErrorV1::Layout(format!(
                "ABI field {index} identity, offset, size, or alignment drifted"
            )));
        }
        match index {
            0 | 1 => {
                if !matches!(
                    field.kind(),
                    AbiKind::Slice {
                        element_size: 2,
                        element_alignment: 2
                    }
                ) || field.mutability() != ArtifactMutability::Immutable
                    || field.access() != Access::ReadOnly
                    || field.address_space() != AddressSpace::Global
                    || field.ownership() != ArgumentOwnership::SharedBorrow
                    || field.alias_class() != AliasClass::SharedReadOnly
                {
                    return Err(CollectedLdsSlice1ErrorV1::Layout(format!(
                        "field {index} is not the exact shared &[u16] carrier"
                    )));
                }
            }
            2 => {
                if !matches!(
                    field.kind(),
                    AbiKind::Slice {
                        element_size: 4,
                        element_alignment: 4
                    }
                ) || field.mutability() != ArtifactMutability::Mutable
                    || field.access() != Access::ReadWrite
                    || field.address_space() != AddressSpace::Global
                    || field.ownership() != ArgumentOwnership::UniqueBorrow
                    || field.alias_class() != AliasClass::Exclusive
                {
                    return Err(CollectedLdsSlice1ErrorV1::Layout(
                        "field 2 is not the exact DisjointSlice<f32> output".into(),
                    ));
                }
            }
            _ => unreachable!(),
        }
    }
    let transport = contract.launch();
    if transport.rank() != 1
        || transport.block_size()
            != BlockSize::Exact(
                Dimensions::new(64, 1, 1)
                    .map_err(|error| CollectedLdsSlice1ErrorV1::Layout(error.to_string()))?,
            )
        || transport.max_grid()
            != Dimensions::new(u32::MAX, 1, 1)
                .map_err(|error| CollectedLdsSlice1ErrorV1::Layout(error.to_string()))?
        || transport.static_shared_memory_bytes() != 0
        || transport.max_dynamic_shared_memory_bytes() != 0
    {
        return Err(CollectedLdsSlice1ErrorV1::Layout(
            "generated source launch is not exact WG64 with no user-supplied shared memory".into(),
        ));
    }
    let frontend = root
        .frontend_contract
        .as_ref()
        .ok_or_else(|| CollectedLdsSlice1ErrorV1::Layout("frontend contract is absent".into()))?;
    if frontend.canonical_bytes() != EXACT_FRONTEND_CONTRACT_V1 {
        return Err(CollectedLdsSlice1ErrorV1::Layout(
            "frontend bytes differ from exact required=max=64x1x1".into(),
        ));
    }
    Ok(())
}

fn require_fn_abi<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: rustc_middle::ty::Instance<'tcx>,
) -> Result<[u8; 32], CollectedLdsSlice1ErrorV1> {
    let query = TypingEnv::fully_monomorphized()
        .as_query_input((instance, rustc_middle::ty::List::empty()));
    let abi = tcx.fn_abi_of_instance(query).map_err(|error| {
        CollectedLdsSlice1ErrorV1::Abi(format!("FnAbi query failed: {error:?}"))
    })?;
    if abi.conv != CanonAbi::Rust
        || abi.c_variadic
        || abi.fixed_count != 3
        || abi.args.len() != 3
        || !matches!(abi.ret.mode, PassMode::Ignore)
        || abi.ret.layout.size.bytes() != 0
    {
        return Err(CollectedLdsSlice1ErrorV1::Abi(format!(
            "FnAbi header must be Rust(args=3)->unit, found {abi:?}"
        )));
    }
    let mut digest = Sha256::new();
    hash_field(&mut digest, FN_ABI_DOMAIN_V1);
    hash_field(&mut digest, &[u8::from(abi.c_variadic)]);
    hash_field(&mut digest, &abi.fixed_count.to_le_bytes());
    hash_field(&mut digest, &[u8::from(abi.can_unwind)]);
    for (index, argument) in abi.args.iter().enumerate() {
        if argument.layout.size.bytes() != 16
            || argument.layout.align.abi.bytes() != 8
            || !matches!(argument.mode, PassMode::Pair(_, _))
        {
            return Err(CollectedLdsSlice1ErrorV1::Abi(format!(
                "FnAbi argument {index} is not Pair(size=16, align=8)"
            )));
        }
        hash_field(&mut digest, &argument.layout.size.bytes().to_le_bytes());
        hash_field(
            &mut digest,
            &argument.layout.align.abi.bytes().to_le_bytes(),
        );
        let PassMode::Pair(first, second) = argument.mode else {
            unreachable!("checked above")
        };
        hash_arg_attributes(&mut digest, first);
        hash_arg_attributes(&mut digest, second);
    }
    let actual: [u8; 32] = digest.finalize().into();
    if actual != RUSTC_FN_ABI_IDENTITY_V1 {
        return Err(CollectedLdsSlice1ErrorV1::FnAbiIdentity {
            expected: RUSTC_FN_ABI_IDENTITY_V1,
            actual,
        });
    }
    Ok(actual)
}

fn hash_arg_attributes(digest: &mut Sha256, attributes: ArgAttributes) {
    hash_field(digest, &attributes.regular.bits().to_le_bytes());
    let extension = match attributes.arg_ext {
        ArgExtension::None => 0_u8,
        ArgExtension::Zext => 1,
        ArgExtension::Sext => 2,
    };
    hash_field(digest, &[extension]);
    hash_field(digest, &attributes.pointee_size.bytes().to_le_bytes());
    let alignment = attributes
        .pointee_align
        .map_or(0, |alignment| alignment.bytes());
    hash_field(digest, &alignment.to_le_bytes());
}

fn trusted_definitions_identity(tcx: TyCtxt<'_>) -> Result<[u8; 32], CollectedLdsSlice1ErrorV1> {
    let mut digest = Sha256::new();
    hash_field(&mut digest, TRUSTED_DEFINITIONS_DOMAIN_V1);
    let mut provider = None;
    for item in REQUIRED_TRUSTED_ITEMS_V1 {
        let definition = trusted_device_items::definition(tcx, item).ok_or_else(|| {
            CollectedLdsSlice1ErrorV1::TrustedDefinitions(format!(
                "missing exact diagnostic item `{}`",
                item.canonical_path()
            ))
        })?;
        if definition.is_local() {
            return Err(CollectedLdsSlice1ErrorV1::TrustedDefinitions(format!(
                "`{}` came from the kernel crate",
                item.canonical_path()
            )));
        }
        if provider.is_some_and(|krate| krate != definition.krate) {
            return Err(CollectedLdsSlice1ErrorV1::TrustedDefinitions(format!(
                "`{}` came from a different provider crate",
                item.canonical_path()
            )));
        }
        provider.get_or_insert(definition.krate);
        hash_field(&mut digest, item.canonical_path().as_bytes());
        hash_field(&mut digest, &tcx.def_path_hash(definition).0.to_le_bytes());
    }
    Ok(digest.finalize().into())
}

fn target_identity() -> Result<TargetIdentity, CollectedLdsSlice1ErrorV1> {
    TargetIdentity::new(
        IdentityText::new(dialect_amdgcn::AMDGPU_TRIPLE)
            .map_err(|error| CollectedLdsSlice1ErrorV1::Admission(error.to_string()))?,
        IdentityText::new(crate::collected_tiled_gemm_v1::EXACT_TILED_GEMM_TARGET_V1)
            .map_err(|error| CollectedLdsSlice1ErrorV1::Admission(error.to_string()))?,
        PointerWidth::Bits64,
        Endianness::Little,
        vec![
            Capability::WorkgroupMemory,
            Capability::MatrixMultiply,
            Capability::AmdWave,
            Capability::AmdMfma,
        ],
    )
    .map_err(|error| CollectedLdsSlice1ErrorV1::Admission(error.to_string()))
}

fn exact_launch() -> Result<LaunchContract, CollectedLdsSlice1ErrorV1> {
    LaunchContract::new(
        1,
        BlockSize::Exact(
            Dimensions::new(64, 1, 1)
                .map_err(|error| CollectedLdsSlice1ErrorV1::Layout(error.to_string()))?,
        ),
        Dimensions::new(1, 1, 1)
            .map_err(|error| CollectedLdsSlice1ErrorV1::Layout(error.to_string()))?,
        fe2o3_kernel_ir::TILED_GEMM_LDS_V1_STATIC_LDS_BYTES,
        0,
    )
    .map_err(|error| CollectedLdsSlice1ErrorV1::Layout(error.to_string()))
}

fn prepare_resource_metadata(
    source_launch: &LaunchContract,
) -> Result<PreparedLdsSlice1ResourceMetadataV1, CollectedLdsSlice1ErrorV1> {
    if source_launch != &exact_source_launch()? {
        return Err(CollectedLdsSlice1ErrorV1::ReceiptBinding("source geometry"));
    }
    let profile = TiledGemmLdsV1Profile::exact_gfx942_xnack_minus_cov6();
    let prepared = PreparedLdsSlice1ResourceMetadataV1 {
        source_launch: source_launch.clone(),
        compiler_launch: exact_launch()?,
        lds_allocations: profile.lds_allocations,
        lds_bytes_per_allocation: profile.lds_bytes_per_allocation,
        lds_alignment: profile.lds_alignment,
    };
    validate_prepared_resource_metadata(&prepared)?;
    Ok(prepared)
}

fn exact_source_launch() -> Result<LaunchContract, CollectedLdsSlice1ErrorV1> {
    LaunchContract::new(
        1,
        BlockSize::Exact(
            Dimensions::new(64, 1, 1)
                .map_err(|error| CollectedLdsSlice1ErrorV1::Layout(error.to_string()))?,
        ),
        Dimensions::new(u32::MAX, 1, 1)
            .map_err(|error| CollectedLdsSlice1ErrorV1::Layout(error.to_string()))?,
        0,
        0,
    )
    .map_err(|error| CollectedLdsSlice1ErrorV1::Layout(error.to_string()))
}

fn validate_prepared_resource_metadata(
    prepared: &PreparedLdsSlice1ResourceMetadataV1,
) -> Result<(), CollectedLdsSlice1ErrorV1> {
    if prepared.source_launch != exact_source_launch()?
        || prepared.source_launch.static_shared_memory_bytes() != 0
        || prepared.source_launch.max_dynamic_shared_memory_bytes() != 0
    {
        return Err(CollectedLdsSlice1ErrorV1::ReceiptBinding("source geometry"));
    }
    let exact_compiler_launch = exact_launch()?;
    if prepared.compiler_launch != exact_compiler_launch
        || prepared.compiler_launch.static_shared_memory_bytes()
            != fe2o3_kernel_ir::TILED_GEMM_LDS_V1_STATIC_LDS_BYTES
        || prepared.lds_allocations != fe2o3_kernel_ir::TILED_GEMM_LDS_V1_ALLOCATION_COUNT
        || prepared.lds_bytes_per_allocation != fe2o3_kernel_ir::TILED_GEMM_LDS_V1_TILE_BYTES
        || prepared.lds_alignment != fe2o3_kernel_ir::TILED_GEMM_LDS_V1_LDS_ALIGNMENT
        || prepared
            .lds_allocations
            .checked_mul(prepared.lds_bytes_per_allocation)
            != Some(prepared.compiler_launch.static_shared_memory_bytes())
    {
        return Err(CollectedLdsSlice1ErrorV1::ReceiptBinding(
            "compiler-derived LDS resources",
        ));
    }
    Ok(())
}

fn finalize_resource_metadata(
    prepared: PreparedLdsSlice1ResourceMetadataV1,
    module: &Module,
) -> Result<FinalLdsSlice1ResourceMetadataV1, CollectedLdsSlice1ErrorV1> {
    validate_prepared_resource_metadata(&prepared)?;
    if module != &tiled_gemm_lds_v1_module() {
        return Err(CollectedLdsSlice1ErrorV1::CanonicalIr(
            "resource metadata was paired with a noncanonical module".into(),
        ));
    }
    Ok(FinalLdsSlice1ResourceMetadataV1 {
        source_launch: prepared.source_launch,
        compiler_launch: prepared.compiler_launch,
        lds_allocations: prepared.lds_allocations,
        lds_bytes_per_allocation: prepared.lds_bytes_per_allocation,
        lds_alignment: prepared.lds_alignment,
    })
}

fn canonical_ir_identity(module: &Module) -> Result<[u8; 32], CollectedLdsSlice1ErrorV1> {
    if module != &tiled_gemm_lds_v1_module() {
        return Err(CollectedLdsSlice1ErrorV1::CanonicalIr(
            "structural identity requested for a noncanonical module".to_owned(),
        ));
    }
    let canonical = fe2o3_kernel_ir::encode_module_v5(module).map_err(|error| {
        CollectedLdsSlice1ErrorV1::CanonicalIr(format!(
            "canonical V5 Kernel IR encoding failed: {error}"
        ))
    })?;
    let mut digest = Sha256::new();
    hash_field(&mut digest, CANONICAL_IR_DOMAIN_V1);
    hash_field(&mut digest, &canonical);
    Ok(digest.finalize().into())
}

fn validate_descriptor_source(
    source: &CompilerDescriptorSourceV1,
) -> Result<(), CollectedLdsSlice1ErrorV1> {
    let table = source.table();
    if table.device_target().to_string()
        != crate::collected_tiled_gemm_v1::EXACT_TILED_GEMM_TARGET_V1
        || table.code_object_version() != CodeObjectVersion::V6
        || table.kernels().len() != 1
    {
        return Err(CollectedLdsSlice1ErrorV1::Descriptor(
            "target, COV6, or one-kernel closure drifted".to_owned(),
        ));
    }
    let kernel = &table.kernels()[0];
    if kernel.logical_name().as_str() != LDS_SLICE1_KERNEL_EXPORT_V1
        || kernel.entry_name().as_str() != fe2o3_kernel_ir::TILED_GEMM_LDS_V1_KERNEL_ID
        || kernel.descriptor_symbol().as_str()
            != format!("{}.kd", fe2o3_kernel_ir::TILED_GEMM_LDS_V1_KERNEL_ID)
    {
        return Err(CollectedLdsSlice1ErrorV1::Descriptor(
            "source logical name or canonical LLVM symbol closure drifted".to_owned(),
        ));
    }
    let abi = kernel.abi_layout();
    if abi.explicit_argument_size() != LDS_SLICE1_EXPLICIT_KERNARG_BYTES_V1 as u32
        || abi.kernarg_segment_size() != LDS_SLICE1_COMPLETE_KERNARG_BYTES_V1 as u32
        || abi.kernarg_segment_alignment() != 8
    {
        return Err(CollectedLdsSlice1ErrorV1::Descriptor(
            "48/304-byte COV6 kernarg contract drifted".to_owned(),
        ));
    }
    let launch = kernel.launch();
    let BlockSizeV1::Exact(block) = launch.block_size() else {
        return Err(CollectedLdsSlice1ErrorV1::Descriptor(
            "workgroup size is not exact".to_owned(),
        ));
    };
    let grid = launch.max_grid();
    if launch.rank() != 1
        || (block.x(), block.y(), block.z()) != (64, 1, 1)
        || (grid.x(), grid.y(), grid.z()) != (1, 1, 1)
        || launch.max_flat_workgroup_size() != 64
        || launch.static_shared_memory_bytes()
            != fe2o3_kernel_ir::TILED_GEMM_LDS_V1_STATIC_LDS_BYTES
        || launch.max_dynamic_shared_memory_bytes() != 0
    {
        return Err(CollectedLdsSlice1ErrorV1::Descriptor(
            "compiler descriptor is not exact WG64/grid1/1024-static-LDS".to_owned(),
        ));
    }
    let capabilities = kernel
        .capabilities()
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let required_capabilities = [
        CapabilityV1::Subgroup,
        CapabilityV1::WorkgroupMemory,
        CapabilityV1::MatrixMultiply,
        CapabilityV1::AmdWave,
        CapabilityV1::AmdMfma,
    ]
    .into_iter()
    .collect::<std::collections::BTreeSet<_>>();
    if capabilities != required_capabilities {
        return Err(CollectedLdsSlice1ErrorV1::Descriptor(
            "subgroup/LDS/MFMA capability closure drifted".to_owned(),
        ));
    }
    let [a, b, c] = kernel.arguments() else {
        return Err(CollectedLdsSlice1ErrorV1::Descriptor(
            "descriptor does not contain exactly A, B, and C".to_owned(),
        ));
    };
    let inputs_exact = [a, b].into_iter().enumerate().all(|(index, argument)| {
        argument.source_index() == index as u16
            && argument.name().as_str() == format!("arg{index}")
            && argument.ownership() == OwnershipSemantics::SharedBorrow
            && argument.access() == AccessMode::ReadOnly
            && argument.alias() == AliasSemantics::SharedReadOnly
    });
    if !inputs_exact
        || c.source_index() != 2
        || c.name().as_str() != "arg2"
        || c.ownership() != OwnershipSemantics::UniqueBorrow
        || c.access() != AccessMode::ReadWrite
        || c.alias() != AliasSemantics::Exclusive
    {
        return Err(CollectedLdsSlice1ErrorV1::Descriptor(
            "A/B shared-readonly or C exclusive-readwrite role drifted".to_owned(),
        ));
    }
    Ok(())
}

fn resource_transcript(
    authority: &LdsSlice1AuthorityV1,
    resources: &FinalLdsSlice1ResourceMetadataV1,
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(320);
    append_transcript_field(&mut bytes, RESOURCE_TRANSCRIPT_DOMAIN_V1);
    append_transcript_field(&mut bytes, &authority.authority_commitment);
    append_transcript_field(&mut bytes, authority.target.as_bytes());
    append_transcript_field(&mut bytes, &authority.code_object_version.to_le_bytes());
    append_transcript_field(&mut bytes, &authority.canonical_ir_identity);
    append_transcript_field(&mut bytes, &authority.descriptor_source_identity);
    append_transcript_field(&mut bytes, &[64, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0]);
    append_transcript_field(
        &mut bytes,
        &resources
            .source_launch
            .static_shared_memory_bytes()
            .to_le_bytes(),
    );
    append_transcript_field(
        &mut bytes,
        &resources
            .compiler_launch
            .static_shared_memory_bytes()
            .to_le_bytes(),
    );
    append_transcript_field(&mut bytes, &resources.lds_allocations.to_le_bytes());
    append_transcript_field(
        &mut bytes,
        &resources.lds_bytes_per_allocation.to_le_bytes(),
    );
    append_transcript_field(&mut bytes, &resources.lds_alignment.to_le_bytes());
    bytes
}

fn append_transcript_field(bytes: &mut Vec<u8>, field: &[u8]) {
    bytes.extend_from_slice(&(field.len() as u64).to_le_bytes());
    bytes.extend_from_slice(field);
}

fn authority_identity(authority: &LdsSlice1AuthorityV1) -> [u8; 32] {
    let mut digest = Sha256::new();
    hash_field(&mut digest, AUTHORITY_DOMAIN_V1);
    hash_field(&mut digest, authority.target.as_bytes());
    hash_field(&mut digest, &authority.code_object_version.to_le_bytes());
    hash_field(&mut digest, &authority.explicit_kernarg_bytes.to_le_bytes());
    hash_field(&mut digest, &authority.complete_kernarg_bytes.to_le_bytes());
    hash_field(&mut digest, authority.root_instance_identity.as_bytes());
    hash_field(&mut digest, authority.kernel_export.as_bytes());
    hash_field(&mut digest, &authority.portable_mir_identity);
    hash_field(&mut digest, &authority.compiler_semantics_identity);
    hash_field(&mut digest, &authority.fn_abi_identity);
    hash_field(&mut digest, &authority.trusted_definitions_identity);
    hash_field(&mut digest, &authority.frontend_contract_identity);
    hash_field(&mut digest, &authority.abi_binding_identity);
    hash_field(&mut digest, &authority.source_geometry_identity);
    hash_field(&mut digest, &authority.derived_resource_identity);
    hash_field(&mut digest, &authority.correspondence_identity);
    hash_field(&mut digest, &authority.canonical_ir_identity);
    hash_field(&mut digest, &authority.descriptor_source_identity);
    digest.finalize().into()
}

fn validate_authority(authority: &LdsSlice1AuthorityV1) -> Result<(), CollectedLdsSlice1ErrorV1> {
    let field = if authority.target != crate::collected_tiled_gemm_v1::EXACT_TILED_GEMM_TARGET_V1 {
        Some("target")
    } else if authority.code_object_version != LDS_SLICE1_CODE_OBJECT_VERSION_V1 {
        Some("code object version")
    } else if authority.explicit_kernarg_bytes != LDS_SLICE1_EXPLICIT_KERNARG_BYTES_V1
        || authority.complete_kernarg_bytes != LDS_SLICE1_COMPLETE_KERNARG_BYTES_V1
    {
        Some("kernarg sizes")
    } else if authority.kernel_export != LDS_SLICE1_KERNEL_EXPORT_V1 {
        Some("kernel export")
    } else if !is_kernel_root_build_identity(&authority.root_instance_identity) {
        Some("root instance")
    } else if authority.portable_mir_identity != PORTABLE_MIR_SEMANTIC_IDENTITY_V1 {
        Some("portable MIR")
    } else if authority.compiler_semantics_identity
        != crate::collected_tiled_gemm_v1::reviewed_compiler_semantics_identity()
    {
        Some("compiler semantics")
    } else if authority.fn_abi_identity != RUSTC_FN_ABI_IDENTITY_V1 {
        Some("rustc FnAbi")
    } else if authority
        .trusted_definitions_identity
        .iter()
        .all(|byte| *byte == 0)
    {
        Some("trusted definitions")
    } else if authority.frontend_contract_identity != sha256(EXACT_FRONTEND_CONTRACT_V1) {
        Some("frontend contract")
    } else if authority.abi_binding_identity != sha256(ABI_BINDING_V1) {
        Some("ABI binding")
    } else if authority.source_geometry_identity != sha256(SOURCE_GEOMETRY_BINDING_V1) {
        Some("source geometry")
    } else if authority.derived_resource_identity != sha256(DERIVED_RESOURCE_BINDING_V1) {
        Some("compiler-derived LDS resources")
    } else if authority.correspondence_identity != sha256(CORRESPONDENCE_V1) {
        Some("reviewed correspondence")
    } else if authority.canonical_ir_identity != canonical_ir_identity(&tiled_gemm_lds_v1_module())?
    {
        Some("canonical Kernel IR")
    } else if authority.descriptor_source_identity == [0; 32] {
        Some("compiler descriptor source")
    } else if authority.authority_commitment != authority_identity(authority) {
        Some("source authority commitment")
    } else {
        None
    };
    if let Some(field) = field {
        return Err(CollectedLdsSlice1ErrorV1::ReceiptBinding(field));
    }
    Ok(())
}

#[cfg(test)]
fn exact_authority_for_test() -> LdsSlice1AuthorityV1 {
    let descriptor_source =
        crate::compiler_descriptor::tiled_gemm_lds_slice1_descriptor_source_for_test();
    let mut authority = LdsSlice1AuthorityV1 {
        target: crate::collected_tiled_gemm_v1::EXACT_TILED_GEMM_TARGET_V1.to_owned(),
        code_object_version: LDS_SLICE1_CODE_OBJECT_VERSION_V1,
        explicit_kernarg_bytes: LDS_SLICE1_EXPLICIT_KERNARG_BYTES_V1,
        complete_kernarg_bytes: LDS_SLICE1_COMPLETE_KERNARG_BYTES_V1,
        root_instance_identity: format!("__fe2o3_host_kernel_v1_{}", "1".repeat(64)),
        kernel_export: LDS_SLICE1_KERNEL_EXPORT_V1.to_owned(),
        portable_mir_identity: PORTABLE_MIR_SEMANTIC_IDENTITY_V1,
        compiler_semantics_identity:
            crate::collected_tiled_gemm_v1::reviewed_compiler_semantics_identity(),
        fn_abi_identity: RUSTC_FN_ABI_IDENTITY_V1,
        trusted_definitions_identity: sha256(b"measured trusted definitions"),
        frontend_contract_identity: sha256(EXACT_FRONTEND_CONTRACT_V1),
        abi_binding_identity: sha256(ABI_BINDING_V1),
        source_geometry_identity: sha256(SOURCE_GEOMETRY_BINDING_V1),
        derived_resource_identity: sha256(DERIVED_RESOURCE_BINDING_V1),
        correspondence_identity: sha256(CORRESPONDENCE_V1),
        canonical_ir_identity: canonical_ir_identity(&tiled_gemm_lds_v1_module()).unwrap(),
        descriptor_source_identity: *descriptor_source.identity().sha256(),
        authority_commitment: [0; 32],
    };
    authority.authority_commitment = authority_identity(&authority);
    authority
}

#[cfg(test)]
pub(crate) fn exact_lds_slice1_frontend_receipt_for_test() -> LdsSlice1FrontendReceiptV1 {
    let profile = TiledGemmLdsV1Profile::exact_gfx942_xnack_minus_cov6();
    let compiler_module =
        crate::kernel_ir_codegen::construct_inert_tiled_gemm_lds_slice1_module_text(
            &tiled_gemm_lds_v1_module(),
        )
        .unwrap();
    LdsSlice1FrontendReceiptV1 {
        authority: Some(exact_authority_for_test()),
        prepared_resources: Some(PreparedLdsSlice1ResourceMetadataV1 {
            source_launch: exact_source_launch().unwrap(),
            compiler_launch: exact_launch().unwrap(),
            lds_allocations: profile.lds_allocations,
            lds_bytes_per_allocation: profile.lds_bytes_per_allocation,
            lds_alignment: profile.lds_alignment,
        }),
        descriptor_source: Some(
            crate::compiler_descriptor::tiled_gemm_lds_slice1_descriptor_source_for_test(),
        ),
        compiler_module: Some(compiler_module),
    }
}

fn hash_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn encode_hex(bytes: &[u8; 32]) -> String {
    use fmt::Write as _;

    let mut encoded = String::with_capacity(64);
    for byte in bytes {
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exact_authority() -> LdsSlice1AuthorityV1 {
        exact_authority_for_test()
    }

    #[test]
    fn source_geometry_and_compiler_derived_lds_resources_are_distinct_and_bound() {
        let exact = exact_authority();
        assert!(validate_authority(&exact).is_ok());
        assert_ne!(
            exact.source_geometry_identity,
            exact.derived_resource_identity
        );

        let mut source_mismatch = exact_authority();
        source_mismatch.source_geometry_identity[0] ^= 1;
        assert!(matches!(
            validate_authority(&source_mismatch),
            Err(CollectedLdsSlice1ErrorV1::ReceiptBinding("source geometry"))
        ));

        let mut resource_mismatch = exact_authority();
        resource_mismatch.derived_resource_identity[0] ^= 1;
        assert!(matches!(
            validate_authority(&resource_mismatch),
            Err(CollectedLdsSlice1ErrorV1::ReceiptBinding(
                "compiler-derived LDS resources"
            ))
        ));

        let mut compiler_mismatch = exact_authority();
        compiler_mismatch.compiler_semantics_identity[0] ^= 1;
        assert!(matches!(
            validate_authority(&compiler_mismatch),
            Err(CollectedLdsSlice1ErrorV1::ReceiptBinding(
                "compiler semantics"
            ))
        ));
    }

    #[test]
    fn descriptor_resource_and_receipt_replay_mutations_fail_closed() {
        let mut descriptor_substitution = exact_lds_slice1_frontend_receipt_for_test();
        descriptor_substitution.descriptor_source =
            Some(crate::compiler_descriptor::tiled_gemm_v1_descriptor_source_for_test());
        assert!(matches!(
            descriptor_substitution.consume(),
            Err(CollectedLdsSlice1ErrorV1::ReceiptBinding(
                "compiler descriptor source"
            ))
        ));

        let mut compiler_module_substitution = exact_lds_slice1_frontend_receipt_for_test();
        compiler_module_substitution.compiler_module = Some(
            crate::kernel_ir_codegen::construct_inert_tiled_gemm_v1_module_text(
                &fe2o3_kernel_ir::tiled_gemm_v1_module(),
            )
            .unwrap(),
        );
        let authenticated = compiler_module_substitution.consume().unwrap();
        assert!(matches!(
            crate::worker_v2_producer::prepare_tiled_gemm_lds_slice1_worker_handoff(
                authenticated,
            ),
            Err(crate::worker_v2_producer::WorkerV2ProducerError::CompilerDescriptor(
                crate::compiler_descriptor::CompilerDescriptorError::TiledGemmLdsSlice1DescriptorMismatch(
                    "pre-section LLVM evidence"
                )
            ))
        ));

        let mut resource_drift = exact_lds_slice1_frontend_receipt_for_test();
        resource_drift
            .prepared_resources
            .as_mut()
            .unwrap()
            .compiler_launch = exact_source_launch().unwrap();
        assert!(matches!(
            resource_drift.consume(),
            Err(CollectedLdsSlice1ErrorV1::ReceiptBinding(
                "compiler-derived LDS resources"
            ))
        ));

        let mut replay = exact_lds_slice1_frontend_receipt_for_test();
        replay.consume().expect("first receipt consumption");
        assert!(matches!(
            replay.consume(),
            Err(CollectedLdsSlice1ErrorV1::ReceiptAlreadyConsumed)
        ));
    }
}
