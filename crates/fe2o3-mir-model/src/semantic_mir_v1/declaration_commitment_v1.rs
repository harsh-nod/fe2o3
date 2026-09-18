//! Streaming identities for complete declaration tables, not producer authority.

use super::function_commitment_v1::canonical_encoded_commitment_v1;
use super::*;

const DOMAIN: &[u8] = b"fe2o3.inert-semantic-mir.declaration-tables-commitment.v1";

/// Identity of the versioned type and callable tables, including referenced
/// indices and complete inline metadata. Function bodies and allocation tables
/// are not encoded here. This record grants no admission or execution authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticDeclarationTablesCommitmentV1 {
    wire_version: SemanticMirWireVersionV1,
    canonical_bytes: u64,
    sha256: [u8; 32],
}

impl SemanticDeclarationTablesCommitmentV1 {
    pub const fn wire_version(self) -> SemanticMirWireVersionV1 {
        self.wire_version
    }
    pub const fn canonical_bytes(self) -> u64 {
        self.canonical_bytes
    }
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }
}

/// Encode both complete tables with the existing canonical encoders and no
/// payload buffer. Only CanonicalBytes is enforced from limits; the cumulative
/// work callback is prepaid and never refunded. As with canonical function
/// commitments, field coverage is determined by the caller-selected schema.
/// This is encoding, not validation or authentication.
pub fn canonical_declaration_tables_commitment_v1(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    wire_version: SemanticMirWireVersionV1,
    limits: SemanticMirLimitsV1,
    charge_work: &mut dyn FnMut(usize) -> Result<(), SemanticMirErrorV1>,
) -> Result<SemanticDeclarationTablesCommitmentV1, SemanticMirErrorV1> {
    let (canonical_bytes, sha256) =
        canonical_encoded_commitment_v1(DOMAIN, wire_version, limits, charge_work, |writer| {
            writer.count(types.len())?;
            for ty in types {
                encode_type(writer, ty, wire_version)?;
            }
            writer.count(callables.len())?;
            for callable in callables {
                encode_callable(writer, callable, wire_version)?;
            }
            Ok(())
        })?;
    Ok(SemanticDeclarationTablesCommitmentV1 {
        wire_version,
        canonical_bytes,
        sha256,
    })
}
