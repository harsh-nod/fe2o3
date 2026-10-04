//! Live compiler-only origin reobservation. No wire attachment or owner constructor.
use super::{AuthenticatedCollectedKernelClosureV1, CollectedFunctionRole};
use crate::rustc_semantic_adapter_v1::{
    CanonicalSourceProvenanceV1, borrowed_rustc_mir_body_sha256_v1,
    canonical_function_identities_v1, canonical_source_provenance_v1, rustc_block_identity_v1,
};
use fe2o3_lower_mir_kernel::ProductionOrderedProgramInspectionV1;
use fe2o3_mir_model::semantic_mir_v1::{SemanticBlockIdentityV1, SemanticFunctionIdentityV1};
use rustc_middle::mir::{TerminatorKind, UnwindAction};
use rustc_middle::ty::{Instance, TyCtxt};
use rustc_span::{Pos, Span};

pub(crate) const MAX_ORIGIN_WORK_V1: usize = 1_048_576;
pub(crate) const MAX_ORIGIN_EXPANSION_DEPTH_V1: usize = 256;
const MAX_BODY_ITEMS: usize = 65_536;

/// Move-only seed borrowed from the collector-sealed root in the same TyCtxt.
pub(crate) struct OrderedOriginRootV1<'tcx> {
    instance: Instance<'tcx>,
}
pub(crate) struct OrderedOriginCaptureV1 {
    pub(crate) source: CanonicalSourceProvenanceV1,
    pub(crate) function: [u8; 32],
    pub(crate) monomorphization: [u8; 32],
    pub(crate) mir_body: [u8; 32],
    pub(crate) mir_block: u32,
    pub(crate) block_identity: [u8; 32],
    pub(crate) work_used: usize,
}

impl<'tcx> OrderedOriginRootV1<'tcx> {
    pub(crate) fn from_collected(
        closure: &AuthenticatedCollectedKernelClosureV1<'tcx>,
    ) -> Result<Self, &'static str> {
        let [root] = closure.roots.as_ref() else {
            return Err("origin requires one actual collected root");
        };
        let [function] = closure.collection.functions.as_slice() else {
            return Err("origin profile excludes helper bodies");
        };
        if root.instance != function.instance
            || root.role != CollectedFunctionRole::KernelEntry
            || function.role != CollectedFunctionRole::KernelEntry
        {
            return Err("origin root does not match collected kernel");
        }
        Ok(Self {
            instance: root.instance,
        })
    }

    /// Called only after the SAME transaction has passed normal V32 admission.
    pub(crate) fn capture(
        self,
        tcx: TyCtxt<'tcx>,
        view: &ProductionOrderedProgramInspectionV1<'_>,
        expected_function: SemanticFunctionIdentityV1,
        expected_block: SemanticBlockIdentityV1,
    ) -> Result<OrderedOriginCaptureV1, &'static str> {
        let identities = canonical_function_identities_v1(tcx, self.instance);
        if identities.function() != expected_function
            || identities.function().as_bytes() != &view.ordered_program().source().function
        {
            return Err("origin root identity mismatch");
        }
        let body = tcx.instance_mir(self.instance.def);
        if body.basic_blocks.len() > crate::production_ordered_program_v32::MAX_PROFILE_BLOCKS
            || body.local_decls.len() > MAX_BODY_ITEMS
        {
            return Err("origin body bound exceeded");
        }
        let mut items = body.local_decls.len();
        for block in body.basic_blocks.iter() {
            items = items
                .checked_add(block.statements.len())
                .and_then(|value| value.checked_add(1))
                .ok_or("origin body item overflow")?;
            if items > MAX_BODY_ITEMS {
                return Err("origin body item bound exceeded");
            }
        }
        // Existing stable full-body producer; rustc query/hash internals are not
        // represented as a diagnostic allocator or whole-compiler RSS bound.
        let mir_body = borrowed_rustc_mir_body_sha256_v1(tcx, self.instance, body);
        let mut meter = Work::new(MAX_ORIGIN_WORK_V1);
        meter.charge(items)?;
        let mut selected = None;
        for (raw, _) in body.basic_blocks.iter_enumerated() {
            meter.charge(1)?;
            let identity = rustc_block_identity_v1(identities.function(), mir_body, raw.as_u32());
            if identity == expected_block {
                take_unique(&mut selected, raw)?;
            }
        }
        let raw = selected.ok_or("origin compiler block identity not found")?;
        let terminator = body.basic_blocks[raw].terminator();
        let TerminatorKind::Call {
            func,
            args,
            destination,
            target: Some(_),
            unwind: UnwindAction::Continue | UnwindAction::Unreachable,
            ..
        } = &terminator.kind
        else {
            return Err("origin compiler block does not end in the admitted call");
        };
        if args.len() != 8 || !destination.projection.is_empty() {
            return Err("origin call shape mismatch");
        }
        let actual = crate::production_ordered_program_v32::observe_call(
            tcx,
            self.instance,
            body,
            func,
            [
                &args[0].node,
                &args[1].node,
                &args[2].node,
                &args[3].node,
                &args[4].node,
                &args[5].node,
                &args[6].node,
                &args[7].node,
            ],
        )?;
        let declared = view.ordered_program();
        if actual.program().count() != declared.program().count()
            || actual.program().packed_words() != declared.program().packed_words()
            || actual.registers().scratch() != declared.registers().scratch()
            || actual.registers().output() != declared.registers().output()
            || actual.registers().inputs() != declared.registers().inputs()
        {
            return Err("origin actual call descriptor or register mismatch");
        }
        let span = if terminator.source_info.span.is_dummy() {
            body.span
        } else {
            terminator.source_info.span
        };
        let source = capture_source(tcx, span, &mut meter)?;
        require_provenance(source.provenance(), view.source_provenance())?;
        Ok(OrderedOriginCaptureV1 {
            source,
            function: *identities.function().as_bytes(),
            monomorphization: *identities.monomorphization().as_bytes(),
            mir_body,
            mir_block: raw.as_u32(),
            block_identity: *expected_block.as_bytes(),
            work_used: meter.used,
        })
    }
}

