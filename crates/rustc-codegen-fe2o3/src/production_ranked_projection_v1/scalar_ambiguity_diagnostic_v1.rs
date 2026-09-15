//! Inert failure detail. Never reconstructs SSA events or supplies a scalar.
use fe2o3_mir_model::{SemanticExpandedRootV1, SsaConstructionPlanV1, SsaResolvedEventV1};
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::ProductionSemanticSsaOwnerV1;
use std::fmt::{self, Write};

const MAX_WORK: usize = 16_384;
const MAX_ROWS: usize = 8;
const MAX_BYTES: usize = 8_192;

#[derive(Clone, Copy)]
pub(super) struct AmbiguousUse<'a> {
    pub(super) local: u32,
    pub(super) place: &'a SemanticPlaceV1,
}

pub(super) struct Context<'a> {
    function: &'a SemanticFunctionDeclV1,
    plan: &'a SsaConstructionPlanV1,
    view: Option<&'a SemanticExpandedRootV1>,
}

impl<'a> Context<'a> {
    pub(super) fn for_owner(
        owner: &'a ProductionSemanticSsaOwnerV1,
        root: SemanticFunctionIdV1,
        function: &'a SemanticFunctionDeclV1,
    ) -> Option<Self> {
        let view = owner.execution_view_for_root(root)?;
        let plan = owner.execution_plan_for_root(root)?;
        if !same_function(function, view.body(), plan.function_identity())
            || plan.function() != view.source_body()
        {
            return None;
        }
        Some(Self {
            function,
            plan: plan.plan(),
            view: Some(view),
        })
    }

    pub(super) fn describe(
        &self,
        ranked_block: usize,
        operation: usize,
        write_site: Option<(usize, Option<usize>)>,
        ambiguity: AmbiguousUse<'_>,
    ) -> String {
        self.describe_with_limit(ranked_block, operation, write_site, ambiguity, MAX_WORK)
    }

    fn describe_with_limit(
        &self,
        ranked_block: usize,
        operation: usize,
        write_site: Option<(usize, Option<usize>)>,
        ambiguity: AmbiguousUse<'_>,
        limit: usize,
    ) -> String {
        let mut output = Text::default();
        let mut work = Work {
            remaining: limit.min(MAX_WORK),
            visited: 0,
            truncated: false,
        };
        let _ = write!(
            output,
            "ranked=bb{ranked_block}:op{operation} execution-write={write_site:?} local={} type={} function=",
            ambiguity.local,
            ambiguity.place.ty().index()
        );
        for byte in self.function.identity().as_bytes() {
            let _ = write!(output, "{byte:02x}");
        }
        let _ = write!(output, " plan={}", self.plan.identity());
        if let Some(view) = self.view {
            let _ = write!(
                output,
                " root={} local-origin={:?}",
                view.root().index(),
                view.local_origins().get(ambiguity.local as usize)
            );
        }
        if let Some((block, statement)) = write_site {
            self.origin(&mut output, block, statement);
        }
        let mut definitions = 0;
        let mut occurrences = 0;
        'source: for (block_index, block) in self.function.blocks().iter().enumerate() {
            if !work.step() {
                break;
            }
            for (statement_index, statement) in block.statements().iter().enumerate() {
                if !work.step() {
                    break 'source;
                }
                if let SemanticStatementKindV1::Assign(assignment) = statement.kind() {
                    if assignment.destination().projections().is_empty()
                        && assignment.destination().local().index() == ambiguity.local
                    {
                        definitions += 1;
                        if definitions <= MAX_ROWS {
                            let _ = write!(
                                output,
                                "\n  historical-def=bb{block_index}:s{statement_index} value={} type={}",
                                rvalue_kind(assignment.value().kind()),
                                assignment.value().result_type().index()
                            );
                            self.origin(&mut output, block_index, Some(statement_index));
                        }
                    }
                }
                let matched = match statement.kind() {
                    SemanticStatementKindV1::Assign(assignment) => {
                        let mut matched = false;
                        let result = assignment.value().kind().try_visit_operands(|operand| {
                            if !work.step() {
                                return Err(());
                            }
                            matched |= exact_operand(operand, ambiguity.place);
                            Ok(())
                        });
                        if result.is_err() {
                            break 'source;
                        }
                        matched
                    }
                    SemanticStatementKindV1::Store(store) => {
                        exact_operand(store.value(), ambiguity.place)
                    }
                    _ => false,
                };
                if matched {
                    occurrences += 1;
                    if occurrences <= MAX_ROWS {
                        let _ = write!(output, "\n  exact-use=bb{block_index}:s{statement_index}");
                        self.origin(&mut output, block_index, Some(statement_index));
                    }
                }
            }
            if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() {
                for (argument, operand) in call.arguments().iter().enumerate() {
                    if !work.step() {
                        break 'source;
                    }
                    if exact_operand(operand, ambiguity.place) {
                        occurrences += 1;
                        if occurrences <= MAX_ROWS {
                            let _ = write!(
                                output,
                                "\n  exact-use=bb{block_index}:terminator:argument{argument}"
                            );
                            self.origin(&mut output, block_index, None);
                        }
                    }
                }
            }
        }
        // These are actual planner event coordinates. Adapter statement ranges
        // are currently transient; never infer their endpoints by re-counting.
        let mut events = 0;
        'ssa: for block in self.plan.reverse_postorder() {
            if !work.step() {
                break;
            }
            let Some(resolved) = self.plan.resolved_events(*block) else {
                continue;
            };
            for (event, resolved) in resolved {
                if !work.step() {
                    break 'ssa;
                }
                let variable = match resolved {
                    SsaResolvedEventV1::Use { variable, .. }
                    | SsaResolvedEventV1::Define { variable, .. }
                    | SsaResolvedEventV1::Kill { variable, .. } => variable.get(),
                };
                if variable != ambiguity.local {
                    continue;
                }
                events += 1;
                if events <= MAX_ROWS {
                    let _ = write!(
                        output,
                        "\n  ssa-event=bb{}:e{event} {resolved:?}",
                        block.get()
                    );
                    if let Some(origin) = self
                        .view
                        .and_then(|view| view.block_origins().get(block.get() as usize))
                    {
                        let _ = write!(
                            output,
                            " source-block=instance{}:fn{}:bb{}",
                            origin.instance().index(),
                            origin.function().index(),
                            origin.block().index()
                        );
                    }
                }
            }
        }
        let _ = write!(
            output,
            "\n  ssa-statement-mapping=unavailable(adapter-endpoints-not-retained) historical-defs-seen={definitions} exact-use-occurrences-seen={occurrences} ssa-events-seen={events} scan={} visited={} samples-capped={}",
            if work.truncated {
                "lower-bound"
            } else {
                "complete"
            },
            work.visited,
            definitions > MAX_ROWS || occurrences > MAX_ROWS || events > MAX_ROWS
        );
        output.finish()
    }

    fn origin(&self, output: &mut Text, block: usize, statement: Option<usize>) {
        let Some(origin) = self.view.and_then(|view| view.block_origins().get(block)) else {
            return;
        };
        let _ = write!(
            output,
            " source=instance{}:fn{}:bb{}",
            origin.instance().index(),
            origin.function().index(),
            origin.block().index()
        );
        if let Some(statement) = statement {
            let _ = write!(output, ":{:?}", origin.statements().get(statement));
        } else {
            let _ = write!(output, ":{:?}", origin.terminator());
        }
    }
}

