use std::str;

use crate::DecodeError;
use crate::encode_v2::{
    HEADER_BYTES, INVOCATION_DESCRIPTOR_MAGIC_V2, INVOCATION_DESCRIPTOR_VERSION_V2,
    encode_descriptor_v2,
};
use crate::model_v2::{
    CompileEnvironmentEntryV2, CompileEnvironmentV2, MAX_ARGUMENT_BYTES_V2,
    MAX_COMPILE_ENVIRONMENT_ENTRIES_V2, MAX_DESCRIPTOR_BYTES_V2, MAX_ENVIRONMENT_VALUE_BYTES_V2,
    MAX_NAME_BYTES_V2, MAX_PATH_BYTES_V2, MAX_RUSTC_ARGUMENTS_V2, RustcInvocationDescriptorV2,
    RustcUnitV2,
};

/// Decodes and validates canonical V2 descriptor bytes.
///
/// Length and count bounds are checked before allocation. Successful decoding
/// includes an exact re-encoding check.
pub fn decode_descriptor_v2(bytes: &[u8]) -> Result<RustcInvocationDescriptorV2, DecodeError> {
    if bytes.len() > MAX_DESCRIPTOR_BYTES_V2 {
        return Err(DecodeError::TooLarge {
            max: MAX_DESCRIPTOR_BYTES_V2,
        });
    }

    let mut reader = Reader::new(bytes);
    if reader.fixed::<8>()? != INVOCATION_DESCRIPTOR_MAGIC_V2 {
        return Err(DecodeError::InvalidMagic);
    }
    let version = reader.u16()?;
    if version != INVOCATION_DESCRIPTOR_VERSION_V2 {
        return Err(DecodeError::UnknownVersion(version));
    }
    let flags = reader.u16()?;
    if flags != 0 {
        return Err(DecodeError::UnsupportedFlags(flags));
    }
    let declared_len = reader.u32()?;
    if declared_len < HEADER_BYTES as u32 {
        return Err(DecodeError::InvalidLength {
            declared: declared_len,
        });
    }
    let declared_len_usize =
        usize::try_from(declared_len).map_err(|_| DecodeError::InvalidLength {
            declared: declared_len,
        })?;
    if declared_len_usize > bytes.len() {
        return Err(DecodeError::Truncated);
    }
    if declared_len_usize < bytes.len() {
        return Err(DecodeError::TrailingBytes);
    }
    reader.reserved_u32("descriptor header")?;

    let rustc_executable_sha256 = reader.fixed::<32>()?;
    let codegen_backend_sha256 = reader.fixed::<32>()?;
    let working_directory = reader.path("rustc working directory")?;
    let argument_count = reader.count_u32("rustc arguments", MAX_RUSTC_ARGUMENTS_V2)?;
    let mut argv = Vec::with_capacity(argument_count);
    for _ in 0..argument_count {
        argv.push(reader.argument()?);
    }
    let rustc = RustcUnitV2::new(working_directory, argv)?;

    let environment_count =
        reader.count_u16("compile environment", MAX_COMPILE_ENVIRONMENT_ENTRIES_V2)?;
    reader.reserved_u16("compile environment")?;
    let mut compile_environment = Vec::with_capacity(environment_count);
    for _ in 0..environment_count {
        compile_environment.push(CompileEnvironmentEntryV2::new(
            reader.name("compile environment key")?,
            reader.environment_value()?,
        )?);
    }
    let compile_environment = CompileEnvironmentV2::from_encoded_entries(compile_environment)?;

    if !reader.is_finished() {
        return Err(DecodeError::TrailingBytes);
    }
    let descriptor = RustcInvocationDescriptorV2::new(
        rustc_executable_sha256,
        codegen_backend_sha256,
        rustc,
        compile_environment,
    )?;
    if encode_descriptor_v2(&descriptor)? != bytes {
        return Err(DecodeError::NonCanonical);
    }
    Ok(descriptor)
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], DecodeError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(DecodeError::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(DecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        self.take(N)?.try_into().map_err(|_| DecodeError::Truncated)
    }

    fn u16(&mut self) -> Result<u16, DecodeError> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn reserved_u16(&mut self, field: &'static str) -> Result<(), DecodeError> {
        if self.u16()? != 0 {
            return Err(DecodeError::NonzeroReserved { field });
        }
        Ok(())
    }

    fn reserved_u32(&mut self, field: &'static str) -> Result<(), DecodeError> {
        if self.u32()? != 0 {
            return Err(DecodeError::NonzeroReserved { field });
        }
        Ok(())
    }

    fn count_u16(&mut self, field: &'static str, max: usize) -> Result<usize, DecodeError> {
        let count = usize::from(self.u16()?);
        if count > max {
            return Err(DecodeError::CountOutOfRange {
                field,
                count: count as u64,
                max,
            });
        }
        Ok(count)
    }

    fn count_u32(&mut self, field: &'static str, max: usize) -> Result<usize, DecodeError> {
        let raw = self.u32()?;
        let count = usize::try_from(raw).map_err(|_| DecodeError::CountOutOfRange {
            field,
            count: u64::from(raw),
            max,
        })?;
        if count > max {
            return Err(DecodeError::CountOutOfRange {
                field,
                count: u64::from(raw),
                max,
            });
        }
        Ok(count)
    }

    fn name(&mut self, field: &'static str) -> Result<String, DecodeError> {
        let length = usize::from(self.u16()?);
        self.string(length, MAX_NAME_BYTES_V2, field)
    }

    fn path(&mut self, field: &'static str) -> Result<String, DecodeError> {
        let raw = self.u32()?;
        let length = usize::try_from(raw).map_err(|_| DecodeError::CountOutOfRange {
            field,
            count: u64::from(raw),
            max: MAX_PATH_BYTES_V2,
        })?;
        self.string(length, MAX_PATH_BYTES_V2, field)
    }

    fn argument(&mut self) -> Result<String, DecodeError> {
        let raw = self.u32()?;
        let length = usize::try_from(raw).map_err(|_| DecodeError::CountOutOfRange {
            field: "rustc argument bytes",
            count: u64::from(raw),
            max: MAX_ARGUMENT_BYTES_V2,
        })?;
        self.string(length, MAX_ARGUMENT_BYTES_V2, "rustc argument")
    }

    fn environment_value(&mut self) -> Result<String, DecodeError> {
        let raw = self.u32()?;
        let length = usize::try_from(raw).map_err(|_| DecodeError::CountOutOfRange {
            field: "compile environment value bytes",
            count: u64::from(raw),
            max: MAX_ENVIRONMENT_VALUE_BYTES_V2,
        })?;
        self.string(
            length,
            MAX_ENVIRONMENT_VALUE_BYTES_V2,
            "compile environment value",
        )
    }

    fn string(
        &mut self,
        length: usize,
        max: usize,
        field: &'static str,
    ) -> Result<String, DecodeError> {
        if length > max {
            return Err(DecodeError::CountOutOfRange {
                field,
                count: length as u64,
                max,
            });
        }
        let bytes = self.take(length)?;
        let value = str::from_utf8(bytes).map_err(|_| DecodeError::InvalidUtf8 { field })?;
        Ok(value.to_owned())
    }

    fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}
