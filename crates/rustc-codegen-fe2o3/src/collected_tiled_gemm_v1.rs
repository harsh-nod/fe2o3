//! Source-authenticated admission for the fixed tiled GEMM V1 profile.
//!
//! This checkpoint authenticates one exact collected rustc root and mints a
//! private receipt whose consumption selects one canonical Kernel IR module.
//! Neither the receipt nor that selection grants executable authority.

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
use fe2o3_kernel_ir::{Module, tiled_gemm_v1_module};
use rustc_abi::{CanonAbi, ExternAbi};
use rustc_hir::{Mutability, Safety};
use rustc_middle::ty::{FloatTy, Instance, InstanceKind, Ty, TyCtxt, TyKind, TypingEnv, UintTy};
use rustc_target::callconv::{ArgAttributes, ArgExtension, PassMode};
use sha2::{Digest as _, Sha256};

use crate::AmdGpuTarget;
use crate::collector::{
    CollectedFunction, CollectedFunctionRole, CollectionResult, TypedKernelProfile,
};
use crate::rust_type_layout_v3::GeneralTypedArgumentKindV3;
use crate::trusted_device_items::{self, TrustedDeviceItem};

pub(crate) const COLLECTED_TILED_GEMM_PIPELINE_V1: &str = "collected-tiled-gemm-v1";
pub(crate) const EXACT_TILED_GEMM_TARGET_V1: &str = "gfx942:xnack-";
pub(crate) const TILED_GEMM_CODE_OBJECT_VERSION_V1: u16 = 6;
pub(crate) const TILED_GEMM_EXPLICIT_KERNARG_BYTES_V1: u64 = 64;
pub(crate) const TILED_GEMM_COMPLETE_KERNARG_BYTES_V1: u64 = 320;
pub(crate) const TILED_GEMM_KERNEL_SYMBOL_V1: &str = "tiled_gemm_v1";

const FIXED_KERNEL_EXPORT: &str = TILED_GEMM_KERNEL_SYMBOL_V1;
const FIXED_LOGICAL_NAME: &str = TILED_GEMM_KERNEL_SYMBOL_V1;
const REVIEWED_ROOT_INSTANCE_IDENTITY: &str =
    "__fe2o3_host_kernel_v1_e81f6647397a26ed285264c5197ea93db7dc3b50fd9e0b635ebb4a988916250e";
const REVIEWED_RUSTC_RELEASE: &str = "1.96.0-nightly";
const REVIEWED_RUSTC_COMMIT: &str = "55e86c996809902e8bbad512cfb4d2c18be446d9";
const REVIEWED_RUSTC_LLVM: &str = "22.1.2";
const REVIEWED_CRATE_METADATA: &str = "fe2o3-tiled-gemm-v1-reviewed";
const REVIEWED_CARGO_CRATE_METADATA: &str = "4ceb166423714bdc";
const COMPILER_SEMANTICS_DOMAIN_V1: &[u8] = b"fe2o3.tiled-gemm.compiler-semantics.v1";
const COLLECTED_AUTHORITY_DOMAIN_V1: &[u8] = b"fe2o3.tiled-gemm.collected-authority.v1";
const ABI_BINDING_DOMAIN_V1: &[u8] = b"fe2o3.tiled-gemm.abi-binding.v1";
const FN_ABI_BINDING_DOMAIN_V1: &[u8] = b"fe2o3.tiled-gemm.rustc-fn-abi.v1";
const LAUNCH_BINDING_DOMAIN_V1: &[u8] = b"fe2o3.tiled-gemm.launch-binding.v1";
const CORRESPONDENCE_DOMAIN_V1: &[u8] = b"fe2o3.tiled-gemm.reviewed-correspondence.v1";
const EXACT_ABI_BINDING_V1: &[u8] = b"ptr64;size=64;align=8;a@0:16:8:slice-u16:shared-readonly:bfloat16-bit-carrier;b@16:16:8:slice-u16:shared-readonly:bfloat16-bit-carrier;c@32:16:8:slice-f32:shared-readonly;d@48:16:8:slice-f32:exclusive-readwrite";
const EXACT_LAUNCH_BINDING_V1: &[u8] =
    b"rank=1;block=exact(64,1,1);max-grid=(1,1,1);static-shared=0;dynamic-shared=0;wave=64;cov=6";
const REVIEWED_CORRESPONDENCE_V1: &[u8] = b"exact reviewed Rust portable-MIR identity selects fe2o3::tiled_gemm_v1;canonical one-wave mapping;bounded reviewed correspondence only;not a compiler-refinement proof";
const EXACT_FRONTEND_CONTRACT_V1: &[u8] = &[
    70, 69, 50, 79, 51, 75, 70, 0, 1, 0, 1, 0, 52, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 64, 0, 0, 0, 1,
    0, 0, 0, 1, 0, 0, 0, 64, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0,
];

// Reviewed from the exact fixture through path-independent portable-MIR
// collection under the compiler-semantics profile below.
const PORTABLE_MIR_SEMANTIC_IDENTITY: [u8; 32] = [
    0x48, 0xdf, 0x32, 0xb6, 0x08, 0xf5, 0xda, 0xfa, 0x30, 0x0f, 0x35, 0xd1, 0x86, 0x41, 0xb6, 0x57,
    0xf7, 0x75, 0x83, 0x65, 0x79, 0x11, 0x20, 0xd6, 0x49, 0x49, 0x5b, 0x1a, 0xea, 0x72, 0xdf, 0xe8,
];

const RUSTC_FN_ABI_IDENTITY: [u8; 32] = [
    0xa0, 0x09, 0x7f, 0x66, 0xc3, 0xad, 0xcf, 0x8b, 0x68, 0x1a, 0xb2, 0x70, 0x20, 0xc1, 0xca, 0x29,
    0x42, 0x1c, 0x68, 0x59, 0x8d, 0xd7, 0x50, 0xbe, 0xf3, 0x7e, 0xad, 0xdc, 0x8f, 0x55, 0x56, 0xb4,
];

const ARGUMENT_KINDS: [GeneralTypedArgumentKindV3; 4] = [
    GeneralTypedArgumentKindV3::SharedSlice(RustScalarElementTypeV1::U16),
    GeneralTypedArgumentKindV3::SharedSlice(RustScalarElementTypeV1::U16),
    GeneralTypedArgumentKindV3::SharedSlice(RustScalarElementTypeV1::F32),
    GeneralTypedArgumentKindV3::DisjointSlice(RustScalarElementTypeV1::F32),
];

#[derive(Clone, Debug, Eq, PartialEq)]
struct CompilerSemanticsV1 {
    rustc_release: &'static str,
    rustc_commit: &'static str,
    llvm_version: &'static str,
    panic_strategy: String,
    overflow_checks: bool,
    optimize: String,
    debug_assertions: bool,
    mir_opt_level: usize,
    mir_enable_passes: Vec<(String, bool)>,
    llvm_args: Vec<String>,
    llvm_passes: Vec<String>,
    target_cpu: Option<String>,
    target_features: String,
    rustc_codegen_opt_level: String,
    crate_metadata: Vec<String>,
    remap_path_destinations: Vec<String>,
}

