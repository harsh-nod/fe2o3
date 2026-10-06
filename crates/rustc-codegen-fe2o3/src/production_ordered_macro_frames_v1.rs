//! Same-session, whole-region expansion frames. No decoder or owner constructor.
use super::{CanonicalSourceProvenanceV1, Work, prepay_origin};
use crate::rustc_semantic_adapter_v1::{
    canonical_expansion_frame_sha256_v1, canonical_source_origin_v1,
};
use fe2o3_mir_model::semantic_mir_v1::SemanticSourceOriginV1;
use rustc_middle::ty::TyCtxt;
use rustc_span::{ExpnKind, Span};
use serde::{Serialize, Serializer, ser::SerializeSeq};
use std::io::{self, Write};

pub(crate) const MAX_MACRO_FRAMES_V1: usize = 32;
pub(crate) const MAX_MACRO_NAME_BYTES_V1: usize = 256;
pub(crate) const MAX_MACRO_REPORT_BYTES_V1: usize = 64 * 1024;
// Fixed capture + serialized report coexistence. Not rustc/allocator RSS.
pub(crate) const MACRO_LOGICAL_PREPAY_V1: usize = 2 * MAX_MACRO_REPORT_BYTES_V1 + 16 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "availability", rename_all = "snake_case")]
enum Origin {
    Available {
        file_identity: [u8; 32],
        byte_start: u64,
        byte_end: u64,
        line_start: u32,
        column_start: u32,
        line_end: u32,
        column_end: u32,
    },
    UnavailableDummyDefinition,
}
impl Origin {
    fn known(origin: SemanticSourceOriginV1) -> Self {
        let (byte_start, byte_end) = origin.byte_range();
        let (line_start, column_start) = origin.start_coordinate();
        let (line_end, column_end) = origin.end_coordinate();
        Self::Available {
            file_identity: *origin.file().as_bytes(),
            byte_start,
            byte_end,
            line_start,
            column_start,
            line_end,
            column_end,
        }
    }
}
#[derive(Clone, Copy)]
struct Name {
    bytes: [u8; MAX_MACRO_NAME_BYTES_V1],
    len: usize,
}
impl Name {
    fn new(text: &str, meter: &mut Work) -> Result<Self, &'static str> {
        if text.is_empty() || text.len() > MAX_MACRO_NAME_BYTES_V1 {
            return Err("macro frame name bound");
        }
        meter.charge(text.len())?;
        let mut result = Self {
            bytes: [0; MAX_MACRO_NAME_BYTES_V1],
            len: text.len(),
        };
        result.bytes[..text.len()].copy_from_slice(text.as_bytes());
        Ok(result)
    }
}
impl Serialize for Name {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(
            std::str::from_utf8(&self.bytes[..self.len]).map_err(serde::ser::Error::custom)?,
        )
    }
}
#[derive(Clone, Copy, Serialize)]
struct Frame {
    ordinal: usize,
    kind: &'static str,
    macro_name: Option<Name>,
    // Binds complete actual ExpnData, including sub-kind/definition identity.
    // The category string is not a reconstruction of that complete identity.
    expansion_identity: [u8; 32],
    expansion: Origin,
    call_site: Origin,
    definition_site: Origin,
}
struct Frames {
    rows: [Option<Frame>; MAX_MACRO_FRAMES_V1],
    len: usize,
}
impl Frames {
    fn new() -> Self {
        Self {
            rows: [None; MAX_MACRO_FRAMES_V1],
            len: 0,
        }
    }
    fn push(&mut self, value: Frame) -> Result<(), &'static str> {
        if self.len == MAX_MACRO_FRAMES_V1 {
            return Err("macro frame depth bound");
        }
        if value.ordinal != self.len {
            return Err("macro frame order mismatch");
        }
        self.rows[self.len] = Some(value);
        self.len += 1;
        Ok(())
    }
}
impl Serialize for Frames {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.len))?;
        for row in &self.rows[..self.len] {
            seq.serialize_element(
                row.as_ref()
                    .ok_or_else(|| <S::Error as serde::ser::Error>::custom("macro frame hole"))?,
            )?;
        }
        seq.end()
    }
}
struct Visited {
    rows: [Span; MAX_MACRO_FRAMES_V1 + 1],
    len: usize,
}
impl Visited {
    fn new() -> Self {
        Self {
            rows: [rustc_span::DUMMY_SP; MAX_MACRO_FRAMES_V1 + 1],
            len: 0,
        }
    }
    fn insert(&mut self, span: Span, meter: &mut Work) -> Result<(), &'static str> {
        if self.len == self.rows.len() {
            return Err("macro frame depth bound");
        }
        meter.charge(self.len.checked_add(1).ok_or("macro cycle work overflow")?)?;
        if self.rows[..self.len].contains(&span) {
            return Err("macro frame cycle");
        }
        self.rows[self.len] = span;
        self.len += 1;
        Ok(())
    }
}
#[derive(Serialize)]
pub(crate) struct OrderedMacroFramesV1 {
    schema: &'static str,
    canonical_sha256: [u8; 32],
    source_frontend_unit: [u8; 32],
    source_contract: [u8; 32],
    source_function: [u8; 32],
    source_statement: [u8; 32],
    expansion_chain_sha256: [u8; 32],
    expansion_depth: usize,
    frame_order: &'static str,
    origin_scope: &'static str,
    frames: Frames,
    source_reobservation_work_used: usize,
    maximum_frames: usize,
    maximum_name_bytes: usize,
    maximum_report_bytes: usize,
    rustc_internal_allocations_accounted: bool,
    is_llvm_inline_stack: bool,
    instruction_specific_origins_available: bool,
    allocator_lifetime_trace_available: bool,
    authenticates_source: bool,
    grants_artifact_or_launch_authority: bool,
}
impl OrderedMacroFramesV1 {
    pub(crate) fn bytes(&self) -> Result<Vec<u8>, &'static str> {
        encode(self)
    }
    pub(crate) fn frame_count(&self) -> usize {
        self.frames.len
    }
}
fn origin(
    tcx: TyCtxt<'_>,
    span: Span,
    definition: bool,
    meter: &mut Work,
) -> Result<Origin, &'static str> {
    if span.is_dummy() && definition {
        return Ok(Origin::UnavailableDummyDefinition);
    }
    prepay_origin(tcx, span, meter)?;
    canonical_source_origin_v1(tcx, span)
        .map(Origin::known)
        .map_err(|_| "macro frame canonical origin unavailable")
}
fn check_binding(
    expected: (
        fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1,
        [u8; 32],
        usize,
    ),
    source: (
        fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1,
        [u8; 32],
        usize,
    ),
    count: usize,
) -> Result<(), &'static str> {
    if expected != source || count != source.2 {
        return Err("macro frame provenance or depth mismatch");
    }
    Ok(())
}
pub(super) fn capture(
    tcx: TyCtxt<'_>,
    span: Span,
    source: CanonicalSourceProvenanceV1,
    view: &fe2o3_lower_mir_kernel::ProductionOrderedProgramInspectionV1<'_>,
    meter: &mut Work,
) -> Result<OrderedMacroFramesV1, &'static str> {
    if source.expansion_depth() > MAX_MACRO_FRAMES_V1 {
        return Err("macro frame depth bound");
    }
    if std::mem::size_of::<OrderedMacroFramesV1>()
        .checked_add(std::mem::size_of::<Visited>())
        .is_none_or(|size| size > MAX_MACRO_REPORT_BYTES_V1)
    {
        return Err("macro frame fixed storage bound");
    }
    let mut frames = Frames::new();
    let mut visited = Visited::new();
    let mut cursor = span;
    visited.insert(cursor, meter)?;
    while let Some(parent) = cursor.parent_callsite() {
        if frames.len == MAX_MACRO_FRAMES_V1 {
            return Err("macro frame depth bound");
        }
        visited.insert(parent, meter)?;
        meter.charge(1)?;
        let data = cursor.ctxt().outer_expn_data();
        let (kind, macro_name) = match data.kind {
            ExpnKind::Macro(_, name) => ("macro", Some(Name::new(name.as_str(), meter)?)),
            ExpnKind::Desugaring(_) => ("desugaring", None),
            ExpnKind::AstPass(_) => ("ast_pass", None),
            ExpnKind::Root => return Err("macro frame root has a parent"),
        };
        // Prepay repeated ExpnData hash/name/features before the actual hash.
        if let Some(name) = macro_name {
            meter.charge(name.len)?;
        }
        if let Some(features) = data.allow_internal_unstable {
            meter.charge(features.len())?;
            for feature in features.iter() {
                meter.charge(feature.as_str().len())?;
            }
        }
        meter.charge(4)?;
        let expansion_identity = canonical_expansion_frame_sha256_v1(tcx, cursor);
        frames.push(Frame {
            ordinal: frames.len,
            kind,
            macro_name,
            expansion_identity,
            expansion: origin(tcx, cursor, false, meter)?,
            call_site: origin(tcx, data.call_site, false, meter)?,
            definition_site: origin(tcx, data.def_site, true, meter)?,
        })?;
        cursor = parent;
    }
    // Reobserve canonical chain from the SAME span, with all endpoint/chain
    // work charged by the original capture helper and shared remaining meter.
    let replay = super::capture_source(tcx, span, meter)?;
    check_binding(
        (
            source.provenance(),
            source.expansion_chain_sha256(),
            source.expansion_depth(),
        ),
        (
            replay.provenance(),
            replay.expansion_chain_sha256(),
            replay.expansion_depth(),
        ),
        frames.len,
    )?;
    if replay.provenance() != view.source_provenance() {
        return Err("macro frame retained source mismatch");
    }
    let declared = view.ordered_program().source();
    Ok(OrderedMacroFramesV1 {
        schema: "fe2o3-diagnostic-ordered-region-macro-frames-v1",
        canonical_sha256: *view.canonical_identity().digest(),
        source_frontend_unit: declared.frontend_unit,
        source_contract: declared.contract,
        source_function: declared.function,
        source_statement: declared.statement,
        expansion_chain_sha256: source.expansion_chain_sha256(),
        expansion_depth: source.expansion_depth(),
        frame_order: "innermost_to_outermost",
        origin_scope: "whole_ordered_region",
        frames,
        source_reobservation_work_used: meter.used,
        maximum_frames: MAX_MACRO_FRAMES_V1,
        maximum_name_bytes: MAX_MACRO_NAME_BYTES_V1,
        maximum_report_bytes: MAX_MACRO_REPORT_BYTES_V1,
        rustc_internal_allocations_accounted: false,
        is_llvm_inline_stack: false,
        instruction_specific_origins_available: false,
        allocator_lifetime_trace_available: false,
        authenticates_source: false,
        grants_artifact_or_launch_authority: false,
    })
}
struct Limited(Vec<u8>);
impl Write for Limited {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_MACRO_REPORT_BYTES_V1.saturating_sub(self.0.len()) {
            return Err(io::Error::other("macro report byte bound"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn encode(value: &impl Serialize) -> Result<Vec<u8>, &'static str> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(MAX_MACRO_REPORT_BYTES_V1)
        .map_err(|_| "macro report allocation")?;
    let mut out = Limited(bytes);
    serde_json::to_writer(&mut out, value).map_err(|_| "macro report encoding bound")?;
    Ok(out.0)
}
#[cfg(test)]
#[path = "production_ordered_macro_frames_v1_tests.rs"]
mod tests;
