//! Source-authenticated admission for one fixed row-softmax V1 profile.
//!
//! This layer authenticates one exact rustc root and consumes a private receipt
//! to select one canonical Kernel IR module. It deliberately stops there. In
//! particular, the canonical exp operation has no authenticated implementation
//! or numerical refinement contract, and this module grants no LLVM, link,
//! machine-body, load, launch, memory-safety, or race-freedom authority.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use fe2o3_artifacts::{
    AbiKind, Access, AddressSpace as ArtifactAddressSpace, AliasClass, ArgumentOwnership,
    BlockSize, Capability, Dimensions, Endianness, IdentityText, LaunchContract,
    Mutability as ArtifactMutability, PointerWidth, RustScalarElementTypeV1, TargetIdentity,
};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, Axis, BasicBlock, BinaryOp, BlockId, ComparePredicate, Constant,
    F32MathFunction, FloatOperation, Function, IndexKind, IntrinsicKind, IntrinsicOperation,
    Kernel, LaunchDomain, LaunchExtent, MemoryAccess, Module, Operation, OperationKind, Signature,
    TargetCapability, Terminator, Type, ValueDef, ValueId, WaveWidth, WorkgroupSize,
    encode_module_v4, gfx942_xnack_minus_target_capability, verify_module,
};
use rustc_abi::{CanonAbi, ExternAbi};
use rustc_hir::{Mutability, Safety};
use rustc_middle::ty::{FloatTy, Instance, InstanceKind, Ty, TyCtxt, TyKind, TypingEnv};
use rustc_target::callconv::{ArgAttributes, ArgExtension, PassMode};
use sha2::{Digest as _, Sha256};

use crate::AmdGpuTarget;
use crate::collector::{
    CollectedFunction, CollectedFunctionRole, CollectionResult, TypedKernelProfile,
};
use crate::rust_type_layout_v3::GeneralTypedArgumentKindV3;
use crate::trusted_device_items::{self, TrustedDeviceItem};

pub(crate) const COLLECTED_ROW_SOFTMAX_PIPELINE_V1: &str = "collected-row-softmax-v1";
pub(crate) const EXACT_ROW_SOFTMAX_TARGET_V1: &str = "gfx942:xnack-";
pub(crate) const ROW_SOFTMAX_CODE_OBJECT_VERSION_V1: u16 = 6;
pub(crate) const ROW_SOFTMAX_EXPLICIT_KERNARG_BYTES_V1: u64 = 32;
pub(crate) const ROW_SOFTMAX_COMPLETE_KERNARG_BYTES_V1: u64 = 288;
pub(crate) const ROW_SOFTMAX_ELEMENTS_V1: u32 = 64;
pub(crate) const ROW_SOFTMAX_KERNEL_SYMBOL_V1: &str = "row_softmax_v1";

const CANONICAL_MODULE_ID: &str = "fe2o3::row_softmax_v1";
const CANONICAL_FUNCTION_ID: &str = "__fe2o3_row_softmax_v1_impl";
const FIXED_KERNEL_EXPORT: &str = ROW_SOFTMAX_KERNEL_SYMBOL_V1;
const FIXED_LOGICAL_NAME: &str = ROW_SOFTMAX_KERNEL_SYMBOL_V1;
const KERNEL_ROOT_BUILD_IDENTITY_PREFIX: &str = "__fe2o3_host_kernel_v1_";
#[cfg(test)]
const REPRESENTATIVE_ROOT_INSTANCE_IDENTITY: &str =
    "__fe2o3_host_kernel_v1_0000000000000000000000000000000000000000000000000000000000000000";
const REVIEWED_RUSTC_RELEASE: &str = "1.96.0-nightly";
const REVIEWED_RUSTC_COMMIT: &str = "55e86c996809902e8bbad512cfb4d2c18be446d9";
const REVIEWED_RUSTC_LLVM: &str = "22.1.2";
const REVIEWED_CRATE_METADATA: &str = "fe2o3-row-softmax-v1-reviewed";
const COMPILER_SEMANTICS_DOMAIN_V1: &[u8] = b"fe2o3.row-softmax.compiler-semantics.v1";
const CARGO_METADATA_OBSERVATION_DOMAIN_V1: &[u8] =
    b"fe2o3.row-softmax.cargo-metadata-observation.v1";
const CARGO_GENERATED_METADATA_SHAPE_V1: &[u8] = b"one-16-byte-lowercase-hex-token";
const COLLECTED_AUTHORITY_DOMAIN_V1: &[u8] = b"fe2o3.row-softmax.collected-authority.v1";
const ABI_BINDING_DOMAIN_V1: &[u8] = b"fe2o3.row-softmax.abi-binding.v1";
const FN_ABI_BINDING_DOMAIN_V1: &[u8] = b"fe2o3.row-softmax.rustc-fn-abi.v1";
const LAUNCH_BINDING_DOMAIN_V1: &[u8] = b"fe2o3.row-softmax.launch-binding.v1";
const CORRESPONDENCE_DOMAIN_V1: &[u8] = b"fe2o3.row-softmax.reviewed-correspondence.v1";
const EXPONENTIAL_BOUNDARY_DOMAIN_V1: &[u8] = b"fe2o3.row-softmax.exponential-boundary.v1";
const MODULE_BINDING_DOMAIN_V1: &[u8] = b"fe2o3.row-softmax.canonical-module.v1";
// Reviewed independently from the constructor below. This binds the exact V4
// graph while leaving the named exp operation's implementation unresolved.
const REVIEWED_CANONICAL_MODULE_V4_COMMITMENT: [u8; 32] = [
    0x1e, 0x1b, 0x14, 0xc6, 0x84, 0x2f, 0xfd, 0x09, 0x10, 0x3e, 0xb5, 0x5e, 0xb3, 0x9b, 0x1b, 0xca,
    0xe9, 0xc0, 0xda, 0x81, 0x59, 0x7f, 0xed, 0x61, 0x86, 0x76, 0x75, 0x62, 0x33, 0x72, 0x30, 0xe6,
];
const EXACT_ABI_BINDING_V1: &[u8] = b"ptr64;size=32;align=8;input@0:16:8:slice-f32:shared-readonly;output@16:16:8:slice-f32:exclusive-readwrite;lengths=exactly-64-by-host-precondition";
const EXACT_LAUNCH_BINDING_V1: &[u8] =
    b"rank=1;block=exact(64,1,1);grid=exact(1,1,1);static-shared=0;dynamic-shared=0;wave=64;cov=6";
const REVIEWED_CORRESPONDENCE_V1: &[u8] = b"exact reviewed Rust portable-MIR identity selects the private fe2o3::row_softmax_v1 canonical module;one lane performs three ordered 64-element loops;bounded reviewed correspondence only;not a compiler-refinement proof";
const EXPONENTIAL_BOUNDARY_V1: &[u8] = b"canonical Kernel IR names its abstract f32 exp operation;no authenticated implementation, approximation/error contract, OCML bitcode, link request, LLVM lowering, or real-number softmax equivalence";
const EXACT_FRONTEND_CONTRACT_V1: &[u8] = &[
    70, 69, 50, 79, 51, 75, 70, 0, 1, 0, 1, 0, 52, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 64, 0, 0, 0, 1,
    0, 0, 0, 1, 0, 0, 0, 64, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0,
];

// Filled from the exact fixture through path-independent portable-MIR
// collection under the compiler-semantics profile below.
const PORTABLE_MIR_SEMANTIC_IDENTITY: [u8; 32] = [
    0xcb, 0x10, 0xb6, 0xfa, 0xc6, 0x47, 0x54, 0x35, 0xe4, 0x5a, 0x6f, 0x91, 0x66, 0x73, 0x9c, 0x9e,
    0x26, 0xba, 0xe1, 0x70, 0x31, 0x10, 0x57, 0x91, 0xab, 0xf3, 0xf4, 0x40, 0xb0, 0x04, 0xd4, 0xdd,
];
const RUSTC_FN_ABI_IDENTITY: [u8; 32] = [
    0x1f, 0x97, 0x82, 0x38, 0x8c, 0x98, 0x28, 0x56, 0x4b, 0xd6, 0x34, 0xce, 0x21, 0x8a, 0x6f, 0xf1,
    0x18, 0x65, 0xdb, 0xba, 0x8a, 0x52, 0x83, 0xf5, 0xa0, 0x26, 0x7b, 0x2b, 0x7a, 0x97, 0xa4, 0xc6,
];

