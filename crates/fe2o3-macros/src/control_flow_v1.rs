use proc_macro2::Span;
use quote::{ToTokens, quote_spanned};
use syn::parse::Parser;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::visit_mut::VisitMut;
use syn::{
    Block, Expr, ExprBreak, ExprContinue, ExprForLoop, ExprIf, ExprLoop, ExprMatch, ExprWhile,
    Ident, ItemFn, LitInt, MetaList, Pat, RangeLimits, Stmt, Token, parenthesized,
    punctuated::Punctuated,
};

pub(crate) const CONTROL_FLOW_REGISTRATION_PREFIX_V1: &str = "__fe2o3_control_flow_contract_v1_";
pub(crate) const CONTROL_FLOW_REGISTRATION_MAGIC_V1: u64 = u64::from_le_bytes(*b"FE2O3CFA");
pub(crate) const CONTROL_FLOW_REGISTRATION_VERSION_V1: u16 = 1;
pub(crate) const CONTROL_FLOW_REGISTRATION_KIND_V1: u16 = 1;

const CONTROL_FLOW_CONTRACT_MAGIC_V1: [u8; 8] = *b"FE2O3CF\0";
const CONTROL_FLOW_CONTRACT_VERSION_V1: u16 = 1;
const MAX_DECLARATIONS_V1: usize = 256;
const MAX_CASES_V1: usize = 256;
const MAX_NODES_V1: usize = 4096;
const MAX_BYTES_V1: usize = 1024 * 1024;
const MAX_LITERAL_FOR_UNROLL_V1: u32 = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ParsedIntegerSwitchTypeV1 {
    width: u16,
    signed: bool,
}

impl ParsedIntegerSwitchTypeV1 {
    fn parse(ident: &Ident) -> syn::Result<Self> {
        let (width, signed) = match ident.to_string().as_str() {
            "i8" => (8, true),
            "i16" => (16, true),
            "i32" => (32, true),
            "i64" => (64, true),
            "i128" => (128, true),
            "u8" => (8, false),
            "u16" => (16, false),
            "u32" => (32, false),
            "u64" => (64, false),
            "u128" => (128, false),
            _ => {
                return Err(syn::Error::new_spanned(
                    ident,
                    "integer_switches supports only fixed-width i8..i128 and u8..u128 types",
                ));
            }
        };
        Ok(Self { width, signed })
    }

