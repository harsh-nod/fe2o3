//! Bounded source-debug metadata for the S09 alpha O0 pilot.

use crate::AmdGpuTarget;
use crate::collector::{AuthenticatedKernelOwner, CollectionResult, TypedKernelProfile};
use crate::mir_import::MirSemanticAdmissionInputsV2;
use crate::s09_identity_v2::{
    BuildIdentityClaimFieldsV2, BuildIdentityClaimV2, DecodedIdentityHandoffV2,
    IdentityCodecErrorV2, S09_IDENTITY_SECTION_V2, SemanticIdentityClaimFieldsV2,
    SemanticIdentityClaimV2,
};
use fe2o3_artifacts::{
    AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership, BlockSize, Capability,
    Endianness, IdentityText, LaunchContract, Mutability, PointerWidth, ScalarType, TargetIdentity,
};
use fe2o3_compiler_ffi::CodeObjectVersion;
use rustc_middle::mir::{Body, VarDebugInfoContents};
use rustc_middle::ty::{FloatTy, TyCtxt, TyKind, UintTy};
use rustc_session::config::{DebugInfo, OptLevel};
use rustc_span::Span;
use sha2::{Digest, Sha256};
use std::env;
use std::error::Error;
use std::fmt::{self, Write};
use std::fs::File;
use std::io::Read;
use std::os::unix::fs::MetadataExt;

pub(crate) const SOURCE_DEBUG_PROFILE_ENV: &str = "FE2O3_WORKER_V2_SOURCE_DEBUG_PROFILE_V1";
const S09_ALPHA_PROFILE: &str = "s09-alpha-gfx942-o0-v1";
const S09_CRATE_NAME: &str = "fe2o3_typed_alias_spoof";
const S09_MODULE_PATH: &str = "general_genuine";
const S09_LOGICAL_NAME: &str = "alpha";
const S09_EXPORT_NAME: &str = "alpha";
const S09_SOURCE_PATH: &str =
    "crates/rustc-codegen-fe2o3/tests/fixtures/typed-alias-spoof/src/main.rs";
const S09_SOURCE_SHA256: [u8; 32] = [
    0x73, 0xc1, 0xff, 0x5e, 0x2f, 0x29, 0xd2, 0x45, 0xc8, 0x07, 0x1b, 0xdb, 0x6c, 0x1a, 0x38, 0xaf,
    0x1c, 0x9e, 0xe1, 0x57, 0x3b, 0x78, 0xd4, 0x7a, 0x98, 0x76, 0x33, 0x48, 0x3b, 0x37, 0xe0, 0x84,
];
const S09_PORTABLE_MIR_SHA256_V2: [u8; 32] = [
    0xe2, 0x1c, 0xe4, 0x57, 0xda, 0x60, 0xa6, 0x05, 0x25, 0xce, 0x9d, 0x80, 0xa9, 0x45, 0x48, 0x2c,
    0xb8, 0xd2, 0x1e, 0x48, 0xad, 0xff, 0x67, 0xa1, 0x6d, 0x63, 0xc1, 0xa2, 0xd1, 0x01, 0xed, 0x99,
];
const S09_SOURCE_BYTES: usize = 3359;
const S09_FUNCTION_LINE: usize = 68;
const S09_INDEX_LINE: usize = 69;
const S09_LOCAL_LINE: usize = 70;
const S09_OBSERVATION_LINE: usize = 71;
const MAX_RUSTC_EXECUTABLE_BYTES: u64 = 256 * 1024 * 1024;

const CARGO_METADATA_BUILD_OBSERVATION_ENV_V2: &str = "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2";
const CODEGEN_BACKEND_BUILD_OBSERVATION_ENV_V2: &str = "FE2O3_CODEGEN_BACKEND_BUILD_OBSERVATION_V2";
const WORKER_CONFIG_BUILD_OBSERVATION_ENV_V2: &str = "FE2O3_WORKER_CONFIG_BUILD_OBSERVATION_V2";
const WORKER_EXECUTABLE_BUILD_OBSERVATION_ENV_V2: &str =
    "FE2O3_WORKER_EXECUTABLE_BUILD_OBSERVATION_V2";
const WORKER_BUILD_IDENTITY_OBSERVATION_ENV_V2: &str = "FE2O3_WORKER_BUILD_IDENTITY_OBSERVATION_V2";
const LLVM_BUILD_IDENTITY_OBSERVATION_ENV_V2: &str = "FE2O3_LLVM_BUILD_IDENTITY_OBSERVATION_V2";
const PROCESS_CONSISTENCY_EXPECTATION_FD_V3: std::os::fd::RawFd =
    fe2o3_process_identity::S09_PROCESS_CONSISTENCY_EXPECTATION_FD_V3;
const CARGO_FE2O3_EXECUTABLE_BUILD_OBSERVATION_ENV_V2: &str =
    "FE2O3_CARGO_FE2O3_EXECUTABLE_BUILD_OBSERVATION_V2";
const DECLARED_CARGO_EXECUTABLE_BUILD_OBSERVATION_ENV_V2: &str =
    "FE2O3_DECLARED_CARGO_EXECUTABLE_BUILD_OBSERVATION_V2";
const PINNED_CARGO_IMAGE_BUILD_OBSERVATION_ENV_V2: &str =
    "FE2O3_PINNED_CARGO_IMAGE_BUILD_OBSERVATION_V2";
const OBSERVED_PARENT_PID_BUILD_OBSERVATION_ENV_V2: &str =
    "FE2O3_OBSERVED_PARENT_PID_BUILD_OBSERVATION_V2";
const OBSERVED_PARENT_START_TIME_BUILD_OBSERVATION_ENV_V2: &str =
    "FE2O3_OBSERVED_PARENT_START_TIME_BUILD_OBSERVATION_V2";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AlphaSourceDebugV2 {
    source_file: String,
    source_directory: String,
    function_line: usize,
    index_line: usize,
    local_line: usize,
    semantic_claim: SemanticIdentityClaimV2,
    build_claim: BuildIdentityClaimV2,
    identity_handoff: DecodedIdentityHandoffV2,
}

impl AlphaSourceDebugV2 {
    pub(crate) const fn semantic_claim(&self) -> &SemanticIdentityClaimV2 {
        &self.semantic_claim
    }

    pub(crate) const fn build_claim(&self) -> &BuildIdentityClaimV2 {
        &self.build_claim
    }

