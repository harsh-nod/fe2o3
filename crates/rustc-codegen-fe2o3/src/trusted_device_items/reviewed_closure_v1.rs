//! Source authentication only: closure captures, ABI, and MIR remain traversed.

use super::*;
use rustc_span::Span;

const MAX_CLOSURE_DEPTH: usize = 64;

#[cfg(test)]
mod compiler_tests;

pub(super) fn authenticate(tcx: TyCtxt<'_>, closure: DefId) -> Result<bool, String> {
    if tcx.def_kind(closure) != DefKind::Closure
        || closure.krate == LOCAL_CRATE
        || tcx.crate_name(closure.krate).as_str() != "fe2o3_device"
    {
        return Ok(false);
    }
    let source = compiled_provider_source_path_v1(tcx, closure)?;
    let mut child = closure;
    for _ in 0..MAX_CLOSURE_DEPTH {
        let parent = tcx
            .opt_parent(child)
            .ok_or("reviewed closure has no owner")?;
        if parent.krate != closure.krate || parent == child {
            return Err("reviewed closure owner is not in the same compiler crate".into());
        }
        if !matches!(
            tcx.def_kind(parent),
            DefKind::Closure | DefKind::Fn | DefKind::AssocFn
        ) {
            return Err("reviewed closure has an unsupported enclosing definition".into());
        }
        let child_span = body_span(tcx, child)?;
        let parent_span = body_span(tcx, parent)?;
        if !nested_spans(child_span, parent_span) {
            return Err("reviewed closure source is not contained by its owner".into());
        }
        let map = tcx.sess.source_map();
        let file = map.lookup_source_file(parent_span.lo());
        // Require the complete spans, not only their start positions, in one
        // compiled file whose bytes were authenticated against rustc metadata.
        if file.cnum != closure.krate
            || parent_span.hi() > file.end_position()
            || compiled_provider_source_path_v1(tcx, parent)? != source
        {
            return Err(
                "reviewed closure and owner do not share an authenticated source file".into(),
            );
        }
        match tcx.def_kind(parent) {
            DefKind::Closure => child = parent,
            DefKind::Fn | DefKind::AssocFn => {
                let signature = tcx.instantiate_bound_regions_with_erased(
                    tcx.fn_sig(parent).instantiate_identity(),
                );
                if signature.safety != Safety::Safe || signature.abi != ExternAbi::Rust {
                    return Err("reviewed closure owner is not a safe Rust function".into());
                }
                let definition = reviewed_provider_semantic_definition_v1(tcx, parent)?;
                validate_safe_execution_provider_definition_v1(&definition)?;
                validate_authenticated_provider_structure_v1(tcx, parent, &definition)?;
                return Ok(true);
            }
            _ => return Err("reviewed closure has an unsupported enclosing definition".into()),
        }
    }
    Err("reviewed closure owner depth exceeds the supported bound".into())
}

fn nested_spans(child: Span, parent: Span) -> bool {
    !child.is_dummy()
        && !parent.is_dummy()
        && !child.from_expansion()
        && !parent.from_expansion()
        && child.lo() < child.hi()
        && parent.lo() < parent.hi()
        && child.ctxt() == parent.ctxt()
        && parent.lo() <= child.lo()
        && child.hi() <= parent.hi()
}

fn body_span(tcx: TyCtxt<'_>, definition: DefId) -> Result<Span, String> {
    if !tcx.is_mir_available(definition) {
        return Err("reviewed closure owner has no retained MIR".into());
    }
    let body = tcx.optimized_mir(definition);
    if body.source.instance != InstanceKind::Item(definition)
        || body.source.promoted.is_some()
        || !nested_spans(tcx.def_span(definition), body.span)
    {
        return Err("reviewed closure source does not match its retained MIR owner".into());
    }
    Ok(body.span)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustc_span::{BytePos, DUMMY_SP, SyntaxContext};

    fn span(lo: u32, hi: u32) -> Span {
        Span::new(BytePos(lo), BytePos(hi), SyntaxContext::root(), None)
    }

    #[test]
    fn closure_span_must_be_nonempty_and_contained_in_its_owner() {
        assert!(nested_spans(span(12, 18), span(10, 20)));
        assert!(nested_spans(span(10, 20), span(10, 20)));
        for child in [
            DUMMY_SP,
            span(12, 12),
            span(9, 18),
            span(12, 21),
            span(21, 24),
        ] {
            assert!(!nested_spans(child, span(10, 20)));
        }
        assert!(!nested_spans(span(12, 18), DUMMY_SP));
        assert!(!nested_spans(span(12, 18), span(10, 10)));
    }
}
