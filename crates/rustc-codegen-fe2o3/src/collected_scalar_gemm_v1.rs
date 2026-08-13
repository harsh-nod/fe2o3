//! Source-authenticated admission for the fixed scalar GEMM V1 profile.
//!
//! This checkpoint authenticates one exact collected rustc root. It deliberately
//! does not construct executable lowering authority.

use std::error::Error;
use std::fmt;

use fe2o3_artifacts::{
    AbiKind, Access, AddressSpace, AliasClass, ArgumentOwnership, BlockSize, Capability,
    Dimensions, Endianness, IdentityText, Mutability as ArtifactMutability, PointerWidth,
    RustScalarElementTypeV1, ScalarType, TargetIdentity,
};
use rustc_abi::ExternAbi;
use rustc_hir::{Mutability, Safety};
use rustc_middle::ty::{FloatTy, Instance, InstanceKind, Ty, TyCtxt, TyKind, TypingEnv, UintTy};
use sha2::{Digest as _, Sha256};

use crate::AmdGpuTarget;
use crate::collector::{
    CollectedFunction, CollectedFunctionRole, CollectionResult, TypedKernelProfile,
};
use crate::rust_type_layout_v3::GeneralTypedArgumentKindV3;
use crate::trusted_device_items::{self, TrustedDeviceItem};

pub(crate) const COLLECTED_SCALAR_GEMM_PIPELINE_V1: &str = "collected-scalar-gemm-v1";
pub(crate) const EXACT_SCALAR_GEMM_TARGET_V1: &str = "gfx942:xnack-";
pub(crate) const SCALAR_GEMM_CODE_OBJECT_VERSION_V1: u16 = 6;
pub(crate) const SCALAR_GEMM_EXPLICIT_KERNARG_BYTES_V1: u64 = 64;
pub(crate) const SCALAR_GEMM_COMPLETE_KERNARG_BYTES_V1: u64 = 320;
pub(crate) const SCALAR_GEMM_KERNEL_SYMBOL_V1: &str = "scalar_gemm_v1";
pub(crate) const NEXT_LOWERING_DEPENDENCY: &str = "body-bound executable lowering, checked host launch admission, and upstream LLVM/LLD COV6 finalization remain required";

const FIXED_KERNEL_EXPORT: &str = SCALAR_GEMM_KERNEL_SYMBOL_V1;
const FIXED_LOGICAL_NAME: &str = SCALAR_GEMM_KERNEL_SYMBOL_V1;
const REVIEWED_RUSTC_RELEASE: &str = "1.96.0-nightly";
const REVIEWED_RUSTC_COMMIT: &str = "55e86c996809902e8bbad512cfb4d2c18be446d9";
const REVIEWED_RUSTC_LLVM: &str = "22.1.2";
const REVIEWED_CRATE_METADATA: &str = "fe2o3-scalar-gemm-v1-reviewed";
const COMPILER_SEMANTICS_DOMAIN_V1: &[u8] = b"fe2o3.scalar-gemm.compiler-semantics.v1";
const COLLECTED_AUTHORITY_DOMAIN_V1: &[u8] = b"fe2o3.scalar-gemm.collected-authority.v1";

// Reviewed from the exact fixture through path-independent portable-MIR
// collection under the compiler-semantics profile below.
const PORTABLE_MIR_SEMANTIC_IDENTITY: [u8; 32] = [
    0xaf, 0x4c, 0xa7, 0x6c, 0x45, 0x17, 0xb7, 0x79, 0xbc, 0xa4, 0xb7, 0xa6, 0x3b, 0xca, 0xe0, 0x9a,
    0x23, 0xca, 0xd9, 0x47, 0xe7, 0x40, 0xb2, 0xe5, 0x1f, 0x87, 0x2d, 0x7c, 0xc0, 0xd6, 0xd0, 0x02,
];

