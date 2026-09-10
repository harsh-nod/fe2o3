use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Expr, ExprRange, LitInt};

use super::MAX_LITERAL_FOR_UNROLL_V1;

pub(super) enum UnrolledRangeV1<'a> {
    Literal {
        start: u128,
        copies: u32,
        suffix: String,
    },
    Constant {
        range: &'a ExprRange,
        bound: u32,
    },
}

impl<'a> UnrolledRangeV1<'a> {
    pub(super) fn parse(range: &'a ExprRange, bound: u32) -> syn::Result<Self> {
        if bound == 0 {
            return Err(syn::Error::new_spanned(
                range,
                "control_flow loop bounds must be nonzero",
            ));
        }
        let (Some(start), Some(end)) = (&range.start, &range.end) else {
            return Err(syn::Error::new_spanned(
                range,
                "bounded for lowering requires both constant range endpoints",
            ));
        };
        for endpoint in [start.as_ref(), end.as_ref()] {
            if !constant_endpoint(endpoint) {
                return Err(syn::Error::new_spanned(
                    endpoint,
                    "bounded for endpoints must be integer literals or named constants without executable const arguments; declare constant expressions separately",
                ));
            }
        }
        let (Some(start), Some(end)) = (integer_literal(start), integer_literal(end)) else {
            if !named_constant(start) && !named_constant(end) {
                return Err(syn::Error::new_spanned(
                    range,
                    "nonliteral bounded for ranges require at least one named constant endpoint",
                ));
            }
            return Ok(Self::Constant { range, bound });
        };
        let start_value = start.base10_parse::<u128>()?;
        let end_value = end.base10_parse::<u128>()?;
        let copies = end_value.saturating_sub(start_value);
        if copies > u128::from(bound) {
            return Err(syn::Error::new_spanned(
                range,
                format!(
                    "literal for range has {copies} iterations, exceeding its declared control_flow bound {bound}"
                ),
            ));
        }
        if copies > u128::from(MAX_LITERAL_FOR_UNROLL_V1) {
            return Err(syn::Error::new_spanned(
                range,
                format!(
                    "literal for lowering supports at most {MAX_LITERAL_FOR_UNROLL_V1} iterations; found {copies}"
                ),
            ));
        }
        let suffix = match (start.suffix(), end.suffix()) {
            (start, end) if !start.is_empty() && !end.is_empty() && start != end => {
                return Err(syn::Error::new_spanned(
                    range,
                    "bounded for lowering requires identical explicit integer suffixes",
                ));
            }
            ("", end) => end,
            (start, _) => start,
        };
        Ok(Self::Literal {
            start: start_value,
            copies: copies as u32,
            suffix: suffix.to_owned(),
        })
    }

    pub(super) fn copies(&self) -> u32 {
        match self {
            Self::Literal { copies, .. } => *copies,
            Self::Constant { bound, .. } => (*bound).min(MAX_LITERAL_FOR_UNROLL_V1),
        }
    }

    pub(super) fn iteration(&self, index: u32, body: TokenStream) -> TokenStream {
        match self {
            Self::Literal { .. } => body,
            Self::Constant { range, bound } => {
                let validation = constant_range_validation(range, *bound);
                let distance = constant_range_distance(range);
                let index = u128::from(index);
                quote_spanned! {range.span()=>
                    if const {
                        #validation
                        (#range).start < (#range).end && #distance > #index
                    } {
                        #body
                    }
                }
            }
        }
    }

    pub(super) fn value(&self, index: u32, span: proc_macro2::Span) -> TokenStream {
        match self {
            Self::Literal { start, suffix, .. } => {
                let literal = LitInt::new(&format!("{}{suffix}", start + u128::from(index)), span);
                quote!(#literal)
            }
            Self::Constant { range, .. } => {
                let distance = constant_range_distance(range);
                let offset = LitInt::new(&index.to_string(), span);
                let index = u128::from(index);
                quote_spanned! {span=> const {
                    if (#range).start < (#range).end && #distance > #index
                    {
                        (#range).start + #offset
                    } else {
                        (#range).start
                    }
                }}
            }
        }
    }
}

fn constant_range_validation(range: &ExprRange, bound: u32) -> TokenStream {
    let distance = constant_range_distance(range);
    let bound = u128::from(bound);
    let limit = u128::from(MAX_LITERAL_FOR_UNROLL_V1);
    let integers = [
        "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
    ]
    .map(|name| syn::Ident::new(name, proc_macro2::Span::mixed_site()));
    // The local, closed trait rules out float, char, and user-defined Step implementations.
    // Rust evaluates endpoints and bounds in CTFE; no helper or panic reaches device MIR.
    quote_spanned! {range.span()=>
        ({
            trait Integer {}
            #(impl Integer for ::core::primitive::#integers {})*
            const fn integer<T: Integer>(_: &T) {}
            integer
        })(&(#range).start);
        if (#range).start < (#range).end {
            ::core::assert!(#distance <= #bound,
                "constant for range exceeds its declared control_flow bound");
            ::core::assert!(#distance <= #limit,
                "constant for lowering supports at most 32 iterations");
        }
    }
}

fn constant_range_distance(range: &ExprRange) -> TokenStream {
    // After start < end, modular subtraction gives the exact distance even across zero
    // for signed integers, without overflowing at either i128/u128 boundary.
    quote_spanned! {range.span()=>
        ((#range).end as ::core::primitive::u128)
            .wrapping_sub((#range).start as ::core::primitive::u128)
    }
}

fn constant_endpoint(expression: &Expr) -> bool {
    match expression {
        Expr::Path(_) => named_constant(expression),
        Expr::Lit(literal) => matches!(literal.lit, syn::Lit::Int(_)),
        Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Neg(_)) => {
            matches!(unary.expr.as_ref(), Expr::Lit(literal) if matches!(literal.lit, syn::Lit::Int(_)))
        }
        Expr::Paren(paren) => constant_endpoint(&paren.expr),
        Expr::Group(group) => constant_endpoint(&group.expr),
        _ => false,
    }
}

fn integer_literal(expression: &Expr) -> Option<&LitInt> {
    match expression {
        Expr::Lit(literal) => match &literal.lit {
            syn::Lit::Int(value) => Some(value),
            _ => None,
        },
        Expr::Paren(paren) => integer_literal(&paren.expr),
        Expr::Group(group) => integer_literal(&group.expr),
        _ => None,
    }
}

fn named_constant(expression: &Expr) -> bool {
    match expression {
        Expr::Path(path) => {
            let mut visitor = ConstantPathVisitor { valid: true };
            visitor.visit_expr_path(path);
            visitor.valid
        }
        Expr::Paren(paren) => named_constant(&paren.expr),
        Expr::Group(group) => named_constant(&group.expr),
        _ => false,
    }
}

struct ConstantPathVisitor {
    valid: bool,
}

impl<'ast> Visit<'ast> for ConstantPathVisitor {
    fn visit_expr(&mut self, expression: &'ast Expr) {
        // CTFE blocks inside path arguments would consume source loop bounds.
        match expression {
            Expr::Path(_) | Expr::Lit(_) => syn::visit::visit_expr(self, expression),
            _ => self.valid = false,
        }
    }

    fn visit_macro(&mut self, _invocation: &'ast syn::Macro) {
        self.valid = false;
    }
}
