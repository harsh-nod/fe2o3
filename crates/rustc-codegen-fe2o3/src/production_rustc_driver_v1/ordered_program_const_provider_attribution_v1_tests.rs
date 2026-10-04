//! Post-clock attribution to the actual validated root and trusted flat macro.
//! No result here is artifact/launch authority. Wrapper/repeat/const-if macros
//! and ambiguous HIR inventories are deliberately outside this first probe.
use super::{DefId, Serialize, TyCtxt, ledger::Snapshot};
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::{CRATE_DEF_INDEX, LocalDefId};
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{BodyId, Expr, ExprKind, ItemId, ItemKind};
use rustc_middle::ty::{Instance, TypeckResults};
use rustc_span::{ExpnKind, Pos, Span};
use std::ops::ControlFlow;

const NODES: usize = 4096;
const DEPTH: usize = 64;
const SOURCE_BYTES: usize = 1_048_576;
const CALLSITE_BYTES: usize = 65_536;

#[derive(Serialize)]
pub(super) struct Attribution {
    pub(super) provider_ordinal: usize,
    pub(super) inclusive_ctfe_and_checking_ns: String,
    const_definition: [u32; 2],
    words_definition: [u32; 2],
    function_definition: [u32; 2],
    trusted_macro_definition: [u32; 2],
    trusted_helper_definition: [u32; 2],
    trusted_marker_definition: [u32; 2],
    canonical_function_identity: String,
    normalized_source_bytes: usize,
    normalized_source_sha256: String,
    normalized_callsite_start: usize,
    normalized_callsite_end: usize,
    normalized_callsite_sha256: String,
    actual_hir_marker_calls: usize,
    nontrivial_const_provider_path: bool,
    timing_is_inclusive: bool,
    nested_rows_summed: bool,
    warm_generation: bool,
}
fn id(definition: DefId) -> [u32; 2] {
    [definition.krate.as_u32(), definition.index.as_u32()]
}
fn site(span: Span, expected: DefId) -> Result<Span, &'static str> {
    if span.is_dummy() || !span.from_expansion() {
        return Err("direct trusted macro expansion required");
    }
    let data = span.ctxt().outer_expn_data();
    if !matches!(data.kind, ExpnKind::Macro(..))
        || data.macro_def_id != Some(expected)
        || data.call_site.is_dummy()
        || data.call_site.from_expansion()
        || data.call_site.lo() >= data.call_site.hi()
    {
        return Err("wrapper or substituted macro expansion refused");
    }
    Ok(data.call_site)
}
fn child(tcx: TyCtxt<'_>, parent: DefId, name: &str) -> Result<(DefKind, DefId), &'static str> {
    let children = tcx.module_children(parent);
    if children.len() > NODES {
        return Err("trusted export roster exceeds bound");
    }
    let mut found = None;
    for entry in children {
        if entry.ident.name.as_str() == name {
            let Res::Def(kind, definition) = entry.res else {
                return Err("trusted export has non-definition resolution");
            };
            if definition.krate != parent.krate || found.replace((kind, definition)).is_some() {
                return Err("trusted export is ambiguous or crosses crate");
            }
        }
    }
    found.ok_or("trusted export absent")
}
fn trusted(tcx: TyCtxt<'_>) -> Result<(DefId, DefId, DefId), &'static str> {
    let marker = crate::trusted_device_items::definition(
        tcx,
        crate::trusted_device_items::TrustedDeviceItem::AmdGpuOrderedProgramE32,
    )
    .ok_or("authenticated ordered-program marker absent")?;
    let root = DefId {
        krate: marker.krate,
        index: CRATE_DEF_INDEX,
    };
    let (kind, macro_def) = child(tcx, root, "amdgpu_ordered_program")?;
    if !matches!(kind, DefKind::Macro(..)) {
        return Err("trusted macro kind differs");
    }
    let (kind, module) = child(tcx, root, "ordered_program")?;
    if kind != DefKind::Mod {
        return Err("trusted helper module kind differs");
    }
    let (kind, helper) = child(tcx, module, "__checked_ordered_program_words_v1")?;
    if kind != DefKind::Fn {
        return Err("trusted helper kind differs");
    }
    Ok((marker, macro_def, helper))
}
fn enclosing_function(tcx: TyCtxt<'_>, mut definition: DefId) -> Result<LocalDefId, &'static str> {
    for _ in 0..DEPTH {
        definition = tcx
            .opt_parent(definition)
            .ok_or("local const parent missing")?;
        if !definition.is_local() {
            return Err("const owner left local crate");
        }
        match tcx.def_kind(definition) {
            DefKind::Fn => return definition.as_local().ok_or("root function is not local"),
            DefKind::Const {
                is_type_const: false,
            }
            | DefKind::AnonConst => {}
            _ => return Err("const is not directly inside a supported function owner"),
        }
    }
    Err("const ancestry exceeds bound")
}

struct Scan<'tcx> {
    tcx: TyCtxt<'tcx>,
    typeck: &'tcx TypeckResults<'tcx>,
    macro_def: DefId,
    marker: DefId,
    expected_site: Span,
    packed: Option<LocalDefId>,
    words: Option<LocalDefId>,
    count: Option<LocalDefId>,
    marker_calls: usize,
    nodes: usize,
    depth: usize,
}
impl Scan<'_> {
    fn tick(&mut self) -> ControlFlow<&'static str> {
        self.nodes += 1;
        if self.nodes > NODES {
            ControlFlow::Break("HIR node bound exceeded")
        } else {
            ControlFlow::Continue(())
        }
    }
    fn enter(&mut self) -> ControlFlow<&'static str> {
        self.tick()?;
        self.depth += 1;
        if self.depth > DEPTH {
            ControlFlow::Break("HIR depth bound exceeded")
        } else {
            ControlFlow::Continue(())
        }
    }
    fn check_site(&self, span: Span) -> ControlFlow<&'static str> {
        match site(span, self.macro_def) {
            Ok(current) if current == self.expected_site => ControlFlow::Continue(()),
            _ => ControlFlow::Break("HIR item is not at the selected trusted macro site"),
        }
    }
}
impl<'tcx> Visitor<'tcx> for Scan<'tcx> {
    type Result = ControlFlow<&'static str>;
    fn visit_name(&mut self, _name: rustc_span::Symbol) -> Self::Result {
        self.tick()
    }
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) -> Self::Result {
        self.enter()?;
        if matches!(expr.kind, ExprKind::Closure(_)) {
            return ControlFlow::Break("nested closure refused");
        }
        if let ExprKind::Call(callee, args) = expr.kind
            && let ExprKind::Path(ref path) = callee.kind
            && let Res::Def(_, definition) = self.typeck.qpath_res(path, callee.hir_id)
            && definition == self.marker
        {
            self.check_site(expr.span)?;
            if args.len() != 8
                || self.typeck.expr_ty(expr) != self.tcx.types.u32
                || !self.typeck.expr_adjustments(expr).is_empty()
                || self.marker_calls != 0
            {
                return ControlFlow::Break("actual HIR marker call shape or uniqueness differs");
            }
            self.marker_calls += 1;
        }
        let result = intravisit::walk_expr(self, expr);
        self.depth -= 1;
        result
    }
    fn visit_pat(&mut self, pat: &'tcx rustc_hir::Pat<'tcx>) -> Self::Result {
        self.enter()?;
        let result = intravisit::walk_pat(self, pat);
        self.depth -= 1;
        result
    }
    fn visit_ty(&mut self, ty: &'tcx rustc_hir::Ty<'tcx, rustc_hir::AmbigArg>) -> Self::Result {
        self.enter()?;
        let result = intravisit::walk_ty(self, ty);
        self.depth -= 1;
        result
    }
    fn visit_block(&mut self, block: &'tcx rustc_hir::Block<'tcx>) -> Self::Result {
        self.tick()?;
        intravisit::walk_block(self, block)
    }
    fn visit_stmt(&mut self, statement: &'tcx rustc_hir::Stmt<'tcx>) -> Self::Result {
        self.tick()?;
        intravisit::walk_stmt(self, statement)
    }
    fn visit_nested_body(&mut self, body: BodyId) -> Self::Result {
        self.tick()?;
        // Const-generic projections remain opaque; their real const provider
        // rows are retained separately. No compiler query is repeated here.
        self.check_site(self.tcx.hir_body(body).value.span)
    }
    fn visit_nested_item(&mut self, id: ItemId) -> Self::Result {
        self.tick()?;
        let item = self.tcx.hir_item(id);
        self.check_site(item.span)?;
        let ItemKind::Const(ident, _, _, _) = item.kind else {
            return ControlFlow::Break("non-macro local item refused");
        };
        let slot = match ident.name.as_str() {
            "__PACKED" => &mut self.packed,
            "__WORDS" => &mut self.words,
            "__COUNT" => &mut self.count,
            _ => return ControlFlow::Break("non-flat macro local constant refused"),
        };
        if slot.replace(item.owner_id.def_id).is_some() {
            return ControlFlow::Break("duplicate macro local constant refused");
        }
        ControlFlow::Continue(())
    }
}
pub(super) fn capture(
    tcx: TyCtxt<'_>,
    snapshot: Snapshot,
    expected_function: [u8; 32],
) -> Result<Attribution, &'static str> {
    if !snapshot.ready() {
        return Err("provider recording did not close cleanly");
    }
    // No naming-only acceptance: this merely locates the candidate that the
    // authenticated function, expansion, helper and complete-byte joins check.
    let mut packed = None;
    for row in snapshot.rows[..snapshot.count].iter().flatten() {
        if matches!(
            tcx.def_kind(row.key.definition),
            DefKind::Const {
                is_type_const: false
            }
        ) && tcx.item_name(row.key.definition).as_str() == "__PACKED"
        {
            if packed.replace(row.key.definition).is_some() {
                return Err("multiple candidate packed provider rows refused");
            }
        }
    }
    let packed = packed.ok_or("no actual packed-constant provider call; not a zero timing")?;
    let (ordinal, row) = snapshot.unique_completed(packed)?;
    let local_packed = packed.as_local().ok_or("packed constant is not local")?;
    let function = enclosing_function(tcx, packed)?;
    let identities = crate::rustc_semantic_adapter_v1::canonical_function_identities_v1(
        tcx,
        Instance::mono(tcx, function.to_def_id()),
    );
    if identities.function().as_bytes() != &expected_function {
        return Err("provider constant is not owned by the actual validated root");
    }
    let (marker, macro_def, helper) = trusted(tcx)?;
    let packed_body = tcx
        .hir_maybe_body_owned_by(local_packed)
        .ok_or("packed HIR body absent")?;
    let callsite = site(packed_body.value.span, macro_def)?;
    let function_body = tcx
        .hir_maybe_body_owned_by(function)
        .ok_or("actual root HIR body absent")?;
    let ExprKind::Block(block, _) = function_body.value.kind else {
        return Err("root is not a direct block body");
    };
    if block.span.is_dummy()
        || block.span.from_expansion()
        || function_body.value.span.from_expansion()
        || block.span.lo() > callsite.lo()
        || block.span.hi() < callsite.hi()
    {
        return Err("selected macro does not lie inside the direct actual root body");
    }
    let mut scan = Scan {
        tcx,
        typeck: tcx.typeck(function),
        macro_def,
        marker,
        expected_site: callsite,
        packed: None,
        words: None,
        count: None,
        marker_calls: 0,
        nodes: 0,
        depth: 0,
    };
    if let ControlFlow::Break(error) = scan.visit_body(function_body) {
        return Err(error);
    }
    let words = scan.words.ok_or("actual macro words constant absent")?;
    if scan.packed != Some(local_packed)
        || scan.count.is_none()
        || scan.marker_calls != 1
        || enclosing_function(tcx, words.to_def_id())? != function
    {
        return Err("flat macro constant inventory or marker join differs");
    }
    let ExprKind::Call(callee, arguments) = packed_body.value.kind else {
        return Err("packed root is not the original helper expression");
    };
    if arguments.len() != 1 {
        return Err("packed helper argument count differs");
    }
    let packed_typeck = tcx.typeck(local_packed);
    let ExprKind::Path(ref path) = callee.kind else {
        return Err("packed helper is not a resolved path");
    };
    if packed_typeck.qpath_res(path, callee.hir_id) != Res::Def(DefKind::Fn, helper) {
        return Err("packed expression does not call the authenticated original helper");
    }
    let ExprKind::Path(ref path) = arguments[0].kind else {
        return Err("packed helper argument is not the actual words constant");
    };
    if packed_typeck.qpath_res(path, arguments[0].hir_id)
        != Res::Def(
            DefKind::Const {
                is_type_const: false,
            },
            words.to_def_id(),
        )
    {
        return Err("packed helper words DefId differs");
    }
    // The selected provider itself already requested this query. This is a
    // post-clock attribution read, not a second eval_to_allocation_raw call.
    if tcx.trivial_const(packed).is_some() {
        return Err("selected constant took a trivial rather than interpreter path");
    }
    let files = tcx.sess.source_map().files();
    if files.len() > NODES {
        return Err("source-map roster bound exceeded");
    }
    let mut selected = None;
    for file in files.iter() {
        let length = u32::try_from(file.normalized_source_len.to_usize())
            .map_err(|_| "source-map length conversion")?;
        let end = file
            .start_pos
            .0
            .checked_add(length)
            .ok_or("source-map end overflow")?;
        if file.start_pos.0 <= block.span.lo().0
            && end >= block.span.hi().0
            && file.start_pos.0 <= callsite.lo().0
            && end >= callsite.hi().0
        {
            if selected.replace(file).is_some() {
                return Err("source-map interval is ambiguous");
            }
        }
    }
    let file = selected.ok_or("actual source-file interval absent")?;
    let source = file
        .src
        .as_ref()
        .ok_or("actual normalized source text absent")?;
    if source.len() > SOURCE_BYTES || source.len() != file.normalized_source_len.to_usize() {
        return Err("actual normalized source byte bound differs");
    }
    let start = usize::try_from(
        callsite
            .lo()
            .0
            .checked_sub(file.start_pos.0)
            .ok_or("callsite start underflow")?,
    )
    .map_err(|_| "callsite start conversion")?;
    let end = usize::try_from(
        callsite
            .hi()
            .0
            .checked_sub(file.start_pos.0)
            .ok_or("callsite end underflow")?,
    )
    .map_err(|_| "callsite end conversion")?;
    let selected_text = source
        .get(start..end)
        .ok_or("callsite is not a valid UTF8 interval")?;
    if selected_text.is_empty() || selected_text.len() > CALLSITE_BYTES {
        return Err("callsite text bound differs");
    }
    Ok(Attribution {
        provider_ordinal: ordinal,
        inclusive_ctfe_and_checking_ns: row.elapsed_ns.ok_or("selected clock absent")?.to_string(),
        const_definition: id(packed),
        words_definition: id(words.to_def_id()),
        function_definition: id(function.to_def_id()),
        trusted_macro_definition: id(macro_def),
        trusted_helper_definition: id(helper),
        trusted_marker_definition: id(marker),
        canonical_function_identity: super::hex32(&expected_function),
        normalized_source_bytes: source.len(),
        normalized_source_sha256: super::digest(source.as_bytes()),
        normalized_callsite_start: start,
        normalized_callsite_end: end,
        normalized_callsite_sha256: super::digest(selected_text.as_bytes()),
        actual_hir_marker_calls: scan.marker_calls,
        nontrivial_const_provider_path: true,
        timing_is_inclusive: true,
        nested_rows_summed: false,
        warm_generation: false,
    })
}