#[derive(Debug, Eq, PartialEq)]
struct TiledGemmFrontendAuthorityV1 {
    target: String,
    code_object_version: u16,
    explicit_kernarg_bytes: u64,
    complete_kernarg_bytes: u64,
    abi_binding_commitment: [u8; 32],
    fn_abi_binding_commitment: [u8; 32],
    launch_binding_commitment: [u8; 32],
    correspondence_commitment: [u8; 32],
    frontend_contract_commitment: [u8; 32],
    kernel_export: String,
    root_instance_identity: String,
    portable_mir_semantic_commitment: [u8; 32],
    compiler_semantics_commitment: [u8; 32],
    descriptor_source_commitment: [u8; 32],
    authority_commitment: [u8; 32],
}

/// Opaque, single-use authority produced only by exact rustc admission.
///
/// The private state and absence of `Clone`/`Copy` prevent downstream code from
/// manufacturing authority from a copied digest. Consumption additionally
/// invalidates the value so an accidental replay fails closed at runtime.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct TiledGemmFrontendReceiptV1 {
    authority: Option<TiledGemmFrontendAuthorityV1>,
    descriptor_source: Option<CompilerDescriptorSourceV1>,
}

impl TiledGemmFrontendReceiptV1 {
    pub(crate) fn kernel_export(&self) -> &str {
        &self
            .authority
            .as_ref()
            .expect("unconsumed tiled GEMM receipt")
            .kernel_export
    }

    pub(crate) fn root_instance_identity(&self) -> &str {
        &self
            .authority
            .as_ref()
            .expect("unconsumed tiled GEMM receipt")
            .root_instance_identity
    }

    pub(crate) fn portable_mir_semantic_hex(&self) -> String {
        encode_hex(
            &self
                .authority
                .as_ref()
                .expect("unconsumed tiled GEMM receipt")
                .portable_mir_semantic_commitment,
        )
    }

    pub(crate) fn compiler_semantics_hex(&self) -> String {
        encode_hex(
            &self
                .authority
                .as_ref()
                .expect("unconsumed tiled GEMM receipt")
                .compiler_semantics_commitment,
        )
    }

    pub(crate) fn authority_hex(&self) -> String {
        encode_hex(
            &self
                .authority
                .as_ref()
                .expect("unconsumed tiled GEMM receipt")
                .authority_commitment,
        )
    }

    pub(crate) fn authority_commitment(&self) -> &[u8; 32] {
        &self
            .authority
            .as_ref()
            .expect("unconsumed tiled GEMM receipt")
            .authority_commitment
    }

    pub(crate) fn consume(
        &mut self,
    ) -> Result<AuthenticatedTiledGemmModuleV1, CollectedTiledGemmErrorV1> {
        let authority = self
            .authority
            .take()
            .ok_or(CollectedTiledGemmErrorV1::ReceiptAlreadyConsumed)?;
        let descriptor_source = self
            .descriptor_source
            .take()
            .ok_or(CollectedTiledGemmErrorV1::ReceiptAlreadyConsumed)?;
        validate_frontend_authority(&authority)?;
        if descriptor_source.identity().sha256() != &authority.descriptor_source_commitment {
            return Err(CollectedTiledGemmErrorV1::ReceiptBindingMismatch {
                field: "descriptor source",
            });
        }
        Ok(AuthenticatedTiledGemmModuleV1 {
            module: tiled_gemm_v1_module(),
            descriptor_source,
            authority_commitment: authority.authority_commitment,
        })
    }
}

/// Canonical Kernel IR selected by consuming an unforgeable frontend receipt.
///
/// Its fields remain private and the value is neither cloneable nor publicly
/// constructible. The Worker V2 preparation path consumes it by value.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedTiledGemmModuleV1 {
    module: Module,
    descriptor_source: CompilerDescriptorSourceV1,
    authority_commitment: [u8; 32],
}

impl AuthenticatedTiledGemmModuleV1 {
    pub(crate) const fn authority_commitment(&self) -> &[u8; 32] {
        &self.authority_commitment
    }

    pub(crate) fn into_parts(self) -> (Module, CompilerDescriptorSourceV1) {
        (self.module, self.descriptor_source)
    }
}

#[derive(Debug)]
pub(crate) enum CollectedTiledGemmErrorV1 {
    WrongTarget {
        actual: String,
    },
    CustomPipeline,
    CompilerSemantics {
        detail: String,
    },
    UnsupportedCollection {
        detail: String,
    },
    AbiMismatch {
        detail: String,
    },
    LayoutMismatch {
        detail: String,
    },
    PortableMirIdentityMismatch {
        expected: [u8; 32],
        actual: [u8; 32],
    },
    FnAbiIdentityMismatch {
        expected: [u8; 32],
        actual: [u8; 32],
    },
    PortableMir {
        detail: String,
    },
    ReceiptAlreadyConsumed,
    ReceiptBindingMismatch {
        field: &'static str,
    },
}

impl fmt::Display for CollectedTiledGemmErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongTarget { actual } => write!(
                formatter,
                "collected tiled GEMM V1 requires exact target `{EXACT_TILED_GEMM_TARGET_V1}`, found `{actual}`"
            ),
            Self::CustomPipeline => formatter
                .write_str("collected tiled GEMM V1 rejects custom LLVM pipeline selection"),
            Self::CompilerSemantics { detail } => write!(
                formatter,
                "collected tiled GEMM V1 compiler semantics mismatch: {detail}"
            ),
            Self::UnsupportedCollection { detail } => write!(
                formatter,
                "unsupported collected tiled GEMM V1 shape: {detail}"
            ),
            Self::AbiMismatch { detail } => {
                write!(formatter, "collected tiled GEMM V1 ABI mismatch: {detail}")
            }
            Self::LayoutMismatch { detail } => write!(
                formatter,
                "collected tiled GEMM V1 typed layout mismatch: {detail}"
            ),
            Self::PortableMirIdentityMismatch { expected, actual } => write!(
                formatter,
                "collected tiled GEMM V1 portable MIR identity mismatch: expected {}, found {}",
                encode_hex(expected),
                encode_hex(actual)
            ),
            Self::FnAbiIdentityMismatch { expected, actual } => write!(
                formatter,
                "collected tiled GEMM V1 rustc FnAbi identity mismatch: expected {}, found {}",
                encode_hex(expected),
                encode_hex(actual)
            ),
            Self::PortableMir { detail } => write!(
                formatter,
                "collected tiled GEMM V1 portable MIR rejected: {detail}"
            ),
            Self::ReceiptAlreadyConsumed => {
                formatter.write_str("collected tiled GEMM V1 frontend receipt was already consumed")
            }
            Self::ReceiptBindingMismatch { field } => write!(
                formatter,
                "collected tiled GEMM V1 frontend receipt binding mismatch: {field}"
            ),
        }
    }
}