    fn name(self) -> &'static str {
        match (self.signed, self.width) {
            (true, 8) => "i8",
            (true, 16) => "i16",
            (true, 32) => "i32",
            (true, 64) => "i64",
            (true, 128) => "i128",
            (false, 8) => "u8",
            (false, 16) => "u16",
            (false, 32) => "u32",
            (false, 64) => "u64",
            (false, 128) => "u128",
            _ => unreachable!("parsed switch type has a supported width"),
        }
    }

    fn accepts_bits(self, bits: u128) -> bool {
        if self.signed {
            if self.width == 128 {
                return true;
            }
            let mask = (1_u128 << self.width) - 1;
            let truncated = bits & mask;
            let sign_bit = 1_u128 << (self.width - 1);
            let canonical = if truncated & sign_bit == 0 {
                truncated
            } else {
                truncated | !mask
            };
            bits == canonical
        } else {
            self.width == 128 || bits < (1_u128 << self.width)
        }
    }

    fn compare(self, left: u128, right: u128) -> std::cmp::Ordering {
        if self.signed {
            (left as i128).cmp(&(right as i128))
        } else {
            left.cmp(&right)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ParsedControlFlowOptionsV1 {
    loop_bounds: Vec<u32>,
    integer_switches: Vec<ParsedIntegerSwitchTypeV1>,
}

pub(crate) fn parse_control_flow_options_v1(
    list: &MetaList,
) -> syn::Result<ParsedControlFlowOptionsV1> {
    let mut loop_bounds = None;
    let mut integer_switches = None;
    list.parse_nested_meta(|meta| {
        if meta.path.is_ident("loop_bounds") {
            if loop_bounds.is_some() {
                return Err(meta.error("control_flow loop_bounds is duplicated"));
            }
            let content;
            parenthesized!(content in meta.input);
            let values =
                Punctuated::<LitInt, Token![,]>::parse_terminated.parse2(content.parse()?)?;
            if values.len() > MAX_DECLARATIONS_V1 {
                return Err(meta.error("control_flow supports at most 256 loop bounds"));
            }
            let mut parsed = Vec::with_capacity(values.len());
            for value in values {
                let value = value.base10_parse::<u32>()?;
                if value == 0 {
                    return Err(meta.error("control_flow loop bounds must be nonzero"));
                }
                parsed.push(value);
            }
            loop_bounds = Some(parsed);
            return Ok(());
        }
        if meta.path.is_ident("integer_switches") {
            if integer_switches.is_some() {
                return Err(meta.error("control_flow integer_switches is duplicated"));
            }
            let content;
            parenthesized!(content in meta.input);
            let values =
                Punctuated::<Ident, Token![,]>::parse_terminated.parse2(content.parse()?)?;
            if values.len() > MAX_DECLARATIONS_V1 {
                return Err(meta.error("control_flow supports at most 256 integer switches"));
            }
            integer_switches = Some(
                values
                    .iter()
                    .map(ParsedIntegerSwitchTypeV1::parse)
                    .collect::<syn::Result<Vec<_>>>()?,
            );
            return Ok(());
        }
        Err(meta.error("control_flow supports only loop_bounds(...) and integer_switches(...)"))
    })?;

    let loop_bounds = loop_bounds.unwrap_or_default();
    let integer_switches = integer_switches.unwrap_or_default();
    if loop_bounds.is_empty() && integer_switches.is_empty() {
        return Err(syn::Error::new_spanned(
            list,
            "control_flow requires at least one loop bound or integer switch type",
        ));
    }
    Ok(ParsedControlFlowOptionsV1 {
        loop_bounds,
        integer_switches,
    })
}

#[derive(Default)]
struct DirectControlFlowUseVisitor {
    first: Option<(Span, &'static str)>,
}

impl DirectControlFlowUseVisitor {
    fn record(&mut self, span: Span, kind: &'static str) {
        if self.first.is_none() {
            self.first = Some((span, kind));
        }
    }
}

impl<'ast> Visit<'ast> for DirectControlFlowUseVisitor {
    fn visit_expr_loop(&mut self, expression: &'ast ExprLoop) {
        self.record(expression.loop_token.span, "loop");
        syn::visit::visit_expr_loop(self, expression);
    }

    fn visit_expr_while(&mut self, expression: &'ast ExprWhile) {
        self.record(expression.while_token.span, "while loop");
        syn::visit::visit_expr_while(self, expression);
    }

    fn visit_expr_for_loop(&mut self, expression: &'ast ExprForLoop) {
        self.record(expression.for_token.span, "for loop");
        syn::visit::visit_expr_for_loop(self, expression);
    }

    fn visit_expr_match(&mut self, expression: &'ast ExprMatch) {
        self.record(expression.match_token.span, "match");
        syn::visit::visit_expr_match(self, expression);
    }

    fn visit_expr_break(&mut self, expression: &'ast ExprBreak) {
        self.record(expression.break_token.span, "break");
        syn::visit::visit_expr_break(self, expression);
    }

    fn visit_expr_continue(&mut self, expression: &'ast ExprContinue) {
        self.record(expression.continue_token.span, "continue");
        syn::visit::visit_expr_continue(self, expression);
    }
}

pub(crate) fn analyze_kernel_control_flow_v1(
    input: &ItemFn,
    declaration: Option<&ParsedControlFlowOptionsV1>,
) -> syn::Result<Option<Vec<u8>>> {
    let mut visitor = DirectControlFlowUseVisitor::default();
    visitor.visit_block(&input.block);
    let Some(declaration) = declaration else {
        if let Some((span, kind)) = visitor.first {
            return Err(syn::Error::new(
                span,
                format!(
                    "direct kernel {kind} requires control_flow(loop_bounds(...), integer_switches(...))"
                ),
            ));
        }
        return Ok(None);
    };

    let mut builder = GraphBuilder::new(input, declaration)?;
    builder.build_function(input)?;
    Ok(Some(builder.encode()?))
}

pub(crate) fn lower_bounded_for_loops_v1(
    input: &mut ItemFn,
    declaration: Option<&ParsedControlFlowOptionsV1>,
) -> syn::Result<()> {
    let Some(declaration) = declaration else {
        return Ok(());
    };
    let mut lowerer = LiteralForLowerer {
        declaration,
        loop_cursor: 0,
        error: None,
    };
    lowerer.visit_block_mut(&mut input.block);
    if let Some(error) = lowerer.error {
        return Err(error);
    }
    Ok(())
}

struct LiteralForLowerer<'a> {
    declaration: &'a ParsedControlFlowOptionsV1,
    loop_cursor: usize,
    error: Option<syn::Error>,
}

impl LiteralForLowerer<'_> {
    fn take_bound(&mut self, span: Span) -> Option<(usize, u32)> {
        let index = self.loop_cursor;
        self.loop_cursor += 1;
        let Some(bound) = self.declaration.loop_bounds.get(index).copied() else {
            self.error = Some(syn::Error::new(
                span,
                "kernel loop has no corresponding control_flow loop bound",
            ));
            return None;
        };
        Some((index, bound))
    }

    fn lower_for(&mut self, expression: &mut Expr) {
        let Expr::ForLoop(for_loop) = expression else {
            unreachable!("literal-for lowering was called for a different expression")
        };
        let span = for_loop.span();
        let Some((loop_index, declared_bound)) = self.take_bound(span) else {
            return;
        };
        if for_loop.label.is_some() {
            self.error = Some(syn::Error::new_spanned(
                &for_loop.label,
                "bounded for lowering does not support labeled loops",
            ));
            return;
        }
        let Pat::Ident(pattern) = for_loop.pat.as_ref() else {
            self.error = Some(syn::Error::new_spanned(
                &for_loop.pat,
                "bounded for lowering requires a single identifier pattern",
            ));
            return;
        };
        if pattern.subpat.is_some() || pattern.by_ref.is_some() {
            self.error = Some(syn::Error::new_spanned(
                pattern,
                "bounded for lowering requires a by-value identifier without a subpattern",
            ));
            return;
        }
        let Expr::Range(range) = for_loop.expr.as_ref() else {
            self.error = Some(syn::Error::new_spanned(
                &for_loop.expr,
                "bounded for lowering requires a literal half-open range START..END",
            ));
            return;
        };
        if !matches!(range.limits, RangeLimits::HalfOpen(_)) {
            self.error = Some(syn::Error::new_spanned(
                range,
                "bounded for lowering requires a half-open range; inclusive ranges are unsupported",
            ));
            return;
        }
        let (Some(start), Some(end)) = (&range.start, &range.end) else {
            self.error = Some(syn::Error::new_spanned(
                range,
                "bounded for lowering requires both literal range endpoints",
            ));
            return;
        };
        let (Expr::Lit(start), Expr::Lit(end)) = (start.as_ref(), end.as_ref()) else {
            self.error = Some(syn::Error::new_spanned(
                range,
                "bounded for lowering requires literal range endpoints",
            ));
            return;
        };
        let (syn::Lit::Int(start), syn::Lit::Int(end)) = (&start.lit, &end.lit) else {
            self.error = Some(syn::Error::new_spanned(
                range,
                "bounded for lowering requires integer literal range endpoints",
            ));
            return;
        };
        let start_value = match start.base10_parse::<u32>() {
            Ok(value) => value,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };
        let end_value = match end.base10_parse::<u32>() {
            Ok(value) => value,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };
        let iterations = end_value.saturating_sub(start_value);
        if iterations > declared_bound {
            self.error = Some(syn::Error::new_spanned(
                range,
                format!(
                    "literal for range has {iterations} iterations, exceeding its declared control_flow bound {declared_bound}"
                ),
            ));
            return;
        }
        if iterations > MAX_LITERAL_FOR_UNROLL_V1 {
            self.error = Some(syn::Error::new_spanned(
                range,
                format!(
                    "literal for lowering supports at most {MAX_LITERAL_FOR_UNROLL_V1} iterations; found {iterations}"
                ),
            ));
            return;
        }

        let break_label = syn::Lifetime::new(&format!("'__fe2o3_unrolled_for_{loop_index}"), span);
        let pattern = for_loop.pat.clone();
        let attrs = for_loop.attrs.clone();
        let start_suffix = start.suffix();
        let end_suffix = end.suffix();
        let suffix = match (start_suffix.is_empty(), end_suffix.is_empty()) {
            (false, false) if start_suffix != end_suffix => {
                self.error = Some(syn::Error::new_spanned(
                    range,
                    "bounded for lowering requires identical explicit integer suffixes",
                ));
                return;
            }
            (false, _) => start_suffix,
            (_, false) => end_suffix,
            (true, true) => "",
        };
        let mut copies = Vec::with_capacity(iterations as usize);
        for (copy_index, value) in (start_value..end_value).enumerate() {
            let continue_label = syn::Lifetime::new(
                &format!("'__fe2o3_unrolled_for_{loop_index}_iteration_{copy_index}"),
                span,
            );
            let mut body = for_loop.body.clone();
            let mut rewriter = LoopExitRewriter {
                break_label: &break_label,
                continue_label: &continue_label,
                error: None,
            };
            rewriter.visit_block_mut(&mut body);
            if let Some(error) = rewriter.error {
                self.error = Some(error);
                return;
            }
            let literal = LitInt::new(&format!("{value}{suffix}"), span);
            copies.push(quote_spanned! {span=>
                #continue_label: {
                    let #pattern = #literal;
                    #body
                }
            });
        }
        match syn::parse2(quote_spanned! {span=>
            #(#attrs)*
            #break_label: {
                #(#copies)*
            }
        }) {
            Ok(replacement) => *expression = replacement,
            Err(error) => self.error = Some(error),
        }
    }
}

