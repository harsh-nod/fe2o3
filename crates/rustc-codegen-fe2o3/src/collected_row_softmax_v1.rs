//! Source-authenticated admission for one fixed row-softmax V1 profile.
//!
//! This layer authenticates one exact rustc root and consumes a private receipt
//! to select one canonical Kernel IR module and descriptor source. Its proof
//! deliberately stops there: downstream compiler code may lower and publish an
//! inert handoff, but this source receipt grants no exp implementation,
//! numerical refinement, LLVM, link, machine-body, load, launch, memory-safety,
//! or race-freedom authority.

use std::collections::BTreeSet;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::File;
use std::io::{Read as _, Write as _};
use std::os::fd::{AsRawFd as _, FromRawFd as _, OwnedFd, RawFd};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use fe2o3_artifacts::{
    AbiKind, Access, AddressSpace as ArtifactAddressSpace, AliasClass, ArgumentOwnership,
    BlockSize, Capability, Dimensions, Endianness, IdentityText, LaunchContract,
    Mutability as ArtifactMutability, PointerWidth, RustScalarElementTypeV1, TargetIdentity,
};
use fe2o3_build_authority::{COMPILER_CLOSURE_IDENTITY_DOMAIN_V2, CompilerClosureV2};
use fe2o3_compiler_ffi::CompilerDescriptorSourceV1;
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
use crate::protected_rustc_invocation::AdmittedProtectedRustcInvocationV1;
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
const CARGO_METADATA_BUILD_OBSERVATION_ENV_V2: &str = "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2";
#[cfg(feature = "row-softmax-metadata-mutation-test-only")]
const CARGO_METADATA_MUTATION_TEST_ONLY_ENV_V1: &str = "FE2O3_CARGO_METADATA_MUTATION_TEST_ONLY_V1";
const EXPECTED_COMPILER_CLOSURE_SHA256_ENV_V1: &str = "FE2O3_EXPECTED_COMPILER_CLOSURE_SHA256_V1";
const CARGO_METADATA_BUILD_OBSERVATION_DOMAIN_V2: &[u8] =
    b"FE2O3/CARGO-METADATA-BUILD-OBSERVATION/V2\0";
const ROW_SOFTMAX_EFFECTIVE_RUSTC_ARGV_DOMAIN_V1: &[u8] =
    b"FE2O3/ROW-SOFTMAX/EFFECTIVE-RUSTC-ARGV/V1\0";
const MANAGED_ARTIFACT_DIRECTORY: &str =
    fe2o3_artifact_transaction::BROKERED_ARTIFACT_DIRECTORY_PATH_V1;
const MANAGED_CODEGEN_BACKEND: &str = fe2o3_artifact_transaction::BROKERED_CODEGEN_BACKEND_PATH_V1;
const MAX_MANAGED_RUSTC_ARGUMENTS: usize = 4096;
const MAX_MANAGED_RUSTC_ARGUMENT_BYTES: usize = 1024 * 1024;
const CARGO_GENERATED_METADATA_SHAPE_V1: &[u8] = b"one-16-byte-lowercase-hex-token";
const COLLECTED_AUTHORITY_DOMAIN_V1: &[u8] = b"fe2o3.row-softmax.collected-authority.v1";
const COLLECTED_AUTHORITY_DOMAIN_V2: &[u8] = b"fe2o3.row-softmax.collected-authority.v2";
pub(crate) const MAX_ROW_SOFTMAX_AUTHORITY_TRANSCRIPT_BYTES_V1: usize = 4096;
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
    0x93, 0x7a, 0xe7, 0x1f, 0xa9, 0x7c, 0x7e, 0x4a, 0x78, 0x2e, 0x5b, 0x27, 0xec, 0x80, 0xa1, 0x00,
    0x8d, 0xf8, 0x96, 0x6c, 0xd0, 0xd1, 0x28, 0xe4, 0xfd, 0x03, 0xbe, 0xd6, 0x6a, 0x1d, 0xc6, 0xf0,
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
enum ManagedCompilerClosureAuthorityV1 {
    UnprotectedQualificationV1([u8; 32]),
    ProtectedV2(CompilerClosureV2),
}

impl ManagedCompilerClosureAuthorityV1 {
    fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::UnprotectedQualificationV1(identity) if identity == &[0; 32] => {
                Err("row-softmax compiler closure identity is absent")
            }
            Self::UnprotectedQualificationV1(_) | Self::ProtectedV2(_) => Ok(()),
        }
    }

    const fn transcript_domain(&self) -> &'static [u8] {
        match self {
            Self::UnprotectedQualificationV1(_) => COLLECTED_AUTHORITY_DOMAIN_V1,
            Self::ProtectedV2(_) => COLLECTED_AUTHORITY_DOMAIN_V2,
        }
    }

    fn identity_sha256(&self) -> [u8; 32] {
        match self {
            Self::UnprotectedQualificationV1(identity) => *identity,
            Self::ProtectedV2(closure) => closure.identity_sha256(),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct ManagedBuildAuthorityV1 {
    generation: u64,
    session: [u8; 16],
    invocation: [u8; 32],
    cargo_metadata_transcript: [u8; 32],
    compiler_closure: ManagedCompilerClosureAuthorityV1,
    broker_executable: [u8; 32],
}

impl ManagedBuildAuthorityV1 {
    fn validate(&self) -> Result<(), &'static str> {
        if self.generation == 0 || self.session == [0; 16] || self.invocation == [0; 32] {
            return Err("row-softmax requires a non-direct managed wrapper build attempt");
        }
        if self.cargo_metadata_transcript == [0; 32] {
            return Err("row-softmax wrapper Cargo metadata transcript is absent");
        }
        self.compiler_closure.validate()?;
        Ok(())
    }

    #[cfg(test)]
    fn canonical_for_test(cargo_metadata_transcript: [u8; 32]) -> Self {
        Self {
            generation: 1,
            session: [0x11; 16],
            invocation: [0x22; 32],
            cargo_metadata_transcript,
            compiler_closure: ManagedCompilerClosureAuthorityV1::UnprotectedQualificationV1(
                exact_compiler_closure_policy_for_test().identity_sha256(),
            ),
            broker_executable: [0x04; 32],
        }
    }
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
    managed_build_authority: ManagedBuildAuthorityV1,
    descriptor_source_commitment: [u8; 32],
    authority_commitment: [u8; 32],
}

/// Opaque single-use authority minted only by exact rustc admission.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct RowSoftmaxFrontendReceiptV1 {
    authority: Option<RowSoftmaxFrontendAuthorityV1>,
    authority_transcript: Option<Vec<u8>>,
    descriptor_source: Option<CompilerDescriptorSourceV1>,
}

impl RowSoftmaxFrontendReceiptV1 {
    pub(crate) fn kernel_export(&self) -> &str {
        &self.authority().kernel_export
    }

    pub(crate) fn root_instance_identity(&self) -> &str {
        &self.authority().root_instance_identity
    }

    pub(crate) fn portable_mir_semantic_hex(&self) -> String {
        crate::encode_hex(&self.authority().portable_mir_semantic_commitment)
    }

    pub(crate) fn compiler_semantics_hex(&self) -> String {
        crate::encode_hex(&self.authority().compiler_semantics_commitment)
    }

    pub(crate) fn authority_hex(&self) -> String {
        crate::encode_hex(&self.authority().authority_commitment)
    }

    pub(crate) fn authority_commitment(&self) -> &[u8; 32] {
        &self.authority().authority_commitment
    }

    pub(crate) fn exponential_boundary_commitment(&self) -> &[u8; 32] {
        &self.authority().exponential_boundary_commitment
    }