impl Error for CollectedTiledGemmErrorV1 {}

pub(crate) fn authenticate_collected_tiled_gemm_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    collection: &CollectionResult<'tcx>,
    target: &AmdGpuTarget,
    custom_llvm_pipeline: bool,
) -> Result<TiledGemmFrontendReceiptV1, CollectedTiledGemmErrorV1> {
    admit_execution_context(target.as_str(), custom_llvm_pipeline)?;
    let compiler_semantics = observe_compiler_semantics(tcx);
    let compiler_semantics_commitment = require_compiler_semantics(&compiler_semantics)?;
    let root = exact_collected_root(&collection.functions)?;
    require_registration(root)?;
    require_signature(tcx, root.instance)?;
    require_layout(root)?;
    let fn_abi_binding_commitment = require_rustc_fn_abi(tcx, root.instance)?;

    let contract = root
        .general_typed_contract
        .as_ref()
        .ok_or_else(|| layout_mismatch("General V3 contract is absent after layout admission"))?;
    let target_identity = tiled_gemm_target_identity()?;
    let launch = exact_tiled_launch_contract()?;
    let imported = crate::mir_import::import_collection(tcx, collection).map_err(|error| {
        CollectedTiledGemmErrorV1::PortableMir {
            detail: error.to_string(),
        }
    })?;
    let portable_mir_semantic_commitment = imported
        .portable_semantic_digest_v2(crate::mir_import::MirSemanticAdmissionInputsV2::new(
            FIXED_KERNEL_EXPORT,
            &target_identity,
            contract.abi(),
            &launch,
        ))
        .map_err(|error| CollectedTiledGemmErrorV1::PortableMir {
            detail: error.to_string(),
        })?;
    let portable_mir_semantic_commitment = *portable_mir_semantic_commitment.as_bytes();
    if portable_mir_semantic_commitment != PORTABLE_MIR_SEMANTIC_IDENTITY {
        return Err(CollectedTiledGemmErrorV1::PortableMirIdentityMismatch {
            expected: PORTABLE_MIR_SEMANTIC_IDENTITY,
            actual: portable_mir_semantic_commitment,
        });
    }

    let root_instance_identity = tcx.def_path_str(root.instance.def_id());
    if root_instance_identity != REVIEWED_ROOT_INSTANCE_IDENTITY {
        return Err(unsupported_collection(format!(
            "root instance must be exactly `{REVIEWED_ROOT_INSTANCE_IDENTITY}`, found `{root_instance_identity}`"
        )));
    }

    let descriptor_roots = crate::compiler_descriptor::typed_descriptor_roots_from_collection(
        tcx,
        &collection.functions,
    )
    .map_err(|error| layout_mismatch(format!("descriptor evidence rejected: {error}")))?;
    let module = tiled_gemm_v1_module();
    let compiler_module = crate::kernel_ir_codegen::construct_inert_tiled_gemm_v1_module_text(
        &module,
    )
    .map_err(|error| unsupported_collection(format!("exact LLVM lowering failed: {error}")))?;
    let target = DeviceTargetV1::parse(EXACT_TILED_GEMM_TARGET_V1)
        .expect("fixed tiled GEMM target is valid");
    let envelope =
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V6)
            .map_err(|error| {
                unsupported_collection(format!("compiler envelope failed: {error}"))
            })?;
    let descriptor_source =
        crate::compiler_descriptor::construct_tiled_gemm_v1_compiler_descriptor_source_v1(
            &envelope,
            &module,
            &compiler_module,
            &descriptor_roots,
        )
        .map_err(|error| layout_mismatch(format!("descriptor source rejected: {error}")))?
        .ok_or_else(|| layout_mismatch("compiler descriptor source is absent"))?;
    let descriptor_source_commitment = *descriptor_source.identity().sha256();
    let abi_binding_commitment = exact_abi_binding_commitment();
    let launch_binding_commitment = exact_launch_binding_commitment();
    let correspondence_commitment = reviewed_correspondence_commitment();
    let frontend_contract_commitment = sha256(
        root.frontend_contract
            .as_ref()
            .expect("registration admission requires frontend contract")
            .canonical_bytes(),
    );
    let mut authority = TiledGemmFrontendAuthorityV1 {
        target: EXACT_TILED_GEMM_TARGET_V1.to_owned(),
        code_object_version: TILED_GEMM_CODE_OBJECT_VERSION_V1,
        explicit_kernarg_bytes: TILED_GEMM_EXPLICIT_KERNARG_BYTES_V1,
        complete_kernarg_bytes: TILED_GEMM_COMPLETE_KERNARG_BYTES_V1,
        abi_binding_commitment,
        fn_abi_binding_commitment,
        launch_binding_commitment,
        correspondence_commitment,
        frontend_contract_commitment,
        kernel_export: root.export_name.clone(),
        root_instance_identity,
        portable_mir_semantic_commitment,
        compiler_semantics_commitment,
        descriptor_source_commitment,
        authority_commitment: [0; 32],
    };
    authority.authority_commitment = collected_authority_commitment(&authority);
    Ok(TiledGemmFrontendReceiptV1 {
        authority: Some(authority),
        descriptor_source: Some(descriptor_source),
    })
}

fn admit_execution_context(
    target: &str,
    custom_llvm_pipeline: bool,
) -> Result<(), CollectedTiledGemmErrorV1> {
    if target != EXACT_TILED_GEMM_TARGET_V1 {
        return Err(CollectedTiledGemmErrorV1::WrongTarget {
            actual: target.to_owned(),
        });
    }
    if custom_llvm_pipeline {
        return Err(CollectedTiledGemmErrorV1::CustomPipeline);
    }
    Ok(())
}

fn exact_collected_root<'a, 'tcx>(
    functions: &'a [CollectedFunction<'tcx>],
) -> Result<&'a CollectedFunction<'tcx>, CollectedTiledGemmErrorV1> {
    if functions.len() != 1 {
        return Err(unsupported_collection(format!(
            "requires exactly one collected function and no helpers, FFI exports, or extra roots; found {}",
            functions.len()
        )));
    }
    let root = &functions[0];
    if root.role != CollectedFunctionRole::KernelEntry {
        return Err(unsupported_collection(format!(
            "the sole collected function must be KernelEntry, found {:?}",
            root.role
        )));
    }
    Ok(root)
}

fn require_registration(root: &CollectedFunction<'_>) -> Result<(), CollectedTiledGemmErrorV1> {
    if root.export_name != FIXED_KERNEL_EXPORT
        || root.logical_name.as_deref() != Some(FIXED_LOGICAL_NAME)
        || !matches!(
            root.typed_profile,
            Some(TypedKernelProfile::GeneralScalarSliceRustcLayoutV3 { .. })
        )
        || root.kernel_binding.is_none()
        || root.frontend_contract.is_none()
    {
        return Err(unsupported_collection(
            "kernel registration must be the unique compiler-authenticated General V3 tiled_gemm_v1 root with its exact WG64 frontend contract",
        ));
    }
    Ok(())
}

