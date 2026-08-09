//! Canonical, bounded S09 semantic-admission and build-observation records.

use sha2::{Digest, Sha256};
use std::error::Error;
use std::fmt;

pub const S09_IDENTITY_SECTION_V2: &str = ".fe2o3.s09.identity.v2";
pub const SEMANTIC_SCHEMA_V2: &str = "fe2o3-s09-semantic-admission-v2";
pub const BUILD_SCHEMA_V2: &str = "fe2o3-s09-build-observation-v2";
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

const SEMANTIC_FIELDS_V2: [&str; 18] = [
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

const BUILD_FIELDS_V2: [&str; 20] = [
    "schema",
    "semantic_admission_sha256",
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

pub(crate) struct SemanticAdmissionFieldsV2<'a> {
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
pub struct SemanticAdmissionV2 {
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

impl SemanticAdmissionV2 {
    pub(crate) fn from_fields(
        fields: SemanticAdmissionFieldsV2<'_>,
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
        let fields = decode_fields(bytes, &SEMANTIC_FIELDS_V2, "semantic admission")?;
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

pub(crate) struct BuildObservationFieldsV2<'a> {
    pub(crate) semantic_admission_sha256: [u8; 32],
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
pub struct BuildObservationV2 {
    canonical_bytes: Vec<u8>,
    identity_sha256: [u8; 32],
    semantic_admission_sha256: [u8; 32],
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

impl BuildObservationV2 {
    pub(crate) fn from_fields(
        fields: BuildObservationFieldsV2<'_>,
    ) -> Result<Self, IdentityCodecErrorV2> {
        let encoded = encode_fields(&[
            ("schema", BUILD_SCHEMA_V2.to_owned()),
            (
                "semantic_admission_sha256",
                hex(&fields.semantic_admission_sha256),
            ),
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
        let fields = decode_fields(bytes, &BUILD_FIELDS_V2, "build observation")?;
        require_exact(fields[0], BUILD_SCHEMA_V2, "build schema")?;
        Ok(Self {
            canonical_bytes: bytes.to_vec(),
            identity_sha256: Sha256::digest(bytes).into(),
            semantic_admission_sha256: decode_digest(fields[1], "semantic_admission_sha256")?,
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

    pub const fn semantic_admission_sha256(&self) -> &[u8; 32] {
        &self.semantic_admission_sha256
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityHandoffV2 {
    canonical_bytes: Vec<u8>,
    semantic_admission: SemanticAdmissionV2,
    build_observation: BuildObservationV2,
}

impl IdentityHandoffV2 {
    pub(crate) fn new(
        semantic_admission: SemanticAdmissionV2,
        build_observation: BuildObservationV2,
    ) -> Result<Self, IdentityCodecErrorV2> {
        if build_observation.semantic_admission_sha256() != semantic_admission.identity_sha256() {
            return Err(IdentityCodecErrorV2::new(
                "build observation does not bind the semantic admission identity",
            ));
        }
        let semantic_len = u32::try_from(semantic_admission.canonical_bytes().len())
            .map_err(|_| IdentityCodecErrorV2::new("semantic admission record is too large"))?;
        let observation_len = u32::try_from(build_observation.canonical_bytes().len())
            .map_err(|_| IdentityCodecErrorV2::new("build observation record is too large"))?;
        let mut bytes = Vec::with_capacity(
            HANDOFF_DOMAIN_V2.len()
                + 8
                + semantic_admission.canonical_bytes().len()
                + build_observation.canonical_bytes().len(),
        );
        bytes.extend_from_slice(HANDOFF_DOMAIN_V2);
        bytes.extend_from_slice(&semantic_len.to_le_bytes());
        bytes.extend_from_slice(semantic_admission.canonical_bytes());
        bytes.extend_from_slice(&observation_len.to_le_bytes());
        bytes.extend_from_slice(build_observation.canonical_bytes());
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
        let semantic_bytes = take_record(bytes, &mut offset, "semantic admission")?;
        let observation_bytes = take_record(bytes, &mut offset, "build observation")?;
        if offset != bytes.len() {
            return Err(IdentityCodecErrorV2::new(
                "identity handoff has trailing bytes or records",
            ));
        }
        let semantic_admission = SemanticAdmissionV2::decode_record(semantic_bytes)?;
        let build_observation = BuildObservationV2::decode_record(observation_bytes)?;
        if build_observation.semantic_admission_sha256() != semantic_admission.identity_sha256() {
            return Err(IdentityCodecErrorV2::new(
                "build observation does not bind the semantic admission identity",
            ));
        }
        Ok(Self {
            canonical_bytes: bytes.to_vec(),
            semantic_admission,
            build_observation,
        })
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn semantic_admission(&self) -> &SemanticAdmissionV2 {
        &self.semantic_admission
    }

    pub const fn build_observation(&self) -> &BuildObservationV2 {
        &self.build_observation
    }
}

pub fn identity_section_v2(hsaco: &[u8]) -> Result<&[u8], IdentityCodecErrorV2> {
    if hsaco.is_empty() || hsaco.len() > MAX_HSACO_BYTES_V2 {
        return Err(IdentityCodecErrorV2::new(format!(
            "HSACO must contain 1 through {MAX_HSACO_BYTES_V2} bytes"
        )));
    }
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
        let name = elf_string(strings, name_offset)?;
        if name != S09_IDENTITY_SECTION_V2.as_bytes() {
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

pub fn decode_hsaco_identity_v2(hsaco: &[u8]) -> Result<IdentityHandoffV2, IdentityCodecErrorV2> {
    IdentityHandoffV2::decode(identity_section_v2(hsaco)?)
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

fn elf_string(strings: &[u8], offset: usize) -> Result<&[u8], IdentityCodecErrorV2> {
    if offset >= strings.len() {
        return Err(IdentityCodecErrorV2::new(
            "ELF section name offset is out of bounds",
        ));
    }
    let suffix = &strings[offset..];
    let end = suffix
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| IdentityCodecErrorV2::new("ELF section name is unterminated"))?;
    Ok(&suffix[..end])
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

    fn semantic() -> SemanticAdmissionV2 {
        SemanticAdmissionV2::from_fields(SemanticAdmissionFieldsV2 {
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
            code_object_version: 6,
            rustc_opt_level: 0,
            rustc_debug_info: "full",
            injected_debug_policy: "dwarf-v5-full",
            abi_sha256: [2; 32],
            launch_sha256: [3; 32],
            portable_mir_sha256: [4; 32],
        })
        .unwrap()
    }

    fn build(semantic: &SemanticAdmissionV2) -> BuildObservationV2 {
        BuildObservationV2::from_fields(BuildObservationFieldsV2 {
            semantic_admission_sha256: *semantic.identity_sha256(),
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

    fn handoff() -> IdentityHandoffV2 {
        let semantic = semantic();
        let build = build(&semantic);
        IdentityHandoffV2::new(semantic, build).unwrap()
    }

    fn replace_record(
        handoff: &IdentityHandoffV2,
        semantic_record: Option<&[u8]>,
        build_record: Option<&[u8]>,
    ) -> Vec<u8> {
        let semantic = semantic_record.unwrap_or(handoff.semantic_admission().canonical_bytes());
        let build = build_record.unwrap_or(handoff.build_observation().canonical_bytes());
        let mut bytes = Vec::new();
        bytes.extend_from_slice(HANDOFF_DOMAIN_V2);
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

    fn elf(identity_sections: &[&[u8]]) -> Vec<u8> {
        let mut strings = b"\0.shstrtab\0".to_vec();
        let identity_name = strings.len() as u32;
        strings.extend_from_slice(S09_IDENTITY_SECTION_V2.as_bytes());
        strings.push(0);
        let mut bytes = vec![0_u8; ELF64_HEADER_BYTES];
        bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        let string_offset = bytes.len();
        bytes.extend_from_slice(&strings);
        let mut identity_offsets = Vec::new();
        for identity in identity_sections {
            identity_offsets.push(bytes.len());
            bytes.extend_from_slice(identity);
        }
        while !bytes.len().is_multiple_of(8) {
            bytes.push(0);
        }
        let section_offset = bytes.len();
        let section_count = 2 + identity_sections.len();
        bytes.resize(
            section_offset + section_count * ELF64_SECTION_HEADER_BYTES,
            0,
        );
        bytes[40..48].copy_from_slice(&(section_offset as u64).to_le_bytes());
        bytes[52..54].copy_from_slice(&(ELF64_HEADER_BYTES as u16).to_le_bytes());
        bytes[58..60].copy_from_slice(&(ELF64_SECTION_HEADER_BYTES as u16).to_le_bytes());
        bytes[60..62].copy_from_slice(&(section_count as u16).to_le_bytes());
        bytes[62..64].copy_from_slice(&1_u16.to_le_bytes());
        let strings_header = section_offset + ELF64_SECTION_HEADER_BYTES;
        bytes[strings_header..strings_header + 4].copy_from_slice(&1_u32.to_le_bytes());
        bytes[strings_header + 4..strings_header + 8].copy_from_slice(&SHT_STRTAB.to_le_bytes());
        bytes[strings_header + 24..strings_header + 32]
            .copy_from_slice(&(string_offset as u64).to_le_bytes());
        bytes[strings_header + 32..strings_header + 40]
            .copy_from_slice(&(strings.len() as u64).to_le_bytes());
        for (index, (offset, identity)) in identity_offsets
            .iter()
            .zip(identity_sections.iter())
            .enumerate()
        {
            let header = section_offset + (index + 2) * ELF64_SECTION_HEADER_BYTES;
            bytes[header..header + 4].copy_from_slice(&identity_name.to_le_bytes());
            bytes[header + 4..header + 8].copy_from_slice(&SHT_PROGBITS.to_le_bytes());
            bytes[header + 24..header + 32].copy_from_slice(&(*offset as u64).to_le_bytes());
            bytes[header + 32..header + 40].copy_from_slice(&(identity.len() as u64).to_le_bytes());
        }
        bytes
    }

    #[test]
    fn canonical_records_and_handoff_round_trip_exactly() {
        let handoff = handoff();
        let decoded = IdentityHandoffV2::decode(handoff.canonical_bytes()).unwrap();
        assert_eq!(decoded, handoff);
        assert_eq!(decoded.semantic_admission().rustc_opt_level(), 0);
        assert_eq!(decoded.semantic_admission().source_bytes(), 3231);
        assert_eq!(
            decoded.build_observation().prepared_rustc_command_sha256(),
            &[9; 32]
        );
        assert_eq!(
            decoded.build_observation().cargo_fe2o3_executable_sha256(),
            &[11; 32]
        );
        assert_eq!(
            decoded
                .build_observation()
                .declared_cargo_executable_sha256(),
            &[12; 32]
        );
        assert_eq!(
            decoded
                .build_observation()
                .cargo_launcher_executable_sha256(),
            &[13; 32]
        );
        assert_eq!(decoded.build_observation().cargo_launcher_pid(), 14);
        assert_eq!(
            decoded
                .build_observation()
                .cargo_launcher_start_time_ticks(),
            15
        );
    }

    #[test]
    fn every_field_rejects_unknown_empty_and_missing_encodings() {
        let handoff = handoff();
        for index in 0..SEMANTIC_FIELDS_V2.len() {
            for mutation in [
                mutate_field_name(handoff.semantic_admission().canonical_bytes(), index),
                empty_field(handoff.semantic_admission().canonical_bytes(), index),
                remove_field(handoff.semantic_admission().canonical_bytes(), index),
            ] {
                assert!(
                    IdentityHandoffV2::decode(&replace_record(&handoff, Some(&mutation), None))
                        .is_err(),
                    "semantic field {index} mutation was accepted"
                );
            }
        }
        for index in 0..BUILD_FIELDS_V2.len() {
            for mutation in [
                mutate_field_name(handoff.build_observation().canonical_bytes(), index),
                empty_field(handoff.build_observation().canonical_bytes(), index),
                remove_field(handoff.build_observation().canonical_bytes(), index),
            ] {
                assert!(
                    IdentityHandoffV2::decode(&replace_record(&handoff, None, Some(&mutation)))
                        .is_err(),
                    "build field {index} mutation was accepted"
                );
            }
        }
    }

    #[test]
    fn rejects_duplicate_reordered_zero_oversize_and_noncanonical_fields() {
        let handoff = handoff();
        let semantic = handoff.semantic_admission().canonical_bytes();
        let mut duplicate = semantic.to_vec();
        let first_end = semantic.iter().position(|byte| *byte == b'\n').unwrap() + 1;
        duplicate.extend_from_slice(&semantic[..first_end]);
        assert!(
            IdentityHandoffV2::decode(&replace_record(&handoff, Some(&duplicate), None)).is_err()
        );

        let mut lines = record_lines(semantic);
        lines.swap(1, 2);
        assert!(
            IdentityHandoffV2::decode(&replace_record(&handoff, Some(&lines.concat()), None))
                .is_err()
        );

        let zero_digest = replace_first(
            handoff.build_observation().canonical_bytes(),
            b"prepared_rustc_command_sha256\t0909090909090909090909090909090909090909090909090909090909090909\n",
            b"prepared_rustc_command_sha256\t0000000000000000000000000000000000000000000000000000000000000000\n",
        );
        assert!(
            IdentityHandoffV2::decode(&replace_record(&handoff, None, Some(&zero_digest))).is_err()
        );

        let mut oversize = b"crate\t".to_vec();
        oversize.extend(std::iter::repeat_n(b'x', MAX_FIELD_VALUE_BYTES_V2 + 1));
        oversize.push(b'\n');
        let mut lines = record_lines(semantic);
        lines[1] = oversize;
        assert!(
            IdentityHandoffV2::decode(&replace_record(&handoff, Some(&lines.concat()), None))
                .is_err()
        );

        let noncanonical_decimal =
            replace_first(semantic, b"source_bytes\t3231\n", b"source_bytes\t03231\n");
        assert!(
            IdentityHandoffV2::decode(&replace_record(&handoff, Some(&noncanonical_decimal), None))
                .is_err()
        );
    }

    #[test]
    fn rejects_zero_lengths_trailing_bytes_and_every_truncation() {
        let handoff = handoff();
        let bytes = handoff.canonical_bytes();
        let mut zero = bytes.to_vec();
        zero[HANDOFF_DOMAIN_V2.len()..HANDOFF_DOMAIN_V2.len() + 4]
            .copy_from_slice(&0_u32.to_le_bytes());
        assert!(IdentityHandoffV2::decode(&zero).is_err());

        let mut trailing = bytes.to_vec();
        trailing.push(0);
        assert!(IdentityHandoffV2::decode(&trailing).is_err());

        for length in 0..bytes.len() {
            assert!(
                IdentityHandoffV2::decode(&bytes[..length]).is_err(),
                "truncation at {length} bytes was accepted"
            );
        }
    }

    #[test]
    fn elf_requires_exactly_one_bounded_identity_section() {
        let handoff = handoff();
        let one = elf(&[handoff.canonical_bytes()]);
        assert_eq!(
            identity_section_v2(&one).unwrap(),
            handoff.canonical_bytes()
        );
        assert_eq!(decode_hsaco_identity_v2(&one).unwrap(), handoff);
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
}
