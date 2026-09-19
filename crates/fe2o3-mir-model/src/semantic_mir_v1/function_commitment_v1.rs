//! Streaming commitments to the existing versioned function encoding.

use super::{
    CanonicalWriterV1, SemanticFunctionDeclV1, SemanticMirErrorV1, SemanticMirLimitsV1,
    SemanticMirResourceV1, SemanticMirWireVersionV1, encode_function,
};
use sha2::{Digest, Sha256};

const DOMAIN: &[u8] = b"fe2o3.inert-semantic-mir.function-commitment.v1";

#[cfg(test)]
mod tests;

pub(super) struct CanonicalCommitmentSinkV1<'a> {
    pub(super) sha256: &'a mut Sha256,
    pub(super) charge_work: &'a mut dyn FnMut(usize) -> Result<(), SemanticMirErrorV1>,
}

/// Inert identity of one versioned canonical function encoding.
///
/// This does not authenticate a producer, validate a function, or grant any
/// compiler, proof, artifact or launch authority. Referenced type, allocation
/// and callable table contents are outside this commitment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticFunctionCanonicalCommitmentV1 {
    wire_version: SemanticMirWireVersionV1,
    canonical_bytes: u64,
    sha256: [u8; 32],
}

impl SemanticFunctionCanonicalCommitmentV1 {
    pub const fn wire_version(self) -> SemanticMirWireVersionV1 {
        self.wire_version
    }

    /// Function encoding length, excluding the fixed hash domain and version.
    pub const fn canonical_bytes(self) -> u64 {
        self.canonical_bytes
    }

    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }
}

/// Hash a function using the same encoder as request canonicalization.
///
/// Only `CanonicalBytes` is enforced from `limits`. The caller's cumulative
/// `charge_work` hook prepays collection traversal, emitted chunks and hashing;
/// hook errors propagate unchanged and earlier charges are not refunded.
/// No canonical payload buffer is allocated. This is a bounded encoding
/// operation, not structural or semantic admission.
///
/// The selected schema determines which fields are encoded. Older schemas can
/// omit newer fields, just as in request encoding; callers requiring complete
/// field coverage must establish a suitable version through their existing
/// admission contract. The hash domain includes that exact version.
pub fn canonical_function_commitment_v1(
    function: &SemanticFunctionDeclV1,
    wire_version: SemanticMirWireVersionV1,
    limits: SemanticMirLimitsV1,
    charge_work: &mut dyn FnMut(usize) -> Result<(), SemanticMirErrorV1>,
) -> Result<SemanticFunctionCanonicalCommitmentV1, SemanticMirErrorV1> {
    let (canonical_bytes, sha256) =
        canonical_encoded_commitment_v1(DOMAIN, wire_version, limits, charge_work, |writer| {
            encode_function(writer, function, wire_version)
        })?;
    Ok(SemanticFunctionCanonicalCommitmentV1 {
        wire_version,
        canonical_bytes,
        sha256,
    })
}

pub(super) fn canonical_encoded_commitment_v1(
    domain: &[u8],
    wire_version: SemanticMirWireVersionV1,
    limits: SemanticMirLimitsV1,
    charge_work: &mut dyn FnMut(usize) -> Result<(), SemanticMirErrorV1>,
    encode: impl FnOnce(&mut CanonicalWriterV1<'_>) -> Result<(), SemanticMirErrorV1>,
) -> Result<(u64, [u8; 32]), SemanticMirErrorV1> {
    // Initialization, fixed domain and version are prepaid separately from
    // the function's canonical byte limit.
    charge_work(1 + domain.len() + size_of::<u16>())?;
    let mut sha256 = Sha256::new();
    sha256.update(domain);
    sha256.update(wire_version.as_u16().to_le_bytes());
    let canonical_bytes = {
        let mut writer =
            CanonicalWriterV1::new(limits.limit(SemanticMirResourceV1::CanonicalBytes));
        writer.commitment = Some(CanonicalCommitmentSinkV1 {
            sha256: &mut sha256,
            charge_work,
        });
        encode(&mut writer)?;
        writer.written
    };
    charge_work(1)?;
    Ok((canonical_bytes, sha256.finalize().into()))
}
