//! Inert selection of an explicitly supported nominal policy decoder.
use crate::attestation::{
    CompilerExecutionAttestationErrorV1 as Error, Reader, decode_header_version, require_length,
};
use crate::issuer_policy_codec::BYTES;

/// A header classification, never a decoded policy or evidence of authority.
/// The selected nominal decoder must still validate the complete original image.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerExecutionPolicyFamilyV1 {
    LegacyV1,
    NativeV3,
}

impl CompilerExecutionPolicyFamilyV1 {
    /// Checks exact magic, version, flags, declared length and header reserved
    /// bytes. Equal record lengths do not select a family; V2 is not supported.
    /// The bounded fixed-header work must be included in the caller's I/O charge.
    pub fn inspect_header(bytes: &[u8]) -> Result<Self, Error> {
        require_length(bytes, BYTES, "issuer policy")?;
        let (family, magic, version) = match &bytes[..8] {
            b"F2O3CEP1" => (Self::LegacyV1, *b"F2O3CEP1", 1),
            b"F2O3CEP3" => (Self::NativeV3, *b"F2O3CEP3", 3),
            _ => return Err(Error::InvalidMagic("supported issuer policy family")),
        };
        decode_header_version(
            &mut Reader::new(bytes),
            magic,
            version,
            BYTES,
            "issuer policy",
        )?;
        Ok(family)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attestation::encode_header_version;

    #[test]
    fn equal_lengths_select_only_exact_supported_headers() {
        for (magic, version, family) in [
            (*b"F2O3CEP1", 1, CompilerExecutionPolicyFamilyV1::LegacyV1),
            (*b"F2O3CEP3", 3, CompilerExecutionPolicyFamilyV1::NativeV3),
        ] {
            let mut bytes = [0; BYTES];
            encode_header_version(&mut bytes, magic, version);
            assert_eq!(
                CompilerExecutionPolicyFamilyV1::inspect_header(&bytes).unwrap(),
                family
            );
            for at in 0..24 {
                let mut altered = bytes;
                altered[at] ^= 0x80;
                assert!(CompilerExecutionPolicyFamilyV1::inspect_header(&altered).is_err());
            }
            assert!(CompilerExecutionPolicyFamilyV1::inspect_header(&bytes[..BYTES - 1]).is_err());
            assert!(CompilerExecutionPolicyFamilyV1::inspect_header(&[0; BYTES + 1]).is_err());
        }
        let mut unsupported = [0; BYTES];
        encode_header_version(&mut unsupported, *b"F2O3CEP2", 2);
        assert!(CompilerExecutionPolicyFamilyV1::inspect_header(&unsupported).is_err());
    }
}
