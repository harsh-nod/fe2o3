//! Bounded joins to the original P0 HIR, MIR and admitted semantic call.
use super::*;
use crate::production_tiled_region_source_v1::{Captured, Role, role_index};
use crate::rustc_semantic_adapter_v1::{
    canonical_function_identities_v1, canonical_source_provenance_v1, rustc_block_identity_v1,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use rustc_hir::{
    BodyId, Expr, ExprKind, ItemId, QPath,
    def::Res,
    intravisit::{self, Visitor},
};
use rustc_middle::{
    mir,
    ty::{self, EarlyBinder, Instance, TypeckResults, TypingEnv},
};
use rustc_span::{Ident, SourceFile, Span};
use std::{ops::ControlFlow, sync::Arc};

pub(super) struct Anchor {
    pub file: Arc<SourceFile>,
    pub body: Span,
    pub selected: Span,
    pub operands: [Ident; 4],
}
struct Scan<'a, 'tcx> {
    result: &'tcx Expr<'tcx>,
    helper: &'a str,
    parent: Option<&'tcx Expr<'tcx>>,
    nodes: usize,
    depth: usize,
}
const _: () =
    assert!(size_of::<Scan<'_, '_>>() + size_of::<Anchor>() + 8 * size_of::<usize>() < 512);
impl Scan<'_, '_> {
    fn tick(&mut self) -> ControlFlow<Error> {
        self.nodes += 1;
        if self.nodes > HIR_NODES {
            ControlFlow::Break(Error::refused("BF16 publisher HIR node bound"))
        } else {
            ControlFlow::Continue(())
        }
    }
    fn enter(&mut self) -> ControlFlow<Error> {
        self.tick()?;
        self.depth += 1;
        if self.depth > HIR_DEPTH {
            ControlFlow::Break(Error::refused("BF16 publisher HIR depth bound"))
        } else {
            ControlFlow::Continue(())
        }
    }
}
impl<'tcx> Visitor<'tcx> for Scan<'_, 'tcx> {
    type Result = ControlFlow<Error>;
    fn visit_name(&mut self, name: rustc_span::Symbol) -> Self::Result {
        self.tick()?;
        if name.as_str() == self.helper {
            return ControlFlow::Break(Error::refused("BF16 helper collides in actual HIR"));
        }
        ControlFlow::Continue(())
    }
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) -> Self::Result {
        self.enter()?;
        if matches!(expr.kind, ExprKind::Closure(_)) {
            return ControlFlow::Break(Error::refused("BF16 publisher refuses closure capture"));
        }
        if let ExprKind::MethodCall(_, receiver, arguments, _) = expr.kind
            && std::ptr::eq(receiver, self.result)
        {
            if !arguments.is_empty() || self.parent.replace(expr).is_some() {
                return ControlFlow::Break(Error::refused("BF16 conversion parent is ambiguous"));
            }
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
    fn visit_nested_body(&mut self, _: BodyId) -> Self::Result {
        ControlFlow::Break(Error::refused("BF16 publisher refuses nested body"))
    }
    fn visit_nested_item(&mut self, _: ItemId) -> Self::Result {
        ControlFlow::Break(Error::refused("BF16 publisher refuses existing local item"))
    }
}
fn resolve_parent<'tcx>(
    captured: &Captured<'tcx>,
    typeck: &TypeckResults<'tcx>,
    parent: &Expr<'tcx>,
) -> Result<Instance<'tcx>> {
    let def = typeck
        .type_dependent_def_id(parent.hir_id)
        .ok_or_else(|| Error::refused("BF16 conversion has no actual method definition"))?;
    let raw = ty::Ty::new_fn_def(captured.tcx, def, typeck.node_args(parent.hir_id));
    let resolved = captured
        .instance
        .try_instantiate_mir_and_normalize_erasing_regions(
            captured.tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(raw),
        )
        .map_err(|_| Error::refused("BF16 conversion normalization refused"))?;
    let ty::FnDef(def, args) = *resolved.kind() else {
        return Err(Error::refused("BF16 conversion is not a direct function"));
    };
    Instance::try_resolve(captured.tcx, TypingEnv::fully_monomorphized(), def, args)
        .map_err(|_| Error::refused("BF16 conversion resolution refused"))?
        .ok_or_else(|| Error::refused("BF16 conversion Instance is absent"))
}
fn inside(outer: Span, inner: Span) -> bool {
    !inner.is_dummy()
        && !inner.from_expansion()
        && inner.ctxt() == outer.ctxt()
        && outer.lo() <= inner.lo()
        && inner.lo() < inner.hi()
        && inner.hi() <= outer.hi()
}
fn local<'tcx>(typeck: &TypeckResults<'tcx>, expression: &Expr<'tcx>) -> Result<Ident> {
    let ExprKind::Path(ref path) = expression.kind else {
        return Err(Error::refused(
            "BF16 publisher operand is not a direct local path",
        ));
    };
    let QPath::Resolved(None, resolved) = path else {
        return Err(Error::refused(
            "BF16 publisher operand is not an unqualified path",
        ));
    };
    if !matches!(typeck.qpath_res(path, expression.hir_id), Res::Local(_))
        || resolved.segments.len() != 1
        || resolved.segments[0].args.is_some()
    {
        return Err(Error::refused(
            "BF16 publisher operand path is not one local identifier",
        ));
    }
    let ident = resolved.segments[0].ident;
    text::identifier(ident.name.as_str())?;
    if ident.span != expression.span || ident.span.is_dummy() || ident.span.from_expansion() {
        return Err(Error::refused("BF16 publisher local spelling is indirect"));
    }
    Ok(ident)
}
pub(super) fn capture(
    source: &SourceOwnedBf16MfmaRegionV1<'_, '_>,
    helper: &str,
) -> Result<Anchor> {
    let captured = &source.seed.captured.payload[0];
    let tcx = captured.tcx;
    let local_root = captured
        .instance
        .def_id()
        .as_local()
        .ok_or_else(|| Error::refused("BF16 publisher root is not local"))?;
    // The original immutable MIR remains borrowed by Captured. Pointer equality
    // reobserves that exact query result, not a reserialized/detached body.
    if !captured.instance.args.is_empty()
        || tcx
            .hir_maybe_body_owned_by(local_root)
            .is_none_or(|body| !std::ptr::eq(body, captured.hir))
        || !std::ptr::eq(tcx.instance_mir(captured.instance.def), captured.mir)
        || canonical_function_identities_v1(tcx, captured.instance) != captured.identities
        || captured.mir.basic_blocks.len() > BLOCKS
    {
        return Err(Error::refused("BF16 publisher current root owner differs"));
    }
    let ExprKind::Block(body, _) = captured.hir.value.kind else {
        return Err(Error::refused(
            "BF16 publisher requires a direct block body",
        ));
    };
    if captured.hir.value.span.from_expansion() || !source.source().contains(body.span) {
        return Err(Error::refused(
            "BF16 publisher body is not current direct source",
        ));
    }
    let module = tcx.parent_module_from_def_id(local_root);
    let children = tcx.module_children_local(module.to_local_def_id());
    if children.len() > NAMES
        || children
            .iter()
            .any(|child| child.ident.name.as_str() == helper)
    {
        return Err(Error::refused(
            "BF16 publisher enclosing namespace bound or collision",
        ));
    }
    let result = source.expressions[role_index(Role::Result)];
    let mut scan = Scan {
        result,
        helper,
        parent: None,
        nodes: 0,
        depth: 0,
    };
    if let ControlFlow::Break(error) = scan.visit_body(captured.hir) {
        return Err(error);
    }
    let parent = scan
        .parent
        .ok_or_else(|| Error::refused("BF16 direct conversion parent absent"))?;
    if !source.source().contains(parent.span) || !inside(body.span, parent.span) {
        return Err(Error::refused(
            "BF16 parent call is not in the actual source body",
        ));
    }
    let typeck = tcx.typeck(local_root);
    let parent_instance = resolve_parent(captured, typeck, parent)?;
    let ExprKind::MethodCall(_, receiver, arguments, _) = result.kind else {
        return Err(Error::refused(
            "BF16 selected MFMA is not a direct method call",
        ));
    };
    let [lhs, rhs, zero] = arguments else {
        return Err(Error::refused("BF16 selected MFMA argument count differs"));
    };
    let actual = [receiver, lhs, rhs, zero];
    for (index, role) in [Role::Context, Role::Lhs, Role::Rhs, Role::Zero]
        .into_iter()
        .enumerate()
    {
        if typeck.expr_ty(actual[index]) != typeck.expr_ty(source.expressions[role_index(role)])
            || (index != 0 && !typeck.expr_adjustments(actual[index]).is_empty())
        {
            return Err(Error::refused(
                "BF16 local nominal type or argument adjustment differs",
            ));
        }
    }
    let operands = [
        local(typeck, receiver)?,
        local(typeck, lhs)?,
        local(typeck, rhs)?,
        local(typeck, zero)?,
    ];
    let mfma = captured.rows[role_index(Role::Result)]
        .as_ref()
        .ok_or_else(|| Error::refused("BF16 captured MFMA row absent"))?;
    let mut raw_selected = None;
    for (raw, block) in captured.mir.basic_blocks.iter_enumerated() {
        let terminator = block.terminator();
        if terminator.source_info.span != parent.span {
            continue;
        }
        let mir::TerminatorKind::Call {
            func,
            args,
            destination,
            target: Some(_),
            fn_span,
            unwind: mir::UnwindAction::Continue | mir::UnwindAction::Unreachable,
            ..
        } = &terminator.kind
        else {
            return Err(Error::refused("BF16 conversion raw call shape"));
        };
        if args.len() != 1
            || !destination.projection.is_empty()
            || !inside(parent.span, *fn_span)
            || crate::production_semantic_body_v1::resolve_direct_call_v1(
                tcx,
                captured.instance,
                captured.mir,
                func,
            )
            .map_err(Error::refused)?
                != parent_instance
        {
            return Err(Error::refused(
                "BF16 conversion HIR/raw Instance or span differs",
            ));
        }
        let mir::Operand::Move(place) = &args[0].node else {
            return Err(Error::refused(
                "BF16 conversion must move the actual MFMA result",
            ));
        };
        if !place.projection.is_empty()
            || place.local.as_u32() != mfma.raw_destination.raw
            || raw_selected.replace(raw.as_u32()).is_some()
        {
            return Err(Error::refused(
                "BF16 conversion raw result or uniqueness differs",
            ));
        }
    }
    let raw =
        raw_selected.ok_or_else(|| Error::refused("BF16 exact conversion MIR call absent"))?;
    let owner = source
        .emission()
        .original()
        .semantic_ssa()
        .source_semantic();
    let function = source.emission().source_function();
    if function.blocks().len() > BLOCKS || owner.types().len() > 4096 {
        return Err(Error::refused("BF16 conversion semantic roster bound"));
    }
    let identity =
        rustc_block_identity_v1(captured.identities.function(), captured.mir_sha256, raw);
    let mut selected = None;
    for block in function.blocks() {
        if block.identity() == identity && selected.replace(block).is_some() {
            return Err(Error::refused(
                "BF16 conversion semantic block is ambiguous",
            ));
        }
    }
    let block = selected.ok_or_else(|| Error::refused("BF16 conversion semantic block absent"))?;
    let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
        return Err(Error::refused(
            "BF16 conversion admitted terminator differs",
        ));
    };
    let Some(SemanticCallableDeclV1::CompilerIntrinsic {
        binding, operation, ..
    }) = owner.callables().get(call.callee().index() as usize)
    else {
        return Err(Error::refused(
            "BF16 conversion is not an authenticated intrinsic",
        ));
    };
    if !matches!(
        operation,
        SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorIntoValues { .. }
    ) || binding.identity() != canonical_function_identities_v1(tcx, parent_instance).function()
        || call.arguments().len() != 1
        || !matches!(
            call.unwind(),
            SemanticUnwindActionV1::Continue | SemanticUnwindActionV1::Unreachable
        )
    {
        return Err(Error::refused(
            "BF16 conversion actual provider/transport differs",
        ));
    }
    let SemanticOperandV1::Move(place) = &call.arguments()[0] else {
        return Err(Error::refused(
            "BF16 admitted conversion must move its result",
        ));
    };
    if !place.projections().is_empty()
        || place.local() != mfma.raw_destination.semantic
        || place.ty()
            != source
                .emission()
                .source_call()
                .destination()
                .ok_or_else(|| Error::refused("BF16 MFMA destination absent"))?
                .place()
                .ty()
    {
        return Err(Error::refused(
            "BF16 conversion semantic MFMA input differs",
        ));
    }
    let destination = call
        .destination()
        .ok_or_else(|| Error::refused("BF16 conversion return absent"))?;
    if !destination.place().projections().is_empty() {
        return Err(Error::refused("BF16 conversion output is projected"));
    }
    let output = owner
        .types()
        .get(destination.place().ty().index() as usize)
        .ok_or_else(|| Error::refused("BF16 conversion output type absent"))?;
    let SemanticTypeShapeV1::Array { element, length: 4 } = output.shape() else {
        return Err(Error::refused(
            "BF16 conversion output is not a four-value array",
        ));
    };
    if !matches!(
        owner
            .types()
            .get(element.index() as usize)
            .map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float {
            bits: 32
        }))
    ) {
        return Err(Error::refused("BF16 conversion output is not f32"));
    }
    // Bounded file lookup precedes the existing provenance helper.
    let file = text::source_file(tcx, body.span)?;
    if !inside(body.span, result.span)
        || canonical_source_provenance_v1(tcx, parent.span, 2)
            .map_err(|_| Error::refused("BF16 conversion source provenance unavailable"))?
            .provenance()
            != block.terminator().source()
    {
        return Err(Error::refused(
            "BF16 conversion original source provenance differs",
        ));
    }
    Ok(Anchor {
        file,
        body: body.span,
        selected: parent.span,
        operands,
    })
}