fn same_function(
    function: &SemanticFunctionDeclV1,
    retained: &SemanticFunctionDeclV1,
    identity: SemanticFunctionIdentityV1,
) -> bool {
    std::ptr::eq(function, retained) && function.identity() == identity
}

fn exact_operand(operand: &SemanticOperandV1, place: &SemanticPlaceV1) -> bool {
    match operand {
        SemanticOperandV1::Copy(actual) | SemanticOperandV1::Move(actual) => {
            std::ptr::eq(actual, place)
        }
        SemanticOperandV1::Constant(_) => false,
    }
}

fn rvalue_kind(value: &SemanticRvalueKindV1) -> &'static str {
    match value {
        SemanticRvalueKindV1::Use(_) => "use",
        SemanticRvalueKindV1::Binary { .. } => "binary",
        SemanticRvalueKindV1::CheckedBinary(_) => "checked-binary",
        SemanticRvalueKindV1::UncheckedBinary(_) => "unchecked-binary",
        SemanticRvalueKindV1::Unary { .. } => "unary",
        SemanticRvalueKindV1::Cast { .. } => "cast",
        SemanticRvalueKindV1::Aggregate(_) => "aggregate",
        SemanticRvalueKindV1::Load(_) => "load",
        SemanticRvalueKindV1::Borrow { .. } => "borrow",
        SemanticRvalueKindV1::AddressOf { .. } => "address-of",
        SemanticRvalueKindV1::Length(_) => "length",
        SemanticRvalueKindV1::Discriminant(_) => "discriminant",
    }
}

struct Work {
    remaining: usize,
    visited: usize,
    truncated: bool,
}
impl Work {
    fn step(&mut self) -> bool {
        if self.remaining == 0 {
            self.truncated = true;
            return false;
        }
        self.remaining -= 1;
        self.visited += 1;
        true
    }
}

#[derive(Default)]
struct Text {
    text: String,
    truncated: bool,
}
impl Write for Text {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if self.truncated || text.len() > (MAX_BYTES - 64).saturating_sub(self.text.len()) {
            self.truncated = true;
            return Err(fmt::Error);
        }
        self.text.push_str(text);
        Ok(())
    }
}
impl Text {
    fn finish(mut self) -> String {
        if self.truncated {
            self.text.push_str("\n  diagnostic-text-truncated=true");
        }
        self.text
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod resolver_tests;
