//! Exact-source authentication for the masked Wave64 collectives V1 profile.
//!
//! This layer authenticates the attributed source bytes (including their
//! Phase A fallback namespace), the distinct wrapper/session-derived
//! registration binding, rustc ABI, and complete reachable portable-MIR
//! closure before selecting the closed semantic profile in `fe2o3-kernel-ir`.
//! The selection is reviewed correspondence, not a compiler-refinement proof.
//! No generic IR is silently substituted and no Worker V2, LLVM, link,
//! artifact, or execution authority is produced here.

use std::error::Error;
use std::fmt;

use fe2o3_artifacts::{
    AbiKind, Access, AddressSpace, AliasClass, ArgumentOwnership, BlockSize, Capability,
    Dimensions, Endianness, IdentityText, LaunchContract, Mutability as ArtifactMutability,
    PointerWidth, RustScalarElementTypeV1, ScalarType, TargetIdentity,
};
use fe2o3_kernel_ir::{
    WAVE64_COLLECTIVES_V1_EXPLICIT_KERNARG_BYTES, WAVE64_COLLECTIVES_V1_KERNEL_ID,
    WAVE64_COLLECTIVES_V1_NAMESPACE, WAVE64_COLLECTIVES_V1_SOURCE_SHA256,
    Wave64CollectivesKernelIrV1, Wave64CollectivesProfileV1, verify_wave64_collectives_v1,
    wave64_collectives_v1_kernel_ir,
};
use reserved_fe2o3_symbols::{
    CrateBindingIdV1, MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1, derive_crate_binding_id_v1,
    derive_kernel_binding_id_v1,
};
use rustc_abi::{CanonAbi, ExternAbi};
use rustc_hir::{Mutability, Safety};
use rustc_middle::ty::{FloatTy, InstanceKind, Ty, TyCtxt, TyKind, TypingEnv, UintTy};
use rustc_target::callconv::{ArgAttributes, ArgExtension, PassMode};
use sha2::{Digest as _, Sha256};

use crate::AmdGpuTarget;
use crate::collector::{CollectedFunction, CollectionResult, TypedKernelProfile};
use crate::rust_type_layout_v3::GeneralTypedArgumentKindV3;
use crate::trusted_device_items::{self, TrustedAmdGpuDiagnosticOperation, TrustedDeviceItem};

pub(crate) const COLLECTED_WAVE64_COLLECTIVES_PIPELINE_V1: &str = "collected-wave64-collectives-v1";
pub(crate) const EXACT_WAVE64_COLLECTIVES_TARGET_V1: &str = "gfx942:xnack-";
pub(crate) const WAVE64_COLLECTIVES_CODE_OBJECT_VERSION_V1: u16 = 6;

const REVIEWED_RUSTC_RELEASE: &str = "1.96.0-nightly";
const REVIEWED_RUSTC_COMMIT: &str = "55e86c996809902e8bbad512cfb4d2c18be446d9";
const REVIEWED_RUSTC_LLVM: &str = "22.1.2";
const REVIEWED_CRATE_NAME: &str = "fe2o3_collected_wave64_collectives_v1_fixture";
const REVIEWED_CRATE_METADATA: &str = "fe2o3-wave64-collectives-v1-reviewed";
// The ordinary macro wrapper overrides the source fallback namespace with the
// binding derived from this exact crate name and ordered metadata. Authority
// commits both identities so the override is visible and cannot substitute
// either the public source bytes or the compiler session.
const REVIEWED_COMPILER_CRATE_BINDING: &str =
    "ba3fa024069d9cee1b86cf6fc1ad80a77d9de5457de020b70182cdc265e64569";
const SOURCE_REMAP_DESTINATION: &str = "/fe2o3-reviewed-workspace/wave64-collectives-v1.rs";
const WORKSPACE_REMAP_DESTINATION: &str = "/fe2o3-reviewed-workspace";
const COMPILER_SEMANTICS_DOMAIN_V1: &[u8] = b"fe2o3.wave64-collectives.compiler-semantics.v1";
const TRUSTED_DEFINITIONS_DOMAIN_V1: &[u8] = b"fe2o3.wave64-collectives.trusted-definitions.v1";
const AUTHORITY_DOMAIN_V1: &[u8] = b"fe2o3.wave64-collectives.source-authority.v1";
const FN_ABI_DOMAIN_V1: &[u8] = b"fe2o3.wave64-collectives.rustc-fn-abi.v1";
const ABI_BINDING_V1: &[u8] = b"ptr64;size=72;align=8;input@0:16:8:slice-f32:shared-readonly;active_mask@16:8:8:scalar-u64:value;reduction_output@24:16:8:slice-f32:exclusive-readwrite;inclusive_output@40:16:8:slice-f32:exclusive-readwrite;exclusive_output@56:16:8:slice-f32:exclusive-readwrite";
const EFFECT_BINDING_V1: &[u8] = b"input:read-only;active-mask:by-value;three-distinct-outputs:lane-index-exclusive-write;all-physical-lanes-convergent;logical-inactive-contribution=positive-zero;logical-inactive-publication=positive-zero";
const SOURCE_LAUNCH_BINDING_V1: &[u8] =
    b"rank=1;block=exact(64,1,1);max-grid=(4294967295,1,1);static-shared=0;dynamic-shared=0";
const PROFILE_LAUNCH_BINDING_V1: &[u8] =
    b"target=gfx942:xnack-;cov=6;wave=64;block=exact(64,1,1);grid=exact(1,1,1)";