const ARGUMENT_KINDS: [GeneralTypedArgumentKindV3; 6] = [
    GeneralTypedArgumentKindV3::SharedSlice(RustScalarElementTypeV1::F32),
    GeneralTypedArgumentKindV3::SharedSlice(RustScalarElementTypeV1::F32),
    GeneralTypedArgumentKindV3::DisjointSlice(RustScalarElementTypeV1::F32),
    GeneralTypedArgumentKindV3::Scalar(RustScalarElementTypeV1::U32),
    GeneralTypedArgumentKindV3::Scalar(RustScalarElementTypeV1::U32),
    GeneralTypedArgumentKindV3::Scalar(RustScalarElementTypeV1::U32),
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedCollectedScalarGemmV1 {
    kernel_export: String,
    root_instance_identity: String,
    portable_mir_semantic_commitment: [u8; 32],
    compiler_semantics_commitment: [u8; 32],
    authority_commitment: [u8; 32],
}

impl AuthenticatedCollectedScalarGemmV1 {
    pub(crate) fn kernel_export(&self) -> &str {
        &self.kernel_export
    }

    pub(crate) fn root_instance_identity(&self) -> &str {
        &self.root_instance_identity
    }

    pub(crate) fn portable_mir_semantic_hex(&self) -> String {
        encode_hex(&self.portable_mir_semantic_commitment)
    }

    pub(crate) fn compiler_semantics_hex(&self) -> String {
        encode_hex(&self.compiler_semantics_commitment)
    }

    pub(crate) fn authority_hex(&self) -> String {
        encode_hex(&self.authority_commitment)
    }
}

#[derive(Debug)]
pub(crate) enum CollectedScalarGemmErrorV1 {
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
    PortableMir {
        detail: String,
    },
}

impl fmt::Display for CollectedScalarGemmErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongTarget { actual } => write!(
                formatter,
                "collected scalar GEMM V1 requires exact target `{EXACT_SCALAR_GEMM_TARGET_V1}`, found `{actual}`"
            ),
            Self::CustomPipeline => formatter
                .write_str("collected scalar GEMM V1 rejects custom LLVM pipeline selection"),
            Self::CompilerSemantics { detail } => write!(
                formatter,
                "collected scalar GEMM V1 compiler semantics mismatch: {detail}"
            ),
            Self::UnsupportedCollection { detail } => write!(
                formatter,
                "unsupported collected scalar GEMM V1 shape: {detail}"
            ),
            Self::AbiMismatch { detail } => {
                write!(formatter, "collected scalar GEMM V1 ABI mismatch: {detail}")
            }
            Self::LayoutMismatch { detail } => write!(
                formatter,
                "collected scalar GEMM V1 typed layout mismatch: {detail}"
            ),
            Self::PortableMirIdentityMismatch { expected, actual } => write!(
                formatter,
                "collected scalar GEMM V1 portable MIR identity mismatch: expected {}, found {}",
                encode_hex(expected),
                encode_hex(actual)
            ),
            Self::PortableMir { detail } => write!(
                formatter,
                "collected scalar GEMM V1 portable MIR rejected: {detail}"
            ),
        }
    }
}

impl Error for CollectedScalarGemmErrorV1 {}

