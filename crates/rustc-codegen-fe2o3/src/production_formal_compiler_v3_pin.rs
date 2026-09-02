//! Reviewed source pin for the trusted Formal Compiler V3 Rust validator.

pub(crate) const FORMAL_COMPILER_V3_RUST_VALIDATOR_SHA256: [u8; 32] = [
    0x56, 0x50, 0x5a, 0x35, 0xf3, 0xe1, 0x0d, 0x7a, 0x98, 0x85, 0x9e, 0x01, 0x69, 0xe4, 0xdd, 0x38,
    0xeb, 0x03, 0x4a, 0x4c, 0xb8, 0x46, 0x8e, 0x5b, 0xe3, 0xdf, 0x56, 0xae, 0xc1, 0x5d, 0xee, 0xaa,
];

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    use super::*;

    #[test]
    fn rust_validator_source_matches_reviewed_pin() {
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(include_bytes!(
                "production_formal_compiler_v3.rs"
            ))),
            FORMAL_COMPILER_V3_RUST_VALIDATOR_SHA256
        );
    }
}