impl VisitMut for LiteralForLowerer<'_> {
    fn visit_expr_mut(&mut self, expression: &mut Expr) {
        if self.error.is_some() {
            return;
        }
        match expression {
            Expr::Loop(loop_expression) => {
                if self.take_bound(loop_expression.span()).is_some() {
                    self.visit_block_mut(&mut loop_expression.body);
                }
            }
            Expr::While(while_expression) => {
                if self.take_bound(while_expression.span()).is_some() {
                    self.visit_block_mut(&mut while_expression.body);
                }
            }
            Expr::ForLoop(_) => self.lower_for(expression),
            _ => syn::visit_mut::visit_expr_mut(self, expression),
        }
    }
}

struct LoopExitRewriter<'a> {
    break_label: &'a syn::Lifetime,
    continue_label: &'a syn::Lifetime,
    error: Option<syn::Error>,
}

impl VisitMut for LoopExitRewriter<'_> {
    fn visit_expr_mut(&mut self, expression: &mut Expr) {
        if self.error.is_some() {
            return;
        }
        match expression {
            Expr::Break(break_expression) => {
                if break_expression.label.is_some() || break_expression.expr.is_some() {
                    self.error = Some(syn::Error::new_spanned(
                        break_expression,
                        "bounded for lowering supports only unlabeled break without a value",
                    ));
                } else {
                    let label = self.break_label;
                    *expression = syn::parse_quote_spanned!(break_expression.span()=> break #label);
                }
            }
            Expr::Continue(continue_expression) => {
                if continue_expression.label.is_some() {
                    self.error = Some(syn::Error::new_spanned(
                        continue_expression,
                        "bounded for lowering supports only unlabeled continue",
                    ));
                } else {
                    let label = self.continue_label;
                    *expression =
                        syn::parse_quote_spanned!(continue_expression.span()=> break #label);
                }
            }
            Expr::Loop(_) | Expr::While(_) | Expr::ForLoop(_) => {
                self.error = Some(syn::Error::new_spanned(
                    expression,
                    "bounded for lowering does not support nested loops",
                ));
            }
            _ => syn::visit_mut::visit_expr_mut(self, expression),
        }
    }
}

#[derive(Clone)]
struct SourceSpanV1 {
    file: String,
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
}

impl SourceSpanV1 {
    fn from_span(span: Span) -> syn::Result<Self> {
        let start = span.start();
        let end = span.end();
        let start_line = u32::try_from(start.line)
            .map_err(|_| syn::Error::new(span, "source line does not fit the V1 sidecar"))?;
        let end_line = u32::try_from(end.line)
            .map_err(|_| syn::Error::new(span, "source line does not fit the V1 sidecar"))?;
        let start_column = u32::try_from(start.column)
            .ok()
            .and_then(|column| column.checked_add(1))
            .ok_or_else(|| syn::Error::new(span, "source column does not fit the V1 sidecar"))?;
        let end_column = u32::try_from(end.column)
            .ok()
            .and_then(|column| column.checked_add(1))
            .ok_or_else(|| syn::Error::new(span, "source column does not fit the V1 sidecar"))?;
        let file = span.file();
        if file.is_empty() || file.len() > 1024 || file.as_bytes().contains(&0) {
            return Err(syn::Error::new(
                span,
                "source file is invalid for the V1 sidecar",
            ));
        }
        if start_line == 0 || end_line == 0 || (end_line, end_column) < (start_line, start_column) {
            return Err(syn::Error::new(
                span,
                "source span is invalid for the V1 sidecar",
            ));
        }
        Ok(Self {
            file,
            start_line,
            start_column,
            end_line,
            end_column,
        })
    }
}

#[derive(Clone)]
enum TempNodeKind {
    Entry {
        target: Option<usize>,
    },
    Block {
        target: Option<usize>,
    },
    Exit,
    Branch {
        then_target: Option<usize>,
        else_target: Option<usize>,
    },
    Loop {
        max_iterations: u32,
        body: Option<usize>,
        exit: Option<usize>,
    },
    Break {
        loop_header: usize,
        target: Option<usize>,
    },
    Continue {
        loop_header: usize,
        target: usize,
    },
    IntegerSwitch {
        ty: ParsedIntegerSwitchTypeV1,
        cases: Vec<(u128, Option<usize>)>,
        default: Option<usize>,
    },
}

#[derive(Clone)]
struct TempNode {
    span: SourceSpanV1,
    kind: TempNodeKind,
}

#[derive(Clone, Copy)]
enum PendingSlot {
    EntryTarget,
    BlockTarget,
    BranchThen,
    BranchElse,
    LoopExit,
    BreakTarget,
    SwitchCase(u128),
    SwitchDefault,
}

#[derive(Clone, Copy)]
struct PendingEdge {
    node: usize,
    slot: PendingSlot,
}

#[derive(Default)]
struct Fragment {
    entry: Option<usize>,
    exits: Vec<PendingEdge>,
}

struct LoopContext {
    label: Option<String>,
    header: usize,
    breaks: Vec<PendingEdge>,
}

struct GraphBuilder<'a> {
    declaration: &'a ParsedControlFlowOptionsV1,
    nodes: Vec<TempNode>,
    loop_cursor: usize,
    switch_cursor: usize,
    loops: Vec<LoopContext>,
    function_exit: usize,
}

