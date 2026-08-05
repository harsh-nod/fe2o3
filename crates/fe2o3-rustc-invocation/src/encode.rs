use crate::ValidationError;
use crate::model::{
    BackendToolsV1, CargoTargetKindV1, CrateTypeV1, EditionV1, MAX_DESCRIPTOR_BYTES,
    RustcInvocationDescriptorV1, TestStateV1, ToolIdentityV1, VerificationModeV1,
};

/// Fixed magic at the start of every V1 invocation descriptor.
pub const INVOCATION_DESCRIPTOR_MAGIC: [u8; 8] = *b"FE2O3RI\0";
/// The only descriptor version implemented by this crate.
pub const INVOCATION_DESCRIPTOR_VERSION: u16 = 1;
pub(crate) const HEADER_BYTES: usize = 20;

/// Encodes a validated invocation descriptor in the canonical V1 format.
pub fn encode_descriptor_v1(
    descriptor: &RustcInvocationDescriptorV1,
) -> Result<Vec<u8>, ValidationError> {
    let mut writer = Writer::new();
    writer.bytes(&INVOCATION_DESCRIPTOR_MAGIC);
    writer.u16(INVOCATION_DESCRIPTOR_VERSION);
    writer.u16(0);
    writer.u32(0);
    writer.u32(0);
    debug_assert_eq!(writer.bytes.len(), HEADER_BYTES);

    encode_tool(&mut writer, &descriptor.cargo.executable);
    writer.name(descriptor.cargo.package.name.as_str());
    writer.text(descriptor.cargo.package.version.as_str());
    writer.path(descriptor.cargo.package.manifest_path.as_str());

    writer.name(descriptor.cargo.target.name.as_str());
    writer.u16(cargo_target_kind_tag(descriptor.cargo.target.kind));
    writer.u16(edition_tag(descriptor.cargo.target.edition));
    writer.u16(0);
    writer.u16(descriptor.cargo.target.crate_types.len() as u16);
    writer.u16(descriptor.cargo.target.features.len() as u16);
    writer.u32(0);
    writer.path(descriptor.cargo.target.source_path.as_str());
    for crate_type in &descriptor.cargo.target.crate_types {
        writer.u16(crate_type_tag(*crate_type));
    }
    for feature in &descriptor.cargo.target.features {
        writer.name(feature.as_str());
    }

    encode_tool(&mut writer, &descriptor.rustc.executable);
    writer.name(descriptor.rustc.unit.crate_name.as_str());
    writer.name(descriptor.rustc.unit.host_target.as_str());
    writer.name(descriptor.rustc.unit.effective_target.as_str());
    writer.u8(test_state_tag(descriptor.rustc.unit.test_state));
    writer.u8(0);
    writer.u16(0);
    writer.u32(descriptor.rustc.unit.argv.len() as u32);
    for argument in &descriptor.rustc.unit.argv {
        writer.argument(argument.as_str());
    }

    encode_backend_tools(&mut writer, &descriptor.tools);

    writer.text(descriptor.device.amd_target.as_str());
    writer.u8(verification_mode_tag(descriptor.device.verification));
    writer.u8(0);
    writer.u16(0);
    writer.path(descriptor.output.workspace_root.as_str());
    writer.path(descriptor.output.artifact_output_directory.as_str());

    writer.u16(descriptor.compile_environment.len() as u16);
    writer.u16(0);
    for entry in &descriptor.compile_environment {
        writer.name(entry.key.as_str());
        writer.environment_value(entry.value.as_str());
    }

    if writer.bytes.len() > MAX_DESCRIPTOR_BYTES {
        return Err(ValidationError::EncodedDescriptorTooLarge {
            max: MAX_DESCRIPTOR_BYTES,
        });
    }
    let total_len = u32::try_from(writer.bytes.len()).map_err(|_| ValidationError::Overflow {
        field: "invocation descriptor length",
    })?;
    writer.bytes[12..16].copy_from_slice(&total_len.to_le_bytes());
    Ok(writer.bytes)
}

pub(crate) fn validate_encoded_size(
    descriptor: &RustcInvocationDescriptorV1,
) -> Result<(), ValidationError> {
    encode_descriptor_v1(descriptor).map(|_| ())
}

fn encode_tool(writer: &mut Writer, tool: &ToolIdentityV1) {
    writer.text(tool.version.as_str());
    writer.bytes(&tool.executable_sha256);
}

fn encode_backend_tools(writer: &mut Writer, tools: &BackendToolsV1) {
    encode_tool(writer, &tools.backend);
    encode_tool(writer, &tools.clang);
    encode_tool(writer, &tools.linker);
    writer.u8(u8::from(tools.inspector.is_some()));
    writer.u8(0);
    writer.u16(0);
    if let Some(inspector) = &tools.inspector {
        encode_tool(writer, inspector);
    }
}

pub(crate) const fn cargo_target_kind_tag(value: CargoTargetKindV1) -> u16 {
    match value {
        CargoTargetKindV1::Library => 1,
        CargoTargetKindV1::Binary => 2,
        CargoTargetKindV1::Example => 3,
        CargoTargetKindV1::Test => 4,
        CargoTargetKindV1::Benchmark => 5,
        CargoTargetKindV1::BuildScript => 6,
        CargoTargetKindV1::ProcMacro => 7,
    }
}

pub(crate) const fn crate_type_tag(value: CrateTypeV1) -> u16 {
    match value {
        CrateTypeV1::Lib => 1,
        CrateTypeV1::Rlib => 2,
        CrateTypeV1::Dylib => 3,
        CrateTypeV1::Cdylib => 4,
        CrateTypeV1::Staticlib => 5,
        CrateTypeV1::ProcMacro => 6,
        CrateTypeV1::Bin => 7,
    }
}

pub(crate) const fn edition_tag(value: EditionV1) -> u16 {
    match value {
        EditionV1::Rust2015 => 1,
        EditionV1::Rust2018 => 2,
        EditionV1::Rust2021 => 3,
        EditionV1::Rust2024 => 4,
    }
}

pub(crate) const fn test_state_tag(value: TestStateV1) -> u8 {
    match value {
        TestStateV1::NotTest => 1,
        TestStateV1::Test => 2,
    }
}

pub(crate) const fn verification_mode_tag(value: VerificationModeV1) -> u8 {
    match value {
        VerificationModeV1::Disabled => 1,
        VerificationModeV1::Required => 2,
    }
}

struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn bytes(&mut self, value: &[u8]) {
        self.bytes.extend_from_slice(value);
    }

    fn name(&mut self, value: &str) {
        self.u16(value.len() as u16);
        self.bytes(value.as_bytes());
    }

    fn text(&mut self, value: &str) {
        self.u16(value.len() as u16);
        self.bytes(value.as_bytes());
    }

    fn path(&mut self, value: &str) {
        self.u32(value.len() as u32);
        self.bytes(value.as_bytes());
    }

    fn argument(&mut self, value: &str) {
        self.u32(value.len() as u32);
        self.bytes(value.as_bytes());
    }

    fn environment_value(&mut self, value: &str) {
        self.u32(value.len() as u32);
        self.bytes(value.as_bytes());
    }
}