    pub(crate) fn identity_handoff(&self) -> &[u8] {
        self.identity_handoff.canonical_bytes()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceDebugError(String);

impl SourceDebugError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for SourceDebugError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for SourceDebugError {}

impl From<IdentityCodecErrorV2> for SourceDebugError {
    fn from(value: IdentityCodecErrorV2) -> Self {
        Self::new(format!("S09 identity V2 codec failed: {value}"))
    }
}

pub(crate) fn collect_requested_profile<'tcx>(
    tcx: TyCtxt<'tcx>,
    collection: &CollectionResult<'tcx>,
    mir_module: &crate::mir_import::MirModule,
    target: &AmdGpuTarget,
) -> Result<Option<AlphaSourceDebugV2>, SourceDebugError> {
    let requested = match env::var(SOURCE_DEBUG_PROFILE_ENV) {
        Err(env::VarError::NotPresent) => return Ok(None),
        Err(env::VarError::NotUnicode(_)) => {
            return Err(SourceDebugError::new(format!(
                "{SOURCE_DEBUG_PROFILE_ENV} is not valid UTF-8"
            )));
        }
        Ok(value) if value == S09_ALPHA_PROFILE => value,
        Ok(value) => {
            return Err(SourceDebugError::new(format!(
                "{SOURCE_DEBUG_PROFILE_ENV} must be exactly {S09_ALPHA_PROFILE:?}; found {value:?}"
            )));
        }
    };
    if target.as_str() != "gfx942:xnack-" {
        return Err(SourceDebugError::new(format!(
            "{requested} requires exact target gfx942:xnack-; found {target}"
        )));
    }
    if tcx.sess.opts.optimize != OptLevel::No || tcx.sess.opts.debuginfo != DebugInfo::Full {
        return Err(SourceDebugError::new(format!(
            "{requested} requires rustc opt-level=0 and full debug info; found {:?} and {:?}",
            tcx.sess.opts.optimize, tcx.sess.opts.debuginfo
        )));
    }
    let envelope = collection
        .compiler_ffi_observation
        .as_ref()
        .ok_or_else(|| {
            SourceDebugError::new("S09 alpha requires one compiler FFI envelope observation")
        })?;
    if envelope.code_object_version() != CodeObjectVersion::V6 {
        return Err(SourceDebugError::new(format!(
            "{requested} requires code object version 6; found {:?}",
            envelope.code_object_version()
        )));
    }

    let matches = collection
        .authenticated_kernel_owners()
        .iter()
        .filter(|owner| {
            owner.crate_name() == S09_CRATE_NAME
                && owner.module_path() == S09_MODULE_PATH
                && owner.logical_name() == S09_LOGICAL_NAME
                && owner.export_name() == S09_EXPORT_NAME
                && matches!(
                    owner.typed_profile(),
                    TypedKernelProfile::GeneralScalarSliceRustcLayoutV3 { .. }
                )
        })
        .collect::<Vec<_>>();
    let [owner] = matches.as_slice() else {
        return Err(SourceDebugError::new(format!(
            "{requested} requires exactly one sealed authenticated S09 owner; found {}",
            matches.len()
        )));
    };
    let alpha = collection
        .functions
        .iter()
        .find(|function| function.instance == owner.target())
        .ok_or_else(|| {
            SourceDebugError::new("sealed S09 owner has no matching collected kernel root")
        })?;
    let contract = alpha.general_typed_contract.as_ref().ok_or_else(|| {
        SourceDebugError::new("sealed S09 owner has no authenticated General V3 ABI contract")
    })?;
    let def_id = owner.target().def_id();
    let crate_name = tcx.crate_name(def_id.krate);
    let def_path = tcx.def_path_str(def_id);
    validate_owner_observation(owner, crate_name.as_str(), &def_path)?;
    let expected_rust_path = format!("{}::{def_path}", owner.crate_name());
    let mir_matches = mir_module
        .functions
        .iter()
        .filter(|function| function.export_name == owner.export_name())
        .collect::<Vec<_>>();
    let [mir_alpha] = mir_matches.as_slice() else {
        return Err(SourceDebugError::new(format!(
            "S09 alpha requires one imported MIR body; found {}",
            mir_matches.len()
        )));
    };
    validate_alpha_mir_body(mir_alpha, &expected_rust_path)?;
    let target_identity = s09_target_identity(target)?;
    let portable_mir_sha256 = *mir_module
        .portable_semantic_digest_v2(MirSemanticAdmissionInputsV2::new(
            owner.export_name(),
            &target_identity,
            contract.abi(),
            contract.launch(),
        ))
        .map_err(|error| {
            SourceDebugError::new(format!("S09 portable MIR identity failed: {error}"))
        })?
        .as_bytes();
    validate_portable_mir_policy_v2(portable_mir_sha256)?;
    eprintln!(
        "[rustc-codegen-fe2o3] S09 portable MIR semantic SHA-256: {}",
        hex(&portable_mir_sha256)
    );

    let body = tcx.instance_mir(owner.target().def);
    validate_alpha_arguments(body)?;
    validate_debug_schema(
        body.var_debug_info.iter().map(|variable| {
            (
                variable.name.as_str(),
                variable.argument_index.map(usize::from),
            )
        }),
        body.arg_count,
    )?;

    let function = source_location(tcx, body.span)?;
    validate_source_identity(crate_name.as_str(), &function.file, function.source_sha256)?;
    let local = body
        .var_debug_info
        .iter()
        .find(|variable| variable.name.as_str() == "i" && variable.argument_index.is_none())
        .ok_or_else(|| SourceDebugError::new("S09 alpha has no source local named `i`"))?;
    let index = body
        .var_debug_info
        .iter()
        .find(|variable| variable.name.as_str() == "index" && variable.argument_index.is_none())
        .ok_or_else(|| SourceDebugError::new("S09 alpha has no source local named `index`"))?;
    let VarDebugInfoContents::Place(place) = local.value else {
        return Err(SourceDebugError::new(
            "S09 alpha local `i` is not represented by a MIR place",
        ));
    };
    let local_decl = body.local_decls.get(place.local).ok_or_else(|| {
        SourceDebugError::new("S09 alpha local `i` references an out-of-range MIR local")
    })?;
    if !place.projection.is_empty() || !matches!(local_decl.ty.kind(), TyKind::Uint(UintTy::Usize))
    {
        return Err(SourceDebugError::new(
            "S09 alpha local `i` must be an unprojected usize place",
        ));
    }
    let local_location = source_location(tcx, local.source_info.span)?;
    let index_location = source_location(tcx, index.source_info.span)?;
    if function.file != local_location.file
        || function.file != index_location.file
        || function.source_sha256 != local_location.source_sha256
        || function.source_sha256 != index_location.source_sha256
        || function.source_bytes != local_location.source_bytes
        || function.source_bytes != index_location.source_bytes
        || function.line != S09_FUNCTION_LINE
        || index_location.line != S09_INDEX_LINE
        || local_location.line != S09_LOCAL_LINE
    {
        return Err(SourceDebugError::new(format!(
            "S09 alpha source spans changed: expected canonical line {S09_FUNCTION_LINE} with `index` at line {S09_INDEX_LINE} and `i` at line {S09_LOCAL_LINE}; found function line {}, index line {}, and local line {}",
            function.line, index_location.line, local_location.line
        )));
    }
    let (source_directory, source_file) = S09_SOURCE_PATH
        .rsplit_once('/')
        .expect("S09 source path has a directory");
    validate_metadata_string(source_directory, "source directory")?;
    validate_metadata_string(source_file, "source file")?;
    let semantic_claim =
        build_semantic_claim_v2(contract.abi(), contract.launch(), portable_mir_sha256)?;
    let rustc_mir_capture_sha256 =
        crate::mir_import_v2::capture_observation_sha256_v2(tcx, owner.target()).map_err(
            |error| SourceDebugError::new(format!("S09 rustc MIR V2 capture failed: {error}")),
        )?;
    let build_claim = build_identity_claim_v2(
        owner,
        def_path,
        rustc_mir_capture_sha256,
        *semantic_claim.identity_sha256(),
        &function.file,
        function.source_sha256,
        function.source_bytes,
    )?;
    let identity_handoff =
        DecodedIdentityHandoffV2::from_claims(semantic_claim.clone(), build_claim.clone())?;
    Ok(Some(AlphaSourceDebugV2 {
        source_file: source_file.to_owned(),
        source_directory: source_directory.to_owned(),
        function_line: function.line,
        index_line: index_location.line,
        local_line: local_location.line,
        semantic_claim,
        build_claim,
        identity_handoff,
    }))
}

fn validate_portable_mir_policy_v2(actual: [u8; 32]) -> Result<(), SourceDebugError> {
    if actual != S09_PORTABLE_MIR_SHA256_V2 {
        return Err(SourceDebugError::new(format!(
            "S09 portable MIR semantic identity changed: expected {}, found {}",
            hex(&S09_PORTABLE_MIR_SHA256_V2),
            hex(&actual)
        )));
    }
    Ok(())
}

fn validate_owner_observation(
    owner: &AuthenticatedKernelOwner<rustc_middle::ty::Instance<'_>>,
    crate_name: &str,
    def_path: &str,
) -> Result<(), SourceDebugError> {
    let expected_symbol = reserved_fe2o3_symbols::host_kernel_symbol_v1(owner.kernel_binding());
    let expected_def_path = format!("{}::{expected_symbol}", owner.module_path());
    if crate_name != owner.crate_name()
        || def_path != owner.target_def_path()
        || def_path != expected_def_path
        || owner.observed_symbol() != expected_symbol
    {
        return Err(SourceDebugError::new(
            "S09 sealed owner disagrees with its exact rustc build observation",
        ));
    }
    validate_observation_text(def_path, "observed DefPath")?;
    validate_observation_text(owner.observed_symbol(), "observed symbol")
}

fn s09_target_identity(target: &AmdGpuTarget) -> Result<TargetIdentity, SourceDebugError> {
    TargetIdentity::new(
        IdentityText::new(dialect_amdgcn::AMDGPU_TRIPLE).map_err(|error| {
            SourceDebugError::new(format!("invalid S09 target triple: {error}"))
        })?,
        IdentityText::new(target.as_str())
            .map_err(|error| SourceDebugError::new(format!("invalid S09 processor: {error}")))?,
        PointerWidth::Bits64,
        Endianness::Little,
        vec![Capability::Atomics, Capability::AmdWave],
    )
    .map_err(|error| SourceDebugError::new(format!("invalid S09 target policy: {error}")))
}

fn build_semantic_claim_v2(
    abi: &AbiLayout,
    launch: &LaunchContract,
    portable_mir_sha256: [u8; 32],
) -> Result<SemanticIdentityClaimV2, SourceDebugError> {
    Ok(SemanticIdentityClaimV2::from_fields(
        SemanticIdentityClaimFieldsV2 {
            crate_name: S09_CRATE_NAME,
            module: S09_MODULE_PATH,
            logical_name: S09_LOGICAL_NAME,
            export_name: S09_EXPORT_NAME,
            profile: "general-scalar-slice-rustc-layout-v3",
            source_path: S09_SOURCE_PATH,
            source_sha256: S09_SOURCE_SHA256,
            source_bytes: S09_SOURCE_BYTES as u64,
            target: "gfx942:xnack-",
            target_capabilities: "atomics,amd-wave",
            code_object_version: 6,
            rustc_opt_level: 0,
            rustc_debug_info: "full",
            injected_debug_policy: "dwarf-v5-full",
            abi_sha256: abi_policy_sha256_v2(abi),
            launch_sha256: launch_policy_sha256_v2(launch),
            portable_mir_sha256,
        },
    )?)
}

fn build_identity_claim_v2(
    owner: &AuthenticatedKernelOwner<rustc_middle::ty::Instance<'_>>,
    observed_def_path: String,
    rustc_mir_capture_sha256: [u8; 32],
    semantic_identity_sha256: [u8; 32],
    protected_source_path: &str,
    protected_source_sha256: [u8; 32],
    protected_source_bytes: u64,
) -> Result<BuildIdentityClaimV2, SourceDebugError> {
    let cargo_metadata_sha256 =
        required_digest_environment(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2)?;
    let codegen_backend_sha256 =
        required_digest_environment(CODEGEN_BACKEND_BUILD_OBSERVATION_ENV_V2)?;
    let worker_config_sha256 = required_digest_environment(WORKER_CONFIG_BUILD_OBSERVATION_ENV_V2)?;
    let worker_executable_sha256 =
        required_digest_environment(WORKER_EXECUTABLE_BUILD_OBSERVATION_ENV_V2)?;
    // This is an inert parent-prepared/child-observed consistency digest. It does not authenticate
    // pre-backend execution or loader history.
    let prepared_rustc_command_sha256 = prepared_rustc_command_consistency_observation(
        protected_source_path,
        protected_source_sha256,
        protected_source_bytes,
    )?;
    let cargo_fe2o3_executable_sha256 =
        required_digest_environment(CARGO_FE2O3_EXECUTABLE_BUILD_OBSERVATION_ENV_V2)?;
    let declared_cargo_executable_sha256 =
        required_digest_environment(DECLARED_CARGO_EXECUTABLE_BUILD_OBSERVATION_ENV_V2)?;
    let pinned_cargo_image_sha256 =
        required_digest_environment(PINNED_CARGO_IMAGE_BUILD_OBSERVATION_ENV_V2)?;
    let observed_parent_pid =
        required_decimal_environment(OBSERVED_PARENT_PID_BUILD_OBSERVATION_ENV_V2)?;
    let observed_parent_start_time_ticks =
        required_decimal_environment(OBSERVED_PARENT_START_TIME_BUILD_OBSERVATION_ENV_V2)?;
    let worker_build_identity =
        required_text_environment(WORKER_BUILD_IDENTITY_OBSERVATION_ENV_V2)?;
    let llvm_build_identity = required_text_environment(LLVM_BUILD_IDENTITY_OBSERVATION_ENV_V2)?;
    let worker_build_identity_sha256 = observed_text_sha256_v2(
        b"FE2O3/S09-WORKER-BUILD-IDENTITY/V2\0",
        &worker_build_identity,
    );
    let llvm_build_identity_sha256 =
        observed_text_sha256_v2(b"FE2O3/S09-LLVM-BUILD-IDENTITY/V2\0", &llvm_build_identity);
    let rustc_executable_sha256 = running_rustc_sha256()?;
    let observed_symbol = owner.observed_symbol().to_owned();
    Ok(BuildIdentityClaimV2::from_fields(
        BuildIdentityClaimFieldsV2 {
            semantic_claim_sha256: semantic_identity_sha256,
            cargo_metadata_sha256,
            crate_binding: owner.crate_binding().as_bytes(),
            kernel_binding: owner.kernel_binding().as_bytes(),
            observed_def_path: &observed_def_path,
            observed_symbol: &observed_symbol,
            rustc_mir_capture_sha256,
            prepared_rustc_command_sha256,
            rustc_executable_sha256,
            cargo_fe2o3_executable_sha256,
            declared_cargo_executable_sha256,
            pinned_cargo_image_sha256,
            observed_parent_pid,
            observed_parent_start_time_ticks,
            codegen_backend_sha256,
            worker_config_sha256,
            worker_executable_sha256,
            worker_build_identity_sha256,
            llvm_build_identity_sha256,
        },
    )?)
}

fn required_digest_environment(name: &'static str) -> Result<[u8; 32], SourceDebugError> {
    let value = required_text_environment(name)?;
    let bytes = value.as_bytes();
    if bytes.len() != 64
        || !bytes.iter().all(u8::is_ascii_hexdigit)
        || bytes.iter().any(u8::is_ascii_uppercase)
    {
        return Err(SourceDebugError::new(format!(
            "{name} must contain exactly 64 lowercase hexadecimal digits"
        )));
    }
    let mut digest = [0_u8; 32];
    for (index, pair) in bytes.chunks_exact(2).enumerate() {
        digest[index] = (hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?;
    }
    if digest == [0; 32] {
        return Err(SourceDebugError::new(format!("{name} must not be zero")));
    }
    Ok(digest)
}

fn prepared_rustc_command_consistency_observation(
    protected_source_path: &str,
    protected_source_sha256: [u8; 32],
    protected_source_bytes: u64,
) -> Result<[u8; 32], SourceDebugError> {
    let current_dir_object = fe2o3_process_identity::current_directory_object_identity_v3()
        .map_err(|error| {
            SourceDebugError::new(format!(
                "S09 cannot observe the compiler cwd object: {error}"
            ))
        })?;
    let protected_source_tree_sha256 = fe2o3_process_identity::protected_source_tree_identity_v3(
        current_dir_object,
        std::path::Path::new(protected_source_path),
        protected_source_sha256,
        protected_source_bytes,
    )
    .map_err(|error| {
        SourceDebugError::new(format!(
            "S09 cannot observe its protected source tree: {error}"
        ))
    })?;
    fe2o3_process_identity::compare_child_observation_with_parent_preparation_v3(
        PROCESS_CONSISTENCY_EXPECTATION_FD_V3,
        protected_source_tree_sha256,
    )
    .map_err(|error| {
        SourceDebugError::new(format!(
            "S09 parent-prepared/child-observed consistency comparison failed: {error}"
        ))
    })
}

fn required_decimal_environment(name: &'static str) -> Result<u64, SourceDebugError> {
    let value = required_text_environment(name)?;
    if value.starts_with('0') || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(SourceDebugError::new(format!(
            "{name} must be a nonzero canonical decimal"
        )));
    }
    value
        .parse::<u64>()
        .map_err(|_| SourceDebugError::new(format!("{name} exceeds the canonical u64 range")))
}

fn required_text_environment(name: &'static str) -> Result<String, SourceDebugError> {
    match env::var(name) {
        Ok(value) if !value.is_empty() && value.len() <= 4096 => Ok(value),
        Ok(_) => Err(SourceDebugError::new(format!(
            "{name} must contain 1 through 4096 UTF-8 bytes"
        ))),
        Err(env::VarError::NotPresent) => Err(SourceDebugError::new(format!(
            "{name} is required for the S09 build observation"
        ))),
        Err(env::VarError::NotUnicode(_)) => {
            Err(SourceDebugError::new(format!("{name} is not valid UTF-8")))
        }
    }
}

fn hex_nibble(byte: u8) -> Result<u8, SourceDebugError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(SourceDebugError::new("invalid lowercase hexadecimal digit")),
    }
}