pub(crate) fn authenticate_collected_scalar_gemm_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    collection: &CollectionResult<'tcx>,
    target: &AmdGpuTarget,
    custom_llvm_pipeline: bool,
) -> Result<AuthenticatedCollectedScalarGemmV1, CollectedScalarGemmErrorV1> {
    admit_execution_context(target.as_str(), custom_llvm_pipeline)?;
    let compiler_semantics = observe_compiler_semantics(tcx);
    let compiler_semantics_commitment = require_compiler_semantics(&compiler_semantics)?;
    let root = exact_collected_root(&collection.functions)?;
    require_registration(root)?;
    require_signature(tcx, root.instance)?;
    require_layout(root)?;

    let contract = root
        .general_typed_contract
        .as_ref()
        .ok_or_else(|| layout_mismatch("General V3 contract is absent after layout admission"))?;
    let target_identity = scalar_gemm_target_identity()?;
    let imported = crate::mir_import::import_collection(tcx, collection).map_err(|error| {
        CollectedScalarGemmErrorV1::PortableMir {
            detail: error.to_string(),
        }
    })?;
    let portable_mir_semantic_commitment = imported
        .portable_semantic_digest_v2(crate::mir_import::MirSemanticAdmissionInputsV2::new(
            FIXED_KERNEL_EXPORT,
            &target_identity,
            contract.abi(),
            contract.launch(),
        ))
        .map_err(|error| CollectedScalarGemmErrorV1::PortableMir {
            detail: error.to_string(),
        })?;
    let portable_mir_semantic_commitment = *portable_mir_semantic_commitment.as_bytes();
    if portable_mir_semantic_commitment != PORTABLE_MIR_SEMANTIC_IDENTITY {
        return Err(CollectedScalarGemmErrorV1::PortableMirIdentityMismatch {
            expected: PORTABLE_MIR_SEMANTIC_IDENTITY,
            actual: portable_mir_semantic_commitment,
        });
    }

    let root_instance_identity = tcx.def_path_str(root.instance.def_id());
    let authority_commitment = collected_authority_commitment(
        portable_mir_semantic_commitment,
        compiler_semantics_commitment,
        &root_instance_identity,
        &root.export_name,
    );
    Ok(AuthenticatedCollectedScalarGemmV1 {
        kernel_export: root.export_name.clone(),
        root_instance_identity,
        portable_mir_semantic_commitment,
        compiler_semantics_commitment,
        authority_commitment,
    })
}

fn admit_execution_context(
    target: &str,
    custom_llvm_pipeline: bool,
) -> Result<(), CollectedScalarGemmErrorV1> {
    if target != EXACT_SCALAR_GEMM_TARGET_V1 {
        return Err(CollectedScalarGemmErrorV1::WrongTarget {
            actual: target.to_owned(),
        });
    }
    if custom_llvm_pipeline {
        return Err(CollectedScalarGemmErrorV1::CustomPipeline);
    }
    Ok(())
}