const NUMERICAL_BINDING_V1: &[u8] = b"input=f32-finite-integral-abs-le-1024;all-64-lane-sums-exact-binary32;mask=u64;empty-mask=accepted-positive-zero";
const DESCRIPTOR_BINDING_V1: &[u8] = b"logical=wave64_collectives_v1;export=wave64_collectives_v1;descriptor=wave64_collectives_v1.kd;explicit-kernarg=72;complete-cov6-kernarg=328;wg=64,1,1;wave=64";
const CANONICAL_IR_BINDING_V1: &[u8] = b"fe2o3::wave64_collectives_v1;args=input-f32-slice,active-mask-u64,three-lane-owned-f32-slices;ordered-collectives=reduce-sum,inclusive-scan-sum,exclusive-scan-sum;inactive=contribute-and-publish-positive-zero";
const CORRESPONDENCE_BINDING_V1: &[u8] = b"exact source bytes including Phase A fallback namespace plus distinct wrapper/session-derived attributed registration plus complete reachable portable-MIR closure select the closed Wave64 collectives semantic sidecar;reviewed correspondence only;not a compiler-refinement proof";
const EXACT_FRONTEND_CONTRACT_V1: &[u8] = &[
    70, 69, 50, 79, 51, 75, 70, 0, 1, 0, 1, 0, 52, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 64, 0, 0, 0, 1,
    0, 0, 0, 1, 0, 0, 0, 64, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0,
];

// Filled from the pinned compiler fixture after V3 portable-MIR import. V3
// uses structured semantic instances, excluding rustc's nonsemantic crate
// disambiguators while binding every reachable body, monomorphization, call
// target, semantically complete type/value, and operation.
const PORTABLE_MIR_CLOSURE_IDENTITY_V1: [u8; 32] = [
    0x33, 0x71, 0x21, 0x6d, 0xb9, 0x73, 0x65, 0x9f, 0x99, 0x01, 0xad, 0x59, 0x78, 0x95, 0xa7, 0x79,
    0xfb, 0x99, 0x93, 0x5b, 0xf8, 0xd3, 0x8a, 0x64, 0xbc, 0xe7, 0x5b, 0xa0, 0xb9, 0xb6, 0xaf, 0xf2,
];
const RUSTC_FN_ABI_IDENTITY_V1: [u8; 32] = [
    0xfa, 0x8c, 0xfc, 0xa7, 0x9d, 0x34, 0x7f, 0x48, 0x86, 0x0e, 0xae, 0xd4, 0x26, 0x51, 0xa5, 0x29,
    0x70, 0x62, 0x98, 0x4a, 0xc3, 0x62, 0x51, 0xe9, 0x5d, 0xd7, 0x63, 0xb4, 0x07, 0x43, 0x2e, 0xa3,
];

const ARGUMENT_KINDS_V1: [GeneralTypedArgumentKindV3; 5] = [
    GeneralTypedArgumentKindV3::SharedSlice(RustScalarElementTypeV1::F32),
    GeneralTypedArgumentKindV3::Scalar(RustScalarElementTypeV1::U64),
    GeneralTypedArgumentKindV3::DisjointSlice(RustScalarElementTypeV1::F32),
    GeneralTypedArgumentKindV3::DisjointSlice(RustScalarElementTypeV1::F32),
    GeneralTypedArgumentKindV3::DisjointSlice(RustScalarElementTypeV1::F32),
];

const REQUIRED_TRUSTED_ITEMS_V1: [TrustedDeviceItem; 11] = [
    TrustedDeviceItem::ThreadIndex1d,
    TrustedDeviceItem::ThreadIndexGet,
    TrustedDeviceItem::DisjointSliceLen,
    TrustedDeviceItem::DisjointSliceGetMutAt,
    TrustedDeviceItem::Gfx942CollectivesContext,
    TrustedDeviceItem::Gfx942CollectivesFromCompiler,
    TrustedDeviceItem::WaveLaneFromRaw,
    TrustedDeviceItem::AmdGpuDiagnostic(TrustedAmdGpuDiagnosticOperation::Trap),
    TrustedDeviceItem::Gfx942Wave64ReduceSum,
    TrustedDeviceItem::Gfx942Wave64InclusiveScanSum,
    TrustedDeviceItem::Gfx942Wave64ExclusiveScanSum,
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
    crate_name: String,
    crate_metadata: Vec<String>,
    remap_path_destinations: Vec<String>,
}

#[derive(Debug, Eq, PartialEq)]
struct Wave64CollectivesAuthorityV1 {
    source_identity: [u8; 32],
    source_namespace: [u8; 32],
    compiler_crate_binding: [u8; 32],
    target: String,
    code_object_version: u16,
    kernel_export: String,
    root_instance_identity: String,
    portable_mir_identity: [u8; 32],
    compiler_semantics_identity: [u8; 32],
    fn_abi_identity: [u8; 32],
    trusted_definitions_identity: [u8; 32],
    frontend_contract_identity: [u8; 32],
    abi_identity: [u8; 32],
    effects_identity: [u8; 32],
    source_launch_identity: [u8; 32],
    profile_launch_identity: [u8; 32],
    numerical_identity: [u8; 32],
    descriptor_identity: [u8; 32],
    canonical_ir_identity: [u8; 32],
    correspondence_identity: [u8; 32],
    authority_identity: [u8; 32],
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Wave64CollectivesFrontendReceiptV1 {
    authority: Option<Wave64CollectivesAuthorityV1>,
    ir: Option<Wave64CollectivesKernelIrV1>,
    profile: Option<Wave64CollectivesProfileV1>,
}

impl Wave64CollectivesFrontendReceiptV1 {
    fn authority(&self) -> &Wave64CollectivesAuthorityV1 {
        self.authority.as_ref().expect("unconsumed Wave64 receipt")
    }

    pub(crate) fn root_instance_identity(&self) -> &str {
        &self.authority().root_instance_identity
    }

    pub(crate) fn portable_mir_hex(&self) -> String {
        crate::encode_hex(&self.authority().portable_mir_identity)
    }

    pub(crate) fn authority_hex(&self) -> String {
        crate::encode_hex(&self.authority().authority_identity)
    }

