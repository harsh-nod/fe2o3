//! Closed same-source context for independently built scalar helper templates.
use super::helper_value_template_v1::{Ledger, Meter, Template, vector};
use super::source_helper_value_templates_v1 as source;
use super::*;

#[cfg(test)]
#[path = "source_helper_value_context_v1_tests.rs"]
mod tests;

type Error = &'static str;

struct Pending {
    function: usize,
    block: usize,
    invalid: bool,
}

/// Only the scope constructor below creates this type. Each indexed template
/// was derived from that exact borrowed semantic function, not a supplied row.
pub(super) struct SourceHelperValues<'a> {
    semantic: &'a AdmittedInertSemanticMirV1,
    root: usize,
    templates: Vec<Option<Template>>,
    headers: usize,
    ledger: Ledger,
    live_floor: usize,
}

impl SourceHelperValues<'_> {
    fn check_live(&self, meter: &mut dyn Meter) -> Result<(), Error> {
        if self.ledger != meter.identity()? {
            return Err("helper template foreign ledger");
        }
        if meter.storage()? < self.live_floor {
            return Err("helper template live storage floor lost");
        }
        Ok(())
    }

    fn destroy(self, meter: &mut dyn Meter) -> Result<(), Error> {
        self.check_live(meter)?;
        let Self {
            templates, headers, ..
        } = self;
        for template in templates.into_iter().flatten() {
            template.destroy(meter)?;
        }
        meter.release(headers)
    }

    /// Checks the exact original Call occurrence before returning a borrowed
    /// template. The root resolver separately proves unique live destination
    /// dominance and evaluates these exact operands at this occurrence.
    pub(super) fn call<'s>(
        &'s self,
        semantic: &AdmittedInertSemanticMirV1,
        function: &SemanticFunctionDeclV1,
        block: usize,
        call: &SemanticDirectCallV1,
        meter: &mut dyn Meter,
    ) -> Result<&'s Template, Error> {
        self.check_live(meter)?;
        meter.work(8)?;
        if !std::ptr::eq(self.semantic, semantic)
            || self
                .semantic
                .functions()
                .get(self.root)
                .is_none_or(|actual| !std::ptr::eq(actual, function))
        {
            return Err("helper template source owner or root substitution");
        }
        let Some(SemanticTerminatorKindV1::Call(actual)) =
            function.blocks().get(block).map(|b| b.terminator().kind())
        else {
            return Err("helper value has no exact source call occurrence");
        };
        if !std::ptr::eq(actual, call) {
            return Err("helper value source call substitution");
        }
        let Some(SemanticCallableDeclV1::Defined { function: callee }) = self
            .semantic
            .callables()
            .get(call.callee().index() as usize)
        else {
            return Err("helper value call target is not Defined");
        };
        let index = callee.index() as usize;
        let function = self
            .semantic
            .functions()
            .get(index)
            .ok_or("helper target outside source owner")?;
        let template = self
            .templates
            .get(index)
            .and_then(Option::as_ref)
            .ok_or("unresolved helper value recipe")?;
        source::abi(self.semantic.types(), function.abi(), meter)?;
        let destination = call
            .destination()
            .ok_or("helper result has no destination")?;
        if !call.variadic_argument_abis().is_empty()
            || !matches!(
                call.unwind(),
                SemanticUnwindActionV1::Continue | SemanticUnwindActionV1::Unreachable
            )
            || destination.edge().role() != SemanticEdgeRoleV1::CallReturn
            || !destination.place().projections().is_empty()
            || destination.place().ty() != function.abi().source_output_type()
            || call.arguments().len() != function.abi().source_input_types().len()
        {
            return Err("helper call ABI, destination or return edge mismatch");
        }
        for (argument, expected) in call
            .arguments()
            .iter()
            .zip(function.abi().source_input_types())
        {
            meter.work(1)?;
            if argument.ty() != *expected {
                return Err("helper call operand type mismatch");
            }
        }
        Ok(template)
    }
}

fn child(semantic: &AdmittedInertSemanticMirV1, kind: &SemanticTerminatorKindV1) -> Option<usize> {
    let SemanticTerminatorKindV1::Call(call) = kind else {
        return None;
    };
    match semantic.callables().get(call.callee().index() as usize) {
        Some(SemanticCallableDeclV1::Defined { function }) => Some(function.index() as usize),
        _ => None,
    }
}

