//! Closed typed-HIR prefix eligibility; no alternate owner or source authority.
use super::*;

const MAX_PREFIX_BINDINGS: usize = 8;
const PREFIX_SHAPE: &str = "source-boundary prefix requires immutable direct-u32 bitwise lets";
const PREFIX_OPERAND: &str =
    "source-boundary prefix operands must be original immutable u32 formals";
const PREFIX_SHADOW: &str =
    "source-boundary prefix binding shadows a formal, prefix or selected result";

fn prefix_count(count: usize, meter: &mut ScanMeter) -> Result<(), String> {
    if count > MAX_PREFIX_BINDINGS {
        return Err("source-boundary prefix binding limit".into());
    }
    meter.rows(count)?;
    meter.storage(std::mem::size_of::<
        [Option<rustc_span::Symbol>; MAX_PREFIX_BINDINGS],
    >())
}

/// Each prefix is one direct AND/OR/XOR of the already captured original
/// immutable formals, with no dependence on earlier prefix results. Checking
/// typed HIR does not replace the subsequent current semantic/KIR owner join.
pub(super) struct SourceContext<'a, 'tcx> {
    pub(super) tcx: TyCtxt<'tcx>,
    pub(super) typeck: &'a TypeckResults<'tcx>,
    pub(super) body: &'a rustc_hir::Body<'tcx>,
    pub(super) parameters: &'a [Parameter; 3],
    pub(super) file: &'a SourceFile,
    pub(super) source: &'a str,
    pub(super) selected: rustc_span::Ident,
}

pub(super) fn validate<'tcx>(
    context: SourceContext<'_, 'tcx>,
    statements: &[rustc_hir::Stmt<'tcx>],
    meter: &mut ScanMeter,
) -> Result<(), String> {
    let SourceContext {
        tcx,
        typeck,
        body,
        parameters,
        file,
        source,
        selected,
    } = context;
    prefix_count(statements.len(), meter)?;
    let mut names = [None; MAX_PREFIX_BINDINGS];
    for (position, statement) in statements.iter().enumerate() {
        meter.scan(16)?;
        let StmtKind::Let(binding) = statement.kind else {
            return Err(PREFIX_SHAPE.into());
        };
        let PatKind::Binding(BindingMode::NONE, _, ident, None) = binding.pat.kind else {
            return Err(PREFIX_SHAPE.into());
        };
        if binding.els.is_some()
            || typeck.pat_ty(binding.pat) != tcx.types.u32
            || statement.span.from_expansion()
            || binding.pat.span.from_expansion()
            || ident.span.from_expansion()
        {
            return Err(PREFIX_SHAPE.into());
        }
        // Reject shadowing any formal, even an unused output/non-u32 formal.
        meter.rows(body.params.len())?;
        for formal in body.params {
            let PatKind::Binding(_, _, formal_name, None) = formal.pat.kind else {
                return Err(PREFIX_SHAPE.into());
            };
            if ident.name == formal_name.name {
                return Err(PREFIX_SHADOW.into());
            }
        }
        meter.scan(position + 1)?;
        if ident.name == selected.name || names[..position].contains(&Some(ident.name)) {
            return Err(PREFIX_SHADOW.into());
        }
        names[position] = Some(ident.name);
        // This checks spelling and original byte coordinates; macro expansion,
        // normalization and synthetic spans never become source insertion facts.
        name(file, source, ident, meter)?;
        range(file, statement.span, meter)?;
        range(file, binding.pat.span, meter)?;
        let initializer = binding.init.ok_or(PREFIX_SHAPE)?;
        let ExprKind::Binary(operator, left, right) = initializer.kind else {
            return Err(PREFIX_SHAPE.into());
        };
        if !matches!(
            operator.node,
            BinOpKind::BitAnd | BinOpKind::BitOr | BinOpKind::BitXor
        ) {
            return Err(PREFIX_SHAPE.into());
        }
        for expression in [initializer, left, right] {
            meter.scan(1)?;
            if typeck.expr_ty(expression) != tcx.types.u32
                || !typeck.expr_adjustments(expression).is_empty()
                || expression.span.from_expansion()
            {
                return Err(PREFIX_SHAPE.into());
            }
            range(file, expression.span, meter)?;
        }
        for operand in [left, right] {
            let id = parameter_id(typeck, operand).map_err(|_| PREFIX_OPERAND)?;
            meter.scan(parameters.len())?;
            if !parameters.iter().any(|formal| formal.hir_id == id) {
                return Err(PREFIX_OPERAND.into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "source_bitselect_prefix_v1_tests.rs"]
mod tests;
