//! Private hash preimage, not a constructor for admitted host evidence.
use sha2::{Digest, Sha256};

type Content = ([u8; 32], u64);

pub(super) struct Preimage<'a> {
    pub(super) subject: ([u8; 32], &'a [u8]),
    pub(super) receipt: ([u8; 32], &'a [u8]),
    pub(super) finalizer: [u8; 32],
    pub(super) record: [u8; 32],
    pub(super) outer: Content,
    pub(super) capsule: Content,
    pub(super) module: Content,
    pub(super) receipts: [Content; 15],
    pub(super) linked: [u8; 32],
    pub(super) finalized: Content,
    pub(super) code_object: [u8; 32],
    pub(super) kernel: [u8; 32],
}

pub(super) fn hash(domain: &[u8], input: &Preimage<'_>) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(input.subject.0);
    digest.update(input.subject.1);
    digest.update(input.receipt.0);
    digest.update(input.receipt.1);
    digest.update(input.finalizer);
    digest.update(input.record);
    for content in [input.outer, input.capsule, input.module] {
        update_content(&mut digest, content);
    }
    digest.update(15_u16.to_le_bytes());
    for content in input.receipts {
        update_content(&mut digest, content);
    }
    digest.update(input.linked);
    update_content(&mut digest, input.finalized);
    digest.update(input.code_object);
    digest.update(input.kernel);
    digest.finalize().into()
}

fn update_content(digest: &mut Sha256, (sha256, byte_len): Content) {
    digest.update(sha256);
    digest.update(byte_len.to_le_bytes());
}

#[cfg(test)]
#[path = "recovered_worker_v3_lineage_identity_tests.rs"]
mod tests;