    pub(crate) fn consume(
        &mut self,
    ) -> Result<AuthenticatedRowSoftmaxModuleV1, CollectedRowSoftmaxErrorV1> {
        let authority = self
            .authority
            .take()
            .ok_or(CollectedRowSoftmaxErrorV1::ReceiptAlreadyConsumed)?;
        let descriptor_source = self
            .descriptor_source
            .take()
            .ok_or(CollectedRowSoftmaxErrorV1::ReceiptAlreadyConsumed)?;
        validate_frontend_authority(&authority)?;
        let authority_transcript = self
            .authority_transcript
            .take()
            .ok_or(CollectedRowSoftmaxErrorV1::ReceiptAlreadyConsumed)?;
        if authority_transcript != collected_authority_transcript(&authority)
            || <[u8; 32]>::from(Sha256::digest(&authority_transcript))
                != authority.authority_commitment
        {
            return Err(CollectedRowSoftmaxErrorV1::ReceiptBindingMismatch {
                field: "authority transcript",
            });
        }
        if descriptor_source.identity().sha256() != &authority.descriptor_source_commitment {
            return Err(CollectedRowSoftmaxErrorV1::ReceiptBindingMismatch {
                field: "descriptor source",
            });
        }
        let module = canonical_row_softmax_v1_module();
        require_canonical_module(&module)?;
        if canonical_module_commitment(&module)? != authority.canonical_module_commitment {
            return Err(CollectedRowSoftmaxErrorV1::ReceiptBindingMismatch {
                field: "canonical module",
            });
        }
        Ok(AuthenticatedRowSoftmaxModuleV1 {
            module,
            descriptor_source,
            authority_transcript,
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
    descriptor_source: CompilerDescriptorSourceV1,
    authority_transcript: Vec<u8>,
    authority_commitment: [u8; 32],
    exponential_boundary_commitment: [u8; 32],
}

impl AuthenticatedRowSoftmaxModuleV1 {
    pub(crate) const fn authority_commitment(&self) -> &[u8; 32] {
        &self.authority_commitment
    }

    pub(crate) const fn exponential_boundary_commitment(&self) -> &[u8; 32] {
        &self.exponential_boundary_commitment
    }

    #[cfg(test)]
    pub(crate) fn authority_transcript(&self) -> &[u8] {
        &self.authority_transcript
    }

    pub(crate) fn into_parts(self) -> (Module, CompilerDescriptorSourceV1, Vec<u8>) {
        (
            self.module,
            self.descriptor_source,
            self.authority_transcript,
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
                crate::encode_hex(expected),
                crate::encode_hex(actual)
            ),
            Self::FnAbiIdentityMismatch { expected, actual } => write!(
                formatter,
                "collected row softmax V1 rustc FnAbi identity mismatch: expected {}, found {}",
                crate::encode_hex(expected),
                crate::encode_hex(actual)
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
    build_attempt: fe2o3_artifact_transaction::BuildAttempt,
    protected_rustc_invocation: Option<&AdmittedProtectedRustcInvocationV1>,
) -> Result<RowSoftmaxFrontendReceiptV1, CollectedRowSoftmaxErrorV1> {
    admit_execution_context(target.as_str(), custom_llvm_pipeline)?;
    let compiler_semantics = observe_compiler_semantics(tcx);
    let admitted_compiler_semantics = require_compiler_semantics(&compiler_semantics)?;
    let managed_build_authority = require_managed_build_authority(
        build_attempt,
        &admitted_compiler_semantics.cargo_metadata_build_observation,
        protected_rustc_invocation,
    )?;
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
    if provider_authority.provider.cargo_metadata_build_observation
        != managed_build_authority.cargo_metadata_transcript
    {
        return Err(CollectedRowSoftmaxErrorV1::CompilerSemantics {
            detail:
                "trusted provider and managed wrapper observed different Cargo metadata transcripts"
                    .to_owned(),
        });
    }
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
    let descriptor_roots = crate::compiler_descriptor::typed_descriptor_roots_from_collection(
        tcx,
        &collection.functions,
    )
    .map_err(|error| layout_mismatch(format!("descriptor evidence rejected: {error}")))?;
    let compiler_module = crate::kernel_ir_codegen::construct_inert_row_softmax_v1_module_text(
        &module,
    )
    .map_err(|error| unsupported_collection(format!("exact LLVM lowering failed: {error}")))?;
    let exponential_boundary_commitment = exponential_boundary_commitment();
    let envelope = crate::worker_v2_producer::construct_row_softmax_v1_compiler_envelope(
        exponential_boundary_commitment,
    )
    .map_err(|error| unsupported_collection(format!("compiler envelope failed: {error}")))?;
    let descriptor_source =
        crate::compiler_descriptor::construct_row_softmax_v1_compiler_descriptor_source_v1(
            &envelope,
            &module,
            &compiler_module,
            &descriptor_roots,
        )
        .map_err(|error| layout_mismatch(format!("descriptor source rejected: {error}")))?
        .ok_or_else(|| layout_mismatch("compiler descriptor source is absent"))?;
    let descriptor_source_commitment = *descriptor_source.identity().sha256();
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
        exponential_boundary_commitment,
        frontend_contract_commitment: sha256(EXACT_FRONTEND_CONTRACT_V1),
        canonical_module_commitment,
        kernel_export: root.export_name.clone(),
        root_instance_identity,
        portable_mir_semantic_commitment,
        compiler_semantics_commitment: admitted_compiler_semantics.normalized_commitment,
        cargo_metadata_build_observation: admitted_compiler_semantics
            .cargo_metadata_build_observation,
        provider_authority,
        managed_build_authority,
        descriptor_source_commitment,
        authority_commitment: [0; 32],
    };
    let authority_transcript = collected_authority_transcript(&authority);
    authority.authority_commitment = Sha256::digest(&authority_transcript).into();
    Ok(RowSoftmaxFrontendReceiptV1 {
        authority: Some(authority),
        authority_transcript: Some(authority_transcript),
        descriptor_source: Some(descriptor_source),
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

    // The General V3 transport and frontend contract must bind the same exact
    // WG64 launch; accepting a legacy default here would split the generated
    // host identity from the compiler-authenticated execution contract.
    let transport_launch = contract.launch();
    if transport_launch.rank() != 1
        || transport_launch.block_size()
            != BlockSize::Exact(Dimensions::new(64, 1, 1).map_err(|error| {
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
            "general V3 layout transport contract drifted from its exact 64x1x1 profile",
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

fn require_managed_build_authority(
    attempt: fe2o3_artifact_transaction::BuildAttempt,
    metadata: &CargoMetadataBuildObservationV1,
    protected_rustc_invocation: Option<&AdmittedProtectedRustcInvocationV1>,
) -> Result<ManagedBuildAuthorityV1, CollectedRowSoftmaxErrorV1> {
    let observed = observed_metadata_transcript_for_managed_authority()
        .map_err(|detail| CollectedRowSoftmaxErrorV1::CompilerSemantics { detail })?;
    let compiler_closure = match protected_rustc_invocation {
        Some(admitted) => {
            ManagedCompilerClosureAuthorityV1::ProtectedV2(admitted.compiler_closure().map_err(
                |detail| CollectedRowSoftmaxErrorV1::CompilerSemantics {
                    detail: format!(
                        "cannot revalidate admitted protected compiler closure: {detail}"
                    ),
                },
            )?)
        }
        None => ManagedCompilerClosureAuthorityV1::UnprotectedQualificationV1(
            require_nonzero_lower_sha256_environment(EXPECTED_COMPILER_CLOSURE_SHA256_ENV_V1)
                .map_err(|detail| CollectedRowSoftmaxErrorV1::CompilerSemantics { detail })?,
        ),
    };
    let observed_invocation =
        observe_managed_wrapper_effective_rustc_argv(compiler_closure.identity_sha256())?;
    let broker_executable = consume_brokered_invocation_authority(attempt, observed_invocation)?;
    admit_managed_build_authority(
        attempt,
        metadata,
        observed,
        compiler_closure,
        observed_invocation,
        broker_executable,
    )
}

fn observed_metadata_transcript_for_managed_authority() -> Result<[u8; 32], String> {
    let genuine = decode_lower_sha256_environment(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2)?;
    #[cfg(feature = "row-softmax-metadata-mutation-test-only")]
    {
        apply_metadata_mutation_test_only(
            genuine,
            std::env::var_os(CARGO_METADATA_MUTATION_TEST_ONLY_ENV_V1).as_deref(),
        )
    }
    #[cfg(not(feature = "row-softmax-metadata-mutation-test-only"))]
    {
        Ok(genuine)
    }
}

#[cfg(feature = "row-softmax-metadata-mutation-test-only")]
fn apply_metadata_mutation_test_only(
    genuine: [u8; 32],
    mutation: Option<&OsStr>,
) -> Result<[u8; 32], String> {
    match mutation {
        None => Ok(genuine),
        Some(value) if value == OsStr::new("omit") => Err(format!(
            "managed wrapper omitted {CARGO_METADATA_BUILD_OBSERVATION_ENV_V2}"
        )),
        Some(value) if value == OsStr::new("substitute") => Ok([0x01; 32]),
        Some(value) => Err(format!(
            "managed wrapper supplied invalid test-only metadata mutation {value:?}"
        )),
    }
}

fn admit_managed_build_authority(
    attempt: fe2o3_artifact_transaction::BuildAttempt,
    metadata: &CargoMetadataBuildObservationV1,
    observed_metadata_transcript: [u8; 32],
    compiler_closure: ManagedCompilerClosureAuthorityV1,
    observed_invocation: [u8; 32],
    broker_executable: [u8; 32],
) -> Result<ManagedBuildAuthorityV1, CollectedRowSoftmaxErrorV1> {
    let expected_metadata_transcript = cargo_metadata_build_transcript(metadata);
    if observed_metadata_transcript != expected_metadata_transcript {
        return Err(CollectedRowSoftmaxErrorV1::CompilerSemantics {
            detail: format!(
                "managed wrapper Cargo metadata transcript does not match rustc's ordered -Cmetadata values: expected {}, found {}",
                crate::encode_hex(&expected_metadata_transcript),
                crate::encode_hex(&observed_metadata_transcript)
            ),
        });
    }
    if attempt.invocation().as_bytes() != &observed_invocation {
        return Err(CollectedRowSoftmaxErrorV1::CompilerSemantics {
            detail: format!(
                "managed wrapper effective rustc argv does not match build attempt invocation: expected {}, found {}",
                crate::encode_hex(attempt.invocation().as_bytes()),
                crate::encode_hex(&observed_invocation)
            ),
        });
    }
    let authority = ManagedBuildAuthorityV1 {
        generation: attempt.generation(),
        session: *attempt.session().as_bytes(),
        invocation: observed_invocation,
        cargo_metadata_transcript: observed_metadata_transcript,
        compiler_closure,
        broker_executable,
    };
    authority
        .validate()
        .map_err(|detail| CollectedRowSoftmaxErrorV1::CompilerSemantics {
            detail: detail.to_owned(),
        })?;
    Ok(authority)
}

fn cargo_metadata_build_transcript(metadata: &CargoMetadataBuildObservationV1) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(CARGO_METADATA_BUILD_OBSERVATION_DOMAIN_V2);
    digest.update((metadata.ordered_tokens.len() as u64).to_le_bytes());
    for token in &metadata.ordered_tokens {
        digest.update((token.len() as u64).to_le_bytes());
        digest.update(token.as_bytes());
    }
    digest.finalize().into()
}

fn observe_managed_wrapper_effective_rustc_argv(
    compiler_closure_sha256: [u8; 32],
) -> Result<[u8; 32], CollectedRowSoftmaxErrorV1> {
    let argv = std::env::args_os().collect::<Vec<_>>();
    validate_managed_wrapper_effective_rustc_argv(&argv)
        .map_err(|detail| CollectedRowSoftmaxErrorV1::CompilerSemantics { detail })?;

    let artifact_directory = std::fs::metadata(MANAGED_ARTIFACT_DIRECTORY).map_err(|error| {
        CollectedRowSoftmaxErrorV1::CompilerSemantics {
            detail: format!(
                "managed wrapper brokered artifact-directory capability is unavailable: {error}"
            ),
        }
    })?;
    if !artifact_directory.is_dir()
        || std::env::var_os("FE2O3_HSACO_DIR").as_deref()
            != Some(OsStr::new(MANAGED_ARTIFACT_DIRECTORY))
    {
        return Err(CollectedRowSoftmaxErrorV1::CompilerSemantics {
            detail:
                "managed wrapper did not install the exact brokered artifact-directory capability"
                    .to_owned(),
        });
    }
    let backend = std::fs::metadata(MANAGED_CODEGEN_BACKEND).map_err(|error| {
        CollectedRowSoftmaxErrorV1::CompilerSemantics {
            detail: format!(
                "managed wrapper brokered codegen-backend capability is unavailable: {error}"
            ),
        }
    })?;
    if !backend.is_file() {
        return Err(CollectedRowSoftmaxErrorV1::CompilerSemantics {
            detail: "managed wrapper codegen-backend capability is not a regular file".to_owned(),
        });
    }

    Ok(effective_rustc_argv_identity(
        &argv,
        compiler_closure_sha256,
    ))
}

fn validate_managed_wrapper_effective_rustc_argv(argv: &[OsString]) -> Result<(), String> {
    if argv.len() < 5 || argv.len() > MAX_MANAGED_RUSTC_ARGUMENTS {
        return Err("managed wrapper effective rustc argv has an invalid count".to_owned());
    }
    if argv.iter().any(|argument| {
        os_bytes(argument).len() > MAX_MANAGED_RUSTC_ARGUMENT_BYTES
            || os_bytes(argument).starts_with(b"@")
    }) {
        return Err(
            "managed wrapper effective rustc argv contains an oversized argument or response file"
                .to_owned(),
        );
    }

    let tail = &argv[argv.len() - 4..];
    let backend_selector = format!("-Zcodegen-backend={MANAGED_CODEGEN_BACKEND}");
    if tail[0] != "-Zmir-enable-passes=-JumpThreading"
        || tail[1] != "--cfg"
        || tail[3] != backend_selector.as_str()
    {
        return Err(
            "managed wrapper effective rustc argv omitted or changed its exact managed tail"
                .to_owned(),
        );
    }
    let generation = os_bytes(&tail[2]);
    let prefix = b"fe2o3_codegen_generation=\"";
    if generation.len() != prefix.len() + 32 + 1
        || !generation.starts_with(prefix)
        || generation.last() != Some(&b'"')
        || generation[prefix.len()..generation.len() - 1]
            .iter()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(byte))
    {
        return Err(
            "managed wrapper effective rustc argv has a malformed generation selector".to_owned(),
        );
    }
    for (index, argument) in argv[..argv.len() - 4].iter().enumerate() {
        let bytes = os_bytes(argument);
        if bytes.starts_with(b"-Zcodegen-backend")
            || (bytes == b"-Z"
                && argv
                    .get(index + 1)
                    .is_some_and(|next| os_bytes(next).starts_with(b"codegen-backend")))
        {
            return Err(
                "managed wrapper effective rustc argv contains a preexisting backend selector"
                    .to_owned(),
            );
        }
    }
    Ok(())
}

fn effective_rustc_argv_identity(argv: &[OsString], compiler_closure_sha256: [u8; 32]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(ROW_SOFTMAX_EFFECTIVE_RUSTC_ARGV_DOMAIN_V1);
    digest.update((argv.len() as u64).to_le_bytes());
    for argument in argv {
        hash_field(&mut digest, os_bytes(argument));
    }
    *fe2o3_artifact_transaction::BuildInvocation::from_bytes(digest.finalize().into())
        .bind_compiler_closure_v1(compiler_closure_sha256)
        .as_bytes()
}

const MAX_INVOCATION_PEER_PROC_BYTES: usize = 4096;

struct RetainedInvocationAuthorityPeer {
    pid: u32,
    uid: u32,
    start_time_ticks: u64,
    pidfd: OwnedFd,
}

impl RetainedInvocationAuthorityPeer {
    fn retain(stream: &UnixStream) -> Result<Self, String> {
        let credentials = invocation_peer_credentials(stream)?;
        if credentials.pid <= 0 || credentials.uid != unsafe { libc::geteuid() } {
            return Err(
                "invocation-capability socket has no same-UID positive-PID peer".to_owned(),
            );
        }
        let pid = u32::try_from(credentials.pid)
            .map_err(|_| "invocation-capability peer PID exceeds u32".to_owned())?;
        let pidfd = invocation_peer_pidfd(stream)?;
        let retained = Self {
            pid,
            uid: credentials.uid,
            start_time_ticks: invocation_peer_start_time_ticks(pid)?,
            pidfd,
        };
        retained.require_live()?;
        let confirmed = invocation_peer_credentials(stream)?;
        if confirmed.pid != credentials.pid
            || confirmed.uid != credentials.uid
            || confirmed.gid != credentials.gid
        {
            return Err("invocation-capability peer credentials changed while retaining it".into());
        }
        retained.require_live()?;
        Ok(retained)
    }

    fn require_live(&self) -> Result<(), String> {
        if self.uid != unsafe { libc::geteuid() } {
            return Err("retained invocation-capability peer UID changed".to_owned());
        }
        if invocation_peer_pidfd_pid(&self.pidfd)? != self.pid {
            return Err(
                "retained invocation-capability pidfd does not identify SO_PEERCRED PID".to_owned(),
            );
        }
        if invocation_peer_start_time_ticks(self.pid)? != self.start_time_ticks {
            return Err("retained invocation-capability peer start time changed".to_owned());
        }
        require_invocation_peer_pidfd_live(&self.pidfd)?;
        if invocation_peer_pidfd_pid(&self.pidfd)? != self.pid
            || invocation_peer_start_time_ticks(self.pid)? != self.start_time_ticks
        {
            return Err(
                "retained invocation-capability peer changed during liveness check".to_owned(),
            );
        }
        Ok(())
    }

    fn measure_executable(&self) -> Result<[u8; 32], String> {
        self.require_live()?;
        let executable_path = PathBuf::from(format!("/proc/{}/exe", self.pid));
        let executable = File::open(&executable_path).map_err(|error| {
            format!(
                "cannot pin invocation-capability peer executable {}: {error}",
                executable_path.display()
            )
        })?;
        self.require_live()?;
        let descriptor_path = PathBuf::from(format!("/proc/self/fd/./{}", executable.as_raw_fd()));
        let sha256 = fe2o3_process_identity::measure_executable_sha256_v3(&descriptor_path)
            .map_err(|error| {
                format!("cannot measure pinned invocation-capability peer executable: {error}")
            })?;
        self.require_live()?;
        Ok(sha256)
    }
}

fn invocation_peer_credentials(stream: &UnixStream) -> Result<libc::ucred, String> {
    let mut credentials = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let expected_bytes = libc::socklen_t::try_from(std::mem::size_of::<libc::ucred>())
        .expect("ucred size fits socklen_t");
    let mut credentials_bytes = expected_bytes;
    // SAFETY: the output pointers name one initialized `ucred` and its exact byte count.
    let result = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            std::ptr::addr_of_mut!(credentials).cast(),
            &mut credentials_bytes,
        )
    };
    if result != 0 || credentials_bytes != expected_bytes {
        return Err(format!(
            "cannot read invocation-capability SO_PEERCRED: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(credentials)
}

fn invocation_peer_pidfd(stream: &UnixStream) -> Result<OwnedFd, String> {
    let mut raw_pidfd: libc::c_int = -1;
    let expected_bytes = libc::socklen_t::try_from(std::mem::size_of::<libc::c_int>())
        .expect("pidfd size fits socklen_t");
    let mut pidfd_bytes = expected_bytes;
    // SAFETY: the output pointers name one initialized `c_int` and its exact byte count.
    let result = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERPIDFD,
            std::ptr::addr_of_mut!(raw_pidfd).cast(),
            &mut pidfd_bytes,
        )
    };
    if result != 0 || pidfd_bytes != expected_bytes || raw_pidfd < 0 {
        if raw_pidfd >= 0 {
            // SAFETY: a successful `SO_PEERPIDFD` returned this new owned descriptor.
            drop(unsafe { OwnedFd::from_raw_fd(raw_pidfd) });
        }
        return Err(format!(
            "cannot retain invocation-capability peer with SO_PEERPIDFD: {}",
            std::io::Error::last_os_error()
        ));
    }
    // SAFETY: successful `SO_PEERPIDFD` returned one new descriptor owned by this process.
    let pidfd = unsafe { OwnedFd::from_raw_fd(raw_pidfd) };
    let descriptor_flags = unsafe { libc::fcntl(pidfd.as_raw_fd(), libc::F_GETFD) };
    if descriptor_flags < 0
        || unsafe {
            libc::fcntl(
                pidfd.as_raw_fd(),
                libc::F_SETFD,
                descriptor_flags | libc::FD_CLOEXEC,
            )
        } != 0
    {
        return Err(format!(
            "cannot make invocation-capability peer pidfd close-on-exec: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(pidfd)
}

fn read_bounded_invocation_peer_proc(path: &Path, label: &str) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::with_capacity(MAX_INVOCATION_PEER_PROC_BYTES + 1);
    File::open(path)
        .map_err(|error| format!("cannot open {label} {}: {error}", path.display()))?
        .take(
            u64::try_from(MAX_INVOCATION_PEER_PROC_BYTES + 1)
                .expect("bounded proc record size fits u64"),
        )
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read {label} {}: {error}", path.display()))?;
    if bytes.is_empty() || bytes.len() > MAX_INVOCATION_PEER_PROC_BYTES {
        return Err(format!(
            "{label} {} must contain 1 through {MAX_INVOCATION_PEER_PROC_BYTES} bytes",
            path.display()
        ));
    }
    Ok(bytes)
}

fn invocation_peer_start_time_ticks(pid: u32) -> Result<u64, String> {
    let path = PathBuf::from(format!("/proc/{pid}/stat"));
    let bytes = read_bounded_invocation_peer_proc(&path, "invocation-capability peer stat")?;
    let close = bytes
        .iter()
        .rposition(|byte| *byte == b')')
        .ok_or_else(|| "invocation-capability peer stat has no command terminator".to_owned())?;
    let recorded_pid = bytes[..close]
        .split(|byte| *byte == b' ')
        .next()
        .and_then(|value| std::str::from_utf8(value).ok())
        .and_then(|value| value.parse::<u32>().ok());
    if recorded_pid != Some(pid) {
        return Err("invocation-capability peer stat PID differs from its proc entry".to_owned());
    }
    bytes[close + 1..]
        .split(u8::is_ascii_whitespace)
        .filter(|field| !field.is_empty())
        .nth(19)
        .and_then(|value| std::str::from_utf8(value).ok())
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value != 0)
        .ok_or_else(|| "invocation-capability peer stat has no valid start time".to_owned())
}

fn invocation_peer_pidfd_pid(pidfd: &OwnedFd) -> Result<u32, String> {
    let path = PathBuf::from(format!("/proc/self/fdinfo/{}", pidfd.as_raw_fd()));
    let bytes = read_bounded_invocation_peer_proc(&path, "invocation-capability pidfd info")?;
    let record = std::str::from_utf8(&bytes)
        .map_err(|_| "invocation-capability pidfd info is not UTF-8".to_owned())?;
    record
        .lines()
        .find_map(|line| line.strip_prefix("Pid:"))
        .and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|pid| *pid != 0)
        .ok_or_else(|| "invocation-capability pidfd has no live positive PID".to_owned())
}

fn require_invocation_peer_pidfd_live(pidfd: &OwnedFd) -> Result<(), String> {
    let mut descriptor = libc::pollfd {
        fd: pidfd.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    for _ in 0..8 {
        // SAFETY: the pointer names one initialized poll descriptor and timeout zero cannot block.
        let result = unsafe { libc::poll(std::ptr::addr_of_mut!(descriptor), 1, 0) };
        if result == 0 && descriptor.revents == 0 {
            return Ok(());
        }
        if result > 0 || descriptor.revents != 0 {
            return Err("invocation-capability peer exited during authentication".to_owned());
        }
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::Interrupted {
            return Err(format!(
                "cannot poll invocation-capability peer pidfd: {error}"
            ));
        }
    }
    Err("invocation-capability peer pidfd polling was repeatedly interrupted".to_owned())
}

fn consume_brokered_invocation_authority(
    attempt: fe2o3_artifact_transaction::BuildAttempt,
    effective_argv_sha256: [u8; 32],
) -> Result<[u8; 32], CollectedRowSoftmaxErrorV1> {
    const INVOCATION_AUTHORITY_FD: RawFd =
        fe2o3_artifact_transaction::BROKERED_INVOCATION_AUTHORITY_CHILD_FD_V1;
    // SAFETY: this function is the unique consumer of the fixed descriptor installed by the
    // managed wrapper. Taking ownership early also closes it on every authentication error.
    let mut stream = unsafe { UnixStream::from_raw_fd(INVOCATION_AUTHORITY_FD) };
    let expected_broker = embedded_cargo_fe2o3_executable_identity()?;
    let peer = RetainedInvocationAuthorityPeer::retain(&stream).map_err(|detail| {
        CollectedRowSoftmaxErrorV1::CompilerSemantics {
            detail: format!("cannot retain invocation-capability broker peer: {detail}"),
        }
    })?;
    let observed_broker = peer.measure_executable().map_err(|detail| {
        CollectedRowSoftmaxErrorV1::CompilerSemantics {
            detail: format!(
                "cannot authenticate invocation-capability broker executable: {detail}"
            ),
        }
    })?;
    if observed_broker != expected_broker {
        return Err(CollectedRowSoftmaxErrorV1::CompilerSemantics {
            detail: format!(
                "invocation-capability peer is not the cargo-fe2o3 executable pinned into this backend: expected {}, found {}",
                crate::encode_hex(&expected_broker),
                crate::encode_hex(&observed_broker),
            ),
        });
    }

    let claim = fe2o3_artifact_transaction::BrokeredInvocationCapabilityClaimV1::new(
        attempt,
        effective_argv_sha256,
    )
    .map_err(|error| CollectedRowSoftmaxErrorV1::CompilerSemantics {
        detail: format!("invalid brokered managed-wrapper invocation claim: {error}"),
    })?;
    let timeout = Some(Duration::from_secs(30));
    stream.set_read_timeout(timeout).map_err(|error| {
        CollectedRowSoftmaxErrorV1::CompilerSemantics {
            detail: format!("cannot bound invocation-capability read: {error}"),
        }
    })?;
    stream.set_write_timeout(timeout).map_err(|error| {
        CollectedRowSoftmaxErrorV1::CompilerSemantics {
            detail: format!("cannot bound invocation-capability write: {error}"),
        }
    })?;
    stream
        .write_all(
            &fe2o3_artifact_transaction::BrokeredInvocationCapabilityRequestV1::Consume(claim)
                .encode(),
        )
        .map_err(|error| CollectedRowSoftmaxErrorV1::CompilerSemantics {
            detail: format!("cannot consume brokered managed-wrapper invocation: {error}"),
        })?;
    let mut response = [0_u8; 16];
    stream.read_exact(&mut response).map_err(|error| {
        CollectedRowSoftmaxErrorV1::CompilerSemantics {
            detail: format!("broker did not admit the managed-wrapper invocation: {error}"),
        }
    })?;
    if response != *fe2o3_artifact_transaction::BROKERED_INVOCATION_ADMITTED_V1 {
        return Err(CollectedRowSoftmaxErrorV1::CompilerSemantics {
            detail: "broker returned a malformed managed-wrapper invocation admission".to_owned(),
        });
    }
    peer.require_live()
        .map_err(|detail| CollectedRowSoftmaxErrorV1::CompilerSemantics {
            detail: format!(
                "invocation-capability broker peer did not survive admission: {detail}"
            ),
        })?;
    Ok(observed_broker)
}

fn embedded_cargo_fe2o3_executable_identity() -> Result<[u8; 32], CollectedRowSoftmaxErrorV1> {
    let encoded = option_env!("FE2O3_BUILD_CARGO_FE2O3_EXECUTABLE_SHA256_V1").ok_or_else(|| {
        CollectedRowSoftmaxErrorV1::CompilerSemantics {
            detail: "backend has no cargo-fe2o3 executable identity for broker authentication"
                .to_owned(),
        }
    })?;
    decode_lower_sha256(encoded)
        .map_err(|detail| CollectedRowSoftmaxErrorV1::CompilerSemantics { detail })
}

fn decode_lower_sha256(encoded: &str) -> Result<[u8; 32], String> {
    if encoded.len() != 64
        || !encoded
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("embedded cargo-fe2o3 executable identity is malformed".to_owned());
    }
    let mut digest = [0; 32];
    for (output, pair) in digest.iter_mut().zip(encoded.as_bytes().chunks_exact(2)) {
        *output = (lower_hex_value(pair[0]) << 4) | lower_hex_value(pair[1]);
    }
    if digest == [0; 32] {
        return Err("embedded cargo-fe2o3 executable identity is zero".to_owned());
    }
    Ok(digest)
}

#[cfg(unix)]
fn os_bytes(value: &OsStr) -> &[u8] {
    use std::os::unix::ffi::OsStrExt as _;
    value.as_bytes()
}

#[cfg(not(unix))]
fn os_bytes(value: &OsStr) -> &[u8] {
    value.to_str().unwrap_or_default().as_bytes()
}

fn decode_lower_sha256_environment(name: &str) -> Result<[u8; 32], String> {
    let value = std::env::var(name).map_err(|_| format!("managed wrapper omitted {name}"))?;
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("managed wrapper supplied malformed {name}"));
    }
    let mut digest = [0; 32];
    for (output, pair) in digest.iter_mut().zip(value.as_bytes().chunks_exact(2)) {
        *output = (lower_hex_value(pair[0]) << 4) | lower_hex_value(pair[1]);
    }
    Ok(digest)
}

fn require_nonzero_lower_sha256_environment(name: &str) -> Result<[u8; 32], String> {
    let value = std::env::var(name).ok();
    decode_required_nonzero_lower_sha256(name, value.as_deref())
}

fn decode_required_nonzero_lower_sha256(
    name: &str,
    encoded: Option<&str>,
) -> Result<[u8; 32], String> {
    let encoded = encoded.ok_or_else(|| format!("managed wrapper omitted {name}"))?;
    if encoded.len() != 64
        || !encoded
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("managed wrapper supplied malformed {name}"));
    }
    let mut digest = [0; 32];
    for (output, pair) in digest.iter_mut().zip(encoded.as_bytes().chunks_exact(2)) {
        *output = (lower_hex_value(pair[0]) << 4) | lower_hex_value(pair[1]);
    }
    if digest == [0; 32] {
        return Err(format!("managed wrapper supplied zero {name}"));
    }
    Ok(digest)
}

fn lower_hex_value(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => unreachable!("lowercase hexadecimal shape checked before decoding"),
    }
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

pub(crate) fn canonical_row_softmax_v1_module() -> Module {
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
                crate::encode_hex(&REVIEWED_CANONICAL_MODULE_V4_COMMITMENT),
                crate::encode_hex(&actual_commitment),
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

fn collected_authority_transcript(authority: &RowSoftmaxFrontendAuthorityV1) -> Vec<u8> {
    let mut transcript = Vec::with_capacity(1024);
    push_transcript_field(
        &mut transcript,
        authority
            .managed_build_authority
            .compiler_closure
            .transcript_domain(),
    );
    push_transcript_field(&mut transcript, &authority.portable_mir_semantic_commitment);
    push_transcript_field(&mut transcript, &authority.compiler_semantics_commitment);
    push_transcript_field(&mut transcript, &authority.canonical_module_commitment);
    push_transcript_field(&mut transcript, &authority.descriptor_source_commitment);
    push_transcript_field(&mut transcript, authority.root_instance_identity.as_bytes());
    push_transcript_field(&mut transcript, authority.kernel_export.as_bytes());
    push_transcript_field(&mut transcript, authority.target.as_bytes());
    push_transcript_field(
        &mut transcript,
        &authority.code_object_version.to_le_bytes(),
    );
    push_transcript_field(
        &mut transcript,
        &authority.explicit_kernarg_bytes.to_le_bytes(),
    );
    push_transcript_field(
        &mut transcript,
        &authority.complete_kernarg_bytes.to_le_bytes(),
    );
    push_transcript_field(&mut transcript, &authority.row_elements.to_le_bytes());
    push_transcript_field(&mut transcript, &authority.abi_binding_commitment);
    push_transcript_field(&mut transcript, &authority.fn_abi_binding_commitment);
    push_transcript_field(&mut transcript, &authority.launch_binding_commitment);
    push_transcript_field(&mut transcript, &authority.correspondence_commitment);
    push_transcript_field(&mut transcript, &authority.exponential_boundary_commitment);
    push_transcript_field(&mut transcript, &authority.frontend_contract_commitment);
    for token in &authority.cargo_metadata_build_observation.ordered_tokens {
        push_transcript_field(&mut transcript, token.as_bytes());
    }
    push_transcript_field(
        &mut transcript,
        &authority.cargo_metadata_build_observation.commitment,
    );
    push_transcript_field(
        &mut transcript,
        authority.provider_authority.provider.crate_name.as_bytes(),
    );
    push_transcript_field(
        &mut transcript,
        &authority
            .provider_authority
            .provider
            .stable_crate_id
            .to_le_bytes(),
    );
    push_transcript_field(
        &mut transcript,
        &authority.provider_authority.provider.crate_hash,
    );
    push_transcript_field(
        &mut transcript,
        &authority
            .provider_authority
            .provider
            .cargo_metadata_build_observation,
    );
    push_transcript_field(
        &mut transcript,
        &authority.provider_authority.provider.source_identity,
    );
    for identity in &authority.provider_authority.definition_identities {
        push_transcript_field(&mut transcript, identity);
    }
    for identity in &authority.provider_authority.source_identities {
        push_transcript_field(&mut transcript, identity);
    }
    push_transcript_field(&mut transcript, &authority.provider_authority.commitment);
    push_transcript_field(
        &mut transcript,
        &authority.managed_build_authority.generation.to_le_bytes(),
    );
    push_transcript_field(&mut transcript, &authority.managed_build_authority.session);
    push_transcript_field(
        &mut transcript,
        &authority.managed_build_authority.invocation,
    );
    push_transcript_field(
        &mut transcript,
        &authority.managed_build_authority.cargo_metadata_transcript,
    );
    push_compiler_closure_transcript(
        &mut transcript,
        &authority.managed_build_authority.compiler_closure,
    );
    push_transcript_field(
        &mut transcript,
        &authority.managed_build_authority.broker_executable,
    );
    assert!(
        transcript.len() <= MAX_ROW_SOFTMAX_AUTHORITY_TRANSCRIPT_BYTES_V1,
        "row-softmax authority transcript exceeds its fixed compiler bound"
    );
    transcript
}

fn push_compiler_closure_transcript(
    transcript: &mut Vec<u8>,
    closure: &ManagedCompilerClosureAuthorityV1,
) {
    match closure {
        ManagedCompilerClosureAuthorityV1::UnprotectedQualificationV1(identity) => {
            push_transcript_field(transcript, identity);
        }
        ManagedCompilerClosureAuthorityV1::ProtectedV2(closure) => {
            push_transcript_field(
                transcript,
                &compiler_closure_v2_canonical_preimage(*closure),
            );
            push_transcript_field(transcript, &closure.identity_sha256());
        }
    }
}

fn compiler_closure_v2_canonical_preimage(closure: CompilerClosureV2) -> Vec<u8> {
    let mut preimage = Vec::with_capacity(COMPILER_CLOSURE_IDENTITY_DOMAIN_V2.len() + 2 + 6 * 32);
    preimage.extend_from_slice(COMPILER_CLOSURE_IDENTITY_DOMAIN_V2);
    preimage.extend_from_slice(
        &closure
            .cargo_binding_transition_protocol_version()
            .to_le_bytes(),
    );
    for digest in [
        closure.cargo_executable_sha256(),
        closure.cargo_binding_trampoline_sha256(),
        closure.cargo_fe2o3_binding_wrapper_sha256(),
        closure.rustc_executable_sha256(),
        closure.rustc_runtime_tree_sha256(),
        closure.codegen_backend_sha256(),
    ] {
        preimage.extend_from_slice(&digest);
    }
    debug_assert_eq!(
        <[u8; 32]>::from(Sha256::digest(&preimage)),
        closure.identity_sha256()
    );
    preimage
}

fn collected_authority_commitment(authority: &RowSoftmaxFrontendAuthorityV1) -> [u8; 32] {
    Sha256::digest(collected_authority_transcript(authority)).into()
}

fn push_transcript_field(transcript: &mut Vec<u8>, bytes: &[u8]) {
    transcript.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    transcript.extend_from_slice(bytes);
}

fn validate_frontend_authority(
    authority: &RowSoftmaxFrontendAuthorityV1,
) -> Result<(), CollectedRowSoftmaxErrorV1> {
    let metadata_observation_is_invalid = authority
        .cargo_metadata_build_observation
        .validate()
        .is_err();
    let provider_authority_is_invalid = authority.provider_authority.validate().is_err();
    let managed_build_authority_is_invalid = authority.managed_build_authority.validate().is_err();
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
    } else if authority.descriptor_source_commitment == [0; 32] {
        Some("descriptor source")
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
    } else if managed_build_authority_is_invalid {
        Some("managed wrapper build attempt")
    } else if authority.managed_build_authority.broker_executable == [0; 32] {
        Some("brokered managed-wrapper invocation authority")
    } else if authority.managed_build_authority.cargo_metadata_transcript
        != authority
            .provider_authority
            .provider
            .cargo_metadata_build_observation
    {
        Some("wrapper Cargo metadata transcript")
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

pub(crate) fn exponential_boundary_commitment() -> [u8; 32] {
    domain_commitment(EXPONENTIAL_BOUNDARY_DOMAIN_V1, EXPONENTIAL_BOUNDARY_V1)
}

fn domain_commitment(domain: &[u8], value: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    hash_field(&mut digest, domain);
    hash_field(&mut digest, value);
    digest.finalize().into()
}

#[cfg(test)]
pub(crate) fn exact_frontend_receipt_for_test() -> RowSoftmaxFrontendReceiptV1 {
    let module = canonical_row_softmax_v1_module();
    let descriptor_source = crate::compiler_descriptor::row_softmax_v1_descriptor_source_for_test();
    let compiler_semantics = reviewed_compiler_semantics("0123456789abcdef");
    let admitted_compiler_semantics =
        require_compiler_semantics(&compiler_semantics).expect("reviewed compiler semantics");
    let metadata_transcript = cargo_metadata_build_transcript(
        &admitted_compiler_semantics.cargo_metadata_build_observation,
    );
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
        provider_authority: crate::mir_import::RowSoftmaxProviderAuthorityV1::canonical_for_test(
            metadata_transcript,
        ),
        managed_build_authority: ManagedBuildAuthorityV1::canonical_for_test(metadata_transcript),
        descriptor_source_commitment: *descriptor_source.identity().sha256(),
        authority_commitment: [0; 32],
    };
    let authority_transcript = collected_authority_transcript(&authority);
    authority.authority_commitment = Sha256::digest(&authority_transcript).into();
    RowSoftmaxFrontendReceiptV1 {
        authority: Some(authority),
        authority_transcript: Some(authority_transcript),
        descriptor_source: Some(descriptor_source),
    }
}

#[cfg(test)]
fn exact_compiler_closure_policy_for_test()
-> fe2o3_hsaco_finalize::RowSoftmaxV1CompilerClosurePolicyV1 {
    fe2o3_hsaco_finalize::RowSoftmaxV1CompilerClosurePolicyV1::new(
        [0x05; 32], [0x06; 32], [0x07; 32], [0x08; 32],
    )
    .expect("exact row test compiler closure")
}

#[cfg(test)]
pub(crate) fn exact_authority_policy_for_test()
-> fe2o3_hsaco_finalize::RowSoftmaxV1AuthorityPolicyV1 {
    use fe2o3_hsaco_finalize::{RowSoftmaxV1AuthorityPolicyV1, RowSoftmaxV1ProviderManifestV1};

    let receipt = exact_frontend_receipt_for_test();
    let authority = receipt.authority();
    let provider = &authority.provider_authority;
    let definitions = provider
        .definition_identities
        .as_slice()
        .try_into()
        .expect("exact row test provider definition count");
    let sources = provider
        .source_identities
        .as_slice()
        .try_into()
        .expect("exact row test provider source count");
    let manifest = RowSoftmaxV1ProviderManifestV1::new(
        provider.provider.stable_crate_id,
        provider.provider.crate_hash,
        definitions,
        sources,
    )
    .expect("exact row test provider manifest");
    let attempt = fe2o3_artifact_transaction::BuildAttempt::from_env_value(&format!(
        "{}:{}:{}",
        authority.managed_build_authority.generation,
        crate::encode_hex(&authority.managed_build_authority.session),
        crate::encode_hex(&authority.managed_build_authority.invocation),
    ))
    .expect("exact row test build attempt");
    RowSoftmaxV1AuthorityPolicyV1::new(
        manifest,
        attempt,
        authority.managed_build_authority.broker_executable,
        exact_compiler_closure_policy_for_test(),
    )
    .expect("exact row test authority policy")
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
    use std::os::unix::net::UnixListener;
    use std::process::{Child, Command, Stdio};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::thread;
    use std::time::Instant;

    const INVOCATION_PEER_HELPER_ENV: &str = "FE2O3_ROW_INVOCATION_PEER_HELPER_SOCKET";
    const INVOCATION_PEER_HELPER_TEST: &str =
        "collected_row_softmax_v1::tests::invocation_peer_process_helper";
    static NEXT_INVOCATION_PEER_TEST: AtomicU64 = AtomicU64::new(1);

    struct InvocationPeerFixture {
        directory: PathBuf,
        child: Option<Child>,
        stream: UnixStream,
        executable: PathBuf,
        expected_sha256: [u8; 32],
    }

    impl InvocationPeerFixture {
        fn spawn() -> Self {
            let directory = std::env::temp_dir().join(format!(
                "fe2o3-row-invocation-peer-{}-{}",
                std::process::id(),
                NEXT_INVOCATION_PEER_TEST.fetch_add(1, Ordering::Relaxed)
            ));
            let mut builder = fs::DirBuilder::new();
            use std::os::unix::fs::DirBuilderExt as _;
            builder.mode(0o700);
            builder
                .create(&directory)
                .expect("create private invocation peer test directory");
            let executable = directory.join("cargo-fe2o3-peer");
            fs::copy(
                std::env::current_exe().expect("locate invocation peer test executable"),
                &executable,
            )
            .expect("copy invocation peer test executable");
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o500))
                .expect("make invocation peer test executable owner-only");
            let expected_sha256 = fe2o3_process_identity::measure_executable_sha256_v3(&executable)
                .expect("measure invocation peer test executable");
            let socket = directory.join("peer.sock");
            let listener = UnixListener::bind(&socket).expect("bind invocation peer test socket");
            listener
                .set_nonblocking(true)
                .expect("make invocation peer listener nonblocking");
            let mut child = Command::new(&executable)
                .args(["--exact", INVOCATION_PEER_HELPER_TEST, "--nocapture"])
                .env(INVOCATION_PEER_HELPER_ENV, &socket)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawn invocation peer helper");
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if let Some(status) = child.try_wait().expect("poll invocation peer helper")
                        {
                            panic!("invocation peer helper exited before connect: {status}");
                        }
                        assert!(
                            Instant::now() < deadline,
                            "invocation peer helper connect timed out"
                        );
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("accept invocation peer helper: {error}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .expect("bound invocation peer helper ready read");
            let mut ready = [0_u8; 1];
            stream
                .read_exact(&mut ready)
                .expect("read invocation peer helper ready byte");
            assert_eq!(ready, [1]);
            Self {
                directory,
                child: Some(child),
                stream,
                executable,
                expected_sha256,
            }
        }

        fn retain(&self) -> RetainedInvocationAuthorityPeer {
            let peer = RetainedInvocationAuthorityPeer::retain(&self.stream)
                .expect("retain exact invocation peer");
            assert_eq!(
                peer.pid,
                self.child.as_ref().expect("live helper child").id()
            );
            peer
        }

        fn substitute_executable_path(&self) {
            let retained = self.directory.join("retained-running-peer");
            fs::rename(&self.executable, &retained)
                .expect("retain running invocation peer executable object");
            fs::copy("/bin/false", &self.executable)
                .expect("install same-UID invocation peer pathname substitution");
            let metadata = fs::metadata(&self.executable)
                .expect("inspect invocation peer pathname substitution");
            assert_eq!(metadata.uid(), unsafe { libc::geteuid() });
            assert_ne!(
                fe2o3_process_identity::measure_executable_sha256_v3(&self.executable)
                    .expect("measure invocation peer pathname substitution"),
                self.expected_sha256
            );
        }

        fn release_and_wait(&mut self) {
            self.stream
                .write_all(&[1])
                .expect("release invocation peer helper");
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                let child = self.child.as_mut().expect("live invocation peer helper");
                if let Some(status) = child.try_wait().expect("poll invocation peer helper exit") {
                    assert!(status.success(), "invocation peer helper failed: {status}");
                    self.child = None;
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "invocation peer helper exit timed out"
                );
                thread::sleep(Duration::from_millis(10));
            }
        }
    }

    impl Drop for InvocationPeerFixture {
        fn drop(&mut self) {
            if let Some(child) = self.child.as_mut() {
                let _ = child.kill();
                let _ = child.wait();
            }
            let _ = fs::remove_dir_all(&self.directory);
        }
    }

    #[test]
    fn invocation_peer_process_helper() {
        let Some(socket) = std::env::var_os(INVOCATION_PEER_HELPER_ENV) else {
            return;
        };
        let mut stream = UnixStream::connect(socket).expect("connect invocation peer helper");
        stream
            .write_all(&[1])
            .expect("publish invocation peer helper ready byte");
        let mut release = [0_u8; 1];
        stream
            .read_exact(&mut release)
            .expect("read invocation peer helper release byte");
        assert_eq!(release, [1]);
    }

    #[test]
    fn retained_invocation_peer_ignores_same_uid_path_substitution() {
        let mut fixture = InvocationPeerFixture::spawn();
        let peer = fixture.retain();
        fixture.substitute_executable_path();
        assert_eq!(
            peer.measure_executable()
                .expect("measure retained running invocation peer"),
            fixture.expected_sha256
        );
        fixture.release_and_wait();
    }

    #[test]
    fn retained_invocation_peer_rejects_pidfd_substitution_and_exit() {
        let mut fixture = InvocationPeerFixture::spawn();
        let mut peer = fixture.retain();
        let (self_socket, _self_peer) = UnixStream::pair().expect("create self pidfd socket");
        let substituted = invocation_peer_pidfd(&self_socket).expect("retain self pidfd");
        let original = std::mem::replace(&mut peer.pidfd, substituted);
        assert!(
            peer.require_live()
                .expect_err("substituted pidfd must fail")
                .contains("does not identify SO_PEERCRED PID")
        );
        peer.pidfd = original;
        peer.require_live().expect("restore exact peer pidfd");
        fixture.release_and_wait();
        assert!(
            peer.require_live()
                .expect_err("exited peer must fail")
                .contains("no live positive PID")
        );
    }

    fn managed_argv() -> Vec<OsString> {
        [
            "/toolchain/bin/rustc",
            "src/lib.rs",
            "--crate-name",
            "row_softmax",
            "-Cmetadata=0123456789abcdef",
            "-Cmetadata=fe2o3-row-softmax-v1-reviewed",
            "-Zmir-enable-passes=-JumpThreading",
            "--cfg",
            "fe2o3_codegen_generation=\"0123456789abcdef0123456789abcdef\"",
            "-Zcodegen-backend=/proc/./self/fd/198",
        ]
        .into_iter()
        .map(OsString::from)
        .collect()
    }

    #[test]
    fn managed_argv_is_reconstructed_independently_and_rejects_malformed_shapes() {
        let argv = managed_argv();
        let compiler_closure = [0x73; 32];
        validate_managed_wrapper_effective_rustc_argv(&argv).expect("exact managed argv");

        let mut oracle = Sha256::new();
        oracle.update(ROW_SOFTMAX_EFFECTIVE_RUSTC_ARGV_DOMAIN_V1);
        oracle.update((argv.len() as u64).to_le_bytes());
        for argument in &argv {
            let bytes = os_bytes(argument);
            oracle.update((bytes.len() as u64).to_le_bytes());
            oracle.update(bytes);
        }
        let oracle = fe2o3_artifact_transaction::BuildInvocation::from_bytes(<[u8; 32]>::from(
            oracle.finalize(),
        ))
        .bind_compiler_closure_v1(compiler_closure);
        assert_eq!(
            effective_rustc_argv_identity(&argv, compiler_closure),
            *oracle.as_bytes()
        );
        assert_ne!(
            effective_rustc_argv_identity(&argv, [0x74; 32]),
            *oracle.as_bytes()
        );

        let missing_tail = argv[..argv.len() - 4].to_vec();
        assert!(validate_managed_wrapper_effective_rustc_argv(&missing_tail).is_err());

        let mut malformed_generation = argv.clone();
        malformed_generation[argv.len() - 2] =
            OsString::from("fe2o3_codegen_generation=\"ABCDEF\"");
        assert!(validate_managed_wrapper_effective_rustc_argv(&malformed_generation).is_err());

        let mut substituted_backend = argv.clone();
        substituted_backend[argv.len() - 1] =
            OsString::from("-Zcodegen-backend=/tmp/substitute.so");
        assert!(validate_managed_wrapper_effective_rustc_argv(&substituted_backend).is_err());

        let mut reordered_tail = argv.clone();
        reordered_tail.swap(argv.len() - 4, argv.len() - 3);
        assert!(validate_managed_wrapper_effective_rustc_argv(&reordered_tail).is_err());

        let mut duplicate_backend = argv.clone();
        duplicate_backend.insert(1, OsString::from("-Zcodegen-backend=/tmp/earlier.so"));
        assert!(validate_managed_wrapper_effective_rustc_argv(&duplicate_backend).is_err());

        let mut response_file = argv;
        response_file.insert(1, OsString::from("@attacker.rsp"));
        assert!(validate_managed_wrapper_effective_rustc_argv(&response_file).is_err());
    }

    #[test]
    fn managed_authority_requires_the_attempt_to_name_the_observed_argv() {
        let admitted = require_compiler_semantics(&reviewed_compiler_semantics("0123456789abcdef"))
            .expect("reviewed compiler semantics");
        let metadata = admitted.cargo_metadata_build_observation;
        let mut transcript = Sha256::new();
        transcript.update(CARGO_METADATA_BUILD_OBSERVATION_DOMAIN_V2);
        transcript.update((metadata.ordered_tokens.len() as u64).to_le_bytes());
        for token in &metadata.ordered_tokens {
            transcript.update((token.len() as u64).to_le_bytes());
            transcript.update(token.as_bytes());
        }
        let transcript: [u8; 32] = transcript.finalize().into();
        let attempt = fe2o3_artifact_transaction::BuildAttempt::from_env_value(&format!(
            "1:{}:{}",
            "11".repeat(16),
            "44".repeat(32)
        ))
        .expect("canonical test attempt");

        assert!(
            admit_managed_build_authority(
                attempt,
                &metadata,
                transcript,
                ManagedCompilerClosureAuthorityV1::UnprotectedQualificationV1([0x77; 32]),
                [0x44; 32],
                [0x66; 32],
            )
            .is_ok()
        );
        let metadata_mismatch = admit_managed_build_authority(
            attempt,
            &metadata,
            [0x99; 32],
            ManagedCompilerClosureAuthorityV1::UnprotectedQualificationV1([0x77; 32]),
            [0x44; 32],
            [0x66; 32],
        )
        .expect_err("substituted metadata transcript must fail");
        assert!(matches!(
            metadata_mismatch,
            CollectedRowSoftmaxErrorV1::CompilerSemantics { detail }
                if detail.contains("Cargo metadata transcript does not match")
        ));
        let mismatch = admit_managed_build_authority(
            attempt,
            &metadata,
            transcript,
            ManagedCompilerClosureAuthorityV1::UnprotectedQualificationV1([0x77; 32]),
            [0x55; 32],
            [0x66; 32],
        )
        .expect_err("substituted observed argv must fail");
        assert!(matches!(
            mismatch,
            CollectedRowSoftmaxErrorV1::CompilerSemantics { detail }
                if detail.contains("effective rustc argv does not match build attempt invocation")
        ));
    }

    #[cfg(feature = "row-softmax-metadata-mutation-test-only")]
    #[test]
    fn metadata_mutation_is_deferred_until_managed_authority_admission() {
        let genuine = [0x5a; 32];
        assert_eq!(
            apply_metadata_mutation_test_only(genuine, None),
            Ok(genuine)
        );
        assert_eq!(
            apply_metadata_mutation_test_only(genuine, Some(OsStr::new("substitute"))),
            Ok([0x01; 32])
        );
        assert_eq!(
            apply_metadata_mutation_test_only(genuine, Some(OsStr::new("omit"))),
            Err(format!(
                "managed wrapper omitted {CARGO_METADATA_BUILD_OBSERVATION_ENV_V2}"
            ))
        );
    }

    #[test]
    fn compiler_closure_environment_is_canonical_nonzero_and_exact() {
        let name = EXPECTED_COMPILER_CLOSURE_SHA256_ENV_V1;
        let canonical = "01".repeat(32);
        assert_eq!(
            decode_required_nonzero_lower_sha256(name, Some(&canonical)).unwrap(),
            [1; 32]
        );
        for malformed in [
            None,
            Some("00".repeat(32)),
            Some("01".repeat(31)),
            Some(format!("{}00", "01".repeat(32))),
            Some("AA".repeat(32)),
            Some(format!("{}\n", "01".repeat(32))),
        ] {
            assert!(
                decode_required_nonzero_lower_sha256(name, malformed.as_deref()).is_err(),
                "accepted {malformed:?}"
            );
        }
    }

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
    fn exact_source_profile_is_closed_without_executable_authority() {
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
        assert_ne!(authenticated.authority_commitment(), &[0; 32]);
        assert_eq!(
            authenticated.exponential_boundary_commitment(),
            &exponential_boundary_commitment()
        );
        assert!(!authenticated.authority_transcript().is_empty());
        let authority_commitment = *authenticated.authority_commitment();
        let (module, descriptor_source, authority_transcript) = authenticated.into_parts();
        assert_eq!(module, canonical_row_softmax_v1_module());
        assert_eq!(descriptor_source.table().kernels().len(), 1);
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(authority_transcript)),
            authority_commitment
        );
        assert!(matches!(
            receipt.consume(),
            Err(CollectedRowSoftmaxErrorV1::ReceiptAlreadyConsumed)
        ));
    }

