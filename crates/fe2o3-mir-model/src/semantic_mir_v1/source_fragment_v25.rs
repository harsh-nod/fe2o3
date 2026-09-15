//! V25-only source-body commitments for owned-flow data.
//! This is inert canonical identity, not Rust source authentication.
use super::*;

/// Exact new source-body format. This does not follow CurrentProduction and
/// never changes the historic function-fragment commitment entry point.
pub const SEMANTIC_SOURCE_BODY_FRAGMENT_VERSION_V25: SemanticMirWireVersionV1 =
    SemanticMirWireVersionV1::V25;

/// Hash the retained canonical body without its own optional defined record.
/// Excluding that attachment avoids a body/incoming/attachment hash cycle; all
/// ABI, role, export, local, block, operation and source fields remain encoded.
/// The returned byte count lets the source/admission owner charge its work.
pub fn canonical_semantic_source_body_sha256_v25(
    function: &SemanticFunctionDeclV1,
    max_bytes: u64,
) -> Result<([u8; 32], usize), SemanticMirErrorV1> {
    let mut writer = CanonicalWriterV1::new(max_bytes.min(HARD_MAX_CANONICAL_BYTES_V1));
    writer.u16(SEMANTIC_SOURCE_BODY_FRAGMENT_VERSION_V25.as_u16())?;
    encode_function_with_defined_contract(
        &mut writer,
        function,
        SEMANTIC_SOURCE_BODY_FRAGMENT_VERSION_V25,
        None,
    )?;
    let bytes = writer.finish();
    let mut digest = Sha256::new();
    digest.update(b"FE2O3/SEMANTIC-FUNCTION-FRAGMENT/V1\0");
    digest.update(&bytes);
    Ok((digest.finalize().into(), bytes.len()))
}
