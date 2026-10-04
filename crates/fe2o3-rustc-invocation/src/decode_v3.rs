use fe2o3_build_authority::{CompilerClosureV2, derive_compiler_closure_identity_v2};

use crate::encode_v3::{
    COMPILER_CLOSURE_PREIMAGE_BYTES_V3, HEADER_BYTES_V3, INVOCATION_DESCRIPTOR_MAGIC_V3,
    INVOCATION_DESCRIPTOR_VERSION_V3,
};
use crate::{
    DecodeError, MAX_DESCRIPTOR_BYTES_V3, RustcInvocationDescriptorV3, decode_descriptor_v2,
    encode_descriptor_v3,
};

/// Decodes and validates canonical V3 descriptor bytes.
///
/// Length bounds are checked before allocation. The embedded process body is
/// decoded by the frozen V2 decoder, the closure is reconstructed from its
/// canonical identity preimage, and successful decoding requires exact
/// byte-for-byte re-encoding.
pub fn decode_descriptor_v3(bytes: &[u8]) -> Result<RustcInvocationDescriptorV3, DecodeError> {
    if bytes.len() > MAX_DESCRIPTOR_BYTES_V3 {
        return Err(DecodeError::TooLarge {
            max: MAX_DESCRIPTOR_BYTES_V3,
        });
    }

    let mut reader = Reader::new(bytes);
    if reader.fixed::<8>()? != INVOCATION_DESCRIPTOR_MAGIC_V3 {
        return Err(DecodeError::InvalidMagic);
    }
    let version = reader.u16()?;
    if version != INVOCATION_DESCRIPTOR_VERSION_V3 {
        return Err(DecodeError::UnknownVersion(version));
    }
    let flags = reader.u16()?;
    if flags != 0 {
        return Err(DecodeError::UnsupportedFlags(flags));
    }
    let declared_len = reader.u32()?;
    let minimum_len = HEADER_BYTES_V3 + COMPILER_CLOSURE_PREIMAGE_BYTES_V3;
    if declared_len < minimum_len as u32 {
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

    let transition_protocol_version = reader.u16()?;
    let cargo_executable_sha256 = reader.fixed::<32>()?;
    let cargo_binding_trampoline_sha256 = reader.fixed::<32>()?;
    let cargo_fe2o3_binding_wrapper_sha256 = reader.fixed::<32>()?;
    let rustc_executable_sha256 = reader.fixed::<32>()?;
    let rustc_runtime_tree_sha256 = reader.fixed::<32>()?;
    let codegen_backend_sha256 = reader.fixed::<32>()?;
    let identity_sha256 = derive_compiler_closure_identity_v2(
        cargo_executable_sha256,
        cargo_binding_trampoline_sha256,
        cargo_fe2o3_binding_wrapper_sha256,
        rustc_executable_sha256,
        rustc_runtime_tree_sha256,
        codegen_backend_sha256,
        transition_protocol_version,
    );
    let compiler_closure = CompilerClosureV2::from_pins_and_identity(
        cargo_executable_sha256,
        cargo_binding_trampoline_sha256,
        cargo_fe2o3_binding_wrapper_sha256,
        rustc_executable_sha256,
        rustc_runtime_tree_sha256,
        codegen_backend_sha256,
        transition_protocol_version,
        identity_sha256,
    )?;

    let v2_len = declared_len_usize - COMPILER_CLOSURE_PREIMAGE_BYTES_V3;
    let v2_len_u32 = u32::try_from(v2_len).map_err(|_| DecodeError::InvalidLength {
        declared: declared_len,
    })?;
    let mut encoded_v2 = Vec::with_capacity(v2_len);
    encoded_v2.extend_from_slice(&crate::INVOCATION_DESCRIPTOR_MAGIC_V2);
    encoded_v2.extend_from_slice(&crate::INVOCATION_DESCRIPTOR_VERSION_V2.to_le_bytes());
    encoded_v2.extend_from_slice(&0_u16.to_le_bytes());
    encoded_v2.extend_from_slice(&v2_len_u32.to_le_bytes());
    encoded_v2.extend_from_slice(&0_u32.to_le_bytes());
    encoded_v2.extend_from_slice(&bytes[minimum_len..]);
    debug_assert_eq!(encoded_v2.len(), v2_len);

    let descriptor_v2 = decode_descriptor_v2(&encoded_v2)?;
    let descriptor =
        RustcInvocationDescriptorV3::from_v2_and_compiler_closure(descriptor_v2, compiler_closure)?;
    if encode_descriptor_v3(&descriptor)? != bytes {
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

    fn reserved_u32(&mut self, field: &'static str) -> Result<(), DecodeError> {
        if self.u32()? != 0 {
            return Err(DecodeError::NonzeroReserved { field });
        }
        Ok(())
    }
}
