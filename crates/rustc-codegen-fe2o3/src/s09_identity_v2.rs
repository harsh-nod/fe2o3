//! Canonical, bounded, inert S09 identity claims.
//!
//! The embedded handoff contains one 18-field semantic claim and one 20-field
//! build claim, with exact manifest digests for both records. Decoding grants
//! no authority. HSACO decoding first delegates envelope and metadata checks
//! to `fe2o3_hsaco::inspect_and_bind_kernel_descriptors`, then checks exact
//! gfx942:xnack- claim linkage.

use sha2::{Digest, Sha256};
use std::error::Error;
use std::fmt;

pub const S09_IDENTITY_SECTION_V2: &str = ".fe2o3.s09.identity.v2";
pub const SEMANTIC_SCHEMA_V2: &str = "fe2o3-s09-semantic-identity-claim-v2";
pub const BUILD_SCHEMA_V2: &str = "fe2o3-s09-build-identity-claim-v2";
pub const MAX_HANDOFF_BYTES_V2: usize = 64 * 1024;
pub const MAX_RECORD_BYTES_V2: usize = 16 * 1024;
pub const MAX_FIELD_NAME_BYTES_V2: usize = 64;
pub const MAX_FIELD_VALUE_BYTES_V2: usize = 4096;
pub const MAX_HSACO_BYTES_V2: usize = 16 * 1024 * 1024;
pub const MAX_ELF_SECTIONS_V2: usize = 4096;
pub const MAX_ELF_STRING_TABLE_BYTES_V2: usize = 1024 * 1024;

const HANDOFF_DOMAIN_V2: &[u8] = b"FE2O3/S09-IDENTITY-HANDOFF/V2\0";
const ELF64_HEADER_BYTES: usize = 64;
const ELF64_SECTION_HEADER_BYTES: usize = 64;
const SHT_PROGBITS: u32 = 1;
const SHT_STRTAB: u32 = 3;

const SEMANTIC_CLAIM_FIELDS_V2: [&str; 18] = [
    "schema",
    "crate",
    "module",
    "logical_name",
    "export_name",
    "profile",
    "source_path",
    "source_sha256",
    "source_bytes",
    "target",
    "target_capabilities",
    "code_object_version",
    "rustc_opt_level",
    "rustc_debug_info",
    "injected_debug_policy",
    "abi_sha256",
    "launch_sha256",
    "portable_mir_sha256",
];