impl<'a> GraphBuilder<'a> {
    fn new(input: &ItemFn, declaration: &'a ParsedControlFlowOptionsV1) -> syn::Result<Self> {
        let span = SourceSpanV1::from_span(input.sig.ident.span())?;
        Ok(Self {
            declaration,
            nodes: vec![
                TempNode {
                    span: span.clone(),
                    kind: TempNodeKind::Entry { target: None },
                },
                TempNode {
                    span,
                    kind: TempNodeKind::Exit,
                },
            ],
            loop_cursor: 0,
            switch_cursor: 0,
            loops: Vec::new(),
            function_exit: 1,
        })
    }

    fn build_function(&mut self, input: &ItemFn) -> syn::Result<()> {
        let body = self.build_block(&input.block)?;
        let target = body.entry.unwrap_or(self.function_exit);
        self.patch(
            PendingEdge {
                node: 0,
                slot: PendingSlot::EntryTarget,
            },
            target,
        )?;
        self.patch_all(body.exits, self.function_exit)?;
        if self.loop_cursor != self.declaration.loop_bounds.len() {
            return Err(syn::Error::new_spanned(
                &input.sig,
                format!(
                    "control_flow declares {} loop bounds but the kernel contains {} direct loops",
                    self.declaration.loop_bounds.len(),
                    self.loop_cursor
                ),
            ));
        }
        if self.switch_cursor != self.declaration.integer_switches.len() {
            return Err(syn::Error::new_spanned(
                &input.sig,
                format!(
                    "control_flow declares {} integer switch types but the kernel contains {} direct matches",
                    self.declaration.integer_switches.len(),
                    self.switch_cursor
                ),
            ));
        }
        if self.nodes.len() > MAX_NODES_V1 {
            return Err(syn::Error::new_spanned(
                &input.sig,
                "kernel control flow exceeds 4096 source nodes",
            ));
        }
        Ok(())
    }

    fn build_block(&mut self, block: &Block) -> syn::Result<Fragment> {
        let mut result = Fragment::default();
        for statement in &block.stmts {
            let next = self.build_statement(statement)?;
            result = self.sequence(result, next, statement.span())?;
        }
        Ok(result)
    }

    fn build_statement(&mut self, statement: &Stmt) -> syn::Result<Fragment> {
        match statement {
            Stmt::Local(local) => {
                let Some(init) = &local.init else {
                    return self.simple(statement.span());
                };
                if let Some((_, diverge)) = &init.diverge {
                    reject_nested_control_flow(&init.expr)?;
                    let branch = self.push_node(
                        local.span(),
                        TempNodeKind::Branch {
                            then_target: None,
                            else_target: None,
                        },
                    )?;
                    let else_fragment = self.build_expression(diverge)?;
                    let mut exits = vec![PendingEdge {
                        node: branch,
                        slot: PendingSlot::BranchThen,
                    }];
                    self.bind_fragment_entry(
                        branch,
                        PendingSlot::BranchElse,
                        else_fragment.entry,
                        &mut exits,
                    )?;
                    exits.extend(else_fragment.exits);
                    return Ok(Fragment {
                        entry: Some(branch),
                        exits,
                    });
                }
                self.build_expression(&init.expr)
            }
            Stmt::Item(_) => Ok(Fragment::default()),
            Stmt::Expr(expression, _) => self.build_expression(expression),
            Stmt::Macro(statement) => Err(syn::Error::new_spanned(
                statement,
                "statement macros are opaque to the V1 kernel control-flow sidecar",
            )),
        }
    }

    fn build_expression(&mut self, expression: &Expr) -> syn::Result<Fragment> {
        match expression {
            Expr::Block(expression) => self.build_block(&expression.block),
            Expr::Unsafe(expression) => self.build_block(&expression.block),
            Expr::If(expression) => self.build_if(expression),
            Expr::Loop(expression) => self.build_loop(expression),
            Expr::While(expression) => self.build_while(expression),
            Expr::ForLoop(expression) => self.build_for(expression),
            Expr::Break(expression) => self.build_break(expression),
            Expr::Continue(expression) => self.build_continue(expression),
            Expr::Match(expression) => self.build_match(expression),
            Expr::Assign(expression) => {
                reject_nested_control_flow(&expression.left)?;
                self.build_expression(&expression.right)
            }
            Expr::Return(expression) => {
                if let Some(value) = &expression.expr {
                    reject_nested_control_flow(value)?;
                }
                let node = self.push_node(
                    expression.span(),
                    TempNodeKind::Block {
                        target: Some(self.function_exit),
                    },
                )?;
                Ok(Fragment {
                    entry: Some(node),
                    exits: Vec::new(),
                })
            }
            _ => {
                reject_nested_control_flow(expression)?;
                self.simple(expression.span())
            }
        }
    }

    fn build_if(&mut self, expression: &ExprIf) -> syn::Result<Fragment> {
        reject_nested_control_flow(&expression.cond)?;
        let branch = self.push_node(
            expression.span(),
            TempNodeKind::Branch {
                then_target: None,
                else_target: None,
            },
        )?;
        let then_fragment = self.build_block(&expression.then_branch)?;
        let else_fragment = match &expression.else_branch {
            Some((_, expression)) => self.build_expression(expression)?,
            None => Fragment::default(),
        };
        let mut exits = Vec::new();
        self.bind_fragment_entry(
            branch,
            PendingSlot::BranchThen,
            then_fragment.entry,
            &mut exits,
        )?;
        self.bind_fragment_entry(
            branch,
            PendingSlot::BranchElse,
            else_fragment.entry,
            &mut exits,
        )?;
        exits.extend(then_fragment.exits);
        exits.extend(else_fragment.exits);
        Ok(Fragment {
            entry: Some(branch),
            exits,
        })
    }

    fn build_loop(&mut self, expression: &ExprLoop) -> syn::Result<Fragment> {
        self.build_loop_block(
            expression.label.as_ref(),
            &expression.body,
            expression.span(),
        )
    }

    fn build_while(&mut self, expression: &ExprWhile) -> syn::Result<Fragment> {
        reject_nested_control_flow(&expression.cond)?;
        self.build_loop_block(
            expression.label.as_ref(),
            &expression.body,
            expression.span(),
        )
    }

    fn build_for(&mut self, expression: &ExprForLoop) -> syn::Result<Fragment> {
        reject_nested_control_flow(&expression.expr)?;
        self.build_loop_block(
            expression.label.as_ref(),
            &expression.body,
            expression.span(),
        )
    }