const ARGUMENT_KINDS: [GeneralTypedArgumentKindV3; 2] = [
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
struct CargoMetadataBuildObservationV1 {
    ordered_tokens: Vec<String>,
    commitment: [u8; 32],
}

impl CargoMetadataBuildObservationV1 {
    fn from_ordered_tokens(tokens: &[String]) -> Result<Self, String> {
        if tokens.len() != 2 {
            return Err(format!(
                "crate metadata must contain exactly Cargo's generated token followed by {REVIEWED_CRATE_METADATA:?}; found {tokens:?}"
            ));
        }
        let generated = &tokens[0];
        if generated.len() != 16
            || !generated
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(format!(
                "Cargo-generated crate metadata must be exactly 16 lowercase hexadecimal bytes; found {generated:?}"
            ));
        }
        if tokens[1] != REVIEWED_CRATE_METADATA {
            return Err(format!(
                "reviewed crate metadata must be the second token and exactly {REVIEWED_CRATE_METADATA:?}; found {tokens:?}"
            ));
        }

        let mut digest = Sha256::new();
        hash_field(&mut digest, CARGO_METADATA_OBSERVATION_DOMAIN_V1);
        for token in tokens {
            hash_field(&mut digest, token.as_bytes());
        }
        Ok(Self {
            ordered_tokens: tokens.to_vec(),
            commitment: digest.finalize().into(),
        })
    }

    fn validate(&self) -> Result<(), String> {
        let expected = Self::from_ordered_tokens(&self.ordered_tokens)?;
        if self.commitment != expected.commitment {
            return Err("Cargo metadata build-observation commitment mismatch".to_owned());
        }
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq)]
struct AdmittedCompilerSemanticsV1 {
    normalized_commitment: [u8; 32],
    cargo_metadata_build_observation: CargoMetadataBuildObservationV1,
}

#[derive(Debug, Eq, PartialEq)]
struct RowSoftmaxFrontendAuthorityV1 {
    target: String,
    code_object_version: u16,
    explicit_kernarg_bytes: u64,
    complete_kernarg_bytes: u64,
    row_elements: u32,
    abi_binding_commitment: [u8; 32],
    fn_abi_binding_commitment: [u8; 32],
    launch_binding_commitment: [u8; 32],
    correspondence_commitment: [u8; 32],
    exponential_boundary_commitment: [u8; 32],
    frontend_contract_commitment: [u8; 32],
    canonical_module_commitment: [u8; 32],
    kernel_export: String,
    root_instance_identity: String,
    portable_mir_semantic_commitment: [u8; 32],
    compiler_semantics_commitment: [u8; 32],
    cargo_metadata_build_observation: CargoMetadataBuildObservationV1,
    provider_authority: crate::mir_import::RowSoftmaxProviderAuthorityV1,
    authority_commitment: [u8; 32],
}

/// Opaque single-use authority minted only by exact rustc admission.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct RowSoftmaxFrontendReceiptV1 {
    authority: Option<RowSoftmaxFrontendAuthorityV1>,
}

impl RowSoftmaxFrontendReceiptV1 {
    pub(crate) fn kernel_export(&self) -> &str {
        &self.authority().kernel_export
    }

    pub(crate) fn root_instance_identity(&self) -> &str {
        &self.authority().root_instance_identity
    }

    pub(crate) fn portable_mir_semantic_hex(&self) -> String {
        encode_hex(&self.authority().portable_mir_semantic_commitment)
    }

    pub(crate) fn compiler_semantics_hex(&self) -> String {
        encode_hex(&self.authority().compiler_semantics_commitment)
    }

    pub(crate) fn authority_hex(&self) -> String {
        encode_hex(&self.authority().authority_commitment)
    }

    pub(crate) fn consume(
        &mut self,
    ) -> Result<AuthenticatedRowSoftmaxModuleV1, CollectedRowSoftmaxErrorV1> {
        let authority = self
            .authority
            .take()
            .ok_or(CollectedRowSoftmaxErrorV1::ReceiptAlreadyConsumed)?;
        validate_frontend_authority(&authority)?;
        let module = canonical_row_softmax_v1_module();
        require_canonical_module(&module)?;
        if canonical_module_commitment(&module)? != authority.canonical_module_commitment {
            return Err(CollectedRowSoftmaxErrorV1::ReceiptBindingMismatch {
                field: "canonical module",
            });
        }
        Ok(AuthenticatedRowSoftmaxModuleV1 {
            module,
            authority_commitment: authority.authority_commitment,
            exponential_boundary_commitment: authority.exponential_boundary_commitment,
        })
    }

    fn authority(&self) -> &RowSoftmaxFrontendAuthorityV1 {
        self.authority
            .as_ref()
            .expect("unconsumed row-softmax receipt")
    }
}

/// Canonical Kernel IR selected by the source receipt, without executable authority.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedRowSoftmaxModuleV1 {
    module: Module,
    authority_commitment: [u8; 32],
    exponential_boundary_commitment: [u8; 32],
}

impl AuthenticatedRowSoftmaxModuleV1 {
    pub(crate) fn into_parts(self) -> (Module, [u8; 32], [u8; 32]) {
        (
            self.module,
            self.authority_commitment,
            self.exponential_boundary_commitment,
        )
    }
}

#[derive(Debug)]
pub(crate) enum CollectedRowSoftmaxErrorV1 {
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
    CanonicalModule {
        detail: String,
    },
    ReceiptAlreadyConsumed,
    ReceiptBindingMismatch {
        field: &'static str,
    },
}

impl fmt::Display for CollectedRowSoftmaxErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongTarget { actual } => write!(
                formatter,
                "collected row softmax V1 requires exact target `{EXACT_ROW_SOFTMAX_TARGET_V1}`, found `{actual}`"
            ),
            Self::CustomPipeline => formatter
                .write_str("collected row softmax V1 rejects custom LLVM pipeline selection"),
            Self::CompilerSemantics { detail } => write!(
                formatter,
                "collected row softmax V1 compiler semantics mismatch: {detail}"
            ),
            Self::UnsupportedCollection { detail } => write!(
                formatter,
                "unsupported collected row softmax V1 shape: {detail}"
            ),
            Self::AbiMismatch { detail } => {
                write!(formatter, "collected row softmax V1 ABI mismatch: {detail}")
            }
            Self::LayoutMismatch { detail } => write!(
                formatter,
                "collected row softmax V1 typed layout mismatch: {detail}"
            ),
            Self::PortableMirIdentityMismatch { expected, actual } => write!(
                formatter,
                "collected row softmax V1 portable MIR identity mismatch: expected {}, found {}",
                encode_hex(expected),
                encode_hex(actual)
            ),
            Self::FnAbiIdentityMismatch { expected, actual } => write!(
                formatter,
                "collected row softmax V1 rustc FnAbi identity mismatch: expected {}, found {}",
                encode_hex(expected),
                encode_hex(actual)
            ),
            Self::PortableMir { detail } => write!(
                formatter,
                "collected row softmax V1 portable MIR rejected: {detail}"
            ),
            Self::CanonicalModule { detail } => write!(
                formatter,
                "collected row softmax V1 canonical module rejected: {detail}"
            ),
            Self::ReceiptAlreadyConsumed => formatter
                .write_str("collected row softmax V1 frontend receipt was already consumed"),
            Self::ReceiptBindingMismatch { field } => write!(
                formatter,
                "collected row softmax V1 frontend receipt binding mismatch: {field}"
            ),
        }
    }
}

impl Error for CollectedRowSoftmaxErrorV1 {}