const BUILD_CLAIM_FIELDS_V2: [&str; 20] = [
    "schema",
    "semantic_claim_sha256",
    "cargo_metadata_sha256",
    "crate_binding",
    "kernel_binding",
    "observed_def_path",
    "observed_symbol",
    "rustc_mir_capture_sha256",
    "prepared_rustc_command_sha256",
    "rustc_executable_sha256",
    "cargo_fe2o3_executable_sha256",
    "declared_cargo_executable_sha256",
    "cargo_launcher_executable_sha256",
    "cargo_launcher_pid",
    "cargo_launcher_start_time_ticks",
    "codegen_backend_sha256",
    "worker_config_sha256",
    "worker_executable_sha256",
    "worker_build_identity_sha256",
    "llvm_build_identity_sha256",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityCodecErrorV2(String);

impl IdentityCodecErrorV2 {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for IdentityCodecErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for IdentityCodecErrorV2 {}

pub(crate) struct SemanticIdentityClaimFieldsV2<'a> {
    pub(crate) crate_name: &'a str,
    pub(crate) module: &'a str,
    pub(crate) logical_name: &'a str,
    pub(crate) export_name: &'a str,
    pub(crate) profile: &'a str,
    pub(crate) source_path: &'a str,
    pub(crate) source_sha256: [u8; 32],
    pub(crate) source_bytes: u64,
    pub(crate) target: &'a str,
    pub(crate) target_capabilities: &'a str,
    pub(crate) code_object_version: u16,
    pub(crate) rustc_opt_level: u8,
    pub(crate) rustc_debug_info: &'a str,
    pub(crate) injected_debug_policy: &'a str,
    pub(crate) abi_sha256: [u8; 32],
    pub(crate) launch_sha256: [u8; 32],
    pub(crate) portable_mir_sha256: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticIdentityClaimV2 {
    canonical_bytes: Vec<u8>,
    identity_sha256: [u8; 32],
    crate_name: String,
    module: String,
    logical_name: String,
    export_name: String,
    profile: String,
    source_path: String,
    source_sha256: [u8; 32],
    source_bytes: u64,
    target: String,
    target_capabilities: String,
    code_object_version: u16,
    rustc_opt_level: u8,
    rustc_debug_info: String,
    injected_debug_policy: String,
    abi_sha256: [u8; 32],
    launch_sha256: [u8; 32],
    portable_mir_sha256: [u8; 32],
}

impl SemanticIdentityClaimV2 {
    pub(crate) fn from_fields(
        fields: SemanticIdentityClaimFieldsV2<'_>,
    ) -> Result<Self, IdentityCodecErrorV2> {
        let encoded = encode_fields(&[
            ("schema", SEMANTIC_SCHEMA_V2.to_owned()),
            ("crate", fields.crate_name.to_owned()),
            ("module", fields.module.to_owned()),
            ("logical_name", fields.logical_name.to_owned()),
            ("export_name", fields.export_name.to_owned()),
            ("profile", fields.profile.to_owned()),
            ("source_path", fields.source_path.to_owned()),
            ("source_sha256", hex(&fields.source_sha256)),
            ("source_bytes", fields.source_bytes.to_string()),
            ("target", fields.target.to_owned()),
            ("target_capabilities", fields.target_capabilities.to_owned()),
            (
                "code_object_version",
                fields.code_object_version.to_string(),
            ),
            ("rustc_opt_level", fields.rustc_opt_level.to_string()),
            ("rustc_debug_info", fields.rustc_debug_info.to_owned()),
            (
                "injected_debug_policy",
                fields.injected_debug_policy.to_owned(),
            ),
            ("abi_sha256", hex(&fields.abi_sha256)),
            ("launch_sha256", hex(&fields.launch_sha256)),
            ("portable_mir_sha256", hex(&fields.portable_mir_sha256)),
        ])?;
        Self::decode_record(&encoded)
    }

    fn decode_record(bytes: &[u8]) -> Result<Self, IdentityCodecErrorV2> {
        let fields = decode_fields(bytes, &SEMANTIC_CLAIM_FIELDS_V2, "semantic identity claim")?;
        require_exact(fields[0], SEMANTIC_SCHEMA_V2, "semantic schema")?;
        let source_sha256 = decode_digest(fields[7], "source_sha256")?;
        let source_bytes = decode_decimal(fields[8], "source_bytes", false)?;
        let code_object_version =
            u16::try_from(decode_decimal(fields[11], "code_object_version", false)?)
                .map_err(|_| IdentityCodecErrorV2::new("code_object_version exceeds u16"))?;
        let rustc_opt_level = u8::try_from(decode_decimal(fields[12], "rustc_opt_level", true)?)
            .map_err(|_| IdentityCodecErrorV2::new("rustc_opt_level exceeds u8"))?;
        Ok(Self {
            canonical_bytes: bytes.to_vec(),
            identity_sha256: Sha256::digest(bytes).into(),
            crate_name: fields[1].to_owned(),
            module: fields[2].to_owned(),
            logical_name: fields[3].to_owned(),
            export_name: fields[4].to_owned(),
            profile: fields[5].to_owned(),
            source_path: fields[6].to_owned(),
            source_sha256,
            source_bytes,
            target: fields[9].to_owned(),
            target_capabilities: fields[10].to_owned(),
            code_object_version,
            rustc_opt_level,
            rustc_debug_info: fields[13].to_owned(),
            injected_debug_policy: fields[14].to_owned(),
            abi_sha256: decode_digest(fields[15], "abi_sha256")?,
            launch_sha256: decode_digest(fields[16], "launch_sha256")?,
            portable_mir_sha256: decode_digest(fields[17], "portable_mir_sha256")?,
        })
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn identity_sha256(&self) -> &[u8; 32] {
        &self.identity_sha256
    }

    pub fn crate_name(&self) -> &str {
        &self.crate_name
    }

    pub fn module(&self) -> &str {
        &self.module
    }

    pub fn logical_name(&self) -> &str {
        &self.logical_name
    }

    pub fn export_name(&self) -> &str {
        &self.export_name
    }

    pub fn profile(&self) -> &str {
        &self.profile
    }

    pub fn source_path(&self) -> &str {
        &self.source_path
    }

    pub const fn source_sha256(&self) -> &[u8; 32] {
        &self.source_sha256
    }

    pub const fn source_bytes(&self) -> u64 {
        self.source_bytes
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn target_capabilities(&self) -> &str {
        &self.target_capabilities
    }

    pub const fn code_object_version(&self) -> u16 {
        self.code_object_version
    }

    pub const fn rustc_opt_level(&self) -> u8 {
        self.rustc_opt_level
    }

    pub fn rustc_debug_info(&self) -> &str {
        &self.rustc_debug_info
    }

    pub fn injected_debug_policy(&self) -> &str {
        &self.injected_debug_policy
    }

    pub const fn abi_sha256(&self) -> &[u8; 32] {
        &self.abi_sha256
    }

    pub const fn launch_sha256(&self) -> &[u8; 32] {
        &self.launch_sha256
    }

    pub const fn portable_mir_sha256(&self) -> &[u8; 32] {
        &self.portable_mir_sha256
    }
}

pub(crate) struct BuildIdentityClaimFieldsV2<'a> {
    pub(crate) semantic_claim_sha256: [u8; 32],
    pub(crate) cargo_metadata_sha256: [u8; 32],
    pub(crate) crate_binding: [u8; 32],
    pub(crate) kernel_binding: [u8; 32],
    pub(crate) observed_def_path: &'a str,
    pub(crate) observed_symbol: &'a str,
    pub(crate) rustc_mir_capture_sha256: [u8; 32],
    pub(crate) prepared_rustc_command_sha256: [u8; 32],
    pub(crate) rustc_executable_sha256: [u8; 32],
    pub(crate) cargo_fe2o3_executable_sha256: [u8; 32],
    pub(crate) declared_cargo_executable_sha256: [u8; 32],
    pub(crate) cargo_launcher_executable_sha256: [u8; 32],
    pub(crate) cargo_launcher_pid: u64,
    pub(crate) cargo_launcher_start_time_ticks: u64,
    pub(crate) codegen_backend_sha256: [u8; 32],
    pub(crate) worker_config_sha256: [u8; 32],
    pub(crate) worker_executable_sha256: [u8; 32],
    pub(crate) worker_build_identity_sha256: [u8; 32],
    pub(crate) llvm_build_identity_sha256: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildIdentityClaimV2 {
    canonical_bytes: Vec<u8>,
    identity_sha256: [u8; 32],
    semantic_claim_sha256: [u8; 32],
    cargo_metadata_sha256: [u8; 32],
    crate_binding: [u8; 32],
    kernel_binding: [u8; 32],
    observed_def_path: String,
    observed_symbol: String,
    rustc_mir_capture_sha256: [u8; 32],
    prepared_rustc_command_sha256: [u8; 32],
    rustc_executable_sha256: [u8; 32],
    cargo_fe2o3_executable_sha256: [u8; 32],
    declared_cargo_executable_sha256: [u8; 32],
    cargo_launcher_executable_sha256: [u8; 32],
    cargo_launcher_pid: u64,
    cargo_launcher_start_time_ticks: u64,
    codegen_backend_sha256: [u8; 32],
    worker_config_sha256: [u8; 32],
    worker_executable_sha256: [u8; 32],
    worker_build_identity_sha256: [u8; 32],
    llvm_build_identity_sha256: [u8; 32],
}

impl BuildIdentityClaimV2 {
    pub(crate) fn from_fields(
        fields: BuildIdentityClaimFieldsV2<'_>,
    ) -> Result<Self, IdentityCodecErrorV2> {
        let encoded = encode_fields(&[
            ("schema", BUILD_SCHEMA_V2.to_owned()),
            ("semantic_claim_sha256", hex(&fields.semantic_claim_sha256)),
            ("cargo_metadata_sha256", hex(&fields.cargo_metadata_sha256)),
            ("crate_binding", hex(&fields.crate_binding)),
            ("kernel_binding", hex(&fields.kernel_binding)),
            ("observed_def_path", fields.observed_def_path.to_owned()),
            ("observed_symbol", fields.observed_symbol.to_owned()),
            (
                "rustc_mir_capture_sha256",
                hex(&fields.rustc_mir_capture_sha256),
            ),
            (
                "prepared_rustc_command_sha256",
                hex(&fields.prepared_rustc_command_sha256),
            ),
            (
                "rustc_executable_sha256",
                hex(&fields.rustc_executable_sha256),
            ),
            (
                "cargo_fe2o3_executable_sha256",
                hex(&fields.cargo_fe2o3_executable_sha256),
            ),
            (
                "declared_cargo_executable_sha256",
                hex(&fields.declared_cargo_executable_sha256),
            ),
            (
                "cargo_launcher_executable_sha256",
                hex(&fields.cargo_launcher_executable_sha256),
            ),
            ("cargo_launcher_pid", fields.cargo_launcher_pid.to_string()),
            (
                "cargo_launcher_start_time_ticks",
                fields.cargo_launcher_start_time_ticks.to_string(),
            ),
            (
                "codegen_backend_sha256",
                hex(&fields.codegen_backend_sha256),
            ),
            ("worker_config_sha256", hex(&fields.worker_config_sha256)),
            (
                "worker_executable_sha256",
                hex(&fields.worker_executable_sha256),
            ),
            (
                "worker_build_identity_sha256",
                hex(&fields.worker_build_identity_sha256),
            ),
            (
                "llvm_build_identity_sha256",
                hex(&fields.llvm_build_identity_sha256),
            ),
        ])?;
        Self::decode_record(&encoded)
    }

    fn decode_record(bytes: &[u8]) -> Result<Self, IdentityCodecErrorV2> {
        let fields = decode_fields(bytes, &BUILD_CLAIM_FIELDS_V2, "build identity claim")?;
        require_exact(fields[0], BUILD_SCHEMA_V2, "build schema")?;
        Ok(Self {
            canonical_bytes: bytes.to_vec(),
            identity_sha256: Sha256::digest(bytes).into(),
            semantic_claim_sha256: decode_digest(fields[1], "semantic_claim_sha256")?,
            cargo_metadata_sha256: decode_digest(fields[2], "cargo_metadata_sha256")?,
            crate_binding: decode_digest(fields[3], "crate_binding")?,
            kernel_binding: decode_digest(fields[4], "kernel_binding")?,
            observed_def_path: fields[5].to_owned(),
            observed_symbol: fields[6].to_owned(),
            rustc_mir_capture_sha256: decode_digest(fields[7], "rustc_mir_capture_sha256")?,
            prepared_rustc_command_sha256: decode_digest(
                fields[8],
                "prepared_rustc_command_sha256",
            )?,
            rustc_executable_sha256: decode_digest(fields[9], "rustc_executable_sha256")?,
            cargo_fe2o3_executable_sha256: decode_digest(
                fields[10],
                "cargo_fe2o3_executable_sha256",
            )?,
            declared_cargo_executable_sha256: decode_digest(
                fields[11],
                "declared_cargo_executable_sha256",
            )?,
            cargo_launcher_executable_sha256: decode_digest(
                fields[12],
                "cargo_launcher_executable_sha256",
            )?,
            cargo_launcher_pid: decode_decimal(fields[13], "cargo_launcher_pid", false)?,
            cargo_launcher_start_time_ticks: decode_decimal(
                fields[14],
                "cargo_launcher_start_time_ticks",
                false,
            )?,
            codegen_backend_sha256: decode_digest(fields[15], "codegen_backend_sha256")?,
            worker_config_sha256: decode_digest(fields[16], "worker_config_sha256")?,
            worker_executable_sha256: decode_digest(fields[17], "worker_executable_sha256")?,
            worker_build_identity_sha256: decode_digest(
                fields[18],
                "worker_build_identity_sha256",
            )?,
            llvm_build_identity_sha256: decode_digest(fields[19], "llvm_build_identity_sha256")?,
        })
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn identity_sha256(&self) -> &[u8; 32] {
        &self.identity_sha256
    }

    pub const fn semantic_claim_sha256(&self) -> &[u8; 32] {
        &self.semantic_claim_sha256
    }

    pub const fn cargo_metadata_sha256(&self) -> &[u8; 32] {
        &self.cargo_metadata_sha256
    }

    pub const fn crate_binding(&self) -> &[u8; 32] {
        &self.crate_binding
    }

    pub const fn kernel_binding(&self) -> &[u8; 32] {
        &self.kernel_binding
    }

    pub fn observed_def_path(&self) -> &str {
        &self.observed_def_path
    }

    pub fn observed_symbol(&self) -> &str {
        &self.observed_symbol
    }

    pub const fn rustc_mir_capture_sha256(&self) -> &[u8; 32] {
        &self.rustc_mir_capture_sha256
    }

    pub const fn prepared_rustc_command_sha256(&self) -> &[u8; 32] {
        &self.prepared_rustc_command_sha256
    }

    pub const fn rustc_executable_sha256(&self) -> &[u8; 32] {
        &self.rustc_executable_sha256
    }

    pub const fn cargo_fe2o3_executable_sha256(&self) -> &[u8; 32] {
        &self.cargo_fe2o3_executable_sha256
    }

    pub const fn declared_cargo_executable_sha256(&self) -> &[u8; 32] {
        &self.declared_cargo_executable_sha256
    }

    pub const fn cargo_launcher_executable_sha256(&self) -> &[u8; 32] {
        &self.cargo_launcher_executable_sha256
    }

    pub const fn cargo_launcher_pid(&self) -> u64 {
        self.cargo_launcher_pid
    }

    pub const fn cargo_launcher_start_time_ticks(&self) -> u64 {
        self.cargo_launcher_start_time_ticks
    }

    pub const fn codegen_backend_sha256(&self) -> &[u8; 32] {
        &self.codegen_backend_sha256
    }

    pub const fn worker_config_sha256(&self) -> &[u8; 32] {
        &self.worker_config_sha256
    }

    pub const fn worker_executable_sha256(&self) -> &[u8; 32] {
        &self.worker_executable_sha256
    }

    pub const fn worker_build_identity_sha256(&self) -> &[u8; 32] {
        &self.worker_build_identity_sha256
    }

    pub const fn llvm_build_identity_sha256(&self) -> &[u8; 32] {
        &self.llvm_build_identity_sha256
    }
}

/// Bounded identity claims decoded from an inert handoff.
///
/// Decoding proves canonical syntax and internal digest linkage only. It does
/// not authenticate the containing artifact or admit any claim as true.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedIdentityHandoffV2 {
    canonical_bytes: Vec<u8>,
    semantic_claim_sha256: [u8; 32],
    build_claim_sha256: [u8; 32],
    semantic_claim: SemanticIdentityClaimV2,
    build_claim: BuildIdentityClaimV2,
}

impl DecodedIdentityHandoffV2 {
    pub(crate) fn from_claims(
        semantic_claim: SemanticIdentityClaimV2,
        build_claim: BuildIdentityClaimV2,
    ) -> Result<Self, IdentityCodecErrorV2> {
        if build_claim.semantic_claim_sha256() != semantic_claim.identity_sha256() {
            return Err(IdentityCodecErrorV2::new(
                "build identity claim does not bind the semantic identity claim",
            ));
        }
        let semantic_len = u32::try_from(semantic_claim.canonical_bytes().len())
            .map_err(|_| IdentityCodecErrorV2::new("semantic claim record is too large"))?;
        let build_len = u32::try_from(build_claim.canonical_bytes().len())
            .map_err(|_| IdentityCodecErrorV2::new("build claim record is too large"))?;
        let mut bytes = Vec::with_capacity(
            HANDOFF_DOMAIN_V2.len()
                + 64
                + 8
                + semantic_claim.canonical_bytes().len()
                + build_claim.canonical_bytes().len(),
        );
        bytes.extend_from_slice(HANDOFF_DOMAIN_V2);
        bytes.extend_from_slice(semantic_claim.identity_sha256());
        bytes.extend_from_slice(build_claim.identity_sha256());
        bytes.extend_from_slice(&semantic_len.to_le_bytes());
        bytes.extend_from_slice(semantic_claim.canonical_bytes());
        bytes.extend_from_slice(&build_len.to_le_bytes());
        bytes.extend_from_slice(build_claim.canonical_bytes());
        Self::decode(&bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, IdentityCodecErrorV2> {
        if bytes.is_empty() || bytes.len() > MAX_HANDOFF_BYTES_V2 {
            return Err(IdentityCodecErrorV2::new(format!(
                "identity handoff must contain 1 through {MAX_HANDOFF_BYTES_V2} bytes"
            )));
        }
        if !bytes.starts_with(HANDOFF_DOMAIN_V2) {
            return Err(IdentityCodecErrorV2::new(
                "identity handoff has a missing or unknown domain",
            ));
        }
        let mut offset = HANDOFF_DOMAIN_V2.len();
        let semantic_claim_sha256 = take_digest(bytes, &mut offset, "semantic_claim_sha256")?;
        let build_claim_sha256 = take_digest(bytes, &mut offset, "build_claim_sha256")?;
        let semantic_bytes = take_record(bytes, &mut offset, "semantic identity claim")?;
        let build_bytes = take_record(bytes, &mut offset, "build identity claim")?;
        if offset != bytes.len() {
            return Err(IdentityCodecErrorV2::new(
                "identity handoff has trailing bytes or records",
            ));
        }
        let semantic_claim = SemanticIdentityClaimV2::decode_record(semantic_bytes)?;
        let build_claim = BuildIdentityClaimV2::decode_record(build_bytes)?;
        if semantic_claim_sha256 != *semantic_claim.identity_sha256()
            || build_claim_sha256 != *build_claim.identity_sha256()
        {
            return Err(IdentityCodecErrorV2::new(
                "identity handoff manifest does not bind the exact claim records",
            ));
        }
        if build_claim.semantic_claim_sha256() != semantic_claim.identity_sha256() {
            return Err(IdentityCodecErrorV2::new(
                "build identity claim does not bind the semantic identity claim",
            ));
        }
        Ok(Self {
            canonical_bytes: bytes.to_vec(),
            semantic_claim_sha256,
            build_claim_sha256,
            semantic_claim,
            build_claim,
        })
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn semantic_claim_sha256(&self) -> &[u8; 32] {
        &self.semantic_claim_sha256
    }

    pub const fn build_claim_sha256(&self) -> &[u8; 32] {
        &self.build_claim_sha256
    }

    pub const fn semantic_claim(&self) -> &SemanticIdentityClaimV2 {
        &self.semantic_claim
    }

    pub const fn build_claim(&self) -> &BuildIdentityClaimV2 {
        &self.build_claim
    }
}

pub fn identity_section_v2(hsaco: &[u8]) -> Result<&[u8], IdentityCodecErrorV2> {
    if hsaco.is_empty() || hsaco.len() > MAX_HSACO_BYTES_V2 {
        return Err(IdentityCodecErrorV2::new(format!(
            "HSACO must contain 1 through {MAX_HSACO_BYTES_V2} bytes"
        )));
    }
    fe2o3_hsaco::inspect_and_bind_kernel_descriptors(hsaco).map_err(|error| {
        IdentityCodecErrorV2::new(format!("authoritative HSACO inspection failed: {error}"))
    })?;
    identity_section_after_hsaco_inspection_v2(hsaco)
}

fn identity_section_after_hsaco_inspection_v2(hsaco: &[u8]) -> Result<&[u8], IdentityCodecErrorV2> {
    if hsaco.len() < ELF64_HEADER_BYTES
        || &hsaco[..4] != b"\x7fELF"
        || hsaco[4] != 2
        || hsaco[5] != 1
        || hsaco[6] != 1
    {
        return Err(IdentityCodecErrorV2::new(
            "HSACO is not a supported ELF64 little-endian object",
        ));
    }
    let section_offset = usize_from_u64(read_u64(hsaco, 40, "ELF section table offset")?)?;
    let section_entry_size = usize::from(read_u16(hsaco, 58, "ELF section entry size")?);
    let section_count = usize::from(read_u16(hsaco, 60, "ELF section count")?);
    let string_index = usize::from(read_u16(hsaco, 62, "ELF string-table index")?);
    if section_entry_size != ELF64_SECTION_HEADER_BYTES {
        return Err(IdentityCodecErrorV2::new(
            "ELF section entry size is not canonical ELF64",
        ));
    }
    if section_count == 0 || section_count > MAX_ELF_SECTIONS_V2 {
        return Err(IdentityCodecErrorV2::new(format!(
            "ELF section count must be 1 through {MAX_ELF_SECTIONS_V2}"
        )));
    }
    if string_index == 0 || string_index >= section_count || string_index == 0xffff {
        return Err(IdentityCodecErrorV2::new(
            "ELF section-name string-table index is invalid or extended",
        ));
    }
    let table_bytes = section_entry_size
        .checked_mul(section_count)
        .ok_or_else(|| IdentityCodecErrorV2::new("ELF section table size overflow"))?;
    checked_range(hsaco, section_offset, table_bytes, "ELF section table")?;

    let string_header = section_header(hsaco, section_offset, string_index)?;
    if read_u32(string_header, 4, "ELF string-table type")? != SHT_STRTAB {
        return Err(IdentityCodecErrorV2::new(
            "ELF section-name table is not SHT_STRTAB",
        ));
    }
    let string_offset = usize_from_u64(read_u64(string_header, 24, "ELF string-table offset")?)?;
    let string_size = usize_from_u64(read_u64(string_header, 32, "ELF string-table size")?)?;
    if string_size == 0 || string_size > MAX_ELF_STRING_TABLE_BYTES_V2 {
        return Err(IdentityCodecErrorV2::new(format!(
            "ELF section-name table must contain 1 through {MAX_ELF_STRING_TABLE_BYTES_V2} bytes"
        )));
    }
    let strings = checked_range(hsaco, string_offset, string_size, "ELF section-name table")?;
    if strings[0] != 0 {
        return Err(IdentityCodecErrorV2::new(
            "ELF section-name table has no leading NUL",
        ));
    }

    let mut found = None;
    for index in 0..section_count {
        let header = section_header(hsaco, section_offset, index)?;
        let name_offset = usize::try_from(read_u32(header, 0, "ELF section name offset")?)
            .map_err(|_| IdentityCodecErrorV2::new("ELF section name offset exceeds usize"))?;
        if !elf_name_matches(strings, name_offset, S09_IDENTITY_SECTION_V2.as_bytes())? {
            continue;
        }
        if found.is_some() {
            return Err(IdentityCodecErrorV2::new(
                "HSACO contains duplicate S09 identity sections",
            ));
        }
        if read_u32(header, 4, "S09 identity section type")? != SHT_PROGBITS {
            return Err(IdentityCodecErrorV2::new(
                "S09 identity section is not SHT_PROGBITS",
            ));
        }
        let offset = usize_from_u64(read_u64(header, 24, "S09 identity section offset")?)?;
        let size = usize_from_u64(read_u64(header, 32, "S09 identity section size")?)?;
        if size == 0 || size > MAX_HANDOFF_BYTES_V2 {
            return Err(IdentityCodecErrorV2::new(format!(
                "S09 identity section must contain 1 through {MAX_HANDOFF_BYTES_V2} bytes"
            )));
        }
        found = Some(checked_range(hsaco, offset, size, "S09 identity section")?);
    }
    found.ok_or_else(|| IdentityCodecErrorV2::new("HSACO has no S09 identity section"))
}

pub fn decode_hsaco_identity_claims_v2(
    hsaco: &[u8],
) -> Result<DecodedIdentityHandoffV2, IdentityCodecErrorV2> {
    if hsaco.is_empty() || hsaco.len() > MAX_HSACO_BYTES_V2 {
        return Err(IdentityCodecErrorV2::new(format!(
            "HSACO must contain 1 through {MAX_HSACO_BYTES_V2} bytes"
        )));
    }
    let physical_bindings =
        fe2o3_hsaco::inspect_and_bind_kernel_descriptors(hsaco).map_err(|error| {
            IdentityCodecErrorV2::new(format!("authoritative HSACO inspection failed: {error}"))
        })?;
    let inspection = physical_bindings.inspection();
    let decoded =
        DecodedIdentityHandoffV2::decode(identity_section_after_hsaco_inspection_v2(hsaco)?)?;
    let semantic = decoded.semantic_claim();
    let observed_target = inspection.target().to_string();
    if semantic.target() != "gfx942:xnack-" || observed_target != semantic.target() {
        return Err(IdentityCodecErrorV2::new(format!(
            "S09 target claim must exactly match inspected gfx942:xnack-; claim {:?}, inspection {:?}",
            semantic.target(),
            observed_target
        )));
    }
    if inspection.code_object_version() != fe2o3_hsaco::CodeObjectVersion::V6
        || semantic.code_object_version() != 6
    {
        return Err(IdentityCodecErrorV2::new(
            "S09 requires an inspected V6 code object and a semantic version-6 claim",
        ));
    }
    let kernels = inspection.kernels();
    let bindings = physical_bindings.bindings();
    if semantic.export_name() != "alpha"
        || kernels.len() != 1
        || bindings.len() != 1
        || kernels[0].name() != semantic.export_name()
        || kernels[0].symbol() != "alpha.kd"
        || bindings[0].kernel_index() != 0
    {
        return Err(IdentityCodecErrorV2::new(
            "S09 requires exactly one physically bound alpha/alpha.kd export matching the semantic claim",
        ));
    }
    Ok(decoded)
}

fn encode_fields(fields: &[(&str, String)]) -> Result<Vec<u8>, IdentityCodecErrorV2> {
    let mut bytes = Vec::new();
    for (name, value) in fields {
        validate_field_name(name)?;
        validate_field_value(value, name)?;
        bytes.extend_from_slice(name.as_bytes());
        bytes.push(b'\t');
        bytes.extend_from_slice(value.as_bytes());
        bytes.push(b'\n');
    }
    if bytes.is_empty() || bytes.len() > MAX_RECORD_BYTES_V2 {
        return Err(IdentityCodecErrorV2::new(format!(
            "identity record must contain 1 through {MAX_RECORD_BYTES_V2} bytes"
        )));
    }
    Ok(bytes)
}

fn decode_fields<'a>(
    bytes: &'a [u8],
    expected: &[&str],
    record: &str,
) -> Result<Vec<&'a str>, IdentityCodecErrorV2> {
    if bytes.is_empty() || bytes.len() > MAX_RECORD_BYTES_V2 {
        return Err(IdentityCodecErrorV2::new(format!(
            "{record} record must contain 1 through {MAX_RECORD_BYTES_V2} bytes"
        )));
    }
    if !bytes.ends_with(b"\n") {
        return Err(IdentityCodecErrorV2::new(format!(
            "{record} record is truncated or has trailing data"
        )));
    }
    let lines = bytes[..bytes.len() - 1].split(|byte| *byte == b'\n');
    let mut values = Vec::with_capacity(expected.len());
    for (index, line) in lines.enumerate() {
        if index >= expected.len() {
            return Err(IdentityCodecErrorV2::new(format!(
                "{record} record has an unknown or duplicate field"
            )));
        }
        let mut parts = line.split(|byte| *byte == b'\t');
        let name = parts.next().unwrap_or_default();
        let value = parts
            .next()
            .ok_or_else(|| IdentityCodecErrorV2::new(format!("{record} field has no separator")))?;
        if parts.next().is_some() {
            return Err(IdentityCodecErrorV2::new(format!(
                "{record} field has a duplicate separator"
            )));
        }
        let name = std::str::from_utf8(name)
            .map_err(|_| IdentityCodecErrorV2::new(format!("{record} field name is not UTF-8")))?;
        let value = std::str::from_utf8(value)
            .map_err(|_| IdentityCodecErrorV2::new(format!("{record} field value is not UTF-8")))?;
        validate_field_name(name)?;
        validate_field_value(value, name)?;
        if name != expected[index] {
            return Err(IdentityCodecErrorV2::new(format!(
                "{record} field {index} must be {}; found unknown, duplicate, missing, or reordered field {name}",
                expected[index]
            )));
        }
        values.push(value);
    }
    if values.len() != expected.len() {
        return Err(IdentityCodecErrorV2::new(format!(
            "{record} record has {} fields; expected exactly {}",
            values.len(),
            expected.len()
        )));
    }
    Ok(values)
}

fn validate_field_name(name: &str) -> Result<(), IdentityCodecErrorV2> {
    if name.is_empty()
        || name.len() > MAX_FIELD_NAME_BYTES_V2
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(IdentityCodecErrorV2::new(format!(
            "identity field name must contain 1 through {MAX_FIELD_NAME_BYTES_V2} canonical bytes"
        )));
    }
    Ok(())
}

fn validate_field_value(value: &str, name: &str) -> Result<(), IdentityCodecErrorV2> {
    if value.is_empty()
        || value.len() > MAX_FIELD_VALUE_BYTES_V2
        || !value.bytes().all(|byte| matches!(byte, 0x21..=0x7e))
    {
        return Err(IdentityCodecErrorV2::new(format!(
            "identity field {name} must contain 1 through {MAX_FIELD_VALUE_BYTES_V2} canonical bytes"
        )));
    }
    Ok(())
}

fn require_exact(actual: &str, expected: &str, field: &str) -> Result<(), IdentityCodecErrorV2> {
    if actual != expected {
        return Err(IdentityCodecErrorV2::new(format!(
            "{field} is missing or unknown"
        )));
    }
    Ok(())
}

fn decode_digest(value: &str, field: &str) -> Result<[u8; 32], IdentityCodecErrorV2> {
    let bytes = value.as_bytes();
    if bytes.len() != 64
        || !bytes
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(IdentityCodecErrorV2::new(format!(
            "{field} must contain exactly 64 lowercase hexadecimal digits"
        )));
    }
    let mut digest = [0_u8; 32];
    for (index, pair) in bytes.chunks_exact(2).enumerate() {
        digest[index] = (hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]);
    }
    if digest == [0; 32] {
        return Err(IdentityCodecErrorV2::new(format!(
            "{field} must not be zero"
        )));
    }
    Ok(digest)
}

fn decode_decimal(value: &str, field: &str, allow_zero: bool) -> Result<u64, IdentityCodecErrorV2> {
    if !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(IdentityCodecErrorV2::new(format!(
            "{field} is not a canonical decimal"
        )));
    }
    let decoded = value
        .parse::<u64>()
        .map_err(|_| IdentityCodecErrorV2::new(format!("{field} exceeds u64")))?;
    if decoded == 0 && !allow_zero {
        return Err(IdentityCodecErrorV2::new(format!(
            "{field} must not be zero"
        )));
    }
    Ok(decoded)
}

fn take_record<'a>(
    bytes: &'a [u8],
    offset: &mut usize,
    record: &str,
) -> Result<&'a [u8], IdentityCodecErrorV2> {
    let length_bytes = checked_range(bytes, *offset, 4, &format!("{record} length"))?;
    *offset += 4;
    let length = usize::try_from(u32::from_le_bytes(
        length_bytes
            .try_into()
            .expect("checked four-byte record length"),
    ))
    .map_err(|_| IdentityCodecErrorV2::new(format!("{record} length exceeds usize")))?;
    if length == 0 || length > MAX_RECORD_BYTES_V2 {
        return Err(IdentityCodecErrorV2::new(format!(
            "{record} length must be 1 through {MAX_RECORD_BYTES_V2}"
        )));
    }
    let record_bytes = checked_range(bytes, *offset, length, record)?;
    *offset += length;
    Ok(record_bytes)
}