fn observed_text_sha256_v2(domain: &[u8], value: &str) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((value.len() as u64).to_le_bytes());
    digest.update(value.as_bytes());
    digest.finalize().into()
}

fn validate_observation_text(value: &str, field: &str) -> Result<(), SourceDebugError> {
    if value.is_empty()
        || value.len() > 4096
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
    {
        return Err(SourceDebugError::new(format!(
            "S09 {field} is not a bounded canonical observation"
        )));
    }
    Ok(())
}

fn running_rustc_sha256() -> Result<[u8; 32], SourceDebugError> {
    let mut file = File::open("/proc/self/exe")
        .map_err(|error| SourceDebugError::new(format!("cannot open running rustc: {error}")))?;
    let initial = file
        .metadata()
        .map_err(|error| SourceDebugError::new(format!("cannot inspect running rustc: {error}")))?;
    if !initial.is_file() || initial.len() == 0 || initial.len() > MAX_RUSTC_EXECUTABLE_BYTES {
        return Err(SourceDebugError::new(format!(
            "running rustc has invalid bounded size {}",
            initial.len()
        )));
    }
    let mut digest = Sha256::new();
    let mut remaining = initial.len();
    let mut buffer = [0_u8; 64 * 1024];
    while remaining != 0 {
        let requested = usize::try_from(remaining.min(buffer.len() as u64))
            .expect("bounded rustc chunk fits usize");
        let read = file.read(&mut buffer[..requested]).map_err(|error| {
            SourceDebugError::new(format!("cannot hash running rustc: {error}"))
        })?;
        if read == 0 {
            return Err(SourceDebugError::new(
                "running rustc became shorter while hashing",
            ));
        }
        digest.update(&buffer[..read]);
        remaining -= read as u64;
    }
    if file
        .read(&mut buffer[..1])
        .map_err(|error| SourceDebugError::new(format!("cannot finish hashing rustc: {error}")))?
        != 0
    {
        return Err(SourceDebugError::new("running rustc grew while hashing"));
    }
    let final_metadata = file
        .metadata()
        .map_err(|error| SourceDebugError::new(format!("cannot re-inspect rustc: {error}")))?;
    if metadata_identity(&initial) != metadata_identity(&final_metadata) {
        return Err(SourceDebugError::new(
            "running rustc metadata changed while hashing",
        ));
    }
    Ok(digest.finalize().into())
}

fn metadata_identity(metadata: &std::fs::Metadata) -> (u64, u64, u64, i64, i64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
    )
}

fn abi_policy_sha256_v2(abi: &AbiLayout) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"FE2O3/S09-ABI-POLICY/V2\0");
    digest.update(abi.size().to_le_bytes());
    digest.update(abi.alignment().to_le_bytes());
    digest.update([pointer_width_tag(abi.pointer_width())]);
    digest.update((abi.fields().len() as u64).to_le_bytes());
    for field in abi.fields() {
        digest.update((field.name().as_str().len() as u64).to_le_bytes());
        digest.update(field.name().as_str().as_bytes());
        digest.update(field.offset().to_le_bytes());
        digest.update(field.size().to_le_bytes());
        digest.update(field.alignment().to_le_bytes());
        encode_abi_kind(&mut digest, field.kind());
        digest.update([mutability_tag(field.mutability())]);
        digest.update([access_tag(field.access())]);
        digest.update([address_space_tag(field.address_space())]);
        digest.update(field.type_identity().rust_type().bytes().as_bytes());
        digest.update(field.type_identity().layout().bytes().as_bytes());
        digest.update([ownership_tag(field.ownership())]);
        digest.update([alias_tag(field.alias_class())]);
    }
    digest.finalize().into()
}

fn launch_policy_sha256_v2(launch: &LaunchContract) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"FE2O3/S09-LAUNCH-POLICY/V2\0");
    digest.update([launch.rank()]);
    match launch.block_size() {
        BlockSize::Any => digest.update([0]),
        BlockSize::Exact(dimensions) => {
            digest.update([1]);
            encode_dimensions(&mut digest, dimensions);
        }
        BlockSize::AtMost(dimensions) => {
            digest.update([2]);
            encode_dimensions(&mut digest, dimensions);
        }
    }
    encode_dimensions(&mut digest, launch.max_grid());
    digest.update(launch.static_shared_memory_bytes().to_le_bytes());
    digest.update(launch.max_dynamic_shared_memory_bytes().to_le_bytes());
    digest.finalize().into()
}

fn encode_dimensions(digest: &mut Sha256, dimensions: fe2o3_artifacts::Dimensions) {
    digest.update(dimensions.x().to_le_bytes());
    digest.update(dimensions.y().to_le_bytes());
    digest.update(dimensions.z().to_le_bytes());
}

fn encode_abi_kind(digest: &mut Sha256, kind: AbiKind) {
    match kind {
        AbiKind::Scalar(scalar) => digest.update([0, scalar_tag(scalar)]),
        AbiKind::Pointer {
            pointee_size,
            pointee_alignment,
        } => {
            digest.update([1]);
            digest.update(pointee_size.to_le_bytes());
            digest.update(pointee_alignment.to_le_bytes());
        }
        AbiKind::Slice {
            element_size,
            element_alignment,
        } => {
            digest.update([2]);
            digest.update(element_size.to_le_bytes());
            digest.update(element_alignment.to_le_bytes());
        }
    }
}

fn pointer_width_tag(value: PointerWidth) -> u8 {
    match value {
        PointerWidth::Bits32 => 0,
        PointerWidth::Bits64 => 1,
    }
}

fn scalar_tag(value: ScalarType) -> u8 {
    match value {
        ScalarType::I8 => 0,
        ScalarType::U8 => 1,
        ScalarType::I16 => 2,
        ScalarType::U16 => 3,
        ScalarType::I32 => 4,
        ScalarType::U32 => 5,
        ScalarType::I64 => 6,
        ScalarType::U64 => 7,
        ScalarType::F16 => 8,
        ScalarType::F32 => 9,
        ScalarType::F64 => 10,
    }
}

fn mutability_tag(value: Mutability) -> u8 {
    match value {
        Mutability::Immutable => 0,
        Mutability::Mutable => 1,
    }
}

fn access_tag(value: Access) -> u8 {
    match value {
        Access::ByValue => 0,
        Access::ReadOnly => 1,
        Access::WriteOnly => 2,
        Access::ReadWrite => 3,
    }
}

fn address_space_tag(value: AddressSpace) -> u8 {
    match value {
        AddressSpace::Value => 0,
        AddressSpace::Global => 1,
        AddressSpace::Constant => 2,
        AddressSpace::Workgroup => 3,
        AddressSpace::Private => 4,
        AddressSpace::Generic => 5,
    }
}

fn ownership_tag(value: ArgumentOwnership) -> u8 {
    match value {
        ArgumentOwnership::ByValue => 0,
        ArgumentOwnership::SharedBorrow => 1,
        ArgumentOwnership::UniqueBorrow => 2,
        ArgumentOwnership::RawPointer => 3,
    }
}