    #[test]
    fn protected_authority_transcript_binds_full_v2_closure_without_changing_v1() {
        let legacy = exact_frontend_receipt_for_test();
        let legacy_transcript = legacy.authority_transcript.as_ref().unwrap();
        let legacy_fields = decode_transcript_fields_for_test(legacy_transcript);
        assert_eq!(legacy_fields[0], COLLECTED_AUTHORITY_DOMAIN_V1);
        assert_eq!(
            legacy_fields[legacy_fields.len() - 2],
            exact_compiler_closure_policy_for_test()
                .identity_sha256()
                .as_slice()
        );
        assert_eq!(
            crate::encode_hex(&Sha256::digest(legacy_transcript)),
            "c26eb35a626d5a9f2bd325e30f7ee8a10c7f0a1caae7430569fb2ae20b8a1c7b",
            "unprotected qualification must preserve the V1 transcript wire image",
        );

        let closure = CompilerClosureV2::new(
            [0x51; 32], [0x52; 32], [0x53; 32], [0x54; 32], [0x55; 32], [0x56; 32],
        )
        .unwrap();
        let mut protected = exact_frontend_receipt_for_test();
        let protected_transcript = {
            let authority = protected.authority.as_mut().unwrap();
            authority.managed_build_authority.compiler_closure =
                ManagedCompilerClosureAuthorityV1::ProtectedV2(closure);
            let transcript = collected_authority_transcript(authority);
            authority.authority_commitment = Sha256::digest(&transcript).into();
            transcript
        };
        protected.authority_transcript = Some(protected_transcript.clone());
        protected
            .consume()
            .expect("protected transcript is complete");

        let fields = decode_transcript_fields_for_test(&protected_transcript);
        assert_eq!(fields[0], COLLECTED_AUTHORITY_DOMAIN_V2);
        assert_eq!(fields.len(), legacy_fields.len() + 1);
        let canonical_preimage = compiler_closure_v2_canonical_preimage(closure);
        assert_eq!(
            fields[fields.len() - 3],
            canonical_preimage,
            "protected authority must bind the exact canonical V2 identity preimage",
        );
        assert_eq!(fields[fields.len() - 2], closure.identity_sha256());
    }

