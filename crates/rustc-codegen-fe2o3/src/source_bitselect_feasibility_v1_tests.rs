//! Test-only typed-source capture from the collector's sealed Instance.
//! No serialized record, name, range, or caller-selected function is authority.
//! This deliberately refuses normalization instead of mixing coordinate spaces.

use std::marker::PhantomData;

use rustc_hir::def::Res;
use rustc_hir::{BinOpKind, BindingMode, Expr, ExprKind, HirId, PatKind, StmtKind};
use rustc_middle::ty::{TyCtxt, TypeckResults};
use rustc_span::{FileName, SourceFile, Span};
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{AuthenticatedCollectedKernelClosureV1, CollectedFunctionRole};
use crate::rustc_semantic_adapter_v1::{
    CanonicalFunctionIdentitiesV1, canonical_function_identities_v1, canonical_source_provenance_v1,
};
use fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1;

#[cfg(target_os = "linux")]
#[path = "source_bitselect_retained_v1_tests.rs"]
pub(crate) mod retained;

pub(crate) const SOURCE_CAP: usize = 64 * 1024;
const WORK_CAP: usize = 1024 * 1024;
const STORAGE_CAP: usize = 512 * 1024;
const ROW_CAP: usize = 4096;

/// Cumulative accounting for this leaf's own scans and charged source/name
/// payload upper bounds, NOT compiler-query work, allocator metadata or RSS.
/// Charges never reset between HIR capture, normal stages and the exact join.
#[derive(Debug, Default, Serialize)]
pub(crate) struct ScanMeter {
    work: usize,
    charged_storage: usize,
    source_bytes_read: usize,
}

impl ScanMeter {
    pub(crate) fn scan(&mut self, count: usize) -> Result<(), String> {
        self.work = self
            .work
            .checked_add(count)
            .ok_or("source-boundary work overflow")?;
        if self.work > WORK_CAP {
            return Err("source-boundary work limit".into());
        }
        Ok(())
    }

    pub(crate) fn rows(&mut self, count: usize) -> Result<(), String> {
        if count > ROW_CAP {
            return Err("source-boundary row limit".into());
        }
        self.scan(count)
    }

    pub(crate) fn storage(&mut self, count: usize) -> Result<(), String> {
        self.charged_storage = self
            .charged_storage
            .checked_add(count)
            .ok_or("source-boundary storage overflow")?;
        if self.charged_storage > STORAGE_CAP {
            return Err("source-boundary storage limit".into());
        }
        Ok(())
    }