fn alias_tag(value: AliasClass) -> u8 {
    match value {
        AliasClass::Value => 0,
        AliasClass::SharedReadOnly => 1,
        AliasClass::Exclusive => 2,
        AliasClass::SharedAtomic => 3,
        AliasClass::Unrestricted => 4,
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(DIGITS[usize::from(byte >> 4)] as char);
        encoded.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    encoded
}

fn exact_place(
    place: &crate::mir_import::MirPlaceRef,
    local: usize,
    projection: &[crate::mir_import::MirProjectionElem],
) -> bool {
    place.local == local && place.projection == projection
}

fn exact_operand(
    operand: &crate::mir_import::MirOperandRef,
    local: usize,
    projection: &[crate::mir_import::MirProjectionElem],
) -> bool {
    matches!(operand, crate::mir_import::MirOperandRef::Place(place) if exact_place(place, local, projection))
}

fn exact_assign(
    statement: &crate::mir_import::MirStatement,
    index: usize,
    destination: (usize, &[crate::mir_import::MirProjectionElem]),
    operands: &[(usize, &[crate::mir_import::MirProjectionElem])],
    rvalue: crate::mir_import::MirRvalueKind,
) -> bool {
    statement.index == index
        && statement.kind == crate::mir_import::MirStatementKind::Assign
        && statement
            .destination
            .as_ref()
            .is_some_and(|place| exact_place(place, destination.0, destination.1))
        && statement.operands.len() == operands.len()
        && statement
            .operands
            .iter()
            .zip(operands)
            .all(|(operand, (local, projection))| exact_operand(operand, *local, projection))
        && statement.rvalue == Some(rvalue)
}

fn exact_call(
    terminator: &Option<crate::mir_import::MirTerminator>,
    item: crate::trusted_device_items::TrustedDeviceItem,
    target: usize,
    destination: usize,
    operands: &[(usize, &[crate::mir_import::MirProjectionElem])],
) -> bool {
    let Some(crate::mir_import::MirTerminator {
        kind:
            crate::mir_import::MirTerminatorKind::Call {
                callee: Some(callee),
                target: Some(actual_target),
                destination: Some(actual_destination),
                operands: actual_operands,
            },
        ..
    }) = terminator
    else {
        return false;
    };
    callee.trusted_item() == Some(item)
        && *actual_target == target
        && exact_place(actual_destination, destination, &[])
        && actual_operands.len() == operands.len()
        && actual_operands
            .iter()
            .zip(operands)
            .all(|(operand, (local, projection))| exact_operand(operand, *local, projection))
}

pub(crate) fn validate_alpha_mir_body(
    function: &crate::mir_import::MirFunction,
    expected_rust_path: &str,
) -> Result<(), SourceDebugError> {
    use crate::mir_import::{
        MirBinaryOp, MirFunctionKind, MirKernelProfile, MirLocalRole, MirProjectionElem,
        MirRvalueKind, MirSwitchTarget, MirTerminatorKind, MirTypeShape, MirUnaryOp,
    };
    use crate::trusted_device_items::TrustedDeviceItem;

    if function.rust_path != expected_rust_path
        || function.kind != MirFunctionKind::KernelEntry
        || function.typed_profile != Some(MirKernelProfile::GeneralScalarSliceRustcLayoutV3)
    {
        return Err(SourceDebugError::new(
            "S09 alpha imported MIR owner or profile identity changed",
        ));
    }

    let [bb0, bb1, bb2, bb3, bb4, bb5, bb6, bb7] = function.blocks.as_slice() else {
        return Err(SourceDebugError::new(
            "S09 alpha MIR must contain the exact eight-block CFG",
        ));
    };
    let expected_locals = [
        (MirLocalRole::Return, MirTypeShape::Unit),
        (MirLocalRole::Arg, MirTypeShape::F32),
        (
            MirLocalRole::Arg,
            MirTypeShape::Slice {
                element: Box::new(MirTypeShape::F32),
                mutable: false,
            },
        ),
        (
            MirLocalRole::Arg,
            MirTypeShape::DisjointSlice {
                element: Box::new(MirTypeShape::F32),
            },
        ),
        (
            MirLocalRole::Temp,
            MirTypeShape::Adt {
                identity: TrustedDeviceItem::ThreadIndex.canonical_path().to_owned(),
            },
        ),
        (MirLocalRole::Temp, MirTypeShape::USize),
        (
            MirLocalRole::Temp,
            MirTypeShape::Reference {
                pointee: Box::new(MirTypeShape::Adt {
                    identity: TrustedDeviceItem::ThreadIndex.canonical_path().to_owned(),
                }),
                mutable: false,
            },
        ),
        (
            MirLocalRole::Temp,
            MirTypeShape::Adt {
                identity: "std::option::Option".to_owned(),
            },
        ),
        (
            MirLocalRole::Temp,
            MirTypeShape::Reference {
                pointee: Box::new(MirTypeShape::DisjointSlice {
                    element: Box::new(MirTypeShape::F32),
                }),
                mutable: true,
            },
        ),
        (MirLocalRole::Temp, MirTypeShape::ISize),
        (
            MirLocalRole::Temp,
            MirTypeShape::Reference {
                pointee: Box::new(MirTypeShape::F32),
                mutable: true,
            },
        ),
        (MirLocalRole::Temp, MirTypeShape::F32),
        (MirLocalRole::Temp, MirTypeShape::USize),
        (MirLocalRole::Temp, MirTypeShape::Bool),
    ];
    if function.local_count != expected_locals.len()
        || function.locals.len() != expected_locals.len()
        || function.arg_count != 3
        || function
            .locals
            .iter()
            .zip(&expected_locals)
            .enumerate()
            .any(|(index, (local, (role, shape)))| {
                local.index != index || local.role != *role || local.ty.shape != *shape
            })
        || function
            .blocks
            .iter()
            .enumerate()
            .any(|(index, block)| block.index != index)
    {
        return Err(SourceDebugError::new(
            "S09 alpha MIR local, argument, or block identities changed",
        ));
    }

    let exact = bb0.statements.is_empty()
        && exact_call(&bb0.terminator, TrustedDeviceItem::ThreadIndex1d, 1, 4, &[])
        && matches!(bb1.statements.as_slice(), [statement] if exact_assign(statement, 0, (6, &[]), &[(4, &[])], MirRvalueKind::Ref))
        && exact_call(
            &bb1.terminator,
            TrustedDeviceItem::ThreadIndexGet,
            2,
            5,
            &[(6, &[])],
        )
        && matches!(bb2.statements.as_slice(), [statement] if exact_assign(statement, 0, (8, &[]), &[(3, &[])], MirRvalueKind::Ref))
        && exact_call(
            &bb2.terminator,
            TrustedDeviceItem::DisjointSliceGetMut,
            3,
            7,
            &[(8, &[]), (4, &[])],
        )
        && matches!(bb3.statements.as_slice(), [statement] if exact_assign(statement, 0, (9, &[]), &[(7, &[])], MirRvalueKind::Discriminant))
        && matches!(
            bb3.terminator.as_ref().map(|terminator| &terminator.kind),
            Some(MirTerminatorKind::SwitchInt {
                discriminant,
                targets,
                otherwise: 7,
            }) if exact_operand(discriminant, 9, &[])
                && targets == &[
                    MirSwitchTarget { value: 1, target: 4 },
                    MirSwitchTarget { value: 0, target: 6 },
                ]
        )
        && matches!(
            bb4.statements.as_slice(),
            [payload, length, guard]
                if exact_assign(
                    payload,
                    0,
                    (10, &[]),
                    &[(7, &[MirProjectionElem::Downcast { variant: 1 }, MirProjectionElem::Field(0)])],
                    MirRvalueKind::Use,
                ) && exact_assign(
                    length,
                    1,
                    (12, &[]),
                    &[(2, &[])],
                    MirRvalueKind::Unary(MirUnaryOp::PtrMetadata),
                ) && exact_assign(
                    guard,
                    2,
                    (13, &[]),
                    &[(5, &[]), (12, &[])],
                    MirRvalueKind::Binary(MirBinaryOp::Lt),
                )
        )
        && matches!(
            bb4.terminator.as_ref().map(|terminator| &terminator.kind),
            Some(MirTerminatorKind::Assert {
                condition,
                expected: true,
                target: 5,
            }) if exact_operand(condition, 13, &[])
        )
        && matches!(
            bb5.statements.as_slice(),
            [load, store]
                if exact_assign(
                    load,
                    0,
                    (11, &[]),
                    &[(2, &[MirProjectionElem::Deref, MirProjectionElem::Index { local: 5 }])],
                    MirRvalueKind::Use,
                ) && exact_assign(
                    store,
                    1,
                    (10, &[MirProjectionElem::Deref]),
                    &[(11, &[]), (1, &[])],
                    MirRvalueKind::Binary(MirBinaryOp::Mul),
                )
        )
        && matches!(
            bb5.terminator.as_ref().map(|terminator| &terminator.kind),
            Some(MirTerminatorKind::Goto { target: 6 })
        )
        && bb6.statements.is_empty()
        && matches!(
            bb6.terminator.as_ref().map(|terminator| &terminator.kind),
            Some(MirTerminatorKind::Return)
        )
        && bb7.statements.is_empty()
        && matches!(
            bb7.terminator.as_ref().map(|terminator| &terminator.kind),
            Some(MirTerminatorKind::Unreachable)
        );
    if !exact {
        return Err(SourceDebugError::new(
            "S09 alpha MIR is not the exact guarded alpha CFG and dataflow",
        ));
    }
    Ok(())
}

fn validate_alpha_arguments(body: &Body<'_>) -> Result<(), SourceDebugError> {
    let arguments = body
        .args_iter()
        .map(|local| body.local_decls[local].ty)
        .collect::<Vec<_>>();
    let [scale, input, output] = arguments.as_slice() else {
        return Err(SourceDebugError::new(
            "S09 alpha requires exactly scale, input, and output arguments",
        ));
    };
    if !matches!(scale.kind(), TyKind::Float(FloatTy::F32))
        || !matches!(input.kind(), TyKind::Ref(_, pointee, _) if matches!(pointee.kind(), TyKind::Slice(element) if matches!(element.kind(), TyKind::Float(FloatTy::F32))))
        || !matches!(output.kind(), TyKind::Adt(_, _))
    {
        return Err(SourceDebugError::new(
            "S09 alpha argument layout is not the exact f32/read-only-f32-slice/DisjointSlice profile",
        ));
    }
    Ok(())
}

fn validate_debug_schema<'a>(
    records: impl IntoIterator<Item = (&'a str, Option<usize>)>,
    argument_count: usize,
) -> Result<(), SourceDebugError> {
    const EXPECTED_ARGUMENTS: [&str; 3] = ["scale", "input", "output"];
    const EXPECTED_LOCALS: [&str; 2] = ["i", "index"];

    if argument_count != EXPECTED_ARGUMENTS.len() {
        return Err(SourceDebugError::new(format!(
            "S09 alpha requires exactly three argument debug records; MIR declares {argument_count} arguments"
        )));
    }

    let mut argument_names = [None; EXPECTED_ARGUMENTS.len()];
    let mut local_counts = [0_usize; EXPECTED_LOCALS.len()];
    for (name, argument_index) in records {
        if let Some(argument_index) = argument_index {
            if argument_index == 0 || argument_index > argument_count {
                return Err(SourceDebugError::new(format!(
                    "S09 alpha debug record {name:?} has invalid one-based argument index {argument_index}"
                )));
            }
            let slot = argument_index - 1;
            if argument_names[slot].replace(name).is_some() {
                return Err(SourceDebugError::new(format!(
                    "S09 alpha has duplicate debug records for argument {argument_index}"
                )));
            }
            continue;
        }

        if let Some(slot) = EXPECTED_LOCALS
            .iter()
            .position(|expected| name == *expected)
        {
            local_counts[slot] = local_counts[slot].checked_add(1).ok_or_else(|| {
                SourceDebugError::new("S09 alpha local debug record count overflow")
            })?;
            if local_counts[slot] > 1 {
                return Err(SourceDebugError::new(format!(
                    "S09 alpha has duplicate source local debug records named {name:?}"
                )));
            }
        }
    }

    let expected_argument_names = EXPECTED_ARGUMENTS.map(Some);
    if argument_names != expected_argument_names {
        return Err(SourceDebugError::new(format!(
            "S09 alpha argument debug names changed: expected {expected_argument_names:?}; found {argument_names:?}"
        )));
    }
    if local_counts != [1, 1] {
        return Err(SourceDebugError::new(format!(
            "S09 alpha requires exactly one debug record for each source local {EXPECTED_LOCALS:?}; found counts {local_counts:?}"
        )));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceLocation {
    file: String,
    line: usize,
    source_sha256: [u8; 32],
    source_bytes: u64,
}

fn source_location(tcx: TyCtxt<'_>, span: Span) -> Result<SourceLocation, SourceDebugError> {
    let location = tcx.sess.source_map().lookup_char_pos(span.lo());
    let file = location
        .file
        .name
        .prefer_remapped_unconditionally()
        .to_string_lossy()
        .into_owned();
    if file.is_empty() || location.line == 0 {
        return Err(SourceDebugError::new(
            "S09 alpha source span has no file or line",
        ));
    }
    let source = location.file.src.as_ref().ok_or_else(|| {
        SourceDebugError::new("S09 alpha source text is unavailable for identity binding")
    })?;
    let source_sha256 = bounded_source_sha256(source.as_bytes())?;
    Ok(SourceLocation {
        file,
        line: location.line,
        source_sha256,
        source_bytes: source.len() as u64,
    })
}

fn bounded_source_sha256(source: &[u8]) -> Result<[u8; 32], SourceDebugError> {
    if source.len() != S09_SOURCE_BYTES {
        return Err(SourceDebugError::new(format!(
            "S09 alpha source must contain exactly {S09_SOURCE_BYTES} bytes; found {}",
            source.len()
        )));
    }
    Ok(Sha256::digest(source).into())
}

fn validate_source_identity(
    crate_name: &str,
    source_path: &str,
    source_sha256: [u8; 32],
) -> Result<(), SourceDebugError> {
    let canonical_path = !source_path.starts_with('/')
        && !source_path.contains('\\')
        && source_path
            .split('/')
            .all(|component| !component.is_empty() && component != "." && component != "..");
    if !canonical_path || source_path != S09_SOURCE_PATH {
        return Err(SourceDebugError::new(
            "S09 alpha source path is not the exact canonical remapped path",
        ));
    }
    if crate_name != S09_CRATE_NAME {
        return Err(SourceDebugError::new("S09 alpha crate identity changed"));
    }
    if source_sha256 != S09_SOURCE_SHA256 {
        return Err(SourceDebugError::new(
            "S09 alpha whole-source SHA-256 identity changed",
        ));
    }
    Ok(())
}

fn validate_metadata_string(value: &str, field: &str) -> Result<(), SourceDebugError> {
    if value.is_empty()
        || value.len() > 4096
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'_' | b'-' | b'.'))
    {
        return Err(SourceDebugError::new(format!(
            "S09 alpha {field} is not safe deterministic LLVM metadata"
        )));
    }
    Ok(())
}

pub(crate) fn inject_alpha_dwarf_v1(
    llvm: &str,
    profile: &AlphaSourceDebugV2,
) -> Result<String, SourceDebugError> {
    for forbidden in [
        "!llvm.dbg.cu",
        "!llvm.module.flags",
        "!llvm.ident",
        "@llvm.dbg.",
        "!dbg",
        "!DICompileUnit",
        "!DIFile",
        "!DISubprogram",
        "!DISubroutineType",
        "!DIBasicType",
        "!DIDerivedType",
        "!DICompositeType",
        "!DISubrange",
        "!DILocalVariable",
        "!DILocation",
        "!DIExpression",
        S09_IDENTITY_SECTION_V2,
    ] {
        if llvm.contains(forbidden) {
            return Err(SourceDebugError::new(format!(
                "S09 alpha refuses pre-existing debug construct {forbidden:?}"
            )));
        }
    }
    if llvm.contains(" asm ") {
        return Err(SourceDebugError::new(
            "S09 alpha refuses pre-existing inline assembly",
        ));
    }
    let signature = "define amdgpu_kernel void @alpha(float %arg0, ptr addrspace(1) %arg1.data, i64 %arg1.len, ptr addrspace(1) %arg2.data, i64 %arg2.len)";
    if llvm.matches(signature).count() != 1 {
        return Err(SourceDebugError::new(
            "S09 alpha LLVM signature is absent or ambiguous",
        ));
    }
    let function_start = llvm.find(signature).expect("count checked");
    let function_end = llvm[function_start..]
        .find("\n}\n")
        .map(|offset| function_start + offset + 3)
        .ok_or_else(|| SourceDebugError::new("S09 alpha LLVM definition is unterminated"))?;
    let function = &llvm[function_start..function_end];
    if function.matches("bb0:\n").count() != 1 {
        return Err(SourceDebugError::new(
            "S09 alpha requires exactly one entry block label",
        ));
    }
    let local_value = find_global_index_value(function)?;
    let first_metadata = next_metadata_id(llvm)?;
    let ids = DebugIds::new(first_metadata)?;

    let debug_attachment = format!(" !dbg !{} !reqd_work_group_size ", ids.subprogram);
    let mut rewritten_function = replace_exactly_once(
        function,
        " !reqd_work_group_size ",
        &debug_attachment,
        "function metadata attachment",
    )?;
    if rewritten_function.matches(&debug_attachment).count() != 1 {
        return Err(SourceDebugError::new(
            "S09 alpha function debug attachment was not inserted exactly once",
        ));
    }
    let argument_records = format!("bb0:\n{}", argument_debug_records(ids));
    rewritten_function = replace_exactly_once(
        &rewritten_function,
        "bb0:\n",
        &argument_records,
        "entry block debug records",
    )?;
    let local_definition =
        format!("  {local_value} = add i64 {local_value}.base, {local_value}.local");
    let local_record = format!(
        "{local_definition}\n  call void @llvm.dbg.value(metadata i64 {local_value}, metadata !{}, metadata !DIExpression()), !dbg !{}\n  call void @llvm.dbg.value(metadata i64 {local_value}, metadata !{}, metadata !DIExpression(DW_OP_LLVM_fragment, 0, 64)), !dbg !{}\n  call void @llvm.dbg.value(metadata float %arg0, metadata !{}, metadata !DIExpression(DW_OP_LLVM_fragment, 64, 32)), !dbg !{}\n  call void @llvm.dbg.value(metadata float %arg0, metadata !{}, metadata !DIExpression(DW_OP_LLVM_fragment, 0, 32)), !dbg !{}\n  call void @llvm.dbg.value(metadata float %arg0, metadata !{}, metadata !DIExpression(DW_OP_LLVM_fragment, 32, 32)), !dbg !{}\n  call void asm sideeffect \"s_nop 0\", \"v,~{{memory}}\"(i64 {local_value}), !dbg !{}\n  call void asm sideeffect \"s_nop 0\", \"v,v,~{{memory}}\"(i64 {local_value}, float %arg0), !dbg !{}",
        ids.local,
        ids.local_location,
        ids.index_scale_tuple,
        ids.observation_location,
        ids.index_scale_tuple,
        ids.observation_location,
        ids.scale_pair,
        ids.observation_location,
        ids.scale_pair,
        ids.observation_location,
        ids.local_location,
        ids.observation_location,
    );
    rewritten_function = replace_exactly_once(
        &rewritten_function,
        &local_definition,
        &local_record,
        "global-index local binding",
    )?;

    let mut output = String::with_capacity(llvm.len() + 4096);
    let declaration_point = llvm
        .find("\n\n")
        .map(|index| index + 2)
        .ok_or_else(|| SourceDebugError::new("LLVM module has no declaration boundary"))?;
    output.push_str(&llvm[..declaration_point]);
    output.push_str("declare void @llvm.dbg.value(metadata, metadata, metadata)\n\n");
    output.push_str(&llvm[declaration_point..function_start]);
    output.push_str(&rewritten_function);
    output.push_str(&llvm[function_end..]);
    write_debug_metadata(&mut output, profile, ids)?;
    append_identity_module_assembly(&mut output, profile.identity_handoff());
    Ok(output)
}

fn append_identity_module_assembly(llvm: &mut String, bytes: &[u8]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    llvm.push_str("\nmodule asm \".section ");
    llvm.push_str(S09_IDENTITY_SECTION_V2);
    llvm.push_str(",\\22\\22,@progbits\"\nmodule asm \".balign 8\"\n");
    for chunk in bytes.chunks(16) {
        llvm.push_str("module asm \".byte ");
        for (index, byte) in chunk.iter().copied().enumerate() {
            if index != 0 {
                llvm.push_str(", ");
            }
            llvm.push_str("0x");
            llvm.push(HEX[usize::from(byte >> 4)] as char);
            llvm.push(HEX[usize::from(byte & 0x0f)] as char);
        }
        llvm.push_str("\"\n");
    }
}

fn replace_exactly_once(
    input: &str,
    needle: &str,
    replacement: &str,
    subject: &str,
) -> Result<String, SourceDebugError> {
    let count = input.matches(needle).count();
    if count != 1 {
        return Err(SourceDebugError::new(format!(
            "S09 alpha requires exactly one {subject}; found {count}"
        )));
    }
    Ok(input.replacen(needle, replacement, 1))
}

fn argument_debug_records(ids: DebugIds) -> String {
    format!(
        "  call void @llvm.dbg.value(metadata float %arg0, metadata !{}, metadata !DIExpression()), !dbg !{}\n  call void @llvm.dbg.value(metadata ptr addrspace(1) %arg1.data, metadata !{}, metadata !DIExpression()), !dbg !{}\n  call void @llvm.dbg.value(metadata i64 %arg1.len, metadata !{}, metadata !DIExpression()), !dbg !{}\n  call void @llvm.dbg.value(metadata ptr addrspace(1) %arg2.data, metadata !{}, metadata !DIExpression()), !dbg !{}\n  call void @llvm.dbg.value(metadata i64 %arg2.len, metadata !{}, metadata !DIExpression()), !dbg !{}\n  call void @llvm.dbg.value(metadata ptr addrspace(1) %arg1.data, metadata !{}, metadata !DIExpression(DW_OP_LLVM_fragment, 0, 64)), !dbg !{}\n  call void @llvm.dbg.value(metadata i64 %arg1.len, metadata !{}, metadata !DIExpression(DW_OP_LLVM_fragment, 64, 64)), !dbg !{}\n  call void @llvm.dbg.value(metadata ptr addrspace(1) %arg2.data, metadata !{}, metadata !DIExpression(DW_OP_LLVM_fragment, 0, 64)), !dbg !{}\n  call void @llvm.dbg.value(metadata i64 %arg2.len, metadata !{}, metadata !DIExpression(DW_OP_LLVM_fragment, 64, 64)), !dbg !{}\n  call void @llvm.dbg.value(metadata ptr addrspace(1) %arg1.data, metadata !{}, metadata !DIExpression()), !dbg !{}\n  call void asm sideeffect \"s_nop 0\", \"v,v,v,v,v,~{{memory}}\"(float %arg0, ptr addrspace(1) %arg1.data, i64 %arg1.len, ptr addrspace(1) %arg2.data, i64 %arg2.len), !dbg !{}\n  call void asm sideeffect \"s_nop 0\", \"v,v,v,v,v,~{{memory}}\"(float %arg0, ptr addrspace(1) %arg1.data, i64 %arg1.len, ptr addrspace(1) %arg2.data, i64 %arg2.len), !dbg !{}\n",
        ids.scale,
        ids.function_location,
        ids.input_data,
        ids.function_location,
        ids.input_len,
        ids.function_location,
        ids.output_data,
        ids.function_location,
        ids.output_len,
        ids.function_location,
        ids.input,
        ids.function_location,
        ids.input,
        ids.function_location,
        ids.output,
        ids.function_location,
        ids.output,
        ids.function_location,
        ids.input_first_ref,
        ids.function_location,
        ids.function_location,
        ids.index_location,
    )
}

fn find_global_index_value(function: &str) -> Result<&str, SourceDebugError> {
    let matches = function
        .lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let (value, rhs) = line.split_once(" = add i64 ")?;
            let id = value.strip_prefix("%v")?;
            let rhs = rhs.strip_prefix("%v")?;
            let (base, local) = rhs.split_once(".base, %v")?;
            let local = local.strip_suffix(".local")?;
            (id == base && id == local && !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
                .then_some(value)
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [value] => Ok(value),
        _ => Err(SourceDebugError::new(format!(
            "S09 alpha requires one global-index SSA value; found {}",
            matches.len()
        ))),
    }
}

fn next_metadata_id(llvm: &str) -> Result<usize, SourceDebugError> {
    let bytes = llvm.as_bytes();
    let mut max = None;
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'!' || index + 1 == bytes.len() || !bytes[index + 1].is_ascii_digit() {
            index += 1;
            continue;
        }
        let start = index + 1;
        let mut end = start;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        let value = llvm[start..end]
            .parse::<usize>()
            .map_err(|_| SourceDebugError::new("LLVM metadata identifier overflow"))?;
        max = Some(max.map_or(value, |previous: usize| previous.max(value)));
        index = end;
    }
    max.unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| SourceDebugError::new("LLVM metadata identifier space exhausted"))
}