    fn build_loop_block(
        &mut self,
        label: Option<&syn::Label>,
        body: &Block,
        span: Span,
    ) -> syn::Result<Fragment> {
        let Some(max_iterations) = self.declaration.loop_bounds.get(self.loop_cursor).copied()
        else {
            return Err(syn::Error::new(
                span,
                "kernel loop has no corresponding control_flow loop bound",
            ));
        };
        self.loop_cursor += 1;
        let header = self.push_node(
            span,
            TempNodeKind::Loop {
                max_iterations,
                body: None,
                exit: None,
            },
        )?;
        self.loops.push(LoopContext {
            label: label.map(|label| label.name.ident.to_string()),
            header,
            breaks: Vec::new(),
        });
        let body = self.build_block(body)?;
        let context = self.loops.pop().expect("loop context was just pushed");
        let body_target = body.entry.unwrap_or(header);
        match &mut self.nodes[header].kind {
            TempNodeKind::Loop { body, .. } => *body = Some(body_target),
            _ => unreachable!("loop header retained its node kind"),
        }
        self.patch_all(body.exits, header)?;
        let mut exits = vec![PendingEdge {
            node: header,
            slot: PendingSlot::LoopExit,
        }];
        exits.extend(context.breaks);
        Ok(Fragment {
            entry: Some(header),
            exits,
        })
    }

    fn build_break(&mut self, expression: &ExprBreak) -> syn::Result<Fragment> {
        if expression.expr.is_some() {
            return Err(syn::Error::new_spanned(
                expression,
                "break with a value is unsupported in the V1 kernel sidecar",
            ));
        }
        let loop_index = self.resolve_loop(expression.label.as_ref(), expression.span())?;
        let header = self.loops[loop_index].header;
        let node = self.push_node(
            expression.span(),
            TempNodeKind::Break {
                loop_header: header,
                target: None,
            },
        )?;
        self.loops[loop_index].breaks.push(PendingEdge {
            node,
            slot: PendingSlot::BreakTarget,
        });
        Ok(Fragment {
            entry: Some(node),
            exits: Vec::new(),
        })
    }

    fn build_continue(&mut self, expression: &ExprContinue) -> syn::Result<Fragment> {
        let loop_index = self.resolve_loop(expression.label.as_ref(), expression.span())?;
        let header = self.loops[loop_index].header;
        let node = self.push_node(
            expression.span(),
            TempNodeKind::Continue {
                loop_header: header,
                target: header,
            },
        )?;
        Ok(Fragment {
            entry: Some(node),
            exits: Vec::new(),
        })
    }

    fn build_match(&mut self, expression: &ExprMatch) -> syn::Result<Fragment> {
        reject_nested_control_flow(&expression.expr)?;
        let Some(ty) = self
            .declaration
            .integer_switches
            .get(self.switch_cursor)
            .copied()
        else {
            return Err(syn::Error::new(
                expression.match_token.span,
                "kernel match has no corresponding control_flow integer switch type",
            ));
        };
        self.switch_cursor += 1;
        let switch = self.push_node(
            expression.span(),
            TempNodeKind::IntegerSwitch {
                ty,
                cases: Vec::new(),
                default: None,
            },
        )?;

        let mut exits = Vec::new();
        let arm_count = expression.arms.len();
        for (arm_index, arm) in expression.arms.iter().enumerate() {
            if arm.guard.is_some() {
                return Err(syn::Error::new_spanned(
                    arm,
                    "guarded match arms are unsupported in the V1 integer switch",
                ));
            }
            let pattern = parse_arm_pattern(&arm.pat, ty)?;
            let fragment = self.build_expression(&arm.body)?;
            match pattern {
                ParsedArmPattern::Default => {
                    if arm_index + 1 != arm_count {
                        return Err(syn::Error::new_spanned(
                            &arm.pat,
                            "the integer switch default arm must be last",
                        ));
                    }
                    let already_set = matches!(
                        &self.nodes[switch].kind,
                        TempNodeKind::IntegerSwitch {
                            default: Some(_),
                            ..
                        }
                    );
                    if already_set {
                        return Err(syn::Error::new_spanned(
                            &arm.pat,
                            "integer switch contains more than one default arm",
                        ));
                    }
                    if let Some(target) = fragment.entry {
                        self.patch(
                            PendingEdge {
                                node: switch,
                                slot: PendingSlot::SwitchDefault,
                            },
                            target,
                        )?;
                    } else {
                        exits.push(PendingEdge {
                            node: switch,
                            slot: PendingSlot::SwitchDefault,
                        });
                    }
                }
                ParsedArmPattern::Cases(values) => {
                    for value in values {
                        match &mut self.nodes[switch].kind {
                            TempNodeKind::IntegerSwitch { cases, .. } => {
                                cases.push((value, fragment.entry));
                            }
                            _ => unreachable!("switch retained its node kind"),
                        }
                        if fragment.entry.is_none() {
                            exits.push(PendingEdge {
                                node: switch,
                                slot: PendingSlot::SwitchCase(value),
                            });
                        }
                    }
                }
            }
            exits.extend(fragment.exits);
        }

        let TempNodeKind::IntegerSwitch { cases, default, .. } = &mut self.nodes[switch].kind
        else {
            unreachable!("switch retained its node kind")
        };
        if default.is_none()
            && !exits
                .iter()
                .any(|edge| edge.node == switch && matches!(edge.slot, PendingSlot::SwitchDefault))
        {
            return Err(syn::Error::new_spanned(
                expression,
                "integer switch requires a final wildcard default arm",
            ));
        }
        cases.sort_unstable_by(|left, right| ty.compare(left.0, right.0));
        if let Some(pair) = cases
            .windows(2)
            .find(|pair| ty.compare(pair[0].0, pair[1].0).is_eq())
        {
            return Err(syn::Error::new_spanned(
                expression,
                format!("integer switch duplicates case bits {:#034x}", pair[1].0),
            ));
        }
        if cases.len() > MAX_CASES_V1 {
            return Err(syn::Error::new_spanned(
                expression,
                "integer switch exceeds 256 canonical cases",
            ));
        }
        Ok(Fragment {
            entry: Some(switch),
            exits,
        })
    }

    fn resolve_loop(&self, label: Option<&syn::Lifetime>, span: Span) -> syn::Result<usize> {
        let index = match label {
            Some(label) => {
                let name = label.ident.to_string();
                self.loops
                    .iter()
                    .rposition(|context| context.label.as_deref() == Some(name.as_str()))
            }
            None => self.loops.len().checked_sub(1),
        };
        index.ok_or_else(|| {
            syn::Error::new(
                span,
                "break or continue must target a lexically enclosing kernel loop",
            )
        })
    }