fn build(
    semantic: &AdmittedInertSemanticMirV1,
    root: usize,
    context: &mut SourceHelperValues<'_>,
    meter: &mut dyn Meter,
) -> Result<(), Error> {
    let functions = semantic.functions();
    let function = functions
        .get(root)
        .ok_or("helper root outside exact source owner")?;
    let count = functions.len();
    let (mut states, state_storage) = vector(count, meter)?;
    states.resize(count, 0u8);
    let (mut pending, pending_storage) = match vector::<Pending>(count, meter) {
        Ok(value) => value,
        Err(error) => {
            drop(states);
            meter.release(state_storage)?;
            return Err(error);
        }
    };
    let result = (|| {
        for block in function.blocks() {
            meter.work(2)?;
            let Some(start) = child(semantic, block.terminator().kind()) else {
                continue;
            };
            let state = states
                .get_mut(start)
                .ok_or("helper target outside source owner")?;
            if *state != 0 {
                continue;
            }
            *state = 1;
            pending.push(Pending {
                function: start,
                block: 0,
                invalid: false,
            });
            while let Some(frame) = pending.last_mut() {
                meter.work(5)?;
                let function = &functions[frame.function];
                if let Some(block) = function.blocks().get(frame.block) {
                    frame.block += 1;
                    let Some(next) = child(semantic, block.terminator().kind()) else {
                        continue;
                    };
                    match states
                        .get(next)
                        .copied()
                        .ok_or("helper target outside source owner")?
                    {
                        0 => {
                            states[next] = 1;
                            // Every white function is pushed exactly once. The
                            // paid capacity is the complete source function count.
                            pending.push(Pending {
                                function: next,
                                block: 0,
                                invalid: false,
                            });
                        }
                        1 | 3 => frame.invalid = true,
                        2 => {}
                        _ => return Err("invalid helper traversal state"),
                    }
                    continue;
                }
                let finished = pending.pop().ok_or("helper traversal stack underflow")?;
                let template = if finished.invalid {
                    None
                } else {
                    match source::derive(semantic, finished.function, &context.templates, meter) {
                        Ok(template) => Some(template),
                        Err(error) if meter.exhausted() => return Err(error),
                        Err(_) => None,
                    }
                };
                let state = if template.is_some() { 2 } else { 3 };
                if let Some(template) = &template {
                    context.live_floor = context
                        .live_floor
                        .checked_add(template.retained_storage())
                        .ok_or("helper retained floor overflow")?;
                }
                states[finished.function] = state;
                context.templates[finished.function] = template;
                if state == 3
                    && let Some(parent) = pending.last_mut()
                {
                    parent.invalid = true;
                }
            }
        }
        Ok(())
    })();
    drop(pending);
    drop(states);
    meter.release(pending_storage)?;
    meter.release(state_storage)?;
    result
}

/// The callback may transfer its separately paid instantiated trees, but never
/// a template or context. The cache is dropped before its reservations end.
pub(super) fn with_source_helper_values<R>(
    semantic: &AdmittedInertSemanticMirV1,
    root: usize,
    meter: &mut dyn Meter,
    action: impl for<'s> FnOnce(&'s SourceHelperValues<'_>, &mut dyn Meter) -> Result<R, Error>,
) -> Result<R, Error> {
    let ledger = meter.identity()?;
    let floor = meter.storage()?;
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_source_helper_values_inner(semantic, root, meter, action)
    }));
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(payload) => {
            if meter.identity().ok() == Some(ledger)
                && let Ok(storage) = meter.storage()
                && let Some(additional) = storage.checked_sub(floor)
            {
                let _ = meter.release(additional);
            }
            std::panic::resume_unwind(payload)
        }
    };
    if meter.identity()? != ledger {
        return Err("helper scope caller ledger replaced");
    }
    match outcome {
        Ok(result) => {
            if meter.storage()? < floor {
                return Err("helper scope lost incoming storage floor");
            }
            Ok(result)
        }
        Err(error) => {
            let additional = meter
                .storage()?
                .checked_sub(floor)
                .ok_or("helper scope lost incoming storage floor")?;
            meter.release(additional)?;
            Err(error)
        }
    }
}

fn with_source_helper_values_inner<R>(
    semantic: &AdmittedInertSemanticMirV1,
    root: usize,
    meter: &mut dyn Meter,
    action: impl for<'s> FnOnce(&'s SourceHelperValues<'_>, &mut dyn Meter) -> Result<R, Error>,
) -> Result<R, Error> {
    let floor = meter.storage()?;
    meter.work(2)?;
    let function = semantic
        .functions()
        .get(root)
        .ok_or("helper root outside exact source owner")?;
    let mut needed = false;
    for block in function.blocks() {
        meter.work(2)?;
        if child(semantic, block.terminator().kind()).is_some() {
            needed = true;
            break;
        }
    }
    let header = std::mem::size_of::<SourceHelperValues<'_>>();
    meter.reserve(header)?;
    let (mut templates, rows) = match vector(
        if needed {
            semantic.functions().len()
        } else {
            0
        },
        meter,
    ) {
        Ok(value) => value,
        Err(error) => {
            meter.release(header)?;
            return Err(error);
        }
    };
    if needed {
        templates.resize_with(semantic.functions().len(), || None);
    }
    let mut context = SourceHelperValues {
        semantic,
        root,
        templates,
        headers: header
            .checked_add(rows)
            .ok_or("helper context header overflow")?,
        ledger: meter.identity()?,
        live_floor: floor
            .checked_add(header)
            .and_then(|floor| floor.checked_add(rows))
            .ok_or("helper retained floor overflow")?,
    };
    let result = (|| {
        if needed {
            build(semantic, root, &mut context, meter)?;
        }
        action(&context, meter)
    })();
    context.destroy(meter)?;
    result
}