fn take_digest(
    bytes: &[u8],
    offset: &mut usize,
    field: &str,
) -> Result<[u8; 32], IdentityCodecErrorV2> {
    let digest: [u8; 32] = checked_range(bytes, *offset, 32, field)?
        .try_into()
        .expect("checked 32-byte identity digest");
    *offset += 32;
    if digest == [0; 32] {
        return Err(IdentityCodecErrorV2::new(format!(
            "{field} must not be zero"
        )));
    }
    Ok(digest)
}

fn section_header(
    bytes: &[u8],
    table_offset: usize,
    index: usize,
) -> Result<&[u8], IdentityCodecErrorV2> {
    let offset = index
        .checked_mul(ELF64_SECTION_HEADER_BYTES)
        .and_then(|value| table_offset.checked_add(value))
        .ok_or_else(|| IdentityCodecErrorV2::new("ELF section-header offset overflow"))?;
    checked_range(
        bytes,
        offset,
        ELF64_SECTION_HEADER_BYTES,
        "ELF section header",
    )
}

fn elf_name_matches(
    strings: &[u8],
    offset: usize,
    expected: &[u8],
) -> Result<bool, IdentityCodecErrorV2> {
    if offset >= strings.len() {
        return Err(IdentityCodecErrorV2::new(
            "ELF section name offset is out of bounds",
        ));
    }
    let Some(end) = offset.checked_add(expected.len()) else {
        return Ok(false);
    };
    Ok(strings.get(offset..end) == Some(expected) && strings.get(end) == Some(&0))
}