pub(crate) fn authenticate_collected_row_softmax_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    collection: &CollectionResult<'tcx>,
    target: &AmdGpuTarget,
    custom_llvm_pipeline: bool,
) -> Result<RowSoftmaxFrontendReceiptV1, CollectedRowSoftmaxErrorV1> {
    admit_execution_context(target.as_str(), custom_llvm_pipeline)?;
    let compiler_semantics = observe_compiler_semantics(tcx);
    let admitted_compiler_semantics = require_compiler_semantics(&compiler_semantics)?;
    let root = exact_collected_root(&collection.functions)?;
    require_registration(root)?;
    require_signature(tcx, root.instance)?;
    require_layout(root)?;
    let fn_abi_binding_commitment = require_rustc_fn_abi(tcx, root.instance)?;

    let contract = root
        .general_typed_contract
        .as_ref()
        .ok_or_else(|| layout_mismatch("General V3 contract is absent after layout admission"))?;
    let target_identity = row_softmax_target_identity()?;
    let launch = exact_row_softmax_launch_contract()?;
    let imported = crate::mir_import::import_collection(tcx, collection).map_err(|error| {
        CollectedRowSoftmaxErrorV1::PortableMir {
            detail: error.to_string(),
        }
    })?;
    let provider_authority = crate::mir_import::observe_row_softmax_provider_authority_v1(tcx)
        .map_err(|error| CollectedRowSoftmaxErrorV1::PortableMir {
            detail: error.to_string(),
        })?;
    let portable_mir_semantic_commitment = imported
        .portable_semantic_digest_v2(crate::mir_import::MirSemanticAdmissionInputsV2::new(
            FIXED_KERNEL_EXPORT,
            &target_identity,
            contract.abi(),
            &launch,
        ))
        .map_err(|error| CollectedRowSoftmaxErrorV1::PortableMir {
            detail: error.to_string(),
        })?;
    let portable_mir_semantic_commitment = *portable_mir_semantic_commitment.as_bytes();
    if portable_mir_semantic_commitment != PORTABLE_MIR_SEMANTIC_IDENTITY {
        return Err(CollectedRowSoftmaxErrorV1::PortableMirIdentityMismatch {
            expected: PORTABLE_MIR_SEMANTIC_IDENTITY,
            actual: portable_mir_semantic_commitment,
        });
    }

    let root_instance_identity = tcx.def_path_str(root.instance.def_id());
    if !is_kernel_root_build_identity(&root_instance_identity) {
        return Err(unsupported_collection(format!(
            "root instance must have the exact reviewed kernel-root prefix followed by 64 lowercase ASCII hexadecimal build-identity digits, found `{root_instance_identity}`"
        )));
    }

    let module = canonical_row_softmax_v1_module();
    require_canonical_module(&module)?;
    let canonical_module_commitment = canonical_module_commitment(&module)?;
    let mut authority = RowSoftmaxFrontendAuthorityV1 {
        target: EXACT_ROW_SOFTMAX_TARGET_V1.to_owned(),
        code_object_version: ROW_SOFTMAX_CODE_OBJECT_VERSION_V1,
        explicit_kernarg_bytes: ROW_SOFTMAX_EXPLICIT_KERNARG_BYTES_V1,
        complete_kernarg_bytes: ROW_SOFTMAX_COMPLETE_KERNARG_BYTES_V1,
        row_elements: ROW_SOFTMAX_ELEMENTS_V1,
        abi_binding_commitment: exact_abi_binding_commitment(),
        fn_abi_binding_commitment,
        launch_binding_commitment: exact_launch_binding_commitment(),
        correspondence_commitment: reviewed_correspondence_commitment(),
        exponential_boundary_commitment: exponential_boundary_commitment(),
        frontend_contract_commitment: sha256(EXACT_FRONTEND_CONTRACT_V1),
        canonical_module_commitment,
        kernel_export: root.export_name.clone(),
        root_instance_identity,
        portable_mir_semantic_commitment,
        compiler_semantics_commitment: admitted_compiler_semantics.normalized_commitment,
        cargo_metadata_build_observation: admitted_compiler_semantics
            .cargo_metadata_build_observation,
        provider_authority,
        authority_commitment: [0; 32],
    };
    authority.authority_commitment = collected_authority_commitment(&authority);
    Ok(RowSoftmaxFrontendReceiptV1 {
        authority: Some(authority),
    })
}

fn admit_execution_context(
    target: &str,
    custom_llvm_pipeline: bool,
) -> Result<(), CollectedRowSoftmaxErrorV1> {
    if target != EXACT_ROW_SOFTMAX_TARGET_V1 {
        return Err(CollectedRowSoftmaxErrorV1::WrongTarget {
            actual: target.to_owned(),
        });
    }
    if custom_llvm_pipeline {
        return Err(CollectedRowSoftmaxErrorV1::CustomPipeline);
    }
    Ok(())
}

fn exact_collected_root<'a, 'tcx>(
    functions: &'a [CollectedFunction<'tcx>],
) -> Result<&'a CollectedFunction<'tcx>, CollectedRowSoftmaxErrorV1> {
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

fn require_registration(root: &CollectedFunction<'_>) -> Result<(), CollectedRowSoftmaxErrorV1> {
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
            "kernel registration must be the unique compiler-authenticated General V3 row_softmax_v1 root with its exact WG64 frontend contract",
        ));
    }
    Ok(())
}