    fn source_read(&mut self, length: usize) -> Result<(), String> {
        if length > SOURCE_CAP {
            return Err("source-boundary source byte limit".into());
        }
        let bounded_read = length
            .checked_add(1)
            .ok_or("source-boundary read overflow")?;
        // Read/hash/text comparison plus a conservative String growth envelope.
        self.scan(
            bounded_read
                .checked_mul(4)
                .ok_or("source-boundary work overflow")?,
        )?;
        self.storage(
            bounded_read
                .checked_mul(2)
                .ok_or("source-boundary storage overflow")?,
        )?;
        self.source_bytes_read = self
            .source_bytes_read
            .checked_add(bounded_read)
            .ok_or("source-boundary read overflow")?;
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct SourceRange {
    pub(crate) normalized_start: u32,
    pub(crate) normalized_end: u32,
    pub(crate) original_start: u32,
    pub(crate) original_end: u32,
}

pub(crate) struct Parameter {
    pub(crate) ordinal: u32,
    pub(crate) hir_id: HirId,
    pub(crate) name: String,
    pub(crate) range: SourceRange,
}

/// Move-only and session-tied; construction remains private to this capture.
pub(crate) struct CapturedBitselect<'tcx> {
    pub(crate) identities: CanonicalFunctionIdentitiesV1,
    pub(crate) parameters: [Parameter; 3],
    pub(crate) operators: [SemanticSourceProvenanceV1; 3],
    pub(crate) operator_ranges: [SourceRange; 3],
    pub(crate) result_name: String,
    pub(crate) result_hir_id: HirId,
    pub(crate) initializer: SourceRange,
    pub(crate) original_sha256: [u8; 32],
    pub(crate) meter: ScanMeter,
    original: String,
    span: Span,
    _session: PhantomData<fn(TyCtxt<'tcx>) -> TyCtxt<'tcx>>,
}

impl<'tcx> CapturedBitselect<'tcx> {
    pub(crate) fn recheck_original(&mut self, tcx: TyCtxt<'tcx>) -> Result<(), String> {
        let file = checked_file(tcx, self.span, &mut self.meter)?;
        let original = original_source(&file, &mut self.meter)?;
        if original != self.original {
            return Err("source-boundary stale original bytes".into());
        }
        Ok(())
    }
}

fn checked_file(
    tcx: TyCtxt<'_>,
    span: Span,
    meter: &mut ScanMeter,
) -> Result<std::sync::Arc<SourceFile>, String> {
    if span.is_dummy() || span.from_expansion() || span.lo() > span.hi() {
        return Err("source-boundary requires direct original expression spans".into());
    }
    let files = tcx.sess.source_map().files();
    meter.rows(files.len())?;
    let mut selected = None;
    for file in files.iter() {
        if file.start_pos <= span.lo()
            && span.hi().0 - file.start_pos.0 <= file.normalized_source_len.0
        {
            if selected.replace(file.clone()).is_some() {
                return Err("source-boundary ambiguous source file".into());
            }
        }
    }
    selected.ok_or_else(|| "source-boundary source file unavailable".into())
}

fn original_source(file: &SourceFile, meter: &mut ScanMeter) -> Result<String, String> {
    meter.source_read(file.unnormalized_source_len as usize)?;
    let FileName::Real(name) = &file.name else {
        return Err("source-boundary requires a real source file".into());
    };
    let path = name
        .local_path()
        .ok_or("source-boundary source path unavailable")?;
    let original = super::read_original_source(file, path).map_err(str::to_owned)?;
    let normalized = file
        .src
        .as_deref()
        .ok_or("source-boundary normalized text unavailable")?;
    require_unchanged_normalization(&original, normalized)?;
    Ok(original)
}

fn require_unchanged_normalization(original: &str, normalized: &str) -> Result<(), String> {
    if original != normalized {
        return Err("source-boundary normalization changes original offsets".into());
    }
    Ok(())
}

fn range(file: &SourceFile, span: Span, meter: &mut ScanMeter) -> Result<SourceRange, String> {
    // Both coordinate helpers use binary searches over bounded source metadata.
    // Prepay a conservative fixed envelope (16-bit byte cap, two endpoints).
    meter.scan(128)?;
    if span.from_expansion() {
        return Err("source-boundary macro-generated expression".into());
    }
    let coordinates =
        super::coordinates::source_coordinates_v1(file, span).map_err(str::to_owned)?;
    if coordinates.original_start != coordinates.normalized_start
        || coordinates.original_end != coordinates.normalized_end
    {
        return Err("source-boundary normalization changes original offsets".into());
    }
    Ok(SourceRange {
        normalized_start: coordinates.normalized_start,
        normalized_end: coordinates.normalized_end,
        original_start: coordinates.original_start,
        original_end: coordinates.original_end,
    })
}

fn name(
    file: &SourceFile,
    source: &str,
    ident: rustc_span::Ident,
    meter: &mut ScanMeter,
) -> Result<(String, SourceRange), String> {
    let text = ident.name.as_str();
    if text.is_empty()
        || text.len() > 64
        || !text
            .bytes()
            .enumerate()
            .all(|(i, b)| b == b'_' || b.is_ascii_alphabetic() || i > 0 && b.is_ascii_digit())
    {
        return Err("source-boundary requires a plain bounded ASCII identifier".into());
    }
    meter.scan(text.len())?;
    meter.storage(text.len())?;
    let range = range(file, ident.span, meter)?;
    if source.get(range.original_start as usize..range.original_end as usize) != Some(text) {
        return Err("source-boundary identifier differs from original bytes".into());
    }
    Ok((text.to_owned(), range))
}

struct Tree<'tcx> {
    operators: [&'tcx Expr<'tcx>; 3],
    // a, inner b, mask, outer b, in fixed structural (not numeric SSA) order.
    leaves: [&'tcx Expr<'tcx>; 4],
}

fn tree<'tcx>(expr: &'tcx Expr<'tcx>) -> Option<Tree<'tcx>> {
    let ExprKind::Binary(outer, b_outer, and_expr) = expr.kind else {
        return None;
    };
    let ExprKind::Binary(and, xor_expr, mask) = and_expr.kind else {
        return None;
    };
    let ExprKind::Binary(xor, a, b_inner) = xor_expr.kind else {
        return None;
    };
    (outer.node == BinOpKind::BitXor
        && and.node == BinOpKind::BitAnd
        && xor.node == BinOpKind::BitXor)
        .then_some(Tree {
            operators: [xor_expr, and_expr, expr],
            leaves: [a, b_inner, mask, b_outer],
        })
}

fn parameter_id(typeck: &TypeckResults<'_>, expr: &Expr<'_>) -> Result<HirId, String> {
    let ExprKind::Path(ref path) = expr.kind else {
        return Err("source-boundary live-in is not a parameter path".into());
    };
    let Res::Local(id) = typeck.qpath_res(path, expr.hir_id) else {
        return Err("source-boundary live-in does not resolve to a local binding".into());
    };
    Ok(id)
}

pub(crate) fn capture<'tcx>(
    tcx: TyCtxt<'tcx>,
    closure: &AuthenticatedCollectedKernelClosureV1<'tcx>,
) -> Result<CapturedBitselect<'tcx>, String> {
    let mut meter = ScanMeter::default();
    meter.storage(std::mem::size_of::<CapturedBitselect<'tcx>>())?;
    let [root] = closure.roots.as_ref() else {
        return Err("source-boundary requires exactly one sealed root".into());
    };
    if root.role != CollectedFunctionRole::KernelEntry || !root.instance.args.is_empty() {
        return Err("source-boundary requires a direct nongeneric kernel root".into());
    }
    let local = root
        .instance
        .def_id()
        .as_local()
        .ok_or("source-boundary root is not local")?;
    let body = tcx
        .hir_maybe_body_owned_by(local)
        .ok_or("source-boundary local HIR body unavailable")?;
    let ExprKind::Block(block, _) = body.value.kind else {
        return Err("source-boundary requires a block body".into());
    };
    let file = checked_file(tcx, body.value.span, &mut meter)?;
    let original = original_source(&file, &mut meter)?;
    meter.rows(block.stmts.len())?;
    let mut selected = None;
    for (ordinal, statement) in block.stmts.iter().enumerate() {
        meter.scan(7)?;
        if let StmtKind::Let(binding) = statement.kind
            && let Some(initializer) = binding.init
            && let Some(tree) = tree(initializer)
        {
            if selected
                .replace((ordinal, binding, initializer, tree))
                .is_some()
            {
                return Err("source-boundary ambiguous bitselect initializers".into());
            }
        }
    }
    let (ordinal, binding, initializer, tree) =
        selected.ok_or("source-boundary exact bitselect initializer unavailable")?;
    let typeck = tcx.typeck(local);
    for expression in tree.operators.iter().chain(tree.leaves.iter()) {
        meter.scan(1)?;
        if typeck.expr_ty(expression) != tcx.types.u32
            || !typeck.expr_adjustments(expression).is_empty()
            || expression.span.from_expansion()
        {
            return Err("source-boundary expression is not direct unadjusted u32".into());
        }
        range(&file, expression.span, &mut meter)?;
    }
    let ids = [
        parameter_id(typeck, tree.leaves[0])?,
        parameter_id(typeck, tree.leaves[1])?,
        parameter_id(typeck, tree.leaves[2])?,
    ];
    if ids[0] == ids[1]
        || ids[0] == ids[2]
        || ids[1] == ids[2]
        || parameter_id(typeck, tree.leaves[3])? != ids[1]
    {
        return Err("source-boundary requires three distinct parameter bindings".into());
    }
    let parameters = ids.map(|id| -> Result<Parameter, String> {
        meter.rows(body.params.len())?;
        let mut selected = None;
        for (ordinal, parameter) in body.params.iter().enumerate() {
            if let PatKind::Binding(mode, binding_id, ident, None) = parameter.pat.kind
                && binding_id == id
            {
                if mode != BindingMode::NONE || typeck.pat_ty(parameter.pat) != tcx.types.u32 {
                    return Err("source-boundary parameter is not immutable plain u32".into());
                }
                let (name, range) = name(&file, &original, ident, &mut meter)?;
                if selected
                    .replace(Parameter {
                        ordinal: u32::try_from(ordinal)
                            .map_err(|_| "source-boundary parameter ordinal")?,
                        hir_id: id,
                        name,
                        range,
                    })
                    .is_some()
                {
                    return Err("source-boundary ambiguous HIR parameter".into());
                }
            }
        }
        selected.ok_or_else(|| "source-boundary local alias is not a parameter".into())
    });
    let [a, b, mask] = parameters;
    let parameters = [a?, b?, mask?];
    if ordinal != 0 || binding.els.is_some() {
        return Err("source-boundary initializer must be the first top-level let".into());
    }
    let PatKind::Binding(BindingMode::NONE, result_hir_id, result_ident, None) = binding.pat.kind
    else {
        return Err("source-boundary result requires a plain immutable binding".into());
    };
    let (result_name, _) = name(&file, &original, result_ident, &mut meter)?;
    if parameters
        .iter()
        .any(|parameter| parameter.name == result_name)
    {
        return Err("source-boundary result shadows an input name".into());
    }
    let operators = tree.operators.map(|expr| {
        canonical_source_provenance_v1(tcx, expr.span, 0)
            .map(|captured| captured.provenance())
            .map_err(|error| format!("source-boundary operator provenance: {error:?}"))
    });
    let [xor, and, outer] = operators;
    let operator_ranges = tree
        .operators
        .map(|expr| range(&file, expr.span, &mut meter));
    let [xor_range, and_range, outer_range] = operator_ranges;
    Ok(CapturedBitselect {
        identities: canonical_function_identities_v1(tcx, root.instance),
        parameters,
        operators: [xor?, and?, outer?],
        operator_ranges: [xor_range?, and_range?, outer_range?],
        result_name,
        result_hir_id,
        initializer: range(&file, initializer.span, &mut meter)?,
        original_sha256: Sha256::digest(original.as_bytes()).into(),
        meter,
        original,
        span: initializer.span,
        _session: PhantomData,
    })
}

