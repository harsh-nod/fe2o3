//! Synthetic codec fixtures only: no protected compiler or Verus execution.
#![allow(dead_code)]

use ed25519_dalek::{Signer as _, SigningKey};
use fe2o3_functional_proof::{
    FunctionalRefinementBindingV2, FunctionalRefinementBoundaryV2,
    FunctionalRefinementImportPolicyV2, FunctionalRefinementResultV2,
    FunctionalRefinementSubjectsV2, SafeReferenceKindV2, UnsignedFunctionalRefinementReceiptV2,
    VerusToolchainIdentityV2,
};
use fe2o3_proof_contracts::DigestV1;
use sha2::{Digest as _, Sha256};

pub const DOMAIN: &[u8] = b"FE2O3/CONDITIONAL-GUARDED-OUTPUT-VERUS/V1\0";
pub const NAME: &[u8] = b"output";
pub const LOCATIONS: usize = DOMAIN.len() + 128 + 8 + NAME.len();
pub const FIELDS: usize = LOCATIONS + 64;

pub fn digest(value: u8) -> DigestV1 {
    DigestV1::from_untrusted_bytes([value; 32])
}

pub fn subjects() -> FunctionalRefinementSubjectsV2 {
    FunctionalRefinementSubjectsV2::new(
        SafeReferenceKindV2::Mir,
        digest(10),
        DigestV1::ZERO,
        digest(12),
        digest(13),
        digest(14),
    )
    .unwrap()
}

pub fn preimage(identities: [DigestV1; 4]) -> Vec<u8> {
    let mut bytes = DOMAIN.to_vec();
    for identity in identities {
        bytes.extend_from_slice(identity.as_bytes());
    }
    bytes.extend_from_slice(&(NAME.len() as u64).to_le_bytes());
    bytes.extend_from_slice(NAME);
    for value in [0_u64, 1, 0, 2, 0, 3, 1, 0] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    // Ranked ordinal 2 deliberately differs from source output ordinal 0.
    for value in [2_u64, 0, 1, 1, 32, 0, 0, 64, 1, 1, 64, 0, 1, 1, 1] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for index in 0..14 {
        bytes.extend_from_slice(
            if index == 1 {
                DigestV1::ZERO
            } else {
                digest(10 + index)
            }
            .as_bytes(),
        );
    }
    bytes
}

pub fn signed(preimage: &[u8]) -> Vec<u8> {
    signed_with(
        preimage,
        FunctionalRefinementBoundaryV2::SafeReferenceMirToLivePlironConditionalCoverage,
        FunctionalRefinementResultV2::Proved,
    )
}

pub fn signed_with(
    preimage: &[u8],
    boundary: FunctionalRefinementBoundaryV2,
    result: FunctionalRefinementResultV2,
) -> Vec<u8> {
    let binding = FunctionalRefinementBindingV2::from_subjects(
        subjects(),
        DigestV1::from_untrusted_bytes(Sha256::digest(preimage).into()),
    )
    .unwrap();
    let toolchain =
        VerusToolchainIdentityV2::new(digest(40), digest(41), digest(42), digest(43), digest(44))
            .unwrap();
    let signing = SigningKey::from_bytes(&[42; 32]);
    let key = signing.verifying_key().to_bytes();
    let policy = FunctionalRefinementImportPolicyV2::new(key, toolchain, boundary).unwrap();
    let unsigned = UnsignedFunctionalRefinementReceiptV2::from_verified_execution_join(
        policy.signer_identity(),
        binding,
        toolchain,
        digest(50),
        result,
        boundary,
    )
    .unwrap();
    let signature = signing.sign(unsigned.signing_bytes()).to_bytes();
    let wire = unsigned.attach_signature(signature);
    let mut bytes = b"F2CGVEV1".to_vec();
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&((20 + preimage.len() + 32 + wire.len() + 32) as u32).to_le_bytes());
    bytes.extend_from_slice(&(preimage.len() as u32).to_le_bytes());
    bytes.extend_from_slice(preimage);
    bytes.extend_from_slice(&key);
    bytes.extend_from_slice(&wire);
    bytes.extend_from_slice(&[0; 32]);
    repair_identity(&mut bytes);
    bytes
}

pub fn repair_identity(bytes: &mut [u8]) {
    let terminal = bytes.len() - 32;
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/CONDITIONAL-OUTPUT-EXECUTION-EVIDENCE/V1\0");
    hash.update((terminal as u64).to_le_bytes());
    hash.update(&bytes[..terminal]);
    bytes[terminal..].copy_from_slice(&hash.finalize());
}
