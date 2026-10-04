//! Checked, prepaid coordinate lookup for the supplied once-shim's provenance.

use std::sync::Arc;

use rustc_middle::ty::TyCtxt;
use rustc_span::source_map::SourceMap;
use rustc_span::{BytePos, Pos, SourceFile, Span};

use super::{
    ProductionSemanticBodyErrorV1, ProductionSemanticBodyRequestOwnerV1, SemanticMirResourceV1,
    SemanticSourceProvenanceV1, unsupported,
};
use crate::rustc_semantic_adapter_v1::canonical_source_provenance_v1;

pub(in crate::production_semantic_body_v1) fn reobserve_receiver_source_v1(
    tcx: TyCtxt<'_>,
    terminator_span: Span,
    body_span: Span,
    owner: &mut ProductionSemanticBodyRequestOwnerV1<'_>,
) -> Result<SemanticSourceProvenanceV1, ProductionSemanticBodyErrorV1> {
    // Keep the fallback and depth limit equal to preflight's capture_source_v1.
    const MAX_EXPANSION_DEPTH: usize = 256;
    let span = if terminator_span.is_dummy() {
        body_span
    } else {
        terminator_span
    };
    let mut cursor = span;
    // Prewalk before canonical_source_provenance_v1 calls source_callsite().
    for depth in 0..=MAX_EXPANSION_DEPTH {
        owner.charge(SemanticMirResourceV1::ValidationWork, 1)?;
        let Some(parent) = cursor.parent_callsite() else {
            break;
        };
        if depth == MAX_EXPANSION_DEPTH {
            return Err(unsupported(
                "shared receiver source expansion depth",
                None,
                None,
            ));
        }
        owner.charge(SemanticMirResourceV1::ValidationWork, 1)?;
        let data = cursor.ctxt().outer_expn_data();
        if let rustc_span::ExpnKind::Macro(_, name) = data.kind {
            owner.charge(SemanticMirResourceV1::ValidationWork, name.as_str().len())?;
        }
        if let Some(features) = data.allow_internal_unstable {
            owner.charge(SemanticMirResourceV1::ValidationWork, features.len())?;
            for feature in features.iter() {
                owner.charge(
                    SemanticMirResourceV1::ValidationWork,
                    feature.as_str().len(),
                )?;
            }
        }
        cursor = parent;
    }
    owner.charge(SemanticMirResourceV1::ValidationWork, 2)?;
    // The adapter does four lookups, even when expansion and callsite coincide.
    // Each span must be within one file; the two spans may name different files.
    for origin in [span, cursor] {
        if origin.is_dummy() || origin.lo() > origin.hi() {
            return Err(source_endpoint_error());
        }
        let start = checked_endpoint_v1(tcx.sess.source_map(), origin.lo(), owner)?;
        let end = checked_endpoint_v1(tcx.sess.source_map(), origin.hi(), owner)?;
        if !Arc::ptr_eq(&start, &end) {
            return Err(unsupported(
                "shared receiver cross-file source span",
                None,
                None,
            ));
        }
    }
    canonical_source_provenance_v1(tcx, span, MAX_EXPANSION_DEPTH)
        .map(|captured| captured.provenance())
        .map_err(|_| unsupported("shared receiver source provenance", None, None))
}

fn checked_endpoint_v1(
    source_map: &SourceMap,
    position: BytePos,
    owner: &mut ProductionSemanticBodyRequestOwnerV1<'_>,
) -> Result<Arc<SourceFile>, ProductionSemanticBodyErrorV1> {
    let file = {
        let files = source_map.files();
        // One search here and another in lookup_char_pos. These conservative
        // bounds do not depend on a source-map cache hit.
        charge_passes_v1(owner, files.len(), 2)?;
        let index = files
            .partition_point(|file| file.start_pos <= position)
            .checked_sub(1)
            .ok_or_else(source_endpoint_error)?;
        files
            .get(index)
            .cloned()
            .ok_or_else(source_endpoint_error)?
    };
    let relative = position
        .0
        .checked_sub(file.start_pos.0)
        .ok_or_else(source_endpoint_error)?;
    let source_len = file.normalized_source_len.to_usize();
    if relative as usize > source_len {
        return Err(source_endpoint_error());
    }
    // Boundary validation and rustc's two prefix scans (endpoint and line
    // start). Use trusted metadata, not src: imported files may have no text.
    charge_passes_v1(owner, file.multibyte_chars.len(), 3)?;
    let preceding = file
        .multibyte_chars
        .partition_point(|character| character.pos.0 < relative);
    if let Some(character) = preceding
        .checked_sub(1)
        .and_then(|index| file.multibyte_chars.get(index))
    {
        let end = character
            .pos
            .0
            .checked_add(u32::from(character.bytes))
            .ok_or_else(source_endpoint_error)?;
        if relative < end {
            return Err(source_endpoint_error());
        }
    }
    // A file-byte bound covers: lazy line-table decoding, line search, newline
    // search, optional external-line copying, and display-prefix traversal.
    // Reserve all five passes even with absent text; do not trigger loading.
    // Valid rustc metadata has at most source_len + 1 line starts. EOF is valid.
    charge_passes_v1(owner, source_len, 5)?;
    Ok(file)
}

fn charge_passes_v1(
    owner: &mut ProductionSemanticBodyRequestOwnerV1<'_>,
    length: usize,
    passes: usize,
) -> Result<(), ProductionSemanticBodyErrorV1> {
    let bound = length
        .checked_add(1)
        .ok_or_else(|| unsupported("shared receiver source work overflow", None, None))?;
    // Fixed pass counts avoid multiplying an input length before ledger checks.
    for _ in 0..passes {
        owner.charge(SemanticMirResourceV1::ValidationWork, bound)?;
    }
    Ok(())
}

fn source_endpoint_error() -> ProductionSemanticBodyErrorV1 {
    unsupported("shared receiver source byte endpoint", None, None)
}

#[cfg(test)]
#[path = "receiver_source_tests.rs"]
mod tests;
#[cfg(test)]
pub(in crate::production_semantic_body_v1) use tests::assert_receiver_source_guards_v1;
