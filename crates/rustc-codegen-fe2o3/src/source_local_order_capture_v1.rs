//! Compiler-private live source witness shared by the bounded recipe entry and tests.
//! Intended child of bitselect_feasibility::retained; reuses its bounded I/O.
use super::*;

#[cfg(test)]
#[path = "source_local_order_recipe_codec_v1_tests.rs"]
pub(crate) mod recipes;

pub(crate) struct CapturedLocalOrder<'tcx> {
    pub(crate) identities: CanonicalFunctionIdentitiesV1,
    pub(crate) parameters: [Parameter; 4],
    pub(crate) operators: [SemanticSourceProvenanceV1; 3],
    pub(crate) initializer: SourceRange,
    pub(crate) original_sha256: [u8; 32],
    pub(crate) meter: ScanMeter,
    span: Span,
    _session: PhantomData<fn(TyCtxt<'tcx>) -> TyCtxt<'tcx>>,
}

struct OrderTree<'tcx> {
    operators: [&'tcx Expr<'tcx>; 3],
    leaves: [&'tcx Expr<'tcx>; 4],
}

fn order_tree<'tcx>(expression: &'tcx Expr<'tcx>) -> Option<OrderTree<'tcx>> {
    let ExprKind::Binary(and, left, right) = expression.kind else {
        return None;
    };
    let ExprKind::Binary(xor, a, b) = left.kind else {
        return None;
    };
    let ExprKind::Binary(or, c, d) = right.kind else {
        return None;
    };
    (and.node == BinOpKind::BitAnd && xor.node == BinOpKind::BitXor && or.node == BinOpKind::BitOr)
        .then_some(OrderTree {
            operators: [left, right, expression],
            leaves: [a, b, c, d],
        })
}

pub(crate) fn capture_local_order<'tcx>(
    tcx: TyCtxt<'tcx>,
    closure: &AuthenticatedCollectedKernelClosureV1<'tcx>,
    input: &RetainedInput,
) -> Result<CapturedLocalOrder<'tcx>, String> {
    let mut meter = ScanMeter::default();
    meter.storage(std::mem::size_of::<CapturedLocalOrder<'tcx>>())?;
    require_baseline_profile(closure, &mut meter)?;
    let [root] = closure.roots.as_ref() else {
        return Err("local-order requires one sealed root".into());
    };
    if root.role != CollectedFunctionRole::KernelEntry || !root.instance.args.is_empty() {
        return Err("local-order requires a direct nongeneric root".into());
    }
    let local = root
        .instance
        .def_id()
        .as_local()
        .ok_or("local-order root is not local")?;
    let body = tcx
        .hir_maybe_body_owned_by(local)
        .ok_or("local-order HIR body unavailable")?;
    let ExprKind::Block(block, _) = body.value.kind else {
        return Err("local-order requires block body".into());
    };
    let file = checked_file(tcx, body.value.span, &mut meter)?;
    require_file(&file, input, &mut meter)?;
    // This repeats the bounded original-file/hash/normalization check under the
    // existing scan meter. The descriptor binds exact retained file bytes;
    // semantic authority remains with the live compiler witness/owner.
    let original = original_source(&file, &mut meter)?;
    meter.scan(original.len())?;
    if original.as_bytes() != input.original() {
        return Err("local-order original bytes mismatch".into());
    }
    meter.rows(block.stmts.len())?;
    let mut selected = None;
    for (ordinal, statement) in block.stmts.iter().enumerate() {
        meter.scan(7)?;
        if let StmtKind::Let(binding) = statement.kind
            && let Some(initializer) = binding.init
            && let Some(tree) = order_tree(initializer)
            && selected
                .replace((ordinal, binding, initializer, tree))
                .is_some()
        {
            return Err("local-order ambiguous source initializers".into());
        }
    }
    let (ordinal, binding, initializer, tree) =
        selected.ok_or("local-order exact source initializer unavailable")?;
    let typeck = tcx.typeck(local);
    for expression in tree.operators.iter().chain(tree.leaves.iter()) {
        meter.scan(1)?;
        if typeck.expr_ty(expression) != tcx.types.u32
            || !typeck.expr_adjustments(expression).is_empty()
            || expression.span.from_expansion()
        {
            return Err("local-order requires direct unadjusted u32 expressions".into());
        }
        range(&file, expression.span, &mut meter)?;
    }
    let ids = tree.leaves.map(|leaf| parameter_id(typeck, leaf));
    let [a, b, c, d] = ids;
    let ids = [a?, b?, c?, d?];
    for (index, id) in ids.iter().enumerate() {
        meter.scan(4 - index)?;
        if ids[index + 1..].contains(id) {
            return Err("local-order requires four distinct parameter bindings".into());
        }
    }
    let parameters = ids.map(|id| -> Result<Parameter, String> {
        meter.rows(body.params.len())?;
        let mut selected = None;
        for (ordinal, parameter) in body.params.iter().enumerate() {
            if let PatKind::Binding(mode, binding_id, ident, None) = parameter.pat.kind
                && binding_id == id
            {
                if mode != BindingMode::NONE || typeck.pat_ty(parameter.pat) != tcx.types.u32 {
                    return Err("local-order parameter is not immutable plain u32".into());
                }
                let (name, range) = name(&file, &original, ident, &mut meter)?;
                if selected
                    .replace(Parameter {
                        ordinal: u32::try_from(ordinal)
                            .map_err(|_| "local-order parameter ordinal")?,
                        hir_id: id,
                        name,
                        range,
                    })
                    .is_some()
                {
                    return Err("local-order ambiguous HIR parameter".into());
                }
            }
        }
        selected.ok_or_else(|| "local-order local alias is not a parameter".into())
    });
    let [a, b, c, d] = parameters;
    let parameters = [a?, b?, c?, d?];
    if ordinal != 0
        || binding.els.is_some()
        || parameters.iter().map(|p| p.ordinal).ne([1, 2, 3, 4])
    {
        return Err("local-order requires first initializer and four ordered scalar inputs".into());
    }
    let PatKind::Binding(BindingMode::NONE, _, ident, None) = binding.pat.kind else {
        return Err("local-order result must be a plain immutable binding".into());
    };
    let _ = name(&file, &original, ident, &mut meter)?;
    let operators = tree.operators.map(|expression| {
        canonical_source_provenance_v1(tcx, expression.span, 0)
            .map(|value| value.provenance())
            .map_err(|error| format!("local-order source provenance: {error:?}"))
    });
    let [xor, or, and] = operators;
    Ok(CapturedLocalOrder {
        identities: canonical_function_identities_v1(tcx, root.instance),
        parameters,
        operators: [xor?, or?, and?],
        initializer: range(&file, initializer.span, &mut meter)?,
        original_sha256: Sha256::digest(input.original()).into(),
        meter,
        span: initializer.span,
        _session: PhantomData,
    })
}

impl CapturedLocalOrder<'_> {
    pub(crate) fn recheck(
        &mut self,
        tcx: TyCtxt<'_>,
        input: &mut RetainedInput,
    ) -> Result<(), String> {
        input.recheck()?;
        let file = checked_file(tcx, self.span, &mut self.meter)?;
        require_file(&file, input, &mut self.meter)?;
        self.meter.scan(input.original().len())?;
        if <[u8; 32]>::from(Sha256::digest(input.original())) != self.original_sha256 {
            return Err("local-order retained source identity changed".into());
        }
        Ok(())
    }
}