fn exact_collected_root<'a, 'tcx>(
    functions: &'a [CollectedFunction<'tcx>],
) -> Result<&'a CollectedFunction<'tcx>, CollectedScalarGemmErrorV1> {
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

fn require_registration(root: &CollectedFunction<'_>) -> Result<(), CollectedScalarGemmErrorV1> {
    if root.export_name != FIXED_KERNEL_EXPORT
        || root.logical_name.as_deref() != Some(FIXED_LOGICAL_NAME)
        || !matches!(
            root.typed_profile,
            Some(TypedKernelProfile::GeneralScalarSliceRustcLayoutV3 { .. })
        )
        || root.kernel_binding.is_none()
        || root.frontend_contract.is_some()
    {
        return Err(unsupported_collection(
            "kernel registration must be the unique compiler-authenticated General V3 scalar_gemm_v1 root without an additional frontend contract",
        ));
    }
    Ok(())
}

fn require_signature<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<(), CollectedScalarGemmErrorV1> {
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
        || signature.inputs().len() != 6
    {
        return Err(abi_mismatch(format!(
            "expected safe non-variadic Rust ABI `(&[f32], &[f32], DisjointSlice<f32>, u32, u32, u32) -> ()`, found `{signature}`"
        )));
    }
    let inputs = signature.inputs();
    if !is_shared_f32_slice(inputs[0])
        || !is_shared_f32_slice(inputs[1])
        || !is_disjoint_f32_slice(tcx, inputs[2])
        || inputs[3..]
            .iter()
            .any(|ty| !matches!(ty.kind(), TyKind::Uint(UintTy::U32)))
    {
        return Err(abi_mismatch(format!(
            "expected exact argument order `A:&[f32], B:&[f32], C:DisjointSlice<f32>, m:u32, n:u32, k:u32`, found `{signature}`"
        )));
    }
    Ok(())
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

fn require_layout(root: &CollectedFunction<'_>) -> Result<(), CollectedScalarGemmErrorV1> {
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
            "expected exact scalar GEMM argument kinds {ARGUMENT_KINDS:?}, found {actual:?}"
        )));
    }
    let abi = contract.abi();
    if abi.size() != SCALAR_GEMM_EXPLICIT_KERNARG_BYTES_V1 || abi.alignment() != 8 {
        return Err(layout_mismatch(format!(
            "explicit kernarg must be exactly 64 bytes aligned to 8, found {} bytes aligned to {}",
            abi.size(),
            abi.alignment()
        )));
    }
    let expected_names = ["a", "b", "c", "m", "n", "k"];
    let expected_offsets = [0, 16, 32, 48, 52, 56];
    let expected_sizes = [16, 16, 16, 4, 4, 4];
    if abi.fields().len() != expected_names.len() {
        return Err(layout_mismatch(format!(
            "expected six ABI fields, found {}",
            abi.fields().len()
        )));
    }
    for (index, field) in abi.fields().iter().enumerate() {
        if field.name().as_str() != expected_names[index]
            || field.offset() != expected_offsets[index]
            || field.size() != expected_sizes[index]
        {
            return Err(layout_mismatch(format!(
                "field {index} must be {}@{} size {}, found {}@{} size {}",
                expected_names[index],
                expected_offsets[index],
                expected_sizes[index],
                field.name().as_str(),
                field.offset(),
                field.size()
            )));
        }
        match index {
            0 | 1 => {
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
                    return Err(layout_mismatch(format!(
                        "field {} must be an immutable shared &[f32] global slice",
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
                ) || field.mutability() != ArtifactMutability::Mutable
                    || field.access() != Access::ReadWrite
                    || field.address_space() != AddressSpace::Global
                    || field.ownership() != ArgumentOwnership::UniqueBorrow
                    || field.alias_class() != AliasClass::Exclusive
                {
                    return Err(layout_mismatch(
                        "field c must be the unique genuine DisjointSlice<f32> global slice",
                    ));
                }
            }
            3..=5 => {
                if field.kind() != AbiKind::Scalar(ScalarType::U32)
                    || field.mutability() != ArtifactMutability::Immutable
                    || field.access() != Access::ByValue
                    || field.address_space() != AddressSpace::Value
                    || field.ownership() != ArgumentOwnership::ByValue
                    || field.alias_class() != AliasClass::Value
                {
                    return Err(layout_mismatch(format!(
                        "field {} must be an exact by-value u32 scalar",
                        expected_names[index]
                    )));
                }
            }
            _ => unreachable!(),
        }
    }
    let launch = contract.launch();
    if launch.rank() != 1
        || launch.block_size()
            != BlockSize::Exact(Dimensions::new(256, 1, 1).map_err(|error| {
                layout_mismatch(format!("invalid fixed workgroup dimensions: {error}"))
            })?)
        || launch.max_grid()
            != Dimensions::new(u32::MAX, 1, 1).map_err(|error| {
                layout_mismatch(format!("invalid fixed grid dimensions: {error}"))
            })?
        || launch.static_shared_memory_bytes() != 0
        || launch.max_dynamic_shared_memory_bytes() != 0
    {
        return Err(layout_mismatch(
            "launch contract must be exact 256x1x1 with one-dimensional u32 grid and no shared memory",
        ));
    }
    Ok(())
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
) -> Result<[u8; 32], CollectedScalarGemmErrorV1> {
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
    } else if observed.crate_metadata != [REVIEWED_CRATE_METADATA] {
        Some(format!(
            "crate metadata must be exactly {REVIEWED_CRATE_METADATA:?}, found {:?}",
            observed.crate_metadata
        ))
    } else if observed.remap_path_destinations
        != [
            "/fe2o3-reviewed-workspace/scalar-gemm-v1.rs",
            "/fe2o3-reviewed-workspace",
        ]
    {
        Some(format!(
            "source remapping must contain exactly the canonical scalar GEMM fixture and workspace destinations, found {:?}",
            observed.remap_path_destinations
        ))
    } else {
        None
    };
    if let Some(detail) = mismatch {
        return Err(CollectedScalarGemmErrorV1::CompilerSemantics { detail });
    }

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
    Ok(digest.finalize().into())
}