    pub(crate) fn consume(
        &mut self,
    ) -> Result<AuthenticatedWave64CollectivesV1, CollectedWave64CollectivesErrorV1> {
        let authority = self
            .authority
            .take()
            .ok_or(CollectedWave64CollectivesErrorV1::ReceiptAlreadyConsumed)?;
        let ir = self
            .ir
            .take()
            .ok_or(CollectedWave64CollectivesErrorV1::ReceiptAlreadyConsumed)?;
        let profile = self
            .profile
            .take()
            .ok_or(CollectedWave64CollectivesErrorV1::ReceiptAlreadyConsumed)?;
        validate_authority(&authority)?;
        verify_wave64_collectives_v1(&ir, &profile)
            .map_err(|error| CollectedWave64CollectivesErrorV1::CanonicalIr(error.to_string()))?;
        Ok(AuthenticatedWave64CollectivesV1 {
            ir,
            profile,
            source_authority_identity: authority.authority_identity,
            descriptor_identity: authority.descriptor_identity,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedWave64CollectivesV1 {
    ir: Wave64CollectivesKernelIrV1,
    profile: Wave64CollectivesProfileV1,
    source_authority_identity: [u8; 32],
    descriptor_identity: [u8; 32],
}

impl AuthenticatedWave64CollectivesV1 {
    pub(crate) fn semantic_summary(&self) -> (usize, usize) {
        (self.ir.collectives.len(), self.ir.outputs.len())
    }

    pub(crate) fn profile(&self) -> &Wave64CollectivesProfileV1 {
        &self.profile
    }

    pub(crate) fn source_authority_hex(&self) -> String {
        crate::encode_hex(&self.source_authority_identity)
    }

    pub(crate) fn descriptor_hex(&self) -> String {
        crate::encode_hex(&self.descriptor_identity)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum CollectedWave64CollectivesErrorV1 {
    Admission(String),
    SourceIdentity {
        expected: [u8; 32],
        actual: [u8; 32],
    },
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
    CanonicalIr(String),
    ReceiptAlreadyConsumed,
    ReceiptBinding(&'static str),
}

impl fmt::Display for CollectedWave64CollectivesErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission(detail) => {
                write!(formatter, "Wave64 collectives admission failed: {detail}")
            }
            Self::SourceIdentity { expected, actual } => write!(
                formatter,
                "Wave64 source bytes mismatch: expected {}, found {}",
                crate::encode_hex(expected),
                crate::encode_hex(actual)
            ),
            Self::Abi(detail) => write!(formatter, "Wave64 collectives ABI mismatch: {detail}"),
            Self::Layout(detail) => {
                write!(formatter, "Wave64 collectives layout mismatch: {detail}")
            }
            Self::PortableMir(detail) => {
                write!(formatter, "Wave64 portable MIR rejected: {detail}")
            }
            Self::PortableMirIdentity { expected, actual } => write!(
                formatter,
                "Wave64 complete reachable MIR closure mismatch: expected {}, found {}",
                crate::encode_hex(expected),
                crate::encode_hex(actual)
            ),
            Self::FnAbiIdentity { expected, actual } => write!(
                formatter,
                "Wave64 rustc FnAbi mismatch: expected {}, found {}",
                crate::encode_hex(expected),
                crate::encode_hex(actual)
            ),
            Self::TrustedDefinitions(detail) => write!(
                formatter,
                "Wave64 trusted definition closure rejected: {detail}"
            ),
            Self::CanonicalIr(detail) => {
                write!(formatter, "Wave64 canonical semantic IR rejected: {detail}")
            }
            Self::ReceiptAlreadyConsumed => {
                formatter.write_str("Wave64 frontend receipt was already consumed")
            }
            Self::ReceiptBinding(field) => write!(
                formatter,
                "Wave64 frontend receipt binding mismatch: {field}"
            ),
        }
    }
}

impl Error for CollectedWave64CollectivesErrorV1 {}

pub(crate) fn authenticate_collected_wave64_collectives_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    collection: &CollectionResult<'tcx>,
    target: &AmdGpuTarget,
    custom_llvm_pipeline: bool,
) -> Result<Wave64CollectivesFrontendReceiptV1, CollectedWave64CollectivesErrorV1> {
    admit_execution_context(target.as_str(), custom_llvm_pipeline)?;
    let compiler_semantics_identity = require_compiler_semantics(&observe_compiler_semantics(tcx))?;
    let root = exact_root(&collection.functions)?;
    require_registration(root)?;
    let source_identity = observe_source_identity(tcx, root)?;
    require_signature(tcx, root.instance)?;
    require_layout(root)?;
    let fn_abi_identity = require_fn_abi(tcx, root.instance)?;
    let trusted_definitions_identity = trusted_definitions_identity(tcx)?;
    let target_identity = exact_target_identity()?;
    let profile_launch = exact_profile_launch()?;
    let contract = root
        .general_typed_contract
        .as_ref()
        .expect("layout checked contract");
    let imported = crate::mir_import::import_collection(tcx, collection)
        .map_err(|error| CollectedWave64CollectivesErrorV1::PortableMir(error.to_string()))?;
    let portable_mir_identity = imported
        .portable_semantic_digest_v3(crate::mir_import::MirSemanticAdmissionInputsV3::new(
            WAVE64_COLLECTIVES_V1_KERNEL_ID,
            &target_identity,
            contract.abi(),
            &profile_launch,
        ))
        .map_err(|error| CollectedWave64CollectivesErrorV1::PortableMir(error.to_string()))?;
    let portable_mir_identity = *portable_mir_identity.as_bytes();
    if portable_mir_identity != PORTABLE_MIR_CLOSURE_IDENTITY_V1 {
        return Err(CollectedWave64CollectivesErrorV1::PortableMirIdentity {
            expected: PORTABLE_MIR_CLOSURE_IDENTITY_V1,
            actual: portable_mir_identity,
        });
    }
    let root_instance_identity = tcx.def_path_str(root.instance.def_id());
    if !crate::collected_tiled_gemm_v1::is_kernel_root_build_identity(&root_instance_identity) {
        return Err(CollectedWave64CollectivesErrorV1::Admission(format!(
            "root instance has noncanonical generated identity `{root_instance_identity}`"
        )));
    }
    let ir = wave64_collectives_v1_kernel_ir();
    let profile = Wave64CollectivesProfileV1::exact_gfx942_xnack_minus_cov6();
    verify_wave64_collectives_v1(&ir, &profile)
        .map_err(|error| CollectedWave64CollectivesErrorV1::CanonicalIr(error.to_string()))?;
    let frontend_contract_identity = sha256(
        root.frontend_contract
            .as_ref()
            .expect("registration checked frontend contract")
            .canonical_bytes(),
    );
    let mut authority = Wave64CollectivesAuthorityV1 {
        source_identity,
        source_namespace: WAVE64_COLLECTIVES_V1_NAMESPACE,
        compiler_crate_binding: compiler_crate_binding().as_bytes(),
        target: target.as_str().to_owned(),
        code_object_version: WAVE64_COLLECTIVES_CODE_OBJECT_VERSION_V1,
        kernel_export: root.export_name.clone(),
        root_instance_identity,
        portable_mir_identity,
        compiler_semantics_identity,
        fn_abi_identity,
        trusted_definitions_identity,
        frontend_contract_identity,
        abi_identity: sha256(ABI_BINDING_V1),
        effects_identity: sha256(EFFECT_BINDING_V1),
        source_launch_identity: sha256(SOURCE_LAUNCH_BINDING_V1),
        profile_launch_identity: sha256(PROFILE_LAUNCH_BINDING_V1),
        numerical_identity: sha256(NUMERICAL_BINDING_V1),
        descriptor_identity: sha256(DESCRIPTOR_BINDING_V1),
        canonical_ir_identity: sha256(CANONICAL_IR_BINDING_V1),
        correspondence_identity: sha256(CORRESPONDENCE_BINDING_V1),
        authority_identity: [0; 32],
    };
    authority.authority_identity = authority_identity(&authority);
    Ok(Wave64CollectivesFrontendReceiptV1 {
        authority: Some(authority),
        ir: Some(ir),
        profile: Some(profile),
    })
}

fn admit_execution_context(
    target: &str,
    custom_llvm_pipeline: bool,
) -> Result<(), CollectedWave64CollectivesErrorV1> {
    if target != EXACT_WAVE64_COLLECTIVES_TARGET_V1 {
        return Err(CollectedWave64CollectivesErrorV1::Admission(format!(
            "requires exact target `{EXACT_WAVE64_COLLECTIVES_TARGET_V1}`, found `{target}`"
        )));
    }
    if custom_llvm_pipeline {
        return Err(CollectedWave64CollectivesErrorV1::Admission(
            "custom LLVM arguments or passes are forbidden".into(),
        ));
    }
    Ok(())
}

fn exact_root<'a, 'tcx>(
    functions: &'a [CollectedFunction<'tcx>],
) -> Result<&'a CollectedFunction<'tcx>, CollectedWave64CollectivesErrorV1> {
    let mut roots = functions
        .iter()
        .filter(|function| function.is_kernel_entry());
    let root = roots.next().ok_or_else(|| {
        CollectedWave64CollectivesErrorV1::Admission(
            "the exact Wave64 closure has no kernel root".into(),
        )
    })?;
    if roots.next().is_some() || functions.len() != 3 {
        return Err(CollectedWave64CollectivesErrorV1::Admission(format!(
            "the exact Wave64 closure requires one root plus two reachable helpers, found {} collected functions",
            functions.len()
        )));
    }
    Ok(root)
}

fn require_registration(
    root: &CollectedFunction<'_>,
) -> Result<(), CollectedWave64CollectivesErrorV1> {
    let namespace = compiler_crate_binding();
    let expected_binding = derive_kernel_binding_id_v1(
        namespace,
        MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
        WAVE64_COLLECTIVES_V1_KERNEL_ID,
        WAVE64_COLLECTIVES_V1_KERNEL_ID,
    );
    if root.export_name != WAVE64_COLLECTIVES_V1_KERNEL_ID
        || root.logical_name.as_deref() != Some(WAVE64_COLLECTIVES_V1_KERNEL_ID)
        || !matches!(
            root.typed_profile,
            Some(TypedKernelProfile::GeneralScalarSliceRustcLayoutV3 { .. })
        )
        || root.kernel_binding != Some(expected_binding)
        || root
            .frontend_contract
            .as_ref()
            .map(|value| value.canonical_bytes())
            != Some(EXACT_FRONTEND_CONTRACT_V1)
    {
        return Err(CollectedWave64CollectivesErrorV1::Admission(
            "expected the unique ordinary #[kernel(typed)] Wave64 root with the reviewed wrapper-derived crate binding and required=max=64x1x1 contract".into(),
        ));
    }
    Ok(())
}

fn observe_source_identity(
    tcx: TyCtxt<'_>,
    root: &CollectedFunction<'_>,
) -> Result<[u8; 32], CollectedWave64CollectivesErrorV1> {
    let file_name = tcx
        .sess
        .source_map()
        .span_to_filename(tcx.def_span(root.instance.def_id()))
        .prefer_local_unconditionally()
        .to_string_lossy()
        .into_owned();
    let bytes = std::fs::read(&file_name).map_err(|error| {
        CollectedWave64CollectivesErrorV1::Admission(format!(
            "source file `{file_name}` is unavailable for exact-byte authentication: {error}"
        ))
    })?;
    let namespace_declaration = format!(
        "namespace = \"{}\"",
        crate::encode_hex(&WAVE64_COLLECTIVES_V1_NAMESPACE)
    );
    if bytes
        .windows(namespace_declaration.len())
        .filter(|window| *window == namespace_declaration.as_bytes())
        .count()
        != 1
    {
        return Err(CollectedWave64CollectivesErrorV1::Admission(
            "exact source must contain the unique reviewed Phase A namespace declaration".into(),
        ));
    }
    let actual = sha256(&bytes);
    if actual != WAVE64_COLLECTIVES_V1_SOURCE_SHA256 {
        return Err(CollectedWave64CollectivesErrorV1::SourceIdentity {
            expected: WAVE64_COLLECTIVES_V1_SOURCE_SHA256,
            actual,
        });
    }
    Ok(actual)
}

fn require_signature<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: rustc_middle::ty::Instance<'tcx>,
) -> Result<(), CollectedWave64CollectivesErrorV1> {
    if !matches!(instance.def, InstanceKind::Item(_)) || !instance.args.is_empty() {
        return Err(CollectedWave64CollectivesErrorV1::Abi(
            "kernel must be one nongeneric ordinary function item".into(),
        ));
    }
    let signature = tcx
        .try_instantiate_and_normalize_erasing_regions(
            instance.args,
            TypingEnv::fully_monomorphized(),
            tcx.fn_sig(instance.def_id()),
        )
        .map_err(|_| {
            CollectedWave64CollectivesErrorV1::Abi("signature normalization failed".into())
        })?;
    let signature = tcx.instantiate_bound_regions_with_erased(signature);
    if signature.safety != Safety::Safe
        || signature.abi != ExternAbi::Rust
        || signature.c_variadic
        || signature.output() != tcx.types.unit
        || signature.inputs().len() != 5
        || !is_shared_f32_slice(signature.inputs()[0])
        || !matches!(signature.inputs()[1].kind(), TyKind::Uint(UintTy::U64))
        || !(2..5).all(|index| is_disjoint_f32_slice(tcx, signature.inputs()[index]))
    {
        return Err(CollectedWave64CollectivesErrorV1::Abi(format!(
            "expected safe Rust `(&[f32], u64, DisjointSlice<f32>, DisjointSlice<f32>, DisjointSlice<f32>) -> ()`, found `{signature}`"
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
            .and_then(|value| value.as_type())
            .is_some_and(|element| matches!(element.kind(), TyKind::Float(FloatTy::F32)))
}

fn require_layout(root: &CollectedFunction<'_>) -> Result<(), CollectedWave64CollectivesErrorV1> {
    let contract = root.general_typed_contract.as_ref().ok_or_else(|| {
        CollectedWave64CollectivesErrorV1::Layout("General V3 contract is absent".into())
    })?;
    let actual = contract
        .arguments()
        .iter()
        .map(|value| value.kind())
        .collect::<Vec<_>>();
    if actual != ARGUMENT_KINDS_V1 {
        return Err(CollectedWave64CollectivesErrorV1::Layout(format!(
            "expected argument kinds {ARGUMENT_KINDS_V1:?}, found {actual:?}"
        )));
    }
    if root
        .typed_layout_identities
        .as_ref()
        .map(|identities| identities.len())
        != Some(5)
    {
        return Err(CollectedWave64CollectivesErrorV1::Layout(
            "five compiler-derived argument identities are required".into(),
        ));
    }
    let abi = contract.abi();
    if abi.size() != u64::from(WAVE64_COLLECTIVES_V1_EXPLICIT_KERNARG_BYTES)
        || abi.alignment() != 8
        || abi.pointer_width() != PointerWidth::Bits64
        || abi.fields().len() != 5
    {
        return Err(CollectedWave64CollectivesErrorV1::Layout(format!(
            "expected ptr64 size-72 align-8 five-field ABI, found {abi:?}"
        )));
    }
    // General V3 canonicalizes physical ABI fields positionally; the semantic
    // role names remain bound by `ABI_BINDING_V1` and the closed Kernel IR.
    let names = ["arg0", "arg1", "arg2", "arg3", "arg4"];
    let offsets = [0, 16, 24, 40, 56];
    let sizes = [16, 8, 16, 16, 16];
    let alignments = [8, 8, 8, 8, 8];
    for (index, field) in abi.fields().iter().enumerate() {
        if field.name().as_str() != names[index]
            || field.offset() != offsets[index]
            || field.size() != sizes[index]
            || field.alignment() != alignments[index]
            || field.type_identity() != contract.arguments()[index].type_identity()
        {
            return Err(CollectedWave64CollectivesErrorV1::Layout(format!(
                "ABI field {index} name, offset, size, alignment, or type identity drifted: field={field:?}, argument={:?}",
                contract.arguments()[index]
            )));
        }
        let exact = match index {
            0 => {
                matches!(
                    field.kind(),
                    AbiKind::Slice {
                        element_size: 4,
                        element_alignment: 4
                    }
                ) && field.mutability() == ArtifactMutability::Immutable
                    && field.access() == Access::ReadOnly
                    && field.address_space() == AddressSpace::Global
                    && field.ownership() == ArgumentOwnership::SharedBorrow
                    && field.alias_class() == AliasClass::SharedReadOnly
            }
            1 => {
                field.kind() == AbiKind::Scalar(ScalarType::U64)
                    && field.mutability() == ArtifactMutability::Immutable
                    && field.access() == Access::ByValue
                    && field.address_space() == AddressSpace::Value
                    && field.ownership() == ArgumentOwnership::ByValue
                    && field.alias_class() == AliasClass::Value
            }
            2..=4 => {
                matches!(
                    field.kind(),
                    AbiKind::Slice {
                        element_size: 4,
                        element_alignment: 4
                    }
                ) && field.mutability() == ArtifactMutability::Mutable
                    && field.access() == Access::ReadWrite
                    && field.address_space() == AddressSpace::Global
                    && field.ownership() == ArgumentOwnership::UniqueBorrow
                    && field.alias_class() == AliasClass::Exclusive
            }
            _ => unreachable!(),
        };
        if !exact {
            return Err(CollectedWave64CollectivesErrorV1::Layout(format!(
                "ABI field {index} access, ownership, address space, or kind drifted"
            )));
        }
    }
    let launch = contract.launch();
    if launch.rank() != 1
        || launch.block_size()
            != BlockSize::Exact(
                Dimensions::new(64, 1, 1).map_err(|error| {
                    CollectedWave64CollectivesErrorV1::Layout(error.to_string())
                })?,
            )
        || launch.max_grid()
            != Dimensions::new(u32::MAX, 1, 1)
                .map_err(|error| CollectedWave64CollectivesErrorV1::Layout(error.to_string()))?
        || launch.static_shared_memory_bytes() != 0
        || launch.max_dynamic_shared_memory_bytes() != 0
    {
        return Err(CollectedWave64CollectivesErrorV1::Layout(
            "source launch must be exact WG64 with one-dimensional grid and no LDS".into(),
        ));
    }
    Ok(())
}

fn require_fn_abi<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: rustc_middle::ty::Instance<'tcx>,
) -> Result<[u8; 32], CollectedWave64CollectivesErrorV1> {
    let query = TypingEnv::fully_monomorphized()
        .as_query_input((instance, rustc_middle::ty::List::empty()));
    let abi = tcx.fn_abi_of_instance(query).map_err(|error| {
        CollectedWave64CollectivesErrorV1::Abi(format!("FnAbi query failed: {error:?}"))
    })?;
    if abi.conv != CanonAbi::Rust
        || abi.c_variadic
        || abi.fixed_count != 5
        || abi.args.len() != 5
        || !matches!(abi.ret.mode, PassMode::Ignore)
        || abi.ret.layout.size.bytes() != 0
    {
        return Err(CollectedWave64CollectivesErrorV1::Abi(format!(
            "FnAbi header must be Rust(args=5)->unit, found {abi:?}"
        )));
    }
    let mut digest = Sha256::new();
    hash_field(&mut digest, FN_ABI_DOMAIN_V1);
    hash_field(&mut digest, &[u8::from(abi.c_variadic)]);
    hash_field(&mut digest, &abi.fixed_count.to_le_bytes());
    hash_field(&mut digest, &[u8::from(abi.can_unwind)]);
    for (index, argument) in abi.args.iter().enumerate() {
        let expected_size = if index == 1 { 8 } else { 16 };
        if argument.layout.size.bytes() != expected_size || argument.layout.align.abi.bytes() != 8 {
            return Err(CollectedWave64CollectivesErrorV1::Abi(format!(
                "FnAbi argument {index} size or alignment drifted"
            )));
        }
        hash_field(&mut digest, &argument.layout.size.bytes().to_le_bytes());
        hash_field(
            &mut digest,
            &argument.layout.align.abi.bytes().to_le_bytes(),
        );
        match argument.mode {
            PassMode::Pair(first, second) if index != 1 => {
                hash_field(&mut digest, &[2]);
                hash_arg_attributes(&mut digest, first);
                hash_arg_attributes(&mut digest, second);
            }
            PassMode::Direct(attributes) if index == 1 => {
                hash_field(&mut digest, &[1]);
                hash_arg_attributes(&mut digest, attributes);
            }
            _ => {
                return Err(CollectedWave64CollectivesErrorV1::Abi(format!(
                    "FnAbi argument {index} pass mode drifted"
                )));
            }
        }
    }
    let actual: [u8; 32] = digest.finalize().into();
    if actual != RUSTC_FN_ABI_IDENTITY_V1 {
        return Err(CollectedWave64CollectivesErrorV1::FnAbiIdentity {
            expected: RUSTC_FN_ABI_IDENTITY_V1,
            actual,
        });
    }
    Ok(actual)
}

fn hash_arg_attributes(digest: &mut Sha256, attributes: ArgAttributes) {
    hash_field(digest, &attributes.regular.bits().to_le_bytes());
    let extension = match attributes.arg_ext {
        ArgExtension::None => 0,
        ArgExtension::Zext => 1,
        ArgExtension::Sext => 2,
    };
    hash_field(digest, &[extension]);
    hash_field(digest, &attributes.pointee_size.bytes().to_le_bytes());
    hash_field(
        digest,
        &attributes
            .pointee_align
            .map_or(0, |value| value.bytes())
            .to_le_bytes(),
    );
}

fn trusted_definitions_identity(
    tcx: TyCtxt<'_>,
) -> Result<[u8; 32], CollectedWave64CollectivesErrorV1> {
    let mut digest = Sha256::new();
    hash_field(&mut digest, TRUSTED_DEFINITIONS_DOMAIN_V1);
    let mut provider = None;
    for item in REQUIRED_TRUSTED_ITEMS_V1 {
        let definition = trusted_device_items::definition(tcx, item).ok_or_else(|| {
            CollectedWave64CollectivesErrorV1::TrustedDefinitions(format!(
                "missing exact diagnostic item `{}`",
                item.canonical_path()
            ))
        })?;
        if definition.is_local() || provider.is_some_and(|value| value != definition.krate) {
            return Err(CollectedWave64CollectivesErrorV1::TrustedDefinitions(
                format!(
                    "diagnostic item `{}` did not come from the single external device provider",
                    item.canonical_path()
                ),
            ));
        }
        provider.get_or_insert(definition.krate);
        hash_field(&mut digest, item.canonical_path().as_bytes());
        hash_field(&mut digest, &tcx.def_path_hash(definition).0.to_le_bytes());
    }
    Ok(digest.finalize().into())
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
        crate_name: tcx.crate_name(rustc_hir::def_id::LOCAL_CRATE).to_string(),
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
) -> Result<[u8; 32], CollectedWave64CollectivesErrorV1> {
    let expected_passes = [("JumpThreading".to_owned(), false)];
    let mismatch = if observed.rustc_release != REVIEWED_RUSTC_RELEASE {
        Some(format!("rustc release must be {REVIEWED_RUSTC_RELEASE}"))
    } else if observed.rustc_commit != REVIEWED_RUSTC_COMMIT {
        Some(format!("rustc commit must be {REVIEWED_RUSTC_COMMIT}"))
    } else if observed.llvm_version != REVIEWED_RUSTC_LLVM {
        Some(format!("rustc LLVM must be {REVIEWED_RUSTC_LLVM}"))
    } else if observed.panic_strategy != "Unwind"
        || observed.overflow_checks
        || observed.optimize != "No"
        || !observed.debug_assertions
        || observed.mir_opt_level != 1
        || observed.mir_enable_passes != expected_passes
        || observed.rustc_codegen_opt_level != "0"
    {
        Some("panic/overflow/optimization/debug/MIR semantics drifted".into())
    } else if !observed.llvm_args.is_empty()
        || !observed.llvm_passes.is_empty()
        || observed.target_cpu.is_some()
        || !observed.target_features.is_empty()
    {
        Some("custom LLVM or target feature selection is forbidden".into())
    } else if observed.crate_name != REVIEWED_CRATE_NAME {
        Some(format!(
            "crate name must be exactly {REVIEWED_CRATE_NAME:?}"
        ))
    } else if observed.crate_metadata != [REVIEWED_CRATE_METADATA] {
        Some(format!(
            "crate metadata must be exactly {REVIEWED_CRATE_METADATA:?}"
        ))
    } else if derive_crate_binding_id_v1(
        &observed.crate_name,
        observed.crate_metadata.iter().map(String::as_str),
    ) != compiler_crate_binding()
    {
        Some("crate name and ordered metadata do not derive the reviewed compiler binding".into())
    } else if observed.remap_path_destinations
        != [SOURCE_REMAP_DESTINATION, WORKSPACE_REMAP_DESTINATION]
    {
        Some(format!(
            "source remapping must be exactly the reviewed fixture and workspace destinations, found {:?}",
            observed.remap_path_destinations
        ))
    } else {
        None
    };
    if let Some(detail) = mismatch {
        return Err(CollectedWave64CollectivesErrorV1::Admission(detail));
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
    hash_field(&mut digest, observed.rustc_codegen_opt_level.as_bytes());
    hash_field(&mut digest, observed.crate_name.as_bytes());
    hash_field(&mut digest, observed.crate_metadata[0].as_bytes());
    for destination in &observed.remap_path_destinations {
        hash_field(&mut digest, destination.as_bytes());
    }
    Ok(digest.finalize().into())
}

fn exact_target_identity() -> Result<TargetIdentity, CollectedWave64CollectivesErrorV1> {
    TargetIdentity::new(
        IdentityText::new(dialect_amdgcn::AMDGPU_TRIPLE)
            .map_err(|error| CollectedWave64CollectivesErrorV1::Admission(error.to_string()))?,
        IdentityText::new(EXACT_WAVE64_COLLECTIVES_TARGET_V1)
            .map_err(|error| CollectedWave64CollectivesErrorV1::Admission(error.to_string()))?,
        PointerWidth::Bits64,
        Endianness::Little,
        vec![Capability::Subgroup, Capability::AmdWave],
    )
    .map_err(|error| CollectedWave64CollectivesErrorV1::Admission(error.to_string()))
}

fn exact_profile_launch() -> Result<LaunchContract, CollectedWave64CollectivesErrorV1> {
    LaunchContract::new(
        1,
        BlockSize::Exact(
            Dimensions::new(64, 1, 1)
                .map_err(|error| CollectedWave64CollectivesErrorV1::Layout(error.to_string()))?,
        ),
        Dimensions::new(1, 1, 1)
            .map_err(|error| CollectedWave64CollectivesErrorV1::Layout(error.to_string()))?,
        0,
        0,
    )
    .map_err(|error| CollectedWave64CollectivesErrorV1::Layout(error.to_string()))
}

fn validate_authority(
    authority: &Wave64CollectivesAuthorityV1,
) -> Result<(), CollectedWave64CollectivesErrorV1> {
    let field = if authority.source_identity != WAVE64_COLLECTIVES_V1_SOURCE_SHA256 {
        Some("source bytes")
    } else if authority.source_namespace != WAVE64_COLLECTIVES_V1_NAMESPACE {
        Some("source namespace")
    } else if authority.compiler_crate_binding != compiler_crate_binding().as_bytes() {
        Some("wrapper-derived compiler crate binding")
    } else if authority.target != EXACT_WAVE64_COLLECTIVES_TARGET_V1 {
        Some("target")
    } else if authority.code_object_version != WAVE64_COLLECTIVES_CODE_OBJECT_VERSION_V1 {
        Some("code object version")
    } else if authority.kernel_export != WAVE64_COLLECTIVES_V1_KERNEL_ID {
        Some("kernel export")
    } else if authority.portable_mir_identity != PORTABLE_MIR_CLOSURE_IDENTITY_V1 {
        Some("complete reachable MIR closure")
    } else if authority.fn_abi_identity != RUSTC_FN_ABI_IDENTITY_V1 {
        Some("rustc FnAbi")
    } else if authority.compiler_semantics_identity == [0; 32]
        || authority.trusted_definitions_identity == [0; 32]
    {
        Some("compiler/trusted definition closure")
    } else if authority.frontend_contract_identity != sha256(EXACT_FRONTEND_CONTRACT_V1) {
        Some("frontend contract")
    } else if authority.abi_identity != sha256(ABI_BINDING_V1) {
        Some("ABI")
    } else if authority.effects_identity != sha256(EFFECT_BINDING_V1) {
        Some("effects")
    } else if authority.source_launch_identity != sha256(SOURCE_LAUNCH_BINDING_V1)
        || authority.profile_launch_identity != sha256(PROFILE_LAUNCH_BINDING_V1)
    {
        Some("source/profile launch")
    } else if authority.numerical_identity != sha256(NUMERICAL_BINDING_V1) {
        Some("strict finite integral-f32 policy")
    } else if authority.descriptor_identity != sha256(DESCRIPTOR_BINDING_V1) {
        Some("descriptor")
    } else if authority.canonical_ir_identity != sha256(CANONICAL_IR_BINDING_V1) {
        Some("canonical semantic IR")
    } else if authority.correspondence_identity != sha256(CORRESPONDENCE_BINDING_V1) {
        Some("reviewed correspondence boundary")
    } else if authority.authority_identity != authority_identity(authority) {
        Some("authority commitment")
    } else {
        None
    };
    if let Some(field) = field {
        return Err(CollectedWave64CollectivesErrorV1::ReceiptBinding(field));
    }
    Ok(())
}

fn authority_identity(authority: &Wave64CollectivesAuthorityV1) -> [u8; 32] {
    let mut digest = Sha256::new();
    hash_field(&mut digest, AUTHORITY_DOMAIN_V1);
    hash_field(&mut digest, &authority.source_identity);
    hash_field(&mut digest, &authority.source_namespace);
    hash_field(&mut digest, &authority.compiler_crate_binding);
    hash_field(&mut digest, authority.target.as_bytes());
    hash_field(&mut digest, &authority.code_object_version.to_le_bytes());
    hash_field(&mut digest, authority.kernel_export.as_bytes());
    hash_field(&mut digest, authority.root_instance_identity.as_bytes());
    hash_field(&mut digest, &authority.portable_mir_identity);
    hash_field(&mut digest, &authority.compiler_semantics_identity);
    hash_field(&mut digest, &authority.fn_abi_identity);
    hash_field(&mut digest, &authority.trusted_definitions_identity);
    hash_field(&mut digest, &authority.frontend_contract_identity);
    hash_field(&mut digest, &authority.abi_identity);
    hash_field(&mut digest, &authority.effects_identity);
    hash_field(&mut digest, &authority.source_launch_identity);
    hash_field(&mut digest, &authority.profile_launch_identity);
    hash_field(&mut digest, &authority.numerical_identity);
    hash_field(&mut digest, &authority.descriptor_identity);
    hash_field(&mut digest, &authority.canonical_ir_identity);
    hash_field(&mut digest, &authority.correspondence_identity);
    digest.finalize().into()
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn compiler_crate_binding() -> CrateBindingIdV1 {
    CrateBindingIdV1::from_hex(REVIEWED_COMPILER_CRATE_BINDING)
        .expect("reviewed Wave64 compiler crate binding is canonical")
}

fn hash_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt() -> Wave64CollectivesFrontendReceiptV1 {
        let mut authority = Wave64CollectivesAuthorityV1 {
            source_identity: WAVE64_COLLECTIVES_V1_SOURCE_SHA256,
            source_namespace: WAVE64_COLLECTIVES_V1_NAMESPACE,
            compiler_crate_binding: compiler_crate_binding().as_bytes(),
            target: EXACT_WAVE64_COLLECTIVES_TARGET_V1.into(),
            code_object_version: 6,
            kernel_export: WAVE64_COLLECTIVES_V1_KERNEL_ID.into(),
            root_instance_identity: "__fe2o3_host_kernel_v1_0000000000000000000000000000000000000000000000000000000000000000".into(),
            portable_mir_identity: PORTABLE_MIR_CLOSURE_IDENTITY_V1,
            compiler_semantics_identity: [1; 32],
            fn_abi_identity: RUSTC_FN_ABI_IDENTITY_V1,
            trusted_definitions_identity: [2; 32],
            frontend_contract_identity: sha256(EXACT_FRONTEND_CONTRACT_V1),
            abi_identity: sha256(ABI_BINDING_V1),
            effects_identity: sha256(EFFECT_BINDING_V1),
            source_launch_identity: sha256(SOURCE_LAUNCH_BINDING_V1),
            profile_launch_identity: sha256(PROFILE_LAUNCH_BINDING_V1),
            numerical_identity: sha256(NUMERICAL_BINDING_V1),
            descriptor_identity: sha256(DESCRIPTOR_BINDING_V1),
            canonical_ir_identity: sha256(CANONICAL_IR_BINDING_V1),
            correspondence_identity: sha256(CORRESPONDENCE_BINDING_V1),
            authority_identity: [0; 32],
        };
        authority.authority_identity = authority_identity(&authority);
        Wave64CollectivesFrontendReceiptV1 {
            authority: Some(authority),
            ir: Some(wave64_collectives_v1_kernel_ir()),
            profile: Some(Wave64CollectivesProfileV1::exact_gfx942_xnack_minus_cov6()),
        }
    }

    #[test]
    fn receipt_selects_only_the_exact_semantic_profile_once() {
        let mut value = receipt();
        let admitted = value.consume().unwrap();
        assert_eq!(admitted.semantic_summary(), (3, 3));
        assert_eq!(admitted.profile().grid, [1, 1, 1]);
        assert_eq!(
            value.consume(),
            Err(CollectedWave64CollectivesErrorV1::ReceiptAlreadyConsumed)
        );
    }

    #[test]
    fn authority_mutations_fail_closed() {
        let mutations: Vec<fn(&mut Wave64CollectivesAuthorityV1)> = vec![
            |value| value.source_identity[0] ^= 1,
            |value| value.source_namespace[0] ^= 1,
            |value| value.compiler_crate_binding[0] ^= 1,
            |value| value.target.push('+'),
            |value| value.code_object_version = 5,
            |value| value.portable_mir_identity[0] ^= 1,
            |value| value.fn_abi_identity[0] ^= 1,
            |value| value.abi_identity[0] ^= 1,
            |value| value.effects_identity[0] ^= 1,
            |value| value.profile_launch_identity[0] ^= 1,
            |value| value.numerical_identity[0] ^= 1,
            |value| value.descriptor_identity[0] ^= 1,
            |value| value.canonical_ir_identity[0] ^= 1,
            |value| value.correspondence_identity[0] ^= 1,
        ];
        for mutate in mutations {
            let mut value = receipt();
            mutate(value.authority.as_mut().unwrap());
            value.authority.as_mut().unwrap().authority_identity =
                authority_identity(value.authority.as_ref().unwrap());
            assert!(matches!(
                value.consume(),
                Err(CollectedWave64CollectivesErrorV1::ReceiptBinding(_))
            ));
        }
    }

    #[test]
    fn canonical_ir_and_profile_substitutions_fail_after_source_authentication() {
        let mut ir = receipt();
        ir.ir.as_mut().unwrap().collectives.swap(0, 1);
        assert!(matches!(
            ir.consume(),
            Err(CollectedWave64CollectivesErrorV1::CanonicalIr(_))
        ));

        let mut profile = receipt();
        profile.profile.as_mut().unwrap().code_object_version = 5;
        assert!(matches!(
            profile.consume(),
            Err(CollectedWave64CollectivesErrorV1::CanonicalIr(_))
        ));
    }
}
