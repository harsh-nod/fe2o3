use syn::{ExprMatch, Pat};

const MAX_PATTERN_DEPTH: usize = 32;

// This is syntax admission, not an enum classifier. Names may resolve to
// constructors, constants or bindings. The unchanged compiler body is the only
// source of type, variant, logical discriminant and control-flow authority.
pub(super) fn accepts(expression: &ExprMatch) -> bool {
    if expression.arms.is_empty() || expression.arms.len() > super::MAX_CASES_V1 {
        return false;
    }
    let mut remaining = super::MAX_NODES_V1;
    expression
        .arms
        .iter()
        .all(|arm| arm.guard.is_none() && pattern(&arm.pat, true, 0, &mut remaining))
}

fn pattern(value: &Pat, top: bool, depth: usize, remaining: &mut usize) -> bool {
    if depth > MAX_PATTERN_DEPTH || *remaining == 0 {
        return false;
    }
    *remaining -= 1;
    match value {
        Pat::Wild(_) => true,
        Pat::Rest(_) => !top,
        Pat::Ident(binding) => binding.subpat.is_none(),
        Pat::Paren(parenthesized) => pattern(&parenthesized.pat, top, depth + 1, remaining),
        Pat::Reference(reference) => pattern(&reference.pat, top, depth + 1, remaining),
        Pat::Path(_) => top,
        Pat::Or(alternatives) => {
            top && alternatives
                .cases
                .iter()
                .all(|case| pattern(case, true, depth + 1, remaining))
        }
        Pat::TupleStruct(constructor) => {
            top && constructor
                .elems
                .iter()
                .all(|field| pattern(field, false, depth + 1, remaining))
        }
        Pat::Struct(constructor) => {
            top && constructor
                .fields
                .iter()
                .all(|field| pattern(&field.pat, false, depth + 1, remaining))
        }
        Pat::Tuple(tuple) => {
            !top && tuple
                .elems
                .iter()
                .all(|field| pattern(field, false, depth + 1, remaining))
        }
        // Literal/range, nested refutable constructor, macro, slice, typed and
        // subpattern tests remain outside this bounded source grammar.
        _ => false,
    }
}

#[cfg(test)]
#[path = "rustc_resolved_match_v49_tests.rs"]
mod tests;