    fn simple(&mut self, span: Span) -> syn::Result<Fragment> {
        let node = self.push_node(span, TempNodeKind::Block { target: None })?;
        Ok(Fragment {
            entry: Some(node),
            exits: vec![PendingEdge {
                node,
                slot: PendingSlot::BlockTarget,
            }],
        })
    }

    fn sequence(&mut self, left: Fragment, right: Fragment, span: Span) -> syn::Result<Fragment> {
        match (left.entry, right.entry) {
            (None, _) => Ok(right),
            (Some(_), None) => Ok(left),
            (Some(entry), Some(right_entry)) => {
                if left.exits.is_empty() {
                    return Err(syn::Error::new(
                        span,
                        "unreachable source control flow is unsupported in the V1 kernel sidecar",
                    ));
                }
                self.patch_all(left.exits, right_entry)?;
                Ok(Fragment {
                    entry: Some(entry),
                    exits: right.exits,
                })
            }
        }
    }

    fn bind_fragment_entry(
        &mut self,
        node: usize,
        slot: PendingSlot,
        target: Option<usize>,
        exits: &mut Vec<PendingEdge>,
    ) -> syn::Result<()> {
        let edge = PendingEdge { node, slot };
        if let Some(target) = target {
            self.patch(edge, target)
        } else {
            exits.push(edge);
            Ok(())
        }
    }

    fn push_node(&mut self, span: Span, kind: TempNodeKind) -> syn::Result<usize> {
        if self.nodes.len() >= MAX_NODES_V1 {
            return Err(syn::Error::new(
                span,
                "kernel control flow exceeds 4096 source nodes",
            ));
        }
        let index = self.nodes.len();
        self.nodes.push(TempNode {
            span: SourceSpanV1::from_span(span)?,
            kind,
        });
        Ok(index)
    }

    fn patch_all(&mut self, edges: Vec<PendingEdge>, target: usize) -> syn::Result<()> {
        for edge in edges {
            self.patch(edge, target)?;
        }
        Ok(())
    }

    fn patch(&mut self, edge: PendingEdge, target: usize) -> syn::Result<()> {
        let slot = match (&mut self.nodes[edge.node].kind, edge.slot) {
            (TempNodeKind::Entry { target }, PendingSlot::EntryTarget)
            | (TempNodeKind::Block { target }, PendingSlot::BlockTarget)
            | (TempNodeKind::Loop { exit: target, .. }, PendingSlot::LoopExit)
            | (TempNodeKind::Break { target, .. }, PendingSlot::BreakTarget)
            | (
                TempNodeKind::Branch {
                    then_target: target,
                    ..
                },
                PendingSlot::BranchThen,
            )
            | (
                TempNodeKind::Branch {
                    else_target: target,
                    ..
                },
                PendingSlot::BranchElse,
            )
            | (
                TempNodeKind::IntegerSwitch {
                    default: target, ..
                },
                PendingSlot::SwitchDefault,
            ) => target,
            (TempNodeKind::IntegerSwitch { cases, .. }, PendingSlot::SwitchCase(bits)) => {
                &mut cases
                    .iter_mut()
                    .find(|case| case.0 == bits)
                    .ok_or_else(|| syn::Error::new(Span::call_site(), "missing switch case slot"))?
                    .1
            }
            _ => {
                return Err(syn::Error::new(
                    Span::call_site(),
                    "internal control-flow edge kind mismatch",
                ));
            }
        };
        if slot.replace(target).is_some() {
            return Err(syn::Error::new(
                Span::call_site(),
                "control-flow edge was assigned more than once",
            ));
        }
        Ok(())
    }

    fn encode(&self) -> syn::Result<Vec<u8>> {
        let mut writer = Writer::new();
        writer.bytes(&CONTROL_FLOW_CONTRACT_MAGIC_V1);
        writer.u16(CONTROL_FLOW_CONTRACT_VERSION_V1);
        writer.u16(0);
        writer.u32(0);
        writer.u32(u32::try_from(self.nodes.len()).expect("V1 node limit fits u32"));
        writer.u32(0);
        writer.u32(0);
        for (id, node) in self.nodes.iter().enumerate() {
            writer.u32(u32::try_from(id).expect("V1 node limit fits u32"));
            encode_span(&mut writer, &node.span)?;
            encode_node_kind(&mut writer, &node.kind)?;
        }
        if writer.bytes.len() > MAX_BYTES_V1 {
            return Err(syn::Error::new(
                Span::call_site(),
                "kernel control-flow sidecar exceeds 1 MiB",
            ));
        }
        let length = u32::try_from(writer.bytes.len()).expect("1 MiB fits u32");
        writer.bytes[12..16].copy_from_slice(&length.to_le_bytes());
        Ok(writer.bytes)
    }
}

fn reject_nested_control_flow(expression: &Expr) -> syn::Result<()> {
    let mut visitor = DirectControlFlowUseVisitor::default();
    visitor.visit_expr(expression);
    if let Some((span, kind)) = visitor.first {
        return Err(syn::Error::new(
            span,
            format!(
                "{kind} nested in this expression position is unsupported by the V1 kernel sidecar"
            ),
        ));
    }
    let mut macro_visitor = OpaqueMacroVisitor::default();
    macro_visitor.visit_expr(expression);
    if let Some(span) = macro_visitor.first {
        return Err(syn::Error::new(
            span,
            "expression macros are opaque to the V1 kernel control-flow sidecar",
        ));
    }
    Ok(())
}

#[derive(Default)]
struct OpaqueMacroVisitor {
    first: Option<Span>,
}

impl<'ast> Visit<'ast> for OpaqueMacroVisitor {
    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        let is_assembly = invocation
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "asm");
        if !is_assembly && self.first.is_none() {
            self.first = Some(invocation.span());
        }
    }
}

enum ParsedArmPattern {
    Default,
    Cases(Vec<u128>),
}

