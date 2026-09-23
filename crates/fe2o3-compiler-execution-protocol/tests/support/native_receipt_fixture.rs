use super::fixture::{challenge_wire, header, policy_wire, request_wire, seal};
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};

pub fn signature_message(bytes: &[u8], version: u16) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(format!(
        "FE2O3/COMPILER-EXECUTION-RECEIPT-SIGNATURE/V{version}\0"
    ));
    hash.update(304u64.to_le_bytes());
    hash.update(&bytes[..304]);
    hash.finalize().into()
}
pub fn anchor(bytes: &[u8], version: u16) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(format!(
        "FE2O3/COMPILER-EXECUTION-ROLLBACK-ANCHOR/V{version}\0"
    ));
    for range in [
        200..208,
        208..240,
        24..56,
        96..128,
        128..136,
        168..200,
        64..96,
    ] {
        hash.update(&bytes[range]);
    }
    hash.finalize().into()
}
pub fn sign(bytes: &mut [u8], version: u16, key: &SigningKey) {
    let signature = key.sign(&signature_message(bytes, version)).to_bytes();
    bytes[304..368].copy_from_slice(&signature);
    seal(bytes, "COMPILER-EXECUTION-RECEIPT", version);
}
pub fn rebuild(bytes: &mut [u8], version: u16) {
    let next = anchor(bytes, version);
    bytes[240..272].copy_from_slice(&next);
    sign(bytes, version, &SigningKey::from_bytes(&[0x51; 32]));
}
pub fn receipt_wire(version: u16) -> [u8; 400] {
    let mut bytes = [0; 400];
    header(
        &mut bytes,
        if version == 1 {
            b"F2O3CER1"
        } else {
            b"F2O3CER2"
        },
        version,
    );
    bytes[24..56].copy_from_slice(&request_wire(version)[914..]);
    bytes[56..64].copy_from_slice(&946u64.to_le_bytes());
    bytes[64..96].copy_from_slice(&policy_wire(version)[184..]);
    bytes[96..136].copy_from_slice(&challenge_wire(version)[56..96]);
    bytes[136..168].copy_from_slice(&challenge_wire(version)[168..]);
    bytes[168..200].fill(0x71);
    bytes[200..208].copy_from_slice(&1u64.to_le_bytes());
    bytes[272..304].copy_from_slice(&policy_wire(version)[112..144]);
    rebuild(&mut bytes, version);
    bytes
}