fn scalar_gemm_target_identity() -> Result<TargetIdentity, CollectedScalarGemmErrorV1> {
    TargetIdentity::new(
        IdentityText::new(dialect_amdgcn::AMDGPU_TRIPLE)
            .map_err(|error| unsupported_collection(format!("invalid AMDGPU triple: {error}")))?,
        IdentityText::new(EXACT_SCALAR_GEMM_TARGET_V1)
            .map_err(|error| unsupported_collection(format!("invalid gfx942 profile: {error}")))?,
        PointerWidth::Bits64,
        Endianness::Little,
        vec![Capability::Atomics, Capability::AmdWave],
    )
    .map_err(|error| unsupported_collection(format!("invalid target identity: {error}")))
}

fn collected_authority_commitment(
    portable_mir_semantic_commitment: [u8; 32],
    compiler_semantics: [u8; 32],
    root_instance_identity: &str,
    kernel_export: &str,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    hash_field(&mut digest, COLLECTED_AUTHORITY_DOMAIN_V1);
    hash_field(&mut digest, &portable_mir_semantic_commitment);
    hash_field(&mut digest, &compiler_semantics);
    hash_field(&mut digest, root_instance_identity.as_bytes());
    hash_field(&mut digest, kernel_export.as_bytes());
    hash_field(
        &mut digest,
        &SCALAR_GEMM_CODE_OBJECT_VERSION_V1.to_le_bytes(),
    );
    hash_field(
        &mut digest,
        &SCALAR_GEMM_EXPLICIT_KERNARG_BYTES_V1.to_le_bytes(),
    );
    hash_field(
        &mut digest,
        &SCALAR_GEMM_COMPLETE_KERNARG_BYTES_V1.to_le_bytes(),
    );
    digest.finalize().into()
}

fn hash_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

fn unsupported_collection(detail: impl Into<String>) -> CollectedScalarGemmErrorV1 {
    CollectedScalarGemmErrorV1::UnsupportedCollection {
        detail: detail.into(),
    }
}

fn abi_mismatch(detail: impl Into<String>) -> CollectedScalarGemmErrorV1 {
    CollectedScalarGemmErrorV1::AbiMismatch {
        detail: detail.into(),
    }
}

fn layout_mismatch(detail: impl Into<String>) -> CollectedScalarGemmErrorV1 {
    CollectedScalarGemmErrorV1::LayoutMismatch {
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
            crate_metadata: vec![REVIEWED_CRATE_METADATA.to_owned()],
            remap_path_destinations: vec![
                "/fe2o3-reviewed-workspace/scalar-gemm-v1.rs".to_owned(),
                "/fe2o3-reviewed-workspace".to_owned(),
            ],
        }
    }

    #[test]
    fn exact_execution_profile_is_closed() {
        assert!(admit_execution_context(EXACT_SCALAR_GEMM_TARGET_V1, false).is_ok());
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
                Err(CollectedScalarGemmErrorV1::WrongTarget { .. })
            ));
        }
        assert!(matches!(
            admit_execution_context(EXACT_SCALAR_GEMM_TARGET_V1, true),
            Err(CollectedScalarGemmErrorV1::CustomPipeline)
        ));
        assert_eq!(SCALAR_GEMM_CODE_OBJECT_VERSION_V1, 6);
        assert_eq!(SCALAR_GEMM_EXPLICIT_KERNARG_BYTES_V1, 64);
        assert_eq!(SCALAR_GEMM_COMPLETE_KERNARG_BYTES_V1, 320);
        assert_eq!(SCALAR_GEMM_KERNEL_SYMBOL_V1, "scalar_gemm_v1");
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
                Err(CollectedScalarGemmErrorV1::CompilerSemantics { .. })
            ));
        }
    }

    #[test]
    fn authority_commitment_binds_every_authority_field() {
        let baseline = collected_authority_commitment([1; 32], [2; 32], "root", "export");
        assert_ne!(
            baseline,
            collected_authority_commitment([3; 32], [2; 32], "root", "export")
        );
        assert_ne!(
            baseline,
            collected_authority_commitment([1; 32], [4; 32], "root", "export")
        );
        assert_ne!(
            baseline,
            collected_authority_commitment([1; 32], [2; 32], "other", "export")
        );
        assert_ne!(
            baseline,
            collected_authority_commitment([1; 32], [2; 32], "root", "other")
        );
    }
}