fn parse_arm_pattern(
    pattern: &Pat,
    ty: ParsedIntegerSwitchTypeV1,
) -> syn::Result<ParsedArmPattern> {
    match pattern {
        Pat::Wild(_) => Ok(ParsedArmPattern::Default),
        Pat::Paren(pattern) => parse_arm_pattern(&pattern.pat, ty),
        Pat::Or(pattern) => {
            let mut cases = Vec::new();
            for case in &pattern.cases {
                match parse_arm_pattern(case, ty)? {
                    ParsedArmPattern::Cases(values) => cases.extend(values),
                    ParsedArmPattern::Default => {
                        return Err(syn::Error::new_spanned(
                            case,
                            "wildcards cannot be mixed into an integer switch or-pattern",
                        ));
                    }
                }
            }
            Ok(ParsedArmPattern::Cases(cases))
        }
        Pat::Lit(_) => Ok(ParsedArmPattern::Cases(vec![parse_case_pattern(
            pattern, ty,
        )?])),
        Pat::Range(_) => Err(syn::Error::new_spanned(
            pattern,
            "integer switch range patterns are unsupported in V1; enumerate bounded cases",
        )),
        _ => Err(syn::Error::new_spanned(
            pattern,
            "match is not a fixed-width integer switch supported by control_flow V1",
        )),
    }
}

fn parse_case_pattern(pattern: &Pat, ty: ParsedIntegerSwitchTypeV1) -> syn::Result<u128> {
    let mut text = pattern
        .to_token_stream()
        .to_string()
        .replace([' ', '_'], "");
    let mut explicit_suffix = None;
    for suffix in [
        "i128", "u128", "i64", "u64", "i32", "u32", "i16", "u16", "i8", "u8", "isize", "usize",
    ] {
        if text.ends_with(suffix) {
            explicit_suffix = Some(suffix);
            text.truncate(text.len() - suffix.len());
            break;
        }
    }
    if let Some(suffix) = explicit_suffix
        && suffix != ty.name()
    {
        return Err(syn::Error::new_spanned(
            pattern,
            format!(
                "integer switch case suffix `{suffix}` does not match declared `{}`",
                ty.name()
            ),
        ));
    }
    let negative = text.starts_with('-');
    if negative {
        text.remove(0);
    }
    let magnitude = parse_unsigned_literal(&text).ok_or_else(|| {
        syn::Error::new_spanned(pattern, "integer switch cases must be integer literals")
    })?;
    let bits = if negative {
        if !ty.signed || magnitude > (1_u128 << (ty.width - 1)) {
            return Err(syn::Error::new_spanned(
                pattern,
                "negative integer switch case is outside its declared type",
            ));
        }
        0_u128.wrapping_sub(magnitude)
    } else {
        magnitude
    };
    if !ty.accepts_bits(bits) {
        return Err(syn::Error::new_spanned(
            pattern,
            "integer switch case is outside its declared type",
        ));
    }
    Ok(bits)
}

fn parse_unsigned_literal(text: &str) -> Option<u128> {
    if let Some(value) = text.strip_prefix("0x") {
        u128::from_str_radix(value, 16).ok()
    } else if let Some(value) = text.strip_prefix("0o") {
        u128::from_str_radix(value, 8).ok()
    } else if let Some(value) = text.strip_prefix("0b") {
        u128::from_str_radix(value, 2).ok()
    } else {
        text.parse().ok()
    }
}

fn encode_span(writer: &mut Writer, span: &SourceSpanV1) -> syn::Result<()> {
    writer.u16(u16::try_from(span.file.len()).map_err(|_| {
        syn::Error::new(
            Span::call_site(),
            "source file length exceeds the V1 wire field",
        )
    })?);
    writer.u16(0);
    writer.bytes(span.file.as_bytes());
    writer.u32(span.start_line);
    writer.u32(span.start_column);
    writer.u32(span.end_line);
    writer.u32(span.end_column);
    Ok(())
}

fn encode_node_kind(writer: &mut Writer, kind: &TempNodeKind) -> syn::Result<()> {
    match kind {
        TempNodeKind::Entry { target } => {
            writer.u16(1);
            writer.u16(0);
            writer.node(*target)?;
        }
        TempNodeKind::Block { target } => {
            writer.u16(2);
            writer.u16(0);
            writer.node(*target)?;
        }
        TempNodeKind::Exit => {
            writer.u16(3);
            writer.u16(0);
        }
        TempNodeKind::Branch {
            then_target,
            else_target,
        } => {
            writer.u16(4);
            writer.u16(0);
            writer.node(*then_target)?;
            writer.node(*else_target)?;
        }
        TempNodeKind::Loop {
            max_iterations,
            body,
            exit,
        } => {
            writer.u16(5);
            writer.u16(0);
            writer.u32(*max_iterations);
            writer.node(*body)?;
            writer.node(*exit)?;
        }
        TempNodeKind::Break {
            loop_header,
            target,
        } => {
            writer.u16(6);
            writer.u16(0);
            writer.usize_node(*loop_header)?;
            writer.node(*target)?;
        }
        TempNodeKind::Continue {
            loop_header,
            target,
        } => {
            writer.u16(7);
            writer.u16(0);
            writer.usize_node(*loop_header)?;
            writer.usize_node(*target)?;
        }
        TempNodeKind::IntegerSwitch { ty, cases, default } => {
            writer.u16(8);
            writer.u16(0);
            writer.u16(ty.width);
            writer.u8(u8::from(ty.signed));
            writer.u8(0);
            writer.u16(u16::try_from(cases.len()).map_err(|_| {
                syn::Error::new(Span::call_site(), "integer switch case count exceeds V1")
            })?);
            writer.u16(0);
            writer.node(*default)?;
            for (bits, target) in cases {
                writer.u128(*bits);
                writer.node(*target)?;
            }
        }
    }
    Ok(())
}

struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn bytes(&mut self, value: &[u8]) {
        self.bytes.extend_from_slice(value);
    }

    fn usize_node(&mut self, value: usize) -> syn::Result<()> {
        self.u32(u32::try_from(value).map_err(|_| {
            syn::Error::new(
                Span::call_site(),
                "control-flow node ID exceeds the V1 wire field",
            )
        })?);
        Ok(())
    }

    fn node(&mut self, value: Option<usize>) -> syn::Result<()> {
        self.usize_node(value.ok_or_else(|| {
            syn::Error::new(
                Span::call_site(),
                "control-flow graph contains an unbound edge",
            )
        })?)
    }
}

#[cfg(test)]
mod tests {
    use quote::quote;
    use syn::parse_quote;

    use super::*;