fn require_signature<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<(), CollectedRowSoftmaxErrorV1> {
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
        || signature.inputs().len() != 2
    {
        return Err(abi_mismatch(format!(
            "expected safe non-variadic Rust ABI `(&[f32], DisjointSlice<f32>) -> ()`, found `{signature}`"
        )));
    }
    let inputs = signature.inputs();
    if !is_shared_f32_slice(inputs[0]) || !is_disjoint_f32_slice(tcx, inputs[1]) {
        return Err(abi_mismatch(format!(
            "expected exact argument order `input:&[f32], output:DisjointSlice<f32>`, found `{signature}`"
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

fn require_layout(root: &CollectedFunction<'_>) -> Result<(), CollectedRowSoftmaxErrorV1> {
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
            "expected exact row-softmax argument kinds {ARGUMENT_KINDS:?}, found {actual:?}"
        )));
    }
    let abi = contract.abi();
    if abi.size() != ROW_SOFTMAX_EXPLICIT_KERNARG_BYTES_V1
        || abi.alignment() != 8
        || abi.pointer_width() != PointerWidth::Bits64
    {
        return Err(layout_mismatch(format!(
            "explicit kernarg must be exactly 64-bit, 32 bytes aligned to 8, found {:?}, {} bytes aligned to {}",
            abi.pointer_width(),
            abi.size(),
            abi.alignment()
        )));
    }
    let expected_names = ["arg0", "arg1"];
    let expected_offsets = [0, 16];
    if abi.fields().len() != 2 {
        return Err(layout_mismatch(format!(
            "expected two ABI fields, found {}",
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
            return Err(layout_mismatch(format!(
                "field {index} must be {}@{} size 16 align 8 with its rustc-derived type identity, found {}@{} size {} align {}",
                expected_names[index],
                expected_offsets[index],
                field.name().as_str(),
                field.offset(),
                field.size(),
                field.alignment(),
            )));
        }
        let common_slice = matches!(
            field.kind(),
            AbiKind::Slice {
                element_size: 4,
                element_alignment: 4
            }
        ) && field.address_space() == ArtifactAddressSpace::Global;
        match index {
            0 if common_slice
                && field.mutability() == ArtifactMutability::Immutable
                && field.access() == Access::ReadOnly
                && field.ownership() == ArgumentOwnership::SharedBorrow
                && field.alias_class() == AliasClass::SharedReadOnly => {}
            1 if common_slice
                && field.mutability() == ArtifactMutability::Mutable
                && field.access() == Access::ReadWrite
                && field.ownership() == ArgumentOwnership::UniqueBorrow
                && field.alias_class() == AliasClass::Exclusive => {}
            0 => {
                return Err(layout_mismatch(
                    "field input must be an immutable shared &[f32] global slice",
                ));
            }
            1 => {
                return Err(layout_mismatch(
                    "field output must be the unique genuine DisjointSlice<f32> global slice",
                ));
            }
            _ => unreachable!(),
        }
    }

    // General V3 still transports layout under its legacy WG256 contract. The
    // exact WG64 execution requirement is the independently authenticated
    // frontend contract and launch binding below.
    let transport_launch = contract.launch();
    if transport_launch.rank() != 1
        || transport_launch.block_size()
            != BlockSize::Exact(Dimensions::new(256, 1, 1).map_err(|error| {
                layout_mismatch(format!("invalid transport workgroup dimensions: {error}"))
            })?)
        || transport_launch.max_grid()
            != Dimensions::new(u32::MAX, 1, 1).map_err(|error| {
                layout_mismatch(format!("invalid transport grid dimensions: {error}"))
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
    let launch = frontend
        .contract()
        .launch()
        .ok_or_else(|| layout_mismatch("frontend launch declaration is absent"))?;
    if launch.required().map(|value| value.as_array()) != Some([64, 1, 1])
        || launch.maximum().map(|value| value.as_array()) != Some([64, 1, 1])
        || launch.min_workgroups_per_compute_unit().is_some()
        || frontend.contract().unsafe_assembly().is_some()
    {
        return Err(layout_mismatch(
            "frontend contract must be exact required=max=64x1x1 with no occupancy or unsafe assembly declaration",
        ));
    }
    Ok(())
}

fn exact_row_softmax_launch_contract() -> Result<LaunchContract, CollectedRowSoftmaxErrorV1> {
    LaunchContract::new(
        1,
        BlockSize::Exact(
            Dimensions::new(64, 1, 1)
                .map_err(|error| layout_mismatch(format!("invalid WG64 dimensions: {error}")))?,
        ),
        Dimensions::new(1, 1, 1)
            .map_err(|error| layout_mismatch(format!("invalid one-row grid: {error}")))?,
        0,
        0,
    )
    .map_err(|error| layout_mismatch(format!("invalid exact row-softmax launch: {error}")))
}

fn require_rustc_fn_abi<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<[u8; 32], CollectedRowSoftmaxErrorV1> {
    let query = TypingEnv::fully_monomorphized()
        .as_query_input((instance, rustc_middle::ty::List::empty()));
    let abi = tcx
        .fn_abi_of_instance(query)
        .map_err(|error| abi_mismatch(format!("rustc FnAbi query failed: {error:?}")))?;
    if abi.conv != CanonAbi::Rust
        || abi.c_variadic
        || abi.fixed_count != 2
        || abi.args.len() != 2
        || !matches!(abi.ret.mode, PassMode::Ignore)
        || abi.ret.layout.size.bytes() != 0
    {
        return Err(abi_mismatch(format!(
            "rustc FnAbi header must be exact Rust(args=2)->unit, found {abi:?}"
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
        return Err(CollectedRowSoftmaxErrorV1::FnAbiIdentityMismatch {
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
) -> Result<AdmittedCompilerSemanticsV1, CollectedRowSoftmaxErrorV1> {
    let expected_mir_passes = [("JumpThreading".to_owned(), false)];
    let cargo_metadata_build_observation =
        CargoMetadataBuildObservationV1::from_ordered_tokens(&observed.crate_metadata);
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
    } else if let Err(detail) = &cargo_metadata_build_observation {
        Some(detail.clone())
    } else if observed.remap_path_destinations != ["/fe2o3-reviewed-workspace/row-softmax-v1.rs"] {
        Some(format!(
            "source remapping must contain exactly the canonical row-softmax fixture destination, found {:?}",
            observed.remap_path_destinations
        ))
    } else {
        None
    };
    if let Some(detail) = mismatch {
        return Err(CollectedRowSoftmaxErrorV1::CompilerSemantics { detail });
    }
    Ok(AdmittedCompilerSemanticsV1 {
        normalized_commitment: compiler_semantics_commitment(observed),
        cargo_metadata_build_observation: cargo_metadata_build_observation
            .expect("metadata shape checked above"),
    })
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
    // Cargo's generated token is build context, not portable source semantics.
    // Admission validates its shape; the private receipt binds its exact value.
    hash_field(&mut digest, CARGO_GENERATED_METADATA_SHAPE_V1);
    hash_field(
        &mut digest,
        observed.crate_metadata.get(1).map_or(&[], String::as_bytes),
    );
    for destination in &observed.remap_path_destinations {
        hash_field(&mut digest, destination.as_bytes());
    }
    digest.finalize().into()
}

fn reviewed_compiler_semantics(generated_cargo_metadata: &str) -> CompilerSemanticsV1 {
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
            generated_cargo_metadata.to_owned(),
            REVIEWED_CRATE_METADATA.to_owned(),
        ],
        remap_path_destinations: vec!["/fe2o3-reviewed-workspace/row-softmax-v1.rs".to_owned()],
    }
}

fn is_kernel_root_build_identity(value: &str) -> bool {
    value
        .strip_prefix(KERNEL_ROOT_BUILD_IDENTITY_PREFIX)
        .is_some_and(|suffix| is_lowercase_ascii_hex(suffix, 64))
}

fn is_lowercase_ascii_hex(value: &str, width: usize) -> bool {
    value.len() == width
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn row_softmax_target_identity() -> Result<TargetIdentity, CollectedRowSoftmaxErrorV1> {
    TargetIdentity::new(
        IdentityText::new(dialect_amdgcn::AMDGPU_TRIPLE)
            .map_err(|error| unsupported_collection(format!("invalid AMDGPU triple: {error}")))?,
        IdentityText::new(EXACT_ROW_SOFTMAX_TARGET_V1)
            .map_err(|error| unsupported_collection(format!("invalid gfx942 profile: {error}")))?,
        PointerWidth::Bits64,
        Endianness::Little,
        vec![Capability::AmdWave],
    )
    .map_err(|error| unsupported_collection(format!("invalid target identity: {error}")))
}

fn canonical_row_softmax_v1_module() -> Module {
    let input_slice = Type::slice(Type::F32, AddressSpace::Global, AccessMode::ReadOnly);
    let output_slice = Type::slice(Type::F32, AddressSpace::Global, AccessMode::ReadWrite);
    let input_pointer = Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadOnly);
    let output_pointer = Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadWrite);
    let access = MemoryAccess::new(AddressSpace::Global, 4);

    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        value_op(
            2,
            input_pointer.clone(),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        value_op(
            3,
            output_pointer.clone(),
            OperationKind::SliceData { slice: ValueId(1) },
        ),
        value_op(
            4,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Local,
                    axis: Axis::X,
                },
                Type::INDEX,
            )),
        ),
        value_op(5, Type::INDEX, OperationKind::Constant(Constant::Index(0))),
        value_op(6, Type::BOOL, compare(ComparePredicate::Equal, 4, 5)),
        value_op(
            7,
            Type::INDEX,
            OperationKind::Constant(Constant::Index(u64::from(ROW_SOFTMAX_ELEMENTS_V1))),
        ),
        value_op(8, Type::INDEX, OperationKind::Constant(Constant::Index(1))),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(6),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(10),
        else_arguments: vec![],
    });

    let mut max_init = BasicBlock::new(BlockId(1));
    max_init.operations = vec![value_op(
        9,
        Type::F32,
        OperationKind::Constant(Constant::F32Bits(f32::NEG_INFINITY.to_bits())),
    )];
    max_init.terminator = Some(Terminator::Branch {
        target: BlockId(2),
        arguments: vec![ValueId(5), ValueId(9)],
    });

    let mut max_header = BasicBlock::new(BlockId(2));
    max_header.parameters = vec![
        ValueDef::new(ValueId(10), Type::INDEX),
        ValueDef::new(ValueId(11), Type::F32),
    ];
    max_header.operations = vec![value_op(
        12,
        Type::BOOL,
        compare(ComparePredicate::LessThan, 10, 7),
    )];
    max_header.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(12),
        then_target: BlockId(3),
        then_arguments: vec![],
        else_target: BlockId(4),
        else_arguments: vec![ValueId(11)],
    });

    let mut max_body = BasicBlock::new(BlockId(3));
    max_body.operations = vec![
        value_op(
            13,
            input_pointer.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(2),
                offset: ValueId(10),
            },
        ),
        value_op(
            14,
            Type::F32,
            OperationKind::Load {
                pointer: ValueId(13),
                access,
            },
        ),
        value_op(
            15,
            Type::BOOL,
            compare(ComparePredicate::GreaterThan, 14, 11),
        ),
        value_op(
            16,
            Type::F32,
            OperationKind::Select {
                condition: ValueId(15),
                true_value: ValueId(14),
                false_value: ValueId(11),
            },
        ),
        value_op(17, Type::INDEX, binary(BinaryOp::Add, 10, 8)),
    ];
    max_body.terminator = Some(Terminator::Branch {
        target: BlockId(2),
        arguments: vec![ValueId(17), ValueId(16)],
    });

    let mut sum_init = BasicBlock::new(BlockId(4));
    sum_init.parameters = vec![ValueDef::new(ValueId(18), Type::F32)];
    sum_init.operations = vec![value_op(
        19,
        Type::F32,
        OperationKind::Constant(Constant::F32Bits(0.0_f32.to_bits())),
    )];
    sum_init.terminator = Some(Terminator::Branch {
        target: BlockId(5),
        arguments: vec![ValueId(5), ValueId(19), ValueId(18)],
    });

    let mut sum_header = BasicBlock::new(BlockId(5));
    sum_header.parameters = vec![
        ValueDef::new(ValueId(20), Type::INDEX),
        ValueDef::new(ValueId(21), Type::F32),
        ValueDef::new(ValueId(22), Type::F32),
    ];
    sum_header.operations = vec![value_op(
        23,
        Type::BOOL,
        compare(ComparePredicate::LessThan, 20, 7),
    )];
    sum_header.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(23),
        then_target: BlockId(6),
        then_arguments: vec![],
        else_target: BlockId(7),
        else_arguments: vec![ValueId(22), ValueId(21)],
    });

    let sum_exp = exp_operation(27, 26);
    let mut sum_body = BasicBlock::new(BlockId(6));
    sum_body.operations = vec![
        value_op(
            24,
            input_pointer.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(2),
                offset: ValueId(20),
            },
        ),
        value_op(
            25,
            Type::F32,
            OperationKind::Load {
                pointer: ValueId(24),
                access,
            },
        ),
        value_op(26, Type::F32, binary(BinaryOp::Subtract, 25, 22)),
        sum_exp.clone(),
        value_op(28, Type::F32, binary(BinaryOp::Add, 21, 27)),
        value_op(29, Type::INDEX, binary(BinaryOp::Add, 20, 8)),
    ];
    sum_body.terminator = Some(Terminator::Branch {
        target: BlockId(5),
        arguments: vec![ValueId(29), ValueId(28), ValueId(22)],
    });

    let mut store_init = BasicBlock::new(BlockId(7));
    store_init.parameters = vec![
        ValueDef::new(ValueId(30), Type::F32),
        ValueDef::new(ValueId(31), Type::F32),
    ];
    store_init.terminator = Some(Terminator::Branch {
        target: BlockId(8),
        arguments: vec![ValueId(5), ValueId(30), ValueId(31)],
    });

    let mut store_header = BasicBlock::new(BlockId(8));
    store_header.parameters = vec![
        ValueDef::new(ValueId(32), Type::INDEX),
        ValueDef::new(ValueId(33), Type::F32),
        ValueDef::new(ValueId(34), Type::F32),
    ];
    store_header.operations = vec![value_op(
        35,
        Type::BOOL,
        compare(ComparePredicate::LessThan, 32, 7),
    )];
    store_header.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(35),
        then_target: BlockId(9),
        then_arguments: vec![],
        else_target: BlockId(11),
        else_arguments: vec![],
    });

    let store_exp = exp_operation(39, 38);
    let mut store_body = BasicBlock::new(BlockId(9));
    store_body.operations = vec![
        value_op(
            36,
            input_pointer,
            OperationKind::GetElementPointer {
                base: ValueId(2),
                offset: ValueId(32),
            },
        ),
        value_op(
            37,
            Type::F32,
            OperationKind::Load {
                pointer: ValueId(36),
                access,
            },
        ),
        value_op(38, Type::F32, binary(BinaryOp::Subtract, 37, 33)),
        store_exp,
        value_op(40, Type::F32, binary(BinaryOp::Divide, 39, 34)),
        value_op(
            41,
            output_pointer,
            OperationKind::GetElementPointer {
                base: ValueId(3),
                offset: ValueId(32),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(41),
                value: ValueId(40),
                access,
            },
        ),
        value_op(42, Type::INDEX, binary(BinaryOp::Add, 32, 8)),
    ];
    store_body.terminator = Some(Terminator::Branch {
        target: BlockId(8),
        arguments: vec![ValueId(42), ValueId(33), ValueId(34)],
    });

    let mut inactive = BasicBlock::new(BlockId(10));
    inactive.terminator = Some(Terminator::Return { values: vec![] });
    let mut done = BasicBlock::new(BlockId(11));
    done.terminator = Some(Terminator::Return { values: vec![] });

    let capabilities = exact_capabilities();
    let mut function = Function::kernel_entry(
        CANONICAL_FUNCTION_ID,
        Signature::new(vec![input_slice, output_slice], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![
            entry,
            max_init,
            max_header,
            max_body,
            sum_init,
            sum_header,
            sum_body,
            store_init,
            store_header,
            store_body,
            inactive,
            done,
        ],
    );
    function.required_capabilities = capabilities.clone();

    let mut kernel = Kernel::new(
        ROW_SOFTMAX_KERNEL_SYMBOL_V1,
        CANONICAL_FUNCTION_ID,
        LaunchDomain::D1 {
            x: LaunchExtent::Static(ROW_SOFTMAX_ELEMENTS_V1),
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    kernel.required_capabilities = capabilities.clone();

    let mut module = Module::new(CANONICAL_MODULE_ID);
    module.required_capabilities = capabilities;
    module.functions.push(function);
    module.functions.push(
        FloatOperation::F32Math {
            function: F32MathFunction::Exp,
            implementation: F32MathFunction::Exp.required_implementation(),
            arguments: vec![ValueId(26)],
        }
        .declaration(),
    );
    module.kernels.push(kernel);
    module
}

fn require_canonical_module(module: &Module) -> Result<(), CollectedRowSoftmaxErrorV1> {
    verify_module(module).map_err(|error| CollectedRowSoftmaxErrorV1::CanonicalModule {
        detail: error.to_string(),
    })?;
    let actual_commitment = canonical_module_commitment(module)?;
    if actual_commitment != REVIEWED_CANONICAL_MODULE_V4_COMMITMENT {
        return Err(CollectedRowSoftmaxErrorV1::CanonicalModule {
            detail: format!(
                "V4 module commitment differs from the independently reviewed digest: expected {}, found {}",
                encode_hex(&REVIEWED_CANONICAL_MODULE_V4_COMMITMENT),
                encode_hex(&actual_commitment),
            ),
        });
    }
    if module != &canonical_row_softmax_v1_module() {
        return Err(CollectedRowSoftmaxErrorV1::CanonicalModule {
            detail: "module differs from the exact private row-softmax V1 graph".to_owned(),
        });
    }
    Ok(())
}

fn canonical_module_commitment(module: &Module) -> Result<[u8; 32], CollectedRowSoftmaxErrorV1> {
    let bytes =
        encode_module_v4(module).map_err(|error| CollectedRowSoftmaxErrorV1::CanonicalModule {
            detail: format!("V4 wire encoding failed: {error}"),
        })?;
    Ok(domain_commitment(MODULE_BINDING_DOMAIN_V1, &bytes))
}

fn exact_capabilities() -> BTreeSet<TargetCapability> {
    BTreeSet::from([
        gfx942_xnack_minus_target_capability(),
        TargetCapability::WaveWidth(WaveWidth::Wave64),
    ])
}

fn value_op(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}

fn binary(op: BinaryOp, lhs: u32, rhs: u32) -> OperationKind {
    OperationKind::Binary {
        op,
        lhs: ValueId(lhs),
        rhs: ValueId(rhs),
    }
}

fn compare(predicate: ComparePredicate, lhs: u32, rhs: u32) -> OperationKind {
    OperationKind::Compare {
        predicate,
        lhs: ValueId(lhs),
        rhs: ValueId(rhs),
    }
}

fn exp_operation(result: u32, argument: u32) -> Operation {
    FloatOperation::F32Math {
        function: F32MathFunction::Exp,
        implementation: F32MathFunction::Exp.required_implementation(),
        arguments: vec![ValueId(argument)],
    }
    .operation(ValueId(result))
}

fn collected_authority_commitment(authority: &RowSoftmaxFrontendAuthorityV1) -> [u8; 32] {
    let mut digest = Sha256::new();
    hash_field(&mut digest, COLLECTED_AUTHORITY_DOMAIN_V1);
    hash_field(&mut digest, &authority.portable_mir_semantic_commitment);
    hash_field(&mut digest, &authority.compiler_semantics_commitment);
    hash_field(&mut digest, &authority.canonical_module_commitment);
    hash_field(&mut digest, authority.root_instance_identity.as_bytes());
    hash_field(&mut digest, authority.kernel_export.as_bytes());
    hash_field(&mut digest, authority.target.as_bytes());
    hash_field(&mut digest, &authority.code_object_version.to_le_bytes());
    hash_field(&mut digest, &authority.explicit_kernarg_bytes.to_le_bytes());
    hash_field(&mut digest, &authority.complete_kernarg_bytes.to_le_bytes());
    hash_field(&mut digest, &authority.row_elements.to_le_bytes());
    hash_field(&mut digest, &authority.abi_binding_commitment);
    hash_field(&mut digest, &authority.fn_abi_binding_commitment);
    hash_field(&mut digest, &authority.launch_binding_commitment);
    hash_field(&mut digest, &authority.correspondence_commitment);
    hash_field(&mut digest, &authority.exponential_boundary_commitment);
    hash_field(&mut digest, &authority.frontend_contract_commitment);
    for token in &authority.cargo_metadata_build_observation.ordered_tokens {
        hash_field(&mut digest, token.as_bytes());
    }
    hash_field(
        &mut digest,
        &authority.cargo_metadata_build_observation.commitment,
    );
    hash_field(
        &mut digest,
        authority.provider_authority.provider.crate_name.as_bytes(),
    );
    hash_field(
        &mut digest,
        &authority
            .provider_authority
            .provider
            .stable_crate_id
            .to_le_bytes(),
    );
    hash_field(
        &mut digest,
        &authority.provider_authority.provider.crate_hash,
    );
    hash_field(
        &mut digest,
        &authority
            .provider_authority
            .provider
            .cargo_metadata_build_observation,
    );
    hash_field(
        &mut digest,
        &authority.provider_authority.provider.source_identity,
    );
    for identity in &authority.provider_authority.definition_identities {
        hash_field(&mut digest, identity);
    }
    for identity in &authority.provider_authority.source_identities {
        hash_field(&mut digest, identity);
    }
    hash_field(&mut digest, &authority.provider_authority.commitment);
    digest.finalize().into()
}

fn validate_frontend_authority(
    authority: &RowSoftmaxFrontendAuthorityV1,
) -> Result<(), CollectedRowSoftmaxErrorV1> {
    let metadata_observation_is_invalid = authority
        .cargo_metadata_build_observation
        .validate()
        .is_err();
    let provider_authority_is_invalid = authority.provider_authority.validate().is_err();
    let field = if authority.target != EXACT_ROW_SOFTMAX_TARGET_V1 {
        Some("target")
    } else if authority.code_object_version != ROW_SOFTMAX_CODE_OBJECT_VERSION_V1 {
        Some("code-object version")
    } else if authority.explicit_kernarg_bytes != ROW_SOFTMAX_EXPLICIT_KERNARG_BYTES_V1
        || authority.complete_kernarg_bytes != ROW_SOFTMAX_COMPLETE_KERNARG_BYTES_V1
    {
        Some("kernarg ABI sizes")
    } else if authority.row_elements != ROW_SOFTMAX_ELEMENTS_V1 {
        Some("row extent")
    } else if authority.abi_binding_commitment != exact_abi_binding_commitment() {
        Some("explicit ABI")
    } else if authority.fn_abi_binding_commitment != RUSTC_FN_ABI_IDENTITY {
        Some("rustc FnAbi")
    } else if authority.launch_binding_commitment != exact_launch_binding_commitment() {
        Some("launch contract")
    } else if authority.correspondence_commitment != reviewed_correspondence_commitment() {
        Some("reviewed source-to-canonical-module correspondence")
    } else if authority.exponential_boundary_commitment != exponential_boundary_commitment() {
        Some("unresolved exponential boundary")
    } else if authority.frontend_contract_commitment != sha256(EXACT_FRONTEND_CONTRACT_V1) {
        Some("frontend contract")
    } else if authority.canonical_module_commitment != REVIEWED_CANONICAL_MODULE_V4_COMMITMENT {
        Some("canonical module")
    } else if authority.kernel_export != FIXED_KERNEL_EXPORT {
        Some("kernel export")
    } else if !is_kernel_root_build_identity(&authority.root_instance_identity) {
        Some("root instance")
    } else if authority.portable_mir_semantic_commitment != PORTABLE_MIR_SEMANTIC_IDENTITY {
        Some("portable MIR")
    } else if authority.compiler_semantics_commitment
        != compiler_semantics_commitment(&reviewed_compiler_semantics(""))
    {
        Some("compiler semantics")
    } else if metadata_observation_is_invalid {
        Some("ordered Cargo metadata build observation")
    } else if provider_authority_is_invalid {
        Some("row-softmax trusted provider authority")
    } else {
        None
    };
    if let Some(field) = field {
        return Err(CollectedRowSoftmaxErrorV1::ReceiptBindingMismatch { field });
    }
    if authority.authority_commitment != collected_authority_commitment(authority) {
        return Err(CollectedRowSoftmaxErrorV1::ReceiptBindingMismatch {
            field: "authority commitment",
        });
    }
    Ok(())
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

fn exponential_boundary_commitment() -> [u8; 32] {
    domain_commitment(EXPONENTIAL_BOUNDARY_DOMAIN_V1, EXPONENTIAL_BOUNDARY_V1)
}

fn domain_commitment(domain: &[u8], value: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    hash_field(&mut digest, domain);
    hash_field(&mut digest, value);
    digest.finalize().into()
}

#[cfg(test)]
fn exact_frontend_receipt_for_test() -> RowSoftmaxFrontendReceiptV1 {
    let module = canonical_row_softmax_v1_module();
    let compiler_semantics = reviewed_compiler_semantics("0123456789abcdef");
    let admitted_compiler_semantics =
        require_compiler_semantics(&compiler_semantics).expect("reviewed compiler semantics");
    let mut authority = RowSoftmaxFrontendAuthorityV1 {
        target: EXACT_ROW_SOFTMAX_TARGET_V1.to_owned(),
        code_object_version: ROW_SOFTMAX_CODE_OBJECT_VERSION_V1,
        explicit_kernarg_bytes: ROW_SOFTMAX_EXPLICIT_KERNARG_BYTES_V1,
        complete_kernarg_bytes: ROW_SOFTMAX_COMPLETE_KERNARG_BYTES_V1,
        row_elements: ROW_SOFTMAX_ELEMENTS_V1,
        abi_binding_commitment: exact_abi_binding_commitment(),
        fn_abi_binding_commitment: RUSTC_FN_ABI_IDENTITY,
        launch_binding_commitment: exact_launch_binding_commitment(),
        correspondence_commitment: reviewed_correspondence_commitment(),
        exponential_boundary_commitment: exponential_boundary_commitment(),
        frontend_contract_commitment: sha256(EXACT_FRONTEND_CONTRACT_V1),
        canonical_module_commitment: canonical_module_commitment(&module)
            .expect("canonical test module"),
        kernel_export: FIXED_KERNEL_EXPORT.to_owned(),
        root_instance_identity: REPRESENTATIVE_ROOT_INSTANCE_IDENTITY.to_owned(),
        portable_mir_semantic_commitment: PORTABLE_MIR_SEMANTIC_IDENTITY,
        compiler_semantics_commitment: admitted_compiler_semantics.normalized_commitment,
        cargo_metadata_build_observation: admitted_compiler_semantics
            .cargo_metadata_build_observation,
        provider_authority: crate::mir_import::RowSoftmaxProviderAuthorityV1::canonical_for_test(),
        authority_commitment: [0; 32],
    };
    authority.authority_commitment = collected_authority_commitment(&authority);
    RowSoftmaxFrontendReceiptV1 {
        authority: Some(authority),
    }
}

fn hash_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn unsupported_collection(detail: impl Into<String>) -> CollectedRowSoftmaxErrorV1 {
    CollectedRowSoftmaxErrorV1::UnsupportedCollection {
        detail: detail.into(),
    }
}

fn abi_mismatch(detail: impl Into<String>) -> CollectedRowSoftmaxErrorV1 {
    CollectedRowSoftmaxErrorV1::AbiMismatch {
        detail: detail.into(),
    }
}

fn layout_mismatch(detail: impl Into<String>) -> CollectedRowSoftmaxErrorV1 {
    CollectedRowSoftmaxErrorV1::LayoutMismatch {
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

    #[derive(Debug, Eq, PartialEq)]
    enum ReviewedRowSoftmaxOperation {
        SliceData(u32),
        LocalInvocationIndexX,
        Constant(Constant),
        Compare(ComparePredicate, u32, u32),
        Select(u32, u32, u32),
        Binary(BinaryOp, u32, u32),
        GetElementPointer(u32, u32),
        Load(u32, MemoryAccess),
        AbstractExp(u32),
        Store(u32, u32, MemoryAccess),
    }

    type ReviewedRowSoftmaxBlock = (
        u32,
        Vec<(u32, Type)>,
        Vec<(Option<u32>, ReviewedRowSoftmaxOperation)>,
        Terminator,
    );
    type ReceiptMutation = (fn(&mut RowSoftmaxFrontendAuthorityV1), &'static str);

    fn reviewed_operation(operation: &Operation) -> (Option<u32>, ReviewedRowSoftmaxOperation) {
        let result = match operation.results.as_slice() {
            [] => None,
            [result] => Some(result.id.0),
            results => panic!(
                "reviewed row-softmax operation has {} results",
                results.len()
            ),
        };
        let kind = match &operation.kind {
            OperationKind::SliceData { slice } => ReviewedRowSoftmaxOperation::SliceData(slice.0),
            OperationKind::Intrinsic(intrinsic)
                if intrinsic.kind
                    == (IntrinsicKind::InvocationIndex {
                        kind: IndexKind::Local,
                        axis: Axis::X,
                    })
                    && intrinsic.result_type == Type::INDEX =>
            {
                ReviewedRowSoftmaxOperation::LocalInvocationIndexX
            }
            OperationKind::Constant(constant) => {
                ReviewedRowSoftmaxOperation::Constant(constant.clone())
            }
            OperationKind::Compare {
                predicate,
                lhs,
                rhs,
            } => ReviewedRowSoftmaxOperation::Compare(*predicate, lhs.0, rhs.0),
            OperationKind::Select {
                condition,
                true_value,
                false_value,
            } => ReviewedRowSoftmaxOperation::Select(condition.0, true_value.0, false_value.0),
            OperationKind::Binary { op, lhs, rhs } => {
                ReviewedRowSoftmaxOperation::Binary(*op, lhs.0, rhs.0)
            }
            OperationKind::GetElementPointer { base, offset } => {
                ReviewedRowSoftmaxOperation::GetElementPointer(base.0, offset.0)
            }
            OperationKind::Load { pointer, access } => {
                ReviewedRowSoftmaxOperation::Load(pointer.0, *access)
            }
            OperationKind::Call { callee, arguments }
                if callee.as_str() == "__fe2o3_ir_float_v1_exp_f32" && arguments.len() == 1 =>
            {
                ReviewedRowSoftmaxOperation::AbstractExp(arguments[0].0)
            }
            OperationKind::Store {
                pointer,
                value,
                access,
            } => ReviewedRowSoftmaxOperation::Store(pointer.0, value.0, *access),
            unexpected => panic!("unexpected reviewed row-softmax operation: {unexpected:?}"),
        };
        (result, kind)
    }

    fn parameter_oracle(block: &BasicBlock) -> Vec<(u32, Type)> {
        block
            .parameters
            .iter()
            .map(|parameter| (parameter.id.0, parameter.ty.clone()))
            .collect()
    }

    #[test]
    fn exact_profile_is_closed_and_non_executable() {
        assert!(admit_execution_context(EXACT_ROW_SOFTMAX_TARGET_V1, false).is_ok());
        for target in [
            "gfx942",
            "gfx942:xnack+",
            "gfx942:sramecc+:xnack-",
            "gfx941:xnack-",
            "gfx950:xnack-",
        ] {
            assert!(matches!(
                admit_execution_context(target, false),
                Err(CollectedRowSoftmaxErrorV1::WrongTarget { .. })
            ));
        }
        assert!(matches!(
            admit_execution_context(EXACT_ROW_SOFTMAX_TARGET_V1, true),
            Err(CollectedRowSoftmaxErrorV1::CustomPipeline)
        ));
        assert_eq!(ROW_SOFTMAX_ELEMENTS_V1, 64);
        assert_eq!(ROW_SOFTMAX_EXPLICIT_KERNARG_BYTES_V1, 32);
        assert_eq!(ROW_SOFTMAX_COMPLETE_KERNARG_BYTES_V1, 288);
        assert!(
            EXPONENTIAL_BOUNDARY_V1
                .windows(4)
                .all(|bytes| bytes != b"COMG")
        );
    }

    #[test]
    fn canonical_module_is_exact_but_exp_implementation_remains_unresolved() {
        let module = canonical_row_softmax_v1_module();
        require_canonical_module(&module).expect("canonical row-softmax module");
        assert_eq!(
            canonical_module_commitment(&module).expect("canonical V4 commitment"),
            REVIEWED_CANONICAL_MODULE_V4_COMMITMENT
        );
        assert_eq!(module.id.as_str(), CANONICAL_MODULE_ID);
        assert_eq!(module.kernels.len(), 1);
        assert_eq!(module.functions.len(), 2);
        assert_eq!(
            module.kernels[0].workgroup_size,
            Some(WorkgroupSize::new(64, 1, 1))
        );
        assert_eq!(
            module.kernels[0].domain,
            LaunchDomain::D1 {
                x: LaunchExtent::Static(64)
            }
        );
        let encoded = encode_module_v4(&module).expect("canonical V4 wire module");
        assert!(!encoded.is_empty());
        assert!(
            module
                .functions
                .iter()
                .any(|function| function.id.as_str() == "__fe2o3_ir_float_v1_exp_f32")
        );
        assert!(
            module
                .functions
                .iter()
                .all(|function| !function.id.as_str().contains("__ocml"))
        );
    }

    #[test]
    fn canonical_graph_matches_the_independent_fixed_row_algorithm_oracle() {
        use ReviewedRowSoftmaxOperation as Op;

        let module = canonical_row_softmax_v1_module();
        let function = module
            .functions
            .iter()
            .find(|function| function.id.as_str() == CANONICAL_FUNCTION_ID)
            .expect("canonical row-softmax entry");
        let body = function.body.as_ref().expect("defined row-softmax entry");
        assert_eq!(body.parameters, [ValueId(0), ValueId(1)]);
        let global_f32 = MemoryAccess::new(AddressSpace::Global, 4);
        let expected: Vec<ReviewedRowSoftmaxBlock> = vec![
            (
                0,
                vec![],
                vec![
                    (Some(2), Op::SliceData(0)),
                    (Some(3), Op::SliceData(1)),
                    (Some(4), Op::LocalInvocationIndexX),
                    (Some(5), Op::Constant(Constant::Index(0))),
                    (Some(6), Op::Compare(ComparePredicate::Equal, 4, 5)),
                    (Some(7), Op::Constant(Constant::Index(64))),
                    (Some(8), Op::Constant(Constant::Index(1))),
                ],
                Terminator::ConditionalBranch {
                    condition: ValueId(6),
                    then_target: BlockId(1),
                    then_arguments: vec![],
                    else_target: BlockId(10),
                    else_arguments: vec![],
                },
            ),
            (
                1,
                vec![],
                vec![(
                    Some(9),
                    Op::Constant(Constant::F32Bits(f32::NEG_INFINITY.to_bits())),
                )],
                Terminator::Branch {
                    target: BlockId(2),
                    arguments: vec![ValueId(5), ValueId(9)],
                },
            ),
            (
                2,
                vec![(10, Type::INDEX), (11, Type::F32)],
                vec![(Some(12), Op::Compare(ComparePredicate::LessThan, 10, 7))],
                Terminator::ConditionalBranch {
                    condition: ValueId(12),
                    then_target: BlockId(3),
                    then_arguments: vec![],
                    else_target: BlockId(4),
                    else_arguments: vec![ValueId(11)],
                },
            ),
            (
                3,
                vec![],
                vec![
                    (Some(13), Op::GetElementPointer(2, 10)),
                    (Some(14), Op::Load(13, global_f32)),
                    (Some(15), Op::Compare(ComparePredicate::GreaterThan, 14, 11)),
                    (Some(16), Op::Select(15, 14, 11)),
                    (Some(17), Op::Binary(BinaryOp::Add, 10, 8)),
                ],
                Terminator::Branch {
                    target: BlockId(2),
                    arguments: vec![ValueId(17), ValueId(16)],
                },
            ),
            (
                4,
                vec![(18, Type::F32)],
                vec![(Some(19), Op::Constant(Constant::F32Bits(0.0_f32.to_bits())))],
                Terminator::Branch {
                    target: BlockId(5),
                    arguments: vec![ValueId(5), ValueId(19), ValueId(18)],
                },
            ),
            (
                5,
                vec![(20, Type::INDEX), (21, Type::F32), (22, Type::F32)],
                vec![(Some(23), Op::Compare(ComparePredicate::LessThan, 20, 7))],
                Terminator::ConditionalBranch {
                    condition: ValueId(23),
                    then_target: BlockId(6),
                    then_arguments: vec![],
                    else_target: BlockId(7),
                    else_arguments: vec![ValueId(22), ValueId(21)],
                },
            ),
            (
                6,
                vec![],
                vec![
                    (Some(24), Op::GetElementPointer(2, 20)),
                    (Some(25), Op::Load(24, global_f32)),
                    (Some(26), Op::Binary(BinaryOp::Subtract, 25, 22)),
                    (Some(27), Op::AbstractExp(26)),
                    (Some(28), Op::Binary(BinaryOp::Add, 21, 27)),
                    (Some(29), Op::Binary(BinaryOp::Add, 20, 8)),
                ],
                Terminator::Branch {
                    target: BlockId(5),
                    arguments: vec![ValueId(29), ValueId(28), ValueId(22)],
                },
            ),
            (
                7,
                vec![(30, Type::F32), (31, Type::F32)],
                vec![],
                Terminator::Branch {
                    target: BlockId(8),
                    arguments: vec![ValueId(5), ValueId(30), ValueId(31)],
                },
            ),
            (
                8,
                vec![(32, Type::INDEX), (33, Type::F32), (34, Type::F32)],
                vec![(Some(35), Op::Compare(ComparePredicate::LessThan, 32, 7))],
                Terminator::ConditionalBranch {
                    condition: ValueId(35),
                    then_target: BlockId(9),
                    then_arguments: vec![],
                    else_target: BlockId(11),
                    else_arguments: vec![],
                },
            ),
            (
                9,
                vec![],
                vec![
                    (Some(36), Op::GetElementPointer(2, 32)),
                    (Some(37), Op::Load(36, global_f32)),
                    (Some(38), Op::Binary(BinaryOp::Subtract, 37, 33)),
                    (Some(39), Op::AbstractExp(38)),
                    (Some(40), Op::Binary(BinaryOp::Divide, 39, 34)),
                    (Some(41), Op::GetElementPointer(3, 32)),
                    (None, Op::Store(41, 40, global_f32)),
                    (Some(42), Op::Binary(BinaryOp::Add, 32, 8)),
                ],
                Terminator::Branch {
                    target: BlockId(8),
                    arguments: vec![ValueId(42), ValueId(33), ValueId(34)],
                },
            ),
            (10, vec![], vec![], Terminator::Return { values: vec![] }),
            (11, vec![], vec![], Terminator::Return { values: vec![] }),
        ];

        assert_eq!(body.blocks.len(), expected.len());
        for (block, (id, parameters, operations, terminator)) in body.blocks.iter().zip(expected) {
            assert_eq!(block.id, BlockId(id));
            assert_eq!(parameter_oracle(block), parameters, "parameters in bb{id}");
            assert_eq!(
                block
                    .operations
                    .iter()
                    .map(reviewed_operation)
                    .collect::<Vec<_>>(),
                operations,
                "operations in bb{id}"
            );
            assert_eq!(block.terminator.as_ref(), Some(&terminator), "bb{id}");
        }

        // The oracle names abstract exp calls only. It proves graph ordering and
        // loop carries, not any exponential approximation or exceptional policy.
        assert_eq!(
            body.blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter(|operation| matches!(reviewed_operation(operation).1, Op::AbstractExp(_)))
                .count(),
            2
        );
    }

    #[test]
    fn receipt_is_private_single_use_and_selects_only_the_canonical_module() {
        let mut receipt = exact_frontend_receipt_for_test();
        let authenticated = receipt.consume().expect("first consumption");
        let (module, authority, exponential) = authenticated.into_parts();
        assert_eq!(module, canonical_row_softmax_v1_module());
        assert_ne!(authority, [0; 32]);
        assert_eq!(exponential, exponential_boundary_commitment());
        assert!(matches!(
            receipt.consume(),
            Err(CollectedRowSoftmaxErrorV1::ReceiptAlreadyConsumed)
        ));
    }

    #[test]
    fn kernel_root_build_identity_is_shape_checked_and_fully_receipt_bound() {
        let alternate = "__fe2o3_host_kernel_v1_87e4e114a09ea2b2153fa733dc5925596413c32908cb28f2cc773ff0b3f5102a";
        for identity in [
            REPRESENTATIVE_ROOT_INSTANCE_IDENTITY,
            alternate,
            "__fe2o3_host_kernel_v1_fb3c5857a55066c483e6777719ae5972e44f2128e5fd7146cd6078f502de2b46",
        ] {
            assert!(is_kernel_root_build_identity(identity));
        }
        for identity in [
            "__fe2o3_host_kernel_v1_",
            "__fe2o3_host_kernel_v1_87e4e114a09ea2b2153fa733dc5925596413c32908cb28f2cc773ff0b3f5102",
            "__fe2o3_host_kernel_v1_87e4e114a09ea2b2153fa733dc5925596413c32908cb28f2cc773ff0b3f5102aa",
            "__fe2o3_host_kernel_v1_87E4E114A09EA2B2153FA733DC5925596413C32908CB28F2CC773FF0B3F5102A",
            "__fe2o3_host_kernel_v1_87e4e114a09ea2b2153fa733dc5925596413c32908cb28f2cc773ff0b3f5102g",
            "module::__fe2o3_host_kernel_v1_87e4e114a09ea2b2153fa733dc5925596413c32908cb28f2cc773ff0b3f5102a",
            "__fe2o3_host_kernel_v2_87e4e114a09ea2b2153fa733dc5925596413c32908cb28f2cc773ff0b3f5102a",
            "__fe2o3_host_kernel_v1_87e4e114a09ea2b2153fa733dc5925596413c32908cb28f2cc773ff0b3f5102a\u{200e}",
        ] {
            assert!(!is_kernel_root_build_identity(identity), "{identity:?}");
        }

        let baseline = exact_frontend_receipt_for_test();
        let baseline_commitment = baseline
            .authority
            .as_ref()
            .expect("baseline authority")
            .authority_commitment;
        let mut alternate_receipt = exact_frontend_receipt_for_test();
        let authority = alternate_receipt
            .authority
            .as_mut()
            .expect("test authority");
        authority.root_instance_identity = alternate.to_owned();
        authority.authority_commitment = collected_authority_commitment(authority);
        assert_ne!(authority.authority_commitment, baseline_commitment);
        assert!(validate_frontend_authority(authority).is_ok());
        alternate_receipt
            .consume()
            .expect("alternate well-shaped generated root remains fully receipt-bound");
    }

    #[test]
    fn resigned_receipt_mutations_fail_at_the_exact_individual_binding() {
        let baseline_receipt = exact_frontend_receipt_for_test();
        let baseline = collected_authority_commitment(
            baseline_receipt.authority.as_ref().expect("test authority"),
        );
        let mutations: [ReceiptMutation; 19] = [
            (
                |value| value.portable_mir_semantic_commitment[0] ^= 1,
                "portable MIR",
            ),
            (
                |value| value.compiler_semantics_commitment[0] ^= 1,
                "compiler semantics",
            ),
            (
                |value| value.canonical_module_commitment[0] ^= 1,
                "canonical module",
            ),
            (
                |value| value.root_instance_identity.push_str("_other"),
                "root instance",
            ),
            (
                |value| value.kernel_export.push_str("_other"),
                "kernel export",
            ),
            (|value| value.target = "gfx942:xnack+".to_owned(), "target"),
            (|value| value.code_object_version = 5, "code-object version"),
            (
                |value| value.explicit_kernarg_bytes = 31,
                "kernarg ABI sizes",
            ),
            (
                |value| value.complete_kernarg_bytes = 287,
                "kernarg ABI sizes",
            ),
            (|value| value.row_elements = 63, "row extent"),
            (|value| value.abi_binding_commitment[0] ^= 1, "explicit ABI"),
            (
                |value| value.fn_abi_binding_commitment[0] ^= 1,
                "rustc FnAbi",
            ),
            (
                |value| value.launch_binding_commitment[0] ^= 1,
                "launch contract",
            ),
            (
                |value| value.correspondence_commitment[0] ^= 1,
                "reviewed source-to-canonical-module correspondence",
            ),
            (
                |value| value.exponential_boundary_commitment[0] ^= 1,
                "unresolved exponential boundary",
            ),
            (
                |value| value.frontend_contract_commitment[0] ^= 1,
                "frontend contract",
            ),
            (
                |value| {
                    value.cargo_metadata_build_observation.ordered_tokens[0] =
                        "fedcba9876543210".to_owned()
                },
                "ordered Cargo metadata build observation",
            ),
            (
                |value| value.cargo_metadata_build_observation.commitment[0] ^= 1,
                "ordered Cargo metadata build observation",
            ),
            (
                |value| value.provider_authority.source_identities[0][0] ^= 1,
                "row-softmax trusted provider authority",
            ),
        ];
        for (mutate, expected_field) in mutations {
            let mut receipt = exact_frontend_receipt_for_test();
            let authority = receipt.authority.as_mut().expect("test authority");
            mutate(authority);
            assert_ne!(baseline, collected_authority_commitment(authority));
            authority.authority_commitment = collected_authority_commitment(authority);
            match receipt.consume() {
                Err(CollectedRowSoftmaxErrorV1::ReceiptBindingMismatch { field }) => {
                    assert_eq!(field, expected_field)
                }
                other => panic!(
                    "re-signed mutation for {expected_field:?} reached the wrong result: {other:?}"
                ),
            }
        }
    }

    #[test]
    fn stale_outer_authority_commitment_fails_at_that_binding() {
        let mut receipt = exact_frontend_receipt_for_test();
        receipt
            .authority
            .as_mut()
            .expect("test authority")
            .authority_commitment[0] ^= 1;
        assert!(matches!(
            receipt.consume(),
            Err(CollectedRowSoftmaxErrorV1::ReceiptBindingMismatch {
                field: "authority commitment"
            })
        ));
    }

    #[test]
    fn every_compiler_semantics_substitution_fails_closed() {
        let baseline = reviewed_compiler_semantics("0123456789abcdef");
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
                Err(CollectedRowSoftmaxErrorV1::CompilerSemantics { .. })
            ));
        }
    }

    #[test]
    fn cargo_generated_metadata_is_normalized_but_its_ordered_observation_is_bound() {
        let first = reviewed_compiler_semantics("0123456789abcdef");
        let alternate = reviewed_compiler_semantics("fedcba9876543210");
        let first = require_compiler_semantics(&first).expect("first valid Cargo token");
        let alternate =
            require_compiler_semantics(&alternate).expect("alternate valid Cargo token");
        assert_eq!(
            first.normalized_commitment, alternate.normalized_commitment,
            "Cargo's generated token is not portable source semantics"
        );
        assert_ne!(
            first.cargo_metadata_build_observation.commitment,
            alternate.cargo_metadata_build_observation.commitment,
            "the private wrapper still binds the full ordered build observation"
        );

        for tokens in [
            Vec::<String>::new(),
            vec!["0123456789abcdef".to_owned()],
            vec![
                "0123456789abcdef".to_owned(),
                REVIEWED_CRATE_METADATA.to_owned(),
                "extra".to_owned(),
            ],
            vec![
                "0123456789abcde".to_owned(),
                REVIEWED_CRATE_METADATA.to_owned(),
            ],
            vec![
                "0123456789abcdef0".to_owned(),
                REVIEWED_CRATE_METADATA.to_owned(),
            ],
            vec![
                "0123456789abcdeF".to_owned(),
                REVIEWED_CRATE_METADATA.to_owned(),
            ],
            vec![
                "0123456789abcdeg".to_owned(),
                REVIEWED_CRATE_METADATA.to_owned(),
            ],
            vec![
                REVIEWED_CRATE_METADATA.to_owned(),
                "0123456789abcdef".to_owned(),
            ],
            vec![
                "0123456789abcdef".to_owned(),
                "row-softmax-lookalike".to_owned(),
            ],
        ] {
            let mut malformed = reviewed_compiler_semantics("0123456789abcdef");
            malformed.crate_metadata = tokens;
            assert!(matches!(
                require_compiler_semantics(&malformed),
                Err(CollectedRowSoftmaxErrorV1::CompilerSemantics { .. })
            ));
        }
    }
}