#[derive(Clone, Copy)]
struct DebugIds {
    compile_unit: usize,
    file: usize,
    dwarf_flag: usize,
    debug_flag: usize,
    ident: usize,
    subprogram: usize,
    subroutine_type: usize,
    subroutine_types: usize,
    f32_type: usize,
    pointer_type: usize,
    usize_type: usize,
    retained: usize,
    scale: usize,
    input_data: usize,
    input_len: usize,
    output_data: usize,
    output_len: usize,
    local: usize,
    input: usize,
    output: usize,
    input_first_ref: usize,
    index_scale_tuple: usize,
    scale_pair: usize,
    slice_type: usize,
    slice_elements: usize,
    slice_data_member: usize,
    slice_len_member: usize,
    output_type: usize,
    output_elements: usize,
    output_data_member: usize,
    output_len_member: usize,
    input_first_ref_type: usize,
    tuple_type: usize,
    tuple_elements: usize,
    tuple_index_member: usize,
    tuple_scale_member: usize,
    array_type: usize,
    array_elements: usize,
    array_subrange: usize,
    function_location: usize,
    index_location: usize,
    local_location: usize,
    observation_location: usize,
}

impl DebugIds {
    fn new(first: usize) -> Result<Self, SourceDebugError> {
        let id = |offset| {
            first
                .checked_add(offset)
                .ok_or_else(|| SourceDebugError::new("LLVM metadata identifier space exhausted"))
        };
        Ok(Self {
            compile_unit: id(0)?,
            file: id(1)?,
            dwarf_flag: id(2)?,
            debug_flag: id(3)?,
            ident: id(4)?,
            subprogram: id(5)?,
            subroutine_type: id(6)?,
            subroutine_types: id(7)?,
            f32_type: id(8)?,
            pointer_type: id(9)?,
            usize_type: id(10)?,
            retained: id(11)?,
            scale: id(12)?,
            input_data: id(13)?,
            input_len: id(14)?,
            output_data: id(15)?,
            output_len: id(16)?,
            local: id(17)?,
            input: id(18)?,
            output: id(19)?,
            input_first_ref: id(20)?,
            index_scale_tuple: id(21)?,
            scale_pair: id(22)?,
            slice_type: id(23)?,
            slice_elements: id(24)?,
            slice_data_member: id(25)?,
            slice_len_member: id(26)?,
            output_type: id(27)?,
            output_elements: id(28)?,
            output_data_member: id(29)?,
            output_len_member: id(30)?,
            input_first_ref_type: id(31)?,
            tuple_type: id(32)?,
            tuple_elements: id(33)?,
            tuple_index_member: id(34)?,
            tuple_scale_member: id(35)?,
            array_type: id(36)?,
            array_elements: id(37)?,
            array_subrange: id(38)?,
            function_location: id(39)?,
            index_location: id(40)?,
            local_location: id(41)?,
            observation_location: id(42)?,
        })
    }
}