fn take_unique<T>(selected: &mut Option<T>, candidate: T) -> Result<(), &'static str> {
    if selected.is_some() {
        return Err("origin compiler block identity is ambiguous");
    }
    *selected = Some(candidate);
    Ok(())
}
fn require_provenance(
    actual: fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1,
    expected: fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1,
) -> Result<(), &'static str> {
    if actual != expected || actual.expansion().is_none() || actual.call_site().is_none() {
        return Err("origin retained semantic provenance mismatch or unavailable");
    }
    Ok(())
}

struct Work {
    limit: usize,
    used: usize,
}
impl Work {
    const fn new(limit: usize) -> Self {
        Self { limit, used: 0 }
    }
    fn charge(&mut self, amount: usize) -> Result<(), &'static str> {
        let next = self
            .used
            .checked_add(amount)
            .ok_or("origin work overflow")?;
        if next > self.limit {
            return Err("origin work bound exceeded");
        }
        self.used = next;
        Ok(())
    }
    fn passes(&mut self, length: usize, count: usize) -> Result<(), &'static str> {
        let amount = length.checked_add(1).ok_or("origin work overflow")?;
        for _ in 0..count {
            self.charge(amount)?;
        }
        Ok(())
    }
}

fn capture_source(
    tcx: TyCtxt<'_>,
    span: Span,
    meter: &mut Work,
) -> Result<CanonicalSourceProvenanceV1, &'static str> {
    let mut cursor = span;
    for depth in 0..=MAX_ORIGIN_EXPANSION_DEPTH_V1 {
        meter.charge(1)?;
        let Some(parent) = cursor.parent_callsite() else {
            break;
        };
        if depth == MAX_ORIGIN_EXPANSION_DEPTH_V1 {
            return Err("origin macro expansion bound exceeded");
        }
        let data = cursor.ctxt().outer_expn_data();
        if let rustc_span::ExpnKind::Macro(_, name) = data.kind {
            meter.charge(name.as_str().len())?;
        }
        if let Some(features) = data.allow_internal_unstable {
            meter.charge(features.len())?;
            for feature in features.iter() {
                meter.charge(feature.as_str().len())?;
            }
        }
        cursor = parent;
    }
    // Prepay endpoint lookup/prefix/line work before the canonical adapter's
    // source_callsite/lookup_char_pos operations. No source files are opened.
    for origin in [span, cursor] {
        if origin.is_dummy() || origin.lo() > origin.hi() {
            return Err("origin source endpoints unavailable");
        }
        let mut selected = None;
        for point in [origin.lo(), origin.hi()] {
            let files = tcx.sess.source_map().files();
            meter.passes(files.len(), 2)?;
            let index = files
                .partition_point(|file| file.start_pos <= point)
                .checked_sub(1)
                .ok_or("origin source file unavailable")?;
            let file = &files[index];
            if selected
                .replace(index)
                .is_some_and(|previous| previous != index)
            {
                return Err("origin source span crosses files");
            }
            let relative = point
                .0
                .checked_sub(file.start_pos.0)
                .ok_or("origin source position overflow")?;
            let length = file.normalized_source_len.to_usize();
            if relative as usize > length {
                return Err("origin source endpoint outside file");
            }
            meter.passes(file.multibyte_chars.len(), 3)?;
            let preceding = file
                .multibyte_chars
                .partition_point(|character| character.pos.0 < relative);
            if let Some(character) = preceding
                .checked_sub(1)
                .and_then(|i| file.multibyte_chars.get(i))
                && relative
                    < character
                        .pos
                        .0
                        .checked_add(u32::from(character.bytes))
                        .ok_or("origin source position overflow")?
            {
                return Err("origin source endpoint inside UTF-8 scalar");
            }
            meter.passes(length, 5)?;
        }
    }
    canonical_source_provenance_v1(tcx, span, MAX_ORIGIN_EXPANSION_DEPTH_V1)
        .map_err(|_| "origin canonical source provenance unavailable")
}

#[cfg(test)]
#[path = "production_ordered_origin_capture_v1_tests.rs"]
mod tests;