fn require_signature<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<(), CollectedTiledGemmErrorV1> {
    if !matches!(instance.def, InstanceKind::Item(_)) || !instance.args.is_empty() {
        return Err(abi_mismatch(
            "kernel must be one nongeneric ordinary function item",
        ));
    }
    let signature = tcx
        .try_instantiate_and_normalize_erasing_regions(
            instance.args,
            TypingEnv::fully_monomorphized(),
            tcx.fn_sig(instance.def_id()),
        )
        .map_err(|_| abi_mismatch("signature normalization failed"))?;
    let signature = tcx.instantiate_bound_regions_with_erased(signature);
    if signature.safety != Safety::Safe
        || signature.abi != ExternAbi::Rust
        || signature.c_variadic
        || signature.output() != tcx.types.unit
        || signature.inputs().len() != 4
    {
        return Err(abi_mismatch(format!(
            "expected safe non-variadic Rust ABI `(&[u16], &[u16], &[f32], DisjointSlice<f32>) -> ()`, found `{signature}`"
        )));
    }
    let inputs = signature.inputs();
    if !is_shared_u16_slice(inputs[0])
        || !is_shared_u16_slice(inputs[1])
        || !is_shared_f32_slice(inputs[2])
        || !is_disjoint_f32_slice(tcx, inputs[3])
    {
        return Err(abi_mismatch(format!(
            "expected exact argument order `A:&[u16], B:&[u16], C:&[f32], D:DisjointSlice<f32>`, found `{signature}`"
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

fn is_shared_f32_slice(ty: Ty<'_>) -> bool {
    matches!(
        ty.kind(),
        TyKind::Ref(_, pointee, Mutability::Not)
            if matches!(pointee.kind(), TyKind::Slice(element) if matches!(element.kind(), TyKind::Float(FloatTy::F32)))
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

fn require_layout(root: &CollectedFunction<'_>) -> Result<(), CollectedTiledGemmErrorV1> {
    let identities = root.typed_layout_identities.as_ref().ok_or_else(|| {
        layout_mismatch("compiler-authenticated per-argument type identities are absent")
    })?;
    if identities.len() != ARGUMENT_KINDS.len() {
        return Err(layout_mismatch(format!(
            "expected {} argument identities, found {}",
            ARGUMENT_KINDS.len(),
            identities.len()
        )));
    }
    let contract = root
        .general_typed_contract
        .as_ref()
        .ok_or_else(|| layout_mismatch("General V3 contract is absent"))?;
    let actual = contract
        .arguments()
        .iter()
        .map(|argument| argument.kind())
        .collect::<Vec<_>>();
    if actual != ARGUMENT_KINDS {
        return Err(layout_mismatch(format!(
            "expected exact tiled GEMM argument kinds {ARGUMENT_KINDS:?}, found {actual:?}"
        )));
    }
    let abi = contract.abi();
    if abi.size() != TILED_GEMM_EXPLICIT_KERNARG_BYTES_V1
        || abi.alignment() != 8
        || abi.pointer_width() != PointerWidth::Bits64
    {
        return Err(layout_mismatch(format!(
            "explicit kernarg must be exactly 64-bit, 64 bytes aligned to 8, found {:?}, {} bytes aligned to {}",
            abi.pointer_width(),
            abi.size(),
            abi.alignment()
        )));
    }
    // General V3 deliberately uses positional ABI field names for profiles that
    // do not have a generated host adapter. The reviewed source signature and
    // the per-position rustc identities below bind these to A, B, C, and D.
    let expected_names = ["arg0", "arg1", "arg2", "arg3"];
    let expected_offsets = [0, 16, 32, 48];
    let expected_sizes = [16; 4];
    let expected_alignments = [8; 4];
    if abi.fields().len() != expected_names.len() {
        return Err(layout_mismatch(format!(
            "expected four ABI fields, found {}",
            abi.fields().len()
        )));
    }
    for (index, field) in abi.fields().iter().enumerate() {
        if field.name().as_str() != expected_names[index]
            || field.offset() != expected_offsets[index]
            || field.size() != expected_sizes[index]
            || field.alignment() != expected_alignments[index]
            || field.type_identity() != contract.arguments()[index].type_identity()
        {
            return Err(layout_mismatch(format!(
                "field {index} must be {}@{} size {} align {} with its rustc-derived type identity, found {}@{} size {} align {}",
                expected_names[index],
                expected_offsets[index],
                expected_sizes[index],
                expected_alignments[index],
                field.name().as_str(),
                field.offset(),
                field.size(),
                field.alignment(),
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
                    return Err(layout_mismatch(format!(
                        "field {} must be an immutable shared &[u16] global slice used only as a bit-preserving BF16 carrier",
                        expected_names[index]
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
                ) || field.mutability() != ArtifactMutability::Immutable
                    || field.access() != Access::ReadOnly
                    || field.address_space() != AddressSpace::Global
                    || field.ownership() != ArgumentOwnership::SharedBorrow
                    || field.alias_class() != AliasClass::SharedReadOnly
                {
                    return Err(layout_mismatch(
                        "field c must be an immutable shared &[f32] global slice",
                    ));
                }
            }
            3 => {
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
                    return Err(layout_mismatch(
                        "field d must be the unique genuine DisjointSlice<f32> global slice",
                    ));
                }
            }
            _ => unreachable!(),
        }
    }
    // General typed V3 currently transports rustc layout evidence under its
    // legacy 256-thread generated-host contract. It is not the execution
    // launch authority for this profile. The separately authenticated source
    // frontend contract below is the exact WG64 launch policy committed into
    // the receipt and portable-MIR identity.
    let transport_launch = contract.launch();
    if transport_launch.rank() != 1
        || transport_launch.block_size()
            != BlockSize::Exact(Dimensions::new(256, 1, 1).map_err(|error| {
                layout_mismatch(format!("invalid fixed workgroup dimensions: {error}"))
            })?)
        || transport_launch.max_grid()
            != Dimensions::new(u32::MAX, 1, 1).map_err(|error| {
                layout_mismatch(format!("invalid fixed grid dimensions: {error}"))
            })?
        || transport_launch.static_shared_memory_bytes() != 0
        || transport_launch.max_dynamic_shared_memory_bytes() != 0
    {
        return Err(layout_mismatch(
            "general V3 layout transport contract drifted from its exact 256x1x1 profile",
        ));
    }
    let frontend = root
        .frontend_contract
        .as_ref()
        .ok_or_else(|| layout_mismatch("exact WG64 frontend contract is absent"))?;
    if frontend.canonical_bytes() != EXACT_FRONTEND_CONTRACT_V1 {
        return Err(layout_mismatch(
            "frontend contract bytes do not match exact required=max=64x1x1 policy",
        ));
    }
    let frontend_contract = frontend.contract();
    let launch = frontend_contract
        .launch()
        .ok_or_else(|| layout_mismatch("frontend launch declaration is absent"))?;
    if launch.required().map(|value| value.as_array()) != Some([64, 1, 1])
        || launch.maximum().map(|value| value.as_array()) != Some([64, 1, 1])
        || launch.min_workgroups_per_compute_unit().is_some()
        || frontend_contract.unsafe_assembly().is_some()
    {
        return Err(layout_mismatch(
            "frontend contract must be exact required=max=64x1x1 with no occupancy or unsafe assembly declaration",
        ));
    }
    Ok(())
}

fn exact_tiled_launch_contract() -> Result<LaunchContract, CollectedTiledGemmErrorV1> {
    LaunchContract::new(
        1,
        BlockSize::Exact(
            Dimensions::new(64, 1, 1)
                .map_err(|error| layout_mismatch(format!("invalid WG64 dimensions: {error}")))?,
        ),
        Dimensions::new(1, 1, 1)
            .map_err(|error| layout_mismatch(format!("invalid one-tile grid: {error}")))?,
        0,
        0,
    )
    .map_err(|error| layout_mismatch(format!("invalid exact tiled launch: {error}")))
}

fn require_rustc_fn_abi<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<[u8; 32], CollectedTiledGemmErrorV1> {
    let query = TypingEnv::fully_monomorphized()
        .as_query_input((instance, rustc_middle::ty::List::empty()));
    let abi = tcx
        .fn_abi_of_instance(query)
        .map_err(|error| abi_mismatch(format!("rustc FnAbi query failed: {error:?}")))?;
    if abi.conv != CanonAbi::Rust
        || abi.c_variadic
        || abi.fixed_count != 4
        || abi.args.len() != 4
        || !matches!(abi.ret.mode, PassMode::Ignore)
        || abi.ret.layout.size.bytes() != 0
    {
        return Err(abi_mismatch(format!(
            "rustc FnAbi header must be exact Rust(args=4)->unit, found {abi:?}"
        )));
    }
    for (index, argument) in abi.args.iter().enumerate() {
        if argument.layout.size.bytes() != 16
            || argument.layout.align.abi.bytes() != 8
            || !matches!(argument.mode, PassMode::Pair(_, _))
        {
            return Err(abi_mismatch(format!(
                "rustc FnAbi argument {index} must be Pair(size=16, align=8), found {argument:?}"
            )));
        }
    }

    let mut digest = Sha256::new();
    hash_field(&mut digest, FN_ABI_BINDING_DOMAIN_V1);
    hash_field(&mut digest, &[u8::from(abi.c_variadic)]);
    hash_field(&mut digest, &abi.fixed_count.to_le_bytes());
    hash_field(&mut digest, &[u8::from(abi.can_unwind)]);
    for argument in abi.args.iter() {
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
    if actual != RUSTC_FN_ABI_IDENTITY {
        return Err(CollectedTiledGemmErrorV1::FnAbiIdentityMismatch {
            expected: RUSTC_FN_ABI_IDENTITY,
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

fn observe_compiler_semantics(tcx: TyCtxt<'_>) -> CompilerSemanticsV1 {
    CompilerSemanticsV1 {
        rustc_release: env!("FE2O3_BUILD_RUSTC_RELEASE"),
        rustc_commit: env!("FE2O3_BUILD_RUSTC_COMMIT"),
        llvm_version: env!("FE2O3_BUILD_RUSTC_LLVM"),
        panic_strategy: format!("{:?}", tcx.sess.panic_strategy()),
        overflow_checks: tcx.sess.overflow_checks(),
        optimize: format!("{:?}", tcx.sess.opts.optimize),
        debug_assertions: tcx.sess.opts.debug_assertions,
        mir_opt_level: tcx.sess.mir_opt_level(),
        mir_enable_passes: tcx.sess.opts.unstable_opts.mir_enable_passes.clone(),
        llvm_args: tcx.sess.opts.cg.llvm_args.clone(),
        llvm_passes: tcx.sess.opts.cg.passes.clone(),
        target_cpu: tcx.sess.opts.cg.target_cpu.clone(),
        target_features: tcx.sess.opts.cg.target_feature.clone(),
        rustc_codegen_opt_level: tcx.sess.opts.cg.opt_level.clone(),
        crate_metadata: tcx.sess.opts.cg.metadata.clone(),
        remap_path_destinations: tcx
            .sess
            .opts
            .remap_path_prefix
            .iter()
            .map(|(_, destination)| destination.display().to_string())
            .collect(),
    }
}

fn require_compiler_semantics(
    observed: &CompilerSemanticsV1,
) -> Result<[u8; 32], CollectedTiledGemmErrorV1> {
    let expected_mir_passes = [("JumpThreading".to_owned(), false)];
    let mismatch = if observed.rustc_release != REVIEWED_RUSTC_RELEASE {
        Some(format!(
            "rustc release must be {REVIEWED_RUSTC_RELEASE}, found {}",
            observed.rustc_release
        ))
    } else if observed.rustc_commit != REVIEWED_RUSTC_COMMIT {
        Some(format!(
            "rustc commit must be {REVIEWED_RUSTC_COMMIT}, found {}",
            observed.rustc_commit
        ))
    } else if observed.llvm_version != REVIEWED_RUSTC_LLVM {
        Some(format!(
            "rustc LLVM must be {REVIEWED_RUSTC_LLVM}, found {}",
            observed.llvm_version
        ))
    } else if observed.panic_strategy != "Unwind" {
        Some(format!(
            "panic strategy must be Unwind, found {}",
            observed.panic_strategy
        ))
    } else if observed.overflow_checks {
        Some("overflow checks must be disabled".to_owned())
    } else if observed.optimize != "No" || observed.rustc_codegen_opt_level != "0" {
        Some(format!(
            "rustc optimization must be No/0, found {}/{}",
            observed.optimize, observed.rustc_codegen_opt_level
        ))
    } else if !observed.debug_assertions {
        Some("debug assertions must be enabled".to_owned())
    } else if observed.mir_opt_level != 1 {
        Some(format!(
            "effective MIR optimization level must be 1, found {}",
            observed.mir_opt_level
        ))
    } else if observed.mir_enable_passes != expected_mir_passes {
        Some(format!(
            "MIR pass overrides must be exactly -JumpThreading, found {:?}",
            observed.mir_enable_passes
        ))
    } else if !observed.llvm_args.is_empty() || !observed.llvm_passes.is_empty() {
        Some("custom LLVM arguments or passes are forbidden".to_owned())
    } else if observed.target_cpu.is_some() || !observed.target_features.is_empty() {
        Some(format!(
            "rustc target CPU/features must be unset, found {:?}/{:?}",
            observed.target_cpu, observed.target_features
        ))
    } else if observed.crate_metadata != [REVIEWED_CARGO_CRATE_METADATA, REVIEWED_CRATE_METADATA] {
        Some(format!(
            "crate metadata must be exactly the Cargo fixture identity and {REVIEWED_CRATE_METADATA:?}, found {:?}",
            observed.crate_metadata
        ))
    } else if observed.remap_path_destinations != ["/fe2o3-reviewed-workspace/tiled-gemm-v1.rs"] {
        Some(format!(
            "source remapping must contain exactly the canonical tiled GEMM fixture destination, found {:?}",
            observed.remap_path_destinations
        ))
    } else {
        None
    };
    if let Some(detail) = mismatch {
        return Err(CollectedTiledGemmErrorV1::CompilerSemantics { detail });
    }

    Ok(compiler_semantics_commitment(observed))
}

fn compiler_semantics_commitment(observed: &CompilerSemanticsV1) -> [u8; 32] {
    let mut digest = Sha256::new();
    hash_field(&mut digest, COMPILER_SEMANTICS_DOMAIN_V1);
    hash_field(&mut digest, observed.rustc_release.as_bytes());
    hash_field(&mut digest, observed.rustc_commit.as_bytes());
    hash_field(&mut digest, observed.llvm_version.as_bytes());
    hash_field(&mut digest, observed.panic_strategy.as_bytes());
    hash_field(&mut digest, &[u8::from(observed.overflow_checks)]);
    hash_field(&mut digest, observed.optimize.as_bytes());
    hash_field(&mut digest, &[u8::from(observed.debug_assertions)]);
    hash_field(&mut digest, &(observed.mir_opt_level as u64).to_le_bytes());
    for (name, enabled) in &observed.mir_enable_passes {
        hash_field(&mut digest, name.as_bytes());
        hash_field(&mut digest, &[u8::from(*enabled)]);
    }
    for argument in &observed.llvm_args {
        hash_field(&mut digest, argument.as_bytes());
    }
    for pass in &observed.llvm_passes {
        hash_field(&mut digest, pass.as_bytes());
    }
    match &observed.target_cpu {
        Some(cpu) => {
            hash_field(&mut digest, &[1]);
            hash_field(&mut digest, cpu.as_bytes());
        }
        None => hash_field(&mut digest, &[0]),
    }
    hash_field(&mut digest, observed.target_features.as_bytes());
    hash_field(&mut digest, observed.rustc_codegen_opt_level.as_bytes());
    for metadata in &observed.crate_metadata {
        hash_field(&mut digest, metadata.as_bytes());
    }
    for destination in &observed.remap_path_destinations {
        hash_field(&mut digest, destination.as_bytes());
    }
    digest.finalize().into()
}

fn reviewed_compiler_semantics() -> CompilerSemanticsV1 {
    CompilerSemanticsV1 {
        rustc_release: REVIEWED_RUSTC_RELEASE,
        rustc_commit: REVIEWED_RUSTC_COMMIT,
        llvm_version: REVIEWED_RUSTC_LLVM,
        panic_strategy: "Unwind".to_owned(),
        overflow_checks: false,
        optimize: "No".to_owned(),
        debug_assertions: true,
        mir_opt_level: 1,
        mir_enable_passes: vec![("JumpThreading".to_owned(), false)],
        llvm_args: Vec::new(),
        llvm_passes: Vec::new(),
        target_cpu: None,
        target_features: String::new(),
        rustc_codegen_opt_level: "0".to_owned(),
        crate_metadata: vec![
            REVIEWED_CARGO_CRATE_METADATA.to_owned(),
            REVIEWED_CRATE_METADATA.to_owned(),
        ],
        remap_path_destinations: vec!["/fe2o3-reviewed-workspace/tiled-gemm-v1.rs".to_owned()],
    }
}

fn tiled_gemm_target_identity() -> Result<TargetIdentity, CollectedTiledGemmErrorV1> {
    TargetIdentity::new(
        IdentityText::new(dialect_amdgcn::AMDGPU_TRIPLE)
            .map_err(|error| unsupported_collection(format!("invalid AMDGPU triple: {error}")))?,
        IdentityText::new(EXACT_TILED_GEMM_TARGET_V1)
            .map_err(|error| unsupported_collection(format!("invalid gfx942 profile: {error}")))?,
        PointerWidth::Bits64,
        Endianness::Little,
        vec![
            Capability::MatrixMultiply,
            Capability::AmdWave,
            Capability::AmdMfma,
        ],
    )
    .map_err(|error| unsupported_collection(format!("invalid target identity: {error}")))
}

fn collected_authority_commitment(authority: &TiledGemmFrontendAuthorityV1) -> [u8; 32] {
    let mut digest = Sha256::new();
    hash_field(&mut digest, COLLECTED_AUTHORITY_DOMAIN_V1);
    hash_field(&mut digest, &authority.portable_mir_semantic_commitment);
    hash_field(&mut digest, &authority.compiler_semantics_commitment);
    hash_field(&mut digest, &authority.descriptor_source_commitment);
    hash_field(&mut digest, authority.root_instance_identity.as_bytes());
    hash_field(&mut digest, authority.kernel_export.as_bytes());
    hash_field(&mut digest, authority.target.as_bytes());
    hash_field(&mut digest, &authority.code_object_version.to_le_bytes());
    hash_field(&mut digest, &authority.explicit_kernarg_bytes.to_le_bytes());
    hash_field(&mut digest, &authority.complete_kernarg_bytes.to_le_bytes());
    hash_field(&mut digest, &authority.abi_binding_commitment);
    hash_field(&mut digest, &authority.fn_abi_binding_commitment);
    hash_field(&mut digest, &authority.launch_binding_commitment);
    hash_field(&mut digest, &authority.correspondence_commitment);
    hash_field(&mut digest, &authority.frontend_contract_commitment);
    digest.finalize().into()
}

fn exact_abi_binding_commitment() -> [u8; 32] {
    domain_commitment(ABI_BINDING_DOMAIN_V1, EXACT_ABI_BINDING_V1)
}

fn exact_launch_binding_commitment() -> [u8; 32] {
    domain_commitment(LAUNCH_BINDING_DOMAIN_V1, EXACT_LAUNCH_BINDING_V1)
}

fn reviewed_correspondence_commitment() -> [u8; 32] {
    domain_commitment(CORRESPONDENCE_DOMAIN_V1, REVIEWED_CORRESPONDENCE_V1)
}

fn domain_commitment(domain: &[u8], value: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    hash_field(&mut digest, domain);
    hash_field(&mut digest, value);
    digest.finalize().into()
}

fn validate_frontend_authority(
    authority: &TiledGemmFrontendAuthorityV1,
) -> Result<(), CollectedTiledGemmErrorV1> {
    let field = if authority.target != EXACT_TILED_GEMM_TARGET_V1 {
        Some("target")
    } else if authority.code_object_version != TILED_GEMM_CODE_OBJECT_VERSION_V1 {
        Some("code-object version")
    } else if authority.explicit_kernarg_bytes != TILED_GEMM_EXPLICIT_KERNARG_BYTES_V1
        || authority.complete_kernarg_bytes != TILED_GEMM_COMPLETE_KERNARG_BYTES_V1
    {
        Some("kernarg ABI sizes")
    } else if authority.abi_binding_commitment != exact_abi_binding_commitment() {
        Some("explicit ABI")
    } else if authority.fn_abi_binding_commitment != RUSTC_FN_ABI_IDENTITY {
        Some("rustc FnAbi")
    } else if authority.launch_binding_commitment != exact_launch_binding_commitment() {
        Some("launch contract")
    } else if authority.correspondence_commitment != reviewed_correspondence_commitment() {
        Some("reviewed source-to-canonical-module correspondence")
    } else if authority.frontend_contract_commitment != sha256(EXACT_FRONTEND_CONTRACT_V1) {
        Some("frontend contract")
    } else if authority.kernel_export != FIXED_KERNEL_EXPORT {
        Some("kernel export")
    } else if authority.root_instance_identity != REVIEWED_ROOT_INSTANCE_IDENTITY {
        Some("root instance")
    } else if authority.portable_mir_semantic_commitment != PORTABLE_MIR_SEMANTIC_IDENTITY {
        Some("portable MIR")
    } else if authority.compiler_semantics_commitment
        != compiler_semantics_commitment(&reviewed_compiler_semantics())
    {
        Some("compiler semantics")
    } else if authority.descriptor_source_commitment == [0; 32] {
        Some("descriptor source")
    } else {
        None
    };
    if let Some(field) = field {
        return Err(CollectedTiledGemmErrorV1::ReceiptBindingMismatch { field });
    }
    let expected_authority = collected_authority_commitment(authority);
    if authority.authority_commitment != expected_authority {
        return Err(CollectedTiledGemmErrorV1::ReceiptBindingMismatch {
            field: "authority commitment",
        });
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn exact_frontend_receipt_for_test() -> TiledGemmFrontendReceiptV1 {
    let compiler_semantics_commitment =
        compiler_semantics_commitment(&reviewed_compiler_semantics());
    let abi_binding_commitment = exact_abi_binding_commitment();
    let launch_binding_commitment = exact_launch_binding_commitment();
    let correspondence_commitment = reviewed_correspondence_commitment();
    let frontend_contract_commitment = sha256(EXACT_FRONTEND_CONTRACT_V1);
    let descriptor_source = crate::compiler_descriptor::tiled_gemm_v1_descriptor_source_for_test();
    let descriptor_source_commitment = *descriptor_source.identity().sha256();
    let mut authority = TiledGemmFrontendAuthorityV1 {
        target: EXACT_TILED_GEMM_TARGET_V1.to_owned(),
        code_object_version: TILED_GEMM_CODE_OBJECT_VERSION_V1,
        explicit_kernarg_bytes: TILED_GEMM_EXPLICIT_KERNARG_BYTES_V1,
        complete_kernarg_bytes: TILED_GEMM_COMPLETE_KERNARG_BYTES_V1,
        abi_binding_commitment,
        fn_abi_binding_commitment: RUSTC_FN_ABI_IDENTITY,
        launch_binding_commitment,
        correspondence_commitment,
        frontend_contract_commitment,
        kernel_export: FIXED_KERNEL_EXPORT.to_owned(),
        root_instance_identity: REVIEWED_ROOT_INSTANCE_IDENTITY.to_owned(),
        portable_mir_semantic_commitment: PORTABLE_MIR_SEMANTIC_IDENTITY,
        compiler_semantics_commitment,
        descriptor_source_commitment,
        authority_commitment: [0; 32],
    };
    authority.authority_commitment = collected_authority_commitment(&authority);
    TiledGemmFrontendReceiptV1 {
        authority: Some(authority),
        descriptor_source: Some(descriptor_source),
    }
}

fn hash_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn unsupported_collection(detail: impl Into<String>) -> CollectedTiledGemmErrorV1 {
    CollectedTiledGemmErrorV1::UnsupportedCollection {
        detail: detail.into(),
    }
}

fn abi_mismatch(detail: impl Into<String>) -> CollectedTiledGemmErrorV1 {
    CollectedTiledGemmErrorV1::AbiMismatch {
        detail: detail.into(),
    }
}

fn layout_mismatch(detail: impl Into<String>) -> CollectedTiledGemmErrorV1 {
    CollectedTiledGemmErrorV1::LayoutMismatch {
        detail: detail.into(),
    }
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

    fn compiler_semantics() -> CompilerSemanticsV1 {
        reviewed_compiler_semantics()
    }

    fn exact_test_receipt() -> TiledGemmFrontendReceiptV1 {
        exact_frontend_receipt_for_test()
    }

    #[test]
    fn exact_execution_profile_is_closed() {
        assert!(admit_execution_context(EXACT_TILED_GEMM_TARGET_V1, false).is_ok());
        for target in [
            "gfx942",
            "gfx942:xnack+",
            "gfx942:sramecc+:xnack-",
            "gfx942:xnack-:sramecc+",
            "gfx941:xnack-",
            "gfx950:xnack-",
        ] {
            assert!(matches!(
                admit_execution_context(target, false),
                Err(CollectedTiledGemmErrorV1::WrongTarget { .. })
            ));
        }
        assert!(matches!(
            admit_execution_context(EXACT_TILED_GEMM_TARGET_V1, true),
            Err(CollectedTiledGemmErrorV1::CustomPipeline)
        ));
        assert_eq!(TILED_GEMM_CODE_OBJECT_VERSION_V1, 6);
        assert_eq!(TILED_GEMM_EXPLICIT_KERNARG_BYTES_V1, 64);
        assert_eq!(TILED_GEMM_COMPLETE_KERNARG_BYTES_V1, 320);
        assert_eq!(TILED_GEMM_KERNEL_SYMBOL_V1, "tiled_gemm_v1");
    }

    #[test]
    fn every_compiler_semantics_substitution_fails_closed() {
        let baseline = compiler_semantics();
        assert!(require_compiler_semantics(&baseline).is_ok());

        let mut mutations = Vec::new();
        let mut value = baseline.clone();
        value.panic_strategy = "Abort".to_owned();
        mutations.push(value);
        let mut value = baseline.clone();
        value.overflow_checks = true;
        mutations.push(value);
        let mut value = baseline.clone();
        value.optimize = "Less".to_owned();
        mutations.push(value);
        let mut value = baseline.clone();
        value.debug_assertions = false;
        mutations.push(value);
        let mut value = baseline.clone();
        value.mir_opt_level = 2;
        mutations.push(value);
        let mut value = baseline.clone();
        value.mir_enable_passes.clear();
        mutations.push(value);
        let mut value = baseline.clone();
        value.llvm_args.push("-enable-unsafe-fp-math".to_owned());
        mutations.push(value);
        let mut value = baseline.clone();
        value.llvm_passes.push("default<O3>".to_owned());
        mutations.push(value);
        let mut value = baseline.clone();
        value.target_cpu = Some("native".to_owned());
        mutations.push(value);
        let mut value = baseline.clone();
        value.target_features = "+fma".to_owned();
        mutations.push(value);
        let mut value = baseline.clone();
        value.crate_metadata = vec!["attacker".to_owned()];
        mutations.push(value);
        let mut value = baseline;
        value.remap_path_destinations.push("/attacker".to_owned());
        mutations.push(value);

        for mutation in mutations {
            assert!(matches!(
                require_compiler_semantics(&mutation),
                Err(CollectedTiledGemmErrorV1::CompilerSemantics { .. })
            ));
        }
    }

    #[test]
    fn authority_commitment_binds_every_authority_field() {
        let baseline_receipt = exact_test_receipt();
        let baseline = collected_authority_commitment(
            baseline_receipt.authority.as_ref().expect("test authority"),
        );
        let mutations: [fn(&mut TiledGemmFrontendAuthorityV1); 14] = [
            |authority| authority.portable_mir_semantic_commitment[0] ^= 1,
            |authority| authority.compiler_semantics_commitment[0] ^= 1,
            |authority| authority.root_instance_identity.push_str("_other"),
            |authority| authority.kernel_export.push_str("_other"),
            |authority| authority.target = "gfx942:xnack+".to_owned(),
            |authority| authority.code_object_version = 5,
            |authority| authority.explicit_kernarg_bytes = 63,
            |authority| authority.complete_kernarg_bytes = 319,
            |authority| authority.abi_binding_commitment[0] ^= 1,
            |authority| authority.fn_abi_binding_commitment[0] ^= 1,
            |authority| authority.launch_binding_commitment[0] ^= 1,
            |authority| authority.correspondence_commitment[0] ^= 1,
            |authority| authority.frontend_contract_commitment[0] ^= 1,
            |authority| authority.descriptor_source_commitment[0] ^= 1,
        ];
        for mutate in mutations {
            let mut receipt = exact_test_receipt();
            let authority = receipt.authority.as_mut().expect("test authority");
            mutate(authority);
            assert_ne!(baseline, collected_authority_commitment(authority));
        }
    }

    #[test]
    fn copied_digest_does_not_mint_frontend_authority() {
        let mut receipt = exact_test_receipt();
        let authority = receipt.authority.as_mut().unwrap();
        authority.compiler_semantics_commitment = [0x5a; 32];
        assert!(matches!(
            receipt.consume(),
            Err(CollectedTiledGemmErrorV1::ReceiptBindingMismatch {
                field: "compiler semantics"
            })
        ));
    }

    #[test]
    fn wrong_target_receipt_fails_closed() {
        let mut receipt = exact_test_receipt();
        receipt.authority.as_mut().unwrap().target = "gfx942:xnack+".to_owned();
        assert!(matches!(
            receipt.consume(),
            Err(CollectedTiledGemmErrorV1::ReceiptBindingMismatch { field: "target" })
        ));
    }

    #[test]
    fn descriptor_source_mutation_fails_closed() {
        let mut receipt = exact_test_receipt();
        receipt.descriptor_source =
            Some(crate::compiler_descriptor::scalar_gemm_v1_descriptor_source_for_test());
        assert!(matches!(
            receipt.consume(),
            Err(CollectedTiledGemmErrorV1::ReceiptBindingMismatch {
                field: "descriptor source"
            })
        ));
    }

    #[test]
    fn production_profile_is_not_fragment_probe_or_scalar_gemm() {
        let fragment = fe2o3_kernel_ir::MatrixProjectedKernargPolicyV1::canonical();
        assert_eq!(fragment.explicit_argument_size, 32);
        assert_eq!(fragment.kernarg_segment_size, 288);
        assert_ne!(
            u64::from(fragment.explicit_argument_size),
            TILED_GEMM_EXPLICIT_KERNARG_BYTES_V1
        );
        assert_ne!(
            u64::from(fragment.kernarg_segment_size),
            TILED_GEMM_COMPLETE_KERNARG_BYTES_V1
        );

        let tiled_source = crate::compiler_descriptor::tiled_gemm_v1_descriptor_source_for_test();
        let scalar_source = crate::compiler_descriptor::scalar_gemm_v1_descriptor_source_for_test();
        let tiled = &tiled_source.table().kernels()[0];
        let scalar = &scalar_source.table().kernels()[0];
        assert_eq!(tiled.abi_layout().explicit_argument_size(), 64);
        assert_eq!(tiled.abi_layout().kernarg_segment_size(), 320);
        assert_eq!(scalar.abi_layout().explicit_argument_size(), 64);
        assert_eq!(scalar.abi_layout().kernarg_segment_size(), 320);
        assert_eq!(tiled.entry_name().as_str(), "tiled_gemm_v1");
        assert_eq!(scalar.entry_name().as_str(), "scalar_gemm_v1");
        assert_eq!(tiled.arguments().len(), 4);
        assert_eq!(scalar.arguments().len(), 6);
        assert_eq!(
            tiled.launch().block_size(),
            fe2o3_kernel_descriptor::BlockSizeV1::Exact(
                fe2o3_kernel_descriptor::DimensionsV1::new(64, 1, 1).unwrap()
            )
        );
        assert_eq!(
            scalar.launch().block_size(),
            fe2o3_kernel_descriptor::BlockSizeV1::Exact(
                fe2o3_kernel_descriptor::DimensionsV1::new(256, 1, 1).unwrap()
            )
        );
        assert_eq!(
            tiled.capabilities(),
            &[
                fe2o3_kernel_descriptor::CapabilityV1::Subgroup,
                fe2o3_kernel_descriptor::CapabilityV1::MatrixMultiply,
                fe2o3_kernel_descriptor::CapabilityV1::AmdWave,
                fe2o3_kernel_descriptor::CapabilityV1::AmdMfma,
            ]
        );
        assert_eq!(
            REVIEWED_CORRESPONDENCE_V1,
            b"exact reviewed Rust portable-MIR identity selects fe2o3::tiled_gemm_v1;canonical one-wave mapping;bounded reviewed correspondence only;not a compiler-refinement proof"
        );
    }

    #[test]
    fn frontend_receipt_is_single_use() {
        let mut receipt = exact_test_receipt();
        let authenticated = receipt.consume().expect("first consumption");
        let (module, descriptor_source) = authenticated.into_parts();
        assert_eq!(module, fe2o3_kernel_ir::tiled_gemm_v1_module());
        assert_eq!(descriptor_source.table().kernels().len(), 1);
        assert!(matches!(
            receipt.consume(),
            Err(CollectedTiledGemmErrorV1::ReceiptAlreadyConsumed)
        ));
    }
}
