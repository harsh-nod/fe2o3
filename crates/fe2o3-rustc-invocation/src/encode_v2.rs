use crate::ValidationError;
use crate::model_v2::{MAX_DESCRIPTOR_BYTES_V2, RustcInvocationDescriptorV2};

/// Fixed magic at the start of every V2 invocation descriptor.
pub const INVOCATION_DESCRIPTOR_MAGIC_V2: [u8; 8] = *b"FE2O3RI\0";
/// The only V2 descriptor version implemented by this crate.
pub const INVOCATION_DESCRIPTOR_VERSION_V2: u16 = 2;
pub(crate) const HEADER_BYTES: usize = 20;

/// Encodes a validated invocation descriptor in the canonical V2 format.
pub fn encode_descriptor_v2(
    descriptor: &RustcInvocationDescriptorV2,
) -> Result<Vec<u8>, ValidationError> {
    let mut writer = Writer::new();
    writer.bytes(&INVOCATION_DESCRIPTOR_MAGIC_V2);
    writer.u16(INVOCATION_DESCRIPTOR_VERSION_V2);
    writer.u16(0);
    writer.u32(0);
    writer.u32(0);
    debug_assert_eq!(writer.bytes.len(), HEADER_BYTES);

    writer.bytes(&descriptor.rustc_executable_sha256);
    writer.bytes(&descriptor.codegen_backend_sha256);
    writer.path(descriptor.rustc.working_directory.as_str());
    writer.u32(descriptor.rustc.argv.len() as u32);
    for argument in &descriptor.rustc.argv {
        writer.argument(argument.as_str());
    }

    writer.u16(descriptor.compile_environment.entries.len() as u16);
    writer.u16(0);
    for entry in &descriptor.compile_environment.entries {
        writer.name(entry.key.as_str());
        writer.environment_value(entry.value.as_str());
    }

    if writer.failed {
        return Err(ValidationError::EncodedDescriptorTooLarge {
            max: MAX_DESCRIPTOR_BYTES_V2,
        });
    }
    let total_len = u32::try_from(writer.bytes.len()).map_err(|_| ValidationError::Overflow {
        field: "invocation descriptor length",
    })?;
    writer.bytes[12..16].copy_from_slice(&total_len.to_le_bytes());
    Ok(writer.bytes)
}

pub(crate) fn validate_encoded_size(
    descriptor: &RustcInvocationDescriptorV2,
) -> Result<(), ValidationError> {
    encode_descriptor_v2(descriptor).map(|_| ())
}

struct Writer {
    bytes: Vec<u8>,
    failed: bool,
}

impl Writer {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            failed: false,
        }
    }

    fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    fn bytes(&mut self, value: &[u8]) {
        if self.failed {
            return;
        }
        let Some(length) = self.bytes.len().checked_add(value.len()) else {
            self.failed = true;
            return;
        };
        if length > MAX_DESCRIPTOR_BYTES_V2 {
            self.failed = true;
            return;
        }
        self.bytes.extend_from_slice(value);
    }

    fn name(&mut self, value: &str) {
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
