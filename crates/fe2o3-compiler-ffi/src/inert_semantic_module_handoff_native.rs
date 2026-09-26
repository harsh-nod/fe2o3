//! Shared native outer framing/content mechanics; callers supply closed schemas.
use super::*;
use std::ops::Range;

pub(super) fn decode_work(n: usize) -> Result<usize, InertSemanticCompilerModuleHandoffErrorV3> {
    use crate::{
        MAX_COMPILER_FFI_ENVELOPE_BYTES_V1 as ENVELOPE,
        MAX_COMPILER_MODULE_SYMBOL_MANIFEST_BYTES_V1 as MANIFEST,
    };
    use fe2o3_rustc_invocation::MAX_DESCRIPTOR_BYTES_V3 as INVOCATION;
    if !(MIN_OUTER_BYTES_V3..=MAX_INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_BYTES_V3).contains(&n) {
        return Err(InertSemanticCompilerModuleHandoffErrorV3::InvalidLength(
            n as u64,
        ));
    }
    let mut work: usize = 4 * 1024 * 1024 + 128 * 127 * (128 + 3 * 32 + 4);
    for (bytes, factor) in [
        (n, 8),
        (n.min(INVOCATION), 128),
        (n.min(ENVELOPE), 128),
        (n.min(MANIFEST), 320),
        ((n / 5).min(16384), 4096),
    ] {
        work = bytes
            .checked_mul(factor)
            .and_then(|w| work.checked_add(w))
            .ok_or(InertSemanticCompilerModuleHandoffErrorV3::LengthOverflow)?;
    }
    Ok(work)
}

pub(super) enum SealError<E> {
    Charge(E),
    Wire(InertSemanticCompilerModuleHandoffErrorV3),
}
impl<E> From<InertSemanticCompilerModuleHandoffErrorV3> for SealError<E> {
    fn from(e: InertSemanticCompilerModuleHandoffErrorV3) -> Self {
        Self::Wire(e)
    }
}

pub(super) fn seal<E>(
    schema: &WireSchema,
    extents: (usize, usize, usize),
    bytes: &mut [u8],
    capsule: (&[u8; 32], u64),
    module: (&[u8; 32], u64),
    mut charge: impl FnMut(usize) -> Result<(), E>,
) -> Result<[u8; 32], SealError<E>> {
    let (capsule_len, module_len, total) = extents;
    if bytes.len() != total || capsule.1 != capsule_len as u64 || module.1 != module_len as u64 {
        return Err(
            InertSemanticCompilerModuleHandoffErrorV3::InvalidLength(bytes.len() as u64).into(),
        );
    }
    let work = total
        .checked_add(
            schema.outer_domain.len()
                + schema.pair_domain.len()
                + 16
                + 256
                + PAIR_BINDING_PREIMAGE_BYTES_V3
                + 3 * INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V3
                + 2 * HEADER_BYTES_V3,
        )
        .ok_or(InertSemanticCompilerModuleHandoffErrorV3::LengthOverflow)?;
    charge(work).map_err(SealError::Charge)?;
    drop(charge);
    let (pair, _) = encode_pair_binding(schema, capsule.0, capsule.1, module.0, module.1)?;
    bytes[..HEADER_BYTES_V3].fill(0);
    bytes[..8].copy_from_slice(&schema.magic);
    bytes[8..10].copy_from_slice(&schema.version.to_le_bytes());
    bytes[12..20].copy_from_slice(&(total as u64).to_le_bytes());
    bytes[24..32].copy_from_slice(&(capsule_len as u64).to_le_bytes());
    bytes[32..40].copy_from_slice(&(module_len as u64).to_le_bytes());
    bytes[HEADER_BYTES_V3 + capsule_len + module_len..total - SHA256_BYTES].copy_from_slice(&pair);
    let sha256 = derive_identity_sha256(schema.outer_domain, &bytes[..total - SHA256_BYTES])
        .ok_or(InertSemanticCompilerModuleHandoffErrorV3::ZeroIdentity {
            field: "inert semantic compiler module handoff",
        })?;
    bytes[total - SHA256_BYTES..].copy_from_slice(&sha256);
    Ok(sha256)
}

pub(super) fn target_commitment(
    target: crate::DeviceTargetV1,
    commitment: &[u8],
    module: &CompilerModuleHandoffV2,
) -> Result<(), InertSemanticCompilerModuleHandoffErrorV3> {
    if target != module.target() {
        return Err(InertSemanticCompilerModuleHandoffErrorV3::TargetMismatch);
    }
    let value = InertFinalCompilerModuleCommitmentV3::decode(commitment)
        .map_err(InertSemanticCompilerModuleHandoffErrorV3::FinalCommitment)?;
    if !value.matches_handoff(module) {
        return Err(InertSemanticCompilerModuleHandoffErrorV3::FinalCommitmentMismatch);
    }
    Ok(())
}

pub(super) struct Finished {
    pub module: CompilerModuleHandoffV2,
    pub pair_range: Range<usize>,
    pub pair_sha256: [u8; 32],
}

// Shared post-capsule content decoder. The closure is a private content preflight,
// not semantic admission or a callback allowed to return source/proof custody.
pub(super) fn finish<E: From<InertSemanticCompilerModuleHandoffErrorV3>>(
    schema: &WireSchema,
    backing: &Arc<Vec<u8>>,
    range: &Range<usize>,
    wire: &ValidatedOuterWireV3,
    capsule: (&[u8; 32], u64),
    preflight: impl FnOnce(&CompilerModuleHandoffV2) -> Result<usize, E>,
) -> Result<Finished, E> {
    let module = CompilerModuleHandoffV2::decode_shared_vec_range(
        backing.clone(),
        range.start + wire.module_handoff_range.start,
        wire.module_handoff_range.len(),
    )
    .map_err(InertSemanticCompilerModuleHandoffErrorV3::ModuleHandoff)?;
    let pair = wire.parsed_pair_binding;
    if capsule.0 != &pair.capsule_sha256 || capsule.1 != pair.capsule_len {
        return Err(InertSemanticCompilerModuleHandoffErrorV3::CapsuleIdentityMismatch.into());
    }
    if module.identity().sha256() != &pair.module_handoff_sha256
        || module.identity().byte_len() != pair.module_handoff_len
    {
        return Err(
            InertSemanticCompilerModuleHandoffErrorV3::ModuleHandoffIdentityMismatch.into(),
        );
    }
    if preflight(&module)? != range.len() {
        return Err(InertSemanticCompilerModuleHandoffErrorV3::NonCanonicalEncoding.into());
    }
    let (canonical_pair, pair_sha256) = encode_pair_binding(
        schema,
        capsule.0,
        capsule.1,
        module.identity().sha256(),
        module.identity().byte_len(),
    )?;
    let pair_range =
        range.start + wire.pair_binding_range.start..range.start + wire.pair_binding_range.end;
    if backing[pair_range.clone()] != canonical_pair || pair_sha256 != pair.binding_sha256 {
        return Err(InertSemanticCompilerModuleHandoffErrorV3::NonCanonicalEncoding.into());
    }
    Ok(Finished {
        module,
        pair_range,
        pair_sha256,
    })
}