    fn decode_transcript_fields_for_test(transcript: &[u8]) -> Vec<&[u8]> {
        let mut fields = Vec::new();
        let mut remaining = transcript;
        while !remaining.is_empty() {
            let (length, body) = remaining.split_at(8);
            let length = usize::try_from(u64::from_le_bytes(length.try_into().unwrap())).unwrap();
            let (field, tail) = body.split_at(length);
            fields.push(field);
            remaining = tail;
        }
        fields
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
        let authority_transcript = {
            let authority = alternate_receipt
                .authority
                .as_mut()
                .expect("test authority");
            authority.root_instance_identity = alternate.to_owned();
            authority.authority_commitment = collected_authority_commitment(authority);
            assert_ne!(authority.authority_commitment, baseline_commitment);
            assert!(validate_frontend_authority(authority).is_ok());
            collected_authority_transcript(authority)
        };
        alternate_receipt.authority_transcript = Some(authority_transcript);
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
        let mutations: [ReceiptMutation; 24] = [
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
                |value| value.descriptor_source_commitment[0] ^= 1,
                "descriptor source",
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
            (
                |value| value.managed_build_authority.invocation = [0; 32],
                "managed wrapper build attempt",
            ),
            (
                |value| value.managed_build_authority.cargo_metadata_transcript[0] ^= 1,
                "wrapper Cargo metadata transcript",
            ),
            (
                |value| {
                    value.managed_build_authority.compiler_closure =
                        ManagedCompilerClosureAuthorityV1::UnprotectedQualificationV1([0; 32])
                },
                "managed wrapper build attempt",
            ),
            (
                |value| value.managed_build_authority.broker_executable = [0; 32],
                "brokered managed-wrapper invocation authority",
            ),
        ];
        for (mutate, expected_field) in mutations {
            let mut receipt = exact_frontend_receipt_for_test();
            let authority_transcript = {
                let authority = receipt.authority.as_mut().expect("test authority");
                mutate(authority);
                assert_ne!(baseline, collected_authority_commitment(authority));
                authority.authority_commitment = collected_authority_commitment(authority);
                collected_authority_transcript(authority)
            };
            receipt.authority_transcript = Some(authority_transcript);
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
    fn authority_transcript_bytes_are_commitment_bound() {
        let mut receipt = exact_frontend_receipt_for_test();
        receipt
            .authority_transcript
            .as_mut()
            .expect("test transcript")[0] ^= 1;
        assert!(matches!(
            receipt.consume(),
            Err(CollectedRowSoftmaxErrorV1::ReceiptBindingMismatch {
                field: "authority transcript"
            })
        ));
    }

    #[test]
    fn descriptor_bytes_must_match_the_receipt_bound_identity() {
        let mut receipt = exact_frontend_receipt_for_test();
        receipt.descriptor_source =
            Some(crate::compiler_descriptor::scalar_gemm_v1_descriptor_source_for_test());
        assert!(matches!(
            receipt.consume(),
            Err(CollectedRowSoftmaxErrorV1::ReceiptBindingMismatch {
                field: "descriptor source"
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