    #[test]
    fn declarations_are_ordered_and_fixed_width() {
        let options = parse_control_flow_options_v1(&parse_quote!(control_flow(
            loop_bounds(4, 16),
            integer_switches(u32, i8)
        )))
        .unwrap();
        assert_eq!(options.loop_bounds, vec![4, 16]);
        assert_eq!(options.integer_switches[0].name(), "u32");
        assert_eq!(options.integer_switches[1].name(), "i8");

        for invalid in [
            quote!(control_flow()),
            quote!(control_flow(loop_bounds(0))),
            quote!(control_flow(integer_switches(usize))),
            quote!(control_flow(other(1))),
        ] {
            let list = syn::parse2::<MetaList>(invalid).unwrap();
            assert!(parse_control_flow_options_v1(&list).is_err());
        }
    }

    #[test]
    fn structured_kernel_emits_canonical_sidecar_bytes() {
        let input: ItemFn = parse_quote! {
            fn kernel(mut value: u32) {
                'outer: while value < 8 {
                    value = match value {
                        0 | 1 => value + 1,
                        2 => continue 'outer,
                        _ => break 'outer,
                    };
                }
            }
        };
        let declaration = parse_control_flow_options_v1(&parse_quote!(control_flow(
            loop_bounds(8),
            integer_switches(u32)
        )))
        .unwrap();
        let first = analyze_kernel_control_flow_v1(&input, Some(&declaration))
            .unwrap()
            .unwrap();
        let second = analyze_kernel_control_flow_v1(&input, Some(&declaration))
            .unwrap()
            .unwrap();
        assert_eq!(first, second);
        assert_eq!(&first[..8], &CONTROL_FLOW_CONTRACT_MAGIC_V1);
        assert_eq!(
            u32::from_le_bytes(first[12..16].try_into().unwrap()) as usize,
            first.len()
        );
    }

    #[test]
    fn missing_and_mismatched_declarations_fail_closed() {
        let unbounded: ItemFn = parse_quote! {
            fn kernel() { loop { break; } }
        };
        assert!(
            analyze_kernel_control_flow_v1(&unbounded, None)
                .unwrap_err()
                .to_string()
                .contains("requires control_flow")
        );

        let declaration =
            parse_control_flow_options_v1(&parse_quote!(control_flow(loop_bounds(4, 8)))).unwrap();
        assert!(
            analyze_kernel_control_flow_v1(&unbounded, Some(&declaration))
                .unwrap_err()
                .to_string()
                .contains("declares 2 loop bounds")
        );

        let non_integer: ItemFn = parse_quote! {
            fn kernel(value: Option<u32>) {
                match value { Some(_) => {}, None => {} }
            }
        };
        let declaration =
            parse_control_flow_options_v1(&parse_quote!(control_flow(integer_switches(u32))))
                .unwrap();
        assert!(
            analyze_kernel_control_flow_v1(&non_integer, Some(&declaration))
                .unwrap_err()
                .to_string()
                .contains("not a fixed-width integer switch")
        );
    }

    #[test]
    fn literal_half_open_for_is_unrolled_with_lexical_break_and_continue() {
        let mut input: ItemFn = parse_quote! {
            fn kernel(mut sum: u32) {
                for i in 2u32..6u32 {
                    if i == 3 { continue; }
                    if i == 5 { break; }
                    sum += i;
                }
            }
        };
        let declaration =
            parse_control_flow_options_v1(&parse_quote!(control_flow(loop_bounds(4)))).unwrap();
        let original_contract = analyze_kernel_control_flow_v1(&input, Some(&declaration))
            .unwrap()
            .unwrap();

        lower_bounded_for_loops_v1(&mut input, Some(&declaration)).unwrap();

        let lowered = quote!(#input).to_string();
        assert!(!lowered.contains("for i in"), "{lowered}");
        assert!(!lowered.contains("continue"), "{lowered}");
        assert_eq!(lowered.matches("let i =").count(), 4, "{lowered}");
        assert!(lowered.contains("'__fe2o3_unrolled_for_0"), "{lowered}");
        assert_eq!(&original_contract[..8], &CONTROL_FLOW_CONTRACT_MAGIC_V1);
    }

    #[test]
    fn literal_for_preserves_an_end_owned_element_suffix() {
        let mut input: ItemFn = parse_quote! {
            fn kernel(mut output: u64) {
                for i in 0..2u64 {
                    output ^= i.wrapping_sub(1);
                }
            }
        };
        let declaration =
            parse_control_flow_options_v1(&parse_quote!(control_flow(loop_bounds(2)))).unwrap();

        lower_bounded_for_loops_v1(&mut input, Some(&declaration)).unwrap();

        let lowered = quote!(#input).to_string();
        assert!(lowered.contains("let i = 0u64"), "{lowered}");
        assert!(lowered.contains("let i = 1u64"), "{lowered}");

        let mut mismatched: ItemFn = parse_quote! {
            fn kernel() { for i in 0u32..2u64 { let _ = i; } }
        };
        assert!(
            lower_bounded_for_loops_v1(&mut mismatched, Some(&declaration))
                .unwrap_err()
                .to_string()
                .contains("identical explicit integer suffixes")
        );
    }

    #[test]
    fn unsupported_for_unroll_shapes_fail_closed() {
        let cases: Vec<(ItemFn, Vec<u32>, &str)> = vec![
            (
                parse_quote! { fn kernel(end: u32) { for i in 0..end { let _ = i; } } },
                vec![4],
                "literal range endpoints",
            ),
            (
                parse_quote! { fn kernel() { for i in 0..=4 { let _ = i; } } },
                vec![5],
                "inclusive ranges are unsupported",
            ),
            (
                parse_quote! { fn kernel() { for i in 0..5 { let _ = i; } } },
                vec![4],
                "exceeding its declared control_flow bound 4",
            ),
            (
                parse_quote! { fn kernel() { for i in 0..33 { let _ = i; } } },
                vec![33],
                "supports at most 32 iterations",
            ),
            (
                parse_quote! {
                    fn kernel() {
                        for i in 0..2 {
                            while i == 0 { break; }
                        }
                    }
                },
                vec![2, 1],
                "does not support nested loops",
            ),
            (
                parse_quote! {
                    fn kernel() { 'outer: for i in 0..2 { let _ = i; break 'outer; } }
                },
                vec![2],
                "does not support labeled loops",
            ),
        ];

        for (mut input, bounds, expected) in cases {
            let declaration = ParsedControlFlowOptionsV1 {
                loop_bounds: bounds,
                integer_switches: Vec::new(),
            };
            analyze_kernel_control_flow_v1(&input, Some(&declaration)).unwrap();
            let error = lower_bounded_for_loops_v1(&mut input, Some(&declaration)).unwrap_err();
            assert!(
                error.to_string().contains(expected),
                "missing `{expected}` in `{error}`"
            );
        }
    }
}