fn checked_range<'a>(
    bytes: &'a [u8],
    offset: usize,
    length: usize,
    context: &str,
) -> Result<&'a [u8], IdentityCodecErrorV2> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| IdentityCodecErrorV2::new(format!("{context} range overflow")))?;
    bytes
        .get(offset..end)
        .ok_or_else(|| IdentityCodecErrorV2::new(format!("{context} is truncated")))
}

fn read_u16(bytes: &[u8], offset: usize, context: &str) -> Result<u16, IdentityCodecErrorV2> {
    Ok(u16::from_le_bytes(
        checked_range(bytes, offset, 2, context)?
            .try_into()
            .expect("checked two-byte integer"),
    ))
}

fn read_u32(bytes: &[u8], offset: usize, context: &str) -> Result<u32, IdentityCodecErrorV2> {
    Ok(u32::from_le_bytes(
        checked_range(bytes, offset, 4, context)?
            .try_into()
            .expect("checked four-byte integer"),
    ))
}

fn read_u64(bytes: &[u8], offset: usize, context: &str) -> Result<u64, IdentityCodecErrorV2> {
    Ok(u64::from_le_bytes(
        checked_range(bytes, offset, 8, context)?
            .try_into()
            .expect("checked eight-byte integer"),
    ))
}

fn usize_from_u64(value: u64) -> Result<usize, IdentityCodecErrorV2> {
    usize::try_from(value).map_err(|_| IdentityCodecErrorV2::new("ELF offset exceeds usize"))
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

const fn hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmpv::{Value, encode::write_value};

    fn semantic_with_code_object_version(code_object_version: u16) -> SemanticIdentityClaimV2 {
        SemanticIdentityClaimV2::from_fields(SemanticIdentityClaimFieldsV2 {
            crate_name: "fixture",
            module: "module",
            logical_name: "alpha",
            export_name: "alpha",
            profile: "profile-v2",
            source_path: "src/main.rs",
            source_sha256: [1; 32],
            source_bytes: 3231,
            target: "gfx942:xnack-",
            target_capabilities: "atomics,amd-wave",
            code_object_version,
            rustc_opt_level: 0,
            rustc_debug_info: "full",
            injected_debug_policy: "dwarf-v5-full",
            abi_sha256: [2; 32],
            launch_sha256: [3; 32],
            portable_mir_sha256: [4; 32],
        })
        .unwrap()
    }

    fn semantic() -> SemanticIdentityClaimV2 {
        semantic_with_code_object_version(6)
    }

    fn build(semantic: &SemanticIdentityClaimV2) -> BuildIdentityClaimV2 {
        BuildIdentityClaimV2::from_fields(BuildIdentityClaimFieldsV2 {
            semantic_claim_sha256: *semantic.identity_sha256(),
            cargo_metadata_sha256: [5; 32],
            crate_binding: [6; 32],
            kernel_binding: [7; 32],
            observed_def_path: "module::__fe2o3_host_kernel_v1_abc",
            observed_symbol: "__fe2o3_host_kernel_v1_abc",
            rustc_mir_capture_sha256: [8; 32],
            prepared_rustc_command_sha256: [9; 32],
            rustc_executable_sha256: [10; 32],
            cargo_fe2o3_executable_sha256: [11; 32],
            declared_cargo_executable_sha256: [12; 32],
            cargo_launcher_executable_sha256: [13; 32],
            cargo_launcher_pid: 14,
            cargo_launcher_start_time_ticks: 15,
            codegen_backend_sha256: [16; 32],
            worker_config_sha256: [17; 32],
            worker_executable_sha256: [18; 32],
            worker_build_identity_sha256: [19; 32],
            llvm_build_identity_sha256: [20; 32],
        })
        .unwrap()
    }

    fn handoff() -> DecodedIdentityHandoffV2 {
        let semantic = semantic();
        let build = build(&semantic);
        DecodedIdentityHandoffV2::from_claims(semantic, build).unwrap()
    }

    fn handoff_with_code_object_version(code_object_version: u16) -> DecodedIdentityHandoffV2 {
        let semantic = semantic_with_code_object_version(code_object_version);
        let build = build(&semantic);
        DecodedIdentityHandoffV2::from_claims(semantic, build).unwrap()
    }

    fn replace_record(
        handoff: &DecodedIdentityHandoffV2,
        semantic_record: Option<&[u8]>,
        build_record: Option<&[u8]>,
    ) -> Vec<u8> {
        let semantic = semantic_record.unwrap_or(handoff.semantic_claim().canonical_bytes());
        let build = build_record.unwrap_or(handoff.build_claim().canonical_bytes());
        let mut bytes = Vec::new();
        bytes.extend_from_slice(HANDOFF_DOMAIN_V2);
        bytes.extend_from_slice(&Sha256::digest(semantic));
        bytes.extend_from_slice(&Sha256::digest(build));
        bytes.extend_from_slice(&(semantic.len() as u32).to_le_bytes());
        bytes.extend_from_slice(semantic);
        bytes.extend_from_slice(&(build.len() as u32).to_le_bytes());
        bytes.extend_from_slice(build);
        bytes
    }

    fn record_lines(record: &[u8]) -> Vec<Vec<u8>> {
        record
            .split_inclusive(|byte| *byte == b'\n')
            .map(<[u8]>::to_vec)
            .collect()
    }

    fn mutate_field_name(record: &[u8], index: usize) -> Vec<u8> {
        let mut lines = record_lines(record);
        lines[index][0] = if lines[index][0] == b'x' { b'y' } else { b'x' };
        lines.concat()
    }

    fn remove_field(record: &[u8], index: usize) -> Vec<u8> {
        let mut lines = record_lines(record);
        lines.remove(index);
        lines.concat()
    }

    fn empty_field(record: &[u8], index: usize) -> Vec<u8> {
        let mut lines = record_lines(record);
        let tab = lines[index].iter().position(|byte| *byte == b'\t').unwrap();
        lines[index].truncate(tab + 1);
        lines[index].push(b'\n');
        lines.concat()
    }

    fn replace_first(bytes: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
        let start = bytes
            .windows(from.len())
            .position(|window| window == from)
            .expect("fixture pattern");
        let mut result = Vec::with_capacity(bytes.len() - from.len() + to.len());
        result.extend_from_slice(&bytes[..start]);
        result.extend_from_slice(to);
        result.extend_from_slice(&bytes[start + from.len()..]);
        result
    }

    fn align(bytes: &mut Vec<u8>, alignment: usize) {
        while !bytes.len().is_multiple_of(alignment) {
            bytes.push(0);
        }
    }

    fn hidden_arguments(base: u64) -> Vec<Value> {
        [
            (0, 4, "hidden_block_count_x"),
            (4, 4, "hidden_block_count_y"),
            (8, 4, "hidden_block_count_z"),
            (12, 2, "hidden_group_size_x"),
            (14, 2, "hidden_group_size_y"),
            (16, 2, "hidden_group_size_z"),
            (18, 2, "hidden_remainder_x"),
            (20, 2, "hidden_remainder_y"),
            (22, 2, "hidden_remainder_z"),
            (40, 8, "hidden_global_offset_x"),
            (48, 8, "hidden_global_offset_y"),
            (56, 8, "hidden_global_offset_z"),
            (64, 2, "hidden_grid_dims"),
        ]
        .into_iter()
        .map(|(offset, size, kind)| {
            Value::Map(vec![
                (Value::from(".offset"), Value::from(base + offset)),
                (Value::from(".size"), Value::from(size)),
                (Value::from(".value_kind"), Value::from(kind)),
            ])
        })
        .collect()
    }

    fn metadata_kernel(name: &str, symbol: &str) -> Value {
        Value::Map(vec![
            (Value::from(".name"), Value::from(name)),
            (Value::from(".symbol"), Value::from(symbol)),
            (Value::from(".args"), Value::Array(hidden_arguments(0))),
            (Value::from(".kernarg_segment_size"), Value::from(256)),
            (Value::from(".kernarg_segment_align"), Value::from(8)),
            (Value::from(".group_segment_fixed_size"), Value::from(0)),
            (Value::from(".private_segment_fixed_size"), Value::from(0)),
            (Value::from(".wavefront_size"), Value::from(64)),
            (Value::from(".sgpr_count"), Value::from(14)),
            (Value::from(".vgpr_count"), Value::from(11)),
            (Value::from(".agpr_count"), Value::from(3)),
            (Value::from(".max_flat_workgroup_size"), Value::from(256)),
        ])
    }

    fn metadata(kernels: &[(&str, &str)]) -> Vec<u8> {
        let root = Value::Map(vec![
            (
                Value::from("amdhsa.version"),
                Value::Array(vec![Value::from(1), Value::from(2)]),
            ),
            (
                Value::from("amdhsa.target"),
                Value::from("amdgcn-amd-amdhsa--gfx942:xnack-"),
            ),
            (
                Value::from("amdhsa.kernels"),
                Value::Array(
                    kernels
                        .iter()
                        .map(|(name, symbol)| metadata_kernel(name, symbol))
                        .collect(),
                ),
            ),
        ]);
        let mut encoded = Vec::new();
        write_value(&mut encoded, &root).unwrap();
        encoded
    }

    fn metadata_note(kernels: &[(&str, &str)]) -> Vec<u8> {
        let owner = b"AMDGPU\0";
        let metadata = metadata(kernels);
        let mut note = Vec::new();
        note.extend_from_slice(&(owner.len() as u32).to_le_bytes());
        note.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
        note.extend_from_slice(&32_u32.to_le_bytes());
        note.extend_from_slice(owner);
        align(&mut note, 4);
        note.extend_from_slice(&metadata);
        align(&mut note, 4);
        note
    }

    struct BoundHsacoFixture {
        bytes: Vec<u8>,
        entry_name_offset: usize,
        descriptor_name_offset: usize,
    }

    fn append_string(table: &mut Vec<u8>, value: &str) -> u32 {
        let offset = u32::try_from(table.len()).unwrap();
        table.extend_from_slice(value.as_bytes());
        table.push(0);
        offset
    }

    fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn write_i64(bytes: &mut [u8], offset: usize, value: i64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn bound_elf(
        identity_sections: &[&[u8]],
        abi_version: u8,
        kernels: &[(&str, &str)],
    ) -> BoundHsacoFixture {
        const PROGRAM_HEADER_BYTES: usize = 56;
        const SYMBOL_BYTES: usize = 24;
        const PROGRAM_COUNT: usize = 2;

        let mut bytes = vec![0_u8; ELF64_HEADER_BYTES + PROGRAM_COUNT * PROGRAM_HEADER_BYTES];
        align(&mut bytes, 64);
        let note = metadata_note(kernels);
        let note_offset = bytes.len();
        bytes.extend_from_slice(&note);
        align(&mut bytes, 64);

        let descriptor_offset = bytes.len();
        bytes.resize(descriptor_offset + 64, 0);
        align(&mut bytes, 256);
        let entry_offset = bytes.len();
        bytes.resize(entry_offset + 64, 0xbf);
        let descriptor_address = descriptor_offset as u64;
        let entry_address = entry_offset as u64 + 0x1000;

        let mut identity_offsets = Vec::new();
        for identity in identity_sections {
            identity_offsets.push(bytes.len());
            bytes.extend_from_slice(identity);
        }

        let mut symbol_strings = vec![0];
        let entry_name = append_string(&mut symbol_strings, "alpha");
        let descriptor_name = append_string(&mut symbol_strings, "alpha.kd");
        let other_name = append_string(&mut symbol_strings, "other");
        let symbol_string_offset = bytes.len();
        let entry_name_offset = symbol_string_offset + entry_name as usize;
        let descriptor_name_offset = symbol_string_offset + descriptor_name as usize;
        bytes.extend_from_slice(&symbol_strings);
        align(&mut bytes, 8);

        let symbol_offset = bytes.len();
        bytes.resize(symbol_offset + 4 * SYMBOL_BYTES, 0);
        let entry_symbol = symbol_offset + SYMBOL_BYTES;
        write_u32(&mut bytes, entry_symbol, entry_name);
        bytes[entry_symbol + 4] = 0x12;
        bytes[entry_symbol + 5] = 3;
        write_u16(&mut bytes, entry_symbol + 6, 3);
        write_u64(&mut bytes, entry_symbol + 8, entry_address);
        write_u64(&mut bytes, entry_symbol + 16, 64);

        let descriptor_symbol = symbol_offset + 2 * SYMBOL_BYTES;
        write_u32(&mut bytes, descriptor_symbol, descriptor_name);
        bytes[descriptor_symbol + 4] = 0x11;
        write_u16(&mut bytes, descriptor_symbol + 6, 2);
        write_u64(&mut bytes, descriptor_symbol + 8, descriptor_address);
        write_u64(&mut bytes, descriptor_symbol + 16, 64);

        let spare_symbol = symbol_offset + 3 * SYMBOL_BYTES;
        write_u32(&mut bytes, spare_symbol, other_name);
        bytes[spare_symbol + 4] = 0x10;
        write_u16(&mut bytes, spare_symbol + 6, 0xfff1);

        let mut section_strings = vec![0];
        let note_name = append_string(&mut section_strings, ".note");
        let rodata_name = append_string(&mut section_strings, ".rodata");
        let text_name = append_string(&mut section_strings, ".text");
        let strtab_name = append_string(&mut section_strings, ".strtab");
        let symtab_name = append_string(&mut section_strings, ".symtab");
        let identity_name = append_string(&mut section_strings, S09_IDENTITY_SECTION_V2);
        let shstrtab_name = append_string(&mut section_strings, ".shstrtab");
        let section_string_offset = bytes.len();
        bytes.extend_from_slice(&section_strings);
        align(&mut bytes, 8);

        let section_offset = bytes.len();
        let section_count = 7 + identity_sections.len();
        let string_index = section_count - 1;
        bytes.resize(
            section_offset + section_count * ELF64_SECTION_HEADER_BYTES,
            0,
        );
        bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        bytes[7] = 64;
        bytes[8] = abi_version;
        write_u16(&mut bytes, 16, 3);
        write_u16(&mut bytes, 18, 224);
        write_u32(&mut bytes, 20, 1);
        write_u64(&mut bytes, 32, ELF64_HEADER_BYTES as u64);
        write_u64(&mut bytes, 40, section_offset as u64);
        write_u32(&mut bytes, 48, 0x64c);
        write_u16(&mut bytes, 52, ELF64_HEADER_BYTES as u16);
        write_u16(&mut bytes, 54, PROGRAM_HEADER_BYTES as u16);
        write_u16(&mut bytes, 56, PROGRAM_COUNT as u16);
        write_u16(&mut bytes, 58, ELF64_SECTION_HEADER_BYTES as u16);
        write_u16(&mut bytes, 60, section_count as u16);
        write_u16(&mut bytes, 62, string_index as u16);

        let descriptor_load = ELF64_HEADER_BYTES;
        write_u32(&mut bytes, descriptor_load, 1);
        write_u32(&mut bytes, descriptor_load + 4, 4);
        write_u64(
            &mut bytes,
            descriptor_load + 32,
            (descriptor_offset + 64) as u64,
        );
        write_u64(
            &mut bytes,
            descriptor_load + 40,
            (descriptor_offset + 64) as u64,
        );
        write_u64(&mut bytes, descriptor_load + 48, 0x1000);

        let entry_load = descriptor_load + PROGRAM_HEADER_BYTES;
        write_u32(&mut bytes, entry_load, 1);
        write_u32(&mut bytes, entry_load + 4, 5);
        write_u64(&mut bytes, entry_load + 8, entry_offset as u64);
        write_u64(&mut bytes, entry_load + 16, entry_address);
        write_u64(&mut bytes, entry_load + 32, 64);
        write_u64(&mut bytes, entry_load + 40, 64);
        write_u64(&mut bytes, entry_load + 48, 0x1000);

        let note_header = section_offset + ELF64_SECTION_HEADER_BYTES;
        write_u32(&mut bytes, note_header, note_name);
        write_u32(&mut bytes, note_header + 4, 7);
        write_u64(&mut bytes, note_header + 8, 2);
        write_u64(&mut bytes, note_header + 16, note_offset as u64);
        write_u64(&mut bytes, note_header + 24, note_offset as u64);
        write_u64(&mut bytes, note_header + 32, note.len() as u64);
        write_u64(&mut bytes, note_header + 48, 4);

        let rodata_header = section_offset + 2 * ELF64_SECTION_HEADER_BYTES;
        write_u32(&mut bytes, rodata_header, rodata_name);
        write_u32(&mut bytes, rodata_header + 4, SHT_PROGBITS);
        write_u64(&mut bytes, rodata_header + 8, 2);
        write_u64(&mut bytes, rodata_header + 16, descriptor_address);
        write_u64(&mut bytes, rodata_header + 24, descriptor_offset as u64);
        write_u64(&mut bytes, rodata_header + 32, 64);
        write_u64(&mut bytes, rodata_header + 48, 64);

        let text_header = section_offset + 3 * ELF64_SECTION_HEADER_BYTES;
        write_u32(&mut bytes, text_header, text_name);
        write_u32(&mut bytes, text_header + 4, SHT_PROGBITS);
        write_u64(&mut bytes, text_header + 8, 6);
        write_u64(&mut bytes, text_header + 16, entry_address);
        write_u64(&mut bytes, text_header + 24, entry_offset as u64);
        write_u64(&mut bytes, text_header + 32, 64);
        write_u64(&mut bytes, text_header + 48, 256);

        let strtab_header = section_offset + 4 * ELF64_SECTION_HEADER_BYTES;
        write_u32(&mut bytes, strtab_header, strtab_name);
        write_u32(&mut bytes, strtab_header + 4, SHT_STRTAB);
        write_u64(&mut bytes, strtab_header + 24, symbol_string_offset as u64);
        write_u64(&mut bytes, strtab_header + 32, symbol_strings.len() as u64);
        write_u64(&mut bytes, strtab_header + 48, 1);

        let symtab_header = section_offset + 5 * ELF64_SECTION_HEADER_BYTES;
        write_u32(&mut bytes, symtab_header, symtab_name);
        write_u32(&mut bytes, symtab_header + 4, 2);
        write_u64(&mut bytes, symtab_header + 24, symbol_offset as u64);
        write_u64(&mut bytes, symtab_header + 32, (4 * SYMBOL_BYTES) as u64);
        write_u32(&mut bytes, symtab_header + 40, 4);
        write_u32(&mut bytes, symtab_header + 44, 1);
        write_u64(&mut bytes, symtab_header + 48, 8);
        write_u64(&mut bytes, symtab_header + 56, SYMBOL_BYTES as u64);

        for (index, (offset, identity)) in identity_offsets
            .iter()
            .zip(identity_sections.iter())
            .enumerate()
        {
            let header = section_offset + (index + 6) * ELF64_SECTION_HEADER_BYTES;
            write_u32(&mut bytes, header, identity_name);
            write_u32(&mut bytes, header + 4, SHT_PROGBITS);
            write_u64(&mut bytes, header + 24, *offset as u64);
            write_u64(&mut bytes, header + 32, identity.len() as u64);
        }

        let section_strings_header = section_offset + string_index * ELF64_SECTION_HEADER_BYTES;
        write_u32(&mut bytes, section_strings_header, shstrtab_name);
        write_u32(&mut bytes, section_strings_header + 4, SHT_STRTAB);
        write_u64(
            &mut bytes,
            section_strings_header + 24,
            section_string_offset as u64,
        );
        write_u64(
            &mut bytes,
            section_strings_header + 32,
            section_strings.len() as u64,
        );
        write_u64(&mut bytes, section_strings_header + 48, 1);

        write_u32(&mut bytes, descriptor_offset, 0);
        write_u32(&mut bytes, descriptor_offset + 4, 0);
        write_u32(&mut bytes, descriptor_offset + 8, 256);
        write_i64(
            &mut bytes,
            descriptor_offset + 16,
            i64::try_from(entry_address - descriptor_address).unwrap(),
        );
        write_u32(&mut bytes, descriptor_offset + 44, 1);
        write_u32(&mut bytes, descriptor_offset + 48, 0x00af_0081);
        write_u32(&mut bytes, descriptor_offset + 52, 0);
        write_u16(&mut bytes, descriptor_offset + 56, 0x001e);

        BoundHsacoFixture {
            bytes,
            entry_name_offset,
            descriptor_name_offset,
        }
    }

    fn elf(identity_sections: &[&[u8]]) -> Vec<u8> {
        bound_elf(identity_sections, 4, &[("alpha", "alpha.kd")]).bytes
    }

    #[test]
    fn canonical_records_and_handoff_round_trip_exactly() {
        let handoff = handoff();
        let decoded = DecodedIdentityHandoffV2::decode(handoff.canonical_bytes()).unwrap();
        assert_eq!(decoded, handoff);
        assert_eq!(decoded.semantic_claim().rustc_opt_level(), 0);
        assert_eq!(decoded.semantic_claim().source_bytes(), 3231);
        assert_eq!(
            decoded.build_claim().prepared_rustc_command_sha256(),
            &[9; 32]
        );
        assert_eq!(
            decoded.build_claim().cargo_fe2o3_executable_sha256(),
            &[11; 32]
        );
        assert_eq!(
            decoded.build_claim().declared_cargo_executable_sha256(),
            &[12; 32]
        );
        assert_eq!(
            decoded.build_claim().cargo_launcher_executable_sha256(),
            &[13; 32]
        );
        assert_eq!(decoded.build_claim().cargo_launcher_pid(), 14);
        assert_eq!(decoded.build_claim().cargo_launcher_start_time_ticks(), 15);
    }

    #[test]
    fn every_field_rejects_unknown_empty_and_missing_encodings() {
        let handoff = handoff();
        for index in 0..SEMANTIC_CLAIM_FIELDS_V2.len() {
            for mutation in [
                mutate_field_name(handoff.semantic_claim().canonical_bytes(), index),
                empty_field(handoff.semantic_claim().canonical_bytes(), index),
                remove_field(handoff.semantic_claim().canonical_bytes(), index),
            ] {
                assert!(
                    DecodedIdentityHandoffV2::decode(&replace_record(
                        &handoff,
                        Some(&mutation),
                        None,
                    ))
                    .is_err(),
                    "semantic field {index} mutation was accepted"
                );
            }
        }
        for index in 0..BUILD_CLAIM_FIELDS_V2.len() {
            for mutation in [
                mutate_field_name(handoff.build_claim().canonical_bytes(), index),
                empty_field(handoff.build_claim().canonical_bytes(), index),
                remove_field(handoff.build_claim().canonical_bytes(), index),
            ] {
                assert!(
                    DecodedIdentityHandoffV2::decode(&replace_record(
                        &handoff,
                        None,
                        Some(&mutation),
                    ))
                    .is_err(),
                    "build field {index} mutation was accepted"
                );
            }
        }
    }

    #[test]
    fn rejects_duplicate_reordered_zero_oversize_and_noncanonical_fields() {
        let handoff = handoff();
        let semantic = handoff.semantic_claim().canonical_bytes();
        let mut duplicate = semantic.to_vec();
        let first_end = semantic.iter().position(|byte| *byte == b'\n').unwrap() + 1;
        duplicate.extend_from_slice(&semantic[..first_end]);
        assert!(
            DecodedIdentityHandoffV2::decode(&replace_record(&handoff, Some(&duplicate), None,))
                .is_err()
        );

        let mut lines = record_lines(semantic);
        lines.swap(1, 2);
        assert!(
            DecodedIdentityHandoffV2::decode(&replace_record(
                &handoff,
                Some(&lines.concat()),
                None,
            ))
            .is_err()
        );

        let zero_digest = replace_first(
            handoff.build_claim().canonical_bytes(),
            b"prepared_rustc_command_sha256\t0909090909090909090909090909090909090909090909090909090909090909\n",
            b"prepared_rustc_command_sha256\t0000000000000000000000000000000000000000000000000000000000000000\n",
        );
        assert!(
            DecodedIdentityHandoffV2::decode(&replace_record(&handoff, None, Some(&zero_digest),))
                .is_err()
        );

        let mut oversize = b"crate\t".to_vec();
        oversize.extend(std::iter::repeat_n(b'x', MAX_FIELD_VALUE_BYTES_V2 + 1));
        oversize.push(b'\n');
        let mut lines = record_lines(semantic);
        lines[1] = oversize;
        assert!(
            DecodedIdentityHandoffV2::decode(&replace_record(
                &handoff,
                Some(&lines.concat()),
                None,
            ))
            .is_err()
        );

        let noncanonical_decimal =
            replace_first(semantic, b"source_bytes\t3231\n", b"source_bytes\t03231\n");
        assert!(
            DecodedIdentityHandoffV2::decode(&replace_record(
                &handoff,
                Some(&noncanonical_decimal),
                None,
            ))
            .is_err()
        );
    }

    #[test]
    fn rejects_zero_lengths_trailing_bytes_and_every_truncation() {
        let handoff = handoff();
        let bytes = handoff.canonical_bytes();
        let mut zero = bytes.to_vec();
        let semantic_length = HANDOFF_DOMAIN_V2.len() + 64;
        zero[semantic_length..semantic_length + 4].copy_from_slice(&0_u32.to_le_bytes());
        assert!(DecodedIdentityHandoffV2::decode(&zero).is_err());

        for manifest_offset in [HANDOFF_DOMAIN_V2.len(), HANDOFF_DOMAIN_V2.len() + 32] {
            let mut wrong_manifest = bytes.to_vec();
            wrong_manifest[manifest_offset] ^= 1;
            assert!(DecodedIdentityHandoffV2::decode(&wrong_manifest).is_err());
        }

        let mut trailing = bytes.to_vec();
        trailing.push(0);
        assert!(DecodedIdentityHandoffV2::decode(&trailing).is_err());

        for length in 0..bytes.len() {
            assert!(
                DecodedIdentityHandoffV2::decode(&bytes[..length]).is_err(),
                "truncation at {length} bytes was accepted"
            );
        }
    }

    #[test]
    fn hsaco_claim_decode_requires_exact_s09_elf_target_metadata_and_linkage() {
        let handoff = handoff();
        let one = elf(&[handoff.canonical_bytes()]);
        assert_eq!(
            identity_section_v2(&one).unwrap(),
            handoff.canonical_bytes()
        );
        assert_eq!(decode_hsaco_identity_claims_v2(&one).unwrap(), handoff);

        for (offset, replacement) in [(16, 2_u16.to_le_bytes()), (18, 62_u16.to_le_bytes())] {
            let mut wrong = one.clone();
            wrong[offset..offset + 2].copy_from_slice(&replacement);
            assert!(decode_hsaco_identity_claims_v2(&wrong).is_err());
        }
        let mut wrong_osabi = one.clone();
        wrong_osabi[7] = 0;
        assert!(decode_hsaco_identity_claims_v2(&wrong_osabi).is_err());
        let mut wrong_code_object_abi = one.clone();
        wrong_code_object_abi[8] = 1;
        assert!(decode_hsaco_identity_claims_v2(&wrong_code_object_abi).is_err());

        let mut wrong_features = one.clone();
        wrong_features[48..52].copy_from_slice(&0x74c_u32.to_le_bytes());
        assert!(decode_hsaco_identity_claims_v2(&wrong_features).is_err());

        let mut wrong_metadata_feature = one.clone();
        let feature = wrong_metadata_feature
            .windows(b"gfx942:xnack-".len())
            .position(|window| window == b"gfx942:xnack-")
            .unwrap();
        wrong_metadata_feature[feature + b"gfx942:xnack".len()] = b'+';
        assert!(decode_hsaco_identity_claims_v2(&wrong_metadata_feature).is_err());

        let wrong_target_claim = replace_first(
            handoff.semantic_claim().canonical_bytes(),
            b"target\tgfx942:xnack-\n",
            b"target\tgfx942:xnack+\n",
        );
        let wrong_target_handoff = replace_record(&handoff, Some(&wrong_target_claim), None);
        assert!(decode_hsaco_identity_claims_v2(&elf(&[&wrong_target_handoff])).is_err());

        let wrong_version_claim = replace_first(
            handoff.semantic_claim().canonical_bytes(),
            b"code_object_version\t6\n",
            b"code_object_version\t5\n",
        );
        let wrong_version_handoff = replace_record(&handoff, Some(&wrong_version_claim), None);
        assert!(decode_hsaco_identity_claims_v2(&elf(&[&wrong_version_handoff])).is_err());

        let mut wrong_kernel_linkage = one.clone();
        let kernel = wrong_kernel_linkage
            .windows(b"alpha".len())
            .position(|window| window == b"alpha")
            .unwrap();
        wrong_kernel_linkage[kernel..kernel + b"alpha".len()].copy_from_slice(b"omega");
        assert!(decode_hsaco_identity_claims_v2(&wrong_kernel_linkage).is_err());

        let mut missing_metadata = one.clone();
        let owner = missing_metadata
            .windows(b"AMDGPU".len())
            .position(|window| window == b"AMDGPU")
            .unwrap();
        missing_metadata[owner..owner + b"AMDGPU".len()].copy_from_slice(b"NOTGPU");
        assert!(decode_hsaco_identity_claims_v2(&missing_metadata).is_err());

        let mut malformed_metadata = one.clone();
        let target_key = malformed_metadata
            .windows(b"amdhsa.target".len())
            .position(|window| window == b"amdhsa.target")
            .unwrap();
        malformed_metadata[target_key + b"amdhsa.targe".len()] = b'X';
        assert!(decode_hsaco_identity_claims_v2(&malformed_metadata).is_err());

        assert!(identity_section_v2(&elf(&[])).is_err());
        assert!(
            identity_section_v2(&elf(&[
                handoff.canonical_bytes(),
                handoff.canonical_bytes()
            ]))
            .is_err()
        );
        for length in 0..one.len() {
            assert!(identity_section_v2(&one[..length]).is_err());
        }
    }

    #[test]
    fn hsaco_claim_decode_accepts_one_physically_bound_v6_alpha_export() {
        let handoff = handoff();
        let fixture = bound_elf(&[handoff.canonical_bytes()], 4, &[("alpha", "alpha.kd")]);
        let physical = fe2o3_hsaco::inspect_and_bind_kernel_descriptors(&fixture.bytes).unwrap();
        assert_eq!(physical.inspection().kernels().len(), 1);
        assert_eq!(physical.bindings().len(), 1);
        assert_eq!(physical.bindings()[0].kernel_index(), 0);
        assert_eq!(
            decode_hsaco_identity_claims_v2(&fixture.bytes).unwrap(),
            handoff
        );
    }

    #[test]
    fn hsaco_claim_decode_rejects_coherent_v5_artifact_and_claim() {
        let handoff = handoff_with_code_object_version(5);
        let fixture = bound_elf(&[handoff.canonical_bytes()], 3, &[("alpha", "alpha.kd")]);
        let physical = fe2o3_hsaco::inspect_and_bind_kernel_descriptors(&fixture.bytes).unwrap();
        assert_eq!(
            physical.inspection().code_object_version(),
            fe2o3_hsaco::CodeObjectVersion::V5
        );
        assert!(decode_hsaco_identity_claims_v2(&fixture.bytes).is_err());
    }

    #[test]
    fn hsaco_claim_decode_rejects_wrong_or_missing_physical_symbols() {
        let handoff = handoff();

        let mut wrong_entry = bound_elf(&[handoff.canonical_bytes()], 4, &[("alpha", "alpha.kd")]);
        wrong_entry.bytes[wrong_entry.entry_name_offset..wrong_entry.entry_name_offset + 5]
            .copy_from_slice(b"omega");
        assert!(fe2o3_hsaco::inspect_and_bind_kernel_descriptors(&wrong_entry.bytes).is_err());
        assert!(decode_hsaco_identity_claims_v2(&wrong_entry.bytes).is_err());

        let mut wrong_descriptor =
            bound_elf(&[handoff.canonical_bytes()], 4, &[("alpha", "alpha.kd")]);
        wrong_descriptor.bytes
            [wrong_descriptor.descriptor_name_offset..wrong_descriptor.descriptor_name_offset + 8]
            .copy_from_slice(b"omega.kd");
        assert!(fe2o3_hsaco::inspect_and_bind_kernel_descriptors(&wrong_descriptor.bytes).is_err());
        assert!(decode_hsaco_identity_claims_v2(&wrong_descriptor.bytes).is_err());
    }

    #[test]
    fn hsaco_claim_decode_rejects_metadata_symbol_mismatch_and_extra_kernel() {
        let handoff = handoff();
        let mismatched_metadata =
            bound_elf(&[handoff.canonical_bytes()], 4, &[("alpha", "omega.kd")]);
        assert!(
            fe2o3_hsaco::inspect_and_bind_kernel_descriptors(&mismatched_metadata.bytes).is_err()
        );
        assert!(decode_hsaco_identity_claims_v2(&mismatched_metadata.bytes).is_err());

        let extra_kernel = bound_elf(
            &[handoff.canonical_bytes()],
            4,
            &[("alpha", "alpha.kd"), ("beta", "beta.kd")],
        );
        assert!(fe2o3_hsaco::inspect(&extra_kernel.bytes).is_ok());
        assert!(fe2o3_hsaco::inspect_and_bind_kernel_descriptors(&extra_kernel.bytes).is_err());
        assert!(decode_hsaco_identity_claims_v2(&extra_kernel.bytes).is_err());
    }
}
