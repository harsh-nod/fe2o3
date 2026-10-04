use crate::{
    MAX_DESCRIPTOR_BYTES_V3, RustcInvocationDescriptorV3, ValidationError, encode_descriptor_v2,
};

/// Fixed magic at the start of every V3 invocation descriptor.
pub const INVOCATION_DESCRIPTOR_MAGIC_V3: [u8; 8] = *b"FE2O3RI\0";
/// The only V3 descriptor version implemented by this crate.
pub const INVOCATION_DESCRIPTOR_VERSION_V3: u16 = 3;
pub(crate) const HEADER_BYTES_V3: usize = 20;
pub(crate) const COMPILER_CLOSURE_PREIMAGE_BYTES_V3: usize = 2 + 6 * 32;

/// Encodes a validated invocation descriptor in the canonical V3 format.
pub fn encode_descriptor_v3(
    descriptor: &RustcInvocationDescriptorV3,
) -> Result<Vec<u8>, ValidationError> {
    descriptor.validate_compiler_closure_pins()?;
    let encoded_v2 = encode_descriptor_v2(&descriptor.descriptor_v2)?;
    let total_len = encoded_v2
        .len()
        .checked_add(COMPILER_CLOSURE_PREIMAGE_BYTES_V3)
        .ok_or(ValidationError::Overflow {
            field: "invocation descriptor length",
        })?;
    if total_len > MAX_DESCRIPTOR_BYTES_V3 {
        return Err(ValidationError::EncodedDescriptorTooLarge {
            max: MAX_DESCRIPTOR_BYTES_V3,
        });
    }
    let total_len_u32 = u32::try_from(total_len).map_err(|_| ValidationError::Overflow {
        field: "invocation descriptor length",
    })?;

    let mut encoded = Vec::with_capacity(total_len);
    encoded.extend_from_slice(&INVOCATION_DESCRIPTOR_MAGIC_V3);
    encoded.extend_from_slice(&INVOCATION_DESCRIPTOR_VERSION_V3.to_le_bytes());
    encoded.extend_from_slice(&0_u16.to_le_bytes());
    encoded.extend_from_slice(&total_len_u32.to_le_bytes());
    encoded.extend_from_slice(&0_u32.to_le_bytes());

    let closure = descriptor.compiler_closure;
    encoded.extend_from_slice(
        &closure
            .cargo_binding_transition_protocol_version()
            .to_le_bytes(),
    );
    encoded.extend_from_slice(&closure.cargo_executable_sha256());
    encoded.extend_from_slice(&closure.cargo_binding_trampoline_sha256());
    encoded.extend_from_slice(&closure.cargo_fe2o3_binding_wrapper_sha256());
    encoded.extend_from_slice(&closure.rustc_executable_sha256());
    encoded.extend_from_slice(&closure.rustc_runtime_tree_sha256());
    encoded.extend_from_slice(&closure.codegen_backend_sha256());
    encoded.extend_from_slice(&encoded_v2[crate::encode_v2::HEADER_BYTES..]);
    debug_assert_eq!(encoded.len(), total_len);
    Ok(encoded)
}
