use super::*;
use fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1;
use std::collections::BTreeSet;

// Failure-only test diagnostics. These bounds never participate in admission.
const MAX_LOCALS: usize = 32;
const MAX_LINES: usize = 96;
const MAX_LINE_CHARS: usize = 1_024;

pub(super) struct FailureContext(Vec<(u32, u32, Vec<String>)>);

fn push(lines: &mut Vec<String>, value: impl std::fmt::Debug) {
    if lines.len() < MAX_LINES {
        let text = format!("{value:?}");
        let mut line = text.chars().take(MAX_LINE_CHARS).collect::<String>();
        if line.len() < text.len() {
            line.push_str(" [diagnostic truncated]");
        }
        lines.push(line);
    }
}

impl FailureContext {
    pub(super) fn capture(owner: &ProductionSemanticSsaOwnerV1) -> Self {
        let mir = owner.source_semantic();
        let mut entries = Vec::new();
        for root in owner.execution_expansion().roots() {
            let body = root.body();
            for (block_index, block) in body.blocks().iter().enumerate() {
                let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                    continue;
                };
                let SemanticCallableDeclV1::CompilerIntrinsic {
                    operation:
                        SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
                    ..
                } = &mir.callables()[call.callee().index() as usize]
                else {
                    continue;
                };
                let mut lines = Vec::new();
                push(&mut lines, ("operation", contract.operation()));
                push(
                    &mut lines,
                    ("origin", &root.block_origins()[block_index]),
                );
                let mut pending = Vec::new();
                for (index, argument) in call.arguments().iter().enumerate() {
                    push(&mut lines, ("argument", index, argument));
                    if let SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) =
                        argument
                    {
                        pending.push(place.local());
                    }
                }
                let mut visited = BTreeSet::new();
                while let Some(local) = pending.pop() {
                    if visited.len() == MAX_LOCALS || lines.len() == MAX_LINES {
                        break;
                    }
                    if !visited.insert(local) {
                        continue;
                    }
                    let ty = body.locals()[local.index() as usize].ty();
                    push(
                        &mut lines,
                        ("local", local, ty, mir.types()[ty.index() as usize].shape()),
                    );
                    for (source_index, source) in body.blocks().iter().enumerate() {
                        for (index, statement) in source.statements().iter().enumerate() {
                            let SemanticStatementKindV1::Assign(assignment) = statement.kind()
                            else {
                                continue;
                            };
                            if assignment.destination().local() != local {
                                continue;
                            }
                            push(&mut lines, ("definition", source_index, index, assignment));
                            match assignment.value().kind() {
                                SemanticRvalueKindV1::Use(
                                    SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place),
                                )
                                | SemanticRvalueKindV1::Borrow { place, .. } => {
                                    pending.push(place.local())
                                }
                                _ => {}
                            }
                        }
                        if let SemanticTerminatorKindV1::Call(producer) = source.terminator().kind()
                            && producer
                                .destination()
                                .is_some_and(|destination| destination.place().local() == local)
                        {
                            push(&mut lines, ("call definition", source_index, producer));
                            if let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } =
                                &mir.callables()[producer.callee().index() as usize]
                            {
                                push(&mut lines, ("producer operation", operation));
                            }
                        }
                    }
                }
                entries.push((root.root().index(), block_index as u32, lines));
            }
        }
        Self(entries)
    }

    pub(super) fn report(&self, error: &ProductionSemanticKirErrorV1) {
        let ProductionSemanticKirErrorV1::Unsupported {
            function,
            block: Some(block),
            ..
        } = error
        else {
            return;
        };
        if let Some((_, _, lines)) = self.0.iter().find(|(f, b, _)| f == function && b == block) {
            for line in lines {
                eprintln!("borrowed lowering fn{function} bb{block}: {line}");
            }
        }
    }
}