fn write_debug_metadata(
    output: &mut String,
    profile: &AlphaSourceDebugV2,
    ids: DebugIds,
) -> Result<(), SourceDebugError> {
    writeln!(output, "\n!llvm.dbg.cu = !{{!{}}}", ids.compile_unit).unwrap();
    writeln!(
        output,
        "!llvm.module.flags = !{{!{}, !{}}}",
        ids.dwarf_flag, ids.debug_flag
    )
    .unwrap();
    writeln!(output, "!llvm.ident = !{{!{}}}", ids.ident).unwrap();
    writeln!(output, "!{} = distinct !DICompileUnit(language: DW_LANG_Rust, file: !{}, producer: \"fe2o3 S09 alpha gfx942 O0 v1\", isOptimized: false, runtimeVersion: 0, emissionKind: FullDebug)", ids.compile_unit, ids.file).unwrap();
    writeln!(
        output,
        "!{} = !DIFile(filename: \"{}\", directory: \"{}\")",
        ids.file, profile.source_file, profile.source_directory
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !{{i32 7, !\"Dwarf Version\", i32 5}}",
        ids.dwarf_flag
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !{{i32 2, !\"Debug Info Version\", i32 3}}",
        ids.debug_flag
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !{{!\"fe2o3 S09 alpha gfx942 O0 v1\"}}",
        ids.ident
    )
    .unwrap();
    writeln!(output, "!{} = distinct !DISubprogram(name: \"alpha\", linkageName: \"alpha\", scope: !{}, file: !{}, line: {}, type: !{}, scopeLine: {}, spFlags: DISPFlagDefinition, unit: !{}, retainedNodes: !{})", ids.subprogram, ids.file, ids.file, profile.function_line, ids.subroutine_type, profile.function_line, ids.compile_unit, ids.retained).unwrap();
    writeln!(
        output,
        "!{} = !DISubroutineType(types: !{})",
        ids.subroutine_type, ids.subroutine_types
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !{{null, !{}, !{}, !{}, !{}, !{}}}",
        ids.subroutine_types,
        ids.f32_type,
        ids.pointer_type,
        ids.usize_type,
        ids.pointer_type,
        ids.usize_type
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !DIBasicType(name: \"f32\", size: 32, encoding: DW_ATE_float)",
        ids.f32_type
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !{}, size: 64)",
        ids.pointer_type, ids.f32_type
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !DIBasicType(name: \"usize\", size: 64, encoding: DW_ATE_unsigned)",
        ids.usize_type
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !DICompositeType(tag: DW_TAG_structure_type, name: \"S09SliceRefF32\", file: !{}, line: {}, size: 128, align: 64, elements: !{})",
        ids.slice_type, ids.file, profile.function_line, ids.slice_elements
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !{{!{}, !{}}}",
        ids.slice_elements, ids.slice_data_member, ids.slice_len_member
    )
    .unwrap();
    writeln!(output, "!{} = !DIDerivedType(tag: DW_TAG_member, name: \"data_ptr\", scope: !{}, file: !{}, line: {}, baseType: !{}, size: 64, align: 64, offset: 0)", ids.slice_data_member, ids.slice_type, ids.file, profile.function_line, ids.pointer_type).unwrap();
    writeln!(output, "!{} = !DIDerivedType(tag: DW_TAG_member, name: \"length\", scope: !{}, file: !{}, line: {}, baseType: !{}, size: 64, align: 64, offset: 64)", ids.slice_len_member, ids.slice_type, ids.file, profile.function_line, ids.usize_type).unwrap();
    writeln!(
        output,
        "!{} = !DICompositeType(tag: DW_TAG_structure_type, name: \"DisjointSlice<f32>\", file: !{}, line: {}, size: 128, align: 64, elements: !{})",
        ids.output_type, ids.file, profile.function_line, ids.output_elements
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !{{!{}, !{}}}",
        ids.output_elements, ids.output_data_member, ids.output_len_member
    )
    .unwrap();
    writeln!(output, "!{} = !DIDerivedType(tag: DW_TAG_member, name: \"pointer\", scope: !{}, file: !{}, line: {}, baseType: !{}, size: 64, align: 64, offset: 0)", ids.output_data_member, ids.output_type, ids.file, profile.function_line, ids.pointer_type).unwrap();
    writeln!(output, "!{} = !DIDerivedType(tag: DW_TAG_member, name: \"length\", scope: !{}, file: !{}, line: {}, baseType: !{}, size: 64, align: 64, offset: 64)", ids.output_len_member, ids.output_type, ids.file, profile.function_line, ids.usize_type).unwrap();
    writeln!(
        output,
        "!{} = !DIDerivedType(tag: DW_TAG_reference_type, baseType: !{}, size: 64, align: 64)",
        ids.input_first_ref_type, ids.f32_type
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !DICompositeType(tag: DW_TAG_structure_type, name: \"(usize, f32)\", file: !{}, line: {}, size: 128, align: 64, elements: !{})",
        ids.tuple_type, ids.file, profile.local_line, ids.tuple_elements
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !{{!{}, !{}}}",
        ids.tuple_elements, ids.tuple_index_member, ids.tuple_scale_member
    )
    .unwrap();
    writeln!(output, "!{} = !DIDerivedType(tag: DW_TAG_member, name: \"__0\", scope: !{}, file: !{}, line: {}, baseType: !{}, size: 64, align: 64, offset: 0)", ids.tuple_index_member, ids.tuple_type, ids.file, profile.local_line, ids.usize_type).unwrap();
    writeln!(output, "!{} = !DIDerivedType(tag: DW_TAG_member, name: \"__1\", scope: !{}, file: !{}, line: {}, baseType: !{}, size: 32, align: 32, offset: 64)", ids.tuple_scale_member, ids.tuple_type, ids.file, profile.local_line, ids.f32_type).unwrap();
    writeln!(
        output,
        "!{} = !DICompositeType(tag: DW_TAG_array_type, baseType: !{}, size: 64, align: 32, elements: !{})",
        ids.array_type, ids.f32_type, ids.array_elements
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !{{!{}}}",
        ids.array_elements, ids.array_subrange
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !DISubrange(count: 2, lowerBound: 0)",
        ids.array_subrange
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !{{!{}, !{}, !{}, !{}, !{}, !{}, !{}, !{}, !{}, !{}, !{}}}",
        ids.retained,
        ids.scale,
        ids.input_data,
        ids.input_len,
        ids.output_data,
        ids.output_len,
        ids.local,
        ids.input,
        ids.output,
        ids.input_first_ref,
        ids.index_scale_tuple,
        ids.scale_pair
    )
    .unwrap();
    writeln!(output, "!{} = !DILocalVariable(name: \"scale\", arg: 1, scope: !{}, file: !{}, line: {}, type: !{})", ids.scale, ids.subprogram, ids.file, profile.function_line, ids.f32_type).unwrap();
    writeln!(
        output,
        "!{} = !DILocalVariable(name: \"input_data\", scope: !{}, file: !{}, line: {}, type: !{})",
        ids.input_data, ids.subprogram, ids.file, profile.function_line, ids.pointer_type
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !DILocalVariable(name: \"input_len\", scope: !{}, file: !{}, line: {}, type: !{})",
        ids.input_len, ids.subprogram, ids.file, profile.function_line, ids.usize_type
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !DILocalVariable(name: \"output_data\", scope: !{}, file: !{}, line: {}, type: !{})",
        ids.output_data, ids.subprogram, ids.file, profile.function_line, ids.pointer_type
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !DILocalVariable(name: \"output_len\", scope: !{}, file: !{}, line: {}, type: !{})",
        ids.output_len, ids.subprogram, ids.file, profile.function_line, ids.usize_type
    )
    .unwrap();
    writeln!(output, "!{} = !DILocalVariable(name: \"input\", arg: 2, scope: !{}, file: !{}, line: {}, type: !{})", ids.input, ids.subprogram, ids.file, profile.function_line, ids.slice_type).unwrap();
    writeln!(output, "!{} = !DILocalVariable(name: \"output\", arg: 3, scope: !{}, file: !{}, line: {}, type: !{})", ids.output, ids.subprogram, ids.file, profile.function_line, ids.output_type).unwrap();
    writeln!(output, "!{} = !DILocalVariable(name: \"input_first_ref\", scope: !{}, file: !{}, line: {}, type: !{})", ids.input_first_ref, ids.subprogram, ids.file, profile.function_line, ids.input_first_ref_type).unwrap();
    writeln!(output, "!{} = !DILocalVariable(name: \"index_scale_tuple\", scope: !{}, file: !{}, line: {}, type: !{})", ids.index_scale_tuple, ids.subprogram, ids.file, profile.local_line, ids.tuple_type).unwrap();
    writeln!(
        output,
        "!{} = !DILocalVariable(name: \"scale_pair\", scope: !{}, file: !{}, line: {}, type: !{})",
        ids.scale_pair, ids.subprogram, ids.file, profile.local_line, ids.array_type
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !DILocalVariable(name: \"i\", scope: !{}, file: !{}, line: {}, type: !{})",
        ids.local, ids.subprogram, ids.file, profile.local_line, ids.usize_type
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !DILocation(line: {}, column: 5, scope: !{})",
        ids.function_location, profile.function_line, ids.subprogram
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !DILocation(line: {}, column: 13, scope: !{})",
        ids.index_location, profile.index_line, ids.subprogram
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !DILocation(line: {}, column: 13, scope: !{})",
        ids.local_location, profile.local_line, ids.subprogram
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !DILocation(line: {}, column: 9, scope: !{})",
        ids.observation_location, S09_OBSERVATION_LINE, ids.subprogram
    )
    .unwrap();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PORTABLE_DIGEST_UNIT_VECTOR_V2: [u8; 32] = [
        0x5d, 0xce, 0x95, 0xed, 0x57, 0x0b, 0x07, 0x99, 0x57, 0xb0, 0x4b, 0x56, 0x92, 0xc2, 0xab,
        0x0f, 0x89, 0x7b, 0x6c, 0x74, 0x78, 0x50, 0x52, 0x60, 0x69, 0x2d, 0x88, 0x83, 0x1a, 0xd1,
        0x4f, 0xf5,
    ];

    fn profile() -> AlphaSourceDebugV2 {
        let semantic_claim = SemanticIdentityClaimV2::from_fields(SemanticIdentityClaimFieldsV2 {
            crate_name: S09_CRATE_NAME,
            module: S09_MODULE_PATH,
            logical_name: S09_LOGICAL_NAME,
            export_name: S09_EXPORT_NAME,
            profile: "general-scalar-slice-rustc-layout-v3",
            source_path: S09_SOURCE_PATH,
            source_sha256: S09_SOURCE_SHA256,
            source_bytes: S09_SOURCE_BYTES as u64,
            target: "gfx942:xnack-",
            target_capabilities: "atomics,amd-wave",
            code_object_version: 6,
            rustc_opt_level: 0,
            rustc_debug_info: "full",
            injected_debug_policy: "dwarf-v5-full",
            abi_sha256: [0x21; 32],
            launch_sha256: [0x22; 32],
            portable_mir_sha256: [0x31; 32],
        })
        .unwrap();
        let build_claim = BuildIdentityClaimV2::from_fields(BuildIdentityClaimFieldsV2 {
            semantic_claim_sha256: *semantic_claim.identity_sha256(),
            cargo_metadata_sha256: [0x32; 32],
            crate_binding: [0x33; 32],
            kernel_binding: [0x34; 32],
            observed_def_path: "general_genuine::__fe2o3_host_kernel_v1_test",
            observed_symbol: "__fe2o3_host_kernel_v1_test",
            rustc_mir_capture_sha256: [0x35; 32],
            prepared_rustc_command_sha256: [0x36; 32],
            rustc_executable_sha256: [0x37; 32],
            cargo_fe2o3_executable_sha256: [0x38; 32],
            declared_cargo_executable_sha256: [0x39; 32],
            pinned_cargo_image_sha256: [0x3a; 32],
            observed_parent_pid: 59,
            observed_parent_start_time_ticks: 60,
            codegen_backend_sha256: [0x3d; 32],
            worker_config_sha256: [0x3e; 32],
            worker_executable_sha256: [0x3f; 32],
            worker_build_identity_sha256: [0x40; 32],
            llvm_build_identity_sha256: [0x41; 32],
        })
        .unwrap();
        let identity_handoff =
            DecodedIdentityHandoffV2::from_claims(semantic_claim.clone(), build_claim.clone())
                .unwrap();
        AlphaSourceDebugV2 {
            source_file: "main.rs".to_owned(),
            source_directory: "crates/rustc-codegen-fe2o3/tests/fixtures/typed-alias-spoof/src"
                .to_owned(),
            function_line: S09_FUNCTION_LINE,
            index_line: S09_INDEX_LINE,
            local_line: S09_LOCAL_LINE,
            semantic_claim,
            build_claim,
            identity_handoff,
        }
    }

    fn module() -> &'static str {
        r#"target triple = "amdgcn-amd-amdhsa"

define amdgpu_kernel void @alpha(float %arg0, ptr addrspace(1) %arg1.data, i64 %arg1.len, ptr addrspace(1) %arg2.data, i64 %arg2.len) #0 !reqd_work_group_size !0 {
bb0:
  %v3.local.i32 = call i32 @llvm.amdgcn.workitem.id.x()
  %v3.group.i32 = call i32 @llvm.amdgcn.workgroup.id.x()
  %v3.local = zext i32 %v3.local.i32 to i64
  %v3.group = zext i32 %v3.group.i32 to i64
  %v3.base = mul i64 %v3.group, 256
  %v3 = add i64 %v3.base, %v3.local
  ret void
}

attributes #0 = { nounwind }

!0 = !{i32 256, i32 1, i32 1}
"#
    }

    fn valid_debug_records() -> Vec<(&'static str, Option<usize>)> {
        vec![
            ("scale", Some(1)),
            ("input", Some(2)),
            ("output", Some(3)),
            ("i", None),
            ("index", None),
            ("unrelated", None),
        ]
    }

    #[test]
    fn s09_portable_policy_is_distinct_from_unit_vector_and_rejects_mutation() {
        assert_ne!(S09_PORTABLE_MIR_SHA256_V2, PORTABLE_DIGEST_UNIT_VECTOR_V2);
        validate_portable_mir_policy_v2(S09_PORTABLE_MIR_SHA256_V2).unwrap();

        let mut mutated = S09_PORTABLE_MIR_SHA256_V2;
        mutated[0] ^= 1;
        let error = validate_portable_mir_policy_v2(mutated).unwrap_err();
        assert!(error.to_string().contains("semantic identity changed"));
    }

    #[test]
    fn injects_exact_physical_and_bounded_logical_debug_views() {
        let first = inject_alpha_dwarf_v1(module(), &profile()).unwrap();
        let second = inject_alpha_dwarf_v1(module(), &profile()).unwrap();
        assert_eq!(first, second);
        for expected in [
            "!DICompileUnit(language: DW_LANG_Rust",
            "!DISubprogram(name: \"alpha\"",
            "!DILocalVariable(name: \"scale\", arg: 1",
            "!DILocalVariable(name: \"input_data\", scope:",
            "!DILocalVariable(name: \"input_len\", scope:",
            "!DILocalVariable(name: \"output_data\", scope:",
            "!DILocalVariable(name: \"output_len\", scope:",
            "!DILocalVariable(name: \"i\", scope:",
            "!DICompositeType(tag: DW_TAG_structure_type, name: \"S09SliceRefF32\"",
            "!DIDerivedType(tag: DW_TAG_reference_type",
            "!DICompositeType(tag: DW_TAG_structure_type, name: \"(usize, f32)\"",
            "!DICompositeType(tag: DW_TAG_array_type",
            "!DILocalVariable(name: \"input\", arg: 2",
            "!DILocalVariable(name: \"index_scale_tuple\", scope:",
            "!DILocalVariable(name: \"scale_pair\", scope:",
            "metadata i64 %v3",
            "line: 69",
            "line: 70",
            "line: 71",
            S09_IDENTITY_SECTION_V2,
        ] {
            assert!(first.contains(expected), "missing {expected:?}\n{first}");
        }
        assert_eq!(first.matches("@llvm.dbg.value(").count(), 16);
        assert_eq!(first.matches("DW_OP_LLVM_fragment").count(), 8);
        assert!(first.contains("asm sideeffect \"s_nop 0\", \"v,v,v,v,v,~{memory}\""));
        assert_eq!(
            first
                .matches("asm sideeffect \"s_nop 0\", \"v,v,v,v,v,~{memory}\"")
                .count(),
            2
        );
        assert!(first.contains("asm sideeffect \"s_nop 0\", \"v,v,~{memory}\""));
    }

    #[test]
    fn rejects_mutated_or_predecorated_modules() {
        let wrong_signature = module().replace("float %arg0", "double %arg0");
        assert!(
            inject_alpha_dwarf_v1(&wrong_signature, &profile())
                .unwrap_err()
                .to_string()
                .contains("signature")
        );
        let no_index = module().replace(
            "  %v3 = add i64 %v3.base, %v3.local\n",
            "  %v3 = add i64 %v3.base, 0\n",
        );
        assert!(
            inject_alpha_dwarf_v1(&no_index, &profile())
                .unwrap_err()
                .to_string()
                .contains("global-index")
        );
        assert!(
            inject_alpha_dwarf_v1(
                &module().replace(
                    "ret void",
                    "call void asm sideeffect \"\", \"\"()\n  ret void"
                ),
                &profile(),
            )
            .unwrap_err()
            .to_string()
            .contains("inline assembly")
        );

        let no_attachment_anchor = module().replace(" #0 !reqd_work_group_size !0 {", " #0 {");
        assert!(
            inject_alpha_dwarf_v1(&no_attachment_anchor, &profile())
                .unwrap_err()
                .to_string()
                .contains("function metadata attachment")
        );
        let duplicate_attachment_anchor = module().replace(
            " !reqd_work_group_size !0 {",
            " !reqd_work_group_size !0 !reqd_work_group_size !0 {",
        );
        assert!(
            inject_alpha_dwarf_v1(&duplicate_attachment_anchor, &profile())
                .unwrap_err()
                .to_string()
                .contains("found 2")
        );
    }

    #[test]
    fn rejects_every_preexisting_injected_metadata_family() {
        for forbidden in [
            "!llvm.dbg.cu = !{!9}",
            "!llvm.module.flags = !{!9}",
            "!llvm.ident = !{!9}",
            "declare void @llvm.dbg.value(metadata, metadata, metadata)",
            "!9 = !DICompileUnit()",
            "!9 = !DIFile(filename: \"main.rs\", directory: \".\")",
            "!9 = !DISubprogram()",
            "!9 = !DISubroutineType(types: !10)",
            "!9 = !DIBasicType(name: \"f32\")",
            "!9 = !DIDerivedType(tag: DW_TAG_pointer_type)",
            "!9 = !DICompositeType(tag: DW_TAG_structure_type)",
            "!9 = !DISubrange(count: 2)",
            "!9 = !DILocalVariable(name: \"i\")",
            "!9 = !DILocation(line: 1, scope: !10)",
            "!9 = !DIExpression()",
        ] {
            let decorated = format!("{}\n{forbidden}\n", module());
            let error = inject_alpha_dwarf_v1(&decorated, &profile()).unwrap_err();
            assert!(
                error.to_string().contains("pre-existing"),
                "construct {forbidden:?} was not rejected as pre-existing: {error}"
            );
        }

        let attached = module().replace(
            " #0 !reqd_work_group_size !0 {",
            " #0 !dbg !9 !reqd_work_group_size !0 {",
        );
        assert!(
            inject_alpha_dwarf_v1(&attached, &profile())
                .unwrap_err()
                .to_string()
                .contains("pre-existing")
        );
    }

    #[test]
    fn debug_schema_requires_exact_argument_and_local_records() {
        validate_debug_schema(valid_debug_records(), 3).unwrap();

        for (description, records, expected) in [
            (
                "zero argument index",
                vec![
                    ("scale", Some(0)),
                    ("input", Some(2)),
                    ("output", Some(3)),
                    ("i", None),
                    ("index", None),
                ],
                "invalid one-based argument index 0",
            ),
            (
                "out-of-range argument index",
                vec![
                    ("scale", Some(1)),
                    ("input", Some(2)),
                    ("output", Some(4)),
                    ("i", None),
                    ("index", None),
                ],
                "invalid one-based argument index 4",
            ),
            (
                "duplicate argument",
                vec![
                    ("scale", Some(1)),
                    ("scale-again", Some(1)),
                    ("input", Some(2)),
                    ("output", Some(3)),
                    ("i", None),
                    ("index", None),
                ],
                "duplicate debug records for argument 1",
            ),
            (
                "wrong argument name",
                vec![
                    ("spoof", Some(1)),
                    ("input", Some(2)),
                    ("output", Some(3)),
                    ("i", None),
                    ("index", None),
                ],
                "argument debug names changed",
            ),
            (
                "missing argument",
                vec![
                    ("scale", Some(1)),
                    ("input", Some(2)),
                    ("i", None),
                    ("index", None),
                ],
                "argument debug names changed",
            ),
            (
                "duplicate i",
                vec![
                    ("scale", Some(1)),
                    ("input", Some(2)),
                    ("output", Some(3)),
                    ("i", None),
                    ("i", None),
                    ("index", None),
                ],
                "duplicate source local",
            ),
            (
                "duplicate index",
                vec![
                    ("scale", Some(1)),
                    ("input", Some(2)),
                    ("output", Some(3)),
                    ("i", None),
                    ("index", None),
                    ("index", None),
                ],
                "duplicate source local",
            ),
            (
                "missing i",
                vec![
                    ("scale", Some(1)),
                    ("input", Some(2)),
                    ("output", Some(3)),
                    ("index", None),
                ],
                "exactly one debug record",
            ),
            (
                "missing index",
                vec![
                    ("scale", Some(1)),
                    ("input", Some(2)),
                    ("output", Some(3)),
                    ("i", None),
                ],
                "exactly one debug record",
            ),
        ] {
            let error = validate_debug_schema(records, 3).unwrap_err();
            assert!(
                error.to_string().contains(expected),
                "{description} produced unexpected error: {error}"
            );
        }

        assert!(
            validate_debug_schema(valid_debug_records(), 2)
                .unwrap_err()
                .to_string()
                .contains("exactly three")
        );
        assert!(
            validate_debug_schema(valid_debug_records(), 4)
                .unwrap_err()
                .to_string()
                .contains("exactly three")
        );
    }

    #[test]
    fn source_hash_requires_exact_canonical_byte_length() {
        let source = include_bytes!("../tests/fixtures/typed-alias-spoof/src/main.rs");
        assert_eq!(source.len(), S09_SOURCE_BYTES);
        assert_eq!(bounded_source_sha256(source).unwrap(), S09_SOURCE_SHA256);

        for size in [S09_SOURCE_BYTES - 1, S09_SOURCE_BYTES + 1] {
            let error = bounded_source_sha256(&vec![b'x'; size]).unwrap_err();
            assert!(
                error.to_string().contains("exactly 3359 bytes"),
                "unexpected boundary error for {size} bytes: {error}"
            );
        }
    }

    #[test]
    fn source_identity_rejects_spoofs_and_checkout_paths() {
        validate_source_identity(S09_CRATE_NAME, S09_SOURCE_PATH, S09_SOURCE_SHA256).unwrap();

        for (crate_name, source_path, digest) in [
            ("substitute", S09_SOURCE_PATH, S09_SOURCE_SHA256),
            (
                S09_CRATE_NAME,
                "/checkout/crates/rustc-codegen-fe2o3/tests/fixtures/typed-alias-spoof/src/main.rs",
                S09_SOURCE_SHA256,
            ),
            (
                S09_CRATE_NAME,
                "crates/rustc-codegen-fe2o3/tests/fixtures/typed-alias-spoof/src/../src/main.rs",
                S09_SOURCE_SHA256,
            ),
            (S09_CRATE_NAME, S09_SOURCE_PATH, [0; 32]),
        ] {
            assert!(
                validate_source_identity(crate_name, source_path, digest).is_err(),
                "spoofed source identity was admitted"
            );
        }
    }
}
