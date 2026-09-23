//! Independent wire fixtures; no protected-execution or hardware credit.
use ed25519_dalek::SigningKey;
use sha2::{Digest, Sha256};

pub const CHALLENGE_BYTES: usize = 200;
pub const SUBJECT_BYTES: usize = 690;
pub const REQUEST_BYTES: usize = 946;

pub fn seal(bytes: &mut [u8], kind: &str, version: u16) {
    let prefix = bytes.len() - 32;
    let mut hash = Sha256::new();
    hash.update(format!("FE2O3/{kind}/V{version}\0").as_bytes());
    hash.update((prefix as u64).to_le_bytes());
    hash.update(&bytes[..prefix]);
    bytes[prefix..].copy_from_slice(&hash.finalize());
}
pub fn header(bytes: &mut [u8], magic: &[u8; 8], version: u16) {
    let len = bytes.len();
    bytes[..8].copy_from_slice(magic);
    bytes[8..10].copy_from_slice(&version.to_le_bytes());
    bytes[12..20].copy_from_slice(&(len as u64).to_le_bytes());
}
pub fn policy_wire(version: u16) -> [u8; 216] {
    let mut bytes = [0; 216];
    header(
        &mut bytes,
        if version == 1 {
            b"F2O3CEP1"
        } else {
            b"F2O3CEP2"
        },
        version,
    );
    bytes[24..32].copy_from_slice(&7u64.to_le_bytes());
    bytes[32..64].fill(0x61);
    bytes[64..72].copy_from_slice(&12345u64.to_le_bytes());
    bytes[72..104].fill(0x62);
    bytes[104..112].copy_from_slice(&67890u64.to_le_bytes());
    bytes[112..144].copy_from_slice(
        &SigningKey::from_bytes(&[0x51; 32])
            .verifying_key()
            .to_bytes(),
    );
    bytes[144..176].copy_from_slice(
        &SigningKey::from_bytes(&[0x52; 32])
            .verifying_key()
            .to_bytes(),
    );
    bytes[176..178].copy_from_slice(&version.to_le_bytes());
    seal(&mut bytes, "COMPILER-EXECUTION-ISSUER-POLICY", version);
    bytes
}
pub fn subject_wire(version: u16) -> [u8; SUBJECT_BYTES] {
    let seed = 0x20;
    let mut bytes = [0; SUBJECT_BYTES];
    header(
        &mut bytes,
        if version == 1 {
            b"F2O3CES1"
        } else {
            b"F2O3CES2"
        },
        version,
    );
    bytes[24..32].copy_from_slice(&9u64.to_le_bytes());
    bytes[32..48].fill(seed + 6);
    bytes[48..80].fill(seed + 7);
    bytes[88..120].fill(seed + 8);
    bytes[120..152].fill(seed + 9);
    let mut closure = Sha256::new();
    closure.update(b"fe2o3-compiler-closure-identity-v2\0");
    closure.update(1u16.to_le_bytes());
    for axis in 0..6 {
        let pin = [seed + axis as u8; 32];
        bytes[152 + axis * 32..184 + axis * 32].copy_from_slice(&pin);
        closure.update(pin);
    }
    bytes[344..346].copy_from_slice(&1u16.to_le_bytes());
    bytes[346..378].copy_from_slice(&closure.finalize());
    for axis in 0..7 {
        let offset = 378 + axis * 40;
        bytes[offset..offset + 32].fill(seed + 10 + axis as u8);
        bytes[offset + 32..offset + 40].copy_from_slice(&(1000 + axis as u64).to_le_bytes());
    }
    seal(&mut bytes, "INERT-COMPILER-EXECUTION-SUBJECT", version);
    bytes
}
pub fn challenge_wire(version: u16) -> [u8; CHALLENGE_BYTES] {
    let mut bytes = [0; CHALLENGE_BYTES];
    header(
        &mut bytes,
        if version == 1 {
            b"F2O3CEC1"
        } else {
            b"F2O3CEC2"
        },
        version,
    );
    bytes[24..56].copy_from_slice(&policy_wire(version)[184..]);
    bytes[56..88].copy_from_slice(&subject_wire(version)[658..]);
    bytes[88..96].copy_from_slice(&690u64.to_le_bytes());
    bytes[96..128].fill(0x71);
    bytes[128..136].copy_from_slice(&1u64.to_le_bytes());
    seal(&mut bytes, "COMPILER-EXECUTION-CHALLENGE", version);
    bytes
}
pub fn request_wire(version: u16) -> [u8; REQUEST_BYTES] {
    let mut bytes = [0; REQUEST_BYTES];
    header(
        &mut bytes,
        if version == 1 {
            b"F2O3CEQ1"
        } else {
            b"F2O3CEQ2"
        },
        version,
    );
    bytes[24..224].copy_from_slice(&challenge_wire(version));
    bytes[224..914].copy_from_slice(&subject_wire(version));
    seal(&mut bytes, "COMPILER-EXECUTION-REQUEST", version);
    bytes
}
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