#[test]
fn source_boundary_bounds_are_cumulative_and_fail_before_extra_work() {
    let mut meter = ScanMeter::default();
    meter.scan(WORK_CAP).unwrap();
    assert!(meter.scan(1).unwrap_err().contains("work limit"));
    let mut meter = ScanMeter::default();
    meter.storage(STORAGE_CAP).unwrap();
    assert!(meter.storage(1).unwrap_err().contains("storage limit"));
    assert!(ScanMeter::default().source_read(SOURCE_CAP + 1).is_err());
    assert!(ScanMeter::default().rows(ROW_CAP + 1).is_err());
    let mut meter = ScanMeter::default();
    meter.source_read(SOURCE_CAP).unwrap();
    meter.source_read(SOURCE_CAP).unwrap();
    assert_eq!(meter.source_bytes_read, 2 * (SOURCE_CAP + 1));
}

#[test]
fn source_boundary_never_substitutes_normalized_bytes_for_original_bytes() {
    require_unchanged_normalization("// λ\nlet x = 1;\n", "// λ\nlet x = 1;\n").unwrap();
    for original in ["\u{feff}a\n", "a\r\n", "\u{feff}a\r\n"] {
        assert_eq!(
            require_unchanged_normalization(original, "a\n").unwrap_err(),
            "source-boundary normalization changes original offsets"
        );
    }
}
